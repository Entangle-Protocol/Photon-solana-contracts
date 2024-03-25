use amqprs::{
    callbacks::{ChannelCallback, ConnectionCallback},
    channel::Channel,
    connection::Connection,
    Ack, BasicProperties, Cancel, Close, CloseChannel, Nack, Return,
};
use async_trait::async_trait;
use log::{debug, error, info, warn};

#[derive(Default)]
pub(crate) struct ConnectionControl;

#[async_trait]
impl ConnectionCallback for ConnectionControl {
    async fn close(
        &mut self,
        connection: &Connection,
        close: Close,
    ) -> Result<(), amqprs::error::Error> {
        // TODO: reconnect should be implemented
        warn!("Rabbitmq connection closed: {}, reason: {}", connection, close);
        Ok(())
    }

    async fn blocked(&mut self, connection: &Connection, reason: String) {
        warn!("Rabbitmq connection blocked: {}, reason: {}", connection, reason);
    }

    async fn unblocked(&mut self, connection: &Connection) {
        info!("Rabbitmq connection unblocked: {}", connection);
    }
}

#[derive(Default)]
pub(crate) struct ChannelControl;

#[async_trait::async_trait]
impl ChannelCallback for ChannelControl {
    async fn close(
        &mut self,
        channel: &Channel,
        close: CloseChannel,
    ) -> Result<(), amqprs::error::Error> {
        warn!(
            "Not implemented. Rabbitmq requested to close the channel: {}, cause: {}",
            channel, close
        );
        Ok(())
    }

    async fn cancel(
        &mut self,
        channel: &Channel,
        cancel: Cancel,
    ) -> Result<(), amqprs::error::Error> {
        error!(
            "Not implemented. Rabbitmq requested to cancel consuming on channel: {}, consumer: {}",
            channel,
            cancel.consumer_tag()
        );
        Ok(())
    }

    async fn flow(
        &mut self,
        channel: &Channel,
        active: bool,
    ) -> Result<bool, amqprs::error::Error> {
        // TODO: implement suspending until rabbitmq channel is unlocked
        warn!(
            "Not implemented. Rabbitmq requested to change the flow, channel: {}, active: {}",
            channel, active
        );
        Ok(true)
    }

    async fn publish_ack(&mut self, channel: &Channel, ack: Ack) {
        debug!("Publish ack delivery_tag: {}, channel: {}", ack.delivery_tag(), channel);
    }

    async fn publish_nack(&mut self, channel: &Channel, nack: Nack) {
        warn!("Publish nack delivery_tag: {}, channel: {}", nack.delivery_tag(), channel);
    }

    async fn publish_return(
        &mut self,
        channel: &Channel,
        ret: Return,
        _basic_properties: BasicProperties,
        content: Vec<u8>,
    ) {
        info!("Publish return: {} on channel: {}, content size: {}", ret, channel, content.len());
    }
}
