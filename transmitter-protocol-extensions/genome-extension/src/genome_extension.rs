use anchor_spl::associated_token;
use std::result::Result;
use anchor_lang::{error_code};
use anchor_spl::associated_token::get_associated_token_address;
use config::Config;
use ethabi::ParamType;
use log::{warn};
use serde::{Deserialize, Deserializer};
use solana_sdk::{
    hash::Hash, instruction::AccountMeta, pubkey::Pubkey, system_program, transaction::Transaction,
};
use spl_associated_token_account::get_associated_token_address_with_program_id;
use photon::photon::ROOT;
use transmitter_common::{error::ExtensionError, protocol_extension::ProtocolExtension};
use genome::GENOME_ROOT;

lazy_static::lazy_static! {
    static ref GENOME_EXTENSION: GenomeExtension = {
        env_logger::init();
        GenomeExtension::from_config()
    };
}

#[no_mangle]
pub fn get_extension() -> &'static dyn ProtocolExtension {
    &*GENOME_EXTENSION
}

fn deserialize_pubkey<'de, D>(deserializer: D) -> Result<Pubkey, D::Error>
where
    D: Deserializer<'de>,
{
    let s = String::deserialize(deserializer)?;
    s.parse().map_err(serde::de::Error::custom)
}

#[derive(Deserialize)]
struct GenomeConfig {
    #[serde(deserialize_with = "hex::serde::deserialize")]
    protocol_id: Vec<u8>,
    #[serde(deserialize_with = "deserialize_pubkey")]
    executor: Pubkey,
    #[serde(deserialize_with = "deserialize_pubkey")]
    zs_program: Pubkey,
    #[serde(deserialize_with = "deserialize_pubkey")]
    core_program: Pubkey,
    seed_root: String,
    #[serde(deserialize_with = "deserialize_pubkey")]
    platform_wallet: Pubkey,
}

struct GenomeExtension {
    protocol_id: &'static [u8; 32],
    zs_program: Pubkey,
    executor: Pubkey,
    seed_root: Vec<u8>,
    zs_token_program: Pubkey,
    platform_wallet: Pubkey,
}

