use std::vec;

use anchor_lang::prelude::*;

use anchor_spl::{
    token_2022::TransferChecked,
    token_interface::{transfer_checked, Mint, TokenAccount, TokenInterface},
};

use crate::{
    error::BookmakerError, send_beneficiaries_book_tokens_to_treasury, CaptainBet, FeeMeta,
    GamblerInfo, SendPlatformWalletEvent, Tournament, TournamentBook, TournamentStatus,
    UpdateBeneficiariesClaimEvent, ZeroSumConfig, BP_DEC, ZS_ROOT,
};

#[derive(Accounts)]
#[instruction(gambler: Pubkey, captain: Pubkey, tournament_id: u64, fee_type: u16)]
pub struct ClaimFinishTokens<'info> {
    #[account(signer, mut)]
    pub gambler: Signer<'info>,

    #[account(mut, associated_token::mint = mint, associated_token::authority = gambler, associated_token::token_program = token_program)]
    pub gambler_vault: Box<InterfaceAccount<'info, TokenAccount>>,

    #[account(mut, seeds = [ZS_ROOT, b"GAMBLER", &tournament_id.to_le_bytes().as_ref(), captain.key().as_ref(), gambler.key().as_ref()], bump, constraint = gambler_info.has_claimed_finish == false @ BookmakerError::FinishAlreadyClaimed)]
    pub gambler_info: Box<Account<'info, GamblerInfo>>,

    #[account(mut, seeds = [ZS_ROOT, b"TOURNAMENT", &tournament_id.to_le_bytes().as_ref()], bump, constraint = tournament.status == TournamentStatus::Finished || tournament.status == TournamentStatus::PreFinish @ BookmakerError::InvalidTournamentStatus)]
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

    #[account(mut,seeds = [ZS_ROOT, b"FEE_META", &fee_type.to_le_bytes().as_ref()], bump)]
    pub fee_meta: Box<Account<'info, FeeMeta>>,

    #[account(mut, associated_token::mint = mint, associated_token::authority = platform_wallet, associated_token::token_program = token_program)]
    pub platform_wallet_vault: Box<InterfaceAccount<'info, TokenAccount>>,

    /// CHECK: platform wallet
    #[account(mut)]
    pub platform_wallet: AccountInfo<'info>,

    /// CHECK: treasury authority
    #[account(seeds = [ZS_ROOT, b"AUTHORITY"], bump)]
    pub treasury_authority: AccountInfo<'info>,

    #[account(mut, associated_token::mint = mint, associated_token::authority = treasury_authority, associated_token::token_program = token_program)]
    pub treasury_vault: Box<InterfaceAccount<'info, TokenAccount>>,

    pub system_program: Program<'info, System>,
    pub token_program: Interface<'info, TokenInterface>,
}

pub fn handle_claim_finish_tokens(
    ctx: Context<ClaimFinishTokens>,
    gambler: Pubkey,
    captain: Pubkey,
    tournament_id: u64,
    fee_type: u16,
) -> Result<()> {
    // The captain must be a winner
    require!(
        ctx.accounts
            .tournament
            .finish_metadata
            .winners
            .contains(&captain),
        BookmakerError::CaptainNotWinner
    );

    if ctx.accounts.tournament_book.total_sum == 0 {
        // No Bets
        return Ok(());
    }

    let fee;
    if fee_type == 0 {
        fee = ctx.accounts.config.fees_config.base_fee
    } else {
        fee = ctx.accounts.fee_meta.base_fee
    }

    let mut platform_wallet_fee = 0;
    let claimable_amount;

    if ctx.accounts.captain_bet.sum == 0 {
        // No Bets on winner captain
        let gambler_bet = ctx.accounts.gambler_info.bet;

        claimable_amount = gambler_bet - (gambler_bet * fee / BP_DEC);

        platform_wallet_fee = gambler_bet * fee / BP_DEC;
    } else {
        // Some Bets on winner captain
        // We need to subtract the overbooked still claimable from the total sum
        let total_sum = ctx.accounts.tournament_book.total_sum
            - ctx.accounts.tournament_book.total_overbook_claimable;
        let prize_pool = total_sum - (total_sum * fee / BP_DEC);

        // This will only be reached the first time this instruction is called
        if !ctx.accounts.captain_bet.fees_sent_to_beneficiaries {
            if fee_type != 0 {
                // Update beneficiaries claim
                //   - This ensures the beneficiaries can claim their fees later from the treasury vault (do not reach CUs limit)
                let remaining;
                match ctx.accounts.fee_meta.update_beneficiaries_claim(total_sum) {
                    Ok(val) => {
                        // Val is the remaining fee after updating the beneficiaries claim
                        platform_wallet_fee += val;
                        remaining = val;
                    }
                    Err(err) => {
                        panic!("Error: {}", err);
                    }
                }

                ctx.accounts.captain_bet.fees_sent_to_beneficiaries = true;

                // Send the beneficiaries claim tokens to the treasury vault, so then they can claim them
                let amount = total_sum * fee / BP_DEC - remaining;
                send_beneficiaries_book_tokens_to_treasury(
                    amount,
                    ctx.accounts.mint.to_account_info(),
                    ctx.accounts.mint.decimals,
                    ctx.accounts.treasury_vault.to_account_info(),
                    ctx.accounts.tournament_book_vault.to_account_info(),
                    ctx.accounts.tournament_book.to_account_info(),
                    tournament_id,
                    ctx.bumps.tournament_book,
                    ctx.accounts.token_program.to_account_info(),
                )?;

                emit!(UpdateBeneficiariesClaimEvent {
                    fee_type,
                    total_amount: total_sum,
                    beneficiaries: ctx.accounts.fee_meta.beneficiaries.clone(),
                });
            } else {
                // all the fees are sent to the platform wallet
                platform_wallet_fee = total_sum * fee / BP_DEC;
            }
        }

        claimable_amount =
            prize_pool * ctx.accounts.gambler_info.bet / ctx.accounts.captain_bet.sum;
    }

    if claimable_amount > 0 {
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
        ctx.accounts.gambler_info.has_claimed_finish = true;

        // Emit the event
        emit!(FinishClaimedEvent {
            tournament_id,
            captain,
            gambler,
            amount: claimable_amount,
        });
    }

    if platform_wallet_fee > 0 {
        // Transfer the platform wallet fee
        let bump = &[ctx.bumps.tournament_book];
        let binding = tournament_id.to_le_bytes();
        let seed = &[ZS_ROOT, b"BOOK", &binding, bump][..];
        let seeds = &[seed];
        let accounts = TransferChecked {
            from: ctx.accounts.tournament_book_vault.to_account_info(),
            mint: ctx.accounts.mint.to_account_info(),
            to: ctx.accounts.platform_wallet_vault.to_account_info(),
            authority: ctx.accounts.tournament_book.to_account_info(),
        };
        let cpi = CpiContext::new_with_signer(
            ctx.accounts.token_program.to_account_info(),
            accounts,
            seeds,
        );
        transfer_checked(cpi, platform_wallet_fee, ctx.accounts.mint.decimals)?;

        // Emit the event
        emit!(SendPlatformWalletEvent {
            fee_type,
            amount: platform_wallet_fee,
            platform_wallet: *ctx.accounts.platform_wallet.key,
        });
    }

    Ok(())
}

#[derive(Debug)]
#[event]
pub struct FinishClaimedEvent {
    pub tournament_id: u64,
    pub captain: Pubkey,
    pub gambler: Pubkey,
    pub amount: u64,
}
