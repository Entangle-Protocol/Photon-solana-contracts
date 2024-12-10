use anchor_lang::prelude::*;
use anchor_spl::{
    metadata::mpl_token_metadata::accounts::Metadata,
    token::Token,
    token_interface::{mint_to, Mint, MintTo, TokenAccount},
};

declare_id!("FmHwfH7HvAoD7HNvwX71ffUF3G2ejJoPDcZr4kBu5Y2a");

#[cfg(feature = "devnet")]
pub const DEPLOYER: &str = "7TTGdm9A74mq4eakqwUodpUQoyAYwvg7LDwZMv8UdD58";
#[cfg(not(feature = "devnet"))]
pub const DEPLOYER: &str = "J85q2bNo4FadDqDmUYPLKav14QRexShwEQXxhbkuvEP2";

#[cfg(feature = "devnet")]
pub const ROOT: &[u8] = b"r0";
#[cfg(not(feature = "devnet"))]
pub const ROOT: &[u8] = b"r0";

pub const NGL_DECIMALS: u8 = 6;

#[program]
pub mod ngl_core {
    use anchor_spl::{
        metadata::{
            create_metadata_accounts_v3, mpl_token_metadata::types::DataV2,
            CreateMetadataAccountsV3,
        },
        token_interface::{self, Burn},
    };

    use super::*;

    pub fn initialize(
        ctx: Context<Initialize>,
        bridge_authority: Pubkey,
        name: String,
        symbol: String,
        uri: String,
    ) -> Result<()> {
        ctx.accounts.config.admin = ctx.accounts.admin.key();
        ctx.accounts.config.contracts[0] = bridge_authority;
        ctx.accounts.config.mint = ctx.accounts.mint.key();
        let accounts = CreateMetadataAccountsV3 {
            metadata: ctx.accounts.metadata_account.to_account_info(),
            mint: ctx.accounts.mint.to_account_info(),
            mint_authority: ctx.accounts.authority.to_account_info(),
            update_authority: ctx.accounts.authority.to_account_info(),
            payer: ctx.accounts.admin.to_account_info(),
            system_program: ctx.accounts.system_program.to_account_info(),
            rent: ctx.accounts.rent.to_account_info(),
        };
        let data_v2 = DataV2 {
            name,
            symbol,
            uri,
            seller_fee_basis_points: 0,
            creators: None,
            collection: None,
            uses: None,
        };
        let seed = &[ROOT, &b"AUTHORITY"[..], &[ctx.bumps.authority][..]];
        let seeds = &[&seed[..]];
        let cpi = CpiContext::new_with_signer(
            ctx.accounts.token_metadata_program.to_account_info(),
            accounts,
            seeds,
        );
        create_metadata_accounts_v3(cpi, data_v2, true, true, None)?;
        Ok(())
    }

    pub fn mint_token(ctx: Context<MintToken>, amount: u64) -> Result<()> {
        require_gt!(amount, 0, CustomError::ZeroAmount);
        let cpi_accounts = MintTo {
            mint: ctx.accounts.mint.to_account_info(),
            to: ctx.accounts.vault.to_account_info(),
            authority: ctx.accounts.authority.to_account_info(),
        };
        let seed = &[ROOT, &b"AUTHORITY"[..], &[ctx.bumps.authority][..]];
        let seeds = &[&seed[..]];
        let cpi_ctx = CpiContext::new_with_signer(
            ctx.accounts.token_program.to_account_info(),
            cpi_accounts,
            seeds,
        );
        mint_to(cpi_ctx, amount)
    }

    pub fn burn_token(ctx: Context<BurnToken>, amount: u64) -> Result<()> {
        require_gt!(amount, 0, CustomError::ZeroAmount);
        let accounts = Burn {
            mint: ctx.accounts.mint.to_account_info(),
            from: ctx.accounts.vault.to_account_info(),
            authority: ctx.accounts.vault_owner.to_account_info(),
        };
        let cpi = CpiContext::new(ctx.accounts.token_program.to_account_info(), accounts);
        token_interface::burn(cpi, amount)
    }

    pub fn set_admin(ctx: Context<SetAdmin>, admin: Pubkey) -> Result<()> {
        ctx.accounts.config.admin = admin;
        Ok(())
    }
}

