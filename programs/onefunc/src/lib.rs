extern crate photon;

use anchor_lang::prelude::*;
use photon::{cpi::accounts::Propose, photon::ROOT, program::Photon};

declare_id!("EjpcUpcuJV2Mq9vjELMZHhgpvJ4ggoWtUYCTFqw6D9CZ");

#[program]
pub mod onefunc {
    use ethabi::ParamType;

    use super::*;

    pub static PROTOCOL_ID: &[u8] = b"onefunc_________________________";

    pub fn initialize(ctx: Context<Initialize>) -> Result<()> {
        ctx.accounts.counter.call_authority = ctx.accounts.call_authority.key();
        Ok(())
    }

    /// Example call by method name
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

    /// Example call by method id
    pub fn photon_msg(
        ctx: Context<Increment>,
        _protocol_id: Vec<u8>,
        _src_chain_id: u128,
        _src_block_number: u64,
        _src_op_tx_id: Vec<u8>,
        function_selecor: Vec<u8>,
        params: Vec<u8>,
    ) -> Result<()> {
        let selector: [u8; 4] = function_selecor
            .try_into()
            .map_err(|_| CustomError::InvalidSelector)?;
        match selector {
            [1, 2, 3, 4] => {
                let to_increment = ethabi::decode(&[ParamType::Uint(256)], &params)
                    .map_err(|_| CustomError::InvalidParams)?
                    .get(0)
                    .unwrap()
                    .clone()
                    .into_uint()
                    .unwrap()
                    .as_u64();
                ctx.accounts.counter.count += to_increment;
            }
            _ => return Err(CustomError::InvalidSelector.into()),
        }
        Ok(())
    }

    /// Example, call propose within the entangle multichain environment
    pub fn propose_to_other_chain(ctx: Context<ProposeToOtherChain>) -> Result<()> {
        let protocol_id: Vec<u8> = PROTOCOL_ID.to_vec();
        let dst_chain_id = 33133_u128;
        let protocol_address: Vec<u8> = vec![1, 54, 22, 87, 84, 85, 00, 00, 71];
        let function_selector: Vec<u8> = b"ask1234mkl;1mklasdfasm;lkasdmf__".to_vec();
        let params: Vec<u8> = b"an arbitrary data".to_vec();

        let cpi_program = ctx.accounts.photon_program.to_account_info();
        let cpi_accounts = Propose {
            proposer: ctx.accounts.proposer.to_account_info(),
            config: ctx.accounts.config.to_account_info(),
            protocol_info: ctx.accounts.protocol_info.to_account_info(),
        };
        let bump = [ctx.bumps.proposer];
        let proposer_seeds = [ROOT, b"PROPOSER", &bump[..]];
        let bindings = &[&proposer_seeds[..]][..];
        let ctx = CpiContext::new_with_signer(cpi_program, cpi_accounts, bindings);

        photon::cpi::propose(
            ctx,
            protocol_id,
            dst_chain_id,
            protocol_address,
            function_selector,
            params,
        )
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
    #[msg("InvalidSelector")]
    InvalidSelector,
}

#[derive(Accounts)]
pub struct ProposeToOtherChain<'info> {
    /// Owner account
    #[account(signer, mut)]
    owner: Signer<'info>,

    /// The address of the photon aggregation spotter program to make a proposal
    photon_program: Program<'info, Photon>,

    /// System config to be used by entangle aggregation spotter program
    /// seeds = ["root-0", "CONFIG"]
    /// seeds::program = photon_program
    /// CHECK: Due to be validated within the aggregation spotter program
    #[account(mut)]
    config: UncheckedAccount<'info>,

    /// Protocol info to be used by entangle aggregation spotter program
    /// seeds = ["root-0", "PROTOCOL", "aggregation-gov_________________"]
    /// seeds::program = photon_program
    /// CHECK: Due to be validated within the aggregation spotter program
    protocol_info: UncheckedAccount<'info>,

    /// Proposer account that was registered by the entangle spotter program previously
    /// CHECK: Due to be validated within the aggregation spotter program as a signer and a registered proposer
    #[account(init_if_needed, payer = owner, space = 0, seeds = [ROOT, b"PROPOSER"], bump)]
    proposer: UncheckedAccount<'info>,

    /// System program be able to create the proposer account if it's not created
    system_program: Program<'info, System>,
}
