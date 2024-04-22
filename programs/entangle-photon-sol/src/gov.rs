use anchor_lang::prelude::*;
use ethabi::{ethereum_types::U256, ParamType, Token};
use num_enum::TryFromPrimitive;

use crate::{
    gov_protocol_id, require_ok, signature::OperationData, util::EthAddress, Config, CustomError,
    ProposeEvent, ProtocolInfo, MAX_EXECUTORS, MAX_KEEPERS, MAX_PROPOSERS, SOLANA_CHAIN_ID,
};

#[derive(TryFromPrimitive)]
#[repr(u32)]
enum GovOperation {
    AddAllowedProtocol = 0x45a004b9,
    AddAllowedProtocolAddress = 0xd296a0ff,
    RemoveAllowedProtocolAddress = 0xb0a4ca98,
    AddAllowedProposerAddress = 0xce0940a5,
    RemoveAllowedProposerAddress = 0xb8e5f3f4,
    AddExecutor = 0xe0aafb68,
    RemoveExecutor = 0x04fa384a,
    AddTransmitter = 0xa8da4c51,
    RemoveTransmitter = 0x80936851,
    SetConsensusTargetRate = 0x970b6109,
}

const U32_SIZE: usize = 4;
const HANDLE_ADD_ALLOWED_PROTOCOL_SELECTOR: u32 = 0xba966e5f_u32;

