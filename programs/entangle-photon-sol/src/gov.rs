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
    RATE_DECIMALS, SOLANA_CHAIN_ID,
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
    AddTransmitters = 0x6c5f5666,
    RemoveTransmitters = 0x5206da70,
    UpdateTransmitters = 0x654b46e1,
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
        GovOperation::RemoveExecutor => {
            remove_executor(calldata, &op_data.protocol_id, target_protocol_info)?
        }
        GovOperation::AddTransmitters => add_transmitters(calldata, target_protocol_info)?,
        GovOperation::RemoveTransmitters => remove_transmitters(calldata, target_protocol_info)?,
        GovOperation::UpdateTransmitters => update_transmitters(calldata, target_protocol_info)?,
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
            ParamType::FixedBytes(32),                      // protocolId
            ParamType::Uint(256),                           // consensusTargetRate
            ParamType::Array(Box::new(ParamType::Address)), // transmitters
        ]),
    )?;

    let consensus_target_rate =
        params[1].clone().into_uint().ok_or(CustomError::InvalidGovMsg)?.as_u64();

    check_consensus_target_rate(consensus_target_rate)?;

    let transmitters: Vec<ethabi::Address> = params[2]
        .clone()
        .into_array()
        .ok_or(CustomError::InvalidGovMsg)?
        .into_iter()
        .map(|x| x.into_address().expect("always address"))
        .collect();
    target_protocol_info.is_init = true;
    target_protocol_info.consensus_target_rate = consensus_target_rate;
    for (i, k) in transmitters.into_iter().enumerate() {
        target_protocol_info.transmitters[i] = k.into();
    }
    propose_handle_add_allowed_protocol(params, config)?;
    Ok(())
}

