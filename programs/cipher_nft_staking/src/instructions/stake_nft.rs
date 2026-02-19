use anchor_lang::prelude::*;
use anchor_lang::solana_program::{
    instruction::{AccountMeta, Instruction},
    program::invoke,
};
use borsh::{BorshSerialize, BorshDeserialize};

use crate::state::*;
use crate::events::*;
use crate::error::*;

/// Metaplex Core program ID
const MPL_CORE_PROGRAM_ID: Pubkey = pubkey!("CoREENxT6tW1HoK8ypY1SxRMZTcVPm7R94rH4PZNhX7d");

/// Stake an NFT by freezing it
#[derive(Accounts)]
#[instruction(lock_duration: i64)]
pub struct StakeNft<'info> {
    /// The NFT owner who is staking
    #[account(mut)]
    pub owner: Signer<'info>,

    /// The Core Asset account (the NFT itself)
    /// CHECK: Validated by mpl-core program
    #[account(mut)]
    pub asset: UncheckedAccount<'info>,

    /// The collection that this asset belongs to
    /// CHECK: Validated against whitelist
    pub collection: UncheckedAccount<'info>,

    /// The stake account PDA (stores stake info)
    #[account(
        init,
        payer = owner,
        space = StakeAccount::LEN,
        seeds = [StakeAccount::SEED_PREFIX, asset.key().as_ref(), owner.key().as_ref()],
        bump
    )]
    pub stake_account: Account<'info, StakeAccount>,

    /// The collection config (must be whitelisted)
    #[account(
        mut,
        seeds = [CollectionConfig::SEED_PREFIX, collection_config.collection.as_ref()],
        bump = collection_config.bump,
        constraint = collection_config.enabled @ StakingError::CollectionNotWhitelisted
    )]
    pub collection_config: Account<'info, CollectionConfig>,

    /// Global config
    #[account(
        mut,
        seeds = [GlobalConfig::SEED_PREFIX],
        bump = config.bump,
        constraint = !config.paused @ StakingError::ProgramPaused
    )]
    pub config: Account<'info, GlobalConfig>,

    /// Freeze delegate PDA - will be the freeze authority
    /// CHECK: PDA derived
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

/// FreezeDelegate plugin data
#[derive(BorshSerialize, BorshDeserialize)]
struct FreezeDelegate {
    frozen: bool,
}

/// Freeze a Core Asset by adding FreezeDelegate plugin
fn freeze_core_asset<'info>(
    asset: &AccountInfo<'info>,
    collection: &AccountInfo<'info>,
    payer: &AccountInfo<'info>,
    authority: &AccountInfo<'info>,
    mpl_core_program: &AccountInfo<'info>,
    system_program: &AccountInfo<'info>,
) -> Result<()> {
    // Verify program ID
    require_keys_eq!(
        *mpl_core_program.key,
        MPL_CORE_PROGRAM_ID,
        StakingError::InvalidDelegate
    );

    // AddPluginV1 instruction discriminator
    let discriminator: u8 = 2;

    // Build instruction data: discriminator + plugin variant (1) + FreezeDelegate data
    let mut data = Vec::new();
    data.push(discriminator);

    // Plugin enum variant (FreezeDelegate = 1)
    data.push(1);

    // FreezeDelegate data (frozen = true)
    let freeze_data = FreezeDelegate { frozen: true };
    freeze_data.serialize(&mut data)?;

    let accounts = vec![
        AccountMeta::new(*asset.key, false),
        AccountMeta::new_readonly(*collection.key, false),
        AccountMeta::new(*payer.key, true),
        AccountMeta::new_readonly(*authority.key, true),
        AccountMeta::new_readonly(*system_program.key, false),
    ];

    let ix = Instruction {
        program_id: *mpl_core_program.key,
        accounts,
        data,
    };

    invoke(
        &ix,
        &[
            asset.clone(),
            collection.clone(),
            payer.clone(),
            authority.clone(),
            system_program.clone(),
            mpl_core_program.clone(),
        ],
    )?;

    msg!("FreezeDelegate plugin added - NFT is LOCKED");

    Ok(())
}

pub fn handler(ctx: Context<StakeNft>, lock_duration: i64, associated_pool: Option<Pubkey>) -> Result<()> {

    // VALIDATE LOCK DURATION
    let collection_config = &ctx.accounts.collection_config;

    require!(
        lock_duration >= collection_config.min_lock_duration &&
        lock_duration <= collection_config.max_lock_duration,
        StakingError::InvalidLockDuration
    );


    // FREEZE THE CORE ASSET
    freeze_core_asset(
        &ctx.accounts.asset.to_account_info(),
        &ctx.accounts.collection.to_account_info(),
        &ctx.accounts.owner.to_account_info(),
        &ctx.accounts.owner.to_account_info(),
        &ctx.accounts.mpl_core_program.to_account_info(),
        &ctx.accounts.system_program.to_account_info(),
    )?;


    // INITIALIZE STAKE ACCOUNT
    let clock = Clock::get()?;
    let stake_account_key = ctx.accounts.stake_account.key();
    let stake_account = &mut ctx.accounts.stake_account;

    stake_account.owner = ctx.accounts.owner.key();
    stake_account.nft_mint = ctx.accounts.asset.key();  // Store asset address in nft_mint field
    stake_account.collection = ctx.accounts.collection_config.collection;
    stake_account.staked_at = clock.unix_timestamp;
    stake_account.unlock_at = clock.unix_timestamp
        .checked_add(lock_duration)
        .ok_or(StakingError::ArithmeticOverflow)?;
    stake_account.lock_duration = lock_duration;
    stake_account.bump = ctx.bumps.stake_account;
    stake_account.is_active = true;
    stake_account._reserved = [0; 6];
    stake_account.associated_pool = associated_pool.unwrap_or(Pubkey::default());
    stake_account.nft_type = 2; // Core Asset
    stake_account.leaf_index = 0; // Not used for Core Assets
    stake_account._padding = [0; 135];


    // UPDATE STATS
    let collection_config = &mut ctx.accounts.collection_config;
    collection_config.total_staked = collection_config
        .total_staked
        .checked_add(1)
        .ok_or(StakingError::ArithmeticOverflow)?;
    collection_config.lifetime_stakes = collection_config
        .lifetime_stakes
        .checked_add(1)
        .ok_or(StakingError::ArithmeticOverflow)?;

    let config = &mut ctx.accounts.config;
    config.total_stakes = config
        .total_stakes
        .checked_add(1)
        .ok_or(StakingError::ArithmeticOverflow)?;


    // EMIT EVENT
    emit!(NftStaked {
        staker: stake_account.owner,
        nft_mint: stake_account.nft_mint,
        collection: stake_account.collection,
        staked_at: stake_account.staked_at,
        unlock_at: stake_account.unlock_at,
        lock_duration: stake_account.lock_duration,
        stake_account: stake_account_key,
    });

    msg!("NFT staked successfully");
    msg!("   Asset: {}", stake_account.nft_mint);
    msg!("   Owner: {}", stake_account.owner);
    msg!("   Lock Duration: {} days", lock_duration / 86400);
    msg!("   Unlock At: {}", stake_account.unlock_at);

    Ok(())
}
