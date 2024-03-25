use thiserror::Error;

#[derive(Debug, Error)]
pub(crate) enum ListenError {
    #[error("Config error")]
    Config,
    #[error("Rabbitmq client error")]
    Rabbitmq(#[from] amqprs::error::Error),
    #[error("Solana client error")]
    SolanaClient,
    #[error("Solana parse logs error")]
    SolanaParseLogs,
}
