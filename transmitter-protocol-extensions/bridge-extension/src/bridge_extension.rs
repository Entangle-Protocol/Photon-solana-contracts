use ethabi::ParamType;
use log::{error, warn};
use solana_sdk::{
    hash::Hash, instruction::AccountMeta, pubkey::Pubkey, system_program, transaction::Transaction,
};
use transmitter_common::{error::ExtensionError, protocol_extension::ProtocolExtension};

lazy_static::lazy_static! {
    static ref BRIDGE_EXTENTION: BridgeExtension = {
        env_logger::init();
        BridgeExtension::from_env("NGL")
    };
}

#[no_mangle]
pub fn get_extension() -> &'static dyn ProtocolExtension {
    &*BRIDGE_EXTENTION
}

struct BridgeExtension {
    protocol_id: &'static [u8; 32],
    bridge_program: Pubkey,
    bridge_root: Vec<u8>,
    mint: Pubkey,
    core_program: Pubkey,
    use_token2022: bool,
}

impl ProtocolExtension for BridgeExtension {
    fn get_protocol_id(&self) -> &'static [u8; 32] {
        self.protocol_id
    }

    fn get_accounts(
        &self,
        function_selector: &[u8],
        params: &[u8],
    ) -> Result<Vec<AccountMeta>, ExtensionError> {
        let code = function_selector.first_chunk::<4>().ok_or_else(|| {
            error!("Failed to get first chunk of bridge selector");
            ExtensionError::Extension
        })?;
        Ok(match code {
            &[0x99, 0x45, 0xe3, 0xd3] => self.get_accounts_redeem(params)?,
            _ => {
                let selector = String::from_utf8_lossy(function_selector);
                warn!("Unexpected function selector: {}", selector);
                vec![]
            }
        })
    }

    fn sign_transaction(
        &self,
        _function_selector: &[u8],
        _params: &[u8],
        _transaction: &mut Transaction,
        _recent_block_hash: &Hash,
    ) -> Result<(), ExtensionError> {
        Ok(())
    }
}

impl BridgeExtension {
    fn from_env(prefix: &str) -> Self {
        let _ = dotenvy::dotenv();
        todo!()
    }

    fn get_accounts_redeem(&self, params: &[u8]) -> Result<Vec<AccountMeta>, ExtensionError> {
        let (authority, _) =
            Pubkey::find_program_address(&[&self.bridge_root, b"AUTHORITY"], &self.bridge_program);
        let (config, _) =
            Pubkey::find_program_address(&[&self.bridge_root, b"CONFIG"], &self.bridge_program);
        let (core_authority, _) =
            Pubkey::find_program_address(&[&self.bridge_root, b"AUTHORITY"], &self.core_program);
        let (core_config, _) =
            Pubkey::find_program_address(&[&self.bridge_root, b"CONFIG"], &self.core_program);
        let params = ethabi::decode(
            &[
                ParamType::Bytes,          // bytes memory to
                ParamType::Uint(256),      // uint256 amount
                ParamType::Uint(256),      // uint256 fee
                ParamType::FixedBytes(32), // bytes32 txHash
                ParamType::Uint(256),      // chainIdFrom
                ParamType::FixedBytes(32), // nonce
            ],
            params,
        )
        .map_err(|e| {
            error!("Failed to parse redeem data: {:?}", e);
            ExtensionError::Extension
        })?;
        let user = params[0].clone().into_bytes().expect("Already parsed");
        let user = Pubkey::try_from(user).map_err(|e| {
            error!("Invalid destination vault pubkey: {:?}", e);
            ExtensionError::Extension
        })?;
        let user_vault = spl_associated_token_account::get_associated_token_address_with_program_id(
            &user,
            &self.mint,
            if self.use_token2022 {
                &spl_token_2022::ID
            } else {
                &spl_token::ID
            },
        );
        Ok(vec![
            AccountMeta::new_readonly(authority, false),
            AccountMeta::new(self.mint, false),
            AccountMeta::new_readonly(user, false),
            AccountMeta::new(user_vault, false),
            AccountMeta::new_readonly(config, false),
            AccountMeta::new_readonly(core_config, false),
            AccountMeta::new_readonly(core_authority, false),
            AccountMeta::new_readonly(self.core_program, false),
            AccountMeta::new_readonly(
                if self.use_token2022 {
                    spl_token_2022::ID
                } else {
                    spl_token::ID
                },
                false,
            ),
            AccountMeta::new_readonly(system_program::id(), false),
        ])
    }
}
