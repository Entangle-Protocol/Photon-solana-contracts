use log::{error, warn};
use solana_sdk::{
    hash::Hash, instruction::AccountMeta, pubkey::Pubkey, signature::Keypair, signer::Signer,
    system_program::id as system_program_id, transaction::Transaction,
};
use transmitter_common::{error::ExtensionError, protocol_extension::ProtocolExtension};

lazy_static::lazy_static! {
    static ref ONEFUNC_EXTENTION: OnefuncExtension = {
        env_logger::init();
        OnefuncExtension {
            counter_owner: Keypair::from_bytes(&[
                4, 68, 189, 12, 104, 184, 115, 53, 21, 20, 224, 30, 224, 59, 226, 242, 150, 131, 204,
                211, 139, 84, 45, 105, 124, 89, 238, 5, 100, 74, 130, 246, 45, 9, 227, 148, 3, 83, 49,
                198, 80, 233, 123, 51, 130, 106, 145, 145, 10, 132, 76, 61, 75, 209, 228, 215, 171, 33,
                136, 8, 27, 167, 45, 54,
            ]).expect("Expected counter_authority be initialized well")
    }};
}

#[no_mangle]
pub fn get_extension() -> &'static dyn ProtocolExtension {
    &*ONEFUNC_EXTENTION
}

struct OnefuncExtension {
    counter_owner: Keypair,
}

impl ProtocolExtension for OnefuncExtension {
    fn get_protocol_id(&self) -> &'static [u8; 32] {
        <&[u8; 32]>::try_from("onefunc_________________________".as_bytes())
            .expect("Expected onefunc-extension protocol_id be converted well")
    }

    fn get_accounts(
        &self,
        function_selector: &[u8],
        _params: &[u8],
    ) -> Result<Vec<AccountMeta>, ExtensionError> {
        Ok(match function_selector {
            b"init_owned_counter" => self.get_accounts_init_counter(),
            b"increment_owned_counter" => self.get_accounts_increment(),
            b"\x01\x02\x03\x04" => self.get_accounts_receive_photon_msg(),
            _ => {
                let selector = String::from_utf8_lossy(function_selector);
                warn!("Unexpected function selector: {}", selector);
                vec![]
            }
        })
    }

    fn sign_transaction(
        &self,
        function_selector: &[u8],
        _params: &[u8],
        transaction: &mut Transaction,
        hash: &Hash,
    ) -> Result<(), ExtensionError> {
        match function_selector {
            b"init_owned_counter" | b"increment_owned_counter" => {
                transaction.try_partial_sign(&[&self.counter_owner], *hash).map_err(|err| {
                    error!("Failed to partial sign tx: {}", err);
                    ExtensionError::Sign
                })
            }
            b"\x01\x02\x03\x04" => Ok(()),
            _ => {
                warn!("Unexpected function selector");
                Ok(())
            }
        }
    }
}

impl OnefuncExtension {
    fn get_accounts_init_counter(&self) -> Vec<AccountMeta> {
        let counter_owner = self.counter_owner.pubkey();
        let (onefunc_counter_pda, _) =
            Pubkey::find_program_address(&[b"COUNTER", counter_owner.as_ref()], &onefunc::ID);
        vec![
            AccountMeta::new_readonly(onefunc::ID, false),
            AccountMeta::new_readonly(self.counter_owner.pubkey(), true),
            AccountMeta::new(onefunc_counter_pda, false),
            AccountMeta::new(system_program_id(), false),
        ]
    }

    fn get_accounts_increment(&self) -> Vec<AccountMeta> {
        let counter_owner = self.counter_owner.pubkey();
        let (onefunc_counter_pda, _) =
            Pubkey::find_program_address(&[b"COUNTER", counter_owner.as_ref()], &onefunc::ID);
        vec![
            AccountMeta::new_readonly(onefunc::ID, false),
            AccountMeta::new_readonly(self.counter_owner.pubkey(), true),
            AccountMeta::new(onefunc_counter_pda, false),
        ]
    }

    fn get_accounts_receive_photon_msg(&self) -> Vec<AccountMeta> {
        vec![AccountMeta::new_readonly(onefunc::ID, false)]
    }
}
