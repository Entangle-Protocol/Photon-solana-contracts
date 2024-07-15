use anchor_lang::{
    prelude::{AccountMeta, Pubkey},
    AccountDeserialize, InstructionData, ToAccountMetas,
};
use futures_util::{select, FutureExt, StreamExt};
use log::*;
use photon::{photon::ROOT, OpInfo};
use solana_sdk::{
    address_lookup_table::AddressLookupTableAccount, instruction::Instruction, signer::Signer,
};
use solana_transactor::{ix_compiler::InstructionBundle, log_with_ctx, SolanaTransactor};
use std::ops::Deref;
use std::sync::{
    atomic::{AtomicU64, AtomicUsize, Ordering},
    Arc,
};
use std::time::Duration;
use tokio::sync::{
    mpsc::{Receiver, UnboundedReceiver, UnboundedSender},
    Mutex, Notify,
};
use tokio_stream::wrappers::ReceiverStream;

use transmitter_common::data::{OpHash, OperationData, SignedOperation};

use super::{
    error::ExecutorError, extension_manager::ExtensionManager, ExecutorOpStatus, OpAcknowledge,
    ServiceCmd, OP_DATA_SENDER_CAPACITY,
};
use crate::executor_app::config::SolanaExecutorConfig;

pub(super) struct OperationManager {
    op_data_receiver: Mutex<Option<ReceiverStream<SignedOperation>>>,
    op_acknowledge_sender: UnboundedSender<OpAcknowledge>,
    transactor: SolanaTransactor,
    extension_mng: ExtensionManager,
    solana_config: SolanaExecutorConfig,
    service_receiver: Mutex<UnboundedReceiver<ServiceCmd>>,
    suspending_ctx: SuspendingCtx,
}

#[derive(Default)]
struct SuspendingCtx {
    balance: AtomicU64,
    processing_notify: Mutex<Option<Arc<Notify>>>,
    op_proc_counter: AtomicUsize,
}

