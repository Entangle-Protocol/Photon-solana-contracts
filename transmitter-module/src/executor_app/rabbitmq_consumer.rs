use amqprs::{
    channel::{
        BasicAckArguments, BasicConsumeArguments, Channel, ConfirmSelectArguments,
        QueueBindArguments, QueueDeclareArguments,
    },
    consumer::AsyncConsumer,
    BasicProperties, Deliver,
};
use log::{debug, error, info, warn};
use tokio::sync::mpsc::UnboundedSender;

use transmitter_common::{
    data::{KeeperMsg, KeeperMsgImpl, SignedOperation},
    rabbitmq_client::RabbitmqClient,
};

use super::{config::RabbitmqConfig, error::ExecutorError};
use crate::common::rabbitmq::{ChannelControl, ConnectionControl};

pub(super) struct RabbitmqConsumer {
    config: RabbitmqConfig,
    op_data_sender: UnboundedSender<SignedOperation>,
}

impl RabbitmqConsumer {
    pub(super) fn new(
        config: RabbitmqConfig,
        op_data_sender: UnboundedSender<SignedOperation>,
    ) -> RabbitmqConsumer {
        RabbitmqConsumer {
            config,
            op_data_sender,
        }
    }

    pub(super) async fn execute(self) -> Result<(), ExecutorError> {
        let connection = self.connect(&self.config.connect).await?;
        let channel = self.open_channel(&connection).await?;

        let (queue_name, _, _) = channel
            .queue_declare(QueueDeclareArguments::default().finish())
            .await
            .map_err(|err| {
                error!("Failed to declare queue: {}", err);
                ExecutorError::from(err)
            })?
            .expect("Expected declared queue to be some, no_wait = false");

        let exchange = &self.config.binding.exchange;
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

        let tag = channel.basic_consume(self, args).await.map_err(|err| {
            error!("Failed basic consume from the channel: {}", err);
            ExecutorError::from(err)
        })?;
        info!("Consuming messages with consumer_tag started: {}", tag);
        tokio::time::sleep(tokio::time::Duration::from_secs(u64::MAX)).await;
        Ok(())
    }
}

#[async_trait::async_trait]
impl AsyncConsumer for RabbitmqConsumer {
    async fn consume(
        &mut self,
        channel: &Channel,
        deliver: Deliver,
        _basic_properties: BasicProperties,
        data: Vec<u8>,
    ) {
        let signed_operation = match serde_json::from_slice(&data) {
            Ok(KeeperMsg::V1(KeeperMsgImpl::SignedOperationData(signed_operation))) => {
                signed_operation
            }
            Ok(msg) => {
                warn!("Received unexpected data: {:? }", msg);
                return;
            }
            Err(err) => {
                error!("Failed to deserialize message: {}, data: {}", err, hex::encode(data));
                return;
            }
        };

        debug!(
            "New message consumed, exchange: {}, tag: {}, msg: {:?}",
            deliver.exchange(),
            deliver.consumer_tag(),
            signed_operation,
        );

        let args = BasicAckArguments::new(deliver.delivery_tag(), false);
        if let Err(err) = channel.basic_ack(args).await {
            error!("Failed to do basic ack: {}", err);
            return;
        }
        debug!("Ack to delivery {} on channel {}", deliver, channel);
        self.op_data_sender.send(signed_operation).expect("Expected signed_operation to be sent");
    }
}

impl RabbitmqClient for RabbitmqConsumer {
    type ConnCb = ConnectionControl;
    type ChanCb = ChannelControl;
    type Error = ExecutorError;
}
