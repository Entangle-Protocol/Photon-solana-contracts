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
//! ## Examples
//! Here is a conceptual usage of the `u64_to_bytes32` function:
//!
//! ```rust
//! let number: u64 = 42;
//! let bytes = utils::u64_to_bytes32(number);
//! assert_eq!(bytes[24..], [0, 0, 0, 0, 0, 0, 0, 42]);  // most bytes are zero-padded
//! ```
//!
//! This function is critical for situations where uniform byte arrays are necessary, such as cryptographic operations
//! or when interfacing with systems that require fixed-length byte arrays.

/// Aliasing type for 32 raw bytes sequence
pub type Bytes32 = [u8; 32];
/// Aliasing type for evm compatible shortened address
pub type EthAddress = [u8; 20];

/// Service method to convert u64 to the 32 raw bytes sequence
pub fn u64_to_bytes32(x: u64) -> [u8; 32] {
    let mut buf = [0; 32];
    buf[32 - 8..].copy_from_slice(&x.to_be_bytes());
    buf
}

/// Service method to convert u128 to the 32 raw bytes sequence
pub fn u128_to_bytes32(x: u128) -> [u8; 32] {
    let mut buf = [0; 32];
    buf[32 - 16..].copy_from_slice(&x.to_be_bytes());
    buf
}

pub(super) fn sighash(namespace: &str, name: &str) -> [u8; 8] {
    let preimage = format!("{}:{}", namespace, name);
    let mut sighash = [0u8; 8];
    sighash.copy_from_slice(
        &anchor_lang::solana_program::hash::hash(preimage.as_bytes()).to_bytes()[..8],
    );
    sighash
}

/// Checks whether a result is `Ok` and returns the value if it is.
/// If the result is an `Err`, it triggers an error check using `require_eq!`
/// and then panics with an `unreachable!` call.
///
/// # Arguments
///
/// * `$res` - The result to check. This can be of any type that implements the `Result` trait.
/// * `$err` - The error message to display if `$res` is an `Err`.
///
/// # Examples
///
/// ```
/// let result = Ok(42);
/// let value = require_ok!(result, "Unexpected error");
/// assert_eq!(value, 42);
///
/// let result: Result<i32, &str> = Err("Something went wrong");
/// // This will cause the program to panic with the message: "Error: "Something went wrong"".
/// require_ok!(result, "Unexpected error");
/// ```
#[macro_export]
macro_rules! require_ok {
    ($res:expr, $err:expr) => {
        match $res {
            Ok(x) => x,
            Err(e) => {
                require_eq!("", format!("Error: {:?}", e), $err);
                unreachable!();
            }
        }
    };
}
