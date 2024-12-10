use anchor_lang::prelude::*;

use crate::{error::TournamentError, OperatorInfo, Role, ZeroSumConfig, ZS_ROOT};

#[derive(Accounts)]
pub struct SetTournamentParams<'info> {
    #[account(signer, mut)]
    pub admin: Signer<'info>,
    #[account(
        mut,
        seeds = [ZS_ROOT, b"CONFIG"],
        bump,
    )]
    pub config: Box<Account<'info, ZeroSumConfig>>,

    #[account(seeds = [ZS_ROOT, b"OPERATOR", admin.key().as_ref()], bump, constraint = operator_info.approved && (operator_info.role == Role::DEVELOPER || operator_info.role == Role::OWNER) @ TournamentError::OperatorNotApprovedOrInvalidRole)]
    pub operator_info: Box<Account<'info, OperatorInfo>>,
    pub system_program: Program<'info, System>,
}

pub fn handle_set_tournament_params(
    ctx: Context<SetTournamentParams>,
    minimal_admision_fee: u64,
    minimal_sponsor_pool: u64,
) -> Result<()> {
    if minimal_admision_fee > 0 {
        ctx.accounts.config.tournament_config.minimal_admision_fee = minimal_admision_fee;
    }
    if minimal_sponsor_pool > 0 {
        ctx.accounts.config.tournament_config.minimal_sponsor_pool = minimal_sponsor_pool;
    }

    Ok(())
}
