use anchor_lang::prelude::*;
use anchor_spl::token::{self, Token, TokenAccount, Transfer};
use anchor_spl::associated_token::AssociatedToken;
use mpl_token_metadata::accounts::Metadata;
use mpl_token_metadata::types::TokenStandard;

use crate::state::*;
use crate::events::*;
use crate::error::*;

/// Stake an NFT by locking it in escrow
///
/// **Security Flow:**
/// 1. Verify NFT ownership
/// 2. Verify collection is whitelisted
/// 3. Verify lock duration is within bounds
/// 4. Transfer NFT to escrow PDA
/// 5. Create stake account
/// 6. Update stats
///
/// **Integration with Orbit Finance DLMM:**
/// Orbit Finance DLMM reads the StakeAccount to check if a user has an active
/// stake and is eligible for fee claims.
#[derive(Accounts)]
#[instruction(lock_duration: i64)]
pub struct StakeNft<'info> {
    /// The NFT owner who is staking
    #[account(mut)]
    pub owner: Signer<'info>,

    /// The NFT mint
    /// CHECK: Validated via metadata account
    pub nft_mint: UncheckedAccount<'info>,

    /// The owner's NFT token account (must hold exactly 1 token)
    #[account(
        mut,
        constraint = owner_nft_account.mint == nft_mint.key(),
        constraint = owner_nft_account.owner == owner.key(),
        constraint = owner_nft_account.amount == 1 @ StakingError::InvalidTokenAccount
    )]
    pub owner_nft_account: Account<'info, TokenAccount>,

    /// The escrow PDA that will hold the NFT
    /// Derived from nft_mint for unique escrow per NFT
    #[account(
        init_if_needed,
        payer = owner,
        associated_token::mint = nft_mint,
        associated_token::authority = escrow_authority
    )]
    pub escrow_nft_account: Account<'info, TokenAccount>,

    /// The escrow authority PDA (owns escrow_nft_account)
    /// Per-user escrow authority prevents any collision scenarios
    /// CHECK: PDA derived, no data
    #[account(
        seeds = [b"escrow_authority", owner.key().as_ref()],
        bump
    )]
    pub escrow_authority: UncheckedAccount<'info>,

    /// The NFT's metadata account (for collection verification)
    /// CHECK: Validated via Metaplex deserialization
    #[account(
        seeds = [
            b"metadata",
            mpl_token_metadata::ID.as_ref(),
            nft_mint.key().as_ref()
        ],
        bump,
        seeds::program = mpl_token_metadata::ID
    )]
    pub nft_metadata: UncheckedAccount<'info>,

    /// The stake account PDA (stores stake info)
    #[account(
        init,
        payer = owner,
        space = StakeAccount::LEN,
        seeds = [StakeAccount::SEED_PREFIX, nft_mint.key().as_ref(), owner.key().as_ref()],
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

    pub token_program: Program<'info, Token>,
    pub associated_token_program: Program<'info, AssociatedToken>,
    pub system_program: Program<'info, System>,
}

pub fn handler(ctx: Context<StakeNft>, lock_duration: i64, associated_pool: Option<Pubkey>) -> Result<()> {
    // ========================================
    // STEP 0: EXPLICIT CHECKS (MAXIMUM SECURITY)
    // ========================================

    // SECURITY: Prevent double-stake attempts with clear error
    // While init would fail anyway, this provides explicit error message
    require!(
        ctx.accounts.stake_account.to_account_info().lamports() == 0,
        StakingError::StakeAlreadyExists
    );

    // SECURITY: Verify owner actually owns the NFT token account
    // Double-check even though constraints exist
    require_keys_eq!(
        ctx.accounts.owner_nft_account.owner,
        ctx.accounts.owner.key(),
        StakingError::InvalidNftOwner
    );

    // SECURITY: Verify NFT token account has exactly 1 token
    // Prevents attempts to stake fungible tokens or empty accounts
    require_eq!(
        ctx.accounts.owner_nft_account.amount,
        1,
        StakingError::InvalidTokenAccount
    );

    // ========================================
    // STEP 1: VALIDATE METADATA & COLLECTION
    // ========================================

    let metadata_data = &ctx.accounts.nft_metadata.try_borrow_data()?;
    let metadata = Metadata::safe_deserialize(metadata_data)
        .map_err(|_| StakingError::InvalidMetadata)?;

    // Verify it's an NFT (not a fungible token)
    require!(
        metadata.token_standard == Some(TokenStandard::NonFungible) ||
        metadata.token_standard == Some(TokenStandard::ProgrammableNonFungible),
        StakingError::InvalidMetadata
    );

    // Verify collection matches
    let collection = metadata
        .collection
        .ok_or(StakingError::CollectionNotWhitelisted)?;

    require!(
        collection.verified,
        StakingError::MetadataVerificationFailed
    );

    require_keys_eq!(
        collection.key,
        ctx.accounts.collection_config.collection,
        StakingError::CollectionNotWhitelisted
    );

    // ========================================
    // STEP 2: VALIDATE LOCK DURATION
    // ========================================

    let collection_config = &ctx.accounts.collection_config;

    require!(
        lock_duration >= collection_config.min_lock_duration &&
        lock_duration <= collection_config.max_lock_duration,
        StakingError::InvalidLockDuration
    );

    // ========================================
    // STEP 3: TRANSFER NFT TO ESCROW
    // ========================================

    let transfer_ctx = CpiContext::new(
        ctx.accounts.token_program.to_account_info(),
        Transfer {
            from: ctx.accounts.owner_nft_account.to_account_info(),
            to: ctx.accounts.escrow_nft_account.to_account_info(),
            authority: ctx.accounts.owner.to_account_info(),
        },
    );

    token::transfer(transfer_ctx, 1)?;

    // ========================================
    // STEP 4: INITIALIZE STAKE ACCOUNT
    // ========================================

    let clock = Clock::get()?;
    let stake_account_key = ctx.accounts.stake_account.key();
    let stake_account = &mut ctx.accounts.stake_account;

    stake_account.owner = ctx.accounts.owner.key();
    stake_account.nft_mint = ctx.accounts.nft_mint.key();
    stake_account.collection = collection.key;
    stake_account.staked_at = clock.unix_timestamp;
    stake_account.unlock_at = clock.unix_timestamp
        .checked_add(lock_duration)
        .ok_or(StakingError::ArithmeticOverflow)?;
    stake_account.lock_duration = lock_duration;
    stake_account.bump = ctx.bumps.stake_account;
    stake_account.is_active = true;
    stake_account._reserved = [0; 6];
    stake_account.rewards_claimed = 0;
    stake_account.last_claim_at = clock.unix_timestamp;
    stake_account.associated_pool = associated_pool.unwrap_or(Pubkey::default());
    stake_account._padding = [0; 128];

    // ========================================
    // STEP 5: UPDATE STATS
    // ========================================

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

    // ========================================
    // STEP 6: EMIT EVENT
    // ========================================

    emit!(NftStaked {
        staker: stake_account.owner,
        nft_mint: stake_account.nft_mint,
        collection: stake_account.collection,
        staked_at: stake_account.staked_at,
        unlock_at: stake_account.unlock_at,
        lock_duration: stake_account.lock_duration,
        stake_account: stake_account_key,
    });

    msg!("✅ NFT staked successfully!");
    msg!("   NFT Mint: {}", stake_account.nft_mint);
    msg!("   Owner: {}", stake_account.owner);
    msg!("   Lock Duration: {} days", lock_duration / 86400);
    msg!("   Unlock At: {}", stake_account.unlock_at);

    Ok(())
}
