use solana_sdk::{
    hash::Hash, instruction::AccountMeta, signer::SignerError, transaction::Transaction,
};
use std::panic::RefUnwindSafe;

pub const GET_EXTENSION_EXPORT: &str = "get_extension";

pub trait ProtocolExtension: RefUnwindSafe {
    fn get_protocol_id(&self) -> &'static [u8; 32];
    fn get_accounts(&self, function_selector: &[u8], params: &[u8]) -> Vec<AccountMeta>;
    fn sign_transaction(
        &self,
        function_selector: &[u8],
        params: &[u8],
        transaction: &mut Transaction,
        recent_block_hash: &Hash,
    ) -> Result<(), SignerError>;
}
