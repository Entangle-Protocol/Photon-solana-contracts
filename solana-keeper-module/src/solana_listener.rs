mod solana_logs;

use anyhow::Result;
use futures_util::stream::StreamExt;
use log::info;
use photon::{ProposeEvent, ID as PROGRAM_ID};
use solana_client::{
    nonblocking::pubsub_client::PubsubClient, rpc_config::RpcTransactionLogsConfig,
    rpc_config::RpcTransactionLogsFilter,
};
use solana_logs::parse_logs_response;
use solana_sdk::commitment_config::{CommitmentConfig, CommitmentLevel};
use std::time::Duration;
use tokio::sync::mpsc::{unbounded_channel, UnboundedReceiver, UnboundedSender};

pub(super) struct SolanaListener;

const HANDLE_EVENTS_WARM_UP_SEC: u64 = 2;

impl SolanaListener {
    pub(super) async fn listen(web_socket_url: &str, commitment: CommitmentLevel) -> Result<()> {
        info!(
            "Start listening for new events, cluster: {}, commitment: {}, program_id: {}",
            web_socket_url, commitment, PROGRAM_ID
        );
        let (sender, receiver) = unbounded_channel::<ProposeEvent>();
        tokio::select! {
            res = Self::do_subscription(web_socket_url, commitment, sender) => res?,
            _ = Self::handle_events(receiver) => {}
        };
        Ok(())
    }

    async fn do_subscription(
        web_socket_url: &str,
        commitment: CommitmentLevel,
        sender: UnboundedSender<ProposeEvent>,
    ) -> Result<()> {
        let program_id_str = PROGRAM_ID.to_string();
        let filter = RpcTransactionLogsFilter::Mentions(vec![program_id_str.clone()]);
        let config = RpcTransactionLogsConfig {
            commitment: Some(CommitmentConfig { commitment }),
        };
        let client = PubsubClient::new(web_socket_url).await?;
        let (mut notifications, unsubscribe) = client.logs_subscribe(filter, config).await.unwrap();
        while let Some(logs) = notifications.next().await {
            let Ok(events): Result<Vec<ProposeEvent>> =
                parse_logs_response(logs.clone(), &program_id_str)
            else {
                log::error!("Failed to parse logs: {:?}", logs);
                continue;
            };
            for e in events {
                sender.send(e).unwrap()
            }
        }
        unsubscribe().await;
        info!("Subscription for events cancelled");
        Ok(())
    }

    async fn handle_events(mut receiver: UnboundedReceiver<ProposeEvent>) {
        tokio::time::sleep(Duration::from_secs(HANDLE_EVENTS_WARM_UP_SEC)).await;
        while let Some(event) = receiver.recv().await {
            info!("Event has been gotten: {:?}", event);
        }
        info!("Events handling stopped")
    }
}
