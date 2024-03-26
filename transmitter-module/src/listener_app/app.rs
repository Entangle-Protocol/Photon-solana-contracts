use tokio::sync::mpsc::unbounded_channel;

use super::{
    config::ListenConfig, rabbitmq_publisher::RabbitmqPublisher, solana_listener::SolanaListener,
};

pub(crate) struct ListenerApp {
    solana_listener: SolanaListener,
    rabbitmq_sender: RabbitmqPublisher,
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
        let (op_data_sender, op_data_receiver) = unbounded_channel();
        ListenerApp {
            solana_listener: SolanaListener::new(config.solana, op_data_sender),
            rabbitmq_sender: RabbitmqPublisher::new(config.rabbitmq, op_data_receiver),
        }
    }

    async fn execute_impl(&mut self) {
        tokio::select! {
            _ = self.solana_listener.listen_to_solana() => {}
            _ = self.rabbitmq_sender.publish_to_rabbitmq() => {}
        }
    }
}
