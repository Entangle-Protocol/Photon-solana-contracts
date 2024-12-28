use anchor_lang::prelude::*;

use crate::{
    GameStatus, GameType, Role, TournamentStatus, BP_DEC, MAX_PARTICIPANT_IN_TEAM, MAX_TEAMS_SIZE,
};

// General
#[account]
#[derive(Default)]
pub struct GenomeConfig {
    pub admin: Pubkey,
    pub tournament_config: TournamentConfig,
    pub fees_config: FeesConfig,
    pub games_config: GameConfig,
    pub bookmaker_config: BookmakerConfig,
    // Treasury config excluded ad it's supplanted by "admin"
}

#[account]
#[derive(Default)]
pub struct OperatorInfo {
    pub approved: bool,
    pub role: Role,
}

impl OperatorInfo {
    pub const LEN: usize = 8 + 1 + 1;
}

// Tournaments
#[derive(Default, Clone, AnchorSerialize, AnchorDeserialize)]
pub struct TournamentConfig {
    pub tournament_count: u64,
    pub minimal_admision_fee: u64,
    pub minimal_sponsor_pool: u64,
}

impl TournamentConfig {
    pub const LEN: usize = 8 + 8 + 8 + 8;
}

#[account]
#[derive(Default)]
pub struct Tournament {
    pub id: u64,
    pub organizer: Pubkey,
    pub fee: u64,
    pub sponsor_pool: u64,
    pub start_time: i64,
    pub captains: Vec<Pubkey>,
    pub team_validated_start_game: Vec<bool>,
    pub teams_cancelation_refunded: Vec<bool>,
    pub status: TournamentStatus,
    pub tournament_type: u8,
    pub min_teams: u8,
    pub max_teams: u8,
    pub players_in_team: u8,
    pub organizer_royalty: u16,
    pub bump: u8,
    pub finish_metadata: FinishTournamentMetadata,
}

pub const MAX_TEAMS_USIZE: usize = MAX_TEAMS_SIZE as usize;
pub const MAX_PARTICIPANT_IN_TEAM_USIZE: usize = MAX_PARTICIPANT_IN_TEAM as usize;

impl Tournament {
    pub const LEN: usize = 8 + 8 + 32 + 8 + 8 + 16 + 1 + 1 + 1 + 1 + 1 + 2 + 1;

    pub fn len(max_teams: usize) -> usize {
        Self::LEN + (32 * max_teams) + (1 * max_teams) + (1 * max_teams) + FinishTournamentMetadata::len(max_teams)
    }

    pub fn create_tournament(
        &mut self,
        new_tournament_id: u64,
        organizer: Pubkey,
        params: TournamentParams,
        bump: u8,
    ) -> Result<()> {
        let tournament = self;
        tournament.id = new_tournament_id;
        tournament.min_teams = params.min_teams;
        tournament.max_teams = params.max_teams;
        tournament.bump = bump;
        tournament.fee = params.fee;
        tournament.sponsor_pool = params.sponsor_pool;
        tournament.start_time = params.start_time as i64;
        tournament.players_in_team = params.players_in_team;
        tournament.organizer_royalty = params.organizer_royalty;
        tournament.organizer = organizer;

        Ok(())
    }
}

#[derive(AnchorSerialize, AnchorDeserialize, Default, Clone)]
pub struct FinishTournamentMetadata {
    pub winners: Vec<Pubkey>,
    pub rewarded_winners: Vec<bool>,
    pub rewards_prize_fractions: Vec<u16>,
    pub fee_type: u8,
    pub remaining_prize_pool: u64,
    pub total_prize_pool: u64,
}

impl FinishTournamentMetadata {
    pub const LEN: usize = 8
        + 1
        + 8
        + 8;

    fn len(max_teams: usize) -> usize {
        Self::LEN
            + (32 * max_teams)
            + (1 * max_teams)
            + (2 * max_teams)
    }
}

#[account]
#[derive(Default)]
pub struct Team {
    pub captain: Pubkey,
    pub players: Vec<Pubkey>,
    pub players_verification_payout: Vec<bool>,
    pub players_refunded: Vec<bool>,
    pub players_money_delivered: Vec<bool>,
    pub bump: u8,
}

impl Team {
    pub const LEN: usize = 8
        + 32
        + 1;

    pub fn len(max_players: usize) -> usize {
        Self::LEN
            + (32 * max_players)
            + (1 * max_players)
            + (1 * max_players)
            + (1 * max_players)
            + (1 * max_players)
    }
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone)]
pub struct TournamentParams {
    pub fee: u64,
    pub sponsor_pool: u64,
    pub start_time: u64,
    pub players_in_team: u8,
    pub min_teams: u8,
    pub max_teams: u8,
    pub organizer_royalty: u16,
    pub token: Pubkey,
}

