//! Photon messaging generally comprises a set of distributed on-chain and off-chain facilities
//! that provide a transport layer across various blockchains.
//! The Photon cross-chain messaging Endpoint is a Solana program accessible either via a
//! public API or through the CPI. It facilitates the execution of operations or the submission of
//! proposals to another chain accordingly.
//!
//! Current implementation includes the photon messaging program, the onefunc distributed protocol sample program,
//! and a transmitter-module to both listen for proposals and execute operations and the other.
//!
//! ## Gov Protocol
//!
//! Initially, the Entangle Photon cross-chain messaging protocol is configured during the deployment
//! process with the governance executor and transmitter public keys. This configuration makes possible
//! the execution of additional governance operations, such as adding new protocols, configuring protocol
//! addresses, updating transmitter keys, setting consensus target rates, and other related tasks.
//!
//! ## Executing Operations
//!
//! Operations originate as proposals on another chain. These are monitored by a predetermined number
//! of transmitters and passed to the Master contract on the Entangle Oracle Blockchain.
//! Once sufficient signatures are collected, the operation is ready for execution. It proceeds through
//! three stages in the Endpoint program via the executor module: load, sign, and execute.
//! The executor agent processes each stage sequentially.
//! First, an operation is loaded into the Solana account associated with the op_hash and stored
//! until it is signed and then executed. The program verifies if the required number of signatures
//! is present in the operation data.
//! It's noteworthy that all protocols, including the GOV protocol, are accessible through the same interface.
//! In the case of the GOV protocol, the Photon Endpoint invokes itself using the CPI.
//!
//! Once a signed operation is retrieved from the RabbitMQ message queue, the load_operation, sign_operation,
//! and execute_operation stages are executed. To enable an arbitrary protocol to use accounts
//! it might utilize, the concept of an extension is introduced. An extension is registered as part of
//! the executor module configuration and dynamically linked. The executor extension informs
//! the executor about the specific addresses that will be used during the transaction execution
//! according to the passed `function_selector` and `params`. It also provides all additional signatures before
//! the transaction is sent.
//!
//! ## Listening for Proposals
//!
//! According to the business requirements, the protocol generates a new proposal. External developer's
//! protocol associated program asks the photon layer to emit a proposal as a Solana event that in
//! turn is  captured by the listener module, then transmitted to the output RabbitMQ message queue.
//!
//! # Creating an associated protocol.
//!
//! This documentation is aimed to facilitate creating a Solana based program and make it able to be
//! used within the photon messaging. It means it should be able to be called by `code` or `name` as
//! a callee or make a proposal using the `propose` Endpoint method. It also means the proper executor
//! `extension` should be provided by external developer as well as properly structured associated Solana program.
//!
//! ### Executing operation by name
//!
//! The associated program is supposed to contain methods that handle a single argument, which is a piece of
//! binary data represented as `Vec<u8>`. This data should be interpreted in various ways depending on
//! the design and expectations of the protocol. It's essential that the associated sender encodes
//! the data in a manner that meets the expectations of the protocol's corresponding receiver.
//!
//!```rust
//!use anchor_lang::prelude::Context;
//!pub fn increment(ctx: Context<Increment>, params: Vec<u8>) -> Result<()> {
//!    let inc_item = decode_increment_item(params);
//!    ctx.accounts.counter.count += inc_item;
//!    Ok(())
//!}
//!```
//!
//! The following snippet illustrates what the
//! data might look like. Generated data is intended to be processed by an executor for the correct invocation
//! of the associated contract. In this example, the `function_selector` assumes a code of 0x01 at the
//! first position and specifies the length of the remaining selector data at the second position.
//!
//!```rust
//!let function_selector: Vec<u8> = b"\x01\x17increment_owned_counter".to_vec();
//!let params: Vec<u8> = ethabi::encode(&[Token::Tuple(vec![Token::Uint(Uint::from(*component))])]);
//! ```
//!
//! In the real cross-chain messaging practice this `function_selector` and `params` are to be
//! transferred within a proposal. The params are not to be encoded with ethabi in this instance;
//! rather, this example simply demonstrates a potential approach. Future versions may standardize
//! the encoding of parameters, facilitating their decoding and display within the Entangle Explorer.
//!
//! ### Executing operation by code
//!
//! On the other hand to process a code based invocation an associated program should provide the
//! `receive_photon_msg` method that process `op_hash`, `code`, and `params` as shown in the example below:
//!
//!```rust
//!use anchor_lang::{ prelude::Context, solana_program::msg };
//!use photon::ReceivePhotonMsg;
//!pub fn receive_photon_msg(
//!    _ctx: Context<ReceivePhotonMsg>,
//!    _op_hash: Vec<u8>,
//!    сode: Vec<u8>,
//!    _params: Vec<u8>
//!) -> Result<()> {
//!    msg!("photon msg receive, code: {:?}", code);
//!    Ok(())
//!}
//!```
//!
//! It worth to be noticed that code could be empty value, in that case only `params` should be
//! interpreted as it is supposed by the given protocol.
//!
//! Like in the previous heading here is an example of how the selector and data could be combined
//! to be proposed for executing. It also possible get acquainted to this code [by the link](../src/onefunc/lib.rs.html#57-85)
//!
//!```rust
//!let mut code_function_selector = vec![0u8, code.len() as u8];
//!code_function_selector.extend(code.iter());
//!let params = <Vec<u8>>::default();
//!```
//!
//! ### Extensions
//!
//! Important part of executing an operation on the Solana is a protocol extension that should provide a
//! list of AccountMeta in advance to build and sign the `execute_operation` transaction with a fully
//! qualified list of accounts.
//!
//! The Protocol Extension is a dynamic library that includes methods such as `get_protocol_id`,
//! `get_accounts`, `sign_transaction`, and `get_compute_budget`. Notably, the `ProtocolExtension` serves a
//! critical role. An example of how the ProtocolExtension is implemented for the onefunc protocol
//! can be found [at this link](../src/onefunc_extension/onefunc_extension.rs.html#30-76)
//! Additionally, the implementation of the `gov_extension` is available [under another link](../src/gov_extension/gov_extension.rs.html#27-108)
//! We will explore these methods in detail to deepen our understanding of how they function.
//!
//!  [`get_protocol_id`](../src/transmitter_common/protocol_extension.rs.html#9)
//!
//! is in charge to provide the proper protocol id to be registered as an extension for this protocol.
//! The further dispatching during the executing is proceeding using this protocol id to select the proper extension.
//!
//!  [`get_accounts`](../src/transmitter_common/protocol_extension.rs.html#10-14).
//!
//! The methods of the associated protocol interact with a set of accounts that forms the context for this invocation.
//! The execution operation is set up so that the first three accounts are designated by the photon messaging layer, specifically: `executor`, `call_authority`, and `op_info`.
//! The `executor` is a predetermined authority for this protocol, expected to act as both signer and payer of the operation, and must be the first account provided by an extension.
//! This account should be the first in a set provided by an extension.
//! The `call_authority` is a Program Derived Address (PDA) verified at the photon layer to ensure it is invoked via cross-program invocation—handled by the photon layer.
//! `op_info` is another derived address, designed to provide the associated protocol with any operation-specific details, also facilitated by the photon layer.
//! Subsequently, the remaining accounts follow the executor account and are passed through the photon layer as they are.
//! These accounts are deliberately managed by an extension using a function_selector and params according to the specific expectations and business requirements.
//!
//! [`sign_operation`](../src/transmitter_common/protocol_extension.rs.html#15-21)
//!
//! Provides additional signatures when the transaction contains AccountMetas marked as signers.
//!
//! [`get_compute_budget`](../src/transmitter_common/protocol_extension.rs.html#23)
//!
//! Enables the increase of the compute budget for the operation currently being executed.
//!
//! ### Making a proposal
//!
//! The associated program can do more than just receive and execute operations; it can also send a proposal
//! to the destination chain if it is registered as a proposer. Here is how the
//! [onefunc associated program](../onefunc/index.html) makes a proposal
//!
//! ```rust
//!let protocol_id: Vec<u8> = PROTOCOL_ID.to_vec();
//!let dst_chain_id = 33133_u128;
//!let protocol_address: Vec<u8> = vec![1; 20];
//!let function_selector: Vec<u8> = b"ask1234mkl;1mklasdfasm;lkasdmf__".to_vec();
//!let params: Vec<u8> = b"an arbitrary data".to_vec();
//!
//!let cpi_program = ctx.accounts.photon_program.to_account_info();
//!let cpi_accounts = Propose {
//!    proposer: ctx.accounts.proposer.to_account_info(),
//!    config: ctx.accounts.config.to_account_info(),
//!    protocol_info: ctx.accounts.protocol_info.to_account_info(),
//!};
//!let bump = [ctx.bumps.proposer];
//!let proposer_seeds = [ROOT, b"PROPOSER", &bump[..]];
//!let bindings = &[&proposer_seeds[..]][..];
//!let ctx = CpiContext::new_with_signer(cpi_program, cpi_accounts, bindings);
//!
//!photon::cpi::propose(
//!    ctx,
//!    protocol_id,
//!    dst_chain_id,
//!    protocol_address,
//!    function_selector,
//!    params,
//!)
//! ```
//!

