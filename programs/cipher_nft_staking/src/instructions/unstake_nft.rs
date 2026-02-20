use anchor_lang::prelude::*;
use anchor_lang::solana_program::{
    instruction::{AccountMeta, Instruction},
    program::invoke_signed,
};

use crate::state::*;
use crate::events::*;
use crate::error::*;

/// Metaplex Core program ID
const MPL_CORE_PROGRAM_ID: Pubkey = pubkey!("CoREENxT6tW1HoK8ypY1SxRMZTcVPm7R94rH4PZNhX7d");

/// SPL Noop program ID — required by MPL Core as log wrapper
const SPL_NOOP_PROGRAM_ID: Pubkey = pubkey!("noopb9bkMVfRPU8AsbpTUg8AQkHtKwMYZiFUjNRtMmV");

/// Unstake an NFT by thawing it
#[derive(Accounts)]
pub struct UnstakeNft<'info> {
    /// The NFT owner who is unstaking
    #[account(mut)]
    pub owner: Signer<'info>,

    /// The Core Asset account (the NFT itself)
    /// CHECK: Validated by mpl-core program
    #[account(mut)]
    pub asset: UncheckedAccount<'info>,

    /// The collection that this asset belongs to
    /// CHECK: Read from stake account
    #[account(mut)]
    pub collection: UncheckedAccount<'info>,

    /// The stake account
    #[account(
        mut,
        close = owner,
        seeds = [StakeAccount::SEED_PREFIX, asset.key().as_ref(), owner.key().as_ref()],
        bump = stake_account.bump,
        constraint = stake_account.nft_mint == asset.key() @ StakingError::StakeAccountMismatch,
        constraint = stake_account.is_active @ StakingError::StakeAccountMismatch,
        constraint = stake_account.nft_type == 2 @ StakingError::InvalidNftType
    )]
    pub stake_account: Account<'info, StakeAccount>,

    /// The collection config
    #[account(
        mut,
        seeds = [CollectionConfig::SEED_PREFIX, stake_account.collection.as_ref()],
        bump = collection_config.bump
    )]
    pub collection_config: Account<'info, CollectionConfig>,

    /// Global config
    #[account(
        mut,
        seeds = [GlobalConfig::SEED_PREFIX],
        bump = config.bump
    )]
    pub config: Account<'info, GlobalConfig>,

    /// Freeze delegate PDA - current freeze authority
    /// CHECK: PDA derived, must match the freeze delegate
    #[account(
        seeds = [b"freeze_delegate", owner.key().as_ref()],
        bump
    )]
    pub freeze_delegate: UncheckedAccount<'info>,

    /// MPL Core program
    /// CHECK: Must match mpl-core program ID
    pub mpl_core_program: UncheckedAccount<'info>,

    /// SPL Noop program (log wrapper) - required by MPL Core
    /// CHECK: Must match noop program ID
    pub log_wrapper: UncheckedAccount<'info>,

    pub system_program: Program<'info, System>,
}

/// Unlock a Core Asset by removing FreezeDelegate plugin
fn thaw_core_asset<'info>(
    asset: &AccountInfo<'info>,
    collection: &AccountInfo<'info>,
    payer: &AccountInfo<'info>,
    authority: &AccountInfo<'info>,
    mpl_core_program: &AccountInfo<'info>,
    log_wrapper: &AccountInfo<'info>,
    system_program: &AccountInfo<'info>,
    signer_seeds: &[&[&[u8]]],
) -> Result<()> {
    // Verify program IDs (VULN-03: pin log_wrapper to SPL Noop)
    require_keys_eq!(
        *mpl_core_program.key,
        MPL_CORE_PROGRAM_ID,
        StakingError::InvalidDelegate
    );
    require_keys_eq!(
        *log_wrapper.key,
        SPL_NOOP_PROGRAM_ID,
        StakingError::InvalidDelegate
    );

    // RemovePluginV1 instruction discriminator (1 byte)
    let discriminator: u8 = 4;

    // Build instruction data: discriminator + PluginType enum (u8 NOT u32!)
    let mut data = Vec::new();
    data.push(discriminator);
    // PluginType::FreezeDelegate = 1 (u8)
    data.push(1u8);

    let accounts = vec![
        AccountMeta::new(*asset.key, false),
        AccountMeta::new(*collection.key, false), // Must be writable for MPL Core
        AccountMeta::new(*payer.key, true),
        AccountMeta::new_readonly(*authority.key, true),
        AccountMeta::new_readonly(*system_program.key, false),
        AccountMeta::new_readonly(*log_wrapper.key, false), // Required by MPL Core
    ];

    let ix = Instruction {
        program_id: *mpl_core_program.key,
        accounts,
        data,
    };

    invoke_signed(
        &ix,
        &[
            asset.clone(),
            collection.clone(),
            payer.clone(),
            authority.clone(),
            system_program.clone(),
            log_wrapper.clone(),
            mpl_core_program.clone(),
        ],
        signer_seeds,
    )?;

    msg!("FreezeDelegate plugin removed - NFT is UNLOCKED");

    Ok(())
}

