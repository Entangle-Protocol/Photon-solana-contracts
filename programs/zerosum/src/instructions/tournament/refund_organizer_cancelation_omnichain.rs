use crate::{
    error::TournamentError, ClaimableUserInfo, OperatorInfo, Role, Team, Tournament,
    TournamentStatus, ZS_ROOT,
};
use anchor_lang::prelude::*;

#[derive(Accounts)]
pub struct RefundOrganizerCancelationOmnichain<'info> {
    #[account(signer, mut)]
    pub admin: Signer<'info>,
    #[account(seeds = [ZS_ROOT, b"OPERATOR", admin.key().as_ref()], bump, constraint = operator_info.approved && (operator_info.role == Role::BACKEND || operator_info.role == Role::OWNER) @ TournamentError::OperatorNotApprovedOrInvalidRole)]
    pub operator_info: Box<Account<'info, OperatorInfo>>,
    #[account(
        mut,
        seeds = [ZS_ROOT, b"TOURNAMENT", &tournament.id.to_le_bytes().as_ref()],
        bump
    )]
    pub tournament: Box<Account<'info, Tournament>>,
    #[account()]
    pub team: Box<Account<'info, Team>>,
    #[account(
        init_if_needed,
        payer = admin,
        seeds = [ZS_ROOT, b"USER", admin.key().as_ref()],
        bump,
        space = ClaimableUserInfo::LEN,
    )]
    pub claimable_user_info: Box<Account<'info, ClaimableUserInfo>>,
    #[account()]
    pub participant: SystemAccount<'info>,
    pub system_program: Program<'info, System>,
}

pub fn handle_refund_organizer_cancelation_omnichain(ctx: Context<RefundOrganizerCancelationOmnichain>) -> Result<()> {
    let tournament = &mut ctx.accounts.tournament;
    let team = &mut ctx.accounts.team;
    let claimable_user_info = &mut ctx.accounts.claimable_user_info;

    if tournament.status != TournamentStatus::PreCancel {
        return Err(TournamentError::InvalidTournamentStatus.into());
    }

    if !team.players_refunded.iter().all(|&paid| paid) {
        return Err(TournamentError::ParticipantsNotCompletelyRefunded.into());
    }

    claimable_user_info.claimable+=tournament.sponsor_pool;

    tournament.status = TournamentStatus::Canceled;

    Ok(())
}