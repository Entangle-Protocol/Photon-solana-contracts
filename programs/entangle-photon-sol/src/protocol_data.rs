//! The `protocol_data` module provides essential data structures and utility functions for handling
//! operations within the Photon cross-chain messaging layer. It supports defining and manipulating
//! operation data, such as operation identifiers and signatures, essential for the governance and
//! execution of cross-chain messages.
//!
//! ## Constants
//! - `GOV_PROTOCOL_ID`: The unique identifier used to reference the governance protocol across the Photon layer.
//!
//! ## Types
//! - `FunctionSelector`: Enumerates methods for specifying function calls within a protocol, supporting both
//!   direct binary code selections and human-readable names.
//! - `OperationData`: Central structure representing an operation's data throughout its lifecycle,
//!   including initialization, signing, and execution phases.
//!
//! ## Overview
//! This module encapsulates the core data structures that are used across various stages of operation
//! processing in the Photon messaging layer. It defines how operations are represented and interacted
//! with, ensuring that operation data is consistent and verifiable.
//!
//! ## Usage
//! `OperationData` is a crucial type within this module, used to store all relevant information about an
//! operation. It includes details such as the operation's protocol ID, source and destination chain identifiers,
//! and the specifics of the function to be called, along with necessary parameters.
//!
//! `FunctionSelector` helps in abstracting the function call mechanism, allowing operations to specify
//! either a specific code or a function name for execution, facilitating flexible execution paths.
//!
//! ## Example of OperationData
//! Here is an example of how `OperationData` might be instantiated for a new operation:
//!
//! ```rust
//! let operation_data = OperationData {
//!     protocol_id: vec![1, 2, 3, 4], // Example protocol ID
//!     meta: [0; 32], // Metadata for the operation
//!     src_chain_id: 1, // ID of the source blockchain
//!     src_block_number: 1024, // Block number on the source chain
//!     src_op_tx_id: vec![5, 6, 7, 8], // Transaction ID on the source chain
//!     nonce: 42, // Unique nonce for the operation
//!     dest_chain_id: 2, // ID of the destination blockchain
//!     protocol_addr: Pubkey::new_unique(), // Protocol address on the destination chain
//!     function_selector: FunctionSelector::ByName("transfer".to_string()), // Function to be called
//!     params: vec![9, 10], // Parameters for the function
//!     reserved: vec![0; 32], // Reserved bytes for future use
//! };
//! ```
//!
//! This module does not interact directly with blockchain layers; instead, it provides the structures
//! necessary for other components within the Photon messaging layer to execute their functionalities effectively
//!
use crate::{
    util::{u128_to_bytes32, u64_to_bytes32, Bytes32, EthAddress},
    CustomError,
};
use anchor_lang::{prelude::*, solana_program::secp256k1_recover::secp256k1_recover};
use sha3::{Digest, Keccak256};

/// GOV protocol id bytes to refer gov protocol
pub const GOV_PROTOCOL_ID: &Bytes32 =
    b"photon-gov\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00";

/// Helper to get GOV protocol id
pub const fn gov_protocol_id() -> &'static Bytes32 {
    GOV_PROTOCOL_ID
}

/// Aliasing type for Meta. The `meta` field within the [OperationData] stores protocol version and other subtleties
pub type Meta = [u8; 32];

const MSG: &str = "\x19Ethereum Signed Message:\n32";

/// The EVM-compatible signature format utilized to verify if an operation is signed by a transmitting agent.
///
/// Example:
///
#[derive(Clone, AnchorSerialize, AnchorDeserialize, Debug)]
pub struct TransmitterSignature {
    pub v: u8,
    pub r: Vec<u8>,
    pub s: Vec<u8>,
}

/// Is utilized in the [execute_operation](../photon/fn.execute_operation.html) function to make
/// it able to  match the `function_selector` by either code or name
///
/// Example:
///
#[derive(Clone, AnchorSerialize, AnchorDeserialize, Debug, Default)]
pub enum FunctionSelector {
    ByCode(Vec<u8>),
    ByName(String),
    #[default]
    Dummy,
}

