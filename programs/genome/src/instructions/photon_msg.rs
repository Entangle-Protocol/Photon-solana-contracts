use crate::GenomeConfig;
use crate::{error::OmnichainError, OperatorInfo, Role, TournamentParams, GENOME_PROTOCOL_ID, GENOME_ROOT};
use anchor_lang::prelude::*;
use anchor_spl::{
    associated_token::AssociatedToken,
    token::Token,
    token_interface::{Mint, TokenAccount},
};
use ethabi::ethereum_types::{H160, U256};
use ethabi::ParamType;
use ngl_core::{
    cpi::{accounts::MintToken, mint_token},
    program::NglCore,
    ROOT as NGL_ROOT,
};

use super::BP_DEC;
use crate::error::ControlAccessError;
use photon::{program::Photon, OpInfo};
use solana_program::{instruction::Instruction, program::invoke};
use ngl_core::cpi::accounts::BurnToken;
use ngl_core::cpi::burn_token;
use photon::cpi::accounts::Propose;
use photon::cpi::propose;
use photon::protocol_data::FunctionSelector;
use crate::error::OmnichainError::{InvalidParams, };

#[derive(Accounts)]
pub struct PhotonMsg<'info> {
    /// Executor wallet
    #[account(mut, signer)]
    pub executor: Signer<'info>,
    /// Protocol call authority (from photon program)
    #[account(
        signer,
        seeds = [photon::photon::ROOT, b"CALL_AUTHORITY", GENOME_PROTOCOL_ID],
        bump,
        seeds::program = Photon::id()
    )]
    pub call_authority: Signer<'info>,

    /// Provided by photon program
    pub op_info: Box<Account<'info, OpInfo>>,

    #[account(
        seeds = [GENOME_ROOT, b"OPERATOR", executor.key().as_ref()],
        bump,
        constraint = operator_info.approved @ ControlAccessError::OperatorNotApproved,
        constraint = operator_info.role == Role::MESSENGER @ ControlAccessError::OperatorNotMessenger,
    )]
    pub operator_info: Box<Account<'info, OperatorInfo>>,

    /// The vault to mint
    #[account(mut, associated_token::mint = mint, associated_token::authority = executor, associated_token::token_program = token_program
    )]
    pub executor_vault: Box<InterfaceAccount<'info, TokenAccount>>,

    #[account(mut, seeds = [GENOME_ROOT, b"CONFIG"], bump)]
    pub zs_config: Box<Account<'info, GenomeConfig>>,

    /// Genome Token mint authority
    #[account(seeds = [NGL_ROOT, b"AUTHORITY"], bump, seeds::program = NglCore::id())]
    pub token_authority: UncheckedAccount<'info>,

    /// Token mint (checked by core)
    #[account(mut)]
    pub mint: Box<InterfaceAccount<'info, Mint>>,

    /// Token config
    /// CHECK: by core program
    pub token_config: Box<Account<'info, ngl_core::Config>>,

    pub zs_token_program: Program<'info, NglCore>,

    /// Genome Token mint authority
    #[account(seeds = [GENOME_ROOT, b"AUTHORITY"], bump)]
    pub treasury_authority: UncheckedAccount<'info>,
    #[account(init_if_needed, payer = executor, associated_token::mint = mint, associated_token::authority = treasury_authority, associated_token::token_program = token_program)]
    pub treasury_vault: Box<InterfaceAccount<'info, TokenAccount>>,

    /// CHECK: platform wallet
    #[account(mut)]
    pub platform_wallet: AccountInfo<'info>,
    #[account(init_if_needed, payer = executor, associated_token::mint = mint, associated_token::authority = platform_wallet, associated_token::token_program = token_program)]
    pub platform_wallet_vault: Box<InterfaceAccount<'info, TokenAccount>>,

    pub token_program: Program<'info, Token>,
    pub associated_token_program: Program<'info, AssociatedToken>,
    pub system_program: Program<'info, System>,

    pub protocol_info: AccountInfo<'info>,
    pub photon_config: AccountInfo<'info>,
    pub photon_program: Program<'info, Photon>,
}

