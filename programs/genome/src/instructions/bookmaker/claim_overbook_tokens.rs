use std::vec;

use anchor_lang::prelude::*;

use anchor_spl::{
    token_2022::TransferChecked,
    token_interface::{transfer_checked, Mint, TokenAccount, TokenInterface},
};

use crate::{
    error::BookmakerError, CaptainBet, GamblerInfo, Tournament, TournamentBook, TournamentStatus,
    GenomeConfig, BP_DEC, GENOME_ROOT,
};

#[derive(Accounts)]
#[instruction(gambler: Pubkey, captain: Pubkey, tournament_id: u64)]
pub struct ClaimOverbookTokens<'info> {
    #[account(signer, mut)]
    pub gambler: Signer<'info>,

    #[account(mut, associated_token::mint = mint, associated_token::authority = gambler, associated_token::token_program = token_program)]
    pub gambler_vault: Box<InterfaceAccount<'info, TokenAccount>>,

    #[account(mut, seeds = [GENOME_ROOT, b"GAMBLER", &tournament_id.to_le_bytes().as_ref(), captain.key().as_ref(), gambler.key().as_ref()], bump)]
    pub gambler_info: Box<Account<'info, GamblerInfo>>,

    #[account(mut, seeds = [GENOME_ROOT, b"TOURNAMENT", &tournament_id.to_le_bytes().as_ref()], bump, constraint = tournament.status == TournamentStatus::Started || tournament.status == TournamentStatus::PreCancel || tournament.status == TournamentStatus::Canceled || tournament.status == TournamentStatus::PreFinish || tournament.status == TournamentStatus::Finished @ BookmakerError::InvalidTournamentStatus)]
    pub tournament: Box<Account<'info, Tournament>>,

    #[account(mut, seeds = [GENOME_ROOT, b"BOOK", &tournament_id.to_le_bytes().as_ref()], bump)]
    pub tournament_book: Box<Account<'info, TournamentBook>>,

    #[account(mut, associated_token::mint = mint, associated_token::authority = tournament_book, associated_token::token_program = token_program)]
    pub tournament_book_vault: Box<InterfaceAccount<'info, TokenAccount>>,

    #[account(mut, seeds = [GENOME_ROOT, b"CAPTAIN_BET", &tournament_id.to_le_bytes().as_ref(), captain.key().as_ref()], bump)]
    pub captain_bet: Box<Account<'info, CaptainBet>>,

    pub mint: Box<InterfaceAccount<'info, Mint>>,

    #[account(mut, seeds = [GENOME_ROOT, b"CONFIG"], bump)]
    pub config: Box<Account<'info, GenomeConfig>>,

    pub system_program: Program<'info, System>,
    pub token_program: Interface<'info, TokenInterface>,
}

// 3. Claim Overbook Tokens Instruction: called by the gambler
//     - Receives:
//          - Tournament PDA
//          - TournamentBook PDA
//          - CaptainBet PDA: corresponds to a captain of a certain tournament
//          - GamblerInfo PDA: corresponds to a gambler that has a bet on the captain of a certain tournament
//     - Only callable if the tournament has started
//     - If the captains overbooked == false uses the info in the PDAs to determine if the captain is overbooked.
//             - If is overbooked set overbooked == true and overbook_claimable to the corresponding value
//             - The gambler can claim his bet (or part of it) on the captain
//                   - Send the tokens to the gambler
//                   - Updates the book total_sum
//                   - Updates the captain sum
//                   - Updates the gambler bet and sets the has_claimed_overbook to true
//     - If the captains overbooked == true and overbook_claimable > 0 and gamblers has_claimed_overbook == false
//             - The gambler can claim his bet (or part of it) on the captain
//                   - Send the tokens to the gambler
//                   - Updates the book total_sum
//                   - Updates the captain sum
//                   - Updates the gambler bet and sets the has_claimed_overbook to true

pub fn handle_claim_overbook_tokens(
    ctx: Context<ClaimOverbookTokens>,
    gambler: Pubkey,
    captain: Pubkey,
    tournament_id: u64,
) -> Result<()> {
    // Make a match statement to check if the captain is overbooked
    match ctx.accounts.captain_bet.overbooked {
        // If the captain is overbooked
        true => {
            require!(
                ctx.accounts.gambler_info.has_claimed_overbook == false,
                BookmakerError::OverbookAlreadyClaimed
            );
            require!(
                ctx.accounts.captain_bet.overbook_claimable > 0,
                BookmakerError::NoOverbookLeftToClaim
            );
        }
        // If the captain is not overbooked
        false => {
            // Try to calculate if the captain is overbooked
            let minimal_coef = calc_minimal_coef(ctx.accounts.tournament.captains.len() as u64);

            let total_sum = ctx.accounts.tournament_book.total_sum;

            let captain_sum = ctx.accounts.captain_bet.sum;

            if captain_sum * minimal_coef > total_sum * BP_DEC && minimal_coef > BP_DEC {
                ctx.accounts.captain_bet.overbooked = true;
                let refund =
                    captain_sum - ((total_sum - captain_sum) * BP_DEC / (minimal_coef - BP_DEC));
                ctx.accounts.captain_bet.overbook_claimable = refund;
                ctx.accounts.tournament_book.total_overbook_claimable += refund;
            }
        }
    }

    let claimable_amount = u64::min(
        ctx.accounts.gambler_info.bet,
        ctx.accounts.captain_bet.overbook_claimable,
    );

    // Take tokens from the tournament book vault to the gambler vault
    let bump = &[ctx.bumps.tournament_book];
    let binding = tournament_id.to_le_bytes();
    let seed = &[GENOME_ROOT, b"BOOK", &binding, bump][..];
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

    // Update the book
    ctx.accounts.tournament_book.total_sum -= claimable_amount;
    ctx.accounts.tournament_book.total_overbook_claimable -= claimable_amount;
    ctx.accounts.captain_bet.sum -= claimable_amount;
    ctx.accounts.captain_bet.overbook_claimable -= claimable_amount;
    ctx.accounts.gambler_info.bet -= claimable_amount;
    ctx.accounts.gambler_info.has_claimed_overbook = true;

    // Emit the event
    emit!(OverbookClaimedEvent {
        tournament_id,
        captain,
        gambler,
        amount: claimable_amount,
    });

    Ok(())
}

pub fn calc_minimal_coef(teams_count: u64) -> u64 {
    2 * BP_DEC - BP_DEC / teams_count
}

#[derive(Debug)]
#[event]
pub struct OverbookClaimedEvent {
    pub tournament_id: u64,
    pub captain: Pubkey,
    pub gambler: Pubkey,
    pub amount: u64,
}
