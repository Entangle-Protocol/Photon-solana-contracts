use std::vec;

use anchor_lang::prelude::*;

use anchor_spl::{
    token_2022::TransferChecked,
    token_interface::{transfer_checked, Mint, TokenAccount, TokenInterface},
};

use crate::{
    error::{ControlAccessError, FeesError},
    FeeMeta, OperatorInfo, ZS_ROOT,
};

#[derive(Accounts)]
#[instruction(fee_type: u16)]
pub struct ClaimBeneficiaryFees<'info> {
    //------------------- WITHDRAW TREASURY ACCOUNTS -------------------
    #[account(signer, mut)]
    pub operator: Signer<'info>,

    /// CHECK: treasury authority
    #[account(seeds = [ZS_ROOT, b"AUTHORITY"], bump)]
    pub authority: AccountInfo<'info>,

    #[account(mut, associated_token::mint = mint, associated_token::authority = authority, associated_token::token_program = token_program)]
    pub treasury_vault: Box<InterfaceAccount<'info, TokenAccount>>,

    pub mint: Box<InterfaceAccount<'info, Mint>>,

    //-----------------------------------------------------------------
    #[account(seeds = [ZS_ROOT, b"OPERATOR", operator.key().as_ref()], bump, constraint = operator_info.approved @ ControlAccessError::OperatorNotApproved)]
    pub operator_info: Box<Account<'info, OperatorInfo>>,

    /// CHECK: beneficiary
    #[account(mut)]
    pub beneficiary: AccountInfo<'info>,

    #[account(mut, associated_token::mint = mint, associated_token::authority = beneficiary, associated_token::token_program = token_program)]
    pub beneficiary_vault: Box<InterfaceAccount<'info, TokenAccount>>,

    #[account(mut, seeds = [ZS_ROOT, b"FEE_META", &fee_type.to_le_bytes().as_ref()], bump, constraint = fee_type != 0)]
    pub fee_meta: Box<Account<'info, FeeMeta>>,

    pub token_program: Interface<'info, TokenInterface>,
    pub system_program: Program<'info, System>,
}

pub fn handle_claim_beneficiary_fees(
    ctx: Context<ClaimBeneficiaryFees>,
    _fee_type: u16,
) -> Result<()> {
    let beneficiaries = ctx.accounts.fee_meta.beneficiaries.clone();

    for (i, beneficiary) in beneficiaries.iter().enumerate() {
        if *beneficiary == ctx.accounts.beneficiary.key() {
            let pending_to_claim = ctx.accounts.fee_meta.pending_to_claim[i];

            require!(pending_to_claim > 0, FeesError::NoFeesToClaim);

            // Send the tokens from the opertor vault to the beneficiary vault
            let bump = &[ctx.bumps.authority];
            let seed = &[ZS_ROOT, b"AUTHORITY", bump][..];
            let seeds = &[seed];
            let accounts = TransferChecked {
                from: ctx.accounts.treasury_vault.to_account_info(),
                mint: ctx.accounts.mint.to_account_info(),
                to: ctx.accounts.beneficiary_vault.to_account_info(),
                authority: ctx.accounts.authority.to_account_info(),
            };
            let cpi = CpiContext::new_with_signer(
                ctx.accounts.token_program.to_account_info(),
                accounts,
                seeds,
            );
            transfer_checked(cpi, pending_to_claim, ctx.accounts.mint.decimals)?;

            ctx.accounts.fee_meta.pending_to_claim[i] = 0;

            emit!(ClaimBeneficiaryFeesEvent {
                beneficiary: *beneficiary,
                fees_claimed: pending_to_claim,
            });

            return Ok(());
        }
    }

    return Err(FeesError::BeneficiaryNotFound.into());
}

#[derive(Debug)]
#[event]
pub struct ClaimBeneficiaryFeesEvent {
    pub beneficiary: Pubkey,
    pub fees_claimed: u64,
}
