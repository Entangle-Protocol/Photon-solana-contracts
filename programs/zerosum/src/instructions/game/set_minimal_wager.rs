use anchor_lang::prelude::*;

use crate::{error::ControlAccessError, OperatorInfo, Role, ZeroSumConfig, ZS_ROOT};

#[derive(Accounts)]
pub struct SetGamesMinimalFee<'info> {
    #[account(signer, mut)]
    pub admin: Signer<'info>,

    #[account(mut)]
    pub config: Box<Account<'info, ZeroSumConfig>>,

    #[account(seeds = [ZS_ROOT, b"OPERATOR", admin.key().as_ref()], bump, constraint = operator_info.approved @ ControlAccessError::OperatorNotApproved, constraint= operator_info.role == Role::OWNER @ ControlAccessError::OperatorNotOwner)]
    pub operator_info: Box<Account<'info, OperatorInfo>>,

    pub system_program: Program<'info, System>,
}

pub fn handle_set_games_minimal_fee(
    ctx: Context<SetGamesMinimalFee>,
    minimal_wager: u64,
) -> Result<()> {
    ctx.accounts.config.games_config.minimal_wager = minimal_wager;

    Ok(())
}
