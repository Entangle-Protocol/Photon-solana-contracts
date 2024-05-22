pub mod config;
pub mod data;
pub mod error;
pub mod mongodb;
pub mod protocol_extension;
pub mod rabbitmq_client;
pub mod utils;

extern crate photon;

pub const SOLANA_CHAIN_ID: u128 = 11000000000000000501;
