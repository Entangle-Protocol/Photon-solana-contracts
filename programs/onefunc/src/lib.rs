use anchor_lang::prelude::*;

declare_id!("6cBwMuV2hTAVAXYSqYXULuXitknhzJYu3QXjuH9mKaLg");

#[program]
pub mod onefunc {
    use anchor_lang::context::Context;
    use ethabi::ParamType;

    use super::*;

    pub fn initialize(ctx: Context<Initialize>) -> Result<()> {
        ctx.accounts.counter.call_authority = ctx.accounts.call_authority.key();
        Ok(())
    }

    pub fn increment(
        ctx: Context<Increment>,
        _protocol_id: Vec<u8>,
        _src_chain_id: u128,
        _src_block_number: u64,
        _src_op_tx_id: Vec<u8>,
        params: Vec<u8>,
    ) -> Result<()> {
        let to_increment = ethabi::decode(&[ParamType::Uint(256)], &params)
            .map_err(|_| CustomError::InvalidParams)?
            .get(0)
            .unwrap()
            .clone()
            .into_uint()
            .unwrap()
            .as_u64();
        ctx.accounts.counter.count += to_increment;
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
    /// Protocol executor
    #[account(signer, mut)]
    executor: Signer<'info>,
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

#[error_code]
pub enum CustomError {
    #[msg("InvalidParams")]
    InvalidParams,
}