#[derive(Accounts)]
pub struct Initialize<'info> {
    /// Admin wallet
    #[account(signer, mut)]
    pub admin: Signer<'info>,

    /// Token authority
    /// CHECK: not loaded
    #[account(seeds = [ROOT, b"AUTHORITY"], bump)]
    pub authority: UncheckedAccount<'info>,

    /// Token mint
    #[account(
        mut,
        mint::authority = authority,
        mint::decimals = NGL_DECIMALS,
        mint::token_program = token_program
    )]
    pub mint: Box<InterfaceAccount<'info, Mint>>,

    /// System config
    #[account(init, payer = admin, space = Config::LEN, seeds = [ROOT, b"CONFIG"], bump)]
    pub config: Box<Account<'info, Config>>,

    /// MPL metadata
    /// CHECK: by metadata program
    #[account(
        mut,
        address = Metadata::find_pda(&mint.key()).0,
    )]
    pub metadata_account: AccountInfo<'info>,

    pub rent: Sysvar<'info, Rent>,
    pub token_metadata_program: Program<'info, anchor_spl::metadata::Metadata>,
    pub token_program: Program<'info, Token>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct MintToken<'info> {
    /// Mint authority
    /// CHECK: not loaded
    #[account(
        signer,
        constraint = 
            mint_authority.key() != Pubkey::default() && config.contracts.contains(&mint_authority.key())
                @ CustomError::Unauthorized
    )]
    pub mint_authority: Signer<'info>,

    /// Token authority
    /// CHECK: not loaded
    #[account(seeds = [ROOT, b"AUTHORITY"], bump)]
    pub authority: UncheckedAccount<'info>,

    /// System config
    #[account(seeds = [ROOT, b"CONFIG"], bump)]
    pub config: Box<Account<'info, Config>>,

    /// Token mint
    #[account(
        mut,
        address = config.mint,
        mint::authority = authority,
        mint::decimals = NGL_DECIMALS,
        mint::token_program = token_program
    )]
    pub mint: Box<InterfaceAccount<'info, Mint>>,

    /// Target vault
    #[account(mut, token::mint = mint, mint::token_program = token_program)]
    pub vault: Box<InterfaceAccount<'info, TokenAccount>>,

    pub token_program: Program<'info, Token>,
}

#[derive(Accounts)]
pub struct BurnToken<'info> {
    #[account(signer)]
    pub vault_owner: Signer<'info>,

    /// Burn authority
    /// CHECK: not loaded
    #[account(
        signer,
        constraint = 
            burn_authority.key() != Pubkey::default() && config.contracts.contains(&burn_authority.key())
                @ CustomError::Unauthorized
    )]
    pub burn_authority: Signer<'info>,

    /// System config
    #[account(seeds = [ROOT, b"CONFIG"], bump)]
    pub config: Box<Account<'info, Config>>,

    /// Token mint
    #[account(
        mut,
        address = config.mint,
        mint::decimals = NGL_DECIMALS,
        mint::token_program = token_program
    )]
    pub mint: Box<InterfaceAccount<'info, Mint>>,

    /// Source vault
    #[account(
        mut, 
        token::authority = vault_owner,
        token::mint = mint,
        mint::token_program = token_program
    )]
    pub vault: Box<InterfaceAccount<'info, TokenAccount>>,

    pub token_program: Program<'info, Token>,
}

#[derive(Accounts)]
pub struct SetAdmin<'info> {
    /// Deployer address
    #[account(signer, address = DEPLOYER.parse().expect("Deployer key not set"))]
    pub deployer: Signer<'info>,

    /// Core config
    #[account(mut, seeds = [ROOT, b"CONFIG"], bump)]
    pub config: Box<Account<'info, Config>>,
}

#[account]
#[derive(Default)]
pub struct Config {
    pub admin: Pubkey,
    pub contracts: [Pubkey; 3],
    pub mint: Pubkey,
}

impl Config {
    pub const LEN: usize = 8 + 32 * 5;
}

#[error_code]
pub enum CustomError {
    #[msg("Unauthorized")]
    Unauthorized,
    #[msg("ZeroAmount")]
    ZeroAmount,
}
