use anchor_lang::error_code;

/// Represents custom error types for the Photon cross-chain messaging layer.
///
/// This enum defines specific error conditions that might occur during the operation of the system.
/// Each error is associated with a user-friendly message that helps identify the problem more clearly.
///
/// # Variants
///
/// * `IsNotAdmin` - The operation requires admin privileges and the current user does not have them.
/// * `ProtocolNotInit` - The protocol intended for use has not been initialized.
/// * `InvalidSignature` - The provided signature is invalid.
/// * `OpIsNotForThisChain` - The operation is not intended for this blockchain.
/// * `InvalidEndpoint` - The endpoint specified for the operation is invalid.
/// * `OpStateInvalid` - The operation is in an invalid state for the requested action.
/// * `CachedOpHashMismatch` - The cached hash of the operation does not match the expected value.
/// * `ProtocolAddressMismatch` - The protocol address does not match the expected address.
/// * `TargetProtocolMismatch` - The target protocol does not match the expected protocol.
/// * `ExecutorIsNotAllowed` - The executor attempting the operation is not authorized.
/// * `ProposerIsNotAllowed` - The proposer attempting the operation is not authorized.
/// * `OperationNotApproved` - The operation has not been approved and cannot proceed.
/// * `InvalidProtoMsg` - The protocol message is invalid.
/// * `InvalidGovMsg` - The governance message is invalid.
/// * `InvalidMethodSelector` - The method selector used is invalid.
/// * `InvalidOpData` - The operation data provided is invalid.
/// * `InvalidAddress` - The address provided is invalid.
/// * `ProtocolAddressNotProvided` - A required protocol address was not provided.
/// * `NoTransmittersAllowed` - No transmitters are allowed for this operation.
/// * `MaxTransmittersExceeded` - The maximum number of transmitters has been exceeded.
/// * `MaxExecutorsExceeded` - The maximum number of executors has been exceeded.
/// * `MaxProposersExceeded` - The maximum number of proposers has been exceeded.
///
/// # Usage
///
/// These errors are used throughout the Photon cross-chain messaging layer to ensure that
/// operations are carried out correctly and that any deviations or incorrect configurations are
/// reported accurately.
///
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
    #[msg("ExecutorIsAlreadyAllowed")]
    ExecutorIsAlreadyAllowed,
    #[msg("TryingToRemoveLastGovExecutor")]
    TryingToRemoveLastGovExecutor,
    #[msg("InvalidExecutorAddress")]
    InvalidExecutorAddress,
    #[msg("MaxProposersExceeded")]
    MaxProposersExceeded,
}