pub mod error;
pub mod gov;
mod interface;
pub mod protocol_data;
pub mod util;

use anchor_lang::prelude::*;
use error::CustomError;
use protocol_data::{
    gov_protocol_id, FunctionSelector, OpStatus, OperationData, TransmitterSignature,
};
use util::EthAddress;

declare_id!("JDxWYX5NrL51oPcYunS7ssmikkqMLcuHn9v4HRnedKHT");

/// The `photon` module encapsulates all operations related to cross-chain messaging on the Solana blockchain,
/// leveraging the capabilities of the Photon cross-chain messaging layer. It defines the governance and
/// operational structure necessary to initiate, approve, and execute operations across blockchains.
///
/// ## Constants
/// - `SOLANA_CHAIN_ID`: Unique identifier for the Solana chain, used for validation.
/// - `RATE_DECIMALS`: Used for calculations involving rate percentages in consensus processes.
/// - `ROOT`: A byte string used as a base for seed generation in account addresses.
/// - `MAX_TRANSMITTERS`, `MAX_EXECUTORS`, `MAX_PROPOSERS`: Define the maximum allowable numbers of transmitters,
///   executors, and proposers respectively to ensure the system's scalability and manageability.
///
/// ## Key Operations
/// - **Initialize**: Sets up the initial configuration for protocols, defining administrators, chain IDs,
///   smart contracts, and operational parameters such as rate and role-based limitations.
/// - **Load Operation**: The first step in operation execution, verifying the operation's integrity and
///   preparing it for further processing by setting its initial state.
/// - **Sign Operation**: Involves validating signatures to achieve consensus among transmitters, updating
///   the operation status upon achieving the required threshold.
/// - **Execute Operation**: The final step where the operation is executed based on the received and
///   validated instructions, with potential cross-program invocations if the operation involves governance
///   protocols.
/// - **Propose**: Allows registered proposers to submit operations intended to be executed on other chains,
///   managing these proposals through events that ensure transparency and traceability.
/// - **Receive Photon Message**: Specialized in handling operations directed at the governance protocol,
///   executing code-based operations that affect the system's governance structure.
///
/// ## Structs and Contexts
/// - `Initialize`, `LoadOperation`, `SignOperation`, `ExecuteOperation`: Context structs designed to facilitate
///   the respective operations by providing necessary accounts and permissions checks.
/// - `Propose`, `ReceivePhotonMsg`: Handle specific scenarios where operations need to be proposed to other chains
///   or where governance-related messages are processed.
///
/// ## Custom Errors
/// A comprehensive set of custom errors (`CustomError`) enhances error handling by providing clear, contextual
/// messages that aid in debugging and user feedback, covering a range of issues from permission errors to
/// mismatches in operation data.
///
/// ## Usage
/// This program is crucial for maintaining a robust and secure cross-chain communication infrastructure on Solana,
/// supporting a wide range of decentralized applications that require interaction between different blockchains.
/// It emphasizes security, scalability, and interoperability, ensuring that operations not only adhere to protocol
/// requirements but also maintain integrity across executions.
///
/// The module and its functions are designed to be used by blockchain developers looking to integrate Solana
/// with other chains, leveraging the Photon system's capabilities to enhance their applications' reach and functionalities.
#[program]
pub mod photon {
    /// Unique identifier for the Solana chain used within the Photon cross-chain messaging layer.
    /// This constant helps ensure operations are validated specifically for the Solana blockchain.
    pub const SOLANA_CHAIN_ID: u128 = 100000000000000000000;

