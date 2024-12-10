use anchor_lang::prelude::*;
use anchor_spl::{
    associated_token::AssociatedToken,
    token_2022::TransferChecked,
    token_interface::{transfer_checked, Mint, TokenAccount, TokenInterface},
};

use crate::{error::ControlAccessError, OperatorInfo, ZS_ROOT};

#[derive(Accounts)]
pub struct Deposit<'info> {
    #[account(signer, mut)]
    pub operator: Signer<'info>,

    /// CHECK: treasury authority
    #[account(seeds = [ZS_ROOT, b"AUTHORITY"], bump)]
    pub authority: AccountInfo<'info>,

    #[account(seeds = [ZS_ROOT, b"OPERATOR", operator.key().as_ref()], bump, constraint = operator_info.approved @ ControlAccessError::OperatorNotApproved)]
    pub operator_info: Box<Account<'info, OperatorInfo>>,

    pub mint: Box<InterfaceAccount<'info, Mint>>,

    #[account(init_if_needed, payer = operator, associated_token::mint = mint, associated_token::authority = authority, associated_token::token_program = token_program)]
    pub vault: Box<InterfaceAccount<'info, TokenAccount>>,

    #[account(mut, associated_token::mint = mint, associated_token::authority = operator, associated_token::token_program = token_program)]
    pub source: Box<InterfaceAccount<'info, TokenAccount>>,

    pub token_program: Interface<'info, TokenInterface>,
    pub associated_token_program: Program<'info, AssociatedToken>,
    pub system_program: Program<'info, System>,
}

pub fn handle_deposit(ctx: Context<Deposit>, amount: u64) -> Result<()> {
    let accounts = TransferChecked {
        from: ctx.accounts.source.to_account_info(),
        mint: ctx.accounts.mint.to_account_info(),
        to: ctx.accounts.vault.to_account_info(),
        authority: ctx.accounts.operator.to_account_info(),
    };
    let cpi = CpiContext::new(ctx.accounts.token_program.to_account_info(), accounts);
    transfer_checked(cpi, amount, ctx.accounts.mint.decimals)?;
    Ok(())
}