/// Handle a photon message
/// Accepts as params a code as a vector of u8 and the params as a vector of u8
pub fn handle_photon_msg<'c, 'info>(
    ctx: Context<'_, '_, 'c, 'info, PhotonMsg<'info>>,
    code: Vec<u8>,
    params: Vec<u8>,
) -> Result<()> {
    let protocol_id = &ctx.accounts.op_info.op_data.protocol_id;
    require!(
        protocol_id == GENOME_PROTOCOL_ID,
        OmnichainError::InvalidProtocolId
    );
    // From the code, take the function selector as u32
    let selector = u32::from_be_bytes(
        code[..4]
            .try_into()
            .map_err(|_| OmnichainError::InvalidSelector)?,
    );

    // Match the selector against our expected call
    // Our select should always match first the receiveAndCall(bytes) as seen on:
    // https://github.com/rather-labs/entangle/blob/master/evm_contracts/omnichain/omni-messenger/core/SingleUserMessagging.sol#L83
    match selector {
        // This match against `receiveAndCall(bytes)`, the proof can be found at
        // ../utils/evm_keccak_signature.rs
        0x67b8fb72_u32 => {
            // The params are usually passed similar to:
            // IProposer(endpoint).propose(
            //     protocolId,
            //     destChainId,
            //     remoteProvider,
            //     PhotonFunctionSelectorLib.encodeEvmSelector(
            //         bytes4(keccak256(selector))
            //     ),
            //     params
            // );
            // https://github.com/rather-labs/entangle/blob/master/evm_contracts/omnichain/omni-messenger/common/MessagingUtils.sol#L25-L33

            // Params are as follow:
            // abi.encode(
            //     // mint params
            //     receiver,
            //     dstToken,
            //     amount,
            //     // rollback params
            //     abi.encode(originalOwner),
            //     abi.encode(address(this)),
            //     uint256(block.chainid),
            //     // call params
            //     callParams.target,
            //     callParams.data
            // );
            // https://github.com/rather-labs/entangle/blob/master/evm_contracts/omnichain/omni-messenger/core/SingleUserMessagging.sol#L253-L265

            // A similar decoding to this one can found on the EVM layer:
            // https://github.com/rather-labs/entangle/blob/master/evm_contracts/omnichain/omni-messenger/core/SingleUserMessagging.sol#L281-L321
            let params = ethabi::decode(
                &[
                    // Mint params
                    ParamType::FixedBytes(32), // bytes memory receiver
                    ParamType::FixedBytes(32), // bytes memory srcToken
                    ParamType::FixedBytes(32), // bytes memory dstToken
                    ParamType::Uint(256),      // uint256 amount
                    // Rollback params
                    ParamType::FixedBytes(32), // address zsMessenger rollback
                    ParamType::Uint(256),      // chainId
                    ParamType::FixedBytes(32), // Target
                    // Call params
                    ParamType::Bytes, // data
                ],
                &params,
            )
                .map_err(|_| OmnichainError::InvalidParams)?;

            let src_token = params[2]
                .clone()
                .into_fixed_bytes()
                .ok_or_else(|| OmnichainError::InvalidPubkey)?;

            // Parse the amount
            let amount = params[3]
                .clone()
                .into_uint()
                .ok_or_else(|| OmnichainError::InvalidUserAccount)?
                .as_u64();
            // Parse the provider
            let provider = params[4]
                .clone()
                .into_fixed_bytes()
                .ok_or_else(|| OmnichainError::InvalidPubkey)?;
            // Parse the chain ID
            let src_chain_id = params[5]
                .clone()
                .into_uint()
                .ok_or(OmnichainError::InvalidParams)?
                .as_u64();
            // Parse the target
            let target = params[6]
                .clone()
                .into_fixed_bytes()
                .ok_or_else(|| OmnichainError::InvalidProtocolId)?;
            // Parse the data
            let data = params[7]
                .clone()
                .into_bytes()
                .ok_or_else(|| OmnichainError::InvalidMethodId)?;

            // Mint for treasury
            if amount > 0 {
                let bump = &[ctx.bumps.token_authority];
                let seed = &[NGL_ROOT, &b"AUTHORITY"[..], bump][..];
                let seeds = &[seed];
                let accounts = MintToken {
                    mint_authority: ctx.accounts.executor.to_account_info(),
                    authority: ctx.accounts.token_authority.to_account_info(),
                    config: ctx.accounts.token_config.to_account_info(),
                    mint: ctx.accounts.mint.to_account_info(),
                    vault: ctx.accounts.executor_vault.to_account_info(),
                    token_program: ctx.accounts.token_program.to_account_info(),
                };
                let cpi_ctx = CpiContext::new_with_signer(
                    ctx.accounts.zs_token_program.to_account_info(),
                    accounts,
                    seeds,
                );

                mint_token(cpi_ctx, amount)?;
            }

            // Now we call the remaining functions
            if let Err(e) = handle_cpi_call(&ctx, data.clone()) {
                let accounts = BurnToken {
                    vault_owner: ctx.accounts.executor.to_account_info(),
                    burn_authority: ctx.accounts.token_authority.to_account_info(),
                    config: ctx.accounts.token_config.to_account_info(),
                    mint: ctx.accounts.mint.to_account_info(),
                    vault: ctx.accounts.executor_vault.to_account_info(),
                    token_program: ctx.accounts.token_program.to_account_info(),
                };
                let bump = &[ctx.bumps.token_authority];
                let seed = &[NGL_ROOT, &b"AUTHORITY"[..], bump][..];
                let seeds = &[seed];
                let burn_ctx = CpiContext::new_with_signer(
                    ctx.accounts.zs_token_program.to_account_info(),
                    accounts,
                    seeds,
                );
                burn_token(burn_ctx, amount)?;
                let accounts = Propose {
                    proposer: ctx.accounts.token_authority.to_account_info(),
                    config: ctx.accounts.photon_config.to_account_info(),
                    protocol_info: ctx.accounts.protocol_info.to_account_info(),
                };
                let cpi_ctx =
                    CpiContext::new_with_signer(ctx.accounts.photon_program.to_account_info(), accounts, seeds);
                // rollback(bytes)
                let raw = [&0x91bb65f3_u32.to_be_bytes()[..], &[0_u8; 28]].concat();
                let function_selector = FunctionSelector::ByCode(ethabi::encode(&[ethabi::Token::Uint(
                    U256::from_big_endian(&raw),
                )]));
                let params = ethabi::encode(&[
                    ethabi::Token::FixedBytes(src_token),   // bytes memory srcToken
                    ethabi::Token::FixedBytes(todo!("Parse original sender")),  // uint256 chainIdTo
                    ethabi::Token::Uint(U256::from(amount)),    // uint256 amount
                ]);
                propose(cpi_ctx, GENOME_PROTOCOL_ID.to_vec(), src_chain_id as u128, target, function_selector, params)?;
                return Err(e);
            }
            // This reflects the event https://github.com/rather-labs/entangle/blob/master/evm_contracts/omnichain/omni-messenger/core/SingleUserMessagging.sol#L219
            // Before was the minted event
            emit!(SuccessfulCall {
                src_chain_id,
                provider,
                target,
                data,
            });
        }
        _ => return Err(OmnichainError::InvalidMethodId.into()),
    }
    Ok(())
}

