use futures_util::StreamExt;
use log::{error, info};
use solana_client::{
    nonblocking::pubsub_client::PubsubClient,
    rpc_config::{RpcTransactionLogsConfig, RpcTransactionLogsFilter},
    rpc_response::{Response, RpcLogsResponse},
};
use std::time::Duration;
use tokio::sync::mpsc::UnboundedSender;
use transmitter_common::{config::ReconnectConfig, mongodb::MongodbConfig};

use super::{solana_retro_reader::SolanaRetroReader, EventListenerError};
use crate::common::config::SolanaListenerConfig;

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
        let websocket_url = self.solana_config.client.web_socket_url.clone().ok_or_else(|| {
            error!("web_socket_url is not configured");
            EventListenerError::Config
        })?;
        let commitment = self.solana_config.client.commitment;
        info!(
            "Start listening for new solana events, url: {}, commitment: {}, program_id: {}",
            websocket_url,
            commitment.commitment,
            photon::ID
        );

        let program_id_str = photon::ID.to_string();
        let filter = RpcTransactionLogsFilter::Mentions(vec![program_id_str.clone()]);
        let config = RpcTransactionLogsConfig {
            commitment: Some(commitment),
        };
        let reconnect = &self.solana_config.reconnect;
        while let Ok(client) = self.init_connection(websocket_url.as_str(), reconnect).await {
            info!("Solana logs subscription is done");
            let (mut notifications, unsubscribe) =
                client.logs_subscribe(filter.clone(), config.clone()).await.map_err(|err| {
                    error!("Failed to subscribe for logs: {}, error: {}", program_id_str, err);
                    EventListenerError::SolanaClient
                })?;
            // logs are collected in the pubsub client's internal channel asynchronously meanwhile
            self.logs_retro_reader
                .read_events_backward(&self.solana_config.client, &self.mongodb_config)
                .await?;
            // for finalized commitment solana duplicates messages
            info!("Retrospective logs reading is done, start to process realtime events");
            let mut last_tx_workaround = String::default();
            while let Some(logs) = notifications.next().await {
                if logs.value.signature == last_tx_workaround {
                    continue;
                }
                last_tx_workaround.clone_from(&logs.value.signature);
                self.on_logs(logs);
            }
            unsubscribe().await;
        }
        Ok(())
    }

    fn on_logs(&self, logs: Response<RpcLogsResponse>) {
        self.logs_sender
            .send(LogsBunch {
                tx_signature: logs.value.signature,
                slot: logs.context.slot,
                logs: logs.value.logs,
            })
            .expect("Expected logs_bunch to be sent");
    }

    async fn init_connection(
        &self,
        solana_rpc_url: &str,
        reconnect: &ReconnectConfig,
    ) -> Result<PubsubClient, EventListenerError> {
        let mut attemts = 0;
        Ok(loop {
            match PubsubClient::new(solana_rpc_url).await {
                Err(err) => {
                    attemts += 1;
                    error!(
                        "Failed to subscribe for solana logs, attempt: {}, error: {}",
                        attemts, err
                    );
                    if attemts == reconnect.attempts {
                        return Err(EventListenerError::SolanaClient);
                    }
                    tokio::time::sleep(Duration::from_millis(reconnect.timeout_ms)).await;
                }
                Ok(solana_client) => break solana_client,
            }
        })
    }
}

pub(crate) struct LogsBunch {
    pub tx_signature: String,
    pub logs: Vec<String>,
    pub slot: u64,
}
