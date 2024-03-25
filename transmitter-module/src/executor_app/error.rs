use thiserror::Error;

#[derive(Debug, Error)]
pub(crate) enum ExecutorError {
    #[error("Config error")]
    Config,
    #[error("Protocol extensions error")]
    Extensions,
    #[error("Malformed operation data")]
    MalformedData,
    #[error("Rabbitmq client error")]
    Rabbitmq(#[from] amqprs::error::Error),
    #[error("Solana client error")]
    SolanaClient,
}