    /// Represents the number of decimal places used in rate calculations for consensus mechanisms.
    /// This precision is necessary for accurate calculations when determining the consensus rate.
    pub const RATE_DECIMALS: u64 = 10000;

    /// A base seed used for deriving program-specific addresses within the system.
    /// This root seed acts as a foundational element for generating deterministic account addresses.
    pub const ROOT: &[u8] = b"root-0";

    /// The maximum number of transmitters that can be registered in the system.
    /// Transmitters are critical for the dissemination and signing of cross-chain messages.
    pub const MAX_TRANSMITTERS: usize = 20;

    /// The maximum number of executors allowed within the system.
    /// Executors are responsible for carrying out operations and managing transaction state transitions.
    pub const MAX_EXECUTORS: usize = 20;

    /// The maximum number of proposers that can be registered in the system.
    /// Proposers are authorized to initiate new operations that may affect multiple chains.
    pub const MAX_PROPOSERS: usize = 20;

    use self::{
        gov::handle_gov_operation,
        interface::{PhotonMsg, PhotonMsgWithSelector},
        protocol_data::ecrecover,
        util::sighash,
    };
    use super::*;

    use anchor_lang::solana_program::{instruction::Instruction, program::invoke_signed};

    /// Initializes the Solana program with the provided configuration and protocol information.
    ///
    /// This method sets up the admin, chain ID, master smart contract, target rate, transmitters,
    /// and executors for the protocol. It uses the `Initialize` context to access the necessary accounts.
    ///
    /// # Arguments
    ///
    /// * `ctx` - The context containing the accounts to be initialized.
    /// * `eob_chain_id` - The chain ID for the Entangle Oracle Blockchain (EOB).
    /// * `eob_master_smart_contract` - The master smart contract, represented as a vector of bytes.
    /// * `consensus_target_rate` - The rate of signing operations to be executed.
    /// * `gov_transmitters` - A vector of Ethereum addresses representing the transmitters for the governance.
    /// * `gov_executors` - A vector of public keys representing the executors for the governance.
    ///
    /// # Returns
    ///
    /// Returns a result indicating the success or failure of the operation.
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

