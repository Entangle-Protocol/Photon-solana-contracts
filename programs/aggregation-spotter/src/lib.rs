mod gov;
pub mod interface;
mod signature;
mod util;
use anchor_lang::prelude::*;
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

    use crate::{
        interface::{PhotonMsg, PhotonMsgWithSelector},
        util::sighash,
    };

    use self::{gov::handle_gov_operation, signature::ecrecover};

    use super::*;

    pub fn initialize(ctx: Context<Initialize>, eob_chain_id: u64) -> Result<()> {
        ctx.accounts.config.owner = ctx.accounts.owner.key();
        ctx.accounts.config.admin = ctx.accounts.owner.key();
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
        require_eq!(
            ctx.accounts.config.nonce,
            op_data.nonce,
            CustomError::InvalidNonce
        );
        require!(
            op_data.protocol_id != [0; 32]
                && op_data.protocol_id.len() == 32
                && op_data.src_op_tx_id.len() == 32,
            CustomError::InvalidOpData
        );
        ctx.accounts.op_info.op_data = op_data;
        ctx.accounts.op_info.status = OpStatus::Init;
        ctx.accounts.config.nonce += 1;
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

    pub fn execute_operation<'a, 'b, 'c, 'info>(
        ctx: Context<'a, 'b, 'c, 'info, ExecuteOperation<'info>>,
        op_hash: Vec<u8>,
        call_authority_bump: u8,
    ) -> Result<()> {
        let _ = op_hash;
        let op_data = &ctx.accounts.op_info.op_data;
        require!(
            op_data.protocol_id != gov_protocol_id() && op_data.protocol_addr != photon::ID,
            CustomError::InvalidEndpoint
        );
        // The first account in remaining_accounts should be protocol address, which is added first in account list
        let mut accounts: Vec<_> = ctx.remaining_accounts.get(0).into_iter().cloned().collect();
        require!(
            accounts
                .get(0)
                .filter(|x| x.key() == op_data.protocol_addr)
                .is_some(),
            CustomError::ProtocolAddressNotProvided
        );
        // The second in account list is executor
        accounts.push(ctx.accounts.executor.to_account_info().clone());
        // The third in account list is call authority
        let mut call_authority = ctx.accounts.call_authority.to_account_info().clone();
        call_authority.is_signer = true;
        accounts.push(call_authority);
        // And then the other accounts for protocol instruction
        if ctx.remaining_accounts.len() > 1 {
            accounts.extend_from_slice(&ctx.remaining_accounts[1..]);
        }
        let metas: Vec<_> = accounts
            .iter()
            .filter(|x| x.key() != op_data.protocol_addr)
            .map(|x| x.to_account_metas(None).get(0).unwrap().clone())
            .collect();
        let (method, payload) =
            if op_data.function_selector.len() == 5 && op_data.function_selector[4] == 0 {
                let payload = PhotonMsgWithSelector {
                    protocol_id: op_data.protocol_id.clone(),
                    src_chain_id: op_data.src_chain_id,
                    src_block_number: op_data.src_block_number,
                    src_op_tx_id: op_data.src_op_tx_id.clone(),
                    function_selector: op_data.function_selector[..4].to_vec(),
                    params: op_data.params.clone(),
                };
                ("photon_msg".to_owned(), payload.try_to_vec().unwrap())
            } else {
                let payload = PhotonMsg {
                    protocol_id: op_data.protocol_id.clone(),
                    src_chain_id: op_data.src_chain_id,
                    src_block_number: op_data.src_block_number,
                    src_op_tx_id: op_data.src_op_tx_id.clone(),
                    params: op_data.params.clone(),
                };
                (
                    String::from_utf8(op_data.function_selector.clone())
                        .map_err(|_| CustomError::InvalidMethodSelector)?,
                    payload.try_to_vec().unwrap(),
                )
            };
        let data = [&sighash("global", &method)[..], &payload[..]].concat();
        let instr = Instruction::new_with_bytes(op_data.protocol_addr, &data, metas);
        let err = invoke_signed(
            &instr,
            &accounts,
            &[&[
                ROOT,
                b"CALL_AUTHORITY",
                &op_data.protocol_id,
                &[call_authority_bump],
            ]],
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
        function_selector: Vec<u8>,
        params: Vec<u8>,
    ) -> Result<()> {
        require!(
            function_selector.len() <= 32,
            CustomError::InvalidMethodSelector
        );
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

    /// Initial config
    #[account(init_if_needed, payer = owner, space = Config::LEN, seeds = [ROOT.as_ref(), b"CONFIG"], bump)]
    config: Box<Account<'info, Config>>,

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

    /// Initial config
    #[account(mut, seeds = [ROOT, b"CONFIG"], bump)]
    config: Box<Account<'info, Config>>,

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
#[instruction(op_hash: Vec<u8>, call_authority_bump: u8)]
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

    /// Per-protocol call authority
    /// CHECK: only used as authority account
    #[account(
        seeds = [ROOT, b"CALL_AUTHORITY", &op_info.op_data.protocol_id],
        bump = call_authority_bump
    )]
    call_authority: AccountInfo<'info>,
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
    consensus_target_rate: u64,
    protocol_address: Pubkey,
    keepers: Box<[EthAddress; 20]>, // cannot use const with anchor
    executors: Box<[Pubkey; 20]>,
    proposers: Box<[Pubkey; 20]>,
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
    function_selector: Vec<u8>,
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
    #[msg("InvalidMethodSelector")]
    InvalidMethodSelector,
    #[msg("InvalidOpData")]
    InvalidOpData,
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
