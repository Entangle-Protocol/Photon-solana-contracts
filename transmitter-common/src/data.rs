use hex;
use photon::{
    protocol_data::FunctionSelector,
    util::{u128_to_bytes32, u64_to_bytes32},
};
use serde::{Deserialize, Serialize};
use sha3::{Digest, Keccak256};
use solana_sdk::{bs58, pubkey::Pubkey};
use std::fmt::{Display, Formatter};

#[derive(Clone, Copy, Debug, Default)]
pub struct ProtocolId(pub ProtocolIdImpl);
pub type ProtocolIdImpl = [u8; 32];
pub type OpHash = [u8; 32];
pub type Meta = [u8; 32];

pub fn default_meta() -> Meta {
    [
        0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
        0, 1,
    ]
}

impl Display for ProtocolId {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", String::from_utf8(self.0.to_vec()).unwrap_or_else(|_| hex::encode(self.0)))
    }
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(tag = "version")]
pub enum TransmitterMsg {
    #[serde(rename = "1.0")]
    V1(TransmitterMsgImpl),
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(tag = "type", content = "data", rename_all = "camelCase")]
pub enum TransmitterMsgImpl {
    Propose(Propose),
    ProposalExecuted(ProposalExecuted),
    #[serde(rename = "signedOperation")]
    SignedOperationData(SignedOperation),
}

#[derive(Clone, Debug, derive_more::Display, Deserialize, Serialize)]
#[display(
    fmt = "{{ operation_data: {}, eob_block_number: {}, signatures: {} }}",
    operation_data,
    eob_block_number,
    "signatures.iter().map(|a| a.to_string()).collect::<Vec<String>>().join(\",\")"
)]
#[serde(rename_all = "camelCase")]
pub struct SignedOperation {
    #[serde(rename = "operation")]
    pub operation_data: OperationData,
    pub signatures: Vec<TransmitterSignature>,
    pub eob_block_number: u64,
}

#[derive(Clone, Debug, derive_more::Display, Deserialize, Serialize)]
#[display(fmt = "{:x}{}{}", v, "hex::encode(r)", "hex::encode(s)")]
pub struct TransmitterSignature {
    pub v: u8,
    pub r: Vec<u8>,
    pub s: Vec<u8>,
}

impl From<TransmitterSignature> for photon::protocol_data::TransmitterSignature {
    fn from(value: TransmitterSignature) -> Self {
        photon::protocol_data::TransmitterSignature {
            r: value.r,
            s: value.s,
            v: value.v,
        }
    }
}

#[derive(Clone, Debug, Default, derive_more::Display, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
#[display(
    fmt = "{{ protocol_id: \"{}\", meta: {}, src_chain_id: {}, src_block_number: {}, src_op_tx_id: \
              0x{}, nonce: {}, dest_chain_id: {}, protocol_addr: {}, function_selector: {}, params: \
              {}, reserved: {}}}",
    "String::from_utf8_lossy(&protocol_id.0)",
    "hex::encode(meta)",
    src_chain_id,
    src_block_number,
    "hex::encode(src_op_tx_id)",
    nonce,
    dest_chain_id,
    "bs58::encode(protocol_addr).into_string()",
    "hex::encode(function_selector)",
    "hex::encode(params)",
    "hex::encode(reserved)"
)]
pub struct OperationData {
    #[serde(with = "protocol_id_serialization")]
    pub protocol_id: ProtocolId,
    pub meta: Meta,
    #[serde(with = "u128_serialization")]
    pub src_chain_id: u128,
    pub src_block_number: u64,
    #[serde(with = "tx_id_serialization")]
    pub src_op_tx_id: Vec<u8>,
    pub nonce: u64,
    #[serde(with = "u128_serialization")]
    pub dest_chain_id: u128,
    pub protocol_addr: Vec<u8>,
    pub function_selector: Vec<u8>,
    pub params: Vec<u8>,
    pub reserved: Vec<u8>,
}

impl OperationData {
    pub fn op_hash_with_message(&self) -> OpHash {
        photon::protocol_data::hash_with_message(&self.op_hash())[..32]
            .try_into()
            .expect("Invalid ophash")
    }

