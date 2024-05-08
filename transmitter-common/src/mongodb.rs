use serde::Deserialize;

use super::SOLANA_CHAIN_ID;

#[derive(Clone, Debug, Deserialize)]
pub struct MongodbConfig {
    pub user: String,
    pub password: String,
    pub uri: String,
    pub db: String,
    pub key: String,
}
pub const MDB_LAST_BLOCK_COLLECTION: &str = "last_processed_blocks";

pub fn mdb_solana_chain_id() -> String {
    SOLANA_CHAIN_ID.to_string()
}