impl FunctionSelector {
    pub fn to_bytes(&self) -> std::result::Result<Vec<u8>, CustomError> {
        match self {
            FunctionSelector::ByCode(code) => {
                if code.len() > 32 {
                    return Err(CustomError::SelectorTooBig);
                }
                Ok([&[0_u8, code.len() as u8][..], code].concat())
            }
            FunctionSelector::ByName(name) => {
                if name.as_bytes().len() > 32 {
                    return Err(CustomError::SelectorTooBig);
                }
                Ok([&[1_u8, name.as_bytes().len() as u8][..], name.as_bytes()].concat())
            }
            FunctionSelector::Dummy => Ok(vec![2_u8, 0]),
        }
    }
}

impl TryFrom<&[u8]> for FunctionSelector {
    type Error = Vec<u8>;
    fn try_from(value: &[u8]) -> std::result::Result<Self, Vec<u8>> {
        let mut iter = value.iter().copied();
        Ok(match iter.next().ok_or(vec![])? {
            0 => {
                let _len = iter.next().ok_or(vec![])?;
                let code: Vec<u8> = iter.collect();
                FunctionSelector::ByCode(code)
            }
            1 => {
                let _len = iter.next().ok_or(vec![])?;
                let name: Vec<u8> = iter.collect();
                FunctionSelector::ByName(String::from_utf8(name).map_err(|err| {
                    log::error!("Failed to get function_selector by name from data: {}", err);
                    vec![]
                })?)
            }
            _ => panic!("Unexpected function_selector type byte"),
        })
    }
}

/// The key data structure that is managed and utilized at every execution step: load, sign, execute.
/// It is constructed within the executor module.
#[derive(Clone, AnchorSerialize, AnchorDeserialize, Debug, Default)]
pub struct OperationData {
    pub protocol_id: Vec<u8>, // [u8; 32] is zeroed out due to bug
    pub meta: Meta,
    pub src_chain_id: u128,
    pub src_block_number: u64,
    pub src_op_tx_id: Vec<u8>,
    pub nonce: u64,
    pub dest_chain_id: u128,
    pub protocol_addr: Pubkey,
    pub function_selector: FunctionSelector,
    pub params: Vec<u8>,
    pub reserved: Vec<u8>,
}

impl OperationData {
    fn op_data_evm(&self) -> Vec<u8> {
        let mut buf = Vec::new();
        buf.extend_from_slice(&self.protocol_id);
        buf.extend_from_slice(&self.meta);
        buf.extend_from_slice(&u128_to_bytes32(self.src_chain_id));
        buf.extend_from_slice(&u64_to_bytes32(self.src_block_number));
        buf.extend_from_slice(&self.src_op_tx_id);
        buf.extend_from_slice(&u64_to_bytes32(self.nonce));
        buf.extend_from_slice(&u128_to_bytes32(self.dest_chain_id));
        buf.extend_from_slice(self.protocol_addr.as_ref());
        match &self.function_selector {
            FunctionSelector::ByCode(code) => {
                buf.extend_from_slice(&[0, code.len() as u8]);
                buf.extend_from_slice(code.as_slice())
            }
            FunctionSelector::ByName(name) => {
                buf.extend_from_slice(&[1, name.len() as u8]);
                buf.extend_from_slice(name.as_bytes())
            }
            FunctionSelector::Dummy => panic!("function_selector is not initialized"),
        }
        buf.extend_from_slice(&self.params);
        buf.extend_from_slice(&self.reserved);
        buf
    }

    fn op_hash(&self) -> Vec<u8> {
        let op_data_evm = self.op_data_evm();
        Keccak256::digest(op_data_evm).to_vec()
    }

