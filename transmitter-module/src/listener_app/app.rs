use tokio::sync::mpsc::unbounded_channel;

use super::{
    config::ListenConfig, rabbitmq_publisher::RabbitmqPublisher,
    solana_event_listener::SolanaEventListener, solana_logs_processor::SolanaLogsProcessor,
};

pub(crate) struct ListenerApp {
    solana_listener: SolanaEventListener,
    rabbitmq_sender: RabbitmqPublisher,
    solana_logs_proc: SolanaLogsProcessor,
}

impl ListenerApp {
    pub(crate) async fn execute(config_path: &str) {
        let Ok(config) = ListenConfig::try_from_path(config_path) else {
            return;
        };

        let mut app = ListenerApp::new(config);
        app.execute_impl().await;
    }

    fn new(config: ListenConfig) -> ListenerApp {
        let (propose_sender, propose_receiver) = unbounded_channel();
        let (logs_sender, logs_receiver) = unbounded_channel();
        ListenerApp {
            solana_listener: SolanaEventListener::new(config.solana, config.mongodb, logs_sender),
            rabbitmq_sender: RabbitmqPublisher::new(config.rabbitmq, propose_receiver),
            solana_logs_proc: SolanaLogsProcessor::new(logs_receiver, propose_sender),
        }
    }

    async fn execute_impl(&mut self) {
        tokio::select! {
            _ = self.solana_listener.listen_to_solana() => {}
            _ = self.rabbitmq_sender.publish_to_rabbitmq() => {}
            _ = self.solana_logs_proc.execute() => {}
        }
    }
}
