mod gov;
mod signature;
mod util;
use anchor_lang::prelude::*;
use anchor_spl::token::{self, Mint, Token, TokenAccount};
use signature::{KeeperSignature, OperationData};
use util::{gov_protocol_id, Bytes32, EthAddress, OpStatus};

declare_id!("CQ9k5uzuZycAHiAiLrabRdNYi4JuHWCHieTxJxFk1vH8");

#[program]
pub mod photon {
    pub const SOLANA_CHAIN_ID: u128 = 111111111;
    pub const RATE_DECIMALS: u64 = 10000;
    pub const ROOT: &[u8] = b"root-0";

    use anchor_lang::{
        prelude::borsh::maybestd::collections::HashSet,
        solana_program::{instruction::Instruction, program::invoke_signed},
    };
    use anchor_spl::token::Transfer;

    use crate::{gov::handle_gov_operation, signature::derive_eth_address};

    use super::*;

    pub fn initialize(ctx: Context<Initialize>, eob_chain_id: u64) -> Result<()> {
        ctx.accounts.config.owner = ctx.accounts.owner.key();
        ctx.accounts.config.admin = ctx.accounts.owner.key();
        ctx.accounts.config.fee_collector_vault = ctx.accounts.fee_collector_vault.key();
        ctx.accounts.config.ngl_mint = ctx.accounts.ngl_mint.key();
        ctx.accounts.config.eob_chain_id = eob_chain_id;
        ctx.accounts.config.nonce = 0;
        Ok(())
    }

    pub fn init_gov_protocol(
        ctx: Context<InitGovProtocol>,
        consensus_target_rate: u64,
        gov_keepers: Vec<EthAddress>,
        gov_executors: Vec<Pubkey>,
    ) -> Result<()> {
        ctx.accounts.protocol_info.is_init = true;
        ctx.accounts.protocol_info.protocol_address = photon::ID;
        ctx.accounts.protocol_info.consensus_target_rate = consensus_target_rate;
        ctx.accounts.protocol_info.keepers = Default::default();
        for (i, k) in gov_keepers.into_iter().enumerate() {
            ctx.accounts.protocol_info.keepers[i] = k;
        }
        ctx.accounts.protocol_info.executors = Default::default();
        for (i, e) in gov_executors.into_iter().enumerate() {
            ctx.accounts.protocol_info.executors[i] = e;
        }
        Ok(())
    }

    pub fn load_operation(
        ctx: Context<LoadOperation>,
        op_data: OperationData,
        op_hash_cached: Vec<u8>,
        keeper_sigs: Vec<KeeperSignature>,
        keeper_pubkeys: Vec<Vec<u8>>,
    ) -> Result<()> {
        require!(
            ctx.accounts
                .protocol_info
                .executors
                .contains(&ctx.accounts.executor.key()),
            CustomError::ExecutorIsNotAllowed
        );
        let op_hash = op_data.op_hash_with_message();
        require!(
            &op_hash == &op_hash_cached,
            CustomError::CachedOpHashMismatch
        );
        require_eq!(
            op_data.dest_chain_id,
            SOLANA_CHAIN_ID,
            CustomError::OpIsNotForThisChain
        );
        require_eq!(
            ctx.accounts.protocol_info.protocol_address,
            op_data.protocol_addr,
            CustomError::ProtocolAddressMismatch
        );
        require_eq!(
            ctx.accounts.config.nonce,
            op_data.nonce,
            CustomError::InvalidNonce
        );
        let mut unique_signers = HashSet::new();
        let allowed_keepers = ctx.accounts.protocol_info.keepers();
        let mut consensus_reached = false;
        for (pubkey, _sig) in keeper_pubkeys.into_iter().zip(keeper_sigs.into_iter()) {
            let keeper = derive_eth_address(&pubkey)?;
            /*if !check_signature(&pubkey, &sig, &op_hash)? {
                continue;
            }*/
            if allowed_keepers.contains(&keeper) && !unique_signers.contains(&keeper) {
                unique_signers.insert(keeper);
                let consensus_rate = ((unique_signers.len() as u64) * RATE_DECIMALS)
                    / (allowed_keepers.len() as u64);
                if consensus_rate >= ctx.accounts.protocol_info.consensus_target_rate {
                    consensus_reached = true;
                    break;
                }
            }
        }
        require!(consensus_reached, CustomError::OperationNotApproved);
        if ctx.accounts.protocol_info.protocol_fee > 0 {
            let cpi_accounts = Transfer {
                from: ctx.accounts.executor_ngl_vault.to_account_info(),
                to: ctx.accounts.fee_collector_vault.to_account_info(),
                authority: ctx.accounts.executor.clone().to_account_info(),
            };
            let cpi_program = ctx.accounts.token_program.clone();
            let cpi_ctx = CpiContext::new(cpi_program.to_account_info(), cpi_accounts);
            token::transfer(cpi_ctx, ctx.accounts.protocol_info.protocol_fee)?;
        }
        ctx.accounts.config.nonce += 1;
        ctx.accounts.op_info.status = OpStatus::Pending;
        ctx.accounts.op_info.executor = ctx.accounts.executor.key();
        Ok(())
    }

