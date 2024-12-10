use anchor_lang::prelude::*;
use anchor_spl::token_interface::{
    transfer_checked, Mint, TokenAccount, TokenInterface, TransferChecked,
};

use crate::{error::ControlAccessError, ClaimableUserInfo, OperatorInfo, GENOME_ROOT};

#[derive(Accounts)]
pub struct WithdrawRewards<'info> {
    #[account(signer, mut)]
    pub operator: Signer<'info>,

    /// CHECK: treasury authority
    #[account(seeds = [GENOME_ROOT, b"AUTHORITY"], bump)]
    pub authority: AccountInfo<'info>,

    #[account(seeds = [GENOME_ROOT, b"OPERATOR", operator.key().as_ref()], bump, constraint = operator_info.approved @ ControlAccessError::OperatorNotApproved)]
    pub operator_info: Box<Account<'info, OperatorInfo>>,

    pub mint: Box<InterfaceAccount<'info, Mint>>,

    #[account(mut, associated_token::mint = mint, associated_token::authority = authority, associated_token::token_program = token_program)]
    pub vault: Box<InterfaceAccount<'info, TokenAccount>>,
    #[account(mut)]
    pub claimable_user_info: Box<Account<'info, ClaimableUserInfo>>,

    #[account(mut, associated_token::mint = mint, associated_token::authority = operator, associated_token::token_program = token_program)]
    pub operator_vault: Box<InterfaceAccount<'info, TokenAccount>>,

    /// CHECK: user who will receive the rewards
    #[account(mut)]
    pub user: AccountInfo<'info>,

    #[account(mut, associated_token::mint = mint, associated_token::authority = user, associated_token::token_program = token_program)]
    pub user_vault: Box<InterfaceAccount<'info, TokenAccount>>,

    pub token_program: Interface<'info, TokenInterface>,
    pub system_program: Program<'info, System>,
}

pub fn handle_withdraw_rewards(ctx: Context<WithdrawRewards>) -> Result<()> {
    let claimable_user_info = &mut ctx.accounts.claimable_user_info;
    let withdraw_amount = claimable_user_info.claimable;
    let bump = &[ctx.bumps.authority];
    let seed = &[GENOME_ROOT, b"AUTHORITY", bump][..];
    let seeds = &[seed];
    let accounts = TransferChecked {
        from: ctx.accounts.vault.to_account_info(),
        mint: ctx.accounts.mint.to_account_info(),
        to: ctx.accounts.operator_vault.to_account_info(),
        authority: ctx.accounts.authority.to_account_info(),
    };
    let cpi = CpiContext::new_with_signer(
        ctx.accounts.token_program.to_account_info(),
        accounts,
        seeds,
    );
    transfer_checked(cpi, withdraw_amount as u64, ctx.accounts.mint.decimals)?;

    // Reloaad the operator vault
    ctx.accounts.operator_vault.reload()?;

    // Transfer the rewards to the user
    let accounts = TransferChecked {
        from: ctx.accounts.operator_vault.to_account_info(),
        mint: ctx.accounts.mint.to_account_info(),
        to: ctx.accounts.user_vault.to_account_info(),
        authority: ctx.accounts.operator.to_account_info(),
    };
    let cpi =
        CpiContext::new_with_signer(ctx.accounts.token_program.to_account_info(), accounts, &[]);
    transfer_checked(cpi, withdraw_amount as u64, ctx.accounts.mint.decimals)?;
    claimable_user_info.claimable -= withdraw_amount;

    Ok(())
}
