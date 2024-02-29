use amqprs::{
    callbacks::{ChannelCallback, ConnectionCallback},
    channel::{BasicPublishArguments, Channel, ConfirmSelectArguments},
    connection::{Connection, OpenConnectionArguments},
    Ack, BasicProperties, Cancel, Close, CloseChannel, Nack, Return,
};
use async_trait::async_trait;
use log::{debug, error, info, warn};
use tokio::sync::mpsc::UnboundedReceiver;

use super::config::RabbitmqConfig;
use super::data::{KeeperMsg, KeeperMsgImpl, OperationData};
use super::error::ListenError;

pub(super) struct RabbitmqSender {
    pub(super) config: RabbitmqConfig,
    pub(super) operation_data_receiver: UnboundedReceiver<OperationData>,
}

#[derive(Default)]
struct ConnectionControl;
#[derive(Default)]
struct ChannelControl;

impl RabbitmqSender {
    pub(super) fn new(
        config: RabbitmqConfig,
        receiver: UnboundedReceiver<OperationData>,
    ) -> RabbitmqSender {
        RabbitmqSender {
            config,
            operation_data_receiver: receiver,
        }
    }

    pub(super) async fn handle_events(&mut self) -> Result<(), ListenError> {
        let connection = self.connect().await?;
        let channel = Self::open_channel(&connection).await?;

        let args = BasicPublishArguments::new(&self.config.exchange, &self.config.routing_key);
        info!(
            "Rabbitmq messaging arguments are: exchange: {}, routing_key: {}",
            self.config.exchange, self.config.routing_key
        );
        while let Some(operation_data) = self.operation_data_receiver.recv().await {
            let msg = KeeperMsg::V1(KeeperMsgImpl::OperationData(operation_data));
            debug!("operation_data to be sent: {:?}", msg);
            let json_data = match serde_json::to_vec(&msg) {
                Ok(json_data) => json_data,
                Err(err) => {
                    error!(
                        "Failed to encode operation_data message: {:?}, error: {}",
                        msg, err
                    );
                    continue;
                }
            };
            if let Err(err) = channel
                .basic_publish(BasicProperties::default(), json_data, args.clone())
                .await
            {
                error!("Failed to publish operation_data message, error: {}", err);
            }
        }
        info!("Events handling stopped");
        Ok(())
    }

    async fn connect(&self) -> Result<Connection, ListenError> {
        let RabbitmqConfig {
            host,
            port,
            exchange: _,
            routing_key: _,
            user,
            password,
        } = &self.config;

        info!("Rabbitmq connect, host: {}, port: {}", host, port);
        let connection =
            Connection::open(&OpenConnectionArguments::new(host, *port, user, password))
                .await
                .map_err(|err| {
                    error!("Failed to connect to the rabbitmq: {}", err);
                    ListenError::Rabbitmq
                })?;

        connection
            .register_callback(ConnectionControl)
            .await
            .map_err(|err| {
                error!("Failed to register connection callback: {}", err);
                ListenError::Rabbitmq
            })?;

        Ok(connection)
    }

    async fn open_channel(connection: &Connection) -> Result<Channel, ListenError> {
        info!("Open channel over rabbitmq connection: {}", connection);
        let channel = connection.open_channel(None).await.map_err(|err| {
            error!("Failed to open rabbitmq channel: {}", err);
            ListenError::Rabbitmq
        })?;

        channel
            .confirm_select(ConfirmSelectArguments::new(true))
            .await
            .map_err(|err| {
                error!("Failed to confirm select: {}", err);
                ListenError::Rabbitmq
            })?;

        channel
            .register_callback(ChannelControl)
            .await
            .map_err(|err| {
                error!("Failed to register rabbitmq callback: {}", err);
                ListenError::Rabbitmq
            })?;

        Ok(channel)
    }
}

#[async_trait]
impl ConnectionCallback for ConnectionControl {
    async fn close(
        &mut self,
        connection: &Connection,
        close: Close,
    ) -> Result<(), amqprs::error::Error> {
        // TODO: reconnect should be implemented
        warn!(
            "Rabbitmq connection closed: {}, reason: {}",
            connection, close
        );
        Ok(())
    }

    async fn blocked(&mut self, connection: &Connection, reason: String) {
        warn!(
            "Rabbitmq connection blocked: {}, reason: {}",
            connection, reason
        );
    }

    async fn unblocked(&mut self, connection: &Connection) {
        info!("Rabbitmq connection unblocked: {}", connection);
    }
}

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
        debug!(
            "Publish ack delivery_tag: {}, channel: {}",
            ack.delivery_tag(),
            channel
        );
    }

    async fn publish_nack(&mut self, channel: &Channel, nack: Nack) {
        warn!(
            "Publish nack delivery_tag: {}, channel: {}",
            nack.delivery_tag(),
            channel
        );
    }

    async fn publish_return(
        &mut self,
        channel: &Channel,
        ret: Return,
        _basic_properties: BasicProperties,
        content: Vec<u8>,
    ) {
        info!(
            "Publish return: {} on channel: {}, content size: {}",
            ret,
            channel,
            content.len()
        );
    }
}
