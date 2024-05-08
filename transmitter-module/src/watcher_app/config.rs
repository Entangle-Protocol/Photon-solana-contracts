use config::{Config, File};
use log::{error, info};
use serde::Deserialize;
use transmitter_common::mongodb::MongodbConfig;

use crate::{
    common::{config::SolanaListenerConfig, rabbitmq::RabbitmqListenConfig},
    watcher_app::error::WatcherError,
};

#[derive(Deserialize)]
pub(super) struct WatcherConfig {
    pub(super) rabbitmq: RabbitmqListenConfig,
    pub(super) solana: SolanaListenerConfig,
    pub(super) mongodb: MongodbConfig,
}

impl WatcherConfig {
    pub(super) fn try_from_path(config: &str) -> Result<WatcherConfig, WatcherError> {
        info!("Read config from path: {}", config);
        let config = Config::builder()
            .add_source(File::with_name(config))
            .add_source(config::Environment::with_prefix("ENTANGLE").separator("_"))
            .build()
            .map_err(|err| {
                error!("Failed to build envs due to the error: {}", err);
                WatcherError::Config
            })?;

        config.try_deserialize().map_err(|err| {
            error!("Failed to deserialize config: {}", err);
            WatcherError::Config
        })
    }
}
