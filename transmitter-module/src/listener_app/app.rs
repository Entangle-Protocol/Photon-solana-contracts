use crate::common::solana_logs::solana_event_listener::SolanaEventListener;
use log::info;
use tokio::sync::mpsc::unbounded_channel;

use super::{
    config::ListenConfig, rabbitmq_publisher::RabbitmqPublisher,
    solana_logs_processor::ProposalEventProcessor,
};

pub(crate) struct ListenerApp {
    solana_listener: SolanaEventListener,
    rabbitmq_sender: RabbitmqPublisher,
    solana_logs_proc: ProposalEventProcessor,
}

impl ListenerApp {
    pub(crate) async fn execute(config_path: &str) {
        info!("Application restarted {}", chrono::Local::now().format("%Y-%m-%d %H:%M:%S"));
        let Ok(config) = ListenConfig::try_from_path(config_path) else {
            return;
        };

        let mut app = ListenerApp::new(config);
        app.execute_impl().await;
    }

    fn new(config: ListenConfig) -> ListenerApp {
        Self::trace_config(&config);
        let (propose_sender, propose_receiver) = unbounded_channel();
        let (logs_sender, logs_receiver) = unbounded_channel();
        ListenerApp {
            solana_listener: SolanaEventListener::new(config.solana, config.mongodb, logs_sender),
            rabbitmq_sender: RabbitmqPublisher::new(config.rabbitmq, propose_receiver),
            solana_logs_proc: ProposalEventProcessor::new(
                logs_receiver,
                propose_sender,
                config.allowed_protocols,
            ),
        }
    }

    fn trace_config(config: &ListenConfig) {
        info!("solana_commitment: {}", config.solana.client.commitment.commitment);

        for rpc in &config.solana.client.read_rpcs {
            info!("solana_read_rpc: {}, rate_limit: {}", rpc.url, rpc.ratelimit);
        }

        for rpc in &config.solana.client.write_rpcs {
            info!("solana_write_rpc: {}, rate_limit: {}", rpc.url, rpc.ratelimit);
        }

        info!(
            "mongodb. uri: {}, user: {}, db: {}, key: {}",
            config.mongodb.uri, config.mongodb.user, config.mongodb.db, config.mongodb.key
        );
        info!(
            "rabbitmq. host: {}, port: {}, user: {},  binding: {:?},  reconnect: {:?}",
            config.rabbitmq.connect.host,
            config.rabbitmq.connect.port,
            config.rabbitmq.connect.user,
            config.rabbitmq.binding,
            config.rabbitmq.reconnect
        );
        info!("allowed_protocols: {}", config.allowed_protocols.join(", "));
    }

    async fn execute_impl(&mut self) {
        tokio::select! {
            _ = self.solana_listener.listen_to_solana() => {}
            _ = self.rabbitmq_sender.publish_to_rabbitmq() => {}
            _ = self.solana_logs_proc.execute() => {}
        }
    }
}