pub(super) fn handle_gov_operation(
    config: &mut Config,
    target_protocol_info: &mut ProtocolInfo,
    code: Vec<u8>,
    op_data: &OperationData,
) -> Result<()> {
    let Some(code) = code.first_chunk::<U32_SIZE>() else {
        panic!("Failed to get first chunk of gov selector")
    };

    let selector_u32 = u32::from_be_bytes(*code);

    let gov_operation =
        require_ok!(GovOperation::try_from(selector_u32), CustomError::InvalidMethodSelector);

    let calldata = &op_data.params;
    match gov_operation {
        GovOperation::AddAllowedProtocol => {
            let decoded = require_ok!(
                ethabi::decode(
                    &[
                        ParamType::FixedBytes(32), // protocolId
                        ParamType::Uint(256),      // consensusTargetRate
                        ParamType::Array(Box::new(ParamType::Address)),
                    ],
                    calldata,
                ),
                CustomError::InvalidProtoMsg
            );

            let consensus_target_rate =
                decoded[1].clone().into_uint().ok_or(CustomError::InvalidGovMsg)?;
            let keepers: Vec<ethabi::Address> = decoded[2]
                .clone()
                .into_array()
                .ok_or(CustomError::InvalidGovMsg)?
                .into_iter()
                .map(|x| x.into_address().expect("always address"))
                .collect();
            target_protocol_info.is_init = true;
            target_protocol_info.consensus_target_rate = consensus_target_rate.as_u64();
            for (i, k) in keepers.into_iter().enumerate() {
                target_protocol_info.keepers[i] = k.into();
            }
            // Propose handleAddAllowedProtocol
            let nonce = config.nonce;
            config.nonce += 1;
            let mut function_selector = vec![0_u8, 32];
            function_selector.extend_from_slice(&ethabi::encode(&[Token::Uint(U256::from(
                HANDLE_ADD_ALLOWED_PROTOCOL_SELECTOR,
            ))]));
            let protocol_id =
                decoded[0].clone().into_fixed_bytes().ok_or(CustomError::InvalidGovMsg)?;
            let params = ethabi::encode(&[
                Token::FixedBytes(protocol_id),
                Token::Uint(U256::from(SOLANA_CHAIN_ID)),
            ]);
            emit!(ProposeEvent {
                protocol_id: gov_protocol_id().to_vec(),
                nonce,
                dst_chain_id: config.eob_chain_id as u128,
                protocol_address: config.eob_master_smart_contract.to_vec(),
                function_selector,
                params
            });
        }
        GovOperation::AddAllowedProtocolAddress => {
            let decoded = require_ok!(
                ethabi::decode(
                    &[
                        ParamType::FixedBytes(32), // protocolId
                        ParamType::Bytes,          // protocolAddr
                    ],
                    calldata,
                ),
                CustomError::InvalidProtoMsg
            );

            let protocol_address =
                decoded[1].clone().into_bytes().ok_or(CustomError::InvalidGovMsg)?;
            target_protocol_info.protocol_address = Pubkey::new_from_array(
                protocol_address.try_into().map_err(|_| CustomError::InvalidGovMsg)?,
            )
        }

        GovOperation::RemoveAllowedProtocolAddress => {
            target_protocol_info.protocol_address = Pubkey::default();
        }
        GovOperation::AddAllowedProposerAddress => {
            let decoded = ethabi::decode(
                &[
                    ParamType::FixedBytes(32), // protocolId
                    ParamType::Bytes,          // proposerAddr
                ],
                calldata,
            )
            .map_err(|_| CustomError::InvalidProtoMsg)?;

            let proposer = Pubkey::new_from_array(
                decoded[1]
                    .clone()
                    .into_bytes()
                    .ok_or(CustomError::InvalidGovMsg)?
                    .try_into()
                    .map_err(|_| CustomError::InvalidGovMsg)?,
            );
            let mut proposers: Vec<_> = target_protocol_info.proposers();
            if !proposers.contains(&proposer) && proposer != Pubkey::default() {
                if proposers.len() < MAX_PROPOSERS {
                    proposers.push(proposer);
                    target_protocol_info.proposers = Default::default();
                    for (i, k) in proposers.into_iter().enumerate() {
                        target_protocol_info.proposers[i] = k;
                    }
                } else {
                    return Err(CustomError::MaxExecutorsExceeded.into());
                }
            }
        }
        GovOperation::RemoveAllowedProposerAddress => {
            let decoded = ethabi::decode(
                &[
                    ParamType::FixedBytes(32), // protocolId
                    ParamType::Bytes,          // proposerAddr
                ],
                calldata,
            )
            .map_err(|_| CustomError::InvalidProtoMsg)?;

            let proposer = Pubkey::new_from_array(
                decoded[1]
                    .clone()
                    .into_bytes()
                    .ok_or(CustomError::InvalidGovMsg)?
                    .try_into()
                    .map_err(|_| CustomError::InvalidGovMsg)?,
            );
            let proposers: Vec<_> =
                target_protocol_info.proposers().into_iter().filter(|x| x != &proposer).collect();
            target_protocol_info.proposers = Default::default();
            for (i, k) in proposers.into_iter().enumerate() {
                target_protocol_info.proposers[i] = k;
            }
        }
        GovOperation::AddExecutor => {
            let decoded = ethabi::decode(
                &[
                    ParamType::FixedBytes(32), // protocolId
                    ParamType::Bytes,          // executor
                ],
                calldata,
            )
            .map_err(|_| CustomError::InvalidProtoMsg)?;

            let executor = Pubkey::new_from_array(
                decoded[1]
                    .clone()
                    .into_bytes()
                    .ok_or(CustomError::InvalidGovMsg)?
                    .try_into()
                    .map_err(|_| CustomError::InvalidGovMsg)?,
            );
            let mut executors: Vec<_> = target_protocol_info.executors();
            if !executors.contains(&executor) && executor != Pubkey::default() {
                if executors.len() < MAX_EXECUTORS {
                    executors.push(executor);
                    target_protocol_info.executors = Default::default();
                    for (i, k) in executors.into_iter().enumerate() {
                        target_protocol_info.executors[i] = k;
                    }
                } else {
                    return Err(CustomError::MaxExecutorsExceeded.into());
                }
            }
        }
        GovOperation::RemoveExecutor => {
            let decoded = ethabi::decode(
                &[
                    ParamType::FixedBytes(32), // protocolId
                    ParamType::Bytes,          // executor
                ],
                calldata,
            )
            .map_err(|_| CustomError::InvalidProtoMsg)?;

            let executor = Pubkey::new_from_array(
                decoded[1]
                    .clone()
                    .into_bytes()
                    .ok_or(CustomError::InvalidGovMsg)?
                    .try_into()
                    .map_err(|_| CustomError::InvalidGovMsg)?,
            );
            let executors: Vec<_> =
                target_protocol_info.executors().into_iter().filter(|x| x != &executor).collect();
            target_protocol_info.executors = Default::default();
            for (i, k) in executors.into_iter().enumerate() {
                target_protocol_info.executors[i] = k;
            }
        }
        GovOperation::AddTransmitter => {
            let decoded = ethabi::decode(
                &[
                    ParamType::FixedBytes(32),                      // protocolId
                    ParamType::Array(Box::new(ParamType::Address)), // keepers
                ],
                calldata,
            )
            .map_err(|_| CustomError::InvalidProtoMsg)?;

            let keepers: Vec<EthAddress> = decoded[1]
                .clone()
                .into_array()
                .ok_or(CustomError::InvalidGovMsg)?
                .into_iter()
                .filter_map(|x| x.into_address().map(|x| x.to_fixed_bytes()))
                .filter(|x| x != &EthAddress::default())
                .collect();
            require!(!keepers.is_empty(), CustomError::NoKeepersAllowed);
            let mut total_keepers = target_protocol_info.keepers();
            total_keepers.extend_from_slice(&keepers);
            total_keepers.dedup();
            if total_keepers.len() <= MAX_KEEPERS {
                target_protocol_info.keepers = Default::default();
                for (i, k) in total_keepers.into_iter().enumerate() {
                    target_protocol_info.keepers[i] = k;
                }
            } else {
                return Err(CustomError::MaxKeepersExceeded.into());
            }
        }
        GovOperation::RemoveTransmitter => {
            let decoded = ethabi::decode(
                &[
                    ParamType::FixedBytes(32),                      // protocolId
                    ParamType::Array(Box::new(ParamType::Address)), // keepers
                ],
                calldata,
            )
            .map_err(|_| CustomError::InvalidProtoMsg)?;

            let keepers: std::result::Result<Vec<EthAddress>, CustomError> = decoded[1]
                .clone()
                .into_array()
                .ok_or(CustomError::InvalidGovMsg)?
                .into_iter()
                .map(|x| {
                    x.into_address().map(|x| x.to_fixed_bytes()).ok_or(CustomError::InvalidGovMsg)
                })
                .collect();
            let to_remove = keepers?;
            let total_keepers: Vec<_> = target_protocol_info
                .keepers()
                .into_iter()
                .filter(|x| !to_remove.contains(x))
                .collect();
            target_protocol_info.keepers = Default::default();
            for (i, k) in total_keepers.into_iter().enumerate() {
                target_protocol_info.keepers[i] = k;
            }
        }
        GovOperation::SetConsensusTargetRate => {
            let decoded = ethabi::decode(
                &[
                    ParamType::FixedBytes(32), // protocolId
                    ParamType::Uint(256),      // target rate
                ],
                calldata,
            )
            .map_err(|_| CustomError::InvalidProtoMsg)?;

            let consensus_target_rate =
                decoded[1].clone().into_uint().ok_or(CustomError::InvalidGovMsg)?;
            target_protocol_info.consensus_target_rate = consensus_target_rate.as_u64();
        }
    }
    Ok(())
}
