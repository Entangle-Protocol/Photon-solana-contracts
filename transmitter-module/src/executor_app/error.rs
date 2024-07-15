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
    #[error("Mongodb client error")]
    Mongodb(#[from] mongodb::error::Error),
    #[error("Solana transactor error {0}")]
    SolanaTransactorError(#[from] solana_transactor::TransactorError),
    #[error("Solana client error {0}")]
    SolanaClientError(#[from] solana_client::client_error::ClientError),
}
