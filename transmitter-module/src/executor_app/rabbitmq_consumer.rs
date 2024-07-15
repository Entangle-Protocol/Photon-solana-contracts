use amqprs::{
    channel::{
        BasicAckArguments, BasicGetArguments, Channel, ConfirmSelectArguments,
        ExchangeDeclareArguments, QueueBindArguments, QueueDeclareArguments,
    },
    connection::Connection,
    GetOk,
};
use async_trait::async_trait;
use log::{debug, error, info, warn};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::{mpsc::Sender, Notify};

use transmitter_common::{
    config::ReconnectConfig,
    data::{SignedOperation, TransmitterMsg, TransmitterMsgImpl},
    rabbitmq_client::RabbitmqClient,
};

use super::{config::RabbitmqConfig, error::ExecutorError};
use crate::common::rabbitmq::{ChannelControl, ConnectionControl};

pub(super) struct RabbitmqConsumer {
    config: RabbitmqConfig,
    op_data_sender: Sender<SignedOperation>,
    close_notify: Arc<Notify>,
    connection: Option<Connection>,
    channel: Option<Channel>,
}

impl RabbitmqConsumer {
    pub(super) fn new(
        config: RabbitmqConfig,
        op_data_sender: Sender<SignedOperation>,
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
        self.init_connection().await?;

        let channel = self.channel.as_ref().expect("Expected rabbitmq channel to be set");
        let exchange = &self.config.binding.exchange;
        let exch_args = ExchangeDeclareArguments::new(exchange, "direct").durable(true).finish();
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

        loop {
            let Ok(Some((get_ok, _props, data))) = channel
                .basic_get(BasicGetArguments {
                    queue: queue_name.clone(),
                    no_ack: false,
                })
                .await
            else {
                tokio::time::sleep(Duration::from_secs(1)).await;
                continue;
            };
            self.process_operation_data(channel, get_ok, data).await;
        }
    }

    async fn process_operation_data(&self, channel: &Channel, delivery: GetOk, data: Vec<u8>) {
        let args = BasicAckArguments::new(delivery.delivery_tag(), false);
        if let Err(err) = channel.basic_ack(args).await {
            error!("Failed to do basic ack: {}", err);
            return;
        }
        debug!("Ack to delivery: {}, on channel: {}", delivery.delivery_tag(), channel);

        let Ok(data) = String::from_utf8(data)
            .map_err(|err| error!("Failed to convert data to string: {}", err))
        else {
            return;
        };

        let signed_operation = match serde_json::from_str(&data) {
            Ok(TransmitterMsg::V1(TransmitterMsgImpl::SignedOperationData(signed_operation))) => {
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
            "New message consumed, exchange: {}, routing_key: {}, delivery_tag: {}, msg: {}",
            delivery.exchange(),
            delivery.routing_key(),
            delivery.delivery_tag(),
            signed_operation,
        );

        if self.op_data_sender.send(signed_operation).await.is_err() {
            error!("Failed to send signed operation to the op_data_sender");
        }
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
