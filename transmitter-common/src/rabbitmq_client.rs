use amqprs::{
    callbacks::{ChannelCallback, ConnectionCallback},
    channel::{BasicPublishArguments, Channel, ConfirmSelectArguments},
    connection::{Connection, OpenConnectionArguments},
};
use async_trait::async_trait;
use log::{error, info};
use serde::Deserialize;
use std::error::Error;

#[derive(Debug, Deserialize)]
pub struct RabbitmqConnectConfig {
    pub host: String,
    pub port: u16,
    pub user: String,
    pub password: String,
}

#[derive(Debug, Deserialize)]
pub struct RabbitmqBindingConfig {
    pub exchange: String,
    pub routing_key: String,
}

#[async_trait]
pub trait RabbitmqClient {
    type ConnCb: ConnectionCallback + Default + Send + 'static;
    type ChanCb: ChannelCallback + Default + Send + 'static;
    type Error: Error + Send + From<amqprs::error::Error> + 'static;

    async fn connect(&self, config: &RabbitmqConnectConfig) -> Result<Connection, Self::Error> {
        let RabbitmqConnectConfig {
            host,
            port,
            user,
            password,
        } = config;

        info!("Rabbitmq connect, host: {}, port: {}", host, port);
        let connection =
            Connection::open(&OpenConnectionArguments::new(host, *port, user, password))
                .await
                .map_err(|err| {
                    error!("Failed to connect to the rabbitmq: {}", err);
                    Self::Error::from(err)
                })?;

        connection.register_callback(Self::ConnCb::default()).await.map_err(|err| {
            error!("Failed to register connection callback: {}", err);
            Self::Error::from(err)
        })?;

        Ok(connection)
    }

    async fn open_channel(&self, connection: &Connection) -> Result<Channel, Self::Error> {
        info!("Open channel through rabbitmq connection: {}", connection);
        let channel = connection.open_channel(None).await.map_err(|err| {
            error!("Failed to open rabbitmq channel: {}", err);
            Self::Error::from(err)
        })?;

        channel.confirm_select(ConfirmSelectArguments::new(true)).await.map_err(|err| {
            error!("Failed to confirm select: {}", err);
            Self::Error::from(err)
        })?;

        channel.register_callback(Self::ChanCb::default()).await.map_err(|err| {
            error!("Failed to register rabbitmq callback: {}", err);
            Self::Error::from(err)
        })?;

        Ok(channel)
    }
}

impl From<&RabbitmqBindingConfig> for BasicPublishArguments {
    fn from(value: &RabbitmqBindingConfig) -> Self {
        BasicPublishArguments::new(&value.exchange, &value.routing_key)
    }
}
