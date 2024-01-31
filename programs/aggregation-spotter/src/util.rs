use anchor_lang::prelude::*;

pub type Bytes32 = [u8; 32];
pub type EthAddress = [u8; 20];

pub fn u64_to_bytes32(x: u64) -> [u8; 32] {
    let mut buf = [0; 32];
    buf[32 - 8..].copy_from_slice(&x.to_be_bytes());
    buf
}

pub fn u128_to_bytes32(x: u128) -> [u8; 32] {
    let mut buf = [0; 32];
    buf[32 - 16..].copy_from_slice(&x.to_be_bytes());
    buf
}

pub const fn gov_protocol_id() -> Bytes32 {
    *b"aggregation-gov_________________"
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone, Default, PartialEq, Eq)]
#[repr(u8)]
pub enum OpStatus {
    #[default]
    None,
    Init,
    Signed,
    Executed,
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone, Default)]
pub struct PhotonMsg {
    pub protocol_id: Bytes32,
    pub src_chain_id: u128,
    pub src_block_number: u64,
    pub src_op_tx_id: Bytes32,
    pub params_hash: Bytes32,
}
