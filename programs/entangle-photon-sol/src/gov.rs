//! The `gov` module manages governance-related operations within the Photon cross-chain messaging layer.
//! It handles operations that modify the configuration of allowed protocols, executors, transmitters, and
//! proposers, as well as adjusting consensus rates. These capabilities are critical for maintaining the
//! integrity and flexibility of cross-chain operations within the layer.
//!
//! ## Overview
//! This module serves as the administrative backbone of the Photon messaging layer, providing functions to securely
//! manage governance actions according to predefined rules. It supports dynamic and flexible management of
//! governance actions through Ethereum-style ABI decoding, which interprets call data for governance operations.
//!
//! ## Usage
//! Functions in this module are used during the processing of cross-chain messages that necessitate governance
//! actions, such as adding new protocols or adjusting system parameters. These functions are typically invoked
//! through Cross-Program Invocation (CPI) from other parts of the Photon layer when executing operations that
//! require governance-level permissions and checks.
//!
//! ## Key Features
//! - **Dynamic Configuration**: Allows adding and removing configurations related to cross-chain operations.
//! - **Secure Governance Actions**: Ensures that all modifications to the layer's configuration are executed
//!   securely and only by authorized entities, preventing unauthorized changes.
//! - **Consensus Management**: Facilitates adjustments to consensus parameters, ensuring the layer adapts to
//!   evolving operational needs.
//!
//! ## Public Interfaces
//! - **Propose Event Emission**: Supports the broadcasting of propose events to signal changes in governance
//!   to external systems and participants.
//!
//! ## Examples
//! Usage examples of this module include adding a new allowed protocol, which involves ABI-encoded data to
//! specify the details of the protocol being added, checked, and then integrated into the layer's configuration.
//!
//! ## Related Modules
//! - `protocol_data`: Manages data structures related to protocols such as their identifiers and operational
//!   parameters.
//! - `error`: Defines custom errors used across the Photon layer, providing clear error messages for failed
//!   governance operations.
//!
//! This documentation focuses on the public aspects of the `gov` module that are relevant to users interacting
//! with or building on top of the Photon cross-chain messaging layer. It abstracts away internal implementations
//! to provide a clear view of the module's capabilities and use cases
use anchor_lang::prelude::*;
use ethabi::{ethereum_types::U256, ParamType, Token};
use num_enum::TryFromPrimitive;

use crate::{
    error::CustomError,
    protocol_data::{FunctionSelector, OperationData, GOV_PROTOCOL_ID},
    require_ok,
    util::EthAddress,
    Config, ProposeEvent, ProtocolInfo, MAX_EXECUTORS, MAX_PROPOSERS, MAX_TRANSMITTERS,
    SOLANA_CHAIN_ID,
};