#[account]
#[derive(Default)]
pub struct TournamentParticipant {
    pub captain: Pubkey,
    pub user: Pubkey,
}

impl TournamentParticipant {
    pub const LEN: usize = 8 + 32 + 32;
}

// FEES
#[derive(Default, Clone, AnchorSerialize, AnchorDeserialize)]
pub struct FeesConfig {
    pub base_fee: u64,
    pub platform_wallet: Pubkey,
}

impl FeesConfig {
    pub const LEN: usize = 8 + 32 + 8 + 32; // TODO: check this, 10 is a guess
}

#[account]
#[derive(Default)]
pub struct FeeMeta {
    pub base_fee: u64,
    pub beneficiaries: Vec<Pubkey>,
    pub fractions: Vec<u64>,
    pub pending_to_claim: Vec<u64>,
}

impl FeeMeta {
    pub const LEN: usize = 8 + 10 * 32 + 10 * 8 + 10 * 8;
    pub fn update_beneficiaries_claim(&mut self, amount: u64) -> Result<u64> {
        let fee_meta = self;
        let mut sum = amount * fee_meta.base_fee / BP_DEC;

        let fractions = fee_meta.fractions.clone();

        for (i, pending_to_claim) in fee_meta.pending_to_claim.iter_mut().enumerate() {
            let fee = amount * fractions[i] / BP_DEC;
            sum -= fee;
            *pending_to_claim += fee;
        }

        Ok(sum)
    }
}

// GAME
#[derive(Default, Clone, AnchorSerialize, AnchorDeserialize)]
pub struct GameConfig {
    pub base_fee: u64,
    pub minimal_wager: u64,
    pub games_counter: u64,
}

impl GameConfig {
    pub const LEN: usize = 8 + 32 + 8 + 8 + 8;
}

#[account]
pub struct Game {
    pub game_type: GameType,
    pub wager: u64,
    pub participants: Vec<Pubkey>,
    pub winners: Vec<Pubkey>,
    pub refunded_participants: Vec<Pubkey>,
    pub status: GameStatus,
    pub id: u64,
    pub used_fractions: u64,
    pub total_fractions: u64,
}

impl Game {
    pub const LEN: usize = 8 + 1 + 8 + 32 * 32 + 32 * 32 + 32 * 32 + 1 + 8 + 8 + 8;
    pub fn create_game(
        &mut self,
        new_game_id: u64,
        game_type: GameType,
        wager: u64,
        participants: Vec<Pubkey>,
        status: GameStatus,
    ) -> Result<()> {
        let game = self;
        game.game_type = game_type;
        game.wager = wager;
        game.participants = participants;
        game.winners = Vec::new();
        game.refunded_participants = Vec::new();
        game.status = status;
        game.used_fractions = 0;
        game.total_fractions = BP_DEC;
        game.id = new_game_id;

        Ok(())
    }
}

#[account]
#[derive(Default)]
pub struct ClaimableUserInfo {
    pub user: Pubkey,
    pub claimable: u64,
}

impl ClaimableUserInfo {
    pub const LEN: usize = 8 + 32 + 8;
}

impl GenomeConfig {
    pub const LEN: usize =
        8 + 32 + TournamentConfig::LEN + GameConfig::LEN + FeesConfig::LEN + BookmakerConfig::LEN;
}

// BRIDGE
#[account]
#[derive(Default)]
pub struct BridgeConfig {
    pub admin: Pubkey,
    pub bridge_router_address: [u8; 20],
}

impl BridgeConfig {
    pub const LEN: usize = 8 + 32 + 20 + 8 + 32;
}

// BOOKMAKER
#[account]
#[derive(Default)]
pub struct BookmakerConfig {
    pub minimal_bet: u64,
}
impl BookmakerConfig {
    pub const LEN: usize = 8 + 8;
}
// PDA generated using the tournament id
#[account]
#[derive(Default)]
pub struct TournamentBook {
    pub total_sum: u64,
    pub total_overbook_claimable: u64,
}
impl TournamentBook {
    pub const LEN: usize = 8 + 8 + 8;
}
// PDA generated using the tournament id and the captain pubkey
#[account]
#[derive(Default)]
pub struct CaptainBet {
    pub sum: u64,
    pub overbooked: bool,
    pub overbook_claimable: u64,
    pub fees_sent_to_beneficiaries: bool,
}
impl CaptainBet {
    pub const LEN: usize = 8 + 8 + 1 + 8 + 1;
}
// PDA generated using the tournament id, the captain pubkey and the gambler pubkey
#[account]
#[derive(Default)]
pub struct GamblerInfo {
    pub bet: u64,
    pub has_claimed_overbook: bool,
    pub has_claimed_finish: bool,
    pub has_claimed_cancel: bool,
}
impl GamblerInfo {
    pub const LEN: usize = 8 + 8 + 1 + 1 + 1;
}