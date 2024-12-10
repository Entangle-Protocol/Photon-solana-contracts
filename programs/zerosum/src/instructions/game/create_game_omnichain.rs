use anchor_lang::prelude::*;
use anchor_spl::{
    associated_token::AssociatedToken,
    token_2022::TransferChecked,
    token_interface::{transfer_checked, Mint, TokenAccount, TokenInterface},
};

use crate::{
    are_all_participants_unique,
    error::{ControlAccessError, GameError},
    Game, GameStatus, OperatorInfo, Role, ZeroSumConfig, ZS_ROOT,
};

#[derive(Accounts)]
#[instruction(game_id: u64)]
pub struct CreateGameOmnichain<'info> {
    #[account(mut)]
    pub operator: Signer<'info>,

    #[account(seeds = [ZS_ROOT, b"OPERATOR", operator.key().as_ref()], bump, constraint = operator_info.approved @ ControlAccessError::OperatorNotApproved, constraint = operator_info.role == Role::MESSENGER @ ControlAccessError::OperatorNotMessenger)]
    pub operator_info: Box<Account<'info, OperatorInfo>>,

    #[account(mut, associated_token::mint = mint, associated_token::authority = operator, associated_token::token_program = token_program)]
    pub operator_vault: Box<InterfaceAccount<'info, TokenAccount>>,

    #[account(mut, seeds = [ZS_ROOT, b"CONFIG"], bump)]
    pub config: Box<Account<'info, ZeroSumConfig>>,

    pub mint: Box<InterfaceAccount<'info, Mint>>,

    // the game PDA is derived by using a counter, to guarantee uniqueness
    #[account(init, payer = operator, space = Game::LEN, seeds = [ZS_ROOT, b"GAME", &game_id.to_le_bytes().as_ref()], bump, constraint = config.games_config.games_counter == game_id @ GameError::InvalidGameId)]
    pub game: Box<Account<'info, Game>>,

    #[account(init_if_needed, payer = operator, associated_token::mint = mint, associated_token::authority = game, associated_token::token_program = token_program)]
    pub game_vault: Box<InterfaceAccount<'info, TokenAccount>>,

    pub system_program: Program<'info, System>,
    pub token_program: Interface<'info, TokenInterface>,
    pub associated_token_program: Program<'info, AssociatedToken>,
}

pub fn handle_create_game_omnichain(
    ctx: Context<CreateGameOmnichain>,
    _game_id: u64,
    wager: u64,
    participants: Vec<Pubkey>,
    start_game: bool,
) -> Result<()> {
    let config = &mut ctx.accounts.config;

    // Validate the game does not exist yet
    require!(
        ctx.accounts.game.status == GameStatus::NotExists,
        GameError::InvalidGameStatus
    );
    require!(
        ctx.accounts.game.participants.len() == 0,
        GameError::QuickGameAlreadyExists
    );

    require!(participants.len() > 1, GameError::TooFewParticipants);

    require!(participants.len() <= 32, GameError::TooManyParticipants);

    // Validate the participants are not the default key and are unique
    for participant in participants.iter() {
        require!(
            participant != &Pubkey::default(),
            GameError::InvalidParticipant
        );
        require!(
            are_all_participants_unique(participants.clone()),
            GameError::ParticipantDuplicated
        );
    }

    // game wager must be greater than the minimal wager on config
    require!(
        wager >= config.games_config.minimal_wager,
        GameError::WagerTooSmall
    );

    let total_wager = wager * participants.len() as u64;

    // The operator will transfer the wager to the treasury
    let accounts = TransferChecked {
        from: ctx.accounts.operator_vault.to_account_info(),
        mint: ctx.accounts.mint.to_account_info(),
        to: ctx.accounts.game_vault.to_account_info(),
        authority: ctx.accounts.operator.to_account_info(),
    };
    let cpi = CpiContext::new(ctx.accounts.token_program.to_account_info(), accounts);
    transfer_checked(cpi, total_wager, ctx.accounts.mint.decimals)?;

    let game = &mut ctx.accounts.game;
    // If start_game is false, there are more intended participants to register
    let game_status = if start_game {
        GameStatus::Started
    } else {
        GameStatus::Created
    };
    match game.create_game(
        config.games_config.games_counter,
        crate::GameType::OMNICHAIN,
        wager,
        participants,
        game_status,
    ) {
        Ok(_val) => {}
        Err(err) => {
            panic!("Error: {}", err);
        }
    }

    // Increment the games counter
    config.games_config.games_counter += 1;

    emit!(GameOmnichainCreatedEvent {
        game_id: game.id,
        participants: game.participants.clone(),
        wager: game.wager,
    });

    Ok(())
}

#[derive(Debug)]
#[event]
pub struct GameOmnichainCreatedEvent {
    pub game_id: u64,
    pub participants: Vec<Pubkey>,
    pub wager: u64,
}
