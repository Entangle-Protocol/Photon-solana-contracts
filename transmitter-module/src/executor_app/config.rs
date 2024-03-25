use config::{Config, File};
use log::{error, info};
use serde::{de::Error, Deserialize, Deserializer};
use solana_sdk::{
    bs58,
    commitment_config::{CommitmentConfig, CommitmentLevel},
    signature::Keypair,
};

use transmitter_common::rabbitmq_client::{RabbitmqBindingConfig, RabbitmqConnectConfig};

use super::error::ExecutorError;

#[derive(Debug, Deserialize)]
pub(super) struct ExecutorConfig {
    pub(super) extensions: Vec<String>,
    pub(super) rabbitmq: RabbitmqConfig,
    pub(super) solana: SolanaExecutorConfig,
}

#[derive(Debug, Deserialize)]
pub(super) struct RabbitmqConfig {
    #[serde(flatten)]
    pub(super) connect: RabbitmqConnectConfig,
    #[serde(flatten)]
    pub(super) binding: RabbitmqBindingConfig,
    pub(super) consumer_tag: String,
}

#[derive(Clone, Debug, Deserialize)]
pub(super) struct SolanaClientConfig {
    #[serde(deserialize_with = "deserialize_commitment")]
    pub(super) commitment: CommitmentConfig,
    pub(super) url: String,
}

#[derive(Debug, Deserialize)]
pub(super) struct SolanaExecutorConfig {
    #[serde(deserialize_with = "deserialize_keypair")]
    pub(super) payer: Keypair,
    #[serde(flatten)]
    pub(super) client: SolanaClientConfig,
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
where D: Deserializer<'de> {
    let s = String::deserialize(deserializer)?;
    let keydata = bs58::decode(s)
        .into_vec()
        .map_err(|err| Error::custom(format!("Malformed keypair base58: {}", err)))?;
    Keypair::from_bytes(&keydata)
        .map_err(|err| Error::custom(format!("Malformed keypair bytes: {}", err)))
}

fn deserialize_commitment<'de, D>(deserializer: D) -> Result<CommitmentConfig, D::Error>
where D: Deserializer<'de> {
    let commitment = CommitmentLevel::deserialize(deserializer)
        .map_err(|err| Error::custom(format!("Malformed commitment: {}", err)))?;
    Ok(CommitmentConfig { commitment })
}
