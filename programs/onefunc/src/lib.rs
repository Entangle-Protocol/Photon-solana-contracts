use anchor_lang::prelude::*;

declare_id!("6cBwMuV2hTAVAXYSqYXULuXitknhzJYu3QXjuH9mKaLg");

#[program]
pub mod onefunc {
    use anchor_lang::context::Context;

    use super::*;

    pub fn initialize(ctx: Context<Initialize>) -> Result<()> {
        ctx.accounts.counter.call_authority = ctx.accounts.call_authority.key();
        Ok(())
    }

    pub fn increment(ctx: Context<Increment>) -> Result<()> {
        ctx.accounts.counter.count += 1;
        Ok(())
    }
}

#[derive(Accounts)]
pub struct Initialize<'info> {
    /// Owner account
    #[account(signer, mut)]
    owner: Signer<'info>,

    /// Aggregation Spotter call authority address for the protocol
    /// CHECK: not loaded
    call_authority: AccountInfo<'info>,

    /// Counter
    #[account(
        init,
        space = 8 + 8 + 32,
        payer = owner,
        seeds = [b"COUNTER"],
        bump
    )]
    counter: Box<Account<'info, Counter>>,

    /// System program
    system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct Increment<'info> {
    /// Owner account
    #[account(signer, constraint = call_authority.key() == counter.call_authority)]
    call_authority: Signer<'info>,
    /// Counter
    #[account(
        mut,
        seeds = [b"COUNTER"],
        bump
    )]
    counter: Box<Account<'info, Counter>>,
}

#[account]
#[derive(Default)]
pub struct Counter {
    call_authority: Pubkey,
    count: u64,
}
