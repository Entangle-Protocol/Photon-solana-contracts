use crate::{
    error::TournamentError, ClaimableUserInfo, OperatorInfo, Role, Team, Tournament,
    TournamentStatus, GENOME_ROOT,
};
use anchor_lang::prelude::*;

#[derive(Accounts)]
pub struct TeamParticipantRegistrationRefundOmnichain<'info> {
    #[account(signer, mut)]
    pub admin: Signer<'info>,

    #[account(seeds = [GENOME_ROOT, b"OPERATOR", admin.key().as_ref()], bump, constraint = operator_info.approved && (operator_info.role == Role::BACKEND || operator_info.role == Role::OWNER) @ TournamentError::OperatorNotApprovedOrInvalidRole)]
    pub operator_info: Box<Account<'info, OperatorInfo>>,
    #[account(mut)]
    pub team: Box<Account<'info, Team>>,
    #[account(mut)]
    pub claimable_user_info: Box<Account<'info, ClaimableUserInfo>>,
    #[account(
        mut,
        seeds = [GENOME_ROOT, b"TOURNAMENT", &tournament.id.to_le_bytes().as_ref()],
        bump
    )]
    pub tournament: Box<Account<'info, Tournament>>,
    pub system_program: Program<'info, System>,
}

pub fn handle_team_participant_registration_refund_omnichain(
    ctx: Context<TeamParticipantRegistrationRefundOmnichain>,
    participant: Pubkey,
) -> Result<()> {
    let tournament = &mut ctx.accounts.tournament;
    let team = &mut ctx.accounts.team;
    let claimable_user_info = &mut ctx.accounts.claimable_user_info;

    if tournament.status != TournamentStatus::Registration {
        return Err(TournamentError::InvalidTournamentStatus.into());
    }

    let team_participant_index = team.players.iter().position(|&x| x == participant);

    if team_participant_index.is_none() {
        return Err(TournamentError::ParticipantNotFound.into());
    }
    let team_participant_index = team_participant_index.unwrap();

    // as team already exists, because of above instruction
    // we just unwrap the index
    let team_index = tournament
        .captains
        .iter()
        .position(|&x| x == team.captain)
        .unwrap();

    if team.players_refunded[team_participant_index] {
        return Err(TournamentError::AlreadyPaid.into());
    }

    claimable_user_info.claimable+=tournament.fee;

    team.players_refunded[team_participant_index] = true;
    if team.players_refunded.iter().all(|&refunded| refunded) {
        tournament.team_validated_start_game[team_index] = true;
        tournament.teams_cancelation_refunded[team_index] = true;
    }

    Ok(())
}
