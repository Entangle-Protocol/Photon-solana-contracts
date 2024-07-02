use log::{debug, error};
use solana_client::{
    rpc_client::GetConfirmedSignaturesForAddress2Config, rpc_config::RpcTransactionConfig,
    rpc_response::RpcConfirmedTransactionStatusWithSignature,
};
use solana_sdk::{commitment_config::CommitmentConfig, pubkey::Pubkey, signature::Signature};
use solana_transaction_status::UiTransactionEncoding;
use solana_transactor::RpcPool;
use std::{collections::VecDeque, str::FromStr, time::Duration};
use tokio::sync::mpsc::UnboundedSender;

use transmitter_common::mongodb::MongodbConfig;

use super::{solana_retro_reader::SolanaRetroReader, EventListenerError};
use crate::common::config::{SolanaClientConfig, SolanaListenerConfig};

const LOGS_TIMEOUT_SEC: u64 = 5;

pub(crate) struct SolanaEventListener {
    solana_config: SolanaListenerConfig,
    mongodb_config: MongodbConfig,
    logs_sender: UnboundedSender<LogsBunch>,
    logs_retro_reader: SolanaRetroReader,
}

impl SolanaEventListener {
    pub(crate) fn new(
        solana_config: SolanaListenerConfig,
        mongodb_config: MongodbConfig,
        logs_sender: UnboundedSender<LogsBunch>,
    ) -> Self {
        SolanaEventListener {
            solana_config,
            mongodb_config: mongodb_config.clone(),
            logs_sender: logs_sender.clone(),
            logs_retro_reader: SolanaRetroReader::new(mongodb_config, logs_sender),
        }
    }

    pub(crate) async fn listen_to_solana(&self) -> Result<(), EventListenerError> {
        let config = &self.solana_config.client;
        let rpc_pool = RpcPool::new(&config.read_rpcs, &config.write_rpcs)?;

        let slot = rpc_pool
            .with_read_rpc_loop(
                |rpc| async move { rpc.get_block_height().await },
                config.commitment,
            )
            .await;

        self.logs_retro_reader
            .read_events_backward(&self.solana_config, &self.mongodb_config)
            .await?;

        self.read_events_backward(&self.solana_config.client, rpc_pool, slot).await
    }

    async fn read_events_backward(
        &self,
        solana_config: &SolanaClientConfig,
        rpc_pool: RpcPool,
        init_slot: u64,
    ) -> Result<(), EventListenerError> {
        let mut slot = init_slot;

        loop {
            debug!("read events backward, init_slot: {}", slot);
            tokio::time::sleep(Duration::from_secs(LOGS_TIMEOUT_SEC)).await;
            let Ok(log_bunches) =
                self.read_event_backward_until(&rpc_pool, solana_config, slot).await
            else {
                continue;
            };
            for logs_bunch in log_bunches {
                debug!("update slot: {}", logs_bunch.slot);
                slot = logs_bunch.slot;
                self.logs_sender.send(logs_bunch).expect("Expected logs_bunch to be sent");
            }
        }
    }

    async fn read_event_backward_until(
        &self,
        rpc_pool: &RpcPool,
        solana_config: &SolanaClientConfig,
        slot: u64,
    ) -> Result<VecDeque<LogsBunch>, EventListenerError> {
        let until = None;
        let mut before = None;
        let mut log_bunches = VecDeque::new();
        debug!("read events backward until slot: {}", slot);
        loop {
            let signatures_backward = Self::get_signatures_chunk(
                &photon::ID,
                solana_config,
                rpc_pool,
                until,
                before,
                slot,
            )
            .await?;
            debug!("signatures backward, until: {:?}, before: {:?}, slot: {}", until, before, slot);
            if signatures_backward.is_empty() {
                break;
            }

            Self::process_signatures(
                rpc_pool,
                &mut before,
                &mut log_bunches,
                signatures_backward,
                solana_config.commitment,
            )
            .await;
        }

        Ok(log_bunches)
    }

    async fn process_signatures(
        rpc_pool: &RpcPool,
        before: &mut Option<Signature>,
        log_bunches: &mut VecDeque<LogsBunch>,
        signatures_with_meta: Vec<RpcConfirmedTransactionStatusWithSignature>,
        commitment: CommitmentConfig,
    ) {
        for signature_with_meta in signatures_with_meta {
            debug!("Check if signature with logs: {}", signature_with_meta.signature);
            _ = Self::process_signature(
                rpc_pool,
                before,
                log_bunches,
                signature_with_meta,
                commitment,
            )
            .await;
        }
    }

    async fn process_signature(
        rpc_pool: &RpcPool,
        before: &mut Option<Signature>,
        log_bunches: &mut VecDeque<LogsBunch>,
        signature_with_meta: RpcConfirmedTransactionStatusWithSignature,
        commitment: CommitmentConfig,
    ) -> Result<(), ()> {
        let signature = &Signature::from_str(&signature_with_meta.signature)
            .map_err(|err| error!("Failed to parse signature: {}", err))?;
        before.replace(*signature);
        let transaction = rpc_pool
            .with_read_rpc_loop(
                |rpc| async move {
                    rpc.get_transaction_with_config(
                        signature,
                        RpcTransactionConfig {
                            encoding: Some(UiTransactionEncoding::Json),
                            commitment: Some(commitment),
                            max_supported_transaction_version: Some(0),
                        },
                    )
                    .await
                },
                commitment,
            )
            .await;

        let logs = transaction
            .transaction
            .meta
            .filter(|meta| meta.err.is_none())
            .map(|meta| <Option<Vec<String>>>::from(meta.log_messages))
            .ok_or(())?
            .ok_or(())?;

        if logs.is_empty() {
            return Ok(());
        }

        log_bunches.push_front(LogsBunch {
            tx_signature: signature_with_meta.signature,
            slot: transaction.slot,
            logs,
        });
        Ok(())
    }

    async fn get_signatures_chunk(
        program_id: &Pubkey,
        solana_config: &SolanaClientConfig,
        rpc_pool: &RpcPool,
        until: Option<Signature>,
        before: Option<Signature>,
        slot: u64,
    ) -> Result<Vec<RpcConfirmedTransactionStatusWithSignature>, EventListenerError> {
        let signatures_backward = rpc_pool
            .with_read_rpc_loop(
                |rpc| async move {
                    let args = GetConfirmedSignaturesForAddress2Config {
                        before,
                        until,
                        limit: None,
                        commitment: Some(solana_config.commitment),
                    };
                    rpc.get_signatures_for_address_with_config(program_id, args).await
                },
                solana_config.commitment,
            )
            .await;
        Ok(signatures_backward.iter().filter(|s| s.slot > slot).cloned().collect())
    }
}

pub(crate) struct LogsBunch {
    pub tx_signature: String,
    pub logs: Vec<String>,
    pub slot: u64,
}
