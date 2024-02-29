use futures_util::StreamExt;
use log::{error, info};
use solana_client::{
    nonblocking::pubsub_client::PubsubClient,
    rpc_config::{RpcTransactionLogsConfig, RpcTransactionLogsFilter},
};
use solana_sdk::commitment_config::CommitmentConfig;
use std::str::FromStr;
use tokio::sync::mpsc::UnboundedSender;

use photon::{ProposeEvent, ID as PROGRAM_ID};

use super::config::SolanaConfig;
use super::data::OperationData;
use super::error::ListenError;
use super::solana_logs::parse_logs_response;

pub(super) struct SolanaListener {
    pub(super) config: SolanaConfig,
    pub(super) operation_data_sender: UnboundedSender<OperationData>,
}

impl SolanaListener {
    pub(super) async fn listen(&self) -> Result<(), ListenError> {
        info!(
            "Start listening for new solana events, cluster: {}, commitment: {}, program_id: {}",
            self.config.web_socket_url, self.config.commitment, PROGRAM_ID
        );

        let program_id_str = PROGRAM_ID.to_string();
        let filter = RpcTransactionLogsFilter::Mentions(vec![program_id_str.clone()]);
        let config = RpcTransactionLogsConfig {
            commitment: Some(CommitmentConfig {
                commitment: self.config.commitment,
            }),
        };

        let client = PubsubClient::new(&self.config.web_socket_url)
            .await
            .map_err(|err| {
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
            let signature = solana_sdk::signature::Signature::from_str(&logs.value.signature)
                .map_err(|err| {
                    error!(
                        "Failed to deserialize tx signature from base58: {}, error: {}",
                        logs.value.signature, err
                    );
                    ListenError::SolanaClient
                })?;

            for event in events {
                self.operation_data_sender
                    .send(OperationData {
                        src_chain_id: self.config.chain_id,
                        src_block_number: logs.context.slot,
                        src_op_tx_id: signature.as_ref().to_vec(),
                        protocol_id: event.protocol_id,
                        nonce: event.nonce,
                        dest_chain_id: event.dst_chain_id,
                        protocol_addr: event.protocol_address,
                        function_selector: event.function_selector,
                        params: event.params,
                    })
                    .map_err(|err| {
                        error!("Failed to send operation_data through the channel: {}", err);
                        ListenError::ProposeEventChannel
                    })?
            }
        }
        unsubscribe().await;
        info!("Subscription for events cancelled");
        Ok(())
    }
}