    pub fn execute_operation(
        ctx: Context<ExecuteOperation>,
        op_data: OperationData,
        op_hash_cached: Vec<u8>,
    ) -> Result<()> {
        let op_hash = op_data.op_hash_with_message();
        require!(op_hash == op_hash_cached, CustomError::CachedOpHashMismatch);
        require!(
            op_data.protocol_id != gov_protocol_id(),
            CustomError::InvalidEndpoint
        );
        require!(
            op_data.protocol_addr != photon::ID,
            CustomError::InvalidEndpoint
        );
        let (call_authority, _) = Pubkey::find_program_address(
            &[ROOT, b"CALL_AUTHORITY", &op_data.protocol_id],
            &photon::ID,
        );
        require!(
            ctx.remaining_accounts
                .into_iter()
                .find(|x| x.key() == call_authority)
                .is_some(),
            CustomError::CallAuthorityNotProvided
        );
        let metas: Vec<_> = ctx
            .remaining_accounts
            .into_iter()
            .map(|x| {
                x.to_account_metas(if x.key() == call_authority {
                    Some(true)
                } else {
                    None
                })
                .into_iter()
            })
            .flatten()
            .collect();
        let instr = Instruction::new_with_bytes(op_data.protocol_addr, &op_data.params, metas);
        invoke_signed(
            &instr,
            &ctx.remaining_accounts,
            &[&[ROOT, b"CALL_AUTHORITY"]],
        )?;
        Ok(())
    }

    pub fn execute_gov_operation(
        ctx: Context<ExecuteGovOperation>,
        op_data: OperationData,
        op_hash_cached: Vec<u8>,
        target_protocol: Vec<u8>,
    ) -> Result<()> {
        let op_hash = op_data.op_hash_with_message();
        require!(op_hash == op_hash_cached, CustomError::CachedOpHashMismatch);
        require!(
            op_data.protocol_id == gov_protocol_id(),
            CustomError::InvalidEndpoint
        );
        require!(
            op_data.protocol_addr == photon::ID,
            CustomError::ProtocolAddressMismatch
        );
        handle_gov_operation(ctx, op_data, target_protocol)
    }

    pub fn cancel_operation(ctx: Context<CancelOperation>, _op_hash: Bytes32) -> Result<()> {
        ctx.accounts.op_info.status = OpStatus::Canceled;
        Ok(())
    }
}

