use serde::Deserialize;

#[derive(Clone, Debug, Deserialize)]
pub struct ReconnectConfig {
    #[serde(
        default = "ReconnectConfig::default_reconnect_attempts",
        rename = "reconnect_attempts"
    )]
    pub attempts: usize,
    #[serde(
        default = "ReconnectConfig::default_reconnect_timeout_ms",
        rename = "reconnect_timeout_ms"
    )]
    pub timeout_ms: u64,
}

impl ReconnectConfig {
    fn default_reconnect_timeout_ms() -> u64 {
        500
    }
    fn default_reconnect_attempts() -> usize {
        20
    }
}
