use log::{debug, error};
use mongodb::{
    bson::{doc, Document},
    options::{ClientOptions, Credential, ServerApi, ServerApiVersion, UpdateOptions},
    Client,
};
use std::collections::{BTreeMap, BTreeSet};
use std::time::{Duration, Instant};
use tokio::sync::{mpsc::UnboundedReceiver, Mutex};
use transmitter_common::data::OpHash;

use transmitter_common::mongodb::{mdb_solana_chain_id, MongodbConfig, MDB_LAST_BLOCK_COLLECTION};

use super::{error::ExecutorError, ExecutorOpStatus, OpAcknowledge};

const WRITE_ACK_TIMEOUT_SEC: u64 = 1;
const BLOCK_COLLECTING_TIMEOUT_SEC: u64 = 5;

pub(super) struct LastBlockUpdater {
    client: Client,
    acknowledge_receiver: Mutex<UnboundedReceiver<OpAcknowledge>>,
    db: String,
    last_block_key: String,
    op_registry: Mutex<OpRegistry>,
}

impl LastBlockUpdater {
    pub(super) async fn try_new(
        mongodb_config: MongodbConfig,
        tx_receiver: UnboundedReceiver<OpAcknowledge>,
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
            acknowledge_receiver: Mutex::new(tx_receiver),
            db: mongodb_config.db,
            last_block_key: mongodb_config.key,
            op_registry: Mutex::new(OpRegistry::new(Duration::from_secs(
                BLOCK_COLLECTING_TIMEOUT_SEC,
            ))),
        })
    }

    pub(super) async fn execute(&self) -> Result<(), ExecutorError> {
        tokio::select! {
            result = self.process_acknowledges() => result,
            result = self.write_acknowledges() => result
        }
    }

    async fn process_acknowledges(&self) -> Result<(), ExecutorError> {
        while let Some(acknowledge) = self.acknowledge_receiver.lock().await.recv().await {
            debug!("Operation acknowledge received: {}", acknowledge);
            self.on_acknowledge(acknowledge).await;
        }
        Ok(())
    }

    async fn write_acknowledges(&self) -> Result<(), ExecutorError> {
        let db = self.client.database(&self.db);
        let collection = db.collection::<Document>(MDB_LAST_BLOCK_COLLECTION);
        let chain_id = mdb_solana_chain_id();
        let update_options = UpdateOptions::builder().upsert(true).build();
        loop {
            tokio::time::sleep(Duration::from_secs(WRITE_ACK_TIMEOUT_SEC)).await;
            if let Some(block_number) = self.op_registry.lock().await.get_block_to_ack() {
                let in_ms = transmitter_common::utils::get_time_ms();
                debug!(
                    "Write acknowledge, last_processed_block: {}, updated_at: {}",
                    block_number, in_ms
                );
                collection
                    .update_one(
                        doc! { "direction": "to", "chain": &chain_id },
                        doc! { "$set": { & self.last_block_key: block_number.to_string(), "updated_at": in_ms as i64 }  },
                        update_options.clone(),
                    )
                    .await
                    .map_err(|err| {
                        error!("Failed to update last_processed_block: {}", err);
                        ExecutorError::from(err)
                    })?;
            }
        }
    }

    async fn on_acknowledge(&self, acknowledge: OpAcknowledge) {
        let mut registry = self.op_registry.lock().await;
        registry.on_acknowledge(acknowledge);
    }
}

struct BlockInfo {
    created_at: Instant,
    ops: BTreeSet<OpHash>,
}
#[derive(Default)]
struct OpRegistry {
    known_ops: BTreeMap<u64, BlockInfo>,
    ack_timeout: Duration,
}

impl OpRegistry {
    fn new(ack_timeout: Duration) -> Self {
        Self {
            known_ops: BTreeMap::default(),
            ack_timeout,
        }
    }

    fn on_acknowledge(&mut self, acknowledge: OpAcknowledge) {
        let block_info: &mut BlockInfo =
            self.known_ops.entry(acknowledge.block_number).or_insert(BlockInfo {
                created_at: Instant::now(),
                ops: BTreeSet::default(),
            });

        match acknowledge.status {
            ExecutorOpStatus::New => Self::on_new_operation(block_info, acknowledge.op_hash),
            ExecutorOpStatus::Executed => {
                Self::on_executed_operation(block_info, acknowledge.op_hash)
            }
            unexpected => error!("Unexpected operation status: {:?}", unexpected),
        }
    }

    fn on_new_operation(block_info: &mut BlockInfo, op_hash: OpHash) {
        if !block_info.ops.insert(op_hash) {
            error!("Duplicated operation: {}", hex::encode(op_hash))
        }
    }

