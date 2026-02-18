use anchor_lang::prelude::*;
use anchor_spl::token::{self, Token, TokenAccount, Transfer};

use crate::state::*;
use crate::events::*;
use crate::error::*;

/// Unstake an NFT after lock period expires
///
/// **Security:**
/// - Verifies lock period has expired
/// - Verifies owner matches
/// - Transfers NFT back to owner
/// - Closes stake account (rent refund)
#[derive(Accounts)]
pub struct UnstakeNft<'info> {
    /// The NFT owner who is unstaking
    #[account(mut)]
    pub owner: Signer<'info>,

    /// The NFT mint
    /// CHECK: Validated via stake account
    pub nft_mint: UncheckedAccount<'info>,

    /// The owner's NFT token account (receives NFT back)
    #[account(
        mut,
        constraint = owner_nft_account.mint == nft_mint.key(),
        constraint = owner_nft_account.owner == owner.key()
    )]
    pub owner_nft_account: Account<'info, TokenAccount>,

    /// The escrow account holding the NFT
    #[account(
        mut,
        constraint = escrow_nft_account.mint == nft_mint.key(),
        constraint = escrow_nft_account.amount == 1 @ StakingError::InvalidTokenAccount
    )]
    pub escrow_nft_account: Account<'info, TokenAccount>,

    /// The escrow authority PDA (per-user, matches stake_nft)
    /// CHECK: PDA, used for signing
    #[account(
        seeds = [b"escrow_authority", owner.key().as_ref()],
        bump
    )]
    pub escrow_authority: UncheckedAccount<'info>,

    /// The stake account
    #[account(
        mut,
        close = owner,
        seeds = [StakeAccount::SEED_PREFIX, nft_mint.key().as_ref(), owner.key().as_ref()],
        bump = stake_account.bump,
        constraint = stake_account.nft_mint == nft_mint.key() @ StakingError::StakeAccountMismatch,
        constraint = stake_account.is_active @ StakingError::StakeAccountMismatch
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

    pub token_program: Program<'info, Token>,
}

pub fn handler(ctx: Context<UnstakeNft>) -> Result<()> {
    // ========================================
    // STEP 1: VERIFY OWNER
    // ========================================

    ctx.accounts.stake_account.verify_owner(&ctx.accounts.owner.key())?;

    // ========================================
    // STEP 2: CHECK LOCK PERIOD
    // ========================================

    let clock = Clock::get()?;
    require!(
        ctx.accounts.stake_account.is_unlocked(clock.unix_timestamp),
        StakingError::StillLocked
    );

    // ========================================
    // STEP 3: TRANSFER NFT BACK TO OWNER
    // ========================================

    let escrow_authority_bump = ctx.bumps.escrow_authority;
    let owner_key = ctx.accounts.owner.key();
    let escrow_authority_seeds: &[&[&[u8]]] = &[&[
        b"escrow_authority",
        owner_key.as_ref(),
        &[escrow_authority_bump],
    ]];

    let transfer_ctx = CpiContext::new_with_signer(
        ctx.accounts.token_program.to_account_info(),
        Transfer {
            from: ctx.accounts.escrow_nft_account.to_account_info(),
            to: ctx.accounts.owner_nft_account.to_account_info(),
            authority: ctx.accounts.escrow_authority.to_account_info(),
        },
        escrow_authority_seeds,
    );

    token::transfer(transfer_ctx, 1)?;

    // ========================================
    // STEP 4: UPDATE STATS
    // ========================================

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

    // ========================================
    // STEP 5: EMIT EVENT
    // ========================================

    let total_staked_duration = ctx.accounts.stake_account.total_time_staked(clock.unix_timestamp);

    emit!(NftUnstaked {
        staker: ctx.accounts.stake_account.owner,
        nft_mint: ctx.accounts.stake_account.nft_mint,
        unstaked_at: clock.unix_timestamp,
        total_staked_duration,
        rewards_earned: ctx.accounts.stake_account.rewards_claimed,
    });

    msg!("✅ NFT unstaked successfully!");
    msg!("   NFT Mint: {}", ctx.accounts.stake_account.nft_mint);
    msg!("   Total staked: {} days", total_staked_duration / 86400);
    msg!("   Rewards earned: {}", ctx.accounts.stake_account.rewards_claimed);

    // Stake account automatically closed (rent refunded to owner)
    Ok(())
}
