use thiserror::Error;
use transmitter_common::error::ExtensionError;

#[derive(Debug, Error)]
pub(crate) enum ExecutorError {
    #[error("Config error")]
    Config,
    #[error("Extension manager error")]
    ExtensionMng,
    #[error("Extension error")]
    Extension(#[from] ExtensionError),
    #[error("Malformed operation data")]
    MalformedData,
    #[error("Rabbitmq client error")]
    Rabbitmq(#[from] amqprs::error::Error),
    #[error("Solana client error")]
    SolanaClient,
    #[error("Mongodb client error")]
    Mongodb(#[from] mongodb::error::Error),
}
