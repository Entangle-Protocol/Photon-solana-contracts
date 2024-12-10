use anchor_lang::prelude::*;

#[error_code]
pub enum TournamentError {
    // Tournament Creation
    #[msg("InvalidAdmissionFeeOrSponsorPool")]
    InvalidAdmissionFeeOrSponsorPool,
    #[msg("InvalidTeamRestrictions")]
    InvalidTeamRestrictions,
    #[msg("InvalidAmountOfPlayers")]
    InvalidAmountOfPlayers,
    #[msg("InvalidRoyalty")]
    InvalidRoyalty,
    #[msg("InvalidTournamentType")]
    InvalidTournamentType,
    #[msg("InvalidPaymentApproach")]
    InvalidPaymentApproach,

    // Tournament Registration
    #[msg("InvalidTournamentStatus")]
    InvalidTournamentStatus,
    #[msg("NotExistingTeam")]
    NotExistingTeam,
    #[msg("MaxPlayersExceeded")]
    MaxPlayersExceeded,
    #[msg("NotEligibleTeammate")]
    NotEligibleTeammate,
    #[msg("AlreadyRegistered")]
    AlreadyRegistered,
    #[msg("MaxTeamsExceeded")]
    MaxTeamsExceeded,
    #[msg("InitialPayoutNotCompleted")]
    InitialPayoutNotCompleted,
    #[msg("TeamsValidationCheckNotCompleted")]
    TeamsValidationCheckNotCompleted,
    #[msg("TeamAlreadyValidated")]
    TeamAlreadyValidated,
    
    #[msg("InvalidTeamSizeForRegistration")]
    InvalidTeamSizeForRegistration,

    #[msg("MinTeamsNotFilled")]
    MinTeamsNotFilled,

    #[msg("AlreadyPaid")]
    AlreadyPaid,

    #[msg("InvalidPrizeFractionsAmount")]
    InvalidPrizeFractionsAmount,

    #[msg("NotAdmin")]
    NotAdmin,

    #[msg("OperatorNotApprovedOrInvalidRole")]
    OperatorNotApprovedOrInvalidRole,

    #[msg("NotAllTeamsParticipated")]
    NotAllTeamsParticipated,

    #[msg("TeamsNotCompletelyRefunded")]
    TeamsNotCompletelyRefunded,

    #[msg("TeamsNotCompletelyRewarded")]
    TeamsNotCompletelyRewarded,

    #[msg("ParticipantsNotCompletelyRefunded")]
    ParticipantsNotCompletelyRefunded,

    #[msg("ParticipantNotFound")]
    ParticipantNotFound,

    #[msg("TeamNotFound")]
    TeamNotFound,

    #[msg("NotWinner")]
    NotWinner,

    #[msg("NotEnoughDeposit")]
    NotEnoughDeposit,
}

#[error_code]
pub enum FeesError {
    //---- FEE PARAMS ERRORS ----
    #[msg("Invalid fee length")]
    InvalidFeeLength,
    #[msg("Invalid fee")]
    InvalidFee,
    #[msg("Invalid wallet")]
    InvalidWallet,

    //---- GENERAL ERRORS ----
    #[msg("Beneficiary pending fees")]
    BeneficiaryPendingFees,
    #[msg("No fees to claim")]
    NoFeesToClaim,
    #[msg("Beneficiary not found")]
    BeneficiaryNotFound,
}

#[error_code]
pub enum GameError {
    //---- PARTICIPANTS ERRORS ----
    #[msg("too few participants")]
    TooFewParticipants,
    #[msg("too many participants")]
    TooManyParticipants,
    #[msg("participant already registered")]
    ParticipantAlreadyRegistered,
    #[msg("participant duplicated")]
    ParticipantDuplicated,
    #[msg("invalid participant address")]
    InvalidParticipant,
    #[msg("invalid participant vault")]
    InvalidParticipantVault,
    #[msg("prize fractions and winners mismatch")]
    PrizeFractionsAndWinnersMismatch,
    #[msg("invalid prize fractions")]
    InvalidPrizeFractions,

    //---- STATUS ERRORS ----
    #[msg("invalid game status")]
    InvalidGameStatus,
    #[msg("quick game already exists")]
    QuickGameAlreadyExists,

    //---- GENERAL ERRORS ----
    #[msg("wager amount is too small")]
    WagerTooSmall,
    #[msg("invalid winner address")]
    InvalidWinner,
    #[msg("invalid game id")]
    InvalidGameId,
    #[msg("invalid game type")]
    InvalidGameType,
}

#[error_code]
pub enum OmnichainError {
    #[msg("InvalidProtocolId")]
    InvalidProtocolId,
    #[msg("InvalidSelector")]
    InvalidSelector,
    #[msg("InvalidParams")]
    InvalidParams,
    #[msg("InvalidPubkey")]
    InvalidPubkey,
    #[msg("InvalidUserAccount")]
    InvalidUserAccount,
    #[msg("InvalidMethodId")]
    InvalidMethodId,
    #[msg("MissingAccount")]
    MissingAccount,
}

#[error_code]
pub enum BookmakerError {
    //---- GENERAL ERRORS ----
    #[msg("CannotBetForTournament")]
    CannotBetForTournament,
    #[msg("BetAmountTooLow")]
    BetAmountTooLow,
    #[msg("InvalidGambler")]
    InvalidGambler,
    #[msg("InvalidCaptain")]
    InvalidCaptain,
    #[msg("InvalidTournamentStatus")]
    InvalidTournamentStatus,
    #[msg("OverbookAlreadyClaimed")]
    OverbookAlreadyClaimed,
    #[msg("FinishAlreadyClaimed")]
    FinishAlreadyClaimed,
    #[msg("NoOverbookLeftToClaim")]
    NoOverbookLeftToClaim,
    #[msg("CaptainNotWinner")]
    CaptainNotWinner,
}

#[error_code]
pub enum ControlAccessError {
    #[msg("OperatorNotApproved")]
    OperatorNotApproved,
    #[msg("OperatorNotDeveloper")]
    OperatorNotDeveloper,
    #[msg("OperatorNotBackend")]
    OperatorNotBackend,
    #[msg("OperatorNotMessenger")]
    OperatorNotMessenger,
    #[msg("OperatorNotOwner")]
    OperatorNotOwner,
    #[msg("OperatorNotApprovedOrInvalidRole")]
    OperatorNotApprovedOrInvalidRole,
}
