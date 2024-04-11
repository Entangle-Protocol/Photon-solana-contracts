use serde::Deserialize;

use transmitter_common::{
    mongodb::MongodbConfig,
    rabbitmq_client::{RabbitmqBindingConfig, RabbitmqConnectConfig},
};

#[derive(Debug, Deserialize)]
pub(super) struct TestConfig {
    pub(super) rabbitmq: RabbitmqConfig,
    pub(super) mongodb: MongodbConfig,
}

#[derive(Debug, Deserialize)]
pub(super) struct RabbitmqConfig {
    #[serde(flatten)]
    pub(super) connect: RabbitmqConnectConfig,
    #[serde(flatten)]
    pub(super) binding: RabbitmqBindingConfig,
    pub(super) consumer_tag: String,
    pub(super) queue: String,
}
