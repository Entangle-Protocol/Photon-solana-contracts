use anchor_lang::prelude::*;
use ngl_core::{DEPLOYER, ROOT};

use crate::BridgeConfig;

#[derive(Accounts)]
pub struct InitializeBridge<'info> {
    /// Bridge admin
    #[account(mut, signer, address = DEPLOYER.parse().expect("Deployer key not set"))]
    pub admin: Signer<'info>,

    /// Bridge config
    #[account(init, payer = admin, space = BridgeConfig::LEN, seeds = [ROOT, b"BRIDGE_CONFIG"], bump)]
    pub config: Box<Account<'info, BridgeConfig>>,

    pub system_program: Program<'info, System>,
}

/// Initialize the bridge
pub fn handle_initialize_bridge(
    ctx: Context<InitializeBridge>,
    bridge_router_address: Vec<u8>,
) -> Result<()> {
    ctx.accounts.config.bridge_router_address = bridge_router_address.try_into().unwrap();
    ctx.accounts.config.admin = ctx.accounts.admin.key();
    Ok(())
}
