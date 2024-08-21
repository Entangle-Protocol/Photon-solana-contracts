use amqprs::{
    callbacks::{ChannelCallback, ConnectionCallback},
    channel::{BasicPublishArguments, Channel, ConfirmSelectArguments},
    connection::{Connection, OpenConnectionArguments},
};
use async_trait::async_trait;
use log::{error, info, warn};
use serde::Deserialize;
use std::{error::Error, time::Duration};

use super::config::ReconnectConfig;

#[derive(Clone, Debug, Deserialize)]
pub struct RabbitmqConnectConfig {
    pub host: String,
    pub port: u16,
    pub user: String,
    pub password: String,
}

#[derive(Clone, Debug, Deserialize)]
pub struct RabbitmqBindingConfig {
    pub exchange: String,
    pub routing_key: String,
}

#[async_trait]
pub trait RabbitmqClient {
    type Error: Error + Send + From<amqprs::error::Error> + 'static;

    async fn connect(
        &self,
        config: &RabbitmqConnectConfig,
        conn_cb: impl ConnectionCallback + Send + 'static,
    ) -> Result<Connection, Self::Error> {
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

        connection.register_callback(conn_cb).await.map_err(|err| {
            error!("Failed to register connection callback: {}", err);
            Self::Error::from(err)
        })?;

        Ok(connection)
    }

    async fn open_channel(
        &self,
        connection: &Connection,
        chan_cb: impl ChannelCallback + Send + 'static,
    ) -> Result<Channel, Self::Error> {
        info!("Open channel through rabbitmq connection: {}", connection);
        let channel = connection.open_channel(None).await.map_err(|err| {
            error!("Failed to open rabbitmq channel: {}", err);
            Self::Error::from(err)
        })?;

        channel.confirm_select(ConfirmSelectArguments::new(true)).await.map_err(|err| {
            error!("Failed to confirm select: {}", err);
            Self::Error::from(err)
        })?;

        channel.register_callback(chan_cb).await.map_err(|err| {
            error!("Failed to register rabbitmq callback: {}", err);
            Self::Error::from(err)
        })?;

        Ok(channel)
    }

    async fn init_connection(&self) -> Result<(), Self::Error> {
        let mut attemts = 0;
        let config = self.reconnect_config().clone();
        while let Err(err) = self.reconnect().await {
            attemts += 1;
            warn!("Failed to connect to the rabbitmq, attempt: {}", attemts);
            if attemts == config.attempts {
                return Err(err);
            }
            tokio::time::sleep(Duration::from_millis(config.timeout_ms)).await;
        }
        info!("Rabbitmq connected");
        Ok(())
    }

    async fn reconnect(&self) -> Result<(), Self::Error> {
        panic!("This method is can not to be excluded with cfg(feature) because of clippy");
    }

    fn reconnect_config(&self) -> &ReconnectConfig {
        panic!("This method is can not to be excluded with cfg(feature) because of clippy");
    }
}

impl From<&RabbitmqBindingConfig> for BasicPublishArguments {
    fn from(value: &RabbitmqBindingConfig) -> Self {
        BasicPublishArguments::new(&value.exchange, &value.routing_key)
    }
}
