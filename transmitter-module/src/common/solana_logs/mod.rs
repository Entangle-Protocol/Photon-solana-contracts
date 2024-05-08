use log::error;
use thiserror::Error;

pub(crate) mod event_processor;
pub(crate) mod parse_logs;
pub(crate) mod solana_event_listener;
pub(crate) mod solana_retro_reader;

#[derive(Debug, Error)]
pub(crate) enum EventListenerError {
    #[error("Config error")]
    Config,
    #[error("Solana client error")]
    SolanaClient,
    #[error("Mongodb client error")]
    Mongodb(#[from] mongodb::error::Error),
    #[error("Solana parse logs error")]
    SolanaParseLogs,
}
