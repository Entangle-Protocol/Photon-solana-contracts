use thiserror::Error;

#[derive(Debug, Error)]
pub(crate) enum ListenError {
    #[error("Config error")]
    Config,
    #[error("Rabbitmq client error")]
    Rabbitmq(#[from] amqprs::error::Error),
    #[error("Mongodb client error")]
    Mongodb(#[from] mongodb::error::Error),
}
