use anchor_lang::prelude::*;

#[derive(Debug, Clone, Default, AnchorSerialize, AnchorDeserialize)]
pub struct PhotonMsg {
    pub params: Vec<u8>,
}

#[derive(Debug, Clone, Default, AnchorSerialize, AnchorDeserialize)]
pub struct PhotonMsgWithSelector {
    pub op_hash: Vec<u8>,
    pub selector: Vec<u8>,
    pub params: Vec<u8>,
}
