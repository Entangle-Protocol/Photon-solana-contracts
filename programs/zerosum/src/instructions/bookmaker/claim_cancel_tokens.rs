use std::vec;

use anchor_lang::prelude::*;

use anchor_spl::{
    associated_token::AssociatedToken,
    token_2022::TransferChecked,
    token_interface::{transfer_checked, Mint, TokenAccount, TokenInterface},
};

use crate::{
    error::BookmakerError, CaptainBet, GamblerInfo, Tournament, TournamentBook, TournamentStatus,
    ZeroSumConfig, ZS_ROOT,
};

#[derive(Accounts)]
#[instruction(gambler: Pubkey, captain: Pubkey, tournament_id: u64, fee_type: u16)]
pub struct ClaimCancelTokens<'info> {
    #[account(signer, mut)]
    pub gambler: Signer<'info>,

    #[account(mut, associated_token::mint = mint, associated_token::authority = gambler, associated_token::token_program = token_program)]
    pub gambler_vault: Box<InterfaceAccount<'info, TokenAccount>>,

    #[account(mut, seeds = [ZS_ROOT, b"GAMBLER", &tournament_id.to_le_bytes().as_ref(), captain.key().as_ref(), gambler.key().as_ref()], bump, constraint = gambler_info.has_claimed_cancel == false @ BookmakerError::FinishAlreadyClaimed)]
    pub gambler_info: Box<Account<'info, GamblerInfo>>,

    #[account(mut, seeds = [ZS_ROOT, b"TOURNAMENT", &tournament_id.to_le_bytes().as_ref()], bump, constraint = tournament.status == TournamentStatus::Canceled @ BookmakerError::InvalidTournamentStatus)]
    pub tournament: Box<Account<'info, Tournament>>,

    #[account(mut, seeds = [ZS_ROOT, b"BOOK", &tournament_id.to_le_bytes().as_ref()], bump)]
    pub tournament_book: Box<Account<'info, TournamentBook>>,

    #[account(mut, associated_token::mint = mint, associated_token::authority = tournament_book, associated_token::token_program = token_program)]
    pub tournament_book_vault: Box<InterfaceAccount<'info, TokenAccount>>,

    #[account(mut, seeds = [ZS_ROOT, b"CAPTAIN_BET", &tournament_id.to_le_bytes().as_ref(), captain.key().as_ref()], bump)]
    pub captain_bet: Box<Account<'info, CaptainBet>>,

    pub mint: Box<InterfaceAccount<'info, Mint>>,

    #[account(mut, seeds = [ZS_ROOT, b"CONFIG"], bump)]
    pub config: Box<Account<'info, ZeroSumConfig>>,

    pub system_program: Program<'info, System>,
    pub token_program: Interface<'info, TokenInterface>,
    pub associated_token_program: Program<'info, AssociatedToken>,
}

pub fn handle_claim_cancel_tokens(
    ctx: Context<ClaimCancelTokens>,
    gambler: Pubkey,
    captain: Pubkey,
    tournament_id: u64,
) -> Result<()> {
    let claimable_amount = ctx.accounts.gambler_info.bet;

    // Take tokens from the tournament book vault to the gambler vault
    let bump = &[ctx.bumps.tournament_book];
    let binding = tournament_id.to_le_bytes();
    let seed = &[ZS_ROOT, b"BOOK", &binding, bump][..];
    let seeds = &[seed];
    let accounts = TransferChecked {
        from: ctx.accounts.tournament_book_vault.to_account_info(),
        mint: ctx.accounts.mint.to_account_info(),
        to: ctx.accounts.gambler_vault.to_account_info(),
        authority: ctx.accounts.tournament_book.to_account_info(),
    };
    let cpi = CpiContext::new_with_signer(
        ctx.accounts.token_program.to_account_info(),
        accounts,
        seeds,
    );
    transfer_checked(cpi, claimable_amount, ctx.accounts.mint.decimals)?;

    // Update the gambler info
    ctx.accounts.gambler_info.has_claimed_cancel = true;

    // Emit the event
    emit!(CancelClaimedEvent {
        tournament_id,
        captain,
        gambler,
        amount: claimable_amount,
    });

    Ok(())
}

#[derive(Debug)]
#[event]
pub struct CancelClaimedEvent {
    pub tournament_id: u64,
    pub captain: Pubkey,
    pub gambler: Pubkey,
    pub amount: u64,
}
