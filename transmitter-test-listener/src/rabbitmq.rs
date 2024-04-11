use crate::{test_config::TestConfig, Consumer};
use amqprs::{
    callbacks::{DefaultChannelCallback, DefaultConnectionCallback},
    channel::{
        BasicConsumeArguments, ConfirmSelectArguments, ExchangeDeclareArguments,
        QueueBindArguments, QueueDeclareArguments,
    },
};
use async_trait::async_trait;
use log::info;
use std::{
    fmt::{Display, Formatter},
    time::Duration,
};
use thiserror::Error;
use transmitter_common::rabbitmq_client::RabbitmqClient;

pub(super) struct RabbitmqConsumer;

impl RabbitmqConsumer {
    pub(super) async fn execute(self, config: TestConfig) {
        let connection = self
            .connect(&config.rabbitmq.connect, DefaultConnectionCallback)
            .await
            .expect("Expected rabbitmq consumer be connected");

        let channel = self
            .open_channel(&connection, DefaultChannelCallback)
            .await
            .expect("Expected rabbitmq channel be opened");
        let exchange = &config.rabbitmq.binding.exchange;
        let exch_args = ExchangeDeclareArguments::new(exchange, "direct").durable(true).finish();
        channel.exchange_declare(exch_args).await.expect("Expected exchange be declared");
        let queue_args = QueueDeclareArguments::default()
            .queue(config.rabbitmq.queue.clone())
            .durable(true)
            .finish();
        let (queue_name, _, _) = channel
            .queue_declare(queue_args)
            .await
            .expect("Expected queue be declared")
            .expect("Expected queue be returned");
        let routing_key = &config.rabbitmq.binding.routing_key;
        channel
            .queue_bind(QueueBindArguments::new(&queue_name, exchange, routing_key))
            .await
            .expect("Expected queue be bound to exchange");

        info!(
            "Queue created: {}, has been bound to the exchange: {}, routing key: {}",
            queue_name, exchange, routing_key
        );
        channel
            .confirm_select(ConfirmSelectArguments::new(true))
            .await
            .expect("Expected selecting arguments be confirmed");
        let consumer_tag = &config.rabbitmq.consumer_tag;
        let args = BasicConsumeArguments::new(&queue_name, consumer_tag);

        let consumer = Consumer::new(config.mongodb).await;
        let tag =
            channel.basic_consume(consumer, args).await.expect("Expected consuming be started");
        info!("Consuming messages with consumer_tag started: {}", tag);
        tokio::time::sleep(Duration::from_secs(u64::MAX)).await;
    }
}

#[async_trait]
impl RabbitmqClient for RabbitmqConsumer {
    type Error = TestError;
}

#[derive(Error, Debug)]
pub(super) struct TestError;

impl Display for TestError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "test error")
    }
}

impl From<amqprs::error::Error> for TestError {
    fn from(_value: amqprs::error::Error) -> Self {
        TestError
    }
}
