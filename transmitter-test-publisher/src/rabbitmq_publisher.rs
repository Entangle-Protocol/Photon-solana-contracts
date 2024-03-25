use amqprs::{
    callbacks::{ChannelCallback, ConnectionCallback},
    channel::{BasicPublishArguments, Channel},
    connection::Connection,
    Ack, BasicProperties, Cancel, Close, CloseChannel, Nack, Return,
};
use async_trait::async_trait;
use log::{debug, error, info};
use serde::Deserialize;
use transmitter_common::{
    data::{KeeperMsg, KeeperMsgImpl, KeeperSignature, OperationData, SignedOperation},
    rabbitmq_client::{RabbitmqBindingConfig, RabbitmqClient, RabbitmqConnectConfig},
};

use super::PublisherError;

#[derive(Deserialize)]
pub(super) struct RabbitmqConfig {
    #[serde(flatten)]
    connect: RabbitmqConnectConfig,
    #[serde(flatten)]
    binding: RabbitmqBindingConfig,
}

pub(super) struct RabbitmqPublisher {
    config: RabbitmqConfig,
}

impl RabbitmqPublisher {
    pub(super) fn new(config: RabbitmqConfig) -> RabbitmqPublisher {
        RabbitmqPublisher { config }
    }

    pub(super) async fn publish_operation_data(
        &self,
        operation_data: OperationData,
        signatures: Vec<KeeperSignature>,
    ) -> Result<(), PublisherError> {
        let connection = self.connect(&self.config.connect).await?;
        let channel = self.open_channel(&connection).await?;

        info!(
            "Rabbitmq messaging arguments are: exchange: {}, routing_key: {}",
            self.config.binding.exchange, self.config.binding.routing_key
        );

        let msg = KeeperMsg::V1(KeeperMsgImpl::SignedOperationData(SignedOperation {
            operation_data,
            signatures,
        }));

        debug!("operation_data to be sent: {}", serde_json::to_string(&msg).unwrap());
        let json_data = serde_json::to_vec(&msg).expect("Expected operation be serialized well");

        let args = BasicPublishArguments::from(&self.config.binding);
        if let Err(err) = channel.basic_publish(BasicProperties::default(), json_data, args).await {
            error!("Failed to publish operation_data message, error: {}", err);
            return Err(PublisherError::from(err));
        }

        Ok(())
    }
}

#[async_trait]
impl RabbitmqClient for RabbitmqPublisher {
    type ConnCb = ConnectionControl;
    type ChanCb = ChannelControl;
    type Error = PublisherError;
}

#[derive(Default)]
pub(crate) struct ConnectionControl;

#[async_trait]
impl ConnectionCallback for ConnectionControl {
    async fn close(&mut self, _: &Connection, _: Close) -> Result<(), amqprs::error::Error> {
        Ok(())
    }
    async fn blocked(&mut self, _: &Connection, _: String) {}
    async fn unblocked(&mut self, _: &Connection) {}
}

#[derive(Default)]
pub struct ChannelControl;

#[async_trait::async_trait]
impl ChannelCallback for ChannelControl {
    async fn close(&mut self, _: &Channel, _: CloseChannel) -> Result<(), amqprs::error::Error> {
        Ok(())
    }
    async fn cancel(&mut self, _: &Channel, _: Cancel) -> Result<(), amqprs::error::Error> {
        Ok(())
    }
    async fn flow(&mut self, _: &Channel, _: bool) -> Result<bool, amqprs::error::Error> {
        Ok(true)
    }
    async fn publish_ack(&mut self, _: &Channel, _: Ack) {}
    async fn publish_nack(&mut self, _: &Channel, _: Nack) {}
    async fn publish_return(&mut self, _: &Channel, _: Return, _: BasicProperties, _: Vec<u8>) {}
}
