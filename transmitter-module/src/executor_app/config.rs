use config::{Config, File};
use log::{error, info};
use serde::{de::Error, Deserialize, Deserializer};
use solana_sdk::{self, bs58, signature::Keypair};

use transmitter_common::{
    config::ReconnectConfig,
    mongodb::MongodbConfig,
    rabbitmq_client::{RabbitmqBindingConfig, RabbitmqConnectConfig},
};

use super::error::ExecutorError;
use crate::common::config::SolanaClientConfig;

#[derive(Debug, Deserialize)]
pub(super) struct ExecutorConfig {
    pub(super) extensions: Vec<String>,
    pub(super) rabbitmq: RabbitmqConfig,
    pub(super) solana: SolanaExecutorConfig,
    pub(super) mongodb: MongodbConfig,
}

#[derive(Debug, Deserialize)]
pub(super) struct RabbitmqConfig {
    #[serde(flatten)]
    pub(super) connect: RabbitmqConnectConfig,
    #[serde(flatten)]
    pub(super) binding: RabbitmqBindingConfig,
    pub(super) consumer_tag: String,
    pub(super) queue: String,
    #[serde(flatten)]
    pub(super) reconnect: ReconnectConfig,
}

#[derive(Debug, Deserialize)]
pub(super) struct SolanaExecutorConfig {
    #[serde(deserialize_with = "deserialize_keypair")]
    pub(super) payer: Keypair,
    #[serde(flatten)]
    pub(super) client: SolanaClientConfig,
    #[serde(flatten)]
    pub(super) suspending_config: SuspendingConfig,
    pub(super) executor_attempts: usize,
}

#[derive(Debug, Deserialize)]
pub(super) struct SuspendingConfig {
    pub(super) check_balance_period: usize,
    pub(super) warn_balance_lamports: u64,
    pub(super) suspend_balance_lamports: u64,
}

impl ExecutorConfig {
    pub(super) fn try_from_path(config: &str) -> Result<ExecutorConfig, ExecutorError> {
        info!("Read config from path: {}", config);
        let config = Config::builder()
            .add_source(File::with_name(config))
            .add_source(config::Environment::with_prefix("ENTANGLE").separator("_"))
            .build()
            .map_err(|err| {
                error!("Failed to build envs due to the error: {}", err);
                ExecutorError::Config
            })?;
        config.try_deserialize().map_err(|err| {
            error!("Failed to deserialize config: {}", err);
            ExecutorError::Config
        })
    }
}

fn deserialize_keypair<'de, D>(deserializer: D) -> Result<Keypair, D::Error>
where
    D: Deserializer<'de>,
{
    let s = String::deserialize(deserializer)?;
    let keydata = bs58::decode(s)
        .into_vec()
        .map_err(|err| Error::custom(format!("Malformed keypair base58: {}", err)))?;
    Keypair::from_bytes(&keydata)
        .map_err(|err| Error::custom(format!("Malformed keypair bytes: {}", err)))
}