/// Handles the CPI calls from the data we are are getting packed from ABI
fn handle_cpi_call<'c, 'info>(
    ctx: &Context<'_, '_, 'c, 'info, PhotonMsg<'info>>,
    data: Vec<u8>,
) -> Result<()> {
    let selector = u32::from_be_bytes(
        data[..4]
            .try_into()
            .map_err(|_| OmnichainError::InvalidSelector)?,
    );

    // The rest from the selector
    let remainder_data = &data[4..];

    /*
       Functions to implement:
       finishGame(uint256,uint16,bytes32)
       finishGameWithPlaces(uint256 uuid, uint16 feeType, address[] calldata winners, uint16[] calldata prizeFractions)
       startGameOmnichain(uint256 uuid, uint256 wager, address[] calldata participants)
       makeBetOmnichain(address player,uint256[] calldata params)
           params[0] -> tournament uuid
           params[1] -> captain address
           params[2] -> captain index
           params[3] -> amount
       createTournament(address organizer, address sponsor,ITournamentProvider.TournamentParams memory params,address[] memory policies,ITournamentProvider.PaymentApproach paymentApproach)
       register(uint256 uuid, address player, address captain, address[] memory teammates,ITournamentProvider.PaymentApproach)
    */

    // The following is an example
    match selector {
        // This is finishGame(uint256,uint16,bytes32)
        0x6c5c5b93 => bridge_finish_game(ctx, remainder_data.to_vec()),
        // This is finishGameWithPlaces(uint256,uint16,bytes32[],uint16[])
        0xc1be2f7d => bridge_finish_game_with_places(ctx, remainder_data.to_vec()),
        // This is startGameOmnichain(uint256,uint256,bytes32[],bool)
        0xd4e8642b => bridge_start_game_omnichain(ctx, remainder_data.to_vec()),
        // This is registerGameParticipantsOmnichain(uint256,bytes32[],bool)
        0x6e562341 => bridge_register_participants_omnichain(ctx, remainder_data.to_vec()),
        // This is createTournament(bytes32,bytes32,(uint256,uint256,uint256,uint8,uint8,uint8,uint16,bytes32,uint8),bytes32[],uint8)
        0xb09d2c2c => bridge_create_tournament(ctx, remainder_data.to_vec()),
        // This is register(uint256,bytes32,bytes32[],uint8)
        0x75e63f50 => bridge_register(ctx, remainder_data.to_vec()),
        // This is makeBetOmnichain(bytes32,uint256[])
        0x5b693a31 => bridge_make_bet(ctx, remainder_data.to_vec()),
        _ => return Err(OmnichainError::InvalidMethodId.into()),
    }
}

