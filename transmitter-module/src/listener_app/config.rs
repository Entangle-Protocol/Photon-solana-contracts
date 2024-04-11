use config::{Config, File};
use log::{error, info};
use serde::Deserialize;

use transmitter_common::{
    config::ReconnectConfig,
    mongodb::MongodbConfig,
    rabbitmq_client::{RabbitmqBindingConfig, RabbitmqConnectConfig},
};

use super::error::ListenError;
use crate::common::config::SolanaClientConfig;

#[derive(Deserialize)]
pub(super) struct ListenConfig {
    pub(super) rabbitmq: RabbitmqConfig,
    pub(super) solana: SolanaListenerConfig,
    pub(super) mongodb: MongodbConfig,
}

#[derive(Deserialize)]
pub(super) struct RabbitmqConfig {
    #[serde(flatten)]
    pub(super) connect: RabbitmqConnectConfig,
    #[serde(flatten)]
    pub(super) binding: RabbitmqBindingConfig,
    #[serde(flatten)]
    pub(super) reconnect: ReconnectConfig,
}

#[derive(Deserialize)]
pub(super) struct SolanaListenerConfig {
    #[serde(flatten)]
    pub(super) client: SolanaClientConfig,
    #[serde(flatten)]
    pub(super) reconnect: ReconnectConfig,
}

impl ListenConfig {
    pub(super) fn try_from_path(config: &str) -> Result<ListenConfig, ListenError> {
        info!("Read config from path: {}", config);
        let config = Config::builder()
            .add_source(File::with_name(config))
            .add_source(config::Environment::with_prefix("ENTANGLE").separator("_"))
            .build()
            .map_err(|err| {
                error!("Failed to build envs due to the error: {}", err);
                ListenError::Config
            })?;

        config.try_deserialize().map_err(|err| {
            error!("Failed to deserialize config: {}", err);
            ListenError::Config
        })
    }
}