pub fn handler(ctx: Context<UnstakeNft>) -> Result<()> {

    // VERIFY COLLECTION MATCHES STAKE RECORD (VULN-05 fix)
    // The `collection` UncheckedAccount is not constrained in the struct because
    // `stake_account` is deserialized after it. Validate here before any CPI.
    require_keys_eq!(
        ctx.accounts.collection.key(),
        ctx.accounts.stake_account.collection,
        StakingError::StakeAccountMismatch
    );

    // VERIFY OWNER
    ctx.accounts.stake_account.verify_owner(&ctx.accounts.owner.key())?;


    // CHECK LOCK PERIOD
    let clock = Clock::get()?;
    require!(
        ctx.accounts.stake_account.is_unlocked(clock.unix_timestamp),
        StakingError::StillLocked
    );


    // THAW THE CORE ASSET (remove freeze)
    let owner_key = ctx.accounts.owner.key();
    let seeds = &[
        b"freeze_delegate".as_ref(),
        owner_key.as_ref(),
        &[ctx.bumps.freeze_delegate],
    ];
    let signer_seeds = &[&seeds[..]];

    // SECURITY (VULN-02 fix): pass freeze_delegate PDA as authority, NOT owner.
    // The FreezeDelegate plugin's init_authority was set to this PDA during staking,
    // so MPL Core requires the PDA to sign the removal. invoke_signed provides
    // that signature via signer_seeds derived from [b"freeze_delegate", owner].
    thaw_core_asset(
        &ctx.accounts.asset.to_account_info(),
        &ctx.accounts.collection.to_account_info(),
        &ctx.accounts.owner.to_account_info(),
        &ctx.accounts.freeze_delegate.to_account_info(),
        &ctx.accounts.mpl_core_program.to_account_info(),
        &ctx.accounts.log_wrapper.to_account_info(),
        &ctx.accounts.system_program.to_account_info(),
        signer_seeds,
    )?;


    // INFO-01: Emergency pause intentionally does NOT block unstaking.
    // Pausing halts new stakes so users can always retrieve their locked NFTs
    // even during an incident. This is the standard safe-exit design.

    // UPDATE STATS
    let collection_config = &mut ctx.accounts.collection_config;
    collection_config.total_staked = collection_config
        .total_staked
        .checked_sub(1)
        .ok_or(StakingError::ArithmeticOverflow)?;

    let config = &mut ctx.accounts.config;
    config.total_stakes = config
        .total_stakes
        .checked_sub(1)
        .ok_or(StakingError::ArithmeticOverflow)?;


    // EMIT EVENT
    let total_staked_duration = ctx.accounts.stake_account.total_time_staked(clock.unix_timestamp);

    emit!(NftUnstaked {
        staker: ctx.accounts.stake_account.owner,
        nft_mint: ctx.accounts.stake_account.nft_mint,
        unstaked_at: clock.unix_timestamp,
        total_staked_duration,
    });

    msg!("NFT unstaked successfully");
    msg!("   Asset: {}", ctx.accounts.stake_account.nft_mint);
    msg!("   Owner: {}", ctx.accounts.stake_account.owner);
    msg!("   Total Staked: {} days", total_staked_duration / 86400);

    Ok(())
}
