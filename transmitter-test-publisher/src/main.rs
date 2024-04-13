#![feature(slice_first_last_chunk)]

mod cli;
mod rabbitmq_publisher;
mod util;

extern crate onefunc;
extern crate photon;

use config::{Config, File};
use ethabi::{Token, Uint};
use libsecp256k1::sign;
use log::error;
use rand::{random, RngCore};
use serde::Deserialize;
use std::{env, time::Duration};
use thiserror::Error;

use transmitter_common::data::{Meta, OperationData, ProtocolId};

use cli::Operation;
use rabbitmq_publisher::{RabbitmqConfig, RabbitmqPublisher};
use util::predefined_signers;

#[derive(Debug, Error)]
pub(crate) enum PublisherError {
    #[error("Rabbitmq client error")]
    Rabbitmq(#[from] amqprs::error::Error),
}

#[derive(Deserialize)]
struct PublisherConfig {
    rabbitmq: RabbitmqConfig,
}

#[tokio::main]
async fn main() {
    env_logger::init();
    cli::Cli::execute(env::args()).await;
}

pub(crate) async fn publish(config: &str, operation: &Operation, times: u64) {
    let config = Config::builder()
        .add_source(File::with_name(config))
        .add_source(config::Environment::with_prefix("ENTANGLE").separator("_"))
        .build()
        .expect("Expected publisher config be build from the given sources");

    let config: PublisherConfig =
        config.try_deserialize().expect("Expected publisher_config be deserialized");

    let publisher = RabbitmqPublisher::new(config.rabbitmq);

    let protocol_id = ProtocolId(
        *onefunc::onefunc::PROTOCOL_ID.first_chunk().expect("Expected PROTOCOL_ID be set"),
    );
    let dst_chain_id = photon::photon::SOLANA_CHAIN_ID;
    let protocol_address: Vec<u8> = onefunc::ID.to_bytes().to_vec();
    let meta: Meta =
        *hex::decode("0100000000000000000000000000000000000000000000000000000000000000")
            .expect("Expected meta hex to be decoded")
            .first_chunk()
            .expect("Expected meta chunck to be");
    for nonce in 0..times {
        let mut tx_id = [0u8; 64];
        rand::thread_rng().fill_bytes(&mut tx_id);

        let op_data = match operation {
            Operation::InitOwnedCounter => {
                let function_selector: Vec<u8> = b"\x01\x12init_owned_counter".to_vec();
                OperationData {
                    protocol_id,
                    meta,
                    src_block_number: 1,
                    src_chain_id: dst_chain_id,
                    dest_chain_id: dst_chain_id,
                    nonce,
                    src_op_tx_id: tx_id.to_vec(),
                    protocol_addr: protocol_address.clone(),
                    function_selector,
                    params: <Vec<u8>>::default(),
                    reserved: <Vec<u8>>::default(),
                }
            }
            Operation::Increment(component) => {
                let function_selector: Vec<u8> = b"\x01\x17increment_owned_counter".to_vec();
                let params: Vec<u8> = ethabi::encode(&[Token::Uint(Uint::from(*component))]);
                OperationData {
                    protocol_id,
                    meta,
                    src_block_number: 1,
                    src_chain_id: dst_chain_id,
                    dest_chain_id: dst_chain_id,
                    nonce,
                    src_op_tx_id: tx_id.to_vec(),
                    protocol_addr: protocol_address.clone(),
                    function_selector,
                    params,
                    reserved: <Vec<u8>>::default(),
                }
            }
        };
        let predefined_signers = predefined_signers(3);
        let keepers = predefined_signers
            .iter()
            .map(|wallet| {
                let op_hash = op_data.op_hash_with_message();
                let message = libsecp256k1::Message::parse_slice(&op_hash)
                    .expect("Expected secp256k1 message be built from op_hash");
                let (sig, recover_id) = sign(&message, &wallet.0);
                let serialized_sig = sig.serialize();
                transmitter_common::data::KeeperSignature {
                    r: serialized_sig[..32].to_vec(),
                    s: serialized_sig[32..].to_vec(),
                    v: recover_id.serialize(),
                }
            })
            .collect();

        let eob_block_number: u64 = random();
        publisher
            .publish_operation_data(op_data.clone(), keepers, eob_block_number)
            .await
            .expect("Expected signed op_data be published");
    }

    tokio::time::sleep(Duration::from_millis(1)).await;
}

#[cfg(test)]
mod test {
    use super::OperationData;
    use crate::util::{predefined_signers, KeeperSignature};
    use libsecp256k1::{sign, PublicKey};
    use rand::RngCore;
    use solana_program::secp256k1_recover::{secp256k1_recover, Secp256k1Pubkey};
    use transmitter_common::data::ProtocolId;

    #[test]
    fn test_signature() {
        env_logger::init();
        let keepers = predefined_signers(3);

        let public_key = PublicKey::from_secret_key(&keepers[0].0);

        const TEST_OP_HASH: &str =
            "c9382d122da415500ff93d62be8ea03b68d564beeaba159004cd2c62f48c5e17";
        let op_hash = hex::decode(TEST_OP_HASH).expect("Expected op_hash be decoded");
        let message =
            libsecp256k1::Message::parse_slice(&op_hash).expect("Expected secp256k1 be built");
        let (sig, recover_id) = sign(&message, &keepers[0].0);
        let serialized_sig = sig.serialize();
        let keeper_signature = KeeperSignature {
            r: serialized_sig[..32].to_vec(),
            s: serialized_sig[32..].to_vec(),
            v: recover_id.serialize(),
        };
        let ecrecover_recovered_pubkey = ecrecover(&op_hash, &keeper_signature);
        assert_eq!(&ecrecover_recovered_pubkey.0, &public_key.serialize()[1..]);
    }

    pub(crate) fn ecrecover(op_hash: &[u8], sig: &KeeperSignature) -> Secp256k1Pubkey {
        let signature = [&sig.r[..], &sig.s[..]].concat();
        let v = sig.v % 27;
        assert_eq!(signature.len(), 64);
        secp256k1_recover(op_hash, v, &signature)
            .expect("Expected secp256k1 hash be recovered from signature")
    }

    #[test]
    fn test_op_hash_matches() {
        env_logger::init();
        let meta = [1; 32];
        let protocol_id = ProtocolId(
            *onefunc::onefunc::PROTOCOL_ID.first_chunk().expect("Expected PROTOCOL_ID be set"),
        );
        let protocol_address: Vec<u8> = onefunc::ID.to_bytes().to_vec();
        let mut tx_id = [0u8; 64];
        rand::thread_rng().fill_bytes(&mut tx_id);
        let op_data = OperationData {
            protocol_id,
            meta,
            src_block_number: 1,
            src_chain_id: photon::photon::SOLANA_CHAIN_ID,
            dest_chain_id: photon::photon::SOLANA_CHAIN_ID,
            nonce: 1,
            src_op_tx_id: tx_id.to_vec(),
            protocol_addr: protocol_address.clone(),
            function_selector: b"\x01\x12init_owned_counter".to_vec(),
            params: <Vec<u8>>::default(),
            reserved: <Vec<u8>>::default(),
        };
        let op_hash_module = op_data.op_hash_with_message();
        let op_data = photon::signature::OperationData::try_from(op_data).unwrap();
        let op_hash_contract = op_data.op_hash_with_message();
        assert_eq!(op_hash_contract, op_hash_module);
    }
}
