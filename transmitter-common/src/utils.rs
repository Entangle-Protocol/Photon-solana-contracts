//! The `utils` module provides utility functions and type aliases used throughout the Photon messaging layer.
//! This module contains functions for converting data types and computing hashes, as well as type aliases
//! that simplify the interface of other modules by providing concise and clear types.
//!
//! ## Type Aliases
//! - `Bytes32`: Represents a fixed array of 32 bytes, commonly used for handling raw byte sequences.
//! - `EthAddress`: Represents an Ethereum-compatible address, which is a shortened form consisting of 20 bytes.
//!
//! ## Functions
//! - `u64_to_bytes32`: Converts a `u64` value into a 32-byte array. This is useful for encoding numbers in formats
//!   required by certain cryptographic functions or for ensuring uniform byte length across different data elements.
//! - `u128_to_bytes32`: Converts a `u128` value into a 32-byte array, similar to `u64_to_bytes32`, but supports larger
//!   numbers suitable for identifiers or larger data sizes.
//!
//! ## Utility Macros
//! - `require_ok!`: A macro that simplifies error handling by checking if a result is `Ok`, and extracting the value,
//!   or panicking with a custom error message if it is `Err`. This macro is particularly useful in contract code where
//!   error handling must be explicit and clear.
//!
//! ## Internal Utilities
//! - `sighash`: Generates a signature hash for a given namespace and function name, commonly used for creating a
//!   consistent identifier for function calls within smart contracts or other programmatic contexts.
//!
//! ## Usage
//! The utilities provided by this module are integral to the operation of the Photon messaging layer, ensuring data
//! consistency, facilitating conversions, and aiding in error handling. By abstracting these functionalities into a
//! utility module, the Photon layer maintains clean and maintainable code throughout its implementation.
//!
use std::time::{SystemTime, UNIX_EPOCH};

pub fn get_time_ms() -> u64 {
    let start = SystemTime::now();
    let since_the_epoch = start.duration_since(UNIX_EPOCH).expect("Time went backwards");
    since_the_epoch.as_secs() * 1000 + since_the_epoch.subsec_millis() as u64
}