fn bridge_register_participants_omnichain<'info>(
    ctx: &Context<'_, '_, '_, 'info, PhotonMsg<'info>>,
    params: Vec<u8>,
) -> Result<()> {
    let params = ethabi::decode(
        &[
            ParamType::Uint(256),                                  // uint256 uuid
            ParamType::Array(Box::new(ParamType::FixedBytes(32))), //   bytes[] calldata participants
            ParamType::Bool,
        ],
        &params,
    )
        .map_err(|_| OmnichainError::InvalidParams)?;

    // Parse the participants
    let participants = params[1]
        .clone()
        .into_array()
        .expect("Already parsed")
        .into_iter()
        .map(|p| {
            // TODO:
            // Check if this is the correct way of parsing the account
            Pubkey::try_from(p.into_fixed_bytes().expect("Valid address")).expect("Valid Pubkey")
        })
        .collect::<Vec<_>>();

    let start_game = params[2]
        .clone()
        .into_bool()
        .ok_or(OmnichainError::InvalidParams)?;

    // Parse the accounts
    let remaining_accounts = ctx.remaining_accounts;

    let game_account = remaining_accounts[0].clone();
    let game_vault = remaining_accounts[1].clone();

    let register_participants_game_omnichain_accounts = vec![
        ctx.accounts.executor.to_account_info(),
        ctx.accounts.operator_info.to_account_info(),
        ctx.accounts.executor_vault.to_account_info(),
        ctx.accounts.mint.to_account_info(),
        ctx.accounts.system_program.to_account_info(),
        ctx.accounts.token_program.to_account_info(),
        ctx.accounts.associated_token_program.to_account_info(),
        game_account,
        game_vault,
    ];

    // Serialize the data for the CreateGameOmnichain instruction
    let instruction_data = anchor_lang::InstructionData::data(
        &crate::instruction::RegisterGameParticipantsOmnichain {
            participants,
            start_game,
        },
    );

    // Create the instruction
    let ix = Instruction {
        program_id: ctx.program_id.clone(),
        accounts: register_participants_game_omnichain_accounts.to_account_metas(None),
        data: instruction_data,
    };

    // Invoke the instruction
    invoke(&ix, &register_participants_game_omnichain_accounts)?;

    Ok(())
}

fn bridge_make_bet<'info>(ctx: &Context<'_, '_, '_, 'info, PhotonMsg<'info>>, params: Vec<u8>) -> Result<()> {
    let params = ethabi::decode(
        &[
            ParamType::FixedBytes(32),                        // player
            ParamType::Array(Box::new(ParamType::Uint(256))), // [uuid, captain, captainIndex, amount]
        ],
        &params,
    ).map_err(|_| InvalidParams)?;
    let player = params[0]
        .clone()
        .into_fixed_bytes()
        .ok_or(InvalidParams)?;

    let bet_params = params[1]
        .clone()
        .into_array()
        .ok_or(InvalidParams)?;

    let tournament_id = bet_params[0].clone().into_uint().ok_or(InvalidParams)?.as_u64();
    let captain = match bet_params[1].clone() {
        ethabi::Token::Uint(u) => {
            // Convert U256 to a big-endian 32-byte array
            let mut buf = [0u8; 32];
            u.to_big_endian(&mut buf);
            buf
        },
        _ => return Err(Error::from(InvalidParams))
    };

    let gambler = Pubkey::try_from(player).map_err(|_| InvalidParams)?;
    let captain = Pubkey::from(captain);
    let amount = bet_params[3].clone().into_uint().ok_or(InvalidParams)?.as_u64();


    // Parse the accounts
    let make_bet_accounts = vec![
        ctx.accounts.executor.to_account_info(),
        ctx.accounts.executor_vault.to_account_info(),
        ctx.remaining_accounts[0].to_account_info(), // Tournament
        ctx.remaining_accounts[1].to_account_info(), // Tournament book
        ctx.remaining_accounts[2].to_account_info(), // Tournament book vault
        ctx.remaining_accounts[3].to_account_info(), // Captain bet
        ctx.remaining_accounts[4].to_account_info(), // Gambler info
        ctx.accounts.mint.to_account_info(),
        ctx.accounts.zs_config.to_account_info(),
        ctx.accounts.system_program.to_account_info(),
        ctx.accounts.token_program.to_account_info(),
        ctx.accounts.associated_token_program.to_account_info(),
    ];

    // Serialize the data for the FinishGame instruction
    let instruction_data = anchor_lang::InstructionData::data(&crate::instruction::MakeBet {
        gambler,
        captain,
        tournament_id,
        amount,
    });

    // Create the instruction
    let ix = Instruction {
        program_id: ctx.program_id.clone(),
        accounts: make_bet_accounts.to_account_metas(None),
        data: instruction_data,
    };

    // Invoke the instruction
    invoke(&ix, &make_bet_accounts)?;
    Ok(())
}

