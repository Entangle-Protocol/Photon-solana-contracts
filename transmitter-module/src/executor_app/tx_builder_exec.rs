use anchor_lang::{InstructionData, ToAccountMetas};
use log::{debug, error, info};
use photon::photon::ROOT;
use solana_client::nonblocking::rpc_client::RpcClient;
use solana_sdk::{
    hash::Hash, instruction::Instruction, message::Message, pubkey::Pubkey,
    transaction::Transaction,
};
use tokio::sync::{
    mpsc::{UnboundedReceiver, UnboundedSender},
    Mutex,
};

use transmitter_common::data::{OpHash, OperationData};

use super::{
    config::SolanaClientConfig, error::ExecutorError, extension_manager::ExtensionManager,
    OperationStatus, TransactionSet,
};

pub(super) struct ExecOpTxBuilder {
    payer: Pubkey,
    solana_client: RpcClient,
    op_data_receiver: Mutex<UnboundedReceiver<(OpHash, OperationData)>>,
    tx_set_sender: UnboundedSender<TransactionSet>,
    extension_mng: ExtensionManager,
    status_sender: UnboundedSender<OperationStatus>,
}

impl ExecOpTxBuilder {
    pub(super) fn try_new(
        extensions: Vec<String>,
        payer: Pubkey,
        client_config: SolanaClientConfig,
        op_data_receiver: UnboundedReceiver<(OpHash, OperationData)>,
        tx_set_sender: UnboundedSender<TransactionSet>,
        status_sender: UnboundedSender<OperationStatus>,
    ) -> Result<Self, ExecutorError> {
        let solana_client =
            RpcClient::new_with_commitment(client_config.url, client_config.commitment);
        Ok(Self {
            payer,
            solana_client,
            op_data_receiver: Mutex::new(op_data_receiver),
            tx_set_sender,
            extension_mng: ExtensionManager::try_new(extensions)?,
            status_sender,
        })
    }

    pub(super) async fn execute(self) -> Result<(), ExecutorError> {
        info!("Start building exec operation transactions");
        let mut op_data_receiver = self.op_data_receiver.lock().await;
        while let Some((op_hash, op_data)) = op_data_receiver.recv().await {
            debug!("Build exec_operation tx, op_hash: {}", hex::encode(op_hash));
            let Ok(blockhash) = self.solana_client.get_latest_blockhash().await.map_err(|err| {
                error!("Failed to get latest blockhash: {}", err);
                ExecutorError::SolanaClient
            }) else {
                self.status_sender
                    .send(OperationStatus::Reschedule(op_hash))
                    .expect("Expected status to be sent");
                continue;
            };
            let Ok(transaction_set) = self.build_txs(blockhash, op_hash, op_data) else {
                self.status_sender
                    .send(OperationStatus::Error(op_hash))
                    .expect("Expected status to be sent");
                continue;
            };
            self.tx_set_sender.send(transaction_set).expect("Expected transaction_set to be sent");
        }
        Ok(())
    }

    fn build_txs(
        &self,
        blockhash: Hash,
        op_hash: OpHash,
        op_data: OperationData,
    ) -> Result<TransactionSet, ExecutorError> {
        let protocol_id = op_data.protocol_id;
        let extension = self.extension_mng.get_extension(&protocol_id).ok_or_else(|| {
            error!("Failed to get extension by protocol_id: {}", protocol_id);
            ExecutorError::Extensions
        })?;

        let (op_info_pda, _bump) =
            Pubkey::find_program_address(&[ROOT, b"OP", &op_hash], &photon::ID);
        let (protocol_info_pda, _) =
            Pubkey::find_program_address(&[ROOT, b"PROTOCOL", &protocol_id.0], &photon::ID);
        let (call_authority_pda, _) =
            Pubkey::find_program_address(&[ROOT, b"CALL_AUTHORITY", &protocol_id.0], &photon::ID);
        let mut accounts = photon::accounts::ExecuteOperation {
            executor: self.payer,
            op_info: op_info_pda,
            protocol_info: protocol_info_pda,
            call_authority: call_authority_pda,
        }
        .to_account_metas(None);
        let extension_accounts =
            extension.get_accounts(&op_data.function_selector, &op_data.params);
        accounts.extend(extension_accounts);

        let exec_op_data = photon::instruction::ExecuteOperation {
            op_hash: op_hash.to_vec(),
        }
        .data();

        let instruction = Instruction::new_with_bytes(photon::id(), &exec_op_data, accounts);
        let message = Message::new(&[instruction], Some(&self.payer));
        let mut transaction = Transaction::new_unsigned(message);

        extension.sign_transaction(&mut transaction, &blockhash).map_err(|err| {
            error!("Failed to sign transaction by extension: {}, error: {}", protocol_id, err);
            ExecutorError::Extensions
        })?;

        Ok(TransactionSet {
            op_hash,
            txs: vec![transaction],
            blockhash: Some(blockhash),
        })
    }
}
