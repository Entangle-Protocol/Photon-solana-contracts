use core::panic;
use std::vec;

use anchor_lang::prelude::*;
use anchor_spl::{
    associated_token::AssociatedToken,
    token_2022::TransferChecked,
    token_interface::{transfer_checked, Mint, TokenAccount, TokenInterface},
};

use crate::{
    error::{ControlAccessError, GameError},
    init_user_info, send_game_remaining_tokens_to_treasury, update_claimable_amount, FeeMeta, Game,
    GameStatus, GameType, OperatorInfo, Role, SendPlatformWalletEvent,
    UpdateBeneficiariesClaimEvent, GenomeConfig, BP_DEC, GENOME_ROOT,
};

#[derive(Accounts)]
#[instruction(fee_type: u16)]
pub struct FinishGame<'info> {
    #[account(signer, mut)]
    pub operator: Signer<'info>,

    #[account(seeds = [GENOME_ROOT, b"OPERATOR", operator.key().as_ref()], bump, constraint = operator_info.approved @ ControlAccessError::OperatorNotApproved, constraint = (operator_info.role == Role::MESSENGER || operator_info.role == Role::BACKEND) @ ControlAccessError::OperatorNotApproved)]
    pub operator_info: Box<Account<'info, OperatorInfo>>,

    #[account(mut, seeds = [GENOME_ROOT, b"CONFIG"], bump)]
    pub config: Box<Account<'info, GenomeConfig>>,

    //------------------- TREASURY ACCOUNTS -------------------
    /// CHECK: treasury authority
    #[account(seeds = [GENOME_ROOT, b"AUTHORITY"], bump)]
    pub treasury_authority: AccountInfo<'info>,

    #[account(mut, associated_token::mint = mint, associated_token::authority = treasury_authority, associated_token::token_program = token_program)]
    pub treasury_vault: Box<InterfaceAccount<'info, TokenAccount>>,

    // --------------------- UPDATE BENEFICIARIES CLAIM AND SEND PLATFORM WALLET ACCOUNTS ---------------------
    #[account(mut, associated_token::mint = mint, associated_token::authority = platform_wallet, associated_token::token_program = token_program)]
    pub platform_wallet_vault: Box<InterfaceAccount<'info, TokenAccount>>,

    /// CHECK: platform wallet
    #[account(mut)]
    pub platform_wallet: AccountInfo<'info>,

    // In case the fee_type is 0, this account is not used
    #[account(init_if_needed, payer = operator, space = FeeMeta::LEN, seeds = [GENOME_ROOT, b"FEE_META", &fee_type.to_le_bytes().as_ref()], bump)]
    pub fee_meta: Box<Account<'info, FeeMeta>>,

    // ------------------------------------------------------------------------------
    #[account(mut, seeds = [GENOME_ROOT, b"GAME", &game.id.to_le_bytes().as_ref()], bump)]
    pub game: Box<Account<'info, Game>>,

    #[account(mut, associated_token::mint = mint, associated_token::authority = game, associated_token::token_program = token_program)]
    pub game_vault: Box<InterfaceAccount<'info, TokenAccount>>,

    pub mint: Box<InterfaceAccount<'info, Mint>>,

    pub token_program: Interface<'info, TokenInterface>,
    pub associated_token_program: Program<'info, AssociatedToken>,
    pub system_program: Program<'info, System>,
}

