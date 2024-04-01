use serde::{de::Error, Deserialize, Deserializer};
use solana_sdk::commitment_config::{CommitmentConfig, CommitmentLevel};

#[derive(Clone, Debug, Deserialize)]
pub(crate) struct SolanaClientConfig {
    #[serde(deserialize_with = "deserialize_commitment")]
    pub(crate) commitment: CommitmentConfig,
    pub(crate) web_socket_url: Option<String>,
    pub(crate) rpc_url: String,
}

fn deserialize_commitment<'de, D>(deserializer: D) -> Result<CommitmentConfig, D::Error>
where D: Deserializer<'de> {
    let commitment = CommitmentLevel::deserialize(deserializer)
        .map_err(|err| Error::custom(format!("Malformed commitment: {}", err)))?;
    Ok(CommitmentConfig { commitment })
}
