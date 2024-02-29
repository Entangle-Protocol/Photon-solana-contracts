use tokio::sync::mpsc::unbounded_channel;

use super::config::ListenConfig;
use super::rabbitmq_sender::RabbitmqSender;
use super::solana_listener::SolanaListener;

pub(crate) struct ListenerApp {
    solana_listener: SolanaListener,
    rabbitmq_sender: RabbitmqSender,
}

impl ListenerApp {
    pub(crate) async fn listen(config: &str) {
        let Ok(config) = ListenConfig::try_from_path(config) else {
            return;
        };

        let mut app = ListenerApp::new(config);
        app.execute().await;
    }

    fn new(config: ListenConfig) -> ListenerApp {
        let (sender, reciever) = unbounded_channel();
        ListenerApp {
            solana_listener: SolanaListener {
                config: config.solana,
                operation_data_sender: sender,
            },
            rabbitmq_sender: RabbitmqSender::new(config.rabbitmq, reciever),
        }
    }

    async fn execute(&mut self) {
        tokio::select! {
            _ = self.solana_listener.listen() => {}
            _ = self.rabbitmq_sender.handle_events() => {}
        }
    }
}
