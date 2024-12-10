use anchor_lang::prelude::*;
use ethabi::ParamType;
use photon::{cpi::accounts::Propose, photon::ROOT, program::Photon, OpInfo};

declare_id!("QjB5Zuc3PasXPfdSta54GzKQa5yNiQk9TEmLUJEA2Xk");

#[derive(Debug, Clone, Default, AnchorSerialize, AnchorDeserialize)]
pub struct PhotonMsgWithSelector {
    pub protocol_id: Vec<u8>,
    pub src_chain_id: u128,
    pub src_block_number: u64,
    pub src_op_tx_id: Vec<u8>,
    pub function_selector: Vec<u8>,
    pub params: Vec<u8>,
}

#[program]
pub mod onefunc {
    use photon::protocol_data::FunctionSelector;

    use super::*;

    pub static PROTOCOL_ID: &[u8; 32] = b"onefunc_________________________";

    pub fn initialize(ctx: Context<Initialize>) -> Result<()> {
        ctx.accounts.counter.call_authority = ctx.accounts.call_authority.key();
        Ok(())
    }

    pub fn init_owned_counter(ctx: Context<InitOwnedCounter>) -> Result<()> {
        msg!(
            "init owned counter, owner: {}, counter_pda: {}",
            ctx.accounts.counter_owner.key(),
            ctx.accounts.owned_counter.key()
        );
        ctx.accounts.owned_counter.call_authority = ctx.accounts.call_authority.key();
        Ok(())
    }

    /// Example call by method name
    pub fn increment(ctx: Context<Increment>, params: Vec<u8>) -> Result<()> {
        let inc_item = decode_increment_item(params);
        ctx.accounts.counter.count += inc_item;
        Ok(())
    }

    pub fn to_be_failed(_ctx: Context<ToBeFailed>) -> Result<()> {
        require!(false, CustomError::InvalidParams);
        Ok(())
    }

    pub fn increment_owned_counter(
        ctx: Context<IncrementOwnedCounter>,
        params: Vec<u8>,
    ) -> Result<()> {
        let inc_item = decode_increment_item(params);
        let counter = ctx.accounts.counter.count;
        ctx.accounts.counter.count += inc_item;
        msg!("counter owner: {}", ctx.accounts.counter_owner.key);
        msg!("counter = {} + {} = {}", counter, inc_item, ctx.accounts.counter.count);
        Ok(())
    }
    /// Example, call propose within the entangle multichain environment
    pub fn propose_to_other_chain(ctx: Context<ProposeToOtherChain>) -> Result<()> {
        let protocol_id: Vec<u8> = PROTOCOL_ID.to_vec();
        let dst_chain_id = 33133_u128;
        let protocol_address: Vec<u8> = vec![1; 20];
        let function_selector =
            FunctionSelector::ByName("ask1234mkl;1mklasdfasm;lkasdmf__".to_owned());
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

        // TODO: implement the `receive_photon_msg` to check if the code based function_selector works well
    }

    pub fn propose_to_other_chain_big_selector(ctx: Context<ProposeToOtherChain>) -> Result<()> {
        let protocol_id: Vec<u8> = PROTOCOL_ID.to_vec();
        let dst_chain_id = 33133_u128;
        let protocol_address: Vec<u8> = vec![1; 20];
        let function_selector = FunctionSelector::ByCode(vec![1_u8; 33]);
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

        // TODO: implement the `receive_photon_msg` to check if the code based function_selector works well
    }

    pub fn receive_photon_msg(
        _ctx: Context<ReceivePhotonMsg>,
        _op_hash: Vec<u8>,
        code: Vec<u8>,
        _params: Vec<u8>,
    ) -> Result<()> {
        msg!("photon msg receive, code: {:?}", code);
        Ok(())
    }
}

#[derive(Accounts)]
pub struct Initialize<'info> {
    /// Owner account
    #[account(signer, mut)]
    owner: Signer<'info>,

    /// Endpoint call authority address for the protocol
    /// CHECK: if not loaded
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
pub struct InitOwnedCounter<'info> {
    /// Executor account
    #[account(signer, mut)]
    executor: Signer<'info>,

    /// Endpoint call authority address for the protocol
    #[account(signer)]
    call_authority: Signer<'info>,

    /// Operation info
    #[account()]
    op_info: Account<'info, OpInfo>,

    /// Account that owns and determines which counter to be incremented
    #[account(signer)]
    counter_owner: Signer<'info>,

    /// Specific counter that is owned by counter_owner
    #[account(
        init,
        space = 8 + 8 + 32,
        payer = executor,
        seeds = [b"COUNTER", counter_owner.key().as_ref()],
        bump,
    )]
    owned_counter: Box<Account<'info, Counter>>,

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

    /// Operation info
    #[account()]
    op_info: Account<'info, OpInfo>,

    /// Counter
    #[account(
        mut,
        seeds = [b"COUNTER"],
        bump
    )]
    counter: Box<Account<'info, Counter>>,
}

#[derive(Accounts)]
pub struct ToBeFailed<'info> {
    /// Protocol executor
    #[account(signer, mut)]
    executor: Signer<'info>,

    /// Owner account
    #[account(signer)]
    call_authority: Signer<'info>,

    /// Operation info
    #[account()]
    op_info: Account<'info, OpInfo>,
}

#[derive(Accounts)]
pub struct IncrementOwnedCounter<'info> {
    /// Protocol executor
    #[account(signer, mut)]
    executor: Signer<'info>,

    /// entangle authority account
    #[account(signer)]
    call_authority: Signer<'info>,

    /// Operation info
    #[account()]
    op_info: Account<'info, OpInfo>,

    /// account that owns and determines which counter to be incremented
    #[account(signer)]
    counter_owner: Signer<'info>,

    /// Counter
    #[account(
        mut,
        seeds = [b"COUNTER", counter_owner.key().as_ref()],
        bump,
        constraint = call_authority.key() == counter.call_authority
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

fn decode_increment_item(params: Vec<u8>) -> u64 {
    ethabi::decode(&[ParamType::Uint(256)], &params)
        .expect("Expected params to be decoded as ethabi tokens")
        .first()
        .expect("Expected params to consist of at least one token")
        .clone()
        .into_uint()
        .expect("Expected params first token to be uint")
        .as_u64()
}

#[derive(Accounts)]
pub struct ReceivePhotonMsg<'info> {
    #[account(signer, mut)]
    executor: Signer<'info>,
}
