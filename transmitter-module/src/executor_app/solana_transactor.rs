use log::{debug, error, info};
use solana_client::nonblocking::rpc_client::RpcClient;
use solana_sdk::{hash::Hash, signature::Keypair, transaction::Transaction};
use std::sync::Arc;
use tokio::sync::{
    mpsc::{UnboundedReceiver, UnboundedSender},
    Mutex,
};

use transmitter_common::data::OpHash;

use super::{error::ExecutorError, OperationStatus, TransactionSet};
use crate::common::config::SolanaClientConfig;

pub(super) struct SolanaTransactor {
    payer: Arc<Keypair>,
    solana_client: Arc<RpcClient>,
    tx_set_receiver: Mutex<UnboundedReceiver<TransactionSet>>,
    tx_status_sender: UnboundedSender<OperationStatus>,
}

impl SolanaTransactor {
    pub(super) fn new(
        client_cofig: SolanaClientConfig,
        payer: Arc<Keypair>,
        tx_set_receiver: UnboundedReceiver<TransactionSet>,
        tx_status_sender: UnboundedSender<OperationStatus>,
    ) -> SolanaTransactor {
        let client = RpcClient::new_with_commitment(client_cofig.rpc_url, client_cofig.commitment);
        SolanaTransactor {
            payer,
            solana_client: Arc::new(client),
            tx_status_sender,
            tx_set_receiver: Mutex::new(tx_set_receiver),
        }
    }

    pub(super) async fn execute(self) -> Result<(), ExecutorError> {
        let url = self.solana_client.url();
        let commitment = self.solana_client.commitment().commitment;
        info!("Start to send transactions to solana: {}, commitment: {}", url, commitment);
        self.process_transactions().await
    }

    async fn process_transactions(&self) -> Result<(), ExecutorError> {
        let mut receiver = self.tx_set_receiver.lock().await;
        while let Some(TransactionSet {
            op_hash,
            mut txs,
            blockhash,
        }) = receiver.recv().await
        {
            debug!("Transaction set received, op_hash: {}", hex::encode(op_hash));
            let transaction = txs.pop().expect("Expected tx_set to have at least one transaction");
            tokio::spawn(Self::send_transaction(
                self.payer.clone(),
                blockhash,
                op_hash,
                transaction,
                self.solana_client.clone(),
                self.tx_status_sender.clone(),
            ));
        }
        Ok(())
    }

    async fn send_transaction(
        payer: Arc<Keypair>,
        latest_blockhash: Option<Hash>,
        op_hash: OpHash,
        transaction: Transaction,
        client: Arc<RpcClient>,
        status_sender: UnboundedSender<OperationStatus>,
    ) {
        let latest_blockhash = match latest_blockhash {
            Some(blockhash) => blockhash,
            None => {
                let Ok(blockhash) = client.get_latest_blockhash().await.map_err(|err| {
                    error!("Failed to get latest blockhash: {}", err);
                    status_sender
                        .send(OperationStatus::Reschedule(op_hash))
                        .expect("Expected status to be sent");
                }) else {
                    return;
                };
                blockhash
            }
        };

        if Self::send_transaction_impl(payer, latest_blockhash, op_hash, transaction, client)
            .await
            .is_ok()
        {
            status_sender
                .send(OperationStatus::Complete(op_hash))
                .expect("Expected status to be sent");
        } else {
            status_sender
                .send(OperationStatus::Error(op_hash))
                .expect("Expected status to be sent");
        }
    }

    async fn send_transaction_impl(
        payer: Arc<Keypair>,
        latest_blockhash: Hash,
        op_hash: OpHash,
        mut transaction: Transaction,
        client: Arc<RpcClient>,
    ) -> Result<(), ExecutorError> {
        transaction.try_partial_sign(&[&payer], latest_blockhash).map_err(|err| {
            error!("Failed to sign transaction with payer secret key: {}", err);
            ExecutorError::SolanaClient
        })?;

        if !transaction.is_signed() {
            error!("Transaction is not fully signed: {}", hex::encode(op_hash));
            return Err(ExecutorError::SolanaClient);
        }
        let signature = client.send_and_confirm_transaction(&transaction).await.map_err(|err| {
            error!("Failed to send transaction: {:?}", err);
            ExecutorError::SolanaClient
        })?;

        debug!(
            "Transaction sent, solana tx_signature: {}, op_hash: {}, ",
            signature,
            hex::encode(op_hash),
        );
        Ok(())
    }
}