    fn on_executed_operation(block_info: &mut BlockInfo, op_hash: OpHash) {
        if !block_info.ops.remove(&op_hash) {
            error!("Failed to remove operation, does not exist: {}", hex::encode(op_hash))
        }
    }

    fn get_block_to_ack(&mut self) -> Option<u64> {
        let mut block = None;
        loop {
            let Some(entry) = self.known_ops.first_entry() else {
                break;
            };
            let block_info = entry.get();
            let elapsed = block_info.created_at.elapsed();
            if elapsed > self.ack_timeout && block_info.ops.is_empty() {
                let (block_number, _) = entry.remove_entry();
                block = Some(block_number);
            } else {
                break;
            }
        }
        block
    }
}

#[cfg(test)]
mod test {
    use crate::executor_app::last_block_updater::OpRegistry;
    use crate::executor_app::{ExecutorOpStatus, OpAcknowledge};
    use rand::RngCore;
    use std::time::Duration;
    use transmitter_common::data::OpHash;

    fn gen_op_hash() -> OpHash {
        let mut op_hash = OpHash::default();
        rand::thread_rng().fill_bytes(&mut op_hash);
        op_hash
    }

    #[tokio::test]
    async fn test_op_registry_single_op_gets_executed() {
        let mut op_registry = OpRegistry::new(Duration::from_millis(100));
        let op_hash = gen_op_hash();
        op_registry.on_acknowledge(OpAcknowledge::new(1, op_hash, ExecutorOpStatus::New));
        assert_eq!(op_registry.get_block_to_ack(), None);
        op_registry.on_acknowledge(OpAcknowledge::new(1, op_hash, ExecutorOpStatus::Executed));
        assert_eq!(op_registry.get_block_to_ack(), None);
        tokio::time::sleep(Duration::from_millis(101)).await;
        assert_eq!(op_registry.get_block_to_ack(), Some(1));
    }

    #[tokio::test]
    async fn test_op_registry_one_out_of_few_gets_executed() {
        let mut op_registry = OpRegistry::new(Duration::from_millis(100));
        let op_hash1 = gen_op_hash();
        let op_hash2 = gen_op_hash();
        op_registry.on_acknowledge(OpAcknowledge::new(1, op_hash1, ExecutorOpStatus::New));
        op_registry.on_acknowledge(OpAcknowledge::new(1, op_hash2, ExecutorOpStatus::New));
        assert_eq!(op_registry.get_block_to_ack(), None);
        op_registry.on_acknowledge(OpAcknowledge::new(1, op_hash2, ExecutorOpStatus::Executed));
        assert_eq!(op_registry.get_block_to_ack(), None);
        tokio::time::sleep(Duration::from_millis(101)).await;
        assert_eq!(op_registry.get_block_to_ack(), None);
    }

    #[tokio::test]
    async fn test_op_undetermined_execution() {
        let mut op_registry = OpRegistry::new(Duration::from_millis(100));
        let op_hash1 = gen_op_hash();
        op_registry.on_acknowledge(OpAcknowledge::new(1, op_hash1, ExecutorOpStatus::New));
        let op_hash21 = gen_op_hash();
        op_registry.on_acknowledge(OpAcknowledge::new(2, op_hash21, ExecutorOpStatus::New));
        let op_hash22 = gen_op_hash();
        op_registry.on_acknowledge(OpAcknowledge::new(2, op_hash22, ExecutorOpStatus::New));
        let op_hash3 = gen_op_hash();
        op_registry.on_acknowledge(OpAcknowledge::new(3, op_hash3, ExecutorOpStatus::New));
        assert_eq!(op_registry.get_block_to_ack(), None);
        op_registry.on_acknowledge(OpAcknowledge::new(3, op_hash3, ExecutorOpStatus::Executed));
        op_registry.on_acknowledge(OpAcknowledge::new(2, op_hash21, ExecutorOpStatus::Executed));
        tokio::time::sleep(Duration::from_millis(200)).await;
        assert_eq!(op_registry.get_block_to_ack(), None);
        op_registry.on_acknowledge(OpAcknowledge::new(1, op_hash1, ExecutorOpStatus::Executed));
        assert_eq!(op_registry.get_block_to_ack(), Some(1));
        assert_eq!(op_registry.get_block_to_ack(), None);
        op_registry.on_acknowledge(OpAcknowledge::new(2, op_hash22, ExecutorOpStatus::Executed));
        assert_eq!(op_registry.get_block_to_ack(), Some(3));
        assert_eq!(op_registry.get_block_to_ack(), None);
    }
}
