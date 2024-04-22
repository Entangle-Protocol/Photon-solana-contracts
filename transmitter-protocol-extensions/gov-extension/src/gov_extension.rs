use ethabi::ParamType;
use log::{debug, error};
use photon::{
    photon::ROOT,
    util::{gov_protocol_id, GOV_PROTOCOL_ID},
};
use solana_sdk::{
    hash::Hash, instruction::AccountMeta, pubkey::Pubkey, system_program, transaction::Transaction,
};

use transmitter_common::{error::ExtensionError, protocol_extension::ProtocolExtension};

lazy_static::lazy_static! {
    static ref GOV_EXTENSION: GovExtension = {
        env_logger::init();
        GovExtension
    };
}

#[no_mangle]
pub fn get_extension() -> &'static dyn ProtocolExtension {
    &*GOV_EXTENSION
}

struct GovExtension;

impl ProtocolExtension for GovExtension {
    fn get_protocol_id(&self) -> &'static [u8; 32] {
        GOV_PROTOCOL_ID
    }

    fn get_accounts(
        &self,
        _function_selector: &[u8],
        _params: &[u8],
    ) -> Result<Vec<AccountMeta>, ExtensionError> {
        let (gov_protocol_pda, _) =
            Pubkey::find_program_address(&[ROOT, b"PROTOCOL", gov_protocol_id()], &photon::ID);

        let add_protocol_params = ethabi::decode(
            &[
                ParamType::FixedBytes(32), // protocolId
                ParamType::Uint(256),      // consensusTargetRate
                ParamType::Array(Box::new(ParamType::Address)),
            ],
            _params,
        )
        .map_err(|err| {
            error!("Failed to decode add_protocol_params: {}", err);
            ExtensionError::Extension
        })?;

        let (target_protocol_info_pda, _) = Pubkey::find_program_address(
            &[
                ROOT,
                b"PROTOCOL",
                add_protocol_params[0].clone().into_fixed_bytes().expect("").as_slice(),
            ],
            &photon::ID,
        );

        let (config_pda, _) = Pubkey::find_program_address(&[ROOT, b"CONFIG"], &photon::ID);

        Ok(vec![
            AccountMeta::new_readonly(photon::ID, false),
            AccountMeta::new(config_pda, false),
            AccountMeta::new(gov_protocol_pda, false),
            AccountMeta::new(target_protocol_info_pda, false),
            AccountMeta::new(system_program::id(), false),
        ])
    }

    fn sign_transaction(
        &self,
        _function_selector: &[u8],
        _params: &[u8],
        _transaction: &mut Transaction,
        _hash: &Hash,
    ) -> Result<(), ExtensionError> {
        debug!("gov get sign transaction");
        Ok(())
    }

    fn get_compute_budget(&self, _function_selector: &[u8], _params: &[u8]) -> Option<u32> {
        Some(400000)
    }
}
