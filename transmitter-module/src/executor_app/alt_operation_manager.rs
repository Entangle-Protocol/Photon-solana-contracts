use anchor_lang::{
    prelude::{AccountMeta, Pubkey},
    AccountDeserialize, InstructionData, ToAccountMetas,
};
use futures_util::{select, FutureExt, StreamExt};
use log::*;
use photon::{photon::ROOT, protocol_data::OpStatus, OpInfo};
use solana_sdk::{
    address_lookup_table::AddressLookupTableAccount, commitment_config::CommitmentConfig,
    instruction::Instruction, signature::Keypair, signer::Signer,
};
use solana_transactor::{ix_compiler::InstructionBundle, SolanaTransactor};
use std::ops::Deref;
use tokio::sync::{
    mpsc::{UnboundedReceiver, UnboundedSender},
    Mutex,
};
use tokio_stream::wrappers::UnboundedReceiverStream;
use transmitter_common::data::{OperationData, SignedOperation};

use crate::executor_app::error::ExecutorError;

use super::{extension_manager::ExtensionManager, ServiceCmd};

// TODO: additional signatures from extensions

pub(super) struct AltOperationManager {
    op_data_receiver: Mutex<Option<UnboundedReceiverStream<SignedOperation>>>,
    last_block_sender: UnboundedSender<u64>,
    transactor: SolanaTransactor,
    extension_mng: ExtensionManager,
    executor: Keypair,
    service_receiver: Mutex<UnboundedReceiver<ServiceCmd>>,
}

#[derive(Debug)]
enum ExecutorOpStatus {
    New,
    Loaded,
    Signed,
    Executed,
}

impl AltOperationManager {
    pub fn new(
        op_data_receiver: UnboundedReceiver<SignedOperation>,
        last_block_sender: UnboundedSender<u64>,
        transactor: SolanaTransactor,
        extensions: Vec<String>,
        executor: Keypair,
        service_receiver: UnboundedReceiver<ServiceCmd>,
    ) -> Self {
        let extension_mng = ExtensionManager::new(extensions);
        let op_data_receiver: UnboundedReceiverStream<SignedOperation> =
            UnboundedReceiverStream::new(op_data_receiver);
        Self {
            op_data_receiver: Mutex::new(Some(op_data_receiver)),
            last_block_sender,
            transactor,
            extension_mng,
            executor,
            service_receiver: Mutex::new(service_receiver),
        }
    }

    pub async fn execute(&self) {
        select! {
            _ = self.execute_operations().fuse() => {}
            _ = self.listen_update().fuse() => {}
        }
    }

    async fn listen_update(&self) {
        loop {
            let service_cmd = self.service_receiver.lock().await.recv().await;
            let Some(service_cmd) = service_cmd else {
                break;
            };
            self.on_service_cmd(service_cmd).await;
        }
    }

    async fn on_service_cmd(&self, cmd: ServiceCmd) {
        match cmd {
            ServiceCmd::UpdateExtensions(x) => {
                self.extension_mng.on_update_extensions(x);
            }
        }
    }

    async fn execute_operations(&self) {
        info!("Start listen for incoming operation_data");
        let alt: Pubkey = "DqfKLmNfqxnf3rn1LJeEjGYjNhiqudpUSgXS6ChVxCq2".parse().unwrap();
        let alt = &[self
            .transactor
            .rpc_pool()
            .with_read_rpc_loop(
                |rpc| async move { solana_transactor::alt_manager::load_alt(rpc, alt).await },
                CommitmentConfig::finalized(),
            )
            .await][..];

        self.op_data_receiver
            .lock()
            .await
            .take()
            .unwrap()
            .for_each_concurrent(64, |op_data| async {
                self.last_block_sender
                    .send(op_data.eob_block_number)
                    .expect("Expected last_block_number to be sent");
                let op_hash = op_data.operation_data.op_hash_with_message();
                if let Err(e) = self.process_operation(op_hash, op_data, alt).await {
                    error!("{}: Failed to process: {}", hex::encode(op_hash), e);
                }
            })
            .await;
    }

