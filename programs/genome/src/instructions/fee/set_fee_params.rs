use anchor_lang::prelude::*;

use crate::{
    error::{ControlAccessError, FeesError},
    FeeMeta, OperatorInfo, Role, GenomeConfig, BP_DEC, GENOME_ROOT,
};

#[derive(Accounts)]
#[instruction(fee_type:u16)]
pub struct SetFeeParams<'info> {
    #[account(mut)]
    pub operator: Signer<'info>,

    #[account(seeds = [GENOME_ROOT, b"OPERATOR", operator.key().as_ref()], bump, constraint = operator_info.approved @ ControlAccessError::OperatorNotApproved, constraint = (operator_info.role == Role::DEVELOPER || operator_info.role == Role::OWNER) @ ControlAccessError::OperatorNotDeveloper)]
    pub operator_info: Box<Account<'info, OperatorInfo>>,

    #[account(init_if_needed, payer = operator, space = FeeMeta::LEN, seeds = [GENOME_ROOT, b"FEE_META", &fee_type.to_le_bytes().as_ref()], bump)]
    pub fee_meta: Box<Account<'info, FeeMeta>>,

    #[account(mut, seeds = [GENOME_ROOT, b"CONFIG"], bump)]
    pub config: Box<Account<'info, GenomeConfig>>,

    pub system_program: Program<'info, System>,
}

pub fn handle_set_fee_params(
    ctx: Context<SetFeeParams>,
    fee_type: u16,
    wallet: Pubkey,
    base_fee_param: u64,
    beneficiaries: Vec<Pubkey>,
    fractions: Vec<u64>,
    base_fee_meta: u64,
) -> Result<()> {
    let config: &mut Box<Account<'_, GenomeConfig>> = &mut ctx.accounts.config;

    if fee_type != 0 {
        require!(
            beneficiaries.len() == fractions.len(),
            FeesError::InvalidFeeLength
        );
        require!(base_fee_meta <= BP_DEC, FeesError::InvalidFee);
        let fractions_sum = fractions.iter().sum::<u64>();
        require!(fractions_sum <= base_fee_meta, FeesError::InvalidFee);

        let mut new_pending_to_claim = vec![0; beneficiaries.len()];

        let fee_meta = &mut ctx.accounts.fee_meta;

        // Logic to handle pending to claim tokens of the beneficiaries
        if fee_meta.beneficiaries.len() > 0 {
            for (i, beneficiary) in fee_meta.beneficiaries.iter().enumerate() {
                let beneficiary_pending = fee_meta.pending_to_claim[i];
                if let Some(pos) = beneficiaries.iter().position(|b| b == beneficiary) {
                    // The Pending to claim remains the same as before
                    new_pending_to_claim[pos] = beneficiary_pending;
                } else {
                    // If the beneficiary is present in the previous list but not in the new list, and has pending to claim tokens
                    // Throw an error
                    require!(beneficiary_pending == 0, FeesError::BeneficiaryPendingFees);
                }
            }
        }

        fee_meta.beneficiaries = beneficiaries.clone();
        fee_meta.pending_to_claim = new_pending_to_claim;
        fee_meta.fractions = fractions.clone();
        fee_meta.base_fee = base_fee_meta;
    }

    if base_fee_param != 0 {
        require!(base_fee_param <= BP_DEC / 4, FeesError::InvalidFee);
        config.fees_config.base_fee = base_fee_param;
    }

    require!(wallet != Pubkey::default(), FeesError::InvalidWallet);
    config.fees_config.platform_wallet = wallet;

    // Emit events
    emit!(FeeTypeAddedEvent {
        fee_type,
        base_fee: base_fee_meta,
        beneficiaries: beneficiaries.clone(),
        fractions: fractions.clone()
    });
    emit!(BaseFeeChangedEvent {
        base_fee: base_fee_param
    });
    emit!(PlatformWalletUpgradedEvent { wallet });

    Ok(())
}

#[derive(Debug)]
#[event]
pub struct FeeTypeAddedEvent {
    pub fee_type: u16,
    pub base_fee: u64,
    pub beneficiaries: Vec<Pubkey>,
    pub fractions: Vec<u64>,
}

#[derive(Debug)]
#[event]
pub struct BaseFeeChangedEvent {
    pub base_fee: u64,
}

#[derive(Debug)]
#[event]
pub struct PlatformWalletUpgradedEvent {
    pub wallet: Pubkey,
}
