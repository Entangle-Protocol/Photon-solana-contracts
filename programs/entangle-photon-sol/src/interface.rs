//! The `interface` module within the Photon messaging layer provides structures for packing parameters
//! to facilitate cross-program invocation (CPI) using the Borsh serialization format. These structures
//! are specifically designed to support the efficient serialization and transmission of data for
//! operations conducted across the Photon messaging layer.
//!
//! ## Overview
//! This module is essential for ensuring that the data needed for cross-chain interactions is packaged
//! correctly and is ready for transmission or processing by different parts of the Photon system.
//! It plays a crucial role in maintaining the integrity and format of the data as it moves between
//! different protocols and blockchains.
//!
//! ## Functionality
//! - **PhotonMsg**: Used to encapsulate parameters for operations that are identified by name. This
//!   struct is particularly useful for packing data for operations that require the execution of
//!   specific named functions within the target protocol.
//!
//! - **PhotonMsgWithSelector**: Extends `PhotonMsg` by including an operation hash and a selector,
//!   making it suitable for operations that are identified by a specific code. This struct ensures
//!   that the CPI can be directed correctly to execute specific coded functions or methods on the
//!   target protocol.
//!
//! ## Usage
//! The usage of `PhotonMsg` and `PhotonMsgWithSelector` is integral to the internal mechanisms that
//! prepare data for CPI calls within the Photon messaging layer. These structs are utilized to
//! serialize operation data securely, including any necessary function selectors or operation
//! identifiers, which are handled internally by the system's logic.
//!
//! ## Note
//! The module's functionality is crucial for the robust execution of cross-chain operations within the
//! Photon messaging layer, ensuring seamless interoperability and efficient data handling across
//! various blockchain protocols.

use anchor_lang::prelude::*;

#[derive(Debug, Clone, Default, AnchorSerialize, AnchorDeserialize)]
pub(super) struct PhotonMsg {
    pub params: Vec<u8>,
}

#[derive(Debug, Clone, Default, AnchorSerialize, AnchorDeserialize)]
pub(super) struct PhotonMsgWithSelector {
    pub op_hash: Vec<u8>,
    pub selector: Vec<u8>,
    pub params: Vec<u8>,
}
