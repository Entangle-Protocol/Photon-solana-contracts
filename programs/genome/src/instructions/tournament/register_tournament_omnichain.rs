use crate::{
    error::TournamentError, init_tournament_participant_account, ClaimableUserInfo, OperatorInfo, ParticipantsRegistered, Role, Team, Tournament, TournamentStatus, GENOME_ROOT
};
use anchor_lang::prelude::*;

#[derive(Accounts)]
pub struct RegisterTournamentOmnichain<'info> {
    #[account(signer, mut)]
    pub payer: Signer<'info>,
    #[account(seeds = [GENOME_ROOT, b"OPERATOR", payer.key().as_ref()], bump, constraint = (operator_info.role == Role::OWNER || operator_info.role == Role::MESSENGER) && operator_info.approved @ TournamentError::OperatorNotApprovedOrInvalidRole)]
    pub operator_info: Box<Account<'info, OperatorInfo>>,
    #[account(
        mut,
        seeds = [GENOME_ROOT, b"TOURNAMENT", &tournament.id.to_le_bytes().as_ref()],
        bump
    )]
    pub tournament: Box<Account<'info, Tournament>>,
    #[account(mut)]
    pub captain: UncheckedAccount<'info>,
    #[account(
        init,
        payer = payer,
        seeds = [GENOME_ROOT, b"TEAM", &tournament.id.to_le_bytes().as_ref(), captain.key().as_ref()],
        bump,
        space = Team::LEN,
    )]
    pub team: Box<Account<'info, Team>>,
    #[account(
        init_if_needed,
        payer = payer,
        seeds = [GENOME_ROOT, b"USER", payer.key().as_ref()],
        bump,
        space = ClaimableUserInfo::LEN,
    )]
    pub claimable_user_info: Box<Account<'info, ClaimableUserInfo>>,
    pub system_program: Program<'info, System>,
}

pub fn handle_register_tournament_omnichain<'info>(
    ctx: Context<'_, '_, 'info, 'info, RegisterTournamentOmnichain<'info>>,
    teammates: Vec<Pubkey>,
) -> Result<()> {
    let tournament = &mut ctx.accounts.tournament;
    let team: &mut Box<Account<'_, Team>> = &mut ctx.accounts.team;
    let claimable_user_info = &mut ctx.accounts.claimable_user_info;
    
    if tournament.status != TournamentStatus::Registration {
        return Err(TournamentError::InvalidTournamentStatus.into());
    }

    if team.players.len() + teammates.len() > tournament.players_in_team as usize {
        return Err(TournamentError::MaxPlayersExceeded.into());
    }

    let entrance_fee = tournament.fee as usize * (1 + teammates.len());
    if entrance_fee > claimable_user_info.claimable as usize {
        return Err(TournamentError::NotEnoughDeposit.into())
    }
    claimable_user_info.user = if claimable_user_info.user == Pubkey::default() {
        ctx.accounts.payer.key()
    } else {
        claimable_user_info.user
    };
    claimable_user_info.claimable-=entrance_fee as u64;


    // @TODO: Implement Policy Program CPI
    // if !_is_eligible_participant(tournament.id, captain) {}
    if team.captain == Pubkey::default() {
        tournament.captains.push(ctx.accounts.captain.key());
        team.captain = ctx.accounts.captain.key();
        tournament.teams_cancelation_refunded.push(false);
        tournament.team_validated_start_game.push(false);
        team.players.push(ctx.accounts.captain.key());

        let (captain_participant_pda, bump) = Pubkey::find_program_address(&[GENOME_ROOT, b"TEAM_PARTICIPANT", &tournament.id.to_le_bytes().as_ref(), ctx.accounts.captain.key().as_ref()], &ctx.program_id);
        let pda_account = ctx
            .remaining_accounts
            .iter()
            .find(|account| account.key() == captain_participant_pda)
            .ok_or(TournamentError::ParticipantNotFound)?;
        if !pda_account.to_account_info().data_is_empty() {
            return Err(TournamentError::AlreadyRegistered.into()); 
        } 
        init_tournament_participant_account(
            ctx.accounts.captain.key(),
            pda_account,
            bump,
            *ctx.program_id,
            ctx.accounts.payer.to_account_info(),
            ctx.accounts.system_program.to_account_info(),
            tournament.id
        )?;
    } 
    
    let new_teammates = teammates.clone();
    for t in new_teammates {
        // @TODO: Implement Policy Program CPI
        // if !_is_eligible_participant(tournament.id, participant) {}
        let (participant_pda, bump) = Pubkey::find_program_address(&[GENOME_ROOT, b"TEAM_PARTICIPANT", &tournament.id.to_le_bytes().as_ref(), t.as_ref()], &ctx.program_id);
        let pda_account = ctx
            .remaining_accounts
            .iter()
            .find(|account| account.key() == participant_pda)
            .ok_or(TournamentError::ParticipantNotFound)?;
        if !pda_account.to_account_info().data_is_empty() {
            return Err(TournamentError::AlreadyRegistered.into()); 
        } 
        init_tournament_participant_account(
            t,
            pda_account,
            bump,
            *ctx.program_id,
            ctx.accounts.payer.to_account_info(),
            ctx.accounts.system_program.to_account_info(),
            tournament.id
        )?;

        team.players.push(t.clone());
        team.players_money_delivered.push(false);
        team.players_verification_payout.push(false);
        team.players_refunded.push(false);
    }
        

    if tournament.captains.len() >= tournament.max_teams as usize {
        tournament.status = TournamentStatus::Filled;
    }

    emit!(ParticipantsRegistered {
        uuid: tournament.id,
        players: teammates,
        fee: tournament.fee,
    });

    Ok(())
}