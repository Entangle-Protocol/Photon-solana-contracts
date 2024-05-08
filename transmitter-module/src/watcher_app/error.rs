use thiserror::Error;

#[derive(Debug, Error)]
pub(crate) enum WatcherError {
    #[error("Config error")]
    Config,
    #[error("Rabbitmq client error")]
    Rabbitmq(#[from] amqprs::error::Error),
}
