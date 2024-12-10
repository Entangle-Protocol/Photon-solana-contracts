use anchor_lang::prelude::*;

use crate::{error::ControlAccessError, OperatorInfo, Role, ZeroSumConfig, ZS_ROOT};

#[derive(Accounts)]
pub struct ApproveOperator<'info> {
    #[account(signer, mut, address = config.admin @ ControlAccessError::OperatorNotOwner )]
    pub admin: Signer<'info>,

    #[account(seeds = [ZS_ROOT, b"CONFIG"], bump)]
    pub config: Box<Account<'info, ZeroSumConfig>>,

    /// CHECK: operator account
    pub operator: AccountInfo<'info>,

    #[account(
        init_if_needed,
        payer = admin, space = OperatorInfo::LEN,
        seeds = [ZS_ROOT, b"OPERATOR", operator.key().as_ref()], bump
    )]
    pub operator_info: Box<Account<'info, OperatorInfo>>,

    pub system_program: Program<'info, System>,
}

// Aproves the operator and assigns a role to it
pub fn handle_approve_operator(ctx: Context<ApproveOperator>, role: Role) -> Result<()> {
    ctx.accounts.operator_info.approved = true;
    ctx.accounts.operator_info.role = role;

    Ok(())
}