    fn op_hash(&self) -> Vec<u8> {
        let op_data_evm = self.op_data_evm();
        Keccak256::digest(op_data_evm).to_vec()
    }

    pub fn op_data_evm(&self) -> Vec<u8> {
        let mut buf = Vec::new();
        buf.extend_from_slice(&self.protocol_id.0);
        buf.extend_from_slice(&self.meta);
        buf.extend_from_slice(&u128_to_bytes32(self.src_chain_id));
        buf.extend_from_slice(&u64_to_bytes32(self.src_block_number));
        buf.extend_from_slice(&self.src_op_tx_id);
        buf.extend_from_slice(&u64_to_bytes32(self.nonce));
        buf.extend_from_slice(&u128_to_bytes32(self.dest_chain_id));
        buf.extend_from_slice(self.protocol_addr.as_ref());
        buf.extend_from_slice(&self.function_selector);
        buf.extend_from_slice(&self.params);
        buf.extend_from_slice(&self.reserved);
        buf
    }
}

impl TryFrom<OperationData> for photon::protocol_data::OperationData {
    type Error = Vec<u8>;
    fn try_from(value: OperationData) -> Result<Self, Self::Error> {
        Ok(photon::protocol_data::OperationData {
            protocol_id: <Vec<u8>>::from(value.protocol_id.0),
            meta: value.meta,
            src_chain_id: value.src_chain_id,
            src_block_number: value.src_block_number,
            src_op_tx_id: value.src_op_tx_id,
            nonce: value.nonce,
            dest_chain_id: value.dest_chain_id,
            protocol_addr: Pubkey::try_from(value.protocol_addr)?,
            function_selector: FunctionSelector::try_from(value.function_selector.as_slice())?,
            params: value.params,
            reserved: value.reserved,
        })
    }
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Propose {
    pub latest_block_id: String,
    #[serde(flatten)]
    pub operation_data: OperationData,
}

mod u128_serialization {
    use serde::{Deserialize, Deserializer, Serializer};

    pub(super) fn serialize<S>(chain_id: &u128, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_bytes(&chain_id.to_be_bytes())
    }

    pub(super) fn deserialize<'de, D>(deserializer: D) -> Result<u128, D::Error>
    where
        D: Deserializer<'de>,
    {
        let data = <[u8; 16]>::deserialize(deserializer)?;
        Ok(u128::from_be_bytes(data))
    }
}

mod tx_id_serialization {
    use log::error;
    use serde::{
        ser::{Error, SerializeSeq},
        Deserialize, Deserializer, Serializer,
    };

    pub(super) fn serialize<S>(tx_id: &[u8], serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut chunks = tx_id.chunks(32);
        let first = chunks.next().ok_or_else(|| {
            error!("Failed to get first tx id chunk");
            S::Error::custom("bad tx_id")
        })?;
        let second = chunks.next().ok_or_else(|| {
            error!("Failed to get second tx id chunk");
            S::Error::custom("bad tx_id")
        })?;
        let mut seq = serializer.serialize_seq(Some(2))?;
        seq.serialize_element(first)?;
        seq.serialize_element(second)?;
        seq.end()
    }

    pub(super) fn deserialize<'de, D>(deserializer: D) -> Result<Vec<u8>, D::Error>
    where
        D: Deserializer<'de>,
    {
        let chunks = <Vec<[u8; 32]>>::deserialize(deserializer)?;
        Ok(chunks.into_iter().flatten().collect())
    }
}

mod protocol_id_serialization {
    use super::{ProtocolId, ProtocolIdImpl};
    use serde::{Deserialize, Deserializer, Serializer};

    pub(super) fn serialize<S>(protocol_id: &ProtocolId, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_bytes(&protocol_id.0)
    }

    pub(super) fn deserialize<'de, D>(deserializer: D) -> Result<ProtocolId, D::Error>
    where
        D: Deserializer<'de>,
    {
        let data = ProtocolIdImpl::deserialize(deserializer)?;
        Ok(ProtocolId(data))
    }
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProposalExecuted {
    pub last_watched_block: String,
    pub op_hash: OpHash,
    pub executor: Pubkey,
}
