use serde::{Deserialize, Serialize, Serializer};

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct OperationData {
    pub(super) protocol_id: Vec<u8>,
    #[serde(serialize_with = "serialize_u128_as_bytes")]
    pub(super) src_chain_id: u128,
    pub(super) src_block_number: u64,
    pub(super) src_op_tx_id: Vec<u8>,
    pub(super) nonce: u64,
    #[serde(serialize_with = "serialize_u128_as_bytes")]
    pub(super) dest_chain_id: u128,
    pub(super) protocol_addr: Vec<u8>,
    pub(super) function_selector: Vec<u8>,
    pub(super) params: Vec<u8>,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(tag = "version")]
pub(super) enum KeeperMsg {
    #[serde(rename = "1.0")]
    V1(KeeperMsgImpl),
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(tag = "type", content = "data", rename_all = "camelCase")]
pub(super) enum KeeperMsgImpl {
    #[serde(rename = "operation")]
    OperationData(OperationData),
}

fn serialize_u128_as_bytes<S>(chain_id: &u128, serializer: S) -> Result<S::Ok, S::Error>
where
    S: Serializer,
{
    serializer.serialize_bytes(&chain_id.to_be_bytes())
}
