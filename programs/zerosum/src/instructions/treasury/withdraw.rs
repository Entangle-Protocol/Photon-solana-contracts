use anchor_lang::prelude::*;
use anchor_spl::token_interface::{
    transfer_checked, Mint, TokenAccount, TokenInterface, TransferChecked,
};

use crate::{error::ControlAccessError, OperatorInfo, ZS_ROOT};

#[derive(Accounts)]
pub struct Withdraw<'info> {
    #[account(signer, mut)]
    pub operator: Signer<'info>,

    /// CHECK: treasury authority
    #[account(seeds = [ZS_ROOT, b"AUTHORITY"], bump)]
    pub authority: AccountInfo<'info>,

    #[account(seeds = [ZS_ROOT, b"OPERATOR", operator.key().as_ref()], bump, constraint = operator_info.approved @ ControlAccessError::OperatorNotApproved)]
    pub operator_info: Box<Account<'info, OperatorInfo>>,

    pub mint: Box<InterfaceAccount<'info, Mint>>,

    #[account(mut, associated_token::mint = mint, associated_token::authority = authority, associated_token::token_program = token_program)]
    pub vault: Box<InterfaceAccount<'info, TokenAccount>>,

    #[account(mut, associated_token::mint = mint, associated_token::authority = operator, associated_token::token_program = token_program)]
    pub destination: Box<InterfaceAccount<'info, TokenAccount>>,

    pub token_program: Interface<'info, TokenInterface>,
    pub system_program: Program<'info, System>,
}

pub fn handle_withdraw(ctx: Context<Withdraw>, amount: u64) -> Result<()> {
    let bump = &[ctx.bumps.authority];
    let seed = &[ZS_ROOT, b"AUTHORITY", bump][..];
    let seeds = &[seed];
    let accounts = TransferChecked {
        from: ctx.accounts.vault.to_account_info(),
        mint: ctx.accounts.mint.to_account_info(),
        to: ctx.accounts.destination.to_account_info(),
        authority: ctx.accounts.authority.to_account_info(),
    };
    let cpi = CpiContext::new_with_signer(
        ctx.accounts.token_program.to_account_info(),
        accounts,
        seeds,
    );
    transfer_checked(cpi, amount, ctx.accounts.mint.decimals)?;
    Ok(())
}