/// Enumerates government operations with their corresponding unique operation codes,
/// providing a structured way to serialize and match governance operations rather than relying on magic constants.
///
/// This approach enables clearer and more maintainable code by replacing arbitrary numerical codes
/// with descriptive enum variants, each associated with a specific governance action.
#[derive(TryFromPrimitive)]
#[repr(u32)]
pub enum GovOperation {
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
const HANDLE_ADD_ALLOWED_PROTOCOL_SELECTOR: &[u8] = &[
    0xba, 0x96, 0x6e, 0x5f, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
    0, 0, 0, 0,
];

pub(super) fn handle_gov_operation(
    config: &mut Config,
    target_protocol_info: &mut ProtocolInfo,
    code: Vec<u8>,
    op_data: &OperationData,
) -> Result<()> {
    if code.len() < U32_SIZE {
        return Err(CustomError::InvalidMethodSelector.into());
    }
    let selector_u32 = u32::from_be_bytes(code[..U32_SIZE].try_into().expect("Checked above"));

    let gov_operation =
        require_ok!(GovOperation::try_from(selector_u32), CustomError::InvalidMethodSelector);

    let calldata = &op_data.params;
    match gov_operation {
        GovOperation::AddAllowedProtocol => {
            add_allowed_protocol(calldata, target_protocol_info, config)?
        }
        GovOperation::AddAllowedProtocolAddress => {
            add_allowed_protocol_address(calldata, target_protocol_info)?
        }
        GovOperation::RemoveAllowedProtocolAddress => {
            remove_allowed_protocol_address(target_protocol_info)
        }
        GovOperation::AddAllowedProposerAddress => {
            add_allowed_proposer_address(calldata, target_protocol_info)?
        }
        GovOperation::RemoveAllowedProposerAddress => {
            remove_allowed_proposer_address(calldata, target_protocol_info)?
        }
        GovOperation::AddExecutor => add_executor(calldata, target_protocol_info)?,
        GovOperation::RemoveExecutor => remove_executor(calldata, target_protocol_info)?,
        GovOperation::AddTransmitter => add_transmitter(calldata, target_protocol_info)?,
        GovOperation::RemoveTransmitter => remove_transmitter(calldata, target_protocol_info)?,
        GovOperation::SetConsensusTargetRate => {
            set_consensus_target_rate(calldata, target_protocol_info)?
        }
    }
    Ok(())
}

pub(super) fn add_allowed_protocol(
    calldata: &[u8],
    target_protocol_info: &mut ProtocolInfo,
    config: &mut Config,
) -> Result<()> {
    let params = decode_abi_params(
        calldata,
        ParamType::Tuple(vec![
            ParamType::FixedBytes(32), // protocolId
            ParamType::Uint(256),      // consensusTargetRate
            ParamType::Array(Box::new(ParamType::Address)),
        ]),
    )?;

    let consensus_target_rate = params[1].clone().into_uint().ok_or(CustomError::InvalidGovMsg)?;
    let transmitters: Vec<ethabi::Address> = params[2]
        .clone()
        .into_array()
        .ok_or(CustomError::InvalidGovMsg)?
        .into_iter()
        .map(|x| x.into_address().expect("always address"))
        .collect();
    target_protocol_info.is_init = true;
    target_protocol_info.consensus_target_rate = consensus_target_rate.as_u64();
    for (i, k) in transmitters.into_iter().enumerate() {
        target_protocol_info.transmitters[i] = k.into();
    }
    // Propose handleAddAllowedProtocol
    let nonce = config.nonce;
    config.nonce += 1;
    let mut function_selector = vec![0_u8, 32];

    function_selector.extend_from_slice(&ethabi::encode(&[Token::FixedBytes(
        HANDLE_ADD_ALLOWED_PROTOCOL_SELECTOR.to_vec(),
    )]));
    let protocol_id = params[0].clone().into_fixed_bytes().ok_or(CustomError::InvalidGovMsg)?;
    let params = ethabi::encode(&[Token::Tuple(vec![
        Token::FixedBytes(protocol_id),
        Token::Uint(U256::from(SOLANA_CHAIN_ID)),
    ])]);
    emit!(ProposeEvent {
        protocol_id: GOV_PROTOCOL_ID.to_vec(),
        nonce,
        dst_chain_id: config.eob_chain_id as u128,
        protocol_address: config.eob_master_smart_contract.to_vec(),
        function_selector,
        params
    });
    Ok(())
}

fn add_allowed_protocol_address(
    calldata: &[u8],
    target_protocol_info: &mut ProtocolInfo,
) -> Result<()> {
    let params = decode_abi_params(
        calldata,
        ParamType::Tuple(vec![
            ParamType::FixedBytes(32), // protocolId
            ParamType::Bytes,          // protocolAddr
        ]),
    )?;
    let protocol_address = params[1].clone().into_bytes().ok_or(CustomError::InvalidGovMsg)?;
    target_protocol_info.protocol_address = Pubkey::new_from_array(
        protocol_address.try_into().map_err(|_| CustomError::InvalidGovMsg)?,
    );
    Ok(())
}

fn remove_allowed_protocol_address(target_protocol_info: &mut ProtocolInfo) {
    target_protocol_info.protocol_address = Pubkey::default();
}

fn add_allowed_proposer_address(
    calldata: &[u8],
    target_protocol_info: &mut ProtocolInfo,
) -> Result<()> {
    let params = decode_abi_params(
        calldata,
        ParamType::Tuple(vec![
            ParamType::FixedBytes(32), // protocolId
            ParamType::Bytes,          // proposerAddr
        ]),
    )?;

    let proposer = Pubkey::new_from_array(
        params[1]
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
    Ok(())
}

fn remove_allowed_proposer_address(
    calldata: &[u8],
    target_protocol_info: &mut ProtocolInfo,
) -> Result<()> {
    let params = decode_abi_params(
        calldata,
        ParamType::Tuple(vec![
            ParamType::FixedBytes(32), // protocolId
            ParamType::Bytes,          // proposerAddr
        ]),
    )?;

    let proposer = Pubkey::new_from_array(
        params[1]
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
    Ok(())
}

fn add_executor(calldata: &[u8], target_protocol_info: &mut ProtocolInfo) -> Result<()> {
    let params = decode_abi_params(
        calldata,
        ParamType::Tuple(vec![
            ParamType::FixedBytes(32), // protocolId
            ParamType::Bytes,          // executor
        ]),
    )?;

    let executor = Pubkey::new_from_array(
        params[1]
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
    Ok(())
}

fn remove_executor(calldata: &[u8], target_protocol_info: &mut ProtocolInfo) -> Result<()> {
    let params = decode_abi_params(
        calldata,
        ParamType::Tuple(vec![
            ParamType::FixedBytes(32), // protocolId
            ParamType::Bytes,          // executor
        ]),
    )?;

    let executor = Pubkey::new_from_array(
        params[1]
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
    Ok(())
}

fn add_transmitter(calldata: &[u8], target_protocol_info: &mut ProtocolInfo) -> Result<()> {
    let params = decode_abi_params(
        calldata,
        ParamType::Tuple(vec![
            ParamType::FixedBytes(32),                      // protocolId
            ParamType::Array(Box::new(ParamType::Address)), // transmitters
        ]),
    )?;

    let transmitters: Vec<EthAddress> = params[1]
        .clone()
        .into_array()
        .ok_or(CustomError::InvalidGovMsg)?
        .into_iter()
        .filter_map(|x| x.into_address().map(|x| x.to_fixed_bytes()))
        .filter(|x| x != &EthAddress::default())
        .collect();
    require!(!transmitters.is_empty(), CustomError::NoTransmittersAllowed);
    let mut total_transmitters = target_protocol_info.transmitters();
    total_transmitters.extend_from_slice(&transmitters);
    total_transmitters.dedup();
    if total_transmitters.len() <= MAX_TRANSMITTERS {
        target_protocol_info.transmitters = Default::default();
        for (i, k) in total_transmitters.into_iter().enumerate() {
            target_protocol_info.transmitters[i] = k;
        }
    } else {
        return Err(CustomError::MaxTransmittersExceeded.into());
    }
    Ok(())
}

fn remove_transmitter(calldata: &[u8], target_protocol_info: &mut ProtocolInfo) -> Result<()> {
    let params = decode_abi_params(
        calldata,
        ParamType::Tuple(vec![
            ParamType::FixedBytes(32),                      // protocolId
            ParamType::Array(Box::new(ParamType::Address)), // transmitters
        ]),
    )?;
    let transmitters: std::result::Result<Vec<EthAddress>, CustomError> = params[1]
        .clone()
        .into_array()
        .ok_or(CustomError::InvalidGovMsg)?
        .into_iter()
        .map(|x| x.into_address().map(|x| x.to_fixed_bytes()).ok_or(CustomError::InvalidGovMsg))
        .collect();
    let to_remove = transmitters?;
    let total_transmitters: Vec<_> = target_protocol_info
        .transmitters()
        .into_iter()
        .filter(|x| !to_remove.contains(x))
        .collect();
    target_protocol_info.transmitters = Default::default();
    for (i, k) in total_transmitters.into_iter().enumerate() {
        target_protocol_info.transmitters[i] = k;
    }
    Ok(())
}

fn set_consensus_target_rate(
    calldata: &[u8],
    target_protocol_info: &mut ProtocolInfo,
) -> Result<()> {
    let params = decode_abi_params(
        calldata,
        ParamType::Tuple(vec![
            ParamType::FixedBytes(32), // protocolId
            ParamType::Uint(256),      // target rate
        ]),
    )?;
    let consensus_target_rate = params[1].clone().into_uint().ok_or(CustomError::InvalidGovMsg)?;
    target_protocol_info.consensus_target_rate = consensus_target_rate.as_u64();
    Ok(())
}

pub(super) fn target_protocol(function_selector: &FunctionSelector, params: &[u8]) -> Vec<u8> {
    let FunctionSelector::ByCode(code) = function_selector else {
        panic!("Unexpected function selector");
    };

    let Ok(target_protocol) = target_protocol_by_code(code, params) else {
        panic!("Failed to get target_protocol");
    };
    target_protocol
}

pub(super) fn target_protocol_by_code(
    code: &[u8],
    params: &[u8],
) -> std::result::Result<Vec<u8>, String> {
    if code.len() < U32_SIZE {
        return Err("Selector to short".to_string());
    }
    let selector_u32 =
        u32::from_be_bytes(code[..U32_SIZE].try_into().map_err(|_| "Checked above".to_string())?);
    let gov_operation = GovOperation::try_from(selector_u32)
        .map_err(|_| "Failed to get gov_operation from selector".to_string())?;
    let params = decode_abi_params(params, abi_decode_scheme(gov_operation))
        .map_err(|_| "Failed to decode abi params".to_string())?;
    params
        .first()
        .ok_or_else(|| "Failed to get first decoded abi param".to_string())?
        .clone()
        .into_fixed_bytes()
        .ok_or_else(|| "Failed to convert first decoded abi param as fixed_bytes".to_string())
}

/// Commonly used in the `gov-extension` to extract accounts from encoded `calldata` based on the `param_type`.
pub fn decode_abi_params(calldata: &[u8], param_type: ParamType) -> Result<Vec<Token>> {
    let decoded =
        ethabi::decode(&[param_type], calldata).map_err(|_| CustomError::InvalidProtoMsg)?;

    Ok(decoded
        .first()
        .ok_or(CustomError::InvalidProtoMsg)?
        .clone()
        .into_tuple()
        .ok_or(CustomError::InvalidProtoMsg)?)
}

/// A shortcut to retrieve the decoding schema based on the provided GovOperation.
pub fn abi_decode_scheme(gov_operation: GovOperation) -> ParamType {
    match gov_operation {
        GovOperation::AddAllowedProtocol => {
            ParamType::Tuple(vec![
                ParamType::FixedBytes(32), // protocolId
                ParamType::Uint(256),      // consensusTargetRate
                ParamType::Array(Box::new(ParamType::Address)),
            ])
        }
        GovOperation::AddAllowedProtocolAddress => ParamType::Tuple(vec![
            ParamType::FixedBytes(32), // protocolId
            ParamType::Bytes,          // protocolAddr
        ]),
        GovOperation::RemoveAllowedProtocolAddress => ParamType::Tuple(vec![
            ParamType::FixedBytes(32), // protocolId
            ParamType::Bytes,          // protocolAddr
        ]),
        GovOperation::AddAllowedProposerAddress => ParamType::Tuple(vec![
            ParamType::FixedBytes(32), // protocolId
            ParamType::Bytes,          // proposerAddr
        ]),
        GovOperation::RemoveAllowedProposerAddress => ParamType::Tuple(vec![
            ParamType::FixedBytes(32), // protocolId
            ParamType::Bytes,          // proposerAddr
        ]),
        GovOperation::AddExecutor => ParamType::Tuple(vec![
            ParamType::FixedBytes(32), // protocolId
            ParamType::Bytes,          // executor
        ]),
        GovOperation::RemoveExecutor => ParamType::Tuple(vec![
            ParamType::FixedBytes(32), // protocolId
            ParamType::Bytes,          // executor
        ]),
        GovOperation::AddTransmitter => ParamType::Tuple(vec![
            ParamType::FixedBytes(32),                      // protocolId
            ParamType::Array(Box::new(ParamType::Address)), // transmitters
        ]),
        GovOperation::RemoveTransmitter => ParamType::Tuple(vec![
            ParamType::FixedBytes(32),                      // protocolId
            ParamType::Array(Box::new(ParamType::Address)), // transmitters
        ]),
        GovOperation::SetConsensusTargetRate => ParamType::Tuple(vec![
            ParamType::FixedBytes(32), // protocolId
            ParamType::Uint(256),      // target rate
        ]),
    }
}
