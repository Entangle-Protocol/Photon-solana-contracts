use crate::{error::TournamentError, OperatorInfo, Role, Team, Tournament, TournamentStatus, ZS_ROOT};
use anchor_lang::prelude::*;

#[derive(Accounts)]
pub struct TeamRegistrationVerification<'info> {
    #[account(signer, mut)]
    pub admin: Signer<'info>,
    #[account(seeds = [ZS_ROOT, b"OPERATOR", admin.key().as_ref()], bump, constraint = operator_info.approved && (operator_info.role == Role::BACKEND || operator_info.role == Role::OWNER) @ TournamentError::OperatorNotApprovedOrInvalidRole)]
    pub operator_info: Box<Account<'info, OperatorInfo>>,
    #[account()]
    pub team: Box<Account<'info, Team>>,
    #[account(
        mut,
        seeds = [ZS_ROOT, b"TOURNAMENT", &tournament.id.to_le_bytes().as_ref()],
        bump
    )]
    pub tournament: Box<Account<'info, Tournament>>,
    pub system_program: Program<'info, System>,
}

pub fn handle_team_registration_verification(ctx: Context<TeamRegistrationVerification>) -> Result<()> {
    let tournament = &mut ctx.accounts.tournament;
    let team = &mut ctx.accounts.team;
    if tournament.status != TournamentStatus::Registration && tournament.status != TournamentStatus::Filled {
        return Err(TournamentError::InvalidTournamentStatus.into());
    }

    let index = tournament.captains.iter().position(|&x| x == team.captain);
    if index.is_none() {
        return Err(TournamentError::ParticipantNotFound.into());
    }

    let index = index.unwrap();
    if tournament.team_validated_start_game[index] {
        return Err(TournamentError::TeamAlreadyValidated.into());
    }

    if team.players.len() < tournament.players_in_team.into() {
        return Err(TournamentError::InvalidTeamSizeForRegistration.into());
    }

    tournament.team_validated_start_game[index] = true;
    Ok(())
}
