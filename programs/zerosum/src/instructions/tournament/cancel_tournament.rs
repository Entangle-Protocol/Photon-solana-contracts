use crate::{error::TournamentError, OperatorInfo, Role, Tournament, TournamentStatus, ZS_ROOT};
use anchor_lang::prelude::*;

#[derive(Accounts)]
pub struct CancelTournament<'info> {
    #[account(signer, mut)]
    pub organizer: Signer<'info>,
    #[account(seeds = [ZS_ROOT, b"OPERATOR", organizer.key().as_ref()], bump, constraint = operator_info.approved && (operator_info.role == Role::BACKEND || operator_info.role == Role::OWNER) @ TournamentError::OperatorNotApprovedOrInvalidRole)]
    pub operator_info: Box<Account<'info, OperatorInfo>>,
    #[account(
        mut,
        seeds = [ZS_ROOT, b"TOURNAMENT", &tournament.id.to_le_bytes().as_ref()],
        bump
    )]
    pub tournament: Box<Account<'info, Tournament>>,
    pub system_program: Program<'info, System>,
}

pub fn handle_cancel_tournament(ctx: Context<CancelTournament>) -> Result<()> {
    let tournament = &mut ctx.accounts.tournament;
    if tournament.status != TournamentStatus::Registration
        && tournament.status != TournamentStatus::Filled
        && tournament.status != TournamentStatus::Started
    {
        return Err(TournamentError::InvalidTournamentStatus.into());
    }

    tournament.status = TournamentStatus::PreCancel;

    // @TODO: Implement
    // if unchecked iboomark account has money integrated
    // call the bookmarker Cancel instruction
    emit!(TournamentCanceled {
        uuid: tournament.id,
    });

    Ok(())
}

#[derive(Debug)]
#[event]
pub struct TournamentCanceled {
    pub uuid: u64,
}

#[derive(Debug)]
#[event]
pub struct Refund {
    pub uuid: u64,
    pub organizer: Pubkey,
    pub sponsor_pool: u64,
    pub token: Pubkey,
}