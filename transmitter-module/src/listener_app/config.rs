use config::{Config, File};
use log::{error, info};
use serde::Deserialize;

use transmitter_common::mongodb::MongodbConfig;

use super::error::ListenError;
use crate::common::{config::SolanaListenerConfig, rabbitmq::RabbitmqListenConfig};

#[derive(Deserialize)]
pub(super) struct ListenConfig {
    pub(super) rabbitmq: RabbitmqListenConfig,
    pub(super) solana: SolanaListenerConfig,
    pub(super) mongodb: MongodbConfig,
    pub(super) allowed_protocols: Vec<String>,
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
