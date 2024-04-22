use solana_sdk::{hash::Hash, instruction::AccountMeta, transaction::Transaction};
use std::panic::RefUnwindSafe;

use super::error::ExtensionError;

pub const GET_EXTENSION_EXPORT: &str = "get_extension";

pub trait ProtocolExtension: RefUnwindSafe {
    fn get_protocol_id(&self) -> &'static [u8; 32];
    fn get_accounts(
        &self,
        function_selector: &[u8],
        params: &[u8],
    ) -> Result<Vec<AccountMeta>, ExtensionError>;
    fn sign_transaction(
        &self,
        function_selector: &[u8],
        params: &[u8],
        transaction: &mut Transaction,
        recent_block_hash: &Hash,
    ) -> Result<(), ExtensionError>;

    fn get_compute_budget(&self, _function_selector: &[u8], _params: &[u8]) -> Option<u32> {
        None
    }
}