    /// Loads an operation in the Photon cross-chain messaging layer.
    ///
    /// This method serves as the first step in executing an operation. It verifies the provided operation data
    /// and sets the initial status of the operation.
    ///
    /// # Arguments
    ///
    /// * `ctx` - The context containing the accounts for loading the operation.
    /// * `op_data` - The data related to the operation.
    /// * `op_hash_cached` - The cached hash of the operation data.
    ///
    /// # Returns
    ///
    /// Returns a result indicating the success or failure of the operation.
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
        emit!(ProposalLoaded {
            op_hash,
            executor: ctx.accounts.executor.key()
        });
        Ok(())
    }

    /// Signs an operation in the Photon cross-chain messaging layer.
    ///
    /// This method serves as the step for signing an operation. It verifies the provided signatures
    /// and updates the operation status based on the achieved consensus.
    ///
    /// # Arguments
    ///
    /// * `ctx` - The context containing the accounts for signing the operation.
    /// * `op_hash` - The hash of the operation.
    /// * `signatures` - A vector of transmitter signatures.
    ///
    /// # Returns
    ///
    /// Returns a result indicating whether the consensus was reached or not.
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
        invoke_signed(
            &instr,
            &accounts,
            &[&[
                ROOT,
                b"CALL_AUTHORITY",
                &op_data.protocol_id,
                &[ctx.bumps.call_authority],
            ]],
        )?;

        ctx.accounts.op_info.status = OpStatus::Executed;

        emit!(ProposalExecuted {
            op_hash,
            executor: ctx.accounts.executor.key()
        });
        Ok(())
    }

    /// Proposes a new operation to be processed by a target protocol in the Photon cross-chain messaging layer.
    ///
    /// This function facilitates cross-chain communication by proposing an operation to be executed
    /// on another blockchain. It handles the creation of a proposal event based on the specified
    /// details, incrementing the nonce in the system configuration to maintain a unique identifier
    /// for each proposal.
    ///
    /// # Arguments
    ///
    /// * `ctx` - The context containing the accounts necessary for making a proposal.
    /// * `protocol_id` - The identifier of the protocol.
    /// * `dst_chain_id` - The identifier of the destination chain where the proposal will be executed.
    /// * `protocol_address` - The address of the protocol on the destination chain, represented as a vector of bytes.
    /// * `function_selector` - The function selector for the proposal.
    /// * `params` - The parameters for the proposed function, represented as a vector of bytes.
    ///
    /// # Returns
    ///
    /// Returns a result indicating the success or failure of the proposal creation.
    pub fn propose(
        ctx: Context<Propose>,
        protocol_id: Vec<u8>,
        dst_chain_id: u128,
        protocol_address: Vec<u8>,
        function_selector: FunctionSelector,
        params: Vec<u8>,
    ) -> Result<()> {
        // TODO: check if all requirements are satisfied
        let nonce = ctx.accounts.config.nonce;
        ctx.accounts.config.nonce += 1;
        emit!(ProposeEvent {
            protocol_id,
            nonce,
            dst_chain_id,
            protocol_address,
            function_selector: function_selector.to_bytes()?,
            params
        });
        Ok(())
    }

    /// Handles the reception and execution of a photon message targeted to the gov protocol within
    /// the Photon cross-chain messaging layer.
    ///
    /// This function processes the photon message by invoking the associated program through CPI,
    /// specifically designed for code-based operations that fall under the governance protocol's
    /// scope. It ensures the proper execution path based on the code and parameters of the operation.
    ///
    /// # Arguments
    ///
    /// * `ctx` - The context containing the necessary accounts for processing the photon message.
    /// * `op_hash` - The hash of the operation being processed.
    /// * `code` - The code of the `function_selector` involved in the operation.
    /// * `params` - The parameters associated with the operation.
    ///
    /// # Returns
    ///
    /// Returns a result indicating the success or failure of processing the photon message.
    ///
    pub fn receive_photon_msg(
        ctx: Context<ReceivePhotonMsg>,
        _op_hash: Vec<u8>,
        code: Vec<u8>,
        _params: Vec<u8>,
    ) -> Result<()> {
        let op_data = &ctx.accounts.op_info.op_data;
        require!(
            op_data.protocol_id == gov_protocol_id() && op_data.protocol_addr == ID,
            CustomError::InvalidEndpoint
        );
        handle_gov_operation(
            &mut ctx.accounts.config,
            &mut ctx.accounts.target_protocol_info,
            code,
            op_data,
        )
    }
}

/// Represents the accounts required for initializing the Solana program.
///
/// This struct is used as a context for the `initialize` method. It includes accounts
/// for the admin, protocol information, system configuration, and system program.
///
/// # Fields
///
/// * `admin` - The admin account, which must be a signer and mutable. Additionally, it must either
/// match the `admin` key in the configuration or be a default public key.
/// * `protocol_info` - The protocol information account. It is initialized if needed, with space allocated
/// based on `ProtocolInfo::LEN`, and it utilizes seeds and a bump for addressing.
/// * `config` - The system configuration account. It is initialized if needed, with space allocated
/// based on `Config::LEN`, and it utilizes seeds and a bump for addressing.
/// * `system_program` - The system program.
///
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

/// Represents the context for loading an operation within the Photon cross-chain messaging layer.
///
/// `Loading` is the first step within the operation executing pipeline.
/// This struct is used as a context for the `load_operation` method. It includes accounts
/// for the executor, protocol information, operation information, system configuration,
/// and system program.
///
/// # Fields
///
/// * `executor` - The executor account, which must be a signer and mutable, and should be an authorized executor.
/// * `protocol_info` - The protocol information account, identified using seeds and a bump.
/// * `op_info` - The operation information account, initialized and assigned a bump if needed. It must be uninitialized (status is `None`).
/// * `config` - The system configuration account, which is mutable and identified using seeds and a bump.
/// * `system_program` - The system program.
///
/// # Arguments
///
/// * `op_data` - The data related to the operation.
/// * `op_hash_cached` - The cached hash of the operation data.
///
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

