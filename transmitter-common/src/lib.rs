pub mod config;
pub mod data;
pub mod error;
pub mod mongodb;
pub mod protocol_extension;
pub mod rabbitmq_client;
pub mod utils;

extern crate photon;

#[cfg(not(any(feature = "devnet", feature = "localnet", feature = "mainnet")))]
compile_error!("Either feature \"devnet\", \"localnet\" or \"mainnet\" must be defined");

#[cfg(any(feature = "devnet", feature = "localnet"))]
pub const SOLANA_CHAIN_ID: u128 = 100000000000000000000;

#[cfg(feature = "mainnet")]
pub const SOLANA_CHAIN_ID: u128 = 11100000000000000501;
