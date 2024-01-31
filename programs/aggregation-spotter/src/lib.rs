mod gov;
mod signature;
mod util;
use anchor_lang::prelude::*;
use anchor_spl::token::{self, Mint, Token, TokenAccount};
use signature::{KeeperSignature, OperationData};
use util::{gov_protocol_id, EthAddress, OpStatus};

declare_id!("9pGziQeWKwruehVXiF9ZHToiVs9iv7ajXeFFaPiaLkpD");

#[program]
pub mod photon {
    pub const SOLANA_CHAIN_ID: u128 = 111111111;
    pub const RATE_DECIMALS: u64 = 10000;
    pub const ROOT: &[u8] = b"root-0";
    pub const MAX_KEEPERS: usize = 20;
    pub const MAX_EXECUTORS: usize = 20;
    pub const MAX_PROPOSERS: usize = 20;

    use anchor_lang::solana_program::{instruction::Instruction, program::invoke_signed};
    use anchor_spl::token::Transfer;

    use self::{gov::handle_gov_operation, signature::ecrecover};

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
    ) -> Result<()> {
        let op_hash = op_data.op_hash_with_message();
        require!(op_hash == op_hash_cached, CustomError::CachedOpHashMismatch);
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
        /*require_eq!(
            ctx.accounts.config.nonce,
            op_data.nonce,
            CustomError::InvalidNonce
        );*/
        require!(
            op_data.protocol_id != [0; 32]
                && op_data.protocol_id.len() == 32
                && op_data.src_op_tx_id.len() == 32,
            CustomError::InvalidOpData
        );
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
        ctx.accounts.op_info.op_data = op_data;
        ctx.accounts.op_info.status = OpStatus::Init;
        emit!(ProposalCreated {
            op_hash,
            executor: ctx.accounts.executor.key()
        });
        Ok(())
    }

    pub fn sign_operation(
        ctx: Context<SignOperation>,
        op_hash: Vec<u8>,
        signatures: Vec<KeeperSignature>,
    ) -> Result<bool> {
        let allowed_keepers = &ctx.accounts.protocol_info.keepers();
        require_gt!(allowed_keepers.len(), 0, CustomError::NoKeepersAllowed);
        let mut unique_signers: Vec<EthAddress> = ctx
            .accounts
            .op_info
            .unique_signers
            .into_iter()
            .filter(|x| x != &EthAddress::default())
            .collect();
        let mut consensus_reached = ((unique_signers.len() as u64) * RATE_DECIMALS)
            / (allowed_keepers.len() as u64)
            >= ctx.accounts.protocol_info.consensus_target_rate;
        if consensus_reached {
            return Ok(true);
        }
        for sig in signatures {
            let keeper = ecrecover(&op_hash, &sig)?;
            if allowed_keepers.contains(&keeper) && !unique_signers.contains(&keeper) {
                unique_signers.push(keeper);
                let consensus_rate = ((unique_signers.len() as u64) * RATE_DECIMALS)
                    / (allowed_keepers.len() as u64);
                if consensus_rate >= ctx.accounts.protocol_info.consensus_target_rate {
                    consensus_reached = true;
                    ctx.accounts.op_info.status = OpStatus::Signed;
                    emit!(ProposalApproved {
                        op_hash,
                        executor: ctx.accounts.executor.key()
                    });
                    break;
                }
            }
        }
        ctx.accounts.op_info.unique_signers = Default::default();
        for (i, s) in unique_signers.into_iter().enumerate() {
            ctx.accounts.op_info.unique_signers[i] = s;
        }
        Ok(consensus_reached)
    }

    pub fn execute_operation(ctx: Context<ExecuteOperation>, op_hash: Vec<u8>) -> Result<()> {
        let _ = op_hash;
        let op_data = &ctx.accounts.op_info.op_data;
        require!(
            op_data.protocol_id != gov_protocol_id() && op_data.protocol_addr != photon::ID,
            CustomError::InvalidEndpoint
        );
        let (call_authority, bump) = Pubkey::find_program_address(
            &[ROOT, b"CALL_AUTHORITY", &op_data.protocol_id],
            &photon::ID,
        );
        require!(
            ctx.remaining_accounts
                .into_iter()
                .next()
                .filter(|x| x.key() == op_data.protocol_addr)
                .is_some(),
            CustomError::ProtocolAddressNotProvided
        );
        require!(
            ctx.remaining_accounts
                .into_iter()
                .find(|x| x.key() == call_authority)
                .is_some(),
            CustomError::CallAuthorityNotProvided
        );
        let remaining_accounts: Vec<_> = ctx
            .remaining_accounts
            .into_iter()
            .cloned()
            .map(|mut x| {
                if x.key() == call_authority {
                    x.is_signer = true;
                }
                x
            })
            .collect();
        let metas: Vec<_> = remaining_accounts
            .iter()
            .filter(|x| x.key() != op_data.protocol_addr)
            .map(|x| {
                x.to_account_metas(if x.key() == call_authority {
                    Some(true)
                } else {
                    None
                })
                .into_iter()
                .next()
                .unwrap()
            })
            .collect();
        let instr = Instruction::new_with_bytes(op_data.protocol_addr, &op_data.params, metas);
        let err = invoke_signed(
            &instr,
            &remaining_accounts,
            &[&[ROOT, b"CALL_AUTHORITY", &op_data.protocol_id, &[bump]]],
        )
        .map_err(|e| format!("{}", e))
        .err();
        emit!(ProposalExecuted {
            op_hash,
            err,
            executor: ctx.accounts.executor.key()
        });
        Ok(())
    }

    pub fn execute_gov_operation(
        ctx: Context<ExecuteGovOperation>,
        op_hash: Vec<u8>,
        target_protocol: Vec<u8>,
    ) -> Result<()> {
        let _ = op_hash;
        let op_data = ctx.accounts.op_info.op_data.clone();
        require!(
            op_data.protocol_id == gov_protocol_id() && op_data.protocol_addr == photon::ID,
            CustomError::InvalidEndpoint
        );
        let executor = ctx.accounts.executor.key();
        let err = handle_gov_operation(ctx, op_data, target_protocol)
            .map_err(|e| format!("{}", e))
            .err();
        emit!(ProposalExecuted {
            op_hash,
            err,
            executor
        });
        Ok(())
    }

    pub fn propose(
        _ctx: Context<Propose>,
        protocol_id: Vec<u8>,
        nonce: u128,
        dst_chain_id: u128,
        protocol_address: Vec<u8>,
        function_selector: u32,
        params: Vec<u8>,
    ) -> Result<()> {
        emit!(ProposeEvent {
            protocol_id,
            nonce,
            dst_chain_id,
            protocol_address,
            function_selector,
            params
        });
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
    #[account(
        signer,
        mut,
        constraint = protocol_info.executors.contains(&executor.key()) @ CustomError::ExecutorIsNotAllowed
    )]
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
        space = OpInfo::len(&op_data),
        seeds = [ROOT, b"OP", &op_hash_cached],
        bump,
        constraint = op_info.status == OpStatus::None @ CustomError::OpStateInvalid,
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
#[instruction(op_hash: Vec<u8>)]
pub struct SignOperation<'info> {
    /// Executor account
    #[account(
        signer,
        mut,
        constraint = protocol_info.executors.contains(&executor.key()) @ CustomError::ExecutorIsNotAllowed
    )]
    executor: Signer<'info>,

    /// Operation info
    #[account(
        mut,
        seeds = [ROOT, b"OP", &op_hash],
        bump,
        constraint = (op_info.status == OpStatus::Init || op_info.status == OpStatus::Signed) @ CustomError::OpStateInvalid
    )]
    op_info: Box<Account<'info, OpInfo>>,

    /// Protocol info
    #[account(
        seeds = [ROOT, b"PROTOCOL", &op_info.op_data.protocol_id],
        bump
    )]
    protocol_info: Box<Account<'info, ProtocolInfo>>,
}

