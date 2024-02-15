use anchor_lang::prelude::*;

#[derive(Debug, Clone, Default, AnchorSerialize, AnchorDeserialize)]
pub struct PhotonMsg {
    pub protocol_id: Vec<u8>,
    pub src_chain_id: u128,
    pub src_block_number: u64,
    pub src_op_tx_id: Vec<u8>,
    pub params: Vec<u8>,
}

#[derive(Debug, Clone, Default, AnchorSerialize, AnchorDeserialize)]
pub struct PhotonMsgWithSelector {
    pub protocol_id: Vec<u8>,
    pub src_chain_id: u128,
    pub src_block_number: u64,
    pub src_op_tx_id: Vec<u8>,
    pub function_selector: Vec<u8>,
    pub params: Vec<u8>,
}