impl OperationManager {
    pub fn new(
        op_data_receiver: Receiver<SignedOperation>,
        op_acknowledge_sender: UnboundedSender<OpAcknowledge>,
        transactor: SolanaTransactor,
        extensions: Vec<String>,
        solana_config: SolanaExecutorConfig,
        service_receiver: UnboundedReceiver<ServiceCmd>,
    ) -> Self {
        let extension_mng = ExtensionManager::new(extensions);
        let op_data_receiver: ReceiverStream<SignedOperation> =
            ReceiverStream::new(op_data_receiver);
        Self {
            op_data_receiver: Mutex::new(Some(op_data_receiver)),
            op_acknowledge_sender,
            transactor,
            extension_mng,
            solana_config,
            service_receiver: Mutex::new(service_receiver),
            suspending_ctx: SuspendingCtx::default(),
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
        let alt = &[][..];

        self.op_data_receiver
            .lock()
            .await
            .take()
            .unwrap()
            .for_each_concurrent(OP_DATA_SENDER_CAPACITY, |op_data| async {
                let op_hash = op_data.operation_data.op_hash_with_message();
                self.op_acknowledge_sender
                    .send(OpAcknowledge::new(
                        op_data.eob_block_number,
                        op_hash,
                        ExecutorOpStatus::New,
                    ))
                    .expect("Expected acknowledge to be sent");
                if let Err(e) = self.process_operation(op_hash, op_data, alt).await {
                    error!("{}: Failed to process: {}", hex::encode(op_hash), e);
                }
            })
            .await;
    }

    async fn process_operation(
        &self,
        op_hash: OpHash,
        op: SignedOperation,
        alt: &[AddressLookupTableAccount],
    ) -> Result<(), ExecutorError> {
        let op_hash_str = hex::encode(op_hash);
        debug!("{}. Operation received", op_hash_str);
        let mut last_op_status = (None, 0);
        loop {
            if !self.check_balance_and_suspend(&op_hash_str).await {
                continue;
            }

            self.suspending_ctx.op_proc_counter.fetch_add(1, Ordering::Release);
            let Ok(mut op_status) = self.get_op_status(op_hash).await else {
                return Ok(());
            };

            match last_op_status {
                (Some(value), ref mut attempts) if value == op_status => {
                    *attempts += 1;
                    if *attempts >= self.solana_config.executor_attempts {
                        op_status = ExecutorOpStatus::Failed;
                    }
                }
                _ => last_op_status = (Some(op_status), 0),
            }

            debug!("{}. Operation status: {:?}", op_hash_str, op_status);
            if ExecutorOpStatus::Executed == op_status || ExecutorOpStatus::Failed == op_status {
                self.ack_executed(op.eob_block_number, op_hash, op_status);
                break;
            }

            let ix_bundle = self.build_ixs(op_hash, op.clone(), op_status)?;
            const COMPUTE_UNIT_PRICE_LAMPORTS: u64 = 1000;
            self.transactor
                .send_all_instructions(
                    Some(op_hash_str.deref()),
                    &ix_bundle,
                    &[&self.solana_config.payer],
                    self.solana_config.payer.pubkey(),
                    1,
                    alt,
                    Some(COMPUTE_UNIT_PRICE_LAMPORTS),
                    false,
                )
                .await?;
        }
        Ok(())
    }

    fn ack_executed(&self, eob_block_number: u64, op_hash: OpHash, op_status: ExecutorOpStatus) {
        self.op_acknowledge_sender
            .send(OpAcknowledge::new(eob_block_number, op_hash, op_status))
            .expect("Expected acknowledge to be sent");
    }

    async fn get_op_status(&self, op_hash: OpHash) -> Result<ExecutorOpStatus, ExecutorError> {
        let (op_info, _) = Pubkey::find_program_address(&[ROOT, b"OP", &op_hash], &photon::ID);
        let op_info_data = self
            .transactor
            .rpc_pool()
            .with_read_rpc_loop(
                |rpc| async move {
                    rpc.get_account_with_commitment(&op_info, self.solana_config.client.commitment)
                        .await
                },
                self.solana_config.client.commitment,
            )
            .await
            .value;
        let op_status = match op_info_data {
            Some(acc) => match OpInfo::try_deserialize(&mut &acc.data[..]) {
                Ok(s) => ExecutorOpStatus::from(s.status),
                Err(e) => {
                    error!(
                        "{}. Failed to deserialize op_info, ({}) skipping...",
                        hex::encode(op_hash),
                        e
                    );
                    return Err(ExecutorError::MalformedData);
                }
            },
            None => ExecutorOpStatus::New,
        };
        Ok(op_status)
    }

    async fn get_balance(&self) -> Result<u64, ExecutorError> {
        let rpc = self.transactor.rpc_pool();
        let rpc_balance = rpc
            .with_read_rpc(
                |rpc| async move { rpc.get_balance(&self.solana_config.payer.pubkey()).await },
                self.solana_config.client.commitment,
            )
            .await
            .map_err(|err| {
                error!("Failed to get balance: {}", err);
                ExecutorError::from(err)
            })?;
        Ok(rpc_balance)
    }

    async fn check_balance_and_suspend(&self, op_hash: &str) -> bool {
        let suspending_config = &self.solana_config.suspending_config;

        if self.suspending_ctx.op_proc_counter.load(Ordering::Acquire)
            % suspending_config.check_balance_period
            == 0
        {
            let Ok(new_balance) = self.get_balance().await else {
                return false;
            };
            self.suspending_ctx.balance.store(new_balance, Ordering::Release);
            if new_balance < suspending_config.suspend_balance_lamports {
                warn!(
                    "Executor balance is too low: {} lamports. Processing will be suspended.",
                    new_balance
                );
            } else if new_balance < suspending_config.warn_balance_lamports {
                warn!("Executor balance is getting too low: {} lamports", new_balance);
            }
        }

        let balance = self.suspending_ctx.balance.load(Ordering::Acquire);
        if balance >= suspending_config.suspend_balance_lamports {
            return true;
        }

        // At this place all operation precesses are getting synchronized
        let mut notify_guard = self.suspending_ctx.processing_notify.lock().await;
        if let Some(notify) = notify_guard.clone() {
            log_with_ctx!(
                debug,
                Some(op_hash),
                "Balance is insufficient: {} lamports, suspended",
                balance
            );
            drop(notify_guard);
            notify.notified().await;
            return true;
        }

        notify_guard.replace(Arc::new(Notify::new()));
        drop(notify_guard);

        loop {
            tokio::time::sleep(Duration::from_secs(60)).await;
            let Ok(balance) = self.get_balance().await else {
                continue;
            };
            log_with_ctx!(debug, Some(op_hash), "Balance is being checked: {} lamports", balance);
            if balance >= suspending_config.suspend_balance_lamports {
                self.suspending_ctx.balance.store(balance, Ordering::Release);
                log_with_ctx!(debug, Some(op_hash), "Resume processing, balance: {}", balance);
                let mut proc_notify_guard = self.suspending_ctx.processing_notify.lock().await;
                proc_notify_guard.take().expect("Expected to be set").notify_waiters();
                return true;
            }
        }
    }

    fn build_ixs(
        &self,
        op_hash: [u8; 32],
        op: SignedOperation,
        op_status: ExecutorOpStatus,
    ) -> Result<Vec<InstructionBundle>, ExecutorError> {
        let payer = self.solana_config.payer.pubkey();
        Ok(match op_status {
            ExecutorOpStatus::New => vec![
                build_load_ix(payer, op_hash, op.operation_data.clone())?,
                build_sign_tx(payer, op_hash, op.clone())?,
                build_execute_tx(
                    &self.extension_mng,
                    self.solana_config.payer.pubkey(),
                    op_hash,
                    op.operation_data.clone(),
                )?,
            ],
            ExecutorOpStatus::Loaded => vec![
                build_sign_tx(payer, op_hash, op.clone())?,
                build_execute_tx(&self.extension_mng, payer, op_hash, op.operation_data.clone())?,
            ],
            ExecutorOpStatus::Signed => vec![build_execute_tx(
                &self.extension_mng,
                payer,
                op_hash,
                op.operation_data.clone(),
            )?],
            ExecutorOpStatus::Executed | ExecutorOpStatus::Failed => {
                panic!("Unexpected op status")
            }
        })
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
