use anchor_lang::prelude::*;
use anchor_spl::{
    associated_token::AssociatedToken,
    token_2022::TransferChecked,
    token_interface::{transfer_checked, Mint, TokenAccount, TokenInterface},
};

use crate::{
    are_all_participants_unique,
    error::{ControlAccessError, GameError},
    Game, GameStatus, GameType, OperatorInfo, Role, ZS_ROOT,
};

#[derive(Accounts)]
pub struct RegisterGameParticipantsOmnichain<'info> {
    #[account(mut)]
    pub operator: Signer<'info>,

    #[account(seeds = [ZS_ROOT, b"OPERATOR", operator.key().as_ref()], bump, constraint = operator_info.approved @ ControlAccessError::OperatorNotApproved, constraint = (operator_info.role == Role::BACKEND || operator_info.role == Role::MESSENGER) @ ControlAccessError::OperatorNotBackend)]
    pub operator_info: Box<Account<'info, OperatorInfo>>,

    #[account(mut, associated_token::mint = mint, associated_token::authority = operator, associated_token::token_program = token_program)]
    pub operator_vault: Box<InterfaceAccount<'info, TokenAccount>>,

    pub mint: Box<InterfaceAccount<'info, Mint>>,

    pub system_program: Program<'info, System>,
    pub token_program: Interface<'info, TokenInterface>,
    pub associated_token_program: Program<'info, AssociatedToken>,
    #[account(mut, seeds = [ZS_ROOT, b"GAME", &game.id.to_le_bytes().as_ref()], bump)]
    pub game: Box<Account<'info, Game>>,

    #[account(init_if_needed, payer = operator, associated_token::mint = mint, associated_token::authority = game, associated_token::token_program = token_program)]
    pub game_vault: Box<InterfaceAccount<'info, TokenAccount>>,
}

pub fn handle_register_game_participants_omnichain(
    ctx: Context<RegisterGameParticipantsOmnichain>,
    participants: Vec<Pubkey>,
    start_game: bool,
) -> Result<()> {
    // Validate the game is created
    require!(
        ctx.accounts.game.status == GameStatus::Created,
        GameError::InvalidGameStatus
    );

    require!(
        ctx.accounts.game.game_type == GameType::OMNICHAIN,
        GameError::InvalidGameType
    );

    // Validate the participants length
    require!(participants.len() >= 1, GameError::TooFewParticipants);

    require!(
        participants.len() + ctx.accounts.game.participants.len() <= 32,
        GameError::TooManyParticipants
    );

    // Validate the participants are not the default key and are unique, and not already registered
    for participant in participants.iter() {
        require!(
            participant != &Pubkey::default(),
            GameError::InvalidParticipant
        );
        require!(
            !ctx.accounts.game.participants.contains(participant),
            GameError::ParticipantAlreadyRegistered
        );
        require!(
            are_all_participants_unique(participants.clone()),
            GameError::ParticipantDuplicated
        );

        // Add the participant to the game
        ctx.accounts.game.participants.push(*participant);

        emit!(RegisterGameParticipantsOmnichainEvent {
            game_id: ctx.accounts.game.id,
            participant: *participant,
        });
    }

    let total_wager = ctx.accounts.game.wager * participants.len() as u64;

    // The operator will transfer the wager to the treasury
    let accounts = TransferChecked {
        from: ctx.accounts.operator_vault.to_account_info(),
        mint: ctx.accounts.mint.to_account_info(),
        to: ctx.accounts.game_vault.to_account_info(),
        authority: ctx.accounts.operator.to_account_info(),
    };
    let cpi = CpiContext::new(ctx.accounts.token_program.to_account_info(), accounts);
    transfer_checked(cpi, total_wager, ctx.accounts.mint.decimals)?;

    if ctx.accounts.game.participants.len() == 32 || start_game {
        ctx.accounts.game.status = GameStatus::Started;
    }

    Ok(())
}

#[derive(Debug)]
#[event]
pub struct RegisterGameParticipantsOmnichainEvent {
    pub game_id: u64,
    pub participant: Pubkey,
}
