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

pub fn sighash(namespace: &str, name: &str) -> [u8; 8] {
    let preimage = format!("{}:{}", namespace, name);
    let mut sighash = [0u8; 8];
    sighash.copy_from_slice(
        &anchor_lang::solana_program::hash::hash(preimage.as_bytes()).to_bytes()[..8],
    );
    sighash
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

#[macro_export]
macro_rules! require_ok {
    ($res:expr, $err:expr) => {
        match $res {
            Ok(x) => x,
            Err(e) => {
                require_eq!("", format!("Error: {:?}", e), $err);
                unreachable!();
            }
        }
    };
}
