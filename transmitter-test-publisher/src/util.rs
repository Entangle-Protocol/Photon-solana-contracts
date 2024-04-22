use libsecp256k1::{PublicKey, SecretKey};
use log::debug;
use photon::signature::derive_eth_address;

#[derive(Clone, Debug)]
pub struct KeeperSignature {
    pub r: Vec<u8>,
    pub s: Vec<u8>,
    pub v: u8,
}

pub fn predefined_signers(amount: usize) -> Vec<(SecretKey, PublicKey)> {
    let mut keepers = vec![];
    for (i, data) in KEEPER_DATA.iter().enumerate().take(amount) {
        let keeper_sk = SecretKey::parse_slice(
            &hex::decode(data).expect("Expected keeper data to be decoded well"),
        )
        .expect("Expected secret key to be built well");
        let keeper_pk = PublicKey::from_secret_key(&keeper_sk);
        keepers.push((keeper_sk, keeper_pk));
        let eth_addr =
            hex::encode(derive_eth_address(keeper_pk.serialize().as_slice())).to_uppercase();
        debug!("KEEPER {} {}", i, eth_addr);
    }
    keepers
}

const KEEPER_DATA: [&str; 3] = [
    "74e3ffad2b87174dc1d806edf1a01e3b017cf1be05d1894d329826f10fa1d72f",
    "66a222403ce2448cdf98d7194d9d0e4533c354f8f472d594ba5b50d2568d2c08",
    "b948355471a4013b8166fa7cfee601c6aeae38ee38066a5ddf821a9b0db9dd10",
];
