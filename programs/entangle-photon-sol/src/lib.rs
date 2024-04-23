#![feature(extend_one)]
#![feature(slice_first_last_chunk)]

pub mod interface;
pub mod signature;
pub mod util;

pub mod gov;

use anchor_lang::prelude::*;
use signature::{OperationData, TransmitterSignature};
use util::{gov_protocol_id, EthAddress, OpStatus};

declare_id!("JDxWYX5NrL51oPcYunS7ssmikkqMLcuHn9v4HRnedKHT");

#[program]
pub mod photon {
    pub const SOLANA_CHAIN_ID: u128 = 100000000000000000000;
    pub const RATE_DECIMALS: u64 = 10000;
    pub const ROOT: &[u8] = b"root-0";
    pub const MAX_TRANSMITTERS: usize = 20;
    pub const MAX_EXECUTORS: usize = 20;
    pub const MAX_PROPOSERS: usize = 20;

    use self::{
        gov::handle_gov_operation,
        interface::{PhotonMsg, PhotonMsgWithSelector},
        signature::{ecrecover, FunctionSelector},
        util::sighash,
    };
    use super::*;

    use anchor_lang::solana_program::{instruction::Instruction, program::invoke_signed};

    pub fn initialize(
        ctx: Context<Initialize>,
        eob_chain_id: u64,
        eob_master_smart_contract: Vec<u8>,
        consensus_target_rate: u64,
        gov_transmitters: Vec<EthAddress>,
        gov_executors: Vec<Pubkey>,
    ) -> Result<()> {
        ctx.accounts.config.admin = ctx.accounts.admin.key();
        ctx.accounts.config.eob_chain_id = eob_chain_id;
        require_eq!(eob_master_smart_contract.len(), 32);
        ctx.accounts.config.eob_master_smart_contract.copy_from_slice(&eob_master_smart_contract);
        ctx.accounts.protocol_info.is_init = true;
        ctx.accounts.protocol_info.protocol_address = photon::ID;
        ctx.accounts.protocol_info.consensus_target_rate = consensus_target_rate;
        ctx.accounts.protocol_info.transmitters = Default::default();
        for (i, k) in gov_transmitters.into_iter().enumerate() {
            ctx.accounts.protocol_info.transmitters[i] = k;
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
        require_eq!(op_data.dest_chain_id, SOLANA_CHAIN_ID, CustomError::OpIsNotForThisChain);
        require_eq!(
            ctx.accounts.protocol_info.protocol_address,
            op_data.protocol_addr,
            CustomError::ProtocolAddressMismatch
        );
        require!(
            op_data.protocol_id != [0; 32] && op_data.protocol_id.len() == 32,
            CustomError::InvalidOpData
        );
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
        signatures: Vec<TransmitterSignature>,
    ) -> Result<bool> {
        let allowed_transmitters = &ctx.accounts.protocol_info.transmitters();
        require_gt!(allowed_transmitters.len(), 0, CustomError::NoTransmittersAllowed);
        let mut unique_signers: Vec<EthAddress> = ctx
            .accounts
            .op_info
            .unique_signers
            .into_iter()
            .filter(|x| x != &EthAddress::default())
            .collect();
        let consensus =
            ((unique_signers.len() as u64) * RATE_DECIMALS) / (allowed_transmitters.len() as u64);
        let mut consensus_reached = consensus >= ctx.accounts.protocol_info.consensus_target_rate;
        if consensus_reached {
            return Ok(true);
        }
        for sig in signatures {
            let transmitter = ecrecover(&op_hash, &sig)?;
            if allowed_transmitters.contains(&transmitter) && !unique_signers.contains(&transmitter)
            {
                unique_signers.push(transmitter);
                let consensus_rate = ((unique_signers.len() as u64) * RATE_DECIMALS)
                    / (allowed_transmitters.len() as u64);
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

    pub fn execute_operation<'info>(
        ctx: Context<'_, '_, '_, 'info, ExecuteOperation<'info>>,
        op_hash: Vec<u8>,
    ) -> Result<()> {
        let op_data = &ctx.accounts.op_info.op_data;

        // The first account in remaining_accounts should be protocol address, which is added first in account list
        let mut accounts: Vec<_> = ctx.remaining_accounts.first().into_iter().cloned().collect();
        require!(
            accounts.first().filter(|x| x.key() == op_data.protocol_addr).is_some(),
            CustomError::ProtocolAddressNotProvided
        );
        // The second in account list is executor
        accounts.push(ctx.accounts.executor.to_account_info().clone());
        // The third in account list is call authority
        let mut call_authority = ctx.accounts.call_authority.to_account_info().clone();
        call_authority.is_signer = true;
        accounts.push(call_authority);
        let op_info = ctx.accounts.op_info.to_account_info().clone();
        accounts.push(op_info);
        // And then the other accounts for protocol instruction
        if ctx.remaining_accounts.len() > 1 {
            accounts.extend_from_slice(&ctx.remaining_accounts[1..]);
        }
        let metas: Vec<_> = accounts
            .iter()
            .filter(|x| x.key() != op_data.protocol_addr)
            .map(|x| x.to_account_metas(None).first().expect("always at least one").clone())
            .collect();

        let (method, payload) = match &op_data.function_selector {
            FunctionSelector::ByCode(selector) => {
                let payload = PhotonMsgWithSelector {
                    op_hash: op_hash.clone(),
                    selector: selector.clone(),
                    params: op_data.params.clone(),
                };
                (
                    "receive_photon_msg".to_owned(),
                    payload.try_to_vec().expect("fixed struct serialization"),
                )
            }
            FunctionSelector::ByName(name) => {
                let payload = PhotonMsg {
                    params: op_data.params.clone(),
                };
                (name.clone(), payload.try_to_vec().expect("fixed struct serialization"))
            }
            FunctionSelector::Dummy => panic!("Uninitialized function_selector"),
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
                &[ctx.bumps.call_authority],
            ]],
        )
        .map_err(|e| format!("{}", e))
        .err();

        ctx.accounts.op_info.status = OpStatus::Executed;

        emit!(ProposalExecuted {
            op_hash,
            err,
            executor: ctx.accounts.executor.key()
        });
        Ok(())
    }

    pub fn propose(
        ctx: Context<Propose>,
        protocol_id: Vec<u8>,
        dst_chain_id: u128,
        protocol_address: Vec<u8>,
        function_selector: Vec<u8>,
        params: Vec<u8>,
    ) -> Result<()> {
        // TODO: check if all requirements are satisfied
        require!(function_selector.len() <= 32, CustomError::InvalidMethodSelector);
        let nonce = ctx.accounts.config.nonce;
        ctx.accounts.config.nonce += 1;
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

    pub fn receive_photon_msg(
        ctx: Context<ReceivePhotonMsg>,
        op_hash: Vec<u8>,
        code: Vec<u8>,
        _params: Vec<u8>,
    ) -> Result<()> {
        let op_data = &ctx.accounts.op_info.op_data;
        require!(
            op_data.protocol_id == gov_protocol_id() && op_data.protocol_addr == ID,
            CustomError::InvalidEndpoint
        );
        let executor = ctx.accounts.executor.key();
        let err = handle_gov_operation(
            &mut ctx.accounts.config,
            &mut ctx.accounts.target_protocol_info,
            code,
            op_data,
        )
        .map_err(|e| format!("{}", e))
        .err();

        emit!(ProposalExecuted {
            op_hash,
            err,
            executor
        });

        Ok(())
    }
}

#[derive(Accounts)]
pub struct Initialize<'info> {
    /// Admin account
    #[account(signer, mut, constraint = (admin.key() == config.admin || config.admin == Pubkey::default()) @ CustomError::IsNotAdmin)]
    admin: Signer<'info>,

    /// Protocol info
    #[account(
        init_if_needed,
        payer = admin,
        space = ProtocolInfo::LEN,
        seeds = [ROOT, b"PROTOCOL", gov_protocol_id()],
        bump
    )]
    protocol_info: Box<Account<'info, ProtocolInfo>>,

    /// System config
    #[account(init_if_needed, payer = admin, space = Config::LEN, seeds = [ROOT, b"CONFIG"], bump)]
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

    /// System config
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

    /// Per-protocol call authority
    /// CHECK: only used as authority account
    #[account(
        seeds = [ROOT, b"CALL_AUTHORITY", &op_info.op_data.protocol_id],
        bump
    )]
    call_authority: AccountInfo<'info>,
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

    /// System config
    #[account(mut, seeds = [ROOT, b"CONFIG"], bump)]
    config: Box<Account<'info, Config>>,

    /// Target protocol info
    #[account(
        seeds = [ROOT, b"PROTOCOL", &protocol_id],
        bump
    )]
    protocol_info: Box<Account<'info, ProtocolInfo>>,
}