/// Executes the finish game
fn bridge_finish_game<'info>(
    ctx: &Context<'_, '_, '_, 'info, PhotonMsg<'info>>,
    params: Vec<u8>,
) -> Result<()> {
    let params = ethabi::decode(
        &[
            ParamType::Uint(256),      // uint256 uuid
            ParamType::Uint(16),       // uint16 feeType
            ParamType::FixedBytes(32), // address winner
        ],
        &params,
    )
        .map_err(|_| OmnichainError::InvalidParams)?;

    // Parse the uuid
    // Leaving here in case we need to use in a event or locating accounts
    let _uuid = params[0].clone().into_uint().expect("Already parsed");
    // Parse the feeType
    let fee_type = params[1]
        .clone()
        .into_uint()
        .expect("Already parsed")
        .as_usize() as u16;
    // Parse the winner
    let winner = params[2]
        .clone()
        .into_fixed_bytes()
        .expect("Already parsed");

    // Parse the winner as a account
    let winner_pub_key = Pubkey::try_from(winner).expect("Valid Pubkey");

    // Parse the accounts
    let finish_game_accounts = vec![
        ctx.accounts.executor.to_account_info(),
        ctx.accounts.operator_info.to_account_info(),
        ctx.accounts.zs_config.to_account_info(),
        ctx.accounts.treasury_authority.to_account_info(),
        ctx.remaining_accounts[0].to_account_info(),    // Treasury vault
        ctx.accounts.executor_vault.to_account_info(),  // Platform wallet vault
        ctx.accounts.executor.to_account_info(),
        ctx.remaining_accounts[1].to_account_info(),    // Fee meta
        ctx.remaining_accounts[2].to_account_info(),    // Game
        ctx.remaining_accounts[3].to_account_info(),    // Game vault
        ctx.accounts.mint.to_account_info(),
        ctx.accounts.token_program.to_account_info(),
        ctx.accounts.associated_token_program.to_account_info(),
        ctx.accounts.system_program.to_account_info(),
        ctx.remaining_accounts[4].to_account_info(),
    ];

    // Serialize the data for the FinishGame instruction
    let instruction_data = anchor_lang::InstructionData::data(&crate::instruction::FinishGame {
        fee_type,
        winners: vec![winner_pub_key],
        prize_fractions: vec![BP_DEC],
    });

    // Create the instruction
    let ix = Instruction {
        program_id: ctx.program_id.clone(),
        accounts: finish_game_accounts
            .iter()
            .map(|account| AccountMeta {
                pubkey: account.key(),
                is_signer: account.is_signer,
                is_writable: account.is_writable,
            })
            .collect(),
        data: instruction_data,
    };

    // Invoke the instruction
    invoke(&ix, &finish_game_accounts)?;

    Ok(())
}

