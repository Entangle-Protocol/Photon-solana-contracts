use crate::util::{u128_to_bytes32, u64_to_bytes32, EthAddress};
use anchor_lang::prelude::*;
use sha3::{Digest, Keccak256};

const MSG: &str = "\x19Ethereum Signed Message:\n32";

#[derive(Clone, AnchorSerialize, AnchorDeserialize, Debug)]
pub struct KeeperSignature {
    pub v: u8,
    pub r: Vec<u8>,
    pub s: Vec<u8>,
}

#[derive(Clone, AnchorSerialize, AnchorDeserialize, Debug)]
pub struct OperationData {
    pub protocol_id: Vec<u8>,
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

pub fn derive_eth_address(public_key: &[u8]) -> Result<EthAddress> {
    require_eq!(public_key[0], 0x04);
    let hash = Keccak256::digest(&public_key[1..]);
    let mut bytes = [0u8; 20];
    bytes.copy_from_slice(&hash[12..]);
    Ok(bytes)
}

/*pub fn check_signature(public_key: &[u8], sig: &KeeperSignature, op_hash: &[u8]) -> Result<bool> {
    let mut buf = Vec::new();
    buf.extend_from_slice(&sig.r);
    buf.extend_from_slice(&sig.s);
    require_eq!(buf.len(), 64);
    require_eq!(public_key.len(), 65);
    let public_key: [u8; 64] = public_key.try_into().unwrap();
    let verifying_key = VerifyingKey::from_bytes(&public_key);
    if let Err(e) = verifying_key.as_ref() {
        require_eq!("Error", format!("{}", e));
    }
    let verifying_key = verifying_key.unwrap();
    require_eq!("1", "2");
    let sig: Signature = Signature::from_bytes(&buf.try_into().unwrap());
    Ok(verifying_key.verify(&op_hash, &sig).is_ok())
}*/

/*pub fn recover_address(sig: &KeeperSignature, op_hash: &[u8]) -> Result<EthAddress> {
    let mut buf = Vec::new();
    buf.extend_from_slice(&sig.r);
    buf.extend_from_slice(&sig.s);
    buf.extend_from_slice(&[sig.v]);
    require_eq!(buf.len(), 65);
    let s_raw = k256::ecdsa::Signature::try_from(&buf[..64]);
    require!(s_raw.is_ok(), CustomError::InvalidSignature);
    let s_raw = s_raw.unwrap();
    let id = Id::new(buf[64] - 27).unwrap();
    let signature = Signature::new(&s_raw, id).unwrap();
    let key = signature
        .recover_verify_key_from_digest_bytes(op_hash.into())
        .map_err(|_| CustomError::InvalidSignature)?;
    require_eq!("1", "2");
    Ok(derive_eth_address(&key))
}
*/
