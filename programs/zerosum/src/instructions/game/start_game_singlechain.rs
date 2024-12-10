use anchor_lang::prelude::*;
use anchor_spl::{
    associated_token::AssociatedToken,
    token_interface::{Mint, TokenAccount, TokenInterface},
};

use crate::{
    error::{ControlAccessError, GameError},
    Game, GameStatus, GameType, OperatorInfo, Role, ZS_ROOT,
};

#[derive(Accounts)]
pub struct StartGameSinglechain<'info> {
    #[account(mut)]
    pub operator: Signer<'info>,

    #[account(seeds = [ZS_ROOT, b"OPERATOR", operator.key().as_ref()], bump, constraint = operator_info.approved @ ControlAccessError::OperatorNotApproved, constraint = operator_info.role == Role::BACKEND @ ControlAccessError::OperatorNotBackend)]
    pub operator_info: Box<Account<'info, OperatorInfo>>,

    pub mint: Box<InterfaceAccount<'info, Mint>>,

    #[account(mut ,seeds = [ZS_ROOT, b"GAME", &game.id.to_le_bytes().as_ref()], bump)]
    pub game: Box<Account<'info, Game>>,

    #[account(init_if_needed, payer = operator, associated_token::mint = mint, associated_token::authority = game, associated_token::token_program = token_program)]
    pub game_vault: Box<InterfaceAccount<'info, TokenAccount>>,

    pub system_program: Program<'info, System>,
    pub token_program: Interface<'info, TokenInterface>,
    pub associated_token_program: Program<'info, AssociatedToken>,
}

pub fn handle_start_game_singlechain(ctx: Context<StartGameSinglechain>) -> Result<()> {
    // Validate the game is created
    require!(
        ctx.accounts.game.status == GameStatus::Created,
        GameError::QuickGameAlreadyExists
    );

    require!(
        ctx.accounts.game.game_type == GameType::SINGLECHAIN,
        GameError::InvalidGameType
    );

    require!(
        ctx.accounts.game.participants.len() > 1,
        GameError::TooFewParticipants
    );

    require!(
        ctx.accounts.game.participants.len() <= 32,
        GameError::TooManyParticipants
    );

    ctx.accounts.game.status = GameStatus::Started;

    emit!(StartGameSinglechainEvent {
        game_id: ctx.accounts.game.id,
    });

    Ok(())
}

#[derive(Debug)]
#[event]
pub struct StartGameSinglechainEvent {
    pub game_id: u64,
}