/// Executes the finish game with places
fn bridge_finish_game_with_places<'info>(ctx: &Context<'_, '_, '_, 'info, PhotonMsg<'info>>, params: Vec<u8>) -> Result<()> {
    // Decode the params
    // function finishGameWithPlaces(uint256 uuid, uint16 feeType, address[] calldata winners, uint16[] calldata prizeFractions)
    // https://github.com/rather-labs/entangle/blob/master/evm_contracts/core/quickgame/GameProvider.sol#L100
    let params = ethabi::decode(
        &[
            ParamType::Uint(256),                            // uint256 uuid
            ParamType::Uint(16),                             // uint16 feeType
            ParamType::Array(Box::new(ParamType::Address)),  // address[] calldata winners
            ParamType::Array(Box::new(ParamType::Uint(16))), // uint16[] calldata prizeFractions
        ],
        &params,
    )
        .map_err(|_| OmnichainError::InvalidParams)?;

    // Parse the uuid
    // Leaving here in case we need to use in a event or locating accounts
    let _uuid = params[0].clone().into_uint().expect("Already parsed");
    // Parse the feeType
    let fee_type = params[1]
        .clone()
        .into_uint()
        .expect("Already parsed")
        .as_usize() as u16;
    // Parse the winners
    let winners = params[2]
        .clone()
        .into_array()
        .expect("Already parsed")
        .into_iter()
        .map(|w| Pubkey::try_from(w.into_fixed_bytes().expect("Valid address")).expect("Valid Pubkey"))
        .collect::<Vec<_>>();
    // Parse the prizes
    let prize_fractions = params[3]
        .clone()
        .into_array()
        .expect("Already parsed")
        .into_iter()
        .map(|p| p.into_uint().expect("Valid uint").as_u64())
        .collect::<Vec<_>>();

    // Parse the accounts
    // Parse the accounts
    let mut finish_game_with_places_accounts = vec![
        ctx.accounts.executor.to_account_info(),
        ctx.accounts.operator_info.to_account_info(),
        ctx.accounts.zs_config.to_account_info(),
        ctx.accounts.treasury_authority.to_account_info(),
        ctx.remaining_accounts[0].to_account_info(),    // Treasury vault
        ctx.accounts.executor_vault.to_account_info(),  // Platform wallet vault
        ctx.accounts.executor.to_account_info(),
        ctx.remaining_accounts[1].to_account_info(),    // Fee meta
        ctx.remaining_accounts[2].to_account_info(),    // Game
        ctx.remaining_accounts[3].to_account_info(),    // Game vault
        ctx.accounts.mint.to_account_info(),
        ctx.accounts.token_program.to_account_info(),
        ctx.accounts.associated_token_program.to_account_info(),
        ctx.accounts.system_program.to_account_info(),
    ];

    // Every remaining account except the last one is a participant
    for participant in &ctx.remaining_accounts[4..ctx.remaining_accounts.len() - 1] {
        finish_game_with_places_accounts.push(participant.clone());
    }

    // Serialize the data for the FinishGame instruction
    let instruction_data = anchor_lang::InstructionData::data(&crate::instruction::FinishGame {
        fee_type,
        winners,
        prize_fractions,
    });

    // Create the instruction
    let ix = Instruction {
        program_id: ctx.program_id.clone(),
        accounts: finish_game_with_places_accounts
            .iter()
            .map(|account| solana_program::instruction::AccountMeta {
                pubkey: account.key(),
                is_signer: account.is_signer,
                is_writable: account.is_writable,
            })
            .collect(),
        data: instruction_data,
    };

    // Invoke the instruction
    invoke(&ix, &finish_game_with_places_accounts)?;

    Ok(())
}

/// Executes the start game omnichain
fn bridge_start_game_omnichain<'c, 'info>(
    ctx: &Context<'_, '_, 'c, 'info, PhotonMsg<'info>>,
    params: Vec<u8>,
) -> Result<()> {
    // Decode the params
    // TODO: Solidity function has address[] calldata participants instead of bytes[]
    // Solidity address is 20 bytes while Solana is 32
    // Solidity contract will need to be modified to accept bytes[] array instead
    // function startGameOmnichain(uint256 uuid, uint256 wager, bytes[] calldata participants)
    // https://github.com/rather-labs/entangle/blob/master/evm_contracts/core/quickgame/GameProvider.sol#L36
    let params = ethabi::decode(
        &[
            ParamType::Uint(256),                                  // uint256 uuid
            ParamType::Uint(256),                                  // uint256 wager
            ParamType::Array(Box::new(ParamType::FixedBytes(32))), //   bytes[] calldata participants
            ParamType::Bool,
        ],
        &params,
    )
        .map_err(|_| OmnichainError::InvalidParams)?;

    // Parse the UUID
    let game_id = params[0]
        .clone()
        .into_uint()
        .expect("Already parsed")
        .as_u64();

    // Parse the wager
    let wager = params[1]
        .clone()
        .into_uint()
        .expect("Already parsed")
        .as_u64();
    // Parse the participants
    let participants = params[2]
        .clone()
        .into_array()
        .expect("Already parsed")
        .into_iter()
        .map(|p| {
            // TODO:
            // Check if this is the correct way of parsing the account
            Pubkey::try_from(p.into_fixed_bytes().expect("Valid address")).expect("Valid Pubkey")
        })
        .collect::<Vec<_>>();

    let start_game = params[3]
        .clone()
        .into_bool()
        .ok_or(OmnichainError::InvalidParams)?;

    // Parse the accounts
    let remaining_accounts = ctx.remaining_accounts;

    let game_account = remaining_accounts[0].clone();
    let game_vault = remaining_accounts[1].clone();

    let start_game_omnichain_accounts = vec![
        ctx.accounts.executor.to_account_info(),
        ctx.accounts.operator_info.to_account_info(),
        ctx.accounts.executor_vault.to_account_info(),
        ctx.accounts.zs_config.to_account_info(),
        ctx.accounts.mint.to_account_info(),
        game_account,
        game_vault,
        ctx.accounts.system_program.to_account_info(),
        ctx.accounts.token_program.to_account_info(),
        ctx.accounts.associated_token_program.to_account_info(),
    ];

    // Serialize the data for the CreateGameOmnichain instruction
    let instruction_data =
        anchor_lang::InstructionData::data(&crate::instruction::CreateGameOmnichain {
            wager,
            participants,
            game_id,
            start_game,
        });

    // Create the instruction
    let ix = Instruction {
        program_id: ctx.program_id.clone(),
        accounts: start_game_omnichain_accounts
            .iter()
            .map(|account: &AccountInfo| account.to_account_metas(None)[0].clone())
            .collect(),
        data: instruction_data,
    };

    // Invoke the instruction
    invoke(&ix, &start_game_omnichain_accounts)?;

    Ok(())
}

