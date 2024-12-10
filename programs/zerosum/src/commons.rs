use anchor_lang::prelude::*;
use std::collections::HashSet;

use solana_program::{program::invoke_signed, system_instruction};

use anchor_spl::{
    token::spl_token::{self, state::GenericTokenAccount},
    token_2022::TransferChecked,
    token_interface::transfer_checked,
};

use crate::{error::GameError, ClaimableUserInfo, TournamentParticipant, ZS_ROOT};

pub fn init_user_info<'info>(
    user: Pubkey,
    pda_account_info: &AccountInfo<'info>,
    bump: u8,
    program_id: Pubkey,
    operator_account_info: AccountInfo<'info>,
    system_program_account_info: AccountInfo<'info>,
) -> Result<()> {
    let rent = Rent::get()?.minimum_balance(ClaimableUserInfo::LEN);
    let create_pda_account_ix = system_instruction::create_account(
        &operator_account_info.key(),
        &pda_account_info.key(),
        rent,
        ClaimableUserInfo::LEN.try_into().unwrap(),
        &program_id,
    );

    let signers_seeds: &[&[u8]] = &[ZS_ROOT, b"USER", user.as_ref(), &[bump]];

    invoke_signed(
        &create_pda_account_ix,
        &[
            operator_account_info,
            pda_account_info.clone(),
            system_program_account_info,
        ],
        &[signers_seeds],
    )?;
    Ok(())
}


pub fn init_tournament_participant_account<'info>(
    user: Pubkey,
    pda_account_info: &AccountInfo<'info>,
    bump: u8,
    program_id: Pubkey,
    operator_account_info: AccountInfo<'info>,
    system_program_account_info: AccountInfo<'info>,
    _tournament_id: u64
) -> Result<()> {
    let rent = Rent::get()?.minimum_balance(TournamentParticipant::LEN);
    let create_pda_account_ix = system_instruction::create_account(
        &operator_account_info.key(),
        &pda_account_info.key(),
        rent,
        TournamentParticipant::LEN.try_into().unwrap(),
        &program_id,
    );
    let tournament_id = _tournament_id.to_le_bytes();
    let signers_seeds: &[&[u8]] = &[ZS_ROOT, b"TEAM_PARTICIPANT", &tournament_id.as_ref(), user.as_ref(), &[bump]];

    invoke_signed(
        &create_pda_account_ix,
        &[
            operator_account_info,
            pda_account_info.clone(),
            system_program_account_info,
        ],
        &[signers_seeds],
    )?;
    Ok(())
}

pub fn update_claimable_amount<'info>(
    user: Pubkey,
    amount: u64,
    pda_account_info: AccountInfo<'info>,
) -> Result<()> {
    let discriminator =
        &anchor_lang::solana_program::hash::hash(b"account:ClaimableUserInfo").to_bytes()[..8];

    // Add the discriminator to the account data
    {
        let mut pda_data = pda_account_info.data.borrow_mut();
        if pda_data[..8] != *discriminator {
            pda_data[..8].copy_from_slice(discriminator);
        }
    }
    let mut pda_account_state =
        ClaimableUserInfo::try_from_slice(&pda_account_info.data.borrow()[8..])?;

    pda_account_state.claimable += amount;
    pda_account_state.user = user;

    {
        let mut pda_data = pda_account_info.data.borrow_mut();
        pda_data[..8].copy_from_slice(discriminator); // The discriminator is the first 8 bytes
        pda_account_state.serialize(&mut &mut pda_data[8..])?; // data is the rest of the account data
    }

    emit!(UpdateUserClaimEvent {
        user: user.to_bytes()[0] as u16,
        amount: amount,
    });

    Ok(())
}

pub fn send_game_remaining_tokens_to_treasury<'info>(
    amount: u64,
    mint_account_info: AccountInfo<'info>,
    mint_decimals: u8,
    treasury_vault_account_info: AccountInfo<'info>,
    game_vault_account_info: AccountInfo<'info>,
    game_account_info: AccountInfo<'info>,
    game_id: u64,
    bump: u8,
    token_program_account_info: AccountInfo<'info>,
) -> Result<()> {
    // Send game vault remaining tokens to treasury vault
    let game_amount = amount;

    if game_amount == 0 {
        return Ok(());
    }

    let bump = &[bump];
    let binding = game_id.to_le_bytes();
    let seed = &[ZS_ROOT, b"GAME", &binding, bump][..];
    let seeds = &[seed];
    let accounts = TransferChecked {
        from: game_vault_account_info,
        mint: mint_account_info,
        to: treasury_vault_account_info,
        authority: game_account_info,
    };
    let cpi = CpiContext::new_with_signer(token_program_account_info, accounts, seeds);
    transfer_checked(cpi, game_amount, mint_decimals)?;

    Ok(())
}

pub fn send_beneficiaries_book_tokens_to_treasury<'info>(
    amount: u64,
    mint_account_info: AccountInfo<'info>,
    mint_decimals: u8,
    treasury_vault_account_info: AccountInfo<'info>,
    tournament_book_vault_account_info: AccountInfo<'info>,
    tournament_book_account_info: AccountInfo<'info>,
    tournament_id: u64,
    bump: u8,
    token_program_account_info: AccountInfo<'info>,
) -> Result<()> {
    let book_amount = amount;

    if book_amount == 0 {
        return Ok(());
    }

    let bump = &[bump];
    let binding = tournament_id.to_le_bytes();
    let seed = &[ZS_ROOT, b"BOOK", &binding, bump][..];
    let seeds = &[seed];
    let accounts = TransferChecked {
        from: tournament_book_vault_account_info,
        mint: mint_account_info,
        to: treasury_vault_account_info,
        authority: tournament_book_account_info,
    };
    let cpi = CpiContext::new_with_signer(token_program_account_info, accounts, seeds);
    transfer_checked(cpi, book_amount, mint_decimals)?;

    Ok(())
}

pub fn validate_game_participant_vault_token(
    participant_vault: &AccountInfo,
    mint_account_info: AccountInfo,
) -> Result<()> {
    let participant_vault_data = participant_vault.data.borrow();

    let ata_mint = spl_token::state::Account::unpack_account_mint(&participant_vault_data)
        .ok_or(GameError::InvalidParticipantVault)?;

    require!(
        *ata_mint == *mint_account_info.key,
        GameError::InvalidParticipantVault
    );

    Ok(())
}

pub fn are_all_participants_unique(participants: Vec<Pubkey>) -> bool {
    let unique_participants: HashSet<_> = participants.iter().collect();
    unique_participants.len() == participants.len()
}

#[derive(Debug)]
#[event]
pub struct UpdateUserClaimEvent {
    pub user: u16,
    pub amount: u64,
}

#[derive(Debug)]
#[event]
pub struct SendPlatformWalletEvent {
    pub fee_type: u16,
    pub amount: u64,
    pub platform_wallet: Pubkey,
}

#[derive(Debug)]
#[event]
pub struct UpdateBeneficiariesClaimEvent {
    pub fee_type: u16,
    pub total_amount: u64,
    pub beneficiaries: Vec<Pubkey>,
}
