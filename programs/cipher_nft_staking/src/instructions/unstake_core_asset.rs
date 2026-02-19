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

/// Unstake a Metaplex Core Asset NFT by thawing it
#[derive(Accounts)]
pub struct UnstakeCoreAsset<'info> {
    /// The NFT owner who is unstaking
    #[account(mut)]
    pub owner: Signer<'info>,

    /// The Core Asset account (the NFT itself)
    /// CHECK: Validated by mpl-core program
    #[account(mut)]
    pub asset: UncheckedAccount<'info>,

    /// The collection that this asset belongs to
    /// CHECK: Read from stake account
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

    pub system_program: Program<'info, System>,
}

/// Thaw a Core Asset using raw CPI
fn thaw_core_asset<'info>(
    asset: &AccountInfo<'info>,
    collection: &AccountInfo<'info>,
    payer: &AccountInfo<'info>,
    freeze_delegate: &AccountInfo<'info>,
    mpl_core_program: &AccountInfo<'info>,
    signer_seeds: &[&[&[u8]]],
) -> Result<()> {
    // Verify program ID
    require_keys_eq!(
        *mpl_core_program.key,
        MPL_CORE_PROGRAM_ID,
        StakingError::InvalidDelegate
    );

    // MPL Core Thaw instruction discriminator (thaw_v1)
    let discriminator: [u8; 8] = [131, 191, 15, 35, 143, 97, 234, 31];

    let mut data = Vec::with_capacity(8);
    data.extend_from_slice(&discriminator);

    let accounts = vec![
        AccountMeta::new(*asset.key, false),
        AccountMeta::new(*collection.key, false),
        AccountMeta::new(*payer.key, true),
        AccountMeta::new_readonly(*freeze_delegate.key, true), // Authority must sign
        AccountMeta::new_readonly(anchor_lang::system_program::ID, false),
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
            freeze_delegate.clone(),
            mpl_core_program.clone(),
        ],
        signer_seeds,
    )?;

    Ok(())
}

pub fn handler(ctx: Context<UnstakeCoreAsset>) -> Result<()> {

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

    thaw_core_asset(
        &ctx.accounts.asset.to_account_info(),
        &ctx.accounts.collection.to_account_info(),
        &ctx.accounts.owner.to_account_info(),
        &ctx.accounts.freeze_delegate.to_account_info(),
        &ctx.accounts.mpl_core_program.to_account_info(),
        signer_seeds,
    )?;


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

    msg!("Core Asset NFT unstaked successfully");
    msg!("   Asset: {}", ctx.accounts.stake_account.nft_mint);
    msg!("   Owner: {}", ctx.accounts.stake_account.owner);
    msg!("   Total Staked: {} days", total_staked_duration / 86400);

    Ok(())
}
