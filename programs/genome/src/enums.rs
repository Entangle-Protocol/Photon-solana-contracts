use anchor_lang::prelude::*;

// TOURNAMENT
#[derive(PartialEq, Eq)]
pub enum PaymentApproach {
    Unknown,
    Default,
    Direct,
    FromClaimable,
}

impl Default for PaymentApproach {
    fn default() -> Self {
        PaymentApproach::Unknown
    }
}

#[derive(PartialEq, Eq)]
pub enum TournamentType {
    Unknown,
    Singlechain,
    Omnichain,
}

impl Default for TournamentType {
    fn default() -> Self {
        TournamentType::Unknown
    }
}

#[derive(PartialEq, Eq, AnchorSerialize, AnchorDeserialize, Clone)]
pub enum TournamentStatus {
    Unknown,
    NotExist,
    Registration,
    Filled,
    Started,
    PreFinish,
    Finished,
    PreCancel,
    Canceled,
}

impl Default for TournamentStatus {
    fn default() -> Self {
        TournamentStatus::Unknown
    }
}

#[derive(PartialEq, Eq, AnchorSerialize, AnchorDeserialize, Clone)]
pub enum Role {
    NONE,
    OWNER,
    DEVELOPER,
    BACKEND,
    MESSENGER,
}

impl Default for Role {
    fn default() -> Self {
        Role::NONE
    }
}

#[derive(Debug)]
#[event]
pub struct ParticipantRegistered {
    pub uuid: u64,
    pub player: Pubkey,
    pub fee: u64,
}

#[derive(Debug)]
#[event]
pub struct ParticipantsRegistered {
    pub uuid: u64,
    pub players: Vec<Pubkey>,
    pub fee: u64,
}

// GAME

#[derive(PartialEq, Eq, AnchorSerialize, AnchorDeserialize, Clone)]
pub enum GameStatus {
    NotExists,
    Created,
    Started,
    Finished,
    PreCanceled,
    Canceled,
}

#[derive(Debug, PartialEq, Eq, AnchorSerialize, AnchorDeserialize, Clone)]
pub enum GameType {
    UNKNOWN,
    SINGLECHAIN,
    OMNICHAIN,
}

impl Default for GameStatus {
    fn default() -> Self {
        GameStatus::NotExists
    }
}