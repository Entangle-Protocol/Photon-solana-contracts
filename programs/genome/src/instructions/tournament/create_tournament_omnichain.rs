use crate::{
    error::TournamentError, state::TournamentParams, ClaimableUserInfo, OperatorInfo, Role, Tournament, TournamentStatus, GenomeConfig, MAX_TEAMS_SIZE, MIN_ORGANIZER_ROYALTY, MIN_TEAMS_SIZE, MIN_TEAM_PLAYERS_CAPACITY, GENOME_ROOT
};
use anchor_lang::prelude::*;

#[derive(Accounts)]
#[instruction(params: TournamentParams)]
pub struct CreateTournamentOmnichain<'info> {
    #[account(signer, mut)]
    pub sponsor: Signer<'info>,
    // @CHECK: if is sponsor || operator is messenger
    #[account(seeds = [GENOME_ROOT, b"OPERATOR", sponsor.key().as_ref()], bump, constraint = (operator_info.role == Role::OWNER || operator_info.role == Role::MESSENGER) && operator_info.approved @ TournamentError::OperatorNotApprovedOrInvalidRole)]
    pub operator_info: Box<Account<'info, OperatorInfo>>,
    #[account(
        mut,
        seeds = [GENOME_ROOT, b"CONFIG"],
        bump,
    )]
    pub config: Box<Account<'info, GenomeConfig>>,
    #[account(
        init_if_needed,
        payer = sponsor,
        seeds = [GENOME_ROOT, b"USER", sponsor.key().as_ref()],
        bump,
        space = ClaimableUserInfo::LEN,
    )]
    pub claimable_user_info: Box<Account<'info, ClaimableUserInfo>>,
    #[account(
        init_if_needed,
        payer = sponsor,
        space = Tournament::len(params.max_teams as usize),
        seeds = [GENOME_ROOT, b"TOURNAMENT", &config.tournament_config.tournament_count.to_le_bytes().as_ref()],
        bump
    )]
    pub tournament: Box<Account<'info, Tournament>>,
    pub system_program: Program<'info, System>,
}

pub fn handle_create_tournament_omnichain(
    ctx: Context<CreateTournamentOmnichain>,
    organizer: Pubkey,
    params: TournamentParams,
) -> Result<()> {
    let tournament_config = &mut ctx.accounts.config.tournament_config;
    let min_teams = params.max_teams.clone();
    let max_teams = params.max_teams.clone();
    let claimable_user_info = &mut ctx.accounts.claimable_user_info;

    if !(params.fee >= tournament_config.minimal_admision_fee
        || params.sponsor_pool >= tournament_config.minimal_sponsor_pool)
    {
        return Err(TournamentError::InvalidAdmissionFeeOrSponsorPool.into());
    }
    if !(min_teams >= MIN_TEAMS_SIZE && min_teams <= max_teams && max_teams <= MAX_TEAMS_SIZE) {
        return Err(TournamentError::InvalidTeamRestrictions.into());
    }
    if params.players_in_team <= MIN_TEAM_PLAYERS_CAPACITY {
        return Err(TournamentError::InvalidAmountOfPlayers.into());
    }
    if params.organizer_royalty > MIN_ORGANIZER_ROYALTY {
        return Err(TournamentError::InvalidRoyalty.into());
    }

    let tournament = &mut ctx.accounts.tournament;
    let tournament_creation_params = params.clone();
    tournament.create_tournament(
        tournament_config.tournament_count,
        organizer,
        tournament_creation_params,
        ctx.bumps.tournament,
    )
    .unwrap_or_else(|err| panic!("Error: {}", err));

    if tournament.sponsor_pool != 0 {
        if tournament.sponsor_pool > claimable_user_info.claimable {
            return Err(TournamentError::NotEnoughDeposit.into())
        }
        claimable_user_info.claimable-=tournament.sponsor_pool;
    }
    tournament.status = TournamentStatus::Registration;
    tournament_config.tournament_count = tournament_config.tournament_count.checked_add(1).unwrap();
    emit!(TournamentCreatedEvent {
        uuid: tournament.id,
        organizer: organizer,
        fee: params.fee,
        start: params.start_time,
        organizer_royalty: tournament.organizer_royalty,
        prize_pool: params.sponsor_pool,
        players_in_team: params.players_in_team,
        min_teams: min_teams,
        max_teams: max_teams,
        token: organizer,
        bump: tournament.bump
    });

    Ok(())
}

#[derive(Debug)]
#[event]
pub struct TournamentCreatedEvent {
    pub uuid: u64,
    pub organizer: Pubkey,
    pub fee: u64,
    pub start: u64,
    pub prize_pool: u64,
    pub players_in_team: u8,
    pub min_teams: u8,
    pub max_teams: u8,
    pub organizer_royalty: u16,
    pub token: Pubkey,
    pub bump: u8,
}