    async fn process_operation(
        &self,
        op_hash: [u8; 32],
        op: SignedOperation,
        alt: &[AddressLookupTableAccount],
    ) -> Result<(), ExecutorError> {
        let op_hash_str = hex::encode(op_hash);
        debug!("{}, operation received", op_hash_str);
        let (op_info, _) = Pubkey::find_program_address(&[ROOT, b"OP", &op_hash], &photon::ID);
        loop {
            let op_info_data = self
                .transactor
                .rpc_pool()
                .with_read_rpc_loop(
                    |rpc| async move {
                        rpc.get_account_with_commitment(&op_info, CommitmentConfig::finalized())
                            .await
                    },
                    CommitmentConfig::finalized(),
                )
                .await
                .value;
            let op_status = match op_info_data {
                Some(acc) => match OpInfo::try_deserialize(&mut &acc.data[..]) {
                    Ok(s) => match s.status {
                        OpStatus::None => ExecutorOpStatus::New,
                        OpStatus::Init => ExecutorOpStatus::Loaded,
                        OpStatus::Signed => ExecutorOpStatus::Signed,
                        OpStatus::Executed => ExecutorOpStatus::Executed,
                    },
                    Err(e) => {
                        error!(
                            "{}. Failed to deserialize op_info, ({}) skipping...",
                            op_hash_str, e
                        );
                        return Ok(());
                    }
                },
                None => ExecutorOpStatus::New,
            };
            debug!("{}, current op status: {:?}", op_hash_str, op_status);
            let ix_bundle = match op_status {
                ExecutorOpStatus::New => [
                    build_load_ix(self.executor.pubkey(), op_hash, op.operation_data.clone())?,
                    build_sign_tx(self.executor.pubkey(), op_hash, op.clone())?,
                    build_execute_tx(
                        &self.extension_mng,
                        self.executor.pubkey(),
                        op_hash,
                        op.operation_data.clone(),
                    )?,
                ]
                .to_vec(),
                ExecutorOpStatus::Loaded => [
                    build_sign_tx(self.executor.pubkey(), op_hash, op.clone())?,
                    build_execute_tx(
                        &self.extension_mng,
                        self.executor.pubkey(),
                        op_hash,
                        op.operation_data.clone(),
                    )?,
                ]
                .to_vec(),
                ExecutorOpStatus::Signed => [build_execute_tx(
                    &self.extension_mng,
                    self.executor.pubkey(),
                    op_hash,
                    op.operation_data.clone(),
                )?]
                .to_vec(),
                ExecutorOpStatus::Executed => break,
            };
            self.transactor
                .send_all_instructions(
                    Some(op_hash_str.deref()),
                    &ix_bundle,
                    &[&self.executor],
                    self.executor.pubkey(),
                    1,
                    alt,
                    Some(1000),
                    false,
                )
                .await?;
        }
        Ok(())
    }
}

fn build_load_ix(
    executor: Pubkey,
    op_hash: [u8; 32],
    op_data: OperationData,
) -> Result<InstructionBundle, ExecutorError> {
    let op_hash_str = hex::encode(op_hash);
    let (op_info_pda, _bump) = Pubkey::find_program_address(&[ROOT, b"OP", &op_hash], &photon::ID);
    let (protocol_info_pda, _) =
        Pubkey::find_program_address(&[ROOT, b"PROTOCOL", &op_data.protocol_id.0], &photon::ID);
    let (config_pda, _) = Pubkey::find_program_address(&[ROOT, b"CONFIG"], &photon::ID);
    let accounts: Vec<AccountMeta> = photon::accounts::LoadOperation {
        executor,
        protocol_info: protocol_info_pda,
        op_info: op_info_pda,
        config: config_pda,
        system_program: anchor_lang::system_program::ID,
    }
    .to_account_metas(None);
    let protocol_id = String::from_utf8(op_data.protocol_id.0.to_vec()).map_err(|err| {
        error!("{}. Failed to get protocol id utf8: {}", op_hash_str, err);
        ExecutorError::MalformedData
    })?;
    debug!(
        "{}, Build txs for protocol_id: {}, executor: {}, protocol_info: {}, op_info: {}, config: {}",
        op_hash_str, protocol_id, executor, protocol_info_pda, op_info_pda, config_pda
    );
    let photon_op_data =
        photon::protocol_data::OperationData::try_from(op_data).map_err(|err| {
            error!(
                "{}. Failed to get op_data from op_data_message: {}",
                op_hash_str,
                hex::encode(err)
            );
            ExecutorError::MalformedData
        })?;
    let load_op_data = photon::instruction::LoadOperation {
        op_data: photon_op_data,
        op_hash_cached: op_hash.to_vec(),
    }
    .data();
    let instruction = Instruction::new_with_bytes(photon::id(), &load_op_data, accounts);
    Ok(InstructionBundle::new(instruction, 200000))
}

fn build_sign_tx(
    executor: Pubkey,
    op_hash: [u8; 32],
    op: SignedOperation,
) -> Result<InstructionBundle, ExecutorError> {
    let (op_info_pda, _bump) = Pubkey::find_program_address(&[ROOT, b"OP", &op_hash], &photon::ID);

    let (protocol_info_pda, _) = Pubkey::find_program_address(
        &[ROOT, b"PROTOCOL", &op.operation_data.protocol_id.0],
        &photon::ID,
    );

    let accounts: Vec<AccountMeta> = photon::accounts::SignOperation {
        executor,
        op_info: op_info_pda,
        protocol_info: protocol_info_pda,
    }
    .to_account_metas(None);

    let sign_op_data = photon::instruction::SignOperation {
        op_hash: op_hash.to_vec(),
        signatures: op
            .signatures
            .into_iter()
            .map(photon::protocol_data::TransmitterSignature::from)
            .collect(),
    }
    .data();
    let instruction = Instruction::new_with_bytes(photon::id(), &sign_op_data, accounts);
    Ok(InstructionBundle::new(instruction, 400000))
}

fn build_execute_tx(
    extension_mng: &ExtensionManager,
    executor: Pubkey,
    op_hash: [u8; 32],
    op_data: OperationData,
) -> Result<InstructionBundle, ExecutorError> {
    let protocol_id = op_data.protocol_id;
    let extension = extension_mng.get_extension(&protocol_id).ok_or_else(|| {
        error!("Failed to get extension by protocol_id: {}", protocol_id);
        ExecutorError::ExtensionMng
    })?;

    let (op_info_pda, _bump) = Pubkey::find_program_address(&[ROOT, b"OP", &op_hash], &photon::ID);
    let (protocol_info_pda, _) =
        Pubkey::find_program_address(&[ROOT, b"PROTOCOL", &protocol_id.0], &photon::ID);
    let (call_authority_pda, _) =
        Pubkey::find_program_address(&[ROOT, b"CALL_AUTHORITY", &protocol_id.0], &photon::ID);

    let mut accounts = photon::accounts::ExecuteOperation {
        executor,
        op_info: op_info_pda,
        protocol_info: protocol_info_pda,
        call_authority: call_authority_pda,
    }
    .to_account_metas(None);
    let function_selector = &op_data.function_selector;

    if function_selector.len() < 3 {
        error!("Failed to process function_selector due to its size");
        return Err(ExecutorError::MalformedData);
    }
    let extension_accounts = extension
        .get_accounts(&function_selector[2..], &op_data.params)
        .map_err(ExecutorError::from)?;
    accounts.extend(extension_accounts);

    let exec_op_data = photon::instruction::ExecuteOperation {
        op_hash: op_hash.to_vec(),
    }
    .data();
    let ix = Instruction::new_with_bytes(photon::id(), &exec_op_data, accounts);
    let compute_units =
        extension.get_compute_budget(&function_selector[2..], &op_data.params).unwrap_or(200000);
    Ok(InstructionBundle::new(ix, compute_units))
}
