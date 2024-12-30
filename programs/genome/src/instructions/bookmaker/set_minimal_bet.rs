use anchor_lang::prelude::*;

use crate::{error::ControlAccessError, OperatorInfo, Role, GenomeConfig, GENOME_ROOT};

#[derive(Accounts)]
pub struct SetMinimalBet<'info> {
    #[account(signer, mut)]
    pub admin: Signer<'info>,

    #[account(mut)]
    pub config: Box<Account<'info, GenomeConfig>>,

    #[account(seeds = [GENOME_ROOT, b"OPERATOR", admin.key().as_ref()], bump, constraint = operator_info.approved && (operator_info.role == Role::DEVELOPER || operator_info.role == Role::OWNER) @ ControlAccessError::OperatorNotApprovedOrInvalidRole)]
    pub operator_info: Box<Account<'info, OperatorInfo>>,

    pub system_program: Program<'info, System>,
}

pub fn handle_set_minimal_bet(ctx: Context<SetMinimalBet>, minimal_bet: u64) -> Result<()> {
    ctx.accounts.config.bookmaker_config.minimal_bet = minimal_bet;

    Ok(())
}