#[derive(Accounts)]
#[instruction(op_hash: Vec<u8>)]
pub struct ExecuteOperation<'info> {
    /// Executor account
    #[account(
        signer,
        mut,
        constraint = protocol_info.executors.contains(&executor.key()) @ CustomError::ExecutorIsNotAllowed
    )]
    executor: Signer<'info>,

    /// Operation info
    #[account(
        mut,
        seeds = [ROOT, b"OP", &op_hash],
        bump,
        constraint = op_info.status == OpStatus::Signed @ CustomError::OpStateInvalid
    )]
    op_info: Box<Account<'info, OpInfo>>,

    /// Protocol info
    #[account(
        seeds = [ROOT, b"PROTOCOL", &op_info.op_data.protocol_id],
        bump
    )]
    protocol_info: Box<Account<'info, ProtocolInfo>>,
}

#[derive(Accounts)]
#[instruction(op_hash: Vec<u8>, target_protocol: Vec<u8>)]
pub struct ExecuteGovOperation<'info> {
    /// Executor account
    #[account(
        signer,
        mut,
        constraint = gov_info.executors.contains(&executor.key()) @ CustomError::ExecutorIsNotAllowed
    )]
    executor: Signer<'info>,

    /// Operation info
    #[account(
        mut,
        seeds = [ROOT, b"OP", &op_hash],
        bump,
        constraint = op_info.status == OpStatus::Signed @ CustomError::OpStateInvalid
    )]
    op_info: Box<Account<'info, OpInfo>>,

    /// Gov protocol info
    #[account(
        seeds = [ROOT, b"PROTOCOL", &gov_protocol_id()],
        bump
    )]
    gov_info: Box<Account<'info, ProtocolInfo>>,

    /// Target protocol info
    #[account(
        init_if_needed,
        space = ProtocolInfo::LEN,
        payer = executor,
        seeds = [ROOT, b"PROTOCOL", &target_protocol],
        bump
    )]
    protocol_info: Box<Account<'info, ProtocolInfo>>,

    /// System program
    system_program: Program<'info, System>,
}

