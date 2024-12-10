use crate::{error::TournamentError, OperatorInfo, Role, Tournament, TournamentStatus, ZS_ROOT};
use anchor_lang::prelude::*;

#[derive(Accounts)]
pub struct StartTournament<'info> {
    #[account(signer, mut)]
    pub participant: Signer<'info>,
    #[account(seeds = [ZS_ROOT, b"OPERATOR", participant.key().as_ref()], bump, constraint = operator_info.approved && (operator_info.role == Role::BACKEND || operator_info.role == Role::OWNER) @ TournamentError::OperatorNotApprovedOrInvalidRole)]
    pub operator_info: Box<Account<'info, OperatorInfo>>,
    #[account(
        mut,
        seeds = [ZS_ROOT, b"TOURNAMENT", &tournament.id.to_le_bytes().as_ref()],
        bump
    )]
    pub tournament: Box<Account<'info, Tournament>>,
    pub system_program: Program<'info, System>,
}

pub fn handle_start_tournament(ctx: Context<StartTournament>) -> Result<()> {
    let tournament = &mut ctx.accounts.tournament;
    if tournament.status != TournamentStatus::Registration && tournament.status != TournamentStatus::Filled {
        return Err(TournamentError::InvalidTournamentStatus.into());
    }

    if tournament.captains.len() < tournament.min_teams as usize {
        return Err(TournamentError::MinTeamsNotFilled.into());
    }

    if !tournament
        .team_validated_start_game
        .iter()
        .all(|&validated| validated)
    {
        return Err(TournamentError::TeamsValidationCheckNotCompleted.into());
    }

    tournament.status = TournamentStatus::Started;
    emit!(TournamentStartEvent {
        uuid: tournament.id,
    });

    Ok(())
}

#[derive(Debug)]
#[event]
pub struct TournamentStartEvent {
    pub uuid: u64,
}