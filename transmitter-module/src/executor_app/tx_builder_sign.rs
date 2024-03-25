use anchor_lang::{InstructionData, ToAccountMetas};
use log::{debug, info};
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

use transmitter_common::data::{KeeperSignature, OpHash, ProtocolId};

use super::{error::ExecutorError, OperationStatus, TransactionSet};

pub(super) struct SignOpTxBuilder {
    payer: Pubkey,
    signed_op_receiver: Mutex<UnboundedReceiver<(OpHash, ProtocolId, Vec<KeeperSignature>)>>,
    tx_set_sender: UnboundedSender<TransactionSet>,
    status_sender: UnboundedSender<OperationStatus>,
}

impl SignOpTxBuilder {
    pub(super) fn new(
        payer: Pubkey,
        signed_op_receiver: UnboundedReceiver<(OpHash, ProtocolId, Vec<KeeperSignature>)>,
        tx_set_sender: UnboundedSender<TransactionSet>,
        status_sender: UnboundedSender<OperationStatus>,
    ) -> Self {
        Self {
            payer,
            signed_op_receiver: Mutex::new(signed_op_receiver),
            tx_set_sender,
            status_sender,
        }
    }

    pub(super) async fn execute(self) -> Result<(), ExecutorError> {
        info!("Start building sign operation transactions");
        let mut op_data_receiver = self.signed_op_receiver.lock().await;
        while let Some((op_hash, protocol_id, signatures)) = op_data_receiver.recv().await {
            debug!("Build sign_operation tx, op_hash: {}", hex::encode(op_hash));
            let Ok(transaction_set) = self.build_txs(op_hash, protocol_id, signatures).await else {
                self.status_sender
                    .send(OperationStatus::Error(op_hash))
                    .expect("Expected operation status to be sent");
                continue;
            };
            self.tx_set_sender.send(transaction_set).expect("Expected transaction_set to be sent");
        }
        Ok(())
    }

    async fn build_txs(
        &self,
        op_hash: OpHash,
        protocol_id: ProtocolId,
        mut signatures: Vec<KeeperSignature>,
    ) -> Result<TransactionSet, ExecutorError> {
        let (op_info_pda, _bump) =
            Pubkey::find_program_address(&[ROOT, b"OP", &op_hash], &photon::ID);

        let (protocol_info_pda, _) =
            Pubkey::find_program_address(&[ROOT, b"PROTOCOL", &protocol_id.0], &photon::ID);

        let accounts: Vec<AccountMeta> = photon::accounts::SignOperation {
            executor: self.payer,
            op_info: op_info_pda,
            protocol_info: protocol_info_pda,
        }
        .to_account_metas(None);

        let sign_op_data = photon::instruction::SignOperation {
            op_hash: op_hash.to_vec(),
            signatures: signatures
                .drain(..)
                .map(photon::signature::KeeperSignature::from)
                .collect(),
        }
        .data();
        let instruction = Instruction::new_with_bytes(photon::id(), &sign_op_data, accounts);
        let message = Message::new(&[instruction], Some(&self.payer));
        let transaction = Transaction::new_unsigned(message);

        Ok::<TransactionSet, _>(TransactionSet {
            op_hash,
            txs: vec![transaction],
            blockhash: None,
        })
    }
}
