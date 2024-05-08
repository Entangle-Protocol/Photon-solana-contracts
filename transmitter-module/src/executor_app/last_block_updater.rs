use log::{debug, error};
use mongodb::{
    bson::{doc, Document},
    options::{ClientOptions, Credential, ServerApi, ServerApiVersion, UpdateOptions},
    Client,
};
use tokio::sync::{mpsc::UnboundedReceiver, Mutex};

use transmitter_common::mongodb::{mdb_solana_chain_id, MongodbConfig, MDB_LAST_BLOCK_COLLECTION};

use super::error::ExecutorError;

pub(super) struct LastBlockUpdater {
    client: Client,
    last_block_receiver: Mutex<UnboundedReceiver<u64>>,
    db: String,
    last_block_key: String,
}

impl LastBlockUpdater {
    pub(super) async fn try_new(
        mongodb_config: MongodbConfig,
        tx_receiver: UnboundedReceiver<u64>,
    ) -> Result<LastBlockUpdater, ExecutorError> {
        let mut client_options =
            ClientOptions::parse_async(mongodb_config.uri).await.map_err(|err| {
                error!("Failed to parse client_options: {}", err);
                ExecutorError::from(err)
            })?;
        let server_api = ServerApi::builder().version(ServerApiVersion::V1).build();
        client_options.server_api = Some(server_api);
        client_options.credential = Some(
            Credential::builder()
                .username(mongodb_config.user)
                .password(mongodb_config.password)
                .build(),
        );
        let client = Client::with_options(client_options).map_err(|err| {
            error!("Failed to create mongodb client");
            ExecutorError::from(err)
        })?;

        Ok(LastBlockUpdater {
            client,
            last_block_receiver: Mutex::new(tx_receiver),
            db: mongodb_config.db,
            last_block_key: mongodb_config.key,
        })
    }

    pub(super) async fn execute(&self) -> Result<(), ExecutorError> {
        let db = self.client.database(&self.db);
        let collection = db.collection::<Document>(MDB_LAST_BLOCK_COLLECTION);
        let chain_id = mdb_solana_chain_id();
        let update_options = UpdateOptions::builder().upsert(true).build();
        while let Some(last_block) = self.last_block_receiver.lock().await.recv().await {
            debug!("last_block_number_received to be updated: {}", last_block);
            let block_number = last_block.to_string();
            let in_ms = transmitter_common::utils::get_time_ms();
            collection
                .update_one(
                    doc! { "direction": "to", "chain": &chain_id },
                    doc! { "$set": { & self.last_block_key: block_number, "updated_at": in_ms as i64 }  },
                    update_options.clone(),
                )
                .await
                .map_err(|err| {
                    error!("Failed to update last_processed_block: {}", err);
                    ExecutorError::from(err)
                })?;
        }
        Ok(())
    }
}
