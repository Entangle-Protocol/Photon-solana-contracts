#![feature(slice_first_last_chunk)]
#![feature(slice_as_chunks)]
#![feature(slice_flatten)]

pub mod config;
pub mod data;
pub mod mongodb;
pub mod protocol_extension;
pub mod rabbitmq_client;
pub mod utils;

extern crate photon;

pub const SOLANA_CHAIN_ID: u128 = 100000000000000000000;
