use log::{debug, error};
use photon::{
    gov::{abi_decode_scheme, decode_abi_params, GovOperation},
    photon::ROOT,
    protocol_data::GOV_PROTOCOL_ID,
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
        function_selector: &[u8],
        params: &[u8],
    ) -> Result<Vec<AccountMeta>, ExtensionError> {
        let (gov_protocol_pda, _) =
            Pubkey::find_program_address(&[ROOT, b"PROTOCOL", GOV_PROTOCOL_ID], &photon::ID);

        let code = <[u8; 4]>::try_from(function_selector).map_err(|_| {
            error!("Failed to get first chunk of gov selector");
            ExtensionError::Extension
        })?;

        let selector_u32 = u32::from_be_bytes(code);
        let gov_operation = GovOperation::try_from(selector_u32).map_err(|err| {
            error!("Failed to get gov_operation from selector: {}", err);
            ExtensionError::Extension
        })?;

        let params =
            decode_abi_params(params, abi_decode_scheme(gov_operation)).map_err(|err| {
                error!("Failed to decode abi params: {}", err);
                ExtensionError::Extension
            })?;

        let target_protocol_id = params
            .first()
            .ok_or_else(|| {
                error!("Failed to get first decoded abi param");
                ExtensionError::Extension
            })?
            .clone()
            .into_fixed_bytes()
            .ok_or_else(|| {
                error!("Failed to convert first decoded abi param as fixed_bytes");
                ExtensionError::Extension
            })?;

        let (target_protocol_info_pda, _) = Pubkey::find_program_address(
            &[ROOT, b"PROTOCOL", target_protocol_id.as_slice()],
            &photon::ID,
        );

        let (config_pda, _) = Pubkey::find_program_address(&[ROOT, b"CONFIG"], &photon::ID);

        debug!(
            "target_protocol_id_data: {}, target_protocol_id: {}, config: {}, target_protocol_info: {}",
            hex::encode(&target_protocol_id),
            String::from_utf8(target_protocol_id.clone()).unwrap_or("not-utf8".to_string()),
            config_pda,
            target_protocol_info_pda
        );

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
