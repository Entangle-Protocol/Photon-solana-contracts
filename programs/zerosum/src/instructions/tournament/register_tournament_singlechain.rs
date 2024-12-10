use crate::{
    error::TournamentError, init_tournament_participant_account, OperatorInfo, ParticipantRegistered, Team, Tournament, TournamentStatus, ZS_ROOT
};
use anchor_lang::prelude::*;
use anchor_spl::{
    token_2022::TransferChecked,
    token_interface::{transfer_checked, Mint, TokenAccount, TokenInterface},
};

#[derive(Accounts)]
pub struct RegisterTournamentSinglechain<'info> {
    #[account(signer, mut)]
    pub captain: Signer<'info>,
    #[account()]
    pub operator_info: Box<Account<'info, OperatorInfo>>,
    #[account(
        mut,
        seeds = [ZS_ROOT, b"TOURNAMENT", &tournament.id.to_le_bytes().as_ref()],
        bump
    )]
    pub tournament: Box<Account<'info, Tournament>>,
    #[account(
        init,
        payer = captain,
        seeds = [ZS_ROOT, b"TEAM", &tournament.id.to_le_bytes().as_ref(), captain.key().as_ref()],
        bump,
        space = Team::LEN,
    )]
    pub team: Box<Account<'info, Team>>,
    pub mint: Box<InterfaceAccount<'info, Mint>>,
    #[account(
        mut, 
        associated_token::mint = mint, 
        associated_token::authority = captain,
        associated_token::token_program = token_program,
    )]
    pub captain_vault: Box<InterfaceAccount<'info, TokenAccount>>,
    #[account(
        mut,
        associated_token::mint = mint, 
        associated_token::authority = tournament,
        associated_token::token_program = token_program,
    )]
    pub tournament_vault: Box<InterfaceAccount<'info, TokenAccount>>,
    pub system_program: Program<'info, System>,
    pub token_program: Interface<'info, TokenInterface>,
}

pub fn handle_register_tournament_singlechain<'info>(
    ctx: Context<'_, '_, 'info, 'info, RegisterTournamentSinglechain<'info>>,
) -> Result<()> {
    let tournament = &mut ctx.accounts.tournament;
    let team = &mut ctx.accounts.team;
    let captain = ctx.accounts.captain.key();

    if tournament.status != TournamentStatus::Registration {
        return Err(TournamentError::InvalidTournamentStatus.into());
    }

    if team.players.len() + 1 > tournament.players_in_team as usize {
        return Err(TournamentError::MaxPlayersExceeded.into());
    }

    // @TODO: Implement Policy Program CPI for Captain
    // if !_is_eligible_participant(tournament.id, captain) {}
    let accounts = TransferChecked {
        from: ctx.accounts.captain_vault.to_account_info(),
        mint: ctx.accounts.mint.to_account_info(),
        to: ctx.accounts.tournament_vault.to_account_info(),
        authority: ctx.accounts.captain.to_account_info(),
    };
    let cpi = CpiContext::new(ctx.accounts.token_program.to_account_info(), accounts);
    transfer_checked(cpi, tournament.fee, ctx.accounts.mint.decimals)?;

    if tournament.captains.len() as u8 >= tournament.max_teams {
        return Err(TournamentError::MaxTeamsExceeded.into());
    }
    tournament.captains.push(captain);
    tournament.teams_cancelation_refunded.push(false);
    tournament.team_validated_start_game.push(false);
    team.captain = captain;

    if team.players.len() + 1 > tournament.players_in_team as usize {
        return Err(TournamentError::MaxPlayersExceeded.into());
    }

    let (captain_participant_pda, bump) = Pubkey::find_program_address(&[ZS_ROOT, b"TEAM_PARTICIPANT", &tournament.id.to_le_bytes().as_ref(), ctx.accounts.captain.key().as_ref()], &ctx.program_id);
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
        ctx.accounts.captain.to_account_info(),
        ctx.accounts.system_program.to_account_info(),
        tournament.id
    )?;

    team.players.push(captain);
    team.players_money_delivered.push(false);
    team.players_verification_payout.push(false);
    team.players_refunded.push(false);
    if tournament.captains.len() >= tournament.max_teams as usize {
        tournament.status = TournamentStatus::Filled;
    }

    emit!(ParticipantRegistered {
        uuid: tournament.id,
        player: captain,
        fee: tournament.fee,
    });

    Ok(())
}