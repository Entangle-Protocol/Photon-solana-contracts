use anchor_lang::prelude::*;

use crate::{error::ControlAccessError, ClaimableUserInfo, OperatorInfo, GENOME_ROOT};

#[derive(Accounts)]
pub struct UpdateClaimableRewards<'info> {
    #[account(signer, mut)]
    pub operator: Signer<'info>,

    #[account(seeds = [GENOME_ROOT, b"OPERATOR", operator.key().as_ref()], bump, constraint = operator_info.approved @ ControlAccessError::OperatorNotApproved)]
    pub operator_info: Box<Account<'info, OperatorInfo>>,
    #[account(
        init_if_needed,
        payer = operator,
        seeds = [GENOME_ROOT, b"USER", operator.key().as_ref()],
        bump,
        space = ClaimableUserInfo::LEN,
    )]
    pub claimable_user_info: Box<Account<'info, ClaimableUserInfo>>,

    pub system_program: Program<'info, System>,
}

pub fn handle_update_claimable_rewards(
    ctx: Context<UpdateClaimableRewards>,
    user: Pubkey,
    amount: u64,
) -> Result<()> {
    ctx.accounts.claimable_user_info.claimable += amount;

    emit!(IncrementClaimableRewardsEvent {
        user,
        amount: amount.into()
    });

    Ok(())
}

#[derive(Debug)]
#[event]
pub struct IncrementClaimableRewardsEvent {
    pub user: Pubkey,
    pub amount: i128,
}

#[derive(Debug)]
#[event]
pub struct DecrementClaimableRewardsEvent {
    pub user: Pubkey,
    pub amount: i128,
}
