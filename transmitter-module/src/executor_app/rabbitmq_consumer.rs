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
use std::{sync::Arc, time::Duration};
use tokio::{
    select,
    sync::{mpsc::Sender, Mutex, Notify},
};

use transmitter_common::{
    config::ReconnectConfig,
    data::{SignedOperation, TransmitterMsgImpl, TransmitterMsgVersioned},
    rabbitmq_client::RabbitmqClient,
};

use super::{config::RabbitmqConfig, error::ExecutorError};
use crate::common::rabbitmq::{ChannelControl, ConnectionControl};

pub(super) struct RabbitmqConsumer {
    config: RabbitmqConfig,
    op_data_sender: Sender<SignedOperation>,
    close_notify: Arc<Notify>,
    connection: Mutex<Option<(Connection, Channel)>>,
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
            connection: Mutex::new(None),
        }
    }

    pub(super) async fn execute(self) -> Result<(), ExecutorError> {
        self.init_connection().await?;
        let queue_name = self.init_rabbitmq_structure().await?;
        select! {
            _ = self.process_incoming_data(&queue_name) => Ok(()),
            res = self.process_reconnect(self.close_notify.clone()) => res
        }
    }

    async fn init_rabbitmq_structure(&self) -> Result<String, ExecutorError> {
        let guard = self.connection.lock().await;
        let (_, channel) = guard.as_ref().expect("Expected rabbitmq channel to be set");
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
        Ok(queue_name)
    }

    async fn process_incoming_data(&self, queue_name: &str) {
        loop {
            tokio::time::sleep(Duration::from_millis(200)).await;
            let guard = self.connection.lock().await;
            let Some((_, channel)) = guard.as_ref() else {
                continue;
            };
            if !channel.is_open() {
                continue;
            }
            loop {
                let Ok(Some((get_ok, _props, data))) =
                    channel.basic_get(BasicGetArguments::new(queue_name)).await
                else {
                    break;
                };
                self.process_operation_data(channel, get_ok, data).await
            }
        }
    }

    async fn process_reconnect(&self, notify: Arc<Notify>) -> Result<(), ExecutorError> {
        loop {
            notify.notified().await;
            self.init_connection().await?;
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
            Ok(TransmitterMsgVersioned::V1(TransmitterMsgImpl::SignedOperationData(
                signed_operation,
            ))) => signed_operation,
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

    async fn reconnect(&self) -> Result<(), ExecutorError> {
        let conn_control = ConnectionControl::new(self.close_notify.clone());
        let conn = self.connect(&self.config.connect, conn_control).await?;
        let chann_control = ChannelControl::new(self.close_notify.clone());
        let channel = self.open_channel(&conn, chann_control).await?;
        self.connection.lock().await.replace((conn, channel));
        Ok(())
    }

    fn reconnect_config(&self) -> &ReconnectConfig {
        &self.config.reconnect
    }
}