/// Executes the create tournament
fn bridge_create_tournament<'c, 'info>(
    ctx: &Context<'_, '_, 'c, 'info, PhotonMsg<'info>>,
    params: Vec<u8>,
) -> Result<()> {
    // Decode the params
    /*
    function createTournament(
        address organizer,
        address sponsor,
        TournamentParams calldata params,
        address[] calldata policies,
        PaymentApproach paymentApproach
    )
    struct TournamentParams {
        uint256 fee;
        uint256 sponsorPool;
        uint256 startTime;
        uint8 playersInTeam;
        uint8 minTeams;
        uint8 maxTeams;
        uint16 organizerRoyalty;
        address token;
        TournamentType tournamentType;
    }
    */
    // https://github.com/rather-labs/entangle/blob/master/evm_contracts/core/tournament/TournamentProviderV3.sol#L49
    // https://github.com/rather-labs/entangle/blob/master/evm_contracts/core/tournament/ITournamentProviderV2.sol#L69-L79
    let params = ethabi::decode(
        &[
            ParamType::FixedBytes(32), // address organizer
            ParamType::FixedBytes(32), // address sponsor
            ParamType::Tuple(vec![
                // TournamentParams calldata params
                ParamType::Uint(64),       // uint256 fee
                ParamType::Uint(64),       // uint256 sponsorPool
                ParamType::Uint(64),       // uint256 startTime
                ParamType::Uint(8),        // uint8 playersInTeam
                ParamType::Uint(8),        // uint8 minTeams
                ParamType::Uint(8),        // uint8 maxTeams
                ParamType::Uint(16),       // uint16 organizerRoyalty
                ParamType::FixedBytes(32), // address token
                ParamType::Uint(8),        // TournamentType tournamentType
            ]),
            ParamType::Array(Box::new(ParamType::FixedBytes(32))), // address[] calldata policies
            ParamType::Uint(8), // PaymentApproach paymentApproach
        ],
        &params,
    )
        .map_err(|_| OmnichainError::InvalidParams)?;

    // Parse the organizer and the sponsor
    let organizer = Pubkey::try_from(
        params[0]
            .clone()
            .into_fixed_bytes()
            .expect("Already parsed"),
    )
        .expect("Valid address");
    let _sponsor = Pubkey::try_from(
        params[1]
            .clone()
            .into_fixed_bytes()
            .expect("Already parsed"),
    )
        .expect("Valid address");

    // Parse the TournamentParams
    let tournament_params = params[2].clone().into_tuple().expect("Already parsed");

    // Parse the tournament_params_fee, tournament_params_sponsor_pool and tournament_params_start_time
    let tournament_params_fee = tournament_params[0]
        .clone()
        .into_uint()
        .expect("Already parsed")
        .as_u64();
    let tournament_params_sponsor_pool = tournament_params[1]
        .clone()
        .into_uint()
        .expect("Already parsed")
        .as_u64();
    let tournament_params_start_time = tournament_params[2]
        .clone()
        .into_uint()
        .expect("Already parsed")
        .as_u64();

    // Parse the tournament_params_players_in_team, tournament_params_min_team, tournament_params_max_team
    let tournament_params_players_in_team = tournament_params[3]
        .clone()
        .into_uint()
        .expect("Already parsed")
        .as_u64() as u8;
    let tournament_params_min_team = tournament_params[4]
        .clone()
        .into_uint()
        .expect("Already parsed")
        .as_u64() as u8;
    let tournament_params_max_team = tournament_params[5]
        .clone()
        .into_uint()
        .expect("Already parsed")
        .as_u64() as u8;

    // Parse the tournament_params_organizer_royalty
    let tournament_params_organizer_royalty = tournament_params[6]
        .clone()
        .into_uint()
        .expect("Already parsed")
        .as_u64() as u16;

    // Parse the tournament_params_token
    let tournament_params_token = Pubkey::try_from(
        tournament_params[7]
            .clone()
            .into_fixed_bytes()
            .expect("Already parsed"),
    )
        .expect("Valid address");

    // Parse the tournament_params_tournament_type
    let _tournament_params_tournament_type = tournament_params[8]
        .clone()
        .into_uint()
        .expect("Already parsed")
        .as_u64() as u8;

    // Parse the policies
    let _policies = params[3]
        .clone()
        .into_array()
        .expect("Already parsed")
        .into_iter()
        .map(|w| {
            // TODO:
            // Check the parsing for accounts
            Pubkey::try_from(w.into_fixed_bytes().expect("Valid address")).expect("Valid Pubkey")
        })
        .collect::<Vec<_>>();

    // Parse the payment_approach
    let _payment_approach = params[4]
        .clone()
        .into_uint()
        .expect("Already parsed")
        .as_u64() as u8;

    // Parse the accounts
    let create_tournament_accounts = vec![
        ctx.accounts.executor.to_account_info(),
        ctx.accounts.operator_info.to_account_info(),
        ctx.accounts.zs_config.to_account_info(),
        ctx.remaining_accounts[0].to_account_info(),
        ctx.remaining_accounts[1].to_account_info(),
        ctx.accounts.system_program.to_account_info(),
    ];

    // Get the tournament params
    let tournament_params_solana = TournamentParams {
        fee: tournament_params_fee as u64,
        sponsor_pool: tournament_params_sponsor_pool,
        start_time: tournament_params_start_time,
        players_in_team: tournament_params_players_in_team,
        min_teams: tournament_params_min_team,
        max_teams: tournament_params_max_team,
        organizer_royalty: tournament_params_organizer_royalty,
        token: tournament_params_token,
    };

    // Serialize the data for the Create Tournament instruction
    let instruction_data =
        anchor_lang::InstructionData::data(&crate::instruction::CreateTournamentOmnichain {
            organizer,
            params: tournament_params_solana,
        });

    // Create the instruction
    let ix = Instruction {
        program_id: ctx.program_id.clone(),
        accounts: create_tournament_accounts
            .iter()
            .map(|account| solana_program::instruction::AccountMeta {
                pubkey: account.key(),
                is_signer: account.is_signer,
                is_writable: account.is_writable,
            })
            .collect(),
        data: instruction_data,
    };

    // Invoke the instruction
    invoke(&ix, &create_tournament_accounts)?;

    Ok(())
}