    /// Calculates an operation hash based on its data
    /// Generates the 32-bytes hash
    ///
    /// Example:
    ///
    /// ```rust
    /// let op_hash = op_data.op_hash_with_message();
    /// require!(op_hash == op_hash_cached, CustomError::CachedOpHashMismatch);
    ///```
    pub fn op_hash_with_message(&self) -> Vec<u8> {
        hash_with_message(&self.op_hash())
    }
}

/// Computes a hash using the Keccak256 algorithm, prepending a constant message to the input data.
///
/// This function combines a predefined message `MSG` with the user-provided data `data` before hashing.
/// The constant message `MSG` is defined elsewhere in your module. The hash is computed over the concatenated
/// byte sequence of `MSG` and `data`.
///
/// # Arguments
/// * `data` - A slice of bytes that will be appended to `MSG` and hashed.
///
/// # Returns
/// Returns a `Vec<u8>` containing the Keccak256 hash of the concatenated message and data.
///
/// # Example
/// ```rust
/// // Assuming MSG is defined as "Hello, "
/// let data = b"world!";
/// let hashed_data = hash_with_message(data);
/// println!("Hashed data: {:?}", hashed_data);
/// ```
///
/// Ensure `MSG` is properly defined and visible to this function for correct operation.
pub fn hash_with_message(data: &[u8]) -> Vec<u8> {
    let mut buf = [0x00_u8; 32 + MSG.len()];
    buf[..MSG.len()].copy_from_slice(MSG.as_bytes());
    buf[MSG.len()..].copy_from_slice(data);
    Keccak256::digest(buf).to_vec()
}

/// This function computes the Ethereum address by hashing the public key using the Keccak256
/// hash function and extracting the last 20 bytes of the resultant hash. It skips the first byte
/// of the public key, which is typically used to indicate the format of the key.
/// # Arguments
/// * `public_key` - A slice of the public key bytes.
///
/// # Returns
/// Returns a 20-byte array representing the Ethereum address.
///
/// # Example
/// ```
/// let public_key = [0x04, 0xA1, 0xB2, ...]; // example public key
/// let eth_address = derive_eth_address(&public_key);
/// assert_eq!(eth_address.len(), 20);
/// ```
///
pub fn derive_eth_address(public_key: &[u8]) -> EthAddress {
    let hash = Keccak256::digest(&public_key[1..]);
    let mut bytes = [0u8; 20];
    bytes.copy_from_slice(&hash[12..]);
    bytes
}

pub(crate) fn ecrecover(op_hash: &[u8], sig: &TransmitterSignature) -> Result<EthAddress> {
    let signature = [&sig.r[..], &sig.s[..]].concat();
    let v = sig.v % 27;
    require_eq!(signature.len(), 64);
    let pk =
        secp256k1_recover(op_hash, v, &signature).map_err(|_| CustomError::InvalidSignature)?;
    Ok(derive_eth_address(&[&[0x04], &pk.0[..]].concat()))
}

/// Enumerates the different statuses an operation can have within the Photon cross-chain messaging
/// system's execution pipeline.
///
/// This enum helps track the progression of an operation from its initial state until it is fully
/// executed, providing clarity and control over each phase of the operation's lifecycle.
///
/// # Variants
///
/// * `None` - Represents the default state of an operation before any processing has begun.
/// This is the initial state when an operation is first created.
/// * `Init` - Indicates that the operation has been initialized. This state is assigned after the
/// operation is loaded and ready for further action, such as signing.
/// * `Signed` - Signifies that the operation has been signed by the required parties.
/// This state is crucial for validating that all necessary consents have been obtained before execution.
/// * `Executed` - Marks that the operation has been successfully executed. This final state
/// confirms that the operation's intended effects have been applied.
///
/// Each state transition reflects significant checkpoints in the handling and processing of cross-chain messages, ensuring that each step is clearly delineated and verified.
#[derive(AnchorSerialize, AnchorDeserialize, Clone, Default, PartialEq, Eq)]
#[repr(u8)]
pub enum OpStatus {
    #[default]
    None,
    Init,
    Signed,
    Executed,
}