#[derive(Accounts)]
pub struct Initialize<'info> {
    /// Owner account
    #[account(signer, mut)]
    owner: Signer<'info>,

    /// NGL token mint
    ngl_mint: Box<Account<'info, Mint>>,

    /// Fee collector
    fee_collector_vault: Box<Account<'info, TokenAccount>>,
    /// Initial config
    #[account(init_if_needed, payer = owner, space = Config::LEN, seeds = [ROOT.as_ref(), b"CONFIG"], bump)]
    config: Box<Account<'info, Config>>,

    /// Token program
    token_program: Program<'info, Token>,
    /// System program
    system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct InitGovProtocol<'info> {
    /// Admin account
    #[account(signer, mut, constraint = admin.key() == config.admin @ CustomError::IsNotAdmin)]
    admin: Signer<'info>,

    /// Protocol info
    #[account(
        init_if_needed,
        payer = admin,
        space = ProtocolInfo::LEN,
        seeds = [ROOT, b"PROTOCOL", &gov_protocol_id()],
        bump
    )]
    protocol_info: Box<Account<'info, ProtocolInfo>>,

    /// Initial config
    #[account(seeds = [ROOT, b"CONFIG"], bump)]
    config: Box<Account<'info, Config>>,

    /// System program
    system_program: Program<'info, System>,
}

#[derive(Accounts)]
#[instruction(op_data: OperationData, op_hash_cached: Vec<u8>)]
pub struct LoadOperation<'info> {
    /// Executor account
    #[account(signer, mut)]
    executor: Signer<'info>,

    /// Protocol info
    #[account(
        seeds = [ROOT, b"PROTOCOL", &op_data.protocol_id],
        bump
    )]
    protocol_info: Box<Account<'info, ProtocolInfo>>,

    /// Operation info
    #[account(
        init,
        payer = executor,
        space = OpInfo::LEN,
        seeds = [ROOT, b"OP", &op_hash_cached],
        bump,
        constraint = op_info.status == OpStatus::None @ CustomError::OpIsAlreadyLoaded
    )]
    op_info: Box<Account<'info, OpInfo>>,

    /// NGL token mint
    #[account(constraint = config.ngl_mint == ngl_mint.key())]
    ngl_mint: Box<Account<'info, Mint>>,

    /// NGL token wallet to take fee from
    #[account(
        token::mint = ngl_mint,
        token::authority = executor
    )]
    executor_ngl_vault: Box<Account<'info, TokenAccount>>,

    /// Fee collector
    #[account(
        token::mint = ngl_mint,
        constraint = fee_collector_vault.key() == config.fee_collector_vault @ CustomError::InvalidFeeCollector
    )]
    fee_collector_vault: Box<Account<'info, TokenAccount>>,

    /// Initial config
    #[account(mut, seeds = [ROOT, b"CONFIG"], bump)]
    config: Box<Account<'info, Config>>,

    /// Token program
    token_program: Program<'info, Token>,
    /// System program
    system_program: Program<'info, System>,
}

#[derive(Accounts)]
#[instruction(op_data: OperationData, op_hash_cached: Vec<u8>)]
pub struct ExecuteOperation<'info> {
    /// Executor account
    #[account(signer, mut, constraint = executor.key() == op_info.executor @ CustomError::ExecutorIsNotAllowed)]
    executor: Signer<'info>,

    /// Operation info
    #[account(
        seeds = [ROOT, b"OP", &op_hash_cached],
        bump,
        constraint = op_info.status == OpStatus::Pending @ CustomError::OpIsAlreadyExecuted
    )]
    op_info: Box<Account<'info, OpInfo>>,
    /*/// Authority used for external calls
    /// CHECK: not loaded
    #[account(seeds = [ROOT, b"CALL_AUTHORITY"], bump)]
    call_authority: AccountInfo<'info>,*/
}

