use futures_util::StreamExt;
use log::{debug, error, info};
use solana_client::{
    nonblocking::pubsub_client::PubsubClient,
    rpc_config::{RpcTransactionLogsConfig, RpcTransactionLogsFilter},
};
use solana_sdk::signature::Signature;
use std::str::FromStr;
use tokio::sync::mpsc::UnboundedSender;

use photon::{ProposeEvent, ID as PROGRAM_ID};
use transmitter_common::data::{OperationData, Propose, ProtocolId};

use super::{config::SolanaListenerConfig, error::ListenError, solana_logs::parse_logs_response};

pub(super) struct SolanaEventListener {
    pub(super) config: SolanaListenerConfig,
    pub(super) propose_sender: UnboundedSender<Propose>,
}

impl SolanaEventListener {
    pub(super) fn new(
        config: SolanaListenerConfig,
        propose_sender: UnboundedSender<Propose>,
    ) -> Self {
        SolanaEventListener {
            config,
            propose_sender,
        }
    }

    pub(super) async fn listen_to_solana(&self) -> Result<(), ListenError> {
        let websocket_url = self.config.client.web_socket_url.clone().unwrap();
        info!(
            "Start listening for new solana events, url: {}, commitment: {}, program_id: {}",
            websocket_url, self.config.client.commitment.commitment, PROGRAM_ID
        );

        let program_id_str = PROGRAM_ID.to_string();
        let filter = RpcTransactionLogsFilter::Mentions(vec![program_id_str.clone()]);
        let config = RpcTransactionLogsConfig {
            commitment: Some(self.config.client.commitment),
        };

        let client = PubsubClient::new(&websocket_url).await.map_err(|err| {
            error!("Failed to create solana pubsub client: {}", err);
            ListenError::SolanaClient
        })?;

        let (mut notifications, unsubscribe) =
            client.logs_subscribe(filter, config).await.map_err(|err| {
                error!("Failed to subscribe for logs on solana: {}", err);
                ListenError::SolanaClient
            })?;

        while let Some(logs) = notifications.next().await {
            let Ok(events): Result<Vec<ProposeEvent>, ListenError> =
                parse_logs_response(logs.clone(), &program_id_str)
            else {
                log::error!("Failed to parse logs: {:?}", logs);
                continue;
            };
            debug!(
                "Logs intercepted, tx_signature: {}, events: {}",
                logs.value.signature,
                events.len()
            );

            let Ok(signature) = Signature::from_str(&logs.value.signature).map_err(|err| {
                error!(
                    "Failed to deserialize tx signature from base58: {}, error: {}",
                    logs.value.signature, err
                )
            }) else {
                continue;
            };

            for event in events {
                let Ok(protocol_id) = event.protocol_id.first_chunk().ok_or_else(|| {
                    error!("Failed to get 32 bytes protocol_id chunk from event data, skip")
                }) else {
                    continue;
                };
                debug!("Solana event intercepted: {:?}", event);
                if let Err(err) = self.propose_sender.send(Propose {
                    latest_block_id: logs.value.signature.clone(),
                    operation_data: OperationData {
                        src_chain_id: self.config.chain_id,
                        src_block_number: logs.context.slot,
                        src_op_tx_id: signature.as_ref().to_vec(),
                        protocol_id: ProtocolId(*protocol_id),
                        nonce: event.nonce,
                        dest_chain_id: event.dst_chain_id,
                        protocol_addr: event.protocol_address,
                        function_selector: event.function_selector,
                        params: event.params,
                    },
                }) {
                    error!("Failed to send operation_data through the channel: {}", err);
                    continue;
                }
            }
        }
        unsubscribe().await;
        info!("Subscription for events cancelled");
        Ok(())
    }
}