/// Executes the register
fn bridge_register<'info>(
    ctx: &Context<'_, '_, '_, 'info, PhotonMsg<'info>>,
    params: Vec<u8>,
) -> Result<()> {
    // Decode the params
    // register(uint256 uuid, bytes32 player, bytes32 captain, bytes32[] memory teammates,ITournamentProvider.PaymentApproach)
    // https://github.com/rather-labs/entangle/blob/master/evm_contracts/core/tournament/ITournamentProviderV2.sol#L109
    let params = ethabi::decode(
        &[
            ParamType::Uint(256),                                  // uint256 uuid
            ParamType::FixedBytes(32),                             // bytes32 captain
            ParamType::Array(Box::new(ParamType::FixedBytes(32))), // bytes32[] memory teammates
            ParamType::Uint(8), // PaymentApproach paymentApproach
        ],
        &params,
    )
        .map_err(|_| OmnichainError::InvalidParams)?;

    // Parse the teammates
    let teammates = params[2]
        .clone()
        .into_array()
        .expect("Already parsed")
        .into_iter()
        .map(|w| {
            // TODO:
            // Check the parsing for accounts
            Pubkey::try_from(w.into_fixed_bytes().expect("Valid address")).expect("Valid Pubkey")
        })
        .collect::<Vec<_>>();


    // Parse the accounts
    let mut register_tournament_accounts = vec![
        ctx.accounts.executor.to_account_info(),
        ctx.accounts.operator_info.to_account_info(),
        ctx.remaining_accounts[0].to_account_info(),
        ctx.remaining_accounts[1].to_account_info(),
        ctx.remaining_accounts[2].to_account_info(),
        ctx.remaining_accounts[3].to_account_info(),
        ctx.accounts.system_program.to_account_info(),
    ];

    for acc in &ctx.remaining_accounts[4..ctx.remaining_accounts.len() - 1] {
        register_tournament_accounts.push(acc.clone());
    }

    // Serialize the data for the Create Tournament instruction
    let instruction_data =
        anchor_lang::InstructionData::data(&crate::instruction::RegisterTournamentOmnichain {
            teammates,
        });

    // Create the instruction
    let ix = Instruction {
        program_id: ctx.program_id.clone(),
        accounts: register_tournament_accounts.to_account_metas(None),
        data: instruction_data,
    };

    // Invoke the instruction
    invoke(&ix, &register_tournament_accounts)?;

    Ok(())
}

/// This is a successful call event
/// The solidity counterpart can be found at:
/// https://github.com/rather-labs/entangle/blob/master/evm_contracts/omnichain/omni-messenger/common/MessagingEvents.sol#L16
#[event]
struct SuccessfulCall {
    src_chain_id: u64,
    provider: Vec<u8>,
    target: Vec<u8>,
    data: Vec<u8>,
}
