mod cli;
mod rabbitmq;
mod test_config;

use amqprs::{
    channel::{BasicAckArguments, Channel},
    consumer::AsyncConsumer,
    BasicProperties, Deliver,
};
use async_trait::async_trait;
use config::{Config, Environment, File};
use log::{debug, error};
use mongodb::{
    bson::{doc, Document},
    options::{ClientOptions, Credential, ServerApi, ServerApiVersion, UpdateOptions},
    Client, Collection,
};
use rabbitmq::RabbitmqConsumer;
use std::env;

use transmitter_common::{
    data::TransmitterMsgImpl::Propose,
    mongodb::{mdb_solana_chain_id, MongodbConfig, MDB_LAST_BLOCK_COLLECTION},
    utils::get_time_ms,
};

use crate::test_config::TestConfig;

#[tokio::main]
async fn main() {
    env_logger::init();
    cli::Cli::execute(env::args()).await;
}

async fn execute(config_path: &str) {
    let config = Config::builder()
        .add_source(File::with_name(config_path))
        .add_source(Environment::with_prefix("ENTANGLE").separator("_"))
        .build()
        .expect("Expected test listener config be built");

    let config: TestConfig =
        config.try_deserialize().expect("Expected test_config be deserialized");

    RabbitmqConsumer {}.execute(config).await
}

struct Consumer {
    config: MongodbConfig,
    collection: Collection<Document>,
}

impl Consumer {
    async fn new(config: MongodbConfig) -> Consumer {
        let mut client_options = ClientOptions::parse_async(config.uri.clone())
            .await
            .expect("Expected mongo client_options be parsed");
        let server_api = ServerApi::builder().version(ServerApiVersion::V1).build();
        client_options.server_api = Some(server_api);
        client_options.credential = Some(
            Credential::builder()
                .username(config.user.clone())
                .password(config.password.clone())
                .build(),
        );
        let client =
            Client::with_options(client_options).expect("Expected client be created with options");
        let db = client.database("entangle");
        let collection = db.collection::<Document>(MDB_LAST_BLOCK_COLLECTION);
        Consumer { config, collection }
    }
}

#[async_trait]
impl AsyncConsumer for Consumer {
    async fn consume(
        &mut self,
        channel: &Channel,
        deliver: Deliver,
        _basic_properties: BasicProperties,
        data: Vec<u8>,
    ) {
        let Some(Propose(proposal)) =
            serde_json::from_slice(&data).expect("Expected proposal be consumed")
        else {
            error!("Unexpected data received");
            return;
        };
        let chain_id = mdb_solana_chain_id();

        self.collection
            .update_one(
                doc! { "direction": "from", "chain": chain_id },
                doc! { "$set": { &self.config.key: &proposal.latest_block_id, "updated_at": get_time_ms() as i64 } },
                UpdateOptions::builder().upsert(true).build(),
            )
            .await
            .expect("Expected last_processed_block be updated");
        let args = BasicAckArguments::new(deliver.delivery_tag(), false);
        if let Err(err) = channel.basic_ack(args).await {
            error!("Failed to do basic ack: {}", err);
        } else {
            debug!("Propose message consumed, latest_block_id: {}", proposal.latest_block_id);
        }
    }
}
