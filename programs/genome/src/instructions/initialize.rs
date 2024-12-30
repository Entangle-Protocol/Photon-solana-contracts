use anchor_lang::prelude::*;

use crate::{OperatorInfo, Role, GenomeConfig, GENOME_ROOT};

#[derive(Accounts)]
pub struct Initialize<'info> {
    #[account(signer, mut)]
    pub admin: Signer<'info>,

    #[account(init, payer = admin, space = GenomeConfig::LEN, seeds = [GENOME_ROOT, b"CONFIG"], bump)]
    pub config: Box<Account<'info, GenomeConfig>>,

    #[account(init, payer = admin, space = OperatorInfo::LEN, seeds = [GENOME_ROOT, b"OPERATOR", admin.key().as_ref()], bump)]
    pub operator_info: Box<Account<'info, OperatorInfo>>,

    pub system_program: Program<'info, System>,
}

pub fn handler(ctx: Context<Initialize>) -> Result<()> {
    let config = &mut ctx.accounts.config;

    config.admin = ctx.accounts.admin.key();
    config.tournament_config.tournament_count = 0;
    config.games_config.games_counter = 0;

    // Set the admin as an approved operator and assign the OWNER role
    let operator_info = &mut ctx.accounts.operator_info;
    operator_info.approved = true;
    operator_info.role = Role::OWNER;

    Ok(())
}