/// Represents the context for signing an operation in the Photon cross-chain messaging layer.
///
/// `Signing` is the second step within the operation executing pipeline.
/// This struct is used as a context for the `sign_operation` method. It includes accounts
/// for the executor, operation information, and protocol information.
///
/// # Fields
///
/// * `executor` - The executor account, which must be a signer and mutable, and should be an authorized executor.
/// * `op_info` - The operation information account, which is mutable and identified using seeds and a bump.
///               It should be in either the `Init` or `Signed` state.
/// * `protocol_info` - The protocol information account, identified using seeds and a bump.
///
/// # Arguments
///
/// * `op_hash` - The hash of the operation.
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

/// Represents the context for executing an operation in the Photon cross-chain messaging layer.
///
/// `Executing` is the third and the last step within the operation executing pipeline.
/// This struct is used as a context for the `execute_operation` method. It includes accounts
/// for the executor, operation information, protocol information, and call authority.
///
/// # Fields
///
/// * `executor` - The executor account, which must be a signer and mutable, and should be an authorized executor.
/// * `op_info` - The operation information account, which is mutable and identified using seeds and a bump.
///               It should be in the `Signed` state.
/// * `protocol_info` - The protocol information account, identified using seeds and a bump.
/// * `call_authority` - is a Program Derived Address (PDA) verified at the photon layer to ensure
/// it is invoked via cross-program invocation—handled by the photon layer
///
/// # Arguments
///
/// * `op_hash` - The hash of the operation.
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

/// Represents the accounts context necessary for proposing an operation in the Photon cross-chain messaging layer.
///
/// This struct is used as a context for the `propose` method. It includes accounts for the proposer,
/// system configuration, and target protocol information. Proposing here involves initiating a call
/// to execute an operation on another chain through the Photon messaging layer.
///
/// # Fields
///
/// * `proposer` - The proposer account, which must be a signer and must be listed as an authorized proposer in the protocol info.
/// * `config` - The system configuration account, which is mutable and identified using seeds and a bump.
/// * `protocol_info` - The target protocol information account, identified using seeds and a bump based on the provided `protocol_id`.
///
/// # Arguments
///
/// * `protocol_id` - The identifier for the protocol, used for deriving the `protocol_info` account.
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

/// Represents the account context necessary for receiving and processing a photon message within
/// the governance protocol.
///
/// This struct is used in the `receive_photon_msg` method, designed to handle code-based function
/// selectors that invoke associated programs through cross-program invocation (CPI). This process
/// is crucial when the gov protocol itself needs to execute a received operation, treating the
/// `receive_photon_msg` function as part of its implementation.
///
/// # Fields
///
/// * `executor` - The executor account, which must be a signer, mutable, and an authorized executor within the gov protocol.
/// * `call_authority` - is a Program Derived Address (PDA) verified at the photon layer to ensure
/// it is invoked via cross-program invocation—handled by the photon layer
/// * `op_info` - The operation information account, which should be in the `Signed` state.
/// * `config` - The system configuration account, initialized if needed, with defined space and seeds.
/// * `gov_info` - The governance protocol information account, which governs the operation.
/// * `target_protocol_info` - The target protocol information, potentially initialized and set up for the specific operation being handled.
/// * `system_program` - The system program.
///
/// # Arguments
///
/// * `op_hash` - The hash of the operation being processed.
/// * `code` - The code segment of the function selector, used for determining the appropriate execution path.
/// * `params` - The parameters associated with the operation.
///
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

/// Represents the photon cross-chain messaging configuration stored in a Solana account.
///
/// This structure holds essential information such as the admin's public key,
/// the chain ID for the Entangle Oracle Blockchain (EOB), the master smart contract address,
/// and a nonce value.
///
/// # Fields
///
/// * `admin` - The public key of the administrator.
/// * `eob_chain_id` - The chain ID for the Entangle Oracle Blockchain.
/// * `eob_master_smart_contract` - The address of the master smart contract.
/// * `nonce` - A unique identifier.
///
/// # Usage
///
/// The `Config` struct is used as part of the photon cross-chain messaging layer.
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