#[derive(Accounts)]
#[instruction(op_data: OperationData, op_hash_cached: Vec<u8>, target_protocol: Vec<u8>)]
pub struct ExecuteGovOperation<'info> {
    /// Executor account
    #[account(signer, mut, constraint = executor.key() == op_info.executor @ CustomError::ExecutorIsNotAllowed)]
    executor: Signer<'info>,

    /// Protocol info
    #[account(
        init_if_needed,
        space = ProtocolInfo::LEN,
        payer = executor,
        seeds = [ROOT, b"PROTOCOL", &target_protocol],
        bump
    )]
    protocol_info: Box<Account<'info, ProtocolInfo>>,

    /// Operation info
    #[account(
        seeds = [ROOT, b"OP", &op_hash_cached],
        bump,
        constraint = op_info.status == OpStatus::Pending @ CustomError::OpIsAlreadyExecuted
    )]
    op_info: Box<Account<'info, OpInfo>>,

    /// System program
    system_program: Program<'info, System>,
}

#[derive(Accounts)]
#[instruction(_op_hash: Bytes32)]
pub struct CancelOperation<'info> {
    /// Executor account
    #[account(signer, mut, constraint = executor.key() == op_info.executor @ CustomError::ExecutorIsNotAllowed)]
    executor: Signer<'info>,

    /// Operation info
    #[account(
        seeds = [ROOT, b"OP", &_op_hash],
        bump,
        constraint = op_info.status == OpStatus::Pending @ CustomError::OpIsAlreadyExecuted
    )]
    op_info: Box<Account<'info, OpInfo>>,
}

#[account]
#[derive(Default)]
pub struct Config {
    owner: Pubkey,
    admin: Pubkey,
    fee_collector_vault: Pubkey,
    ngl_mint: Pubkey,
    eob_chain_id: u64,
    nonce: u64,
}

impl Config {
    pub const LEN: usize = 8 + 32 * 4 + 8 * 2;
}

#[account]
#[derive(Default)]
pub struct ProtocolInfo {
    is_init: bool,
    protocol_fee: u64,
    consensus_target_rate: u64,
    protocol_address: Pubkey,
    keepers: [EthAddress; 32],
    executors: [Pubkey; 32],
}

impl ProtocolInfo {
    pub const LEN: usize = 8 + 1 + 8 * 2 + 32 + 20 * 32 + 32 * 32;

    pub fn keepers(&self) -> Vec<EthAddress> {
        self.keepers
            .into_iter()
            .take_while(|k| k != &EthAddress::default())
            .collect()
    }

    pub fn executors(&self) -> Vec<Pubkey> {
        self.executors
            .into_iter()
            .take_while(|k| k != &Pubkey::default())
            .collect()
    }
}

#[account]
#[derive(Default)]
pub struct OpInfo {
    status: OpStatus,
    executor: Pubkey,
}

impl OpInfo {
    pub const LEN: usize = 8 + 1 + 32;
}

#[error_code]
pub enum CustomError {
    #[msg("Is not admin")]
    IsNotAdmin,
    #[msg("Protocol not init")]
    ProtocolNotInit,
    #[msg("Invalid signature")]
    InvalidSignature,
    #[msg("OpIsNotForThisChain")]
    OpIsNotForThisChain,
    #[msg("InvalidNonce")]
    InvalidNonce,
    #[msg("InvalidEndpoint")]
    InvalidEndpoint,
    #[msg("InvalidFeeCollector")]
    InvalidFeeCollector,
    #[msg("OpIsAlreadyLoaded")]
    OpIsAlreadyLoaded,
    #[msg("OpIsAlreadyExecuted")]
    OpIsAlreadyExecuted,
    #[msg("CachedOpHashMismatch")]
    CachedOpHashMismatch,
    #[msg("ProtocolAddressMismatch")]
    ProtocolAddressMismatch,
    #[msg("TargetProtocolMismatch")]
    TargetProtocolMismatch,
    #[msg("ExecutorIsNotAllowed")]
    ExecutorIsNotAllowed,
    #[msg("OperationNotApproved")]
    OperationNotApproved,
    #[msg("InvalidProtoMsg")]
    InvalidProtoMsg,
    #[msg("InvalidGovMsg")]
    InvalidGovMsg,
    #[msg("CallAuthorityNotProvided")]
    CallAuthorityNotProvided,
}
