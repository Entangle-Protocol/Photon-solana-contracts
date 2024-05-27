use log::{debug, error, info, warn};
use solana_client::{nonblocking::rpc_client::RpcClient, rpc_request::RpcError};
use solana_sdk::{
    commitment_config::CommitmentConfig,
    hash::Hash,
    signature::{Keypair, Signer},
    transaction::Transaction,
};
use std::{sync::Arc, time::Duration};
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
        info!(
            "Start to send transactions to solana: {}, commitment: {}, executor: {}",
            url,
            commitment,
            self.payer.pubkey()
        );

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

    async fn get_blockhash_with_retry(client: Arc<RpcClient>) -> Hash {
        loop {
            match client.get_latest_blockhash().await {
                Ok(x) => {
                    return x;
                }
                Err(e) => {
                    warn!("Error getting blockhash {}", e);
                    tokio::time::sleep(Duration::from_secs(3)).await;
                }
            }
        }
    }

    async fn send_transaction_impl(
        payer: Arc<Keypair>,
        _latest_blockhash: Hash,
        op_hash: OpHash,
        mut transaction: Transaction,
        client: Arc<RpcClient>,
    ) -> Result<(), ExecutorError> {
        let mut current_blockhash =
            SolanaTransactor::get_blockhash_with_retry(Arc::clone(&client)).await;
        loop {
            log::info!("Using blockhash {}", current_blockhash);
            transaction.try_partial_sign(&[&payer], current_blockhash).map_err(|err| {
                error!("Failed to sign transaction with payer secret key: {}", err);
                ExecutorError::SolanaClient
            })?;
            if !transaction.is_signed() {
                error!("Transaction is not fully signed: {}", hex::encode(op_hash));
                return Err(ExecutorError::SolanaClient);
            }
            match client.send_transaction(&transaction).await {
                Ok(signature) => {
                    info!(
                            "Transaction sent, solana tx_signature: {}, op_hash: {}, trying to confirm...",
                            signature,
                            hex::encode(op_hash),
                        );
                    tokio::time::sleep(Duration::from_secs(5)).await;
                    for _ in 0..2 {
                        match client
                            .confirm_transaction_with_commitment(
                                &signature,
                                CommitmentConfig::processed(),
                            )
                            .await
                        {
                            Ok(r) if r.value => {
                                info!("Transaction {} status Processed, confirming...", signature);
                                for _ in 0..5 {
                                    match client
                                        .confirm_transaction_with_commitment(
                                            &signature,
                                            CommitmentConfig::finalized(),
                                        )
                                        .await
                                    {
                                        Ok(r) if r.value => {
                                            info!("Transaction {} confirmed", signature);
                                            return Ok(());
                                        }
                                        Ok(_) => {
                                            debug!("{} Not yet confirmed", signature);
                                        }
                                        Err(e) => {
                                            debug!("Not confirmed {}: {}", signature, e);
                                        }
                                    }
                                    tokio::time::sleep(Duration::from_secs(20)).await;
                                }
                                break;
                            }
                            Ok(_) => {
                                debug!("{} Not yet processed", signature);
                            }
                            Err(e) => {
                                debug!("Not processed {}: {}", signature, e);
                            }
                        }
                        tokio::time::sleep(Duration::from_secs(5)).await;
                    }
                    warn!("Failed to confirm {}", signature);
                }
                Err(solana_client::client_error::ClientError {
                    kind:
                        solana_client::client_error::ClientErrorKind::RpcError(
                            RpcError::RpcResponseError {
                                code: -32002,
                                message,
                                data,
                            },
                        ),
                    ..
                }) => {
                    if message.contains("Blockhash not found") {
                        warn!("Transaction failed because blockhash not found, retrying");
                    } else {
                        warn!("Transaction possibly processed {:?}, {:?}", message, data);
                        return Ok(());
                    }
                }
                Err(err) => {
                    warn!("Failed to send transaction: {:?}", err);
                }
            }
            tokio::time::sleep(Duration::from_secs(3)).await;

            loop {
                let new_blockhash =
                    SolanaTransactor::get_blockhash_with_retry(Arc::clone(&client)).await;
                if new_blockhash != current_blockhash {
                    current_blockhash = new_blockhash;
                    break;
                } else {
                    tokio::time::sleep(Duration::from_secs(1)).await;
                }
            }
        }
    }
}
