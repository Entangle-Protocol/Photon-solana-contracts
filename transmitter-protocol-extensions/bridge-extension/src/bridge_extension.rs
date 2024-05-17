use anchor_spl::associated_token;
use config::Config;
use ethabi::ParamType;
use log::{error, warn};
use serde::{Deserialize, Deserializer};
use solana_sdk::{
    hash::Hash, instruction::AccountMeta, pubkey::Pubkey, system_program, transaction::Transaction,
};
use transmitter_common::{error::ExtensionError, protocol_extension::ProtocolExtension};

lazy_static::lazy_static! {
    static ref BRIDGE_EXTENTION: BridgeExtension = {
        env_logger::init();
        BridgeExtension::from_config("NGL")
    };
}

#[no_mangle]
pub fn get_extension() -> &'static dyn ProtocolExtension {
    &*BRIDGE_EXTENTION
}

fn deserialize_pubkey<'de, D>(deserializer: D) -> Result<Pubkey, D::Error>
where
    D: Deserializer<'de>,
{
    let s = String::deserialize(deserializer)?;
    s.parse().map_err(serde::de::Error::custom)
}

#[derive(Deserialize)]
struct BridgeConfig {
    #[serde(deserialize_with = "hex::serde::deserialize")]
    protocol_id: Vec<u8>,
    #[serde(deserialize_with = "deserialize_pubkey")]
    bridge_program: Pubkey,
    #[serde(deserialize_with = "deserialize_pubkey")]
    core_program: Pubkey,
    seed_root: String,
    #[serde(deserialize_with = "deserialize_pubkey")]
    mint: Pubkey,
    use_token2022: bool,
}

struct BridgeExtension {
    protocol_id: &'static [u8; 32],
    bridge_program: Pubkey,
    seed_root: Vec<u8>,
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
    fn from_config(prefix: &str) -> Self {
        let settings: BridgeConfig = Config::builder()
            .add_source(config::File::with_name(&format!(
                "extensions/bridge-{}.toml",
                prefix.to_lowercase()
            )))
            .add_source(config::Environment::with_prefix(&format!("BRIDGE_{}", prefix)))
            .build()
            .expect("Failed to build config for bridge")
            .try_deserialize()
            .expect("Failed to deserialize config for bridge");
        let protocol_id: &'static mut [u8; 32] =
            settings.protocol_id.leak().try_into().expect("Invalid protocol id");
        log::info!("Bridge extension initialized");
        Self {
            protocol_id,
            bridge_program: settings.bridge_program,
            seed_root: settings.seed_root.into_bytes(),
            mint: settings.mint,
            core_program: settings.core_program,
            use_token2022: settings.use_token2022,
        }
    }

    fn get_accounts_redeem(&self, params: &[u8]) -> Result<Vec<AccountMeta>, ExtensionError> {
        let (authority, _) =
            Pubkey::find_program_address(&[&self.seed_root, b"AUTHORITY"], &self.bridge_program);
        let (config, _) =
            Pubkey::find_program_address(&[&self.seed_root, b"CONFIG"], &self.bridge_program);
        let (core_authority, _) =
            Pubkey::find_program_address(&[&self.seed_root, b"AUTHORITY"], &self.core_program);
        let (core_config, _) =
            Pubkey::find_program_address(&[&self.seed_root, b"CONFIG"], &self.core_program);
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
            AccountMeta::new_readonly(self.bridge_program, false),
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
            AccountMeta::new_readonly(associated_token::ID, false),
            AccountMeta::new_readonly(system_program::id(), false),
        ])
    }
}
