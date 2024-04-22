use anchor_lang::{InstructionData, ToAccountMetas};
use log::{debug, error, info};
use photon::photon::ROOT;
use solana_sdk::{
    instruction::{AccountMeta, Instruction},
    message::Message,
    pubkey::Pubkey,
    transaction::Transaction,
};
use tokio::sync::{
    mpsc::{UnboundedReceiver, UnboundedSender},
    Mutex,
};

use transmitter_common::data::{OpHash, OperationData};

use super::{error::ExecutorError, OperationStatus, TransactionSet};

pub(super) struct LoadOpTxBuilder {
    payer: Pubkey,
    op_data_receiver: Mutex<UnboundedReceiver<(OpHash, OperationData)>>,
    tx_set_sender: UnboundedSender<TransactionSet>,
    status_sender: UnboundedSender<OperationStatus>,
}

impl LoadOpTxBuilder {
    pub(super) fn new(
        payer: Pubkey,
        op_data_receiver: UnboundedReceiver<(OpHash, OperationData)>,
        tx_set_sender: UnboundedSender<TransactionSet>,
        status_sender: UnboundedSender<OperationStatus>,
    ) -> Self {
        Self {
            payer,
            op_data_receiver: Mutex::new(op_data_receiver),
            tx_set_sender,
            status_sender,
        }
    }

    pub(super) async fn execute(self) -> Result<(), ExecutorError> {
        info!("Start building load operation transactions");
        let mut op_data_receiver = self.op_data_receiver.lock().await;
        while let Some((op_hash, op_data)) = op_data_receiver.recv().await {
            debug!("Build load_operation tx, op_hash: {}", hex::encode(op_hash));
            let Ok(transaction_set) = self.build_txs(op_hash, op_data) else {
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
        op_hash: OpHash,
        op_data: OperationData,
    ) -> Result<TransactionSet, ExecutorError> {
        let (op_info_pda, _bump) =
            Pubkey::find_program_address(&[ROOT, b"OP", &op_hash], &photon::ID);
        let (protocol_info_pda, _) =
            Pubkey::find_program_address(&[ROOT, b"PROTOCOL", &op_data.protocol_id.0], &photon::ID);
        let (config_pda, _) = Pubkey::find_program_address(&[ROOT, b"CONFIG"], &photon::ID);

        let accounts: Vec<AccountMeta> = photon::accounts::LoadOperation {
            executor: self.payer,
            protocol_info: protocol_info_pda,
            op_info: op_info_pda,
            config: config_pda,
            system_program: anchor_lang::system_program::ID,
        }
        .to_account_metas(None);
        let protocol_id = String::from_utf8(op_data.protocol_id.0.to_vec()).map_err(|err| {
            error!("Failed to get protocol id utf8: {}", err);
            ExecutorError::MalformedData
        })?;

        debug!(
            "Build txs for protocol_id: {}, executor: {}, protocol_info: {}, op_info: {}, config: {}",
            protocol_id, self.payer, protocol_info_pda, op_info_pda, config_pda
        );

        let photon_op_data =
            photon::signature::OperationData::try_from(op_data).map_err(|err| {
                error!("Failed to get op_data from op_data_message: {}", hex::encode(err));
                ExecutorError::MalformedData
            })?;

        let load_op_data = photon::instruction::LoadOperation {
            op_data: photon_op_data,
            op_hash_cached: op_hash.to_vec(),
        }
        .data();

        let instruction = Instruction::new_with_bytes(photon::id(), &load_op_data, accounts);
        let message = Message::new(&[instruction], Some(&self.payer));
        let transaction = Transaction::new_unsigned(message);
        debug!("load_tx transaction: {:?}", transaction);
        Ok(TransactionSet {
            op_hash,
            txs: vec![transaction],
            blockhash: None,
        })
    }
}