fn propose_handle_add_allowed_protocol(params: Vec<Token>, config: &mut Config) -> Result<()> {
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

fn add_allowed_protocol_address(calldata: &[u8], protocol_info: &mut ProtocolInfo) -> Result<()> {
    let params = decode_abi_params(
        calldata,
        ParamType::Tuple(vec![
            ParamType::FixedBytes(32), // protocolId
            ParamType::Bytes,          // protocolAddr
        ]),
    )?;
    let protocol_address = params[1].clone().into_bytes().ok_or(CustomError::InvalidGovMsg)?;
    protocol_info.protocol_address = Pubkey::new_from_array(
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

    if proposer == Pubkey::default() {
        return Err(CustomError::InvalidProposerAddress.into());
    }

    if proposers.contains(&proposer) {
        return Err(CustomError::ProposerIsAlreadyAllowed.into());
    }

    if proposers.len() >= MAX_PROPOSERS {
        return Err(CustomError::MaxProposersExceeded.into());
    }

    proposers.push(proposer);
    target_protocol_info.proposers = Default::default();
    for (i, k) in proposers.into_iter().enumerate() {
        target_protocol_info.proposers[i] = k;
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

    if executor == Pubkey::default() {
        return Err(CustomError::InvalidExecutorAddress.into());
    }

    let mut executors: Vec<_> = target_protocol_info.executors();
    if executors.contains(&executor) {
        return Err(CustomError::ExecutorIsAlreadyAllowed.into());
    }
    if executors.len() >= MAX_EXECUTORS {
        return Err(CustomError::MaxExecutorsExceeded.into());
    }

    executors.push(executor);
    target_protocol_info.executors = Default::default();
    for (i, k) in executors.into_iter().enumerate() {
        target_protocol_info.executors[i] = k;
    }
    Ok(())
}

fn remove_executor(
    calldata: &[u8],
    protocol_id: &[u8],
    target_protocol_info: &mut ProtocolInfo,
) -> Result<()> {
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

    if executors.is_empty() && protocol_id == GOV_PROTOCOL_ID {
        return Err(CustomError::TryingToRemoveLastGovExecutor.into());
    }

    target_protocol_info.executors = Default::default();
    for (i, k) in executors.into_iter().enumerate() {
        target_protocol_info.executors[i] = k;
    }
    Ok(())
}

fn add_transmitters(calldata: &[u8], target_protocol_info: &mut ProtocolInfo) -> Result<()> {
    let params = decode_abi_params(calldata, abi_decode_scheme(GovOperation::AddTransmitters))?;
    let transmitters = get_transmitters_to_add(&params[1])?;
    require!(!transmitters.is_empty(), CustomError::NoTransmittersAllowed);
    add_transmitters_impl(transmitters, target_protocol_info)
}

fn get_transmitters_to_add(params: &Token) -> Result<Vec<EthAddress>> {
    Ok(params
        .clone()
        .into_array()
        .ok_or(CustomError::InvalidGovMsg)?
        .into_iter()
        .filter_map(|x| x.into_address().map(|x| x.to_fixed_bytes()))
        .filter(|x| x != &EthAddress::default())
        .collect())
}

fn add_transmitters_impl(
    to_add: Vec<EthAddress>,
    target_protocol_info: &mut ProtocolInfo,
) -> Result<()> {
    let mut total_transmitters = target_protocol_info.transmitters();
    total_transmitters.extend_from_slice(&to_add);
    total_transmitters.dedup();
    if total_transmitters.len() >= MAX_TRANSMITTERS {
        return Err(CustomError::MaxTransmittersExceeded.into());
    }
    target_protocol_info.transmitters = Default::default();
    for (i, k) in total_transmitters.into_iter().enumerate() {
        target_protocol_info.transmitters[i] = k;
    }
    Ok(())
}

fn remove_transmitters(calldata: &[u8], target_protocol_info: &mut ProtocolInfo) -> Result<()> {
    let params = decode_abi_params(calldata, abi_decode_scheme(GovOperation::RemoveTransmitters))?;
    let to_remove = get_transmitters_to_remove(&params[1])?;
    remove_transmitters_impl(to_remove, target_protocol_info);
    Ok(())
}

fn get_transmitters_to_remove(params: &Token) -> std::result::Result<Vec<EthAddress>, CustomError> {
    params
        .clone()
        .into_array()
        .ok_or(CustomError::InvalidGovMsg)?
        .into_iter()
        .map(|x| x.into_address().map(|x| x.to_fixed_bytes()).ok_or(CustomError::InvalidGovMsg))
        .collect()
}

fn remove_transmitters_impl(to_remove: Vec<EthAddress>, target_protocol_info: &mut ProtocolInfo) {
    let total_transmitters: Vec<_> = target_protocol_info
        .transmitters()
        .into_iter()
        .filter(|x| !to_remove.contains(x))
        .collect();
    target_protocol_info.transmitters = Default::default();
    for (i, k) in total_transmitters.into_iter().enumerate() {
        target_protocol_info.transmitters[i] = k;
    }
}

fn update_transmitters(calldata: &[u8], target_protocol_info: &mut ProtocolInfo) -> Result<()> {
    let params = decode_abi_params(calldata, abi_decode_scheme(GovOperation::UpdateTransmitters))?;

    let to_remove = get_transmitters_to_remove(&params[2])?;
    if !to_remove.is_empty() {
        remove_transmitters_impl(to_remove, target_protocol_info);
    }

    let to_add: Vec<EthAddress> = get_transmitters_to_add(&params[1])?;
    if !to_add.is_empty() {
        add_transmitters_impl(to_add, target_protocol_info)?;
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

    let consensus_target_rate =
        params[1].clone().into_uint().ok_or(CustomError::InvalidGovMsg)?.as_u64();
    check_consensus_target_rate(consensus_target_rate)?;
    target_protocol_info.consensus_target_rate = consensus_target_rate;
    Ok(())
}

fn check_consensus_target_rate(consensus_target_rate: u64) -> Result<()> {
    if consensus_target_rate == 0 {
        return Err(CustomError::ConsensusTargetRateTooLow.into());
    }

    if consensus_target_rate > RATE_DECIMALS {
        return Err(CustomError::ConsensusTargetRateTooHigh.into());
    }
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
        GovOperation::AddTransmitters => ParamType::Tuple(vec![
            ParamType::FixedBytes(32),                      // protocolId
            ParamType::Array(Box::new(ParamType::Address)), // transmitters
        ]),
        GovOperation::RemoveTransmitters => ParamType::Tuple(vec![
            ParamType::FixedBytes(32),                      // protocolId
            ParamType::Array(Box::new(ParamType::Address)), // transmitters
        ]),
        GovOperation::UpdateTransmitters => ParamType::Tuple(vec![
            ParamType::FixedBytes(32),                      // protocolId
            ParamType::Array(Box::new(ParamType::Address)), // transmitters to add
            ParamType::Array(Box::new(ParamType::Address)), // transmitters to remove
        ]),
        GovOperation::SetConsensusTargetRate => ParamType::Tuple(vec![
            ParamType::FixedBytes(32), // protocolId
            ParamType::Uint(256),      // target rate
        ]),
    }
}
