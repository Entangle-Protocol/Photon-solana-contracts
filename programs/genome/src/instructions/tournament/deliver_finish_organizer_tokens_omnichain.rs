use crate::{
    error::TournamentError, ClaimableUserInfo, OperatorInfo, Role, Team, Tournament, TournamentStatus, GENOME_ROOT
};
use anchor_lang::prelude::*;

#[derive(Accounts)]
pub struct DeliverFinishOrganizerTokensOmnichain<'info> {
    #[account(signer, mut)]
    pub admin: Signer<'info>,
    #[account(seeds = [GENOME_ROOT, b"OPERATOR", admin.key().as_ref()], bump, constraint = operator_info.approved && (operator_info.role == Role::BACKEND || operator_info.role == Role::OWNER) @ TournamentError::OperatorNotApprovedOrInvalidRole)]
    pub operator_info: Box<Account<'info, OperatorInfo>>,
    #[account(
        mut,
        seeds = [GENOME_ROOT, b"TOURNAMENT", &tournament.id.to_le_bytes().as_ref()],
        bump
    )]
    pub tournament: Box<Account<'info, Tournament>>,
    #[account()]
    pub team: Box<Account<'info, Team>>,
    #[account(
        init_if_needed,
        payer = admin,
        seeds = [GENOME_ROOT, b"USER", admin.key().as_ref()],
        bump,
        space = ClaimableUserInfo::LEN,
    )]
    pub claimable_user_info: Box<Account<'info, ClaimableUserInfo>>,
    pub system_program: Program<'info, System>,
}

pub fn handle_deliver_finish_organizer_tokens_omnichain(
    ctx: Context<DeliverFinishOrganizerTokensOmnichain>,
) -> Result<()> {
    let tournament = &mut ctx.accounts.tournament;
    let team = &mut ctx.accounts.team;
    let claimable_user_info = &mut ctx.accounts.claimable_user_info;
    if tournament.status != TournamentStatus::PreFinish {
        return Err(TournamentError::InvalidTournamentStatus.into());
    }

    if team.players.len() != tournament.players_in_team as usize {
        return Err(TournamentError::NotAllTeamsParticipated.into());
    }

    if tournament.finish_metadata.winners.contains(&team.key()) {
        return Err(TournamentError::NotWinner.into());
    }

    if !tournament.finish_metadata.rewarded_winners.iter().all(|&paid| paid) {
        return Err(TournamentError::TeamsNotCompletelyRewarded.into());
    }

    claimable_user_info.claimable+=tournament.finish_metadata.remaining_prize_pool;

    tournament.status = TournamentStatus::Finished;
    tournament.finish_metadata.remaining_prize_pool = 0;

    Ok(())
}