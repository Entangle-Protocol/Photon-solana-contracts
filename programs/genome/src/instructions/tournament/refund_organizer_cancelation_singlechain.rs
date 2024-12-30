use crate::{error::TournamentError, OperatorInfo, Role, Team, Tournament, TournamentStatus, GENOME_ROOT};
use anchor_lang::prelude::*;
use anchor_spl::{
    token_2022::TransferChecked,
    token_interface::{transfer_checked, Mint, TokenAccount, TokenInterface},
};

#[derive(Accounts)]
pub struct RefundOrganizerCancelationSinglechain<'info> {
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
    pub mint: Box<InterfaceAccount<'info, Mint>>,
    #[account(
        mut,
        associated_token::mint = mint, 
        associated_token::authority = tournament.organizer,
        associated_token::token_program = token_program,
    )]
    pub organizer_vault: Box<InterfaceAccount<'info, TokenAccount>>,
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

pub fn handle_refund_organizer_cancelation_singlechain(ctx: Context<RefundOrganizerCancelationSinglechain>) -> Result<()> {
    let tournament = &mut ctx.accounts.tournament;
    let team = &mut ctx.accounts.team;

    if tournament.status != TournamentStatus::PreCancel {
        return Err(TournamentError::InvalidTournamentStatus.into());
    }

    if !team.players_refunded.iter().all(|&paid| paid) {
        return Err(TournamentError::ParticipantsNotCompletelyRefunded.into());
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
        to: ctx.accounts.organizer_vault.to_account_info(),
        authority: tournament.to_account_info(),
    };

    let cpi_context = CpiContext::new_with_signer(
        ctx.accounts.token_program.to_account_info(),
        accounts,
        &signer_seeds,
    );

    transfer_checked(cpi_context, tournament.sponsor_pool, ctx.accounts.mint.decimals)?;

    tournament.status = TournamentStatus::Canceled;

    Ok(())
}
