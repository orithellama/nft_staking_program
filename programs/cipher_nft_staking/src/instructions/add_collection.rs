use anchor_lang::prelude::*;
use crate::state::*;
use crate::events::*;
use crate::error::*;

/// Add or update a collection's staking configuration
///
/// **Security:**
/// - Only config authority can call this
/// - Validates lock duration ranges
#[derive(Accounts)]
#[instruction(collection: Pubkey)]
pub struct AddCollection<'info> {
    /// The config authority
    #[account(mut)]
    pub authority: Signer<'info>,

    /// The global config
    #[account(
        mut,
        seeds = [GlobalConfig::SEED_PREFIX],
        bump = config.bump,
        constraint = config.authority == authority.key() @ StakingError::InvalidAuthority
    )]
    pub config: Account<'info, GlobalConfig>,

    /// The collection config PDA
    #[account(
        init_if_needed,
        payer = authority,
        space = CollectionConfig::LEN,
        seeds = [CollectionConfig::SEED_PREFIX, collection.as_ref()],
        bump
    )]
    pub collection_config: Account<'info, CollectionConfig>,

    pub system_program: Program<'info, System>,
}

pub fn handler(
    ctx: Context<AddCollection>,
    collection: Pubkey,
    min_lock_duration: i64,
    max_lock_duration: i64,
    enabled: bool,
) -> Result<()> {
    // VALIDATION CHECKS
    require!(
        min_lock_duration > 0 && min_lock_duration <= max_lock_duration,
        StakingError::InvalidLockDuration
    );

    // UPDATE COLLECTION CONFIG
    let is_new = ctx.accounts.collection_config.collection == Pubkey::default();

    let collection_config = &mut ctx.accounts.collection_config;

    collection_config.collection = collection;
    collection_config.enabled = enabled;
    collection_config.bump = ctx.bumps.collection_config;
    collection_config.min_lock_duration = min_lock_duration;
    collection_config.max_lock_duration = max_lock_duration;
    collection_config._reserved = [0; 8];

    if is_new {
        collection_config.total_staked = 0;
        collection_config.lifetime_stakes = 0;
        collection_config._padding = [0; 128];

        // Increment collection count
        let config = &mut ctx.accounts.config;
        config.collection_count = config
            .collection_count
            .checked_add(1)
            .ok_or(StakingError::ArithmeticOverflow)?;
    }

    // EMIT EVENT
    emit!(CollectionWhitelisted {
        collection,
        min_lock_duration,
        max_lock_duration,
        enabled,
    });

    msg!("Collection {} {}", collection, if is_new { "added" } else { "updated" });
    msg!("   Enabled: {}", enabled);
    msg!("   Lock range: {} - {} seconds", min_lock_duration, max_lock_duration);

    Ok(())
}
