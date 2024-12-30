use crate::{FeeMeta, SendPlatformWalletEvent, UpdateBeneficiariesClaimEvent, GenomeConfig, BP_DEC};
use crate::{
    error::TournamentError, FinishTournamentMetadata, OperatorInfo, Role, Tournament,
    TournamentStatus, GENOME_ROOT,
};
use anchor_lang::prelude::*;
use anchor_spl::{
    token_2022::TransferChecked,
    token_interface::{transfer_checked, Mint, TokenAccount, TokenInterface},
};

#[derive(Accounts)]
pub struct FinishTournament<'info> {
    #[account(signer, mut)]
    pub operator: Signer<'info>,

    #[account(seeds = [GENOME_ROOT, b"OPERATOR", operator.key().as_ref()], bump, constraint = operator_info.approved && (operator_info.role == Role::BACKEND || operator_info.role == Role::OWNER) @ TournamentError::OperatorNotApprovedOrInvalidRole)]
    pub operator_info: Box<Account<'info, OperatorInfo>>,

    #[account(mut, seeds = [GENOME_ROOT, b"CONFIG"], bump)]
    pub config: Box<Account<'info, GenomeConfig>>,
    
    #[account(
        mut,
        seeds = [GENOME_ROOT, b"TOURNAMENT", &tournament.id.to_le_bytes().as_ref()],
        bump
    )]
    pub tournament: Box<Account<'info, Tournament>>,
    pub mint: Box<InterfaceAccount<'info, Mint>>,
    // In case the fee_type is 0, this account is not used
    #[account(mut)]
    pub fee_meta: Box<Account<'info, FeeMeta>>,
    #[account(
        mut,
        associated_token::mint = mint, 
        associated_token::authority = tournament,
        associated_token::token_program = token_program,
    )]
    pub tournament_vault: Box<InterfaceAccount<'info, TokenAccount>>,
    /// CHECK: platform wallet
    #[account(mut)]
    pub platform_wallet: AccountInfo<'info>,
    #[account(mut, associated_token::mint = mint, associated_token::authority = platform_wallet, associated_token::token_program = token_program)]
    pub platform_wallet_vault: Box<InterfaceAccount<'info, TokenAccount>>,
    pub system_program: Program<'info, System>,
    pub token_program: Interface<'info, TokenInterface>,
}

pub fn handle_finish_tournament(
    ctx: Context<FinishTournament>,
    winners: Vec<Pubkey>,
    rewards_prize_fractions: Vec<u16>,
    fee_type: u8,
) -> Result<()> {
    if rewards_prize_fractions.len() != winners.len() {
        return Err(TournamentError::InvalidPrizeFractionsAmount.into());
    }
    let tournament = &mut ctx.accounts.tournament;

    if tournament.status != TournamentStatus::Started {
        return Err(TournamentError::InvalidTournamentStatus.into());
    }

    let prize_pool = tournament.fee
        * tournament.captains.len() as u64
        * tournament.players_in_team as u64
        + tournament.sponsor_pool;

   

    let fee_meta = &mut ctx.accounts.fee_meta;
    let base_fee;
    if fee_type == 0 {
        base_fee = ctx.accounts.config.fees_config.base_fee;
    } else {
        base_fee = fee_meta.base_fee;
    }
    let total_reward = prize_pool - (prize_pool * base_fee / BP_DEC);

    let mut fee = prize_pool - total_reward;

    if fee_type != 0 {
        // Update beneficiaries claim
        //   - This ensures the beneficiaries can claim their fees later from the treasury vault (do not reach CUs limit)
        match fee_meta.update_beneficiaries_claim(prize_pool) {
            Ok(val) => {
                // Val is the remaining fee after updating the beneficiaries claim
                fee = val;
            }
            Err(err) => {
                panic!("Error: {}", err);
            }
        }

        emit!(UpdateBeneficiariesClaimEvent {
            fee_type: fee_type.into(),
            total_amount: prize_pool,
            beneficiaries: fee_meta.beneficiaries.clone(),
        });
    }


    if fee > 0 {
        let seeds = &[
            GENOME_ROOT,
            b"TOURNAMENT",
            &tournament.id.to_le_bytes()[..],
            &[tournament.bump],
        ];
        let signer_seeds = [&seeds[..]];
        let accounts = TransferChecked {
            from: ctx.accounts.tournament_vault.to_account_info(),
            mint: ctx.accounts.mint.to_account_info(),
            to: ctx.accounts.platform_wallet_vault.to_account_info(),
            authority: tournament.to_account_info(),
        };
        let cpi = CpiContext::new_with_signer(
            ctx.accounts.token_program.to_account_info(),
            accounts,
            &signer_seeds,
        );
        transfer_checked(cpi, fee, ctx.accounts.mint.decimals)?;

        emit!(SendPlatformWalletEvent {
            fee_type: fee_type.into(),
            amount: fee,
            platform_wallet: *ctx.accounts.platform_wallet.key,
        });
    }
    let winners_length = winners.len();
    tournament.finish_metadata = FinishTournamentMetadata {
        winners,
        rewards_prize_fractions,
        fee_type,
        remaining_prize_pool: prize_pool - fee,
        total_prize_pool: prize_pool,
        rewarded_winners: vec![false; winners_length]
    };

    tournament.status = TournamentStatus::PreFinish;

    Ok(())
}