#[derive(Accounts)]
#[instruction(protocol_id: Vec<u8>)]
pub struct Propose<'info> {
    /// Proposer account
    #[account(
        signer,
        constraint = protocol_info.proposers.contains(&proposer.key()) @ CustomError::ProposerIsNotAllowed
    )]
    proposer: Signer<'info>,

    /// Target protocol info
    #[account(
        seeds = [ROOT, b"PROTOCOL", &protocol_id],
        bump
    )]
    protocol_info: Box<Account<'info, ProtocolInfo>>,
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
    keepers: Box<[EthAddress; MAX_KEEPERS]>,
    executors: Box<[Pubkey; MAX_EXECUTORS]>,
    proposers: Box<[Pubkey; MAX_PROPOSERS]>,
}

impl ProtocolInfo {
    pub const LEN: usize =
        8 + 1 + 8 * 2 + 32 + (20 * MAX_KEEPERS) + (32 * MAX_EXECUTORS) + (32 * MAX_PROPOSERS);

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

    pub fn proposers(&self) -> Vec<Pubkey> {
        self.proposers
            .into_iter()
            .take_while(|k| k != &Pubkey::default())
            .collect()
    }
}

#[account]
#[derive(Default)]
pub struct OpInfo {
    status: OpStatus,
    unique_signers: [EthAddress; 16],
    op_data: OperationData,
}

impl OpInfo {
    pub fn len(op_data: &OperationData) -> usize {
        8 + 1 + 20 * 16 + borsh::to_vec(op_data).unwrap().len()
    }
}

#[event]
pub struct ProposalCreated {
    op_hash: Vec<u8>,
    executor: Pubkey,
}

#[event]
pub struct ProposalApproved {
    op_hash: Vec<u8>,
    executor: Pubkey,
}

#[event]
pub struct ProposalExecuted {
    op_hash: Vec<u8>,
    err: Option<String>,
    executor: Pubkey,
}

#[event]
pub struct ProposeEvent {
    protocol_id: Vec<u8>,
    nonce: u128,
    dst_chain_id: u128,
    protocol_address: Vec<u8>,
    function_selector: u32,
    params: Vec<u8>,
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
    #[msg("OpStateInvalid")]
    OpStateInvalid,
    #[msg("CachedOpHashMismatch")]
    CachedOpHashMismatch,
    #[msg("ProtocolAddressMismatch")]
    ProtocolAddressMismatch,
    #[msg("TargetProtocolMismatch")]
    TargetProtocolMismatch,
    #[msg("ExecutorIsNotAllowed")]
    ExecutorIsNotAllowed,
    #[msg("ProposerIsNotAllowed")]
    ProposerIsNotAllowed,
    #[msg("OperationNotApproved")]
    OperationNotApproved,
    #[msg("InvalidProtoMsg")]
    InvalidProtoMsg,
    #[msg("InvalidGovMsg")]
    InvalidGovMsg,
    #[msg("InvalidGovMethod")]
    InvalidGovMethod,
    #[msg("InvalidOpData")]
    InvalidOpData,
    #[msg("CallAuthorityNotProvided")]
    CallAuthorityNotProvided,
    #[msg("ProtocolAddressNotProvided")]
    ProtocolAddressNotProvided,
    #[msg("NoKeepersAllowed")]
    NoKeepersAllowed,
    #[msg("MaxKeepersExceeded")]
    MaxKeepersExceeded,
    #[msg("MaxExecutorsExceeded")]
    MaxExecutorsExceeded,
    #[msg("MaxProposersExceeded")]
    MaxProposersExceeded,
}