/// Represents the information for a protocol within the Photon cross-chain messaging layer.
///
/// The protocol is an identified unit registered in the governance (GOV) of the Photon messaging layer.
/// Anything not registered in the GOV cannot send cross-chain messages.
///
/// # Fields
///
/// * `is_init` - Indicates whether the protocol is initialized.
/// * `consensus_target_rate` - The rate of signing operations to be executed.
/// * `protocol_address` - The public key of the protocol.
/// * `transmitters` - The Ethereum addresses of entities that sign operations for execution.
/// * `executors` - The Solana addresses authorized to execute operations in the Photon Endpoint Solana program.
/// * `proposers` - The accounts permitted to call the Photon Endpoint for emitting a `Propose` event, which is meant for execution in a destination chain that is not Solana.
///
/// # Usage
///
/// The `ProtocolInfo` struct is utilized in the Photon cross-chain messaging layer.
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

/// Represents information about an operation in the Photon cross-chain messaging layer.
///
/// The `OpInfo` struct is utilized during the three steps of operation execution:
/// loading, signing, and executing.
///
/// # Fields
///
/// * `status` - The current status of the operation.
/// * `unique_signers` - An array of unique Ethereum addresses that have signed the operation.
/// * `op_data` - The data related to the operation.
#[account]
#[derive(Default)]
pub struct OpInfo {
    status: OpStatus,
    unique_signers: [EthAddress; 16],
    pub op_data: OperationData,
}

impl OpInfo {
    pub fn len(op_data: &OperationData) -> usize {
        8 + 1 + 20 * 16 + borsh::to_vec(op_data).expect("fixed struct serialization").len()
    }
}

/// Emitted when an operation is successfully loaded within the Photon cross-chain messaging layer.
///
/// This event marks the initial loading of an operation, capturing the operation hash and the
/// executor's public key, which indicate that the operation has been prepared for further processing.
///
/// # Fields
///
/// * `op_hash` - The hash of the operation that has been loaded.
/// * `executor` - The public key of the account that loaded the operation.
#[event]
pub struct ProposalLoaded {
    op_hash: Vec<u8>,
    executor: Pubkey,
}

/// Emitted when an operation is approved in the Photon cross-chain messaging layer.
///
/// This event signals that an operation has passed the necessary validations or consensus checks
/// and is approved for execution. It includes the operation hash and the executor's public key to
/// track the approval process.
///
/// # Fields
///
/// * `op_hash` - The hash of the approved operation.
/// * `executor` - The public key of the account that approved the operation.
///
#[event]
pub struct ProposalApproved {
    op_hash: Vec<u8>,
    executor: Pubkey,
}

/// Emitted when an operation is executed within the Photon cross-chain messaging layer.
///
/// This event provides details of the operation execution, including any errors that occurred.
/// The operation hash and the executor's public key are recorded to provide a full account of
/// the execution's outcome.
///
/// # Fields
///
/// * `op_hash` - The hash of the executed operation.
/// * `err` - An optional string describing any error that occurred during the execution, if applicable.
/// * `executor` - The public key of the account that executed the operation.
///

#[derive(Debug)]
#[event]
pub struct ProposalExecuted {
    pub op_hash: Vec<u8>,
    pub executor: Pubkey,
}
/// Represents an event emitted when an associated program, registered in the protocol
/// info as a proposer, proposes an operation.
///
/// This event is crucial for operations intended to be invoked on another chain through the Photon
/// cross-chain messaging layer. It marks the initiation of a proposed operation by an external
/// developer's program, which is authorized as a proposer under the designated protocol info.
///
/// The event details are used to facilitate and verify the cross-chain communication, ensuring
/// that the operation conforms to the established protocols and permissions within the Photon
/// messaging ecosystem.
///
/// # Fields
///
/// * `protocol_id` - The identifier of the protocol associated with the operation being proposed.
/// * `nonce` - A unique number that increments with each proposal to ensure the distinctiveness of each event.
/// * `dst_chain_id` - The identifier of the destination chain where the operation is intended to be executed.
/// * `protocol_address` - The address of the protocol on the destination chain, represented as a vector of bytes.
/// * `function_selector` - The function selector for the operation, formatted as a vector of bytes.
/// * `params` - The parameters required for executing the proposed function, provided as a vector of bytes.
///
/// # Usage
///
/// This event facilitates the process of proposing operations from Solana to other blockchains
/// within the Photon cross-chain messaging ecosystem, ensuring that such proposals are properly
/// authorized and documented under the protocol info of the governing structure.
///
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