pub fn handle_finish_game<'info>(
    ctx: Context<'_, '_, 'info, 'info, FinishGame<'info>>,
    fee_type: u16,
    winners: Vec<Pubkey>,
    prize_fractions: Vec<u64>,
) -> Result<()> {
    let game = &mut ctx.accounts.game;

    require!(
        game.status == GameStatus::Started,
        GameError::InvalidGameStatus
    );

    require!(
        game.game_type == GameType::OMNICHAIN || game.game_type == GameType::SINGLECHAIN,
        GameError::InvalidGameType
    );

    let prize_fractions_sum: u64 = prize_fractions.iter().sum();
    require!(
        game.used_fractions + prize_fractions_sum <= game.total_fractions,
        GameError::InvalidPrizeFractions
    );

    game.used_fractions += prize_fractions_sum;

    require!(
        winners.len() == prize_fractions.len(),
        GameError::PrizeFractionsAndWinnersMismatch
    );

    // Determine the base_fee to be used based on the fee_type.
    // This is needed to know exactly the fees to send to the platform wallet and the beneficiaries claim update
    let fee_meta = &mut ctx.accounts.fee_meta;

    let base_fee;
    if fee_type == 0 {
        base_fee = ctx.accounts.config.fees_config.base_fee;
    } else {
        base_fee = fee_meta.base_fee;
    }

    let prize_pool = game.wager * game.participants.len() as u64;

    let total_reward = prize_pool - (prize_pool * base_fee / BP_DEC);

    let mut fee = prize_pool - total_reward;

    if fee_type != 0 {
        // Update beneficiaries claim
        //   - This ensures the beneficiaries can claim their fees later from the treasury vault (do not reach CUs limit)
        match fee_meta.update_beneficiaries_claim(prize_pool) {
            Ok(val) => {
                // Val is the remaining fee after updating the beneficiaries claim
                fee = val;
            }
            Err(err) => {
                panic!("Error: {}", err);
            }
        }

        emit!(UpdateBeneficiariesClaimEvent {
            fee_type,
            total_amount: prize_pool,
            beneficiaries: fee_meta.beneficiaries.clone(),
        });
    }

    // Send fees to the platform vault: only if fee > 0
    // Explanation: fee != 0 only if fee_type = 0 of if fee_meta.fractions < fee_meta.base_fee
    // In that case the platform wallet will receive the fee from the gam vault:

    if fee > 0 {
        let bump = &[ctx.bumps.game];
        let binding = game.id.to_le_bytes();
        let seed = &[GENOME_ROOT, b"GAME", &binding, bump][..];
        let seeds = &[seed];
        let accounts = TransferChecked {
            from: ctx.accounts.game_vault.to_account_info(),
            mint: ctx.accounts.mint.to_account_info(),
            to: ctx.accounts.platform_wallet_vault.to_account_info(),
            authority: game.to_account_info(),
        };
        let cpi = CpiContext::new_with_signer(
            ctx.accounts.token_program.to_account_info(),
            accounts,
            seeds,
        );
        transfer_checked(cpi, fee, ctx.accounts.mint.decimals)?;

        emit!(SendPlatformWalletEvent {
            fee_type,
            amount: fee,
            platform_wallet: *ctx.accounts.platform_wallet.key,
        });
    }

    // Increment the winners pending to claim fees
    for (i, winner) in winners.iter().enumerate() {
        require!(game.participants.contains(winner), GameError::InvalidWinner);

        game.winners.push(*winner);

        let winner_reward = total_reward * prize_fractions[i] / BP_DEC;

        let (expected_pda, bump) =
            Pubkey::find_program_address(&[GENOME_ROOT, b"USER", winner.as_ref()], &ctx.program_id);

        // Search for the expected PDA account
        let pda_account = ctx
            .remaining_accounts
            .iter()
            .find(|account| account.key() == expected_pda)
            .ok_or(GameError::InvalidWinner)?;

        if pda_account.to_account_info().data_is_empty() {
            init_user_info(
                *winner,
                pda_account,
                bump,
                *ctx.program_id,
                ctx.accounts.operator.to_account_info(),
                ctx.accounts.system_program.to_account_info(),
            )?;
        }

        // Update the winner's claimable amount
        update_claimable_amount(*winner, winner_reward, pda_account.clone())?;
    }

    // set game status to finished once entire pool of winnings has been distributed
    if game.used_fractions == game.total_fractions {
        game.status = GameStatus::Finished;
    }

    emit!(FinishGameEvent {
        game_id: game.id,
        game_type: game.game_type.clone(),
        winners: game.winners.clone(),
        rewards: total_reward,
    });

    // 4) Send remaining tokens from the game vault to the operator vault and then to the treasury vault
    // reload the game vault account
    ctx.accounts.game_vault.reload()?;
    send_game_remaining_tokens_to_treasury(
        ctx.accounts.game_vault.amount,
        ctx.accounts.mint.to_account_info(),
        ctx.accounts.mint.decimals,
        ctx.accounts.treasury_vault.to_account_info(),
        ctx.accounts.game_vault.to_account_info(),
        ctx.accounts.game.to_account_info(),
        ctx.accounts.game.id,
        ctx.bumps.game,
        ctx.accounts.token_program.to_account_info(),
    )?;

    Ok(())
}

#[derive(Debug)]
#[event]
pub struct FinishGameEvent {
    pub game_id: u64,
    pub winners: Vec<Pubkey>,
    pub game_type: GameType,
    pub rewards: u64,
}
