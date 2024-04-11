use serde::Deserialize;

use super::SOLANA_CHAIN_ID;

#[derive(Debug, Deserialize)]
pub struct MongodbConfig {
    pub user: String,
    pub password: String,
    pub uri: String,
    pub db: String,
}
pub const MDB_LAST_BLOCK_COLLECTION: &str = "last_processed_blocks";

pub fn mdb_solana_chain_id() -> String {
    let binding = hex::encode(SOLANA_CHAIN_ID.to_be_bytes());
    let chain_id = binding.trim_start_matches('0');
    format!("0x{}", chain_id)
}
