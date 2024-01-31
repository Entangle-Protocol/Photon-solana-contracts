use crate::{
    util::{u128_to_bytes32, u64_to_bytes32, EthAddress},
    CustomError,
};
use anchor_lang::{prelude::*, solana_program::secp256k1_recover::secp256k1_recover};
use sha3::{Digest, Keccak256};

const MSG: &str = "\x19Ethereum Signed Message:\n32";

#[derive(Clone, AnchorSerialize, AnchorDeserialize, Debug)]
pub struct KeeperSignature {
    pub v: u8,
    pub r: Vec<u8>,
    pub s: Vec<u8>,
}

#[derive(Clone, AnchorSerialize, AnchorDeserialize, Debug, Default)]
pub struct OperationData {
    pub protocol_id: Vec<u8>, // [u8; 32] is zeroed out due to bug
    pub src_chain_id: u128,
    pub src_block_number: u64,
    pub src_op_tx_id: Vec<u8>,
    pub nonce: u64,
    pub dest_chain_id: u128,
    pub protocol_addr: Pubkey,
    pub function_selector: [u8; 4],
    pub params: Vec<u8>,
}

impl OperationData {
    pub fn op_data_evm(&self) -> Vec<u8> {
        let mut buf = Vec::new();
        buf.extend_from_slice(&self.protocol_id);
        buf.extend_from_slice(&u128_to_bytes32(self.src_chain_id));
        buf.extend_from_slice(&u64_to_bytes32(self.src_block_number));
        buf.extend_from_slice(&self.src_op_tx_id);
        buf.extend_from_slice(&u64_to_bytes32(self.nonce));
        buf.extend_from_slice(&u128_to_bytes32(self.dest_chain_id));
        buf.extend_from_slice(&self.protocol_addr.as_ref());
        buf.extend_from_slice(&self.function_selector);
        buf.extend_from_slice(&self.params);
        buf
    }

    pub fn op_hash(&self) -> Vec<u8> {
        let op_data_evm = self.op_data_evm();
        Keccak256::digest(op_data_evm).to_vec()
    }

    pub fn op_hash_with_message(&self) -> Vec<u8> {
        hash_with_message(&self.op_hash())
    }
}

pub fn hash_with_message(data: &[u8]) -> Vec<u8> {
    let mut buf = [0x00_u8; 32 + MSG.len()];
    buf[..MSG.len()].copy_from_slice(MSG.as_bytes());
    buf[MSG.len()..].copy_from_slice(data);
    Keccak256::digest(buf).to_vec()
}

pub fn ecrecover(op_hash: &[u8], sig: &KeeperSignature) -> Result<EthAddress> {
    let signature = [&sig.r[..], &sig.s[..]].concat();
    let v = sig.v % 27;
    require_eq!(signature.len(), 64);
    let pk =
        secp256k1_recover(op_hash, v, &signature).map_err(|_| CustomError::InvalidSignature)?;
    derive_eth_address(&[&[0x04], &pk.0[..]].concat())
}

pub fn derive_eth_address(public_key: &[u8]) -> Result<EthAddress> {
    let hash = Keccak256::digest(&public_key[1..]);
    let mut bytes = [0u8; 20];
    bytes.copy_from_slice(&hash[12..]);
    Ok(bytes)
}
