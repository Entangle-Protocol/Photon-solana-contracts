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
    "e79dad9ac664c5e3c6b7297a18e28f9522f66009b727a73a5370f5b475331560",
    "c9f34cd7e4366498ecc4ab36d339d0176bc835aa4b81629259b9f09981360052",
    "19a4ea1b0e72d931ce0482e58a7166e28e748d028d726a0a4a1efcb70d704f22",
];