#[derive(Accounts)]
#[instruction(op_hash: Vec<u8>, code: Vec<u8>, params: Vec<u8>)]
pub struct ReceivePhotonMsg<'info> {
    /// Executor account
    #[account(
        signer,
        mut,
        constraint = gov_info.executors.contains(&executor.key()) @ CustomError::ExecutorIsNotAllowed
    )]
    executor: Signer<'info>,

    /// Call authority
    #[account(signer)]
    call_authority: Signer<'info>,

    /// Operation info
    #[account(
        seeds = [ROOT, b"OP", &op_hash],
        bump,
        constraint = op_info.status == OpStatus::Signed @ CustomError::OpStateInvalid
    )]
    op_info: Box<Account<'info, OpInfo>>,

    /// System config
    #[account(init_if_needed, space = Config::LEN, payer = executor, seeds = [ROOT, b"CONFIG"], bump)]
    config: Box<Account<'info, Config>>,

    /// Gov protocol info
    #[account(
        seeds = [ROOT, b"PROTOCOL", gov_protocol_id()],
        bump
    )]
    gov_info: Box<Account<'info, ProtocolInfo>>,

    /// Target protocol info
    #[account(
        init_if_needed,
        space = ProtocolInfo::LEN,
        payer = executor,
        seeds = [ROOT, b"PROTOCOL", &gov::target_protocol(&op_info.op_data.function_selector, &op_info.op_data.params)],
        bump
    )]
    target_protocol_info: Box<Account<'info, ProtocolInfo>>,

    /// System program
    system_program: Program<'info, System>,
}

