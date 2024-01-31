use anchor_lang::prelude::*;
use ethabi::ParamType;

use crate::{
    signature::OperationData, util::EthAddress, CustomError, ExecuteGovOperation, MAX_EXECUTORS,
    MAX_KEEPERS, MAX_PROPOSERS,
};

pub fn handle_gov_operation(
    ctx: Context<ExecuteGovOperation>,
    op_data: OperationData,
    target_protocol: Vec<u8>,
) -> Result<()> {
    let selector = u32::from_be_bytes(op_data.function_selector);
    let calldata = &op_data.params;
    match selector {
        // addAllowedProtocol(bytes)
        0x45a004b9 => {
            let decoded = ethabi::decode(
                &[
                    ParamType::FixedBytes(32), // protocolId
                    ParamType::Uint(256),      // consensusTargetRate
                    ParamType::Uint(256),      // protocolFee
                    ParamType::Array(Box::new(ParamType::Address)),
                ],
                &calldata,
            )
            .map_err(|_| CustomError::InvalidProtoMsg)?;
            let protocol_id = decoded[0]
                .clone()
                .into_fixed_bytes()
                .ok_or(CustomError::InvalidGovMsg)?;
            require!(
                protocol_id == target_protocol,
                CustomError::TargetProtocolMismatch
            );
            let consensus_target_rate = decoded[1]
                .clone()
                .into_uint()
                .ok_or(CustomError::InvalidGovMsg)?;
            let protocol_fee = decoded[2]
                .clone()
                .into_uint()
                .ok_or(CustomError::InvalidGovMsg)?;
            let keepers: Vec<ethabi::Address> = decoded[3]
                .clone()
                .into_array()
                .ok_or(CustomError::InvalidGovMsg)?
                .into_iter()
                .map(|x| x.into_address().unwrap())
                .collect();
            ctx.accounts.protocol_info.is_init = true;
            ctx.accounts.protocol_info.consensus_target_rate = consensus_target_rate.as_u64();
            ctx.accounts.protocol_info.protocol_fee = protocol_fee.as_u64();
            for (i, k) in keepers.into_iter().enumerate() {
                ctx.accounts.protocol_info.keepers[i] = k.into();
            }
        }
        // addAllowedProtocolAddress(bytes)
        0xd296a0ff => {
            let decoded = ethabi::decode(
                &[
                    ParamType::FixedBytes(32), // protocolId
                    ParamType::Bytes,          // protocolAddr
                ],
                &calldata,
            )
            .map_err(|_| CustomError::InvalidProtoMsg)?;
            let protocol_id = decoded[0]
                .clone()
                .into_fixed_bytes()
                .ok_or(CustomError::InvalidGovMsg)?;
            require!(
                protocol_id == target_protocol,
                CustomError::TargetProtocolMismatch
            );
            let protocol_address = decoded[1]
                .clone()
                .into_bytes()
                .ok_or(CustomError::InvalidGovMsg)?;
            ctx.accounts.protocol_info.protocol_address = Pubkey::new_from_array(
                protocol_address
                    .try_into()
                    .map_err(|_| CustomError::InvalidGovMsg)?,
            )
        }
        // removeAllowedProtocolAddress(bytes)
        0xb0a4ca98 => {
            let decoded = ethabi::decode(
                &[
                    ParamType::FixedBytes(32), // protocolId
                    ParamType::Bytes,          // protocolAddr
                ],
                &calldata,
            )
            .map_err(|_| CustomError::InvalidProtoMsg)?;
            let protocol_id = decoded[0]
                .clone()
                .into_fixed_bytes()
                .ok_or(CustomError::InvalidGovMsg)?;
            require!(
                protocol_id == target_protocol,
                CustomError::TargetProtocolMismatch
            );
            ctx.accounts.protocol_info.protocol_address = Pubkey::default();
        }
        // addAllowedProposerAddress(bytes)
        0xce0940a5 => {
            let decoded = ethabi::decode(
                &[
                    ParamType::FixedBytes(32), // protocolId
                    ParamType::Bytes,          // proposerAddr
                ],
                &calldata,
            )
            .map_err(|_| CustomError::InvalidProtoMsg)?;
            let protocol_id = decoded[0]
                .clone()
                .into_fixed_bytes()
                .ok_or(CustomError::InvalidGovMsg)?;
            require!(
                protocol_id == target_protocol,
                CustomError::TargetProtocolMismatch
            );
            let proposer = Pubkey::new_from_array(
                decoded[1]
                    .clone()
                    .into_bytes()
                    .ok_or(CustomError::InvalidGovMsg)?
                    .try_into()
                    .map_err(|_| CustomError::InvalidGovMsg)?,
            );
            let mut proposers: Vec<_> = ctx.accounts.protocol_info.proposers();
            if !proposers.contains(&proposer) && proposer != Pubkey::default() {
                if proposers.len() < MAX_PROPOSERS {
                    proposers.push(proposer);
                    ctx.accounts.protocol_info.proposers = Default::default();
                    for (i, k) in proposers.into_iter().enumerate() {
                        ctx.accounts.protocol_info.proposers[i] = k;
                    }
                } else {
                    return Err(CustomError::MaxExecutorsExceeded.into());
                }
            }
        }
        // removeAllowedProposerAddress(bytes)
        0xb8e5f3f4 => {
            let decoded = ethabi::decode(
                &[
                    ParamType::FixedBytes(32), // protocolId
                    ParamType::Bytes,          // proposerAddr
                ],
                &calldata,
            )
            .map_err(|_| CustomError::InvalidProtoMsg)?;
            let protocol_id = decoded[0]
                .clone()
                .into_fixed_bytes()
                .ok_or(CustomError::InvalidGovMsg)?;
            require!(
                protocol_id == target_protocol,
                CustomError::TargetProtocolMismatch
            );
            let proposer = Pubkey::new_from_array(
                decoded[1]
                    .clone()
                    .into_bytes()
                    .ok_or(CustomError::InvalidGovMsg)?
                    .try_into()
                    .map_err(|_| CustomError::InvalidGovMsg)?,
            );
            let proposers: Vec<_> = ctx
                .accounts
                .protocol_info
                .proposers()
                .into_iter()
                .filter(|x| x != &proposer)
                .collect();
            ctx.accounts.protocol_info.proposers = Default::default();
            for (i, k) in proposers.into_iter().enumerate() {
                ctx.accounts.protocol_info.proposers[i] = k;
            }
        }
        // addExecutor(bytes)
        0xe0aafb68 => {
            let decoded = ethabi::decode(
                &[
                    ParamType::FixedBytes(32), // protocolId
                    ParamType::Bytes,          // executor
                ],
                &calldata,
            )
            .map_err(|_| CustomError::InvalidProtoMsg)?;
            let protocol_id = decoded[0]
                .clone()
                .into_fixed_bytes()
                .ok_or(CustomError::InvalidGovMsg)?;
            require!(
                protocol_id == target_protocol,
                CustomError::TargetProtocolMismatch
            );
            let executor = Pubkey::new_from_array(
                decoded[1]
                    .clone()
                    .into_bytes()
                    .ok_or(CustomError::InvalidGovMsg)?
                    .try_into()
                    .map_err(|_| CustomError::InvalidGovMsg)?,
            );
            let mut executors: Vec<_> = ctx.accounts.protocol_info.executors();
            if !executors.contains(&executor) && executor != Pubkey::default() {
                if executors.len() < MAX_EXECUTORS {
                    executors.push(executor);
                    ctx.accounts.protocol_info.executors = Default::default();
                    for (i, k) in executors.into_iter().enumerate() {
                        ctx.accounts.protocol_info.executors[i] = k;
                    }
                } else {
                    return Err(CustomError::MaxExecutorsExceeded.into());
                }
            }
        }
        // removeExecutor(bytes)
        0x04fa384a => {
            let decoded = ethabi::decode(
                &[
                    ParamType::FixedBytes(32), // protocolId
                    ParamType::Bytes,          // executor
                ],
                &calldata,
            )
            .map_err(|_| CustomError::InvalidProtoMsg)?;
            let protocol_id = decoded[0]
                .clone()
                .into_fixed_bytes()
                .ok_or(CustomError::InvalidGovMsg)?;
            require!(
                protocol_id == target_protocol,
                CustomError::TargetProtocolMismatch
            );
            let executor = Pubkey::new_from_array(
                decoded[1]
                    .clone()
                    .into_bytes()
                    .ok_or(CustomError::InvalidGovMsg)?
                    .try_into()
                    .map_err(|_| CustomError::InvalidGovMsg)?,
            );
            let executors: Vec<_> = ctx
                .accounts
                .protocol_info
                .executors()
                .into_iter()
                .filter(|x| x != &executor)
                .collect();
            ctx.accounts.protocol_info.executors = Default::default();
            for (i, k) in executors.into_iter().enumerate() {
                ctx.accounts.protocol_info.executors[i] = k;
            }
        }
        // addKeeper(bytes)
        0xa8da4c51 => {
            let decoded = ethabi::decode(
                &[
                    ParamType::FixedBytes(32),                      // protocolId
                    ParamType::Array(Box::new(ParamType::Address)), // keepers
                ],
                &calldata,
            )
            .map_err(|_| CustomError::InvalidProtoMsg)?;
            let protocol_id = decoded[0]
                .clone()
                .into_fixed_bytes()
                .ok_or(CustomError::InvalidGovMsg)?;
            require!(
                protocol_id == target_protocol,
                CustomError::TargetProtocolMismatch
            );
            let keepers: Vec<EthAddress> = decoded[1]
                .clone()
                .into_array()
                .ok_or(CustomError::InvalidGovMsg)?
                .into_iter()
                .filter_map(|x| x.into_address().map(|x| x.to_fixed_bytes()))
                .filter(|x| x != &EthAddress::default())
                .collect();
            require!(!keepers.is_empty(), CustomError::NoKeepersAllowed);
            let mut total_keepers = ctx.accounts.protocol_info.keepers();
            total_keepers.extend_from_slice(&keepers);
            total_keepers.dedup();
            if total_keepers.len() <= MAX_KEEPERS {
                ctx.accounts.protocol_info.keepers = Default::default();
                for (i, k) in total_keepers.into_iter().enumerate() {
                    ctx.accounts.protocol_info.keepers[i] = k;
                }
            } else {
                return Err(CustomError::MaxKeepersExceeded.into());
            }
        }
        // removeKeeper(bytes)
        0x80936851 => {
            let decoded = ethabi::decode(
                &[
                    ParamType::FixedBytes(32),                      // protocolId
                    ParamType::Array(Box::new(ParamType::Address)), // keepers
                ],
                &calldata,
            )
            .map_err(|_| CustomError::InvalidProtoMsg)?;
            let protocol_id = decoded[0]
                .clone()
                .into_fixed_bytes()
                .ok_or(CustomError::InvalidGovMsg)?;
            require!(
                protocol_id == target_protocol,
                CustomError::TargetProtocolMismatch
            );
            let keepers: std::result::Result<Vec<EthAddress>, CustomError> = decoded[1]
                .clone()
                .into_array()
                .ok_or(CustomError::InvalidGovMsg)?
                .into_iter()
                .map(|x| {
                    x.into_address()
                        .map(|x| x.to_fixed_bytes())
                        .ok_or(CustomError::InvalidGovMsg)
                })
                .collect();
            let to_remove = keepers?;
            let total_keepers: Vec<_> = ctx
                .accounts
                .protocol_info
                .keepers()
                .into_iter()
                .filter(|x| !to_remove.contains(x))
                .collect();
            ctx.accounts.protocol_info.keepers = Default::default();
            for (i, k) in total_keepers.into_iter().enumerate() {
                ctx.accounts.protocol_info.keepers[i] = k;
            }
        }
        // setConsensusTargetRate(bytes)
        0x970b6109 => {
            let decoded = ethabi::decode(
                &[
                    ParamType::FixedBytes(32), // protocolId
                    ParamType::Uint(256),      // target rate
                ],
                &calldata,
            )
            .map_err(|_| CustomError::InvalidProtoMsg)?;
            let protocol_id = decoded[0]
                .clone()
                .into_fixed_bytes()
                .ok_or(CustomError::InvalidGovMsg)?;
            require!(
                protocol_id == target_protocol,
                CustomError::TargetProtocolMismatch
            );
            let consensus_target_rate = decoded[1]
                .clone()
                .into_uint()
                .ok_or(CustomError::InvalidGovMsg)?;
            ctx.accounts.protocol_info.consensus_target_rate = consensus_target_rate.as_u64();
        }
        // setProtocolFee(bytes)
        0xafe50cc2 => {
            let decoded = ethabi::decode(
                &[
                    ParamType::FixedBytes(32), // protocolId
                    ParamType::Uint(256),      // fee
                ],
                &calldata,
            )
            .map_err(|_| CustomError::InvalidProtoMsg)?;
            let protocol_id = decoded[0]
                .clone()
                .into_fixed_bytes()
                .ok_or(CustomError::InvalidGovMsg)?;
            require!(
                protocol_id == target_protocol,
                CustomError::TargetProtocolMismatch
            );
            let fee = decoded[1]
                .clone()
                .into_uint()
                .ok_or(CustomError::InvalidGovMsg)?;
            ctx.accounts.protocol_info.protocol_fee = fee.as_u64();
        }
        _ => {
            return Err(CustomError::InvalidGovMethod.into());
        }
    }
    Ok(())
}
