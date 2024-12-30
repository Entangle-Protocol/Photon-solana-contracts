use anchor_lang::prelude::*;

use anchor_spl::token_interface::{Mint, TokenAccount, TokenInterface};

use crate::{
    error::{ControlAccessError, GameError},
    init_user_info, send_game_remaining_tokens_to_treasury, update_claimable_amount, Game,
    GameStatus, OperatorInfo, Role, GENOME_ROOT,
};

#[derive(Accounts)]
pub struct CancelGame<'info> {
    #[account(mut)]
    pub operator: Signer<'info>,

    #[account(seeds = [GENOME_ROOT, b"OPERATOR", operator.key().as_ref()], bump, constraint = operator_info.approved @ ControlAccessError::OperatorNotApproved, constraint = operator_info.role == Role::BACKEND @ ControlAccessError::OperatorNotBackend)]
    pub operator_info: Box<Account<'info, OperatorInfo>>,

    //------------------- TREASURY ACCOUNTS -------------------
    /// CHECK: treasury authority
    #[account(seeds = [GENOME_ROOT, b"AUTHORITY"], bump)]
    pub treasury_authority: AccountInfo<'info>,

    #[account(mut, associated_token::mint = mint, associated_token::authority = treasury_authority, associated_token::token_program = token_program)]
    pub treasury_vault: Box<InterfaceAccount<'info, TokenAccount>>,

    #[account(mut, seeds = [GENOME_ROOT, b"GAME", &game.id.to_le_bytes().as_ref()], bump)]
    pub game: Box<Account<'info, Game>>,

    #[account(mut, associated_token::mint = mint, associated_token::authority = game, associated_token::token_program = token_program)]
    pub game_vault: Box<InterfaceAccount<'info, TokenAccount>>,

    pub mint: Box<InterfaceAccount<'info, Mint>>,

    pub system_program: Program<'info, System>,
    pub token_program: Interface<'info, TokenInterface>,
}

pub fn handle_cancel_game<'info>(ctx: Context<'_, '_, '_, 'info, CancelGame<'info>>) -> Result<()> {
    require!(
        ctx.accounts.game.status == GameStatus::Started
            || ctx.accounts.game.status == GameStatus::PreCanceled,
        GameError::InvalidGameStatus
    );

    let mut refunded_participants = ctx.accounts.game.refunded_participants.clone();
    let game = &mut ctx.accounts.game;
    let wager = game.wager;

    // Increment the participant's pending to claim amount
    for participant in game.participants.iter_mut() {
        // If the participant has already been refunded, skip it
        if refunded_participants.contains(participant) {
            continue;
        }

        let (expected_pda, bump) = Pubkey::find_program_address(
            &[GENOME_ROOT, b"USER", participant.as_ref()],
            &ctx.program_id,
        );

        // Search for the expected PDA account:
        // If the account is not found, skip the participant
        let pda_account = if let Some(account) = ctx
            .remaining_accounts
            .iter()
            .find(|account| account.key() == expected_pda)
        {
            account
        } else {
            continue;
        };

        if pda_account.to_account_info().data_is_empty() {
            init_user_info(
                *participant,
                pda_account,
                bump,
                *ctx.program_id,
                ctx.accounts.operator.to_account_info(),
                ctx.accounts.system_program.to_account_info(),
            )?;
        }

        // Update the participants's claimable amount
        update_claimable_amount(*participant, wager, pda_account.clone())?;

        // Add the participant to the refunded participants list
        refunded_participants.push(*participant);
    }

    game.refunded_participants = refunded_participants;

    // The remaining accounts passed could be less than the participants
    // due to the bytes limit on the instruction
    match game.status {
        GameStatus::Started if game.refunded_participants.len() < game.participants.len() => {
            // Not all the participants have been set to refund yet
            game.status = GameStatus::PreCanceled;
            emit!(GamePreCanceledEvent { game_id: game.id });
        }
        GameStatus::PreCanceled if game.refunded_participants.len() < game.participants.len() => {
            // The game was pre-canceled but not all the participants have been set to refund yet
            return Ok(());
        }
        _ if game.refunded_participants.len() == game.participants.len() => {
            // All the participants have been set to refund
            game.status = GameStatus::Canceled;
            emit!(GameCanceledEvent { game_id: game.id });
            send_game_remaining_tokens_to_treasury(
                ctx.accounts.game_vault.amount,
                ctx.accounts.mint.to_account_info(),
                ctx.accounts.mint.decimals,
                ctx.accounts.treasury_vault.to_account_info(),
                ctx.accounts.game_vault.to_account_info(),
                ctx.accounts.game.to_account_info(),
                ctx.accounts.game.id,
                ctx.bumps.game,
                ctx.accounts.token_program.to_account_info(),
            )?;
        }
        _ => {}
    }

    Ok(())
}

#[derive(Debug)]
#[event]
pub struct GameCanceledEvent {
    pub game_id: u64,
}

#[derive(Debug)]
#[event]
pub struct GamePreCanceledEvent {
    pub game_id: u64,
}
