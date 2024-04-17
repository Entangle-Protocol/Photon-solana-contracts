use amqprs::{
    channel::{
        BasicAckArguments, BasicConsumeArguments, Channel, ConfirmSelectArguments,
        ExchangeDeclareArguments, QueueBindArguments, QueueDeclareArguments,
    },
    connection::Connection,
    consumer::AsyncConsumer,
    BasicProperties, Deliver,
};
use async_trait::async_trait;
use log::{debug, error, info, warn};
use std::sync::Arc;
use tokio::sync::{mpsc::UnboundedSender, Notify};

use transmitter_common::{
    config::ReconnectConfig,
    data::{KeeperMsg, KeeperMsgImpl, SignedOperation},
    rabbitmq_client::RabbitmqClient,
};

use super::{config::RabbitmqConfig, error::ExecutorError};
use crate::common::rabbitmq::{ChannelControl, ConnectionControl};

pub(super) struct RabbitmqConsumer {
    config: RabbitmqConfig,
    op_data_sender: UnboundedSender<SignedOperation>,
    close_notify: Arc<Notify>,
    connection: Option<Connection>,
    channel: Option<Channel>,
}

struct Consumer(UnboundedSender<SignedOperation>);

impl RabbitmqConsumer {
    pub(super) fn new(
        config: RabbitmqConfig,
        op_data_sender: UnboundedSender<SignedOperation>,
    ) -> RabbitmqConsumer {
        RabbitmqConsumer {
            config,
            op_data_sender,
            close_notify: Arc::new(Notify::new()),
            connection: None,
            channel: None,
        }
    }

    pub(super) async fn execute(mut self) -> Result<(), ExecutorError> {
        loop {
            self.init_connection().await?;

            let channel = self.channel.as_mut().expect("Expected rabbitmq channel to be set");
            let exchange = &self.config.binding.exchange;
            let exch_args =
                ExchangeDeclareArguments::new(exchange, "direct").durable(true).finish();
            channel.exchange_declare(exch_args).await.map_err(|err| {
                error!("Failed to declare exchange: {}, error: {}", exchange, err);
                ExecutorError::from(err)
            })?;

            let queue_args = QueueDeclareArguments::default()
                .queue(self.config.queue.clone())
                .durable(true)
                .finish();
            let (queue_name, _, _) = channel
                .queue_declare(queue_args)
                .await
                .map_err(|err| {
                    error!("Failed to declare queue: {}", err);
                    ExecutorError::from(err)
                })?
                .expect("Expected declared queue to be some, no_wait = false");

            let routing_key = &self.config.binding.routing_key;

            channel
                .queue_bind(QueueBindArguments::new(&queue_name, exchange, routing_key))
                .await
                .map_err(|err| {
                error!("Failed to bind queue: {}", err);
                ExecutorError::from(err)
            })?;

            info!(
                "Queue created: {}, has been bound to the exchange: {}, routing key: {}",
                queue_name, exchange, routing_key
            );

            channel.confirm_select(ConfirmSelectArguments::new(true)).await.map_err(|err| {
                error!("Failed to confirm_select: {}", err);
                ExecutorError::from(err)
            })?;
            let consumer_tag = &self.config.consumer_tag;
            let args = BasicConsumeArguments::new(&queue_name, consumer_tag);

            let consumer = Consumer(self.op_data_sender.clone());
            let tag = channel.basic_consume(consumer, args).await.map_err(|err| {
                error!("Failed basic consume from the channel: {}", err);
                ExecutorError::from(err)
            })?;

            info!("Consuming messages with consumer_tag started: {}", tag);
            self.close_notify.notified().await;
        }
    }
}

#[async_trait]
impl AsyncConsumer for Consumer {
    async fn consume(
        &mut self,
        channel: &Channel,
        deliver: Deliver,
        _basic_properties: BasicProperties,
        data: Vec<u8>,
    ) {
        let args = BasicAckArguments::new(deliver.delivery_tag(), false);
        if let Err(err) = channel.basic_ack(args).await {
            error!("Failed to do basic ack: {}", err);
            return;
        }
        debug!("Ack to delivery {} on channel {}", deliver, channel);
        let Ok(data) = String::from_utf8(data)
            .map_err(|err| error!("Failed to convert data to string: {}", err))
        else {
            return;
        };
        let signed_operation = match serde_json::from_str(&data) {
            Ok(KeeperMsg::V1(KeeperMsgImpl::SignedOperationData(signed_operation))) => {
                signed_operation
            }
            Ok(msg) => {
                warn!("Received unexpected data: {:? }", msg);
                return;
            }
            Err(err) => {
                error!("Failed to deserialize message: {}, data: {}", err, data);
                return;
            }
        };

        debug!(
            "New message consumed, exchange: {}, tag: {}, msg: {:?}",
            deliver.exchange(),
            deliver.consumer_tag(),
            signed_operation,
        );

        self.0.send(signed_operation).expect("Expected signed_operation to be sent");
    }
}

#[async_trait]
impl RabbitmqClient for RabbitmqConsumer {
    type Error = ExecutorError;

    async fn reconnect(&mut self) -> Result<(), ExecutorError> {
        let conn_control = ConnectionControl::new(self.close_notify.clone());
        let conn = self.connect(&self.config.connect, conn_control).await?;
        let chann_control = ChannelControl::new(self.close_notify.clone());
        self.channel = Some(self.open_channel(&conn, chann_control).await?);
        self.connection = conn.into();
        Ok(())
    }

    fn reconnect_config(&self) -> &ReconnectConfig {
        &self.config.reconnect
    }
}