#[account]
#[derive(Default)]
pub struct Config {
    admin: Pubkey,
    eob_chain_id: u64,
    eob_master_smart_contract: [u8; 32],
    nonce: u64,
}

impl Config {
    pub const LEN: usize = 8 + 32 * 2 + 8 * 2;
}

#[account]
#[derive(Default)]
pub struct ProtocolInfo {
    is_init: bool,
    consensus_target_rate: u64,
    protocol_address: Pubkey,
    transmitters: Box<[EthAddress; 20]>, // cannot use const with anchor
    executors: Box<[Pubkey; 20]>,
    proposers: Box<[Pubkey; 20]>,
}

impl ProtocolInfo {
    pub const LEN: usize =
        8 + 1 + 8 + 32 + (20 * MAX_TRANSMITTERS) + (32 * MAX_EXECUTORS) + (32 * MAX_PROPOSERS);

    pub fn transmitters(&self) -> Vec<EthAddress> {
        self.transmitters.into_iter().take_while(|k| k != &EthAddress::default()).collect()
    }

    pub fn executors(&self) -> Vec<Pubkey> {
        self.executors.into_iter().take_while(|k| k != &Pubkey::default()).collect()
    }

    pub fn proposers(&self) -> Vec<Pubkey> {
        self.proposers.into_iter().take_while(|k| k != &Pubkey::default()).collect()
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
        8 + 1 + 20 * 16 + borsh::to_vec(op_data).expect("fixed struct serialization").len()
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

#[derive(Debug)]
#[event]
pub struct ProposeEvent {
    pub protocol_id: Vec<u8>,
    pub nonce: u64,
    pub dst_chain_id: u128,
    pub protocol_address: Vec<u8>,
    pub function_selector: Vec<u8>,
    pub params: Vec<u8>,
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
    #[msg("InvalidAddress")]
    InvalidAddress,
    #[msg("ProtocolAddressNotProvided")]
    ProtocolAddressNotProvided,
    #[msg("NoTransmittersAllowed")]
    NoTransmittersAllowed,
    #[msg("MaxTransmittersExceeded")]
    MaxTransmittersExceeded,
    #[msg("MaxExecutorsExceeded")]
    MaxExecutorsExceeded,
    #[msg("MaxProposersExceeded")]
    MaxProposersExceeded,
}
