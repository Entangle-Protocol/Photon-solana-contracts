use anchor_lang::prelude::*;
use anchor_spl::{
    associated_token::AssociatedToken,
    token_2022::TransferChecked,
    token_interface::{transfer_checked, Mint, TokenAccount, TokenInterface},
};
use std::collections::HashSet;

use crate::{
    error::{ControlAccessError, GameError},
    validate_game_participant_vault_token, Game, GameStatus, GameType, OperatorInfo, Role,
    GenomeConfig, GENOME_ROOT,
};

#[derive(Accounts)]
pub struct CreateGameSinglechain<'info> {
    #[account(mut)]
    pub operator: Signer<'info>,

    #[account(seeds = [GENOME_ROOT, b"OPERATOR", operator.key().as_ref()], bump, constraint = operator_info.approved @ ControlAccessError::OperatorNotApproved, constraint = operator_info.role == Role::BACKEND @ ControlAccessError::OperatorNotBackend)]
    pub operator_info: Box<Account<'info, OperatorInfo>>,

    #[account(mut, seeds = [GENOME_ROOT, b"CONFIG"], bump)]
    pub config: Box<Account<'info, GenomeConfig>>,

    pub mint: Box<InterfaceAccount<'info, Mint>>,

    // the game PDA is derived by using a counter, to guarantee uniqueness
    #[account(init, payer = operator, space = Game::LEN, seeds = [GENOME_ROOT, b"GAME", &config.games_config.games_counter.to_le_bytes().as_ref()], bump)]
    pub game: Box<Account<'info, Game>>,

    #[account(init_if_needed, payer = operator, associated_token::mint = mint, associated_token::authority = game, associated_token::token_program = token_program)]
    pub game_vault: Box<InterfaceAccount<'info, TokenAccount>>,

    pub system_program: Program<'info, System>,
    pub token_program: Interface<'info, TokenInterface>,
    pub associated_token_program: Program<'info, AssociatedToken>,
}

pub fn handle_create_game_singlechain<'info>(
    ctx: Context<'_, '_, '_, 'info, CreateGameSinglechain<'info>>,
    wager: u64,
) -> Result<()> {
    let config = &mut ctx.accounts.config;

    // Validate the game does not exist yet
    require!(
        ctx.accounts.game.status == GameStatus::NotExists
            && ctx.accounts.game.participants.len() == 0,
        GameError::InvalidGameStatus
    );

    // game wager must be greater than the minimal wager on config
    require!(
        wager >= config.games_config.minimal_wager,
        GameError::WagerTooSmall
    );

    // Validate the participants length
    let remaining_accounts = ctx.remaining_accounts;

    require!(remaining_accounts.len() >= 1, GameError::TooFewParticipants);

    require!(
        remaining_accounts.len() <= 32,
        GameError::TooManyParticipants
    );

    let mut participants_set = HashSet::with_capacity(remaining_accounts.len());

    for participant_vault in remaining_accounts.iter() {
        let pubkey = {
            let data = participant_vault.try_borrow_data()?;
            Pubkey::try_from_slice(&data[32..64])?
        };

        require!(pubkey != Pubkey::default(), GameError::InvalidParticipant);

        require!(
            !participants_set.contains(&pubkey),
            GameError::ParticipantDuplicated
        );

        // Validate the token mint of the participant vault
        //validate_participant_vault_token(participant_vault, ctx.accounts.mint.to_account_info())?;
        validate_game_participant_vault_token(
            participant_vault,
            ctx.accounts.mint.to_account_info(),
        )?;

        participants_set.insert(pubkey);

        let accounts = TransferChecked {
            from: participant_vault.to_account_info(),
            mint: ctx.accounts.mint.to_account_info(),
            to: ctx.accounts.game_vault.to_account_info(),
            authority: ctx.accounts.operator.to_account_info(),
        };
        let cpi = CpiContext::new(ctx.accounts.token_program.to_account_info(), accounts);
        transfer_checked(cpi, wager, ctx.accounts.mint.decimals)?;
    }

    let game = &mut ctx.accounts.game;
    match game.create_game(
        config.games_config.games_counter,
        GameType::SINGLECHAIN,
        wager,
        participants_set.into_iter().collect(),
        GameStatus::Created,
    ) {
        Ok(_val) => {}
        Err(err) => {
            panic!("Error: {}", err);
        }
    }

    // Increment the games counter
    config.games_config.games_counter += 1;

    emit!(GameSinglechainCreatedEvent {
        game_id: game.id,
        participants: game.participants.clone(),
        wager: game.wager,
    });

    Ok(())
}

#[derive(Debug)]
#[event]
pub struct GameSinglechainCreatedEvent {
    pub game_id: u64,
    pub participants: Vec<Pubkey>,
    pub wager: u64,
}
