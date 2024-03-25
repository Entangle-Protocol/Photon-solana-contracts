use photon::util::{u128_to_bytes32, u64_to_bytes32};
use serde::{Deserialize, Serialize};
use sha3::{Digest, Keccak256};
use solana_sdk::pubkey::Pubkey;
use std::fmt::{Display, Formatter};

#[derive(Clone, Copy, Debug, Default)]
pub struct ProtocolId(pub ProtocolIdImpl);
pub type ProtocolIdImpl = [u8; 32];
pub type OpHash = [u8; 32];

impl Display for ProtocolId {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", String::from_utf8(self.0.to_vec()).unwrap_or_else(|_| hex::encode(self.0)))
    }
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(tag = "version")]
pub enum KeeperMsg {
    #[serde(rename = "1.0")]
    V1(KeeperMsgImpl),
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(tag = "type", content = "data", rename_all = "camelCase")]
pub enum KeeperMsgImpl {
    #[serde(rename = "operation")]
    OperationData(OperationData),
    #[serde(rename = "signedOperation")]
    SignedOperationData(SignedOperation),
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SignedOperation {
    #[serde(rename = "operation")]
    pub operation_data: OperationData,
    pub signatures: Vec<KeeperSignature>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct KeeperSignature {
    pub v: u8,
    pub r: Vec<u8>,
    pub s: Vec<u8>,
}

impl From<KeeperSignature> for photon::signature::KeeperSignature {
    fn from(value: KeeperSignature) -> Self {
        photon::signature::KeeperSignature {
            r: value.r,
            s: value.s,
            v: value.v,
        }
    }
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OperationData {
    #[serde(with = "protocol_id_serialization")]
    pub protocol_id: ProtocolId,
    #[serde(with = "u128_serialization")]
    pub src_chain_id: u128,
    pub src_block_number: u64,
    pub src_op_tx_id: Vec<u8>,
    pub nonce: u64,
    #[serde(with = "u128_serialization")]
    pub dest_chain_id: u128,
    pub protocol_addr: Vec<u8>,
    pub function_selector: Vec<u8>,
    pub params: Vec<u8>,
}

impl OperationData {
    pub fn op_hash_with_message(&self) -> OpHash {
        photon::signature::hash_with_message(&self.op_hash()).as_chunks().0[0]
    }

    fn op_hash(&self) -> Vec<u8> {
        let op_data_evm = self.op_data_evm();
        Keccak256::digest(op_data_evm).to_vec()
    }

    fn op_data_evm(&self) -> Vec<u8> {
        let mut buf = Vec::new();
        buf.extend_from_slice(&self.protocol_id.0);
        buf.extend_from_slice(&u128_to_bytes32(self.src_chain_id));
        buf.extend_from_slice(&u64_to_bytes32(self.src_block_number));
        buf.extend_from_slice(&self.src_op_tx_id);
        buf.extend_from_slice(&u64_to_bytes32(self.nonce));
        buf.extend_from_slice(&u128_to_bytes32(self.dest_chain_id));
        buf.extend_from_slice(self.protocol_addr.as_ref());
        buf.extend_from_slice(&self.function_selector);
        buf.extend_from_slice(&self.params);
        buf
    }
}

impl TryFrom<OperationData> for photon::signature::OperationData {
    type Error = Vec<u8>;
    fn try_from(value: OperationData) -> Result<Self, Self::Error> {
        Ok(photon::signature::OperationData {
            protocol_id: <Vec<u8>>::from(value.protocol_id.0),
            src_chain_id: value.src_chain_id,
            src_block_number: value.src_block_number,
            src_op_tx_id: value.src_op_tx_id,
            nonce: value.nonce,
            dest_chain_id: value.dest_chain_id,
            protocol_addr: Pubkey::try_from(value.protocol_addr)?,
            function_selector: value.function_selector,
            params: value.params,
        })
    }
}

mod u128_serialization {
    use serde::{Deserialize, Deserializer, Serializer};

    pub(super) fn serialize<S>(chain_id: &u128, serializer: S) -> Result<S::Ok, S::Error>
    where S: Serializer {
        serializer.serialize_bytes(&chain_id.to_be_bytes())
    }

    pub(super) fn deserialize<'de, D>(deserializer: D) -> Result<u128, D::Error>
    where D: Deserializer<'de> {
        let data = <[u8; 16]>::deserialize(deserializer)?;
        Ok(u128::from_be_bytes(data))
    }
}

mod protocol_id_serialization {
    use super::{ProtocolId, ProtocolIdImpl};
    use serde::{Deserialize, Deserializer, Serializer};

    pub(super) fn serialize<S>(protocol_id: &ProtocolId, serializer: S) -> Result<S::Ok, S::Error>
    where S: Serializer {
        serializer.serialize_bytes(&protocol_id.0)
    }

    pub(super) fn deserialize<'de, D>(deserializer: D) -> Result<ProtocolId, D::Error>
    where D: Deserializer<'de> {
        let data = ProtocolIdImpl::deserialize(deserializer)?;
        Ok(ProtocolId(data))
    }
}
