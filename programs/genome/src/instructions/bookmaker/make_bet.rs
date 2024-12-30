use std::vec;

use anchor_lang::prelude::*;

use anchor_spl::{
    associated_token::AssociatedToken,
    token_2022::TransferChecked,
    token_interface::{transfer_checked, Mint, TokenAccount, TokenInterface},
};

use crate::{
    error::BookmakerError, CaptainBet, GamblerInfo, Tournament, TournamentBook, TournamentStatus,
    GenomeConfig, GENOME_ROOT,
};

#[derive(Accounts)]
#[instruction(gambler: Pubkey, captain: Pubkey, tournament_id: u64)]
pub struct MakeBet<'info> {
    #[account(signer, mut)]
    pub payer: Signer<'info>,

    #[account(mut, associated_token::mint = mint, associated_token::authority = payer, associated_token::token_program = token_program)]
    pub payer_vault: Box<InterfaceAccount<'info, TokenAccount>>,

    #[account(mut, seeds = [GENOME_ROOT, b"TOURNAMENT", &tournament_id.to_le_bytes().as_ref()], bump)]
    pub tournament: Box<Account<'info, Tournament>>,

    #[account(init_if_needed, payer = payer, space = TournamentBook::LEN, seeds = [GENOME_ROOT, b"BOOK", &tournament_id.to_le_bytes().as_ref()], bump)]
    pub tournament_book: Box<Account<'info, TournamentBook>>,

    #[account(init_if_needed, payer = payer, associated_token::mint = mint, associated_token::authority = tournament_book, associated_token::token_program = token_program)]
    pub tournament_book_vault: Box<InterfaceAccount<'info, TokenAccount>>,

    #[account(init_if_needed, payer = payer, space = CaptainBet::LEN, seeds = [GENOME_ROOT, b"CAPTAIN_BET", &tournament_id.to_le_bytes().as_ref(), captain.key().as_ref()], bump)]
    pub captain_bet: Box<Account<'info, CaptainBet>>,

    #[account(init_if_needed, payer = payer, space = GamblerInfo::LEN, seeds = [GENOME_ROOT, b"GAMBLER", &tournament_id.to_le_bytes().as_ref(), captain.key().as_ref(), gambler.key().as_ref()], bump)]
    pub gambler_info: Box<Account<'info, GamblerInfo>>,

    pub mint: Box<InterfaceAccount<'info, Mint>>,

    #[account(mut, seeds = [GENOME_ROOT, b"CONFIG"], bump)]
    pub config: Box<Account<'info, GenomeConfig>>,

    pub system_program: Program<'info, System>,
    pub token_program: Interface<'info, TokenInterface>,
    pub associated_token_program: Program<'info, AssociatedToken>,
}

pub fn handle_make_bet(
    ctx: Context<MakeBet>,
    gambler: Pubkey,
    captain: Pubkey,
    tournament_id: u64,
    amount: u64,
) -> Result<()> {

    // Check if the tournament is in the right status
    require!(
        can_bet_for_tournament(
            ctx.accounts.tournament.status.clone(),
            ctx.accounts.tournament.captains.len()
        ),
        BookmakerError::CannotBetForTournament
    );

    require!(
        ctx.accounts.tournament.captains.contains(&captain),
        BookmakerError::InvalidCaptain
    );

    // Check the amount of the bet
    let config = &ctx.accounts.config.bookmaker_config;
    require!(
        amount >= config.minimal_bet,
        BookmakerError::BetAmountTooLow
    );

    // The gambler must be a valid address
    require!(gambler != Pubkey::default(), BookmakerError::InvalidGambler);

    // Take tokens from the fee payer to the book vault
    // The fee payer can be also the gambler
    let accounts = TransferChecked {
        from: ctx.accounts.payer_vault.to_account_info(),
        mint: ctx.accounts.mint.to_account_info(),
        to: ctx.accounts.tournament_book_vault.to_account_info(),
        authority: ctx.accounts.payer.to_account_info(),
    };
    let cpi = CpiContext::new(ctx.accounts.token_program.to_account_info(), accounts);
    transfer_checked(cpi, amount, ctx.accounts.mint.decimals)?;

    // Update the book
    ctx.accounts.tournament_book.total_sum += amount;
    ctx.accounts.captain_bet.sum += amount;
    ctx.accounts.gambler_info.bet += amount;

    // Emit the event
    emit!(BetMadeEvent {
        tournament_id,
        captain,
        gambler,
        amount,
    });

    Ok(())
}

pub fn can_bet_for_tournament(tournament_status: TournamentStatus, filled_teams: usize) -> bool {
    // Check if the tournament is in the right status
    if tournament_status != TournamentStatus::Registration
        && tournament_status != TournamentStatus::Filled
    {
        return false;
    }
    if filled_teams < 2 {
        return false;
    }
    return true;
}

#[derive(Debug)]
#[event]
pub struct BetMadeEvent {
    pub tournament_id: u64,
    pub captain: Pubkey,
    pub gambler: Pubkey,
    pub amount: u64,
}