impl ProtocolExtension for GenomeExtension {
    fn get_protocol_id(&self) -> &'static [u8; 32] {
        self.protocol_id
    }

    fn get_accounts(
        &self,
        function_selector: &[u8],
        data: &[u8],
    ) -> Result<Vec<AccountMeta>, ExtensionError> {
        let code = &function_selector[..4];
        let all_params = &ethabi::decode(
            // Function selector is 'receiveAndCall(bytes data)', need to decode
            // 'data' into the params for the actual function call
            &[
                // data Mint params
                ParamType::FixedBytes(32),     // bytes memory receiver
                ParamType::FixedBytes(32),     // bytes memory dstToken
                ParamType::Uint(256), // uint256 amount
                // data Rollback params
                ParamType::FixedBytes(32),   // address provider
                ParamType::Uint(256), // chain id
                // data Call params
                ParamType::FixedBytes(32), // target
                ParamType::Bytes, // data
            ],
            &data,
        ).map_err(|_| ExtensionError::Extension)?;

        let mint = all_params[1].clone().into_fixed_bytes().ok_or(ExtensionError::Extension)?;
        let mint = Pubkey::try_from(mint).map_err(|_| ExtensionError::Extension)?;

        // Last parameter is the actual data, first 4 bytes are the inner function selector
        let params = all_params.last()
            .ok_or(ExtensionError::Extension)?
            .clone()
            .into_bytes()
            .ok_or(ExtensionError::Extension)?;

        let selector = &params[0..4];
        let params = &params[4..];
        Ok(match code {
            &[0x67, 0xb8, 0xfb, 0x72] => match &selector {
                //finishGame(uint256,uint16,bytes32)
                [0x6c, 0x5c, 0x5b, 0x93] => self.get_accounts_finish_game(&mint, params)?,

                //finishGameWithPlaces(uint256,uint16,bytes32[],uint16[])
                [0xc1, 0xbe, 0x2f, 0x7d] => self.get_accounts_finish_game_with_places(&mint, params)?,

                //startGameOmnichain(uint256,uint256,bytes32[],bool)
                [0xd4, 0xe8, 0x64, 0x2b] => self.get_accounts_create_game(&mint, params)?,
                //createTournament(bytes32,bytes32,(uint256,uint256,uint256,uint8,uint8,uint8,uint16,bytes32,uint8),bytes32[],uint8)
                [0xb0, 0x9d, 0x2c, 0x2c] => todo!("Create tournament implementation is not yet decided due to tx limit constraints"),
                //register(uint256,bytes32,bytes32,bytes32[],uint8)
                [0x75, 0xe6, 0x3f, 0x50] => self.get_accounts_register_tournament(&mint, params)?,
                //makeBetOmnichain(bytes32,uint256[])
                [0x5b, 0x69, 0x3a, 0x31] => self.get_accounts_make_bet(&mint, params)?,
                [0x6e, 0x56, 0x23, 0x41] => self.get_accounts_register_game_participants(&mint, params)?,
                _ => return Err(ExtensionError::Extension.into()),
            },
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

impl GenomeExtension {
    fn from_config() -> Self {
        let settings: GenomeConfig = Config::builder()
            .add_source(config::File::with_name("extensions/genome.toml"))
            .add_source(config::Environment::with_prefix("GENOME"))
            .build()
            .expect("Failed to build config for Genome")
            .try_deserialize()
            .expect("Failed to deserialize config for Genome");
        let protocol_id: &'static mut [u8; 32] =
            settings.protocol_id.leak().try_into().expect("Invalid protocol id");
        log::info!("Genome extension initialized");
        Self {
            executor: settings.executor,
            protocol_id,
            zs_program: settings.zs_program,
            seed_root: settings.seed_root.into_bytes(),
            zs_token_program: settings.core_program,
            platform_wallet: settings.platform_wallet,
        }
    }

    fn get_accounts_make_bet(&self, mint: &Pubkey, params: &[u8]) -> Result<Vec<AccountMeta>, ExtensionError> {
        let params = ethabi::decode(
            &[
                ParamType::FixedBytes(32),                        // player
                ParamType::Array(Box::new(ParamType::Uint(256))), // [uuid, captain, captainIndex, amount]
            ],
            &params,
        ).map_err(|_| ExtensionError::Extension)?;
        let gambler = Pubkey::try_from(params[0]
            .clone()
            .into_fixed_bytes()
            .ok_or(ExtensionError::Extension)?).map_err(|_| ExtensionError::Extension)?;

        let bet_params = params[1]
            .clone()
            .into_array()
            .ok_or(ExtensionError::Extension)?;

        let tournament_id = bet_params[0].clone().into_uint().ok_or(ExtensionError::Extension)?.as_u64();
        let captain = match bet_params[1].clone() {
            ethabi::Token::Uint(u) => {
                // Convert U256 to a big-endian 32-byte array
                let mut buf = [0u8; 32];
                u.to_big_endian(&mut buf);
                Pubkey::try_from(buf.to_vec()).map_err(|_| ExtensionError::Extension)?
            }
            _ => return Err(ExtensionError::Extension.into()),
        };

        let tournament = Pubkey::find_program_address(&[&GENOME_ROOT, b"TOURNAMENT", tournament_id.to_le_bytes().as_ref()], &self.zs_program).0;
        let book = Pubkey::find_program_address(&[&GENOME_ROOT, b"BOOK", tournament_id.to_le_bytes().as_ref()], &self.zs_program).0;
        let book_vault = get_associated_token_address(&book, mint);
        let captain_bet = Pubkey::find_program_address(&[&GENOME_ROOT, b"CAPTAIN_BET", tournament_id.to_le_bytes().as_ref(), captain.as_ref()], &self.zs_program).0;
        let gambler_info = Pubkey::find_program_address(&[&GENOME_ROOT, b"GAMBLER", tournament_id.to_le_bytes().as_ref(), captain.as_ref(), gambler.as_ref()], &self.zs_program).0;

        let mut accounts = self.get_common_accounts(mint);
        accounts.push(AccountMeta::new(tournament, false));
        accounts.push(AccountMeta::new(book, false));
        accounts.push(AccountMeta::new(book_vault, false));
        accounts.push(AccountMeta::new(captain_bet, false));
        accounts.push(AccountMeta::new(gambler_info, false));

        Ok(accounts)
    }

    fn get_accounts_register_tournament(&self, mint: &Pubkey, params: &[u8]) -> Result<Vec<AccountMeta>, ExtensionError> {
        let params = ethabi::decode(
            &[
                ParamType::Uint(256),                                   // uint256 uuid
                ParamType::FixedBytes(32),                              // address player
                ParamType::FixedBytes(32),                              // address captain
                ParamType::Array(Box::new(ParamType::FixedBytes(32))),  // address[] memory teammates
                ParamType::Uint(8),                                     // PaymentApproach paymentApproach
            ],
            &params,
        ).expect("Failed to parse register tournament params");

        // Parse the uuid
        let tournament_id = params[0].clone().into_uint().expect("Already parsed").as_u64();

        // Parse the captain
        let captain = Pubkey::try_from(
            params[2]
                .clone()
                .into_fixed_bytes()
                .expect("Already parsed"),
        )
            .expect("Valid address");

        // Parse the accounts
        let tournament = Pubkey::find_program_address(&[&GENOME_ROOT, b"TOURNAMENT", tournament_id.to_le_bytes().as_ref()], &self.zs_program).0;
        let team = Pubkey::find_program_address(&[&GENOME_ROOT, b"TEAM", tournament_id.to_le_bytes().as_ref(), captain.as_ref()], &self.zs_program).0;
        let claimable_user_info = Pubkey::find_program_address(&[&GENOME_ROOT, b"USER", self.executor.as_ref()], &self.zs_program).0;

        let mut accounts = self.get_common_accounts(mint);
        accounts.push(AccountMeta::new(tournament, false));
        accounts.push(AccountMeta::new(captain, false));
        accounts.push(AccountMeta::new(team, false));
        accounts.push(AccountMeta::new(claimable_user_info, false));

        Ok(accounts)
    }

    fn get_accounts_finish_game_with_places(&self, mint: &Pubkey, params: &[u8]) -> Result<Vec<AccountMeta>, ExtensionError> {
        let finish_game_params = ethabi::decode(
            &[
                ParamType::Uint(256),                            // uint256 uuid
                ParamType::Uint(16),                             // uint16 feeType
                ParamType::Array(Box::new(ParamType::Address)),  // address[] calldata winners
                ParamType::Array(Box::new(ParamType::Uint(16))), // uint16[] calldata prizeFractions
            ],
            &params,
        ).expect("Failed to parse finish game with places params");

        // Parse the feeType
        let fee_type = finish_game_params[1]
            .clone()
            .into_uint()
            .expect("Already parsed")
            .as_usize() as u16;
        let fee_meta = Pubkey::find_program_address(&[&GENOME_ROOT, b"FEE_META", fee_type.to_le_bytes().as_ref()], &self.zs_program).0;

        // Parse the winner
        let winners = finish_game_params[2]
            .clone()
            .into_array()
            .expect("Already parsed")
            .into_iter()
            .map(|w| {
                let p = Pubkey::try_from(w.into_fixed_bytes().expect("Valid address")).expect("Valid Pubkey");
                AccountMeta::new_readonly(p, false)
            })
            .collect::<Vec<_>>();

        let game_id = finish_game_params[0].clone().into_uint().expect("Failed to parse UUID").as_u64();

        let game = Pubkey::find_program_address(&[&GENOME_ROOT, b"GAME", game_id.to_le_bytes().as_ref()], &self.zs_program).0;
        let game_vault = get_associated_token_address(&game, mint);

        let treasury_authority = Pubkey::find_program_address(&[&GENOME_ROOT, b"AUTHORITY"], &self.zs_program).0;
        let treasury_vault = get_associated_token_address(&treasury_authority, mint);

        let mut accounts = self.get_common_accounts(mint);
        accounts.push(AccountMeta::new(treasury_vault, false));
        accounts.push(AccountMeta::new(fee_meta, false));
        accounts.push(AccountMeta::new(game, false));
        accounts.push(AccountMeta::new(game_vault, false));
        winners.into_iter().for_each(|w| accounts.push(w));

        Ok(accounts)
    }

    fn get_accounts_finish_game(&self, mint: &Pubkey, params: &[u8]) -> Result<Vec<AccountMeta>, ExtensionError> {
        let finish_game_params = ethabi::decode(
            &[
                ParamType::Uint(256),      // uint256 uuid
                ParamType::Uint(16),       // uint16 feeType
                ParamType::FixedBytes(32), // address winner
            ],
            &params,
        ).expect("Failed to parse get_accounts_finish_game params");

        // Parse the feeType
        let fee_type = finish_game_params[1]
            .clone()
            .into_uint()
            .expect("Already parsed")
            .as_usize() as u16;
        let fee_meta = Pubkey::find_program_address(&[&GENOME_ROOT, b"FEE_META", fee_type.to_le_bytes().as_ref()], &self.zs_program).0;

        // Parse the winner
        let winner = finish_game_params[2]
            .clone()
            .into_fixed_bytes()
            .expect("Already parsed");

        let game_id = finish_game_params[0].clone().into_uint().expect("Failed to parse UUID").as_u64();

        let game = Pubkey::find_program_address(&[&GENOME_ROOT, b"GAME", game_id.to_le_bytes().as_ref()], &self.zs_program).0;
        let game_vault = get_associated_token_address(&game, mint);
        // Parse the participants
        let winner = AccountMeta::new(Pubkey::try_from(winner).expect("Failed to parse Pubkey"), false);

        let treasury_authority = Pubkey::find_program_address(&[&GENOME_ROOT, b"AUTHORITY"], &self.zs_program).0;
        let treasury_vault = get_associated_token_address(&treasury_authority, mint);

        let mut accounts = self.get_common_accounts(mint);
        accounts.push(AccountMeta::new(treasury_vault, false));
        accounts.push(AccountMeta::new(fee_meta, false));
        accounts.push(AccountMeta::new(game, false));
        accounts.push(AccountMeta::new(game_vault, false));
        accounts.push(winner);

        Ok(accounts)
    }

    fn get_accounts_create_game(&self, mint: &Pubkey, params: &[u8]) -> Result<Vec<AccountMeta>, ExtensionError> {
        let create_game_params = ethabi::decode(
            &[
                ParamType::Uint(256),                                   // uint256 uuid
                ParamType::Uint(256),                                   // uint256 wager
                ParamType::Array(Box::new(ParamType::FixedBytes(32))),  // bytes32[] calldata participants
                ParamType::Bool // bool startGame
            ],
            &params,
        ).map_err(|_| ExtensionError::Extension)?;

        let game_id = create_game_params[0].clone().into_uint().expect("Failed to parse UUID").as_u64();

        let game = Pubkey::find_program_address(&[&GENOME_ROOT, b"GAME", game_id.to_le_bytes().as_ref()], &self.zs_program).0;
        let game_vault = get_associated_token_address(&game, mint);


        let mut accounts = self.get_common_accounts(mint);
        accounts.push(AccountMeta::new(game, false));
        accounts.push(AccountMeta::new(game_vault, false));

        Ok(accounts)
    }

    fn get_accounts_register_game_participants(&self, mint: &Pubkey, params: &[u8]) -> Result<Vec<AccountMeta>, ExtensionError> {
        let create_game_params = ethabi::decode(
            &[
                ParamType::Uint(256),                                   // uint256 uuid
                ParamType::Array(Box::new(ParamType::FixedBytes(32))),  // bytes32[] calldata participants
                ParamType::Bool // start_game
            ],
            &params,
        ).map_err(|_| ExtensionError::Extension)?;

        let game_id = create_game_params[0].clone().into_uint().expect("Failed to parse UUID").as_u64();

        let game = Pubkey::find_program_address(&[&GENOME_ROOT, b"GAME", game_id.to_le_bytes().as_ref()], &self.zs_program).0;
        let game_vault = get_associated_token_address(&game, mint);

        let mut accounts = self.get_common_accounts(mint);
        accounts.push(AccountMeta::new(game, false));
        accounts.push(AccountMeta::new(game_vault, false));

        Ok(accounts)
    }

    fn get_common_accounts(&self, mint: &Pubkey) -> Vec<AccountMeta> {
        let (operator_info, _) =
            Pubkey::find_program_address(&[&self.seed_root, b"OPERATOR", &self.executor.as_ref()], &self.zs_program);

        let (zs_config, _) =
            Pubkey::find_program_address(&[&self.seed_root, b"CONFIG"], &self.zs_program);

        let (token_config, _) =
            Pubkey::find_program_address(&[&ROOT, b"CONFIG"], &self.zs_token_program);

        let (zs_authority, _) = Pubkey::find_program_address(&[&GENOME_ROOT, b"AUTHORITY"], &self.zs_program);

        let (token_authority, _) =
            Pubkey::find_program_address(&[&ROOT, b"AUTHORITY"], &self.zs_token_program);

        let vault = get_associated_token_address_with_program_id(
            &self.executor,
            mint,
            &spl_token::id(),
        );
        vec![
            AccountMeta::new_readonly(self.zs_program, false),
            AccountMeta::new(operator_info, false),
            AccountMeta::new(vault, true),
            AccountMeta::new(zs_config, true),
            AccountMeta::new(token_authority, false),
            AccountMeta::new(mint.clone(), false),
            AccountMeta::new_readonly(token_config, false),
            AccountMeta::new_readonly(self.zs_token_program, false),
            AccountMeta::new_readonly(zs_authority, false),
            AccountMeta::new(get_associated_token_address(&zs_authority, mint), false),
            AccountMeta::new(self.platform_wallet, false),
            AccountMeta::new(get_associated_token_address(&self.platform_wallet, mint), false),
            AccountMeta::new_readonly(spl_token::ID, false),
            AccountMeta::new_readonly(associated_token::ID, false),
            AccountMeta::new_readonly(system_program::id(), false),
        ]
    }
}

#[error_code]
pub enum GenomeError {
    #[msg("InvalidParams")]
    InvalidParams,
    #[msg("InvalidSelector")]
    InvalidSelector,
}