use crate::{
    common::solana_logs::solana_event_listener::SolanaEventListener,
    watcher_app::{
        config::WatcherConfig, rabbitmq_publisher::RabbitmqPublisher,
        solana_logs_processor::OperationExecutedEventProcessor,
    },
};
use tokio::sync::mpsc::unbounded_channel;

pub(crate) struct WatcherApp {
    solana_listener: SolanaEventListener,
    rabbitmq_sender: RabbitmqPublisher,
    solana_logs_proc: OperationExecutedEventProcessor,
}

impl WatcherApp {
    pub(crate) async fn execute(config_path: &str) {
        let Ok(config) = WatcherConfig::try_from_path(config_path) else {
            return;
        };

        let mut app = WatcherApp::new(config);
        app.execute_impl().await;
    }

    fn new(config: WatcherConfig) -> WatcherApp {
        let (op_stat_sender, op_stat_receiver) = unbounded_channel();
        let (logs_sender, logs_receiver) = unbounded_channel();

        WatcherApp {
            solana_listener: SolanaEventListener::new(config.solana, config.mongodb, logs_sender),
            solana_logs_proc: OperationExecutedEventProcessor::new(logs_receiver, op_stat_sender),
            rabbitmq_sender: RabbitmqPublisher::new(config.rabbitmq, op_stat_receiver),
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
