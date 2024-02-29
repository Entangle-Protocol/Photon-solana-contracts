use config::{Config, File};
use log::{error, info};
use serde::{Deserialize, Deserializer};
use solana_sdk::commitment_config::CommitmentLevel;
use std::str::FromStr;

use super::error::ListenError;

#[derive(Debug, Deserialize)]
pub(super) struct ListenConfig {
    pub(super) rabbitmq: RabbitmqConfig,
    pub(super) solana: SolanaConfig,
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

#[derive(Debug, Deserialize)]
pub(super) struct RabbitmqConfig {
    pub(super) host: String,
    pub(super) port: u16,
    pub(super) exchange: String,
    pub(super) routing_key: String,
    pub(super) user: String,
    pub(super) password: String,
}

#[derive(Debug, Deserialize)]
pub(super) struct SolanaConfig {
    pub(super) web_socket_url: String,
    pub(super) commitment: CommitmentLevel,
    #[serde(deserialize_with = "deserialize_chain_id")]
    pub(super) chain_id: u128,
}

fn deserialize_chain_id<'de, D>(deserializer: D) -> Result<u128, D::Error>
where
    D: Deserializer<'de>,
{
    let chain_id_string = String::deserialize(deserializer)?;
    u128::from_str(&chain_id_string).map_err(serde::de::Error::custom)
}
