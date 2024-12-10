use anchor_lang::prelude::*;
use anchor_spl::{
    associated_token::AssociatedToken,
    token_2022::TransferChecked,
    token_interface::{transfer_checked, Mint, TokenAccount, TokenInterface},
};

use crate::{
    error::{ControlAccessError, GameError},
    validate_game_participant_vault_token, Game, GameStatus, GameType, OperatorInfo, Role, ZS_ROOT,
};

#[derive(Accounts)]
pub struct RegisterGameParticipantsSinglechain<'info> {
    #[account(mut)]
    pub operator: Signer<'info>,

    #[account(seeds = [ZS_ROOT, b"OPERATOR", operator.key().as_ref()], bump, constraint = operator_info.approved @ ControlAccessError::OperatorNotApproved, constraint = operator_info.role == Role::BACKEND @ ControlAccessError::OperatorNotBackend)]
    pub operator_info: Box<Account<'info, OperatorInfo>>,

    pub mint: Box<InterfaceAccount<'info, Mint>>,

    #[account(mut, seeds = [ZS_ROOT, b"GAME", &game.id.to_le_bytes().as_ref()], bump)]
    pub game: Box<Account<'info, Game>>,

    #[account(init_if_needed, payer = operator, associated_token::mint = mint, associated_token::authority = game, associated_token::token_program = token_program)]
    pub game_vault: Box<InterfaceAccount<'info, TokenAccount>>,

    pub system_program: Program<'info, System>,
    pub token_program: Interface<'info, TokenInterface>,
    pub associated_token_program: Program<'info, AssociatedToken>,
}

pub fn handle_register_game_participants_singlechain<'info>(
    ctx: Context<'_, '_, '_, 'info, RegisterGameParticipantsSinglechain<'info>>,
    start_game: bool,
) -> Result<()> {
    // Validate the game is created
    require!(
        ctx.accounts.game.status == GameStatus::Created,
        GameError::InvalidGameStatus
    );

    require!(
        ctx.accounts.game.game_type == GameType::SINGLECHAIN,
        GameError::InvalidGameType
    );

    // Validate the participants length
    let remaining_accounts = ctx.remaining_accounts;

    require!(remaining_accounts.len() >= 1, GameError::TooFewParticipants);

    require!(
        remaining_accounts.len() + ctx.accounts.game.participants.len() <= 32,
        GameError::TooManyParticipants
    );

    for participant_vault in remaining_accounts.iter() {
        let pubkey = {
            let data = participant_vault.try_borrow_data()?;
            Pubkey::try_from_slice(&data[32..64])?
        };

        require!(pubkey != Pubkey::default(), GameError::InvalidParticipant);

        require!(
            !ctx.accounts.game.participants.contains(&pubkey),
            GameError::ParticipantAlreadyRegistered
        );

        validate_game_participant_vault_token(
            participant_vault,
            ctx.accounts.mint.to_account_info(),
        )?;

        let accounts = TransferChecked {
            from: participant_vault.to_account_info(),
            mint: ctx.accounts.mint.to_account_info(),
            to: ctx.accounts.game_vault.to_account_info(),
            authority: ctx.accounts.operator.to_account_info(),
        };
        let cpi = CpiContext::new(ctx.accounts.token_program.to_account_info(), accounts);
        transfer_checked(cpi, ctx.accounts.game.wager, ctx.accounts.mint.decimals)?;

        // Add the participant to the game
        ctx.accounts.game.participants.push(pubkey);

        emit!(RegisterGameParticipantsSinglechainEvent {
            game_id: ctx.accounts.game.id,
            participant: pubkey,
        });
    }

    if ctx.accounts.game.participants.len() == 32 || start_game {
        ctx.accounts.game.status = GameStatus::Started;
    }

    Ok(())
}

#[derive(Debug)]
#[event]
pub struct RegisterGameParticipantsSinglechainEvent {
    pub game_id: u64,
    pub participant: Pubkey,
}
