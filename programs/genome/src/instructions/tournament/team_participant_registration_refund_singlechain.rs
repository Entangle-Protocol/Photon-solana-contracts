use crate::{error::TournamentError, OperatorInfo, Role, Team, Tournament, TournamentStatus, GENOME_ROOT};
use anchor_lang::prelude::*;
use anchor_spl::{
    token_2022::TransferChecked,
    token_interface::{transfer_checked, Mint, TokenAccount, TokenInterface},
};

#[derive(Accounts)]
pub struct TeamParticipantRegistrationRefundSinglechain<'info> {
    #[account(signer, mut)]
    pub admin: Signer<'info>,
    #[account(seeds = [GENOME_ROOT, b"OPERATOR", admin.key().as_ref()], bump, constraint = operator_info.approved && (operator_info.role == Role::BACKEND || operator_info.role == Role::OWNER) @ TournamentError::OperatorNotApprovedOrInvalidRole)]
    pub operator_info: Box<Account<'info, OperatorInfo>>,
    #[account(mut)]
    pub team: Box<Account<'info, Team>>,
    #[account(
        mut,
        seeds = [GENOME_ROOT, b"TOURNAMENT", &tournament.id.to_le_bytes().as_ref()],
        bump
    )]
    pub tournament: Box<Account<'info, Tournament>>,
    pub mint: Box<InterfaceAccount<'info, Mint>>,
    #[account(mut)]
    pub participant: AccountInfo<'info>,
    #[account(
        mut,
        associated_token::mint = mint, 
        associated_token::authority = participant,
        associated_token::token_program = token_program,
    )]
    pub participant_vault: Box<InterfaceAccount<'info, TokenAccount>>,
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

pub fn handle_team_participant_registration_refund_singlechain(ctx: Context<TeamParticipantRegistrationRefundSinglechain>) -> Result<()> {
    let tournament = &mut ctx.accounts.tournament;
    let team = &mut ctx.accounts.team;

    if tournament.status != TournamentStatus::Registration {
        return Err(TournamentError::InvalidTournamentStatus.into());
    }

    let team_participant_index = team.players.iter().position(|&x| x == ctx.accounts.participant.key());

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

    let seeds = &[
        GENOME_ROOT,
        b"TOURNAMENT",
        &tournament.id.to_le_bytes()[..],
        &[tournament.bump],
    ];
    let signer_seeds = [&seeds[..]];

    let accounts = TransferChecked {
        from: ctx.accounts.tournament_vault.to_account_info(),
        mint: ctx.accounts.mint.to_account_info(),
        to: ctx.accounts.participant_vault.to_account_info(),
        authority: tournament.to_account_info(),
    };

    let cpi_context = CpiContext::new_with_signer(
        ctx.accounts.token_program.to_account_info(),
        accounts,
        &signer_seeds,
    );

    transfer_checked(
        cpi_context,
        tournament.fee,
        ctx.accounts.mint.decimals,
    )?;

    team.players_refunded[team_participant_index] = true;
    if team.players_refunded.iter().all(|&refunded| refunded) {
        tournament.team_validated_start_game[team_index] = true;
        tournament.teams_cancelation_refunded[team_index] = true;
    }

    Ok(())
}
