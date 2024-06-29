use log::{error, warn};
use solana_client::{
    nonblocking::rpc_client::RpcClient, rpc_client::GetConfirmedSignaturesForAddress2Config,
    rpc_response::RpcConfirmedTransactionStatusWithSignature,
};
use solana_sdk::{pubkey::Pubkey, signature::Signature};
use solana_transaction_status::UiTransactionEncoding;
use std::{collections::VecDeque, str::FromStr, time::Duration};
use tokio::sync::mpsc::UnboundedSender;

use transmitter_common::mongodb::MongodbConfig;

use super::{solana_retro_reader::SolanaRetroReader, EventListenerError};
use crate::common::config::{SolanaClientConfig, SolanaListenerConfig};

const LOGS_TIMEOUT_SEC: u64 = 1;

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
        let client = RpcClient::new_with_commitment(config.rpc_url.clone(), config.commitment);

        let slot = client.get_block_height().await.map_err(|err| {
            error!("Failed to get block_height: {}", err);
            EventListenerError::SolanaClient
        })?;

        self.logs_retro_reader
            .read_events_backward(&self.solana_config, &self.mongodb_config)
            .await?;

        self.read_events_backward(&self.solana_config.client, client, slot).await
    }

    async fn read_events_backward(
        &self,
        solana_config: &SolanaClientConfig,
        client: RpcClient,
        init_slot: u64,
    ) -> Result<(), EventListenerError> {
        let mut slot = init_slot;

        loop {
            tokio::time::sleep(Duration::from_secs(LOGS_TIMEOUT_SEC)).await;
            let Ok(log_bunches) =
                self.read_event_backward_until(&client, solana_config, slot).await
            else {
                continue;
            };
            for logs_bunch in log_bunches {
                slot = logs_bunch.slot;
                self.logs_sender.send(logs_bunch).expect("Expected logs_bunch to be sent");
            }
        }
    }

    async fn read_event_backward_until(
        &self,
        client: &RpcClient,
        solana_config: &SolanaClientConfig,
        slot: u64,
    ) -> Result<VecDeque<LogsBunch>, EventListenerError> {
        let until = None;
        let mut before = None;
        let mut log_bunches = VecDeque::new();
        loop {
            let signatures_backward =
                Self::get_signatures_chunk(&photon::ID, solana_config, client, until, before, slot)
                    .await?;

            if signatures_backward.is_empty() {
                break;
            }

            Self::process_signatures(client, &mut before, &mut log_bunches, signatures_backward)
                .await;
        }

        Ok(log_bunches)
    }

    async fn process_signatures(
        client: &RpcClient,
        before: &mut Option<Signature>,
        log_bunches: &mut VecDeque<LogsBunch>,
        signatures_with_meta: Vec<RpcConfirmedTransactionStatusWithSignature>,
    ) {
        for signature_with_meta in signatures_with_meta {
            _ = Self::process_signature(client, before, log_bunches, signature_with_meta).await;
        }
    }

    async fn process_signature(
        client: &RpcClient,
        before: &mut Option<Signature>,
        log_bunches: &mut VecDeque<LogsBunch>,
        signature_with_meta: RpcConfirmedTransactionStatusWithSignature,
    ) -> Result<(), ()> {
        let signature = &Signature::from_str(&signature_with_meta.signature)
            .map_err(|err| error!("Failed to parse signature: {}", err))?;
        before.replace(*signature);
        let transaction = client
            .get_transaction(signature, UiTransactionEncoding::Json)
            .await
            .map_err(|_err| ())?;

        let logs = transaction
            .transaction
            .meta
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
        client: &RpcClient,
        until: Option<Signature>,
        before: Option<Signature>,
        slot: u64,
    ) -> Result<Vec<RpcConfirmedTransactionStatusWithSignature>, EventListenerError> {
        let args = GetConfirmedSignaturesForAddress2Config {
            before,
            until,
            limit: None,
            commitment: Some(solana_config.commitment),
        };

        let signatures_backward = client
            .get_signatures_for_address_with_config(program_id, args)
            .await
            .map_err(|err| {
                warn!("Failed to get signatures for address: {}", err);
                EventListenerError::SolanaClient
            })?;
        Ok(signatures_backward.iter().filter(|s| s.slot > slot).cloned().collect())
    }
}

pub(crate) struct LogsBunch {
    pub tx_signature: String,
    pub logs: Vec<String>,
    pub slot: u64,
}
