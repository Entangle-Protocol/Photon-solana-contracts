use crate::{
    util::{u128_to_bytes32, u64_to_bytes32, EthAddress},
    CustomError,
};
use anchor_lang::{prelude::*, solana_program::secp256k1_recover::secp256k1_recover};
use sha3::{Digest, Keccak256};

pub type Meta = [u8; 32];

const MSG: &str = "\x19Ethereum Signed Message:\n32";

#[derive(Clone, AnchorSerialize, AnchorDeserialize, Debug)]
pub struct KeeperSignature {
    pub v: u8,
    pub r: Vec<u8>,
    pub s: Vec<u8>,
}

#[derive(Clone, AnchorSerialize, AnchorDeserialize, Debug, Default)]
pub struct Sample {
    value: String,
}

#[derive(Clone, AnchorSerialize, AnchorDeserialize, Debug, Default)]
pub enum FunctionSelector {
    ByCode(Vec<u8>),
    ByName(String),
    #[default]
    Dummy,
}

impl TryFrom<&[u8]> for FunctionSelector {
    type Error = Vec<u8>;
    fn try_from(value: &[u8]) -> std::result::Result<Self, Vec<u8>> {
        let mut iter = value.iter().copied();
        Ok(match iter.next().ok_or(vec![])? {
            0 => {
                let _len = iter.next().ok_or(vec![])?;
                let code: Vec<u8> = iter.collect();
                FunctionSelector::ByCode(code)
            }
            1 => {
                let _len = iter.next().ok_or(vec![])?;
                let name: Vec<u8> = iter.collect();
                FunctionSelector::ByName(String::from_utf8(name).map_err(|err| {
                    log::error!("Failed to get function_selector by name from data: {}", err);
                    vec![]
                })?)
            }
            _ => panic!("Unexpected function_selector type byte"),
        })
    }
}

#[derive(Clone, AnchorSerialize, AnchorDeserialize, Debug, Default)]
pub struct OperationData {
    pub protocol_id: Vec<u8>, // [u8; 32] is zeroed out due to bug
    pub meta: Meta,
    pub src_chain_id: u128,
    pub src_block_number: u64,
    pub src_op_tx_id: Vec<u8>,
    pub nonce: u64,
    pub dest_chain_id: u128,
    pub protocol_addr: Pubkey,
    pub function_selector: FunctionSelector,
    pub params: Vec<u8>,
    pub reserved: Vec<u8>,
}

impl OperationData {
    fn op_data_evm(&self) -> Vec<u8> {
        let mut buf = Vec::new();
        buf.extend_from_slice(&self.protocol_id);
        buf.extend_from_slice(&self.meta);
        buf.extend_from_slice(&u128_to_bytes32(self.src_chain_id));
        buf.extend_from_slice(&u64_to_bytes32(self.src_block_number));
        buf.extend_from_slice(&self.src_op_tx_id);
        buf.extend_from_slice(&u64_to_bytes32(self.nonce));
        buf.extend_from_slice(&u128_to_bytes32(self.dest_chain_id));
        buf.extend_from_slice(self.protocol_addr.as_ref());
        match &self.function_selector {
            FunctionSelector::ByCode(code) => {
                let mut code_selector = vec![0, 32];
                code_selector.extend_from_slice(code.as_slice());
                code_selector.resize(34, 0);
                buf.extend_from_slice(&code_selector)
            }
            FunctionSelector::ByName(name) => {
                let mut name_selector = vec![1, name.len() as u8];
                name_selector.extend_from_slice(name.as_bytes());
                buf.extend_from_slice(&name_selector)
            }
            FunctionSelector::Dummy => panic!("function_selector is not initialized"),
        }
        buf.extend_from_slice(&self.params);
        buf.extend_from_slice(&self.reserved);
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

pub fn derive_eth_address(public_key: &[u8]) -> EthAddress {
    let hash = Keccak256::digest(&public_key[1..]);
    let mut bytes = [0u8; 20];
    bytes.copy_from_slice(&hash[12..]);
    bytes
}

pub(crate) fn ecrecover(op_hash: &[u8], sig: &KeeperSignature) -> Result<EthAddress> {
    let signature = [&sig.r[..], &sig.s[..]].concat();
    let v = sig.v % 27;
    require_eq!(signature.len(), 64);
    let pk =
        secp256k1_recover(op_hash, v, &signature).map_err(|_| CustomError::InvalidSignature)?;
    Ok(derive_eth_address(&[&[0x04], &pk.0[..]].concat()))
}
