use anchor_lang::prelude::*;

pub mod error;
pub mod events;
pub mod instructions;
pub mod state;

use instructions::*;

declare_id!("2qUYFufdaRp6KgGNJg8iqK8ArYscD3LhjDoPwnf5Vq8f");

/// # Cipher NFT Staking Program
///
/// Simple, secure NFT staking program for the Cipher/Orbit Finance ecosystem.
///
/// ## Features
/// - NFT escrow via PDA (cannot be moved during lock)
/// - Configurable lock periods per collection
/// - Binary lock/unlock system (no complex multipliers)
/// - Emergency pause functionality
/// - No admin backdoors (authority cannot steal NFTs)
/// - Full event emission for off-chain indexing
///
/// ## Integration with Orbit Finance DLMM
///
/// This program tracks NFT staking positions which Orbit Finance DLMM
/// uses to determine fee claim eligibility:
///
/// 1. User stakes NFT here -> NFT locked in escrow
/// 2. User trades on Orbit Finance DLMM pools -> earns trading fees
/// 3. Orbit Finance DLMM reads stake_account: is_active && !is_unlocked()
/// 4. If true -> user is eligible to claim fees
///
/// ## Security Model
///
/// - **NFTs locked in escrow PDA** - physically cannot move until unlock_at
/// - **No admin withdrawal** - authority cannot access user NFTs
/// - **Checked math everywhere** - prevents overflows
/// - **Collection whitelist** - only approved collections stakeable
/// - **Metadata verification** - validates NFT authenticity
///
/// ## Program Flow
///
/// ```text
/// 1. Admin: initialize_config()
/// 2. Admin: add_collection() for each allowed NFT collection
/// 3. User:  stake_nft() -> NFT locked, stake_account created
/// 4. User:  [waits for lock period]
/// 5. User:  claim_rewards() -> optional, tracks rewards claimed
/// 6. User:  unstake_nft() -> NFT returned, stake_account closed
/// ```
#[program]
pub mod cipher_nft_staking {
    use super::*;

    /// Initialize the global config (call once)
    ///
    /// # Arguments
    /// * `protocol_fee_bps` - Optional fee in basis points (max 1000 = 10%)
    pub fn initialize_config(
        ctx: Context<InitializeConfig>,
        protocol_fee_bps: u16,
    ) -> Result<()> {
        instructions::initialize_config::handler(ctx, protocol_fee_bps)
    }

    /// Update global configuration
    ///
    /// # Arguments
    /// * `new_authority` - Transfer authority (optional)
    /// * `paused` - Pause/unpause staking (optional)
    /// * `protocol_fee_bps` - Update protocol fee (optional)
    ///
    /// # Security
    /// Only current authority can call this
    pub fn update_config(
        ctx: Context<UpdateConfig>,
        new_authority: Option<Pubkey>,
        paused: Option<bool>,
        protocol_fee_bps: Option<u16>,
    ) -> Result<()> {
        instructions::update_config::handler(ctx, new_authority, paused, protocol_fee_bps)
    }

    /// Add or update a collection's staking configuration
    ///
    /// # Arguments
    /// * `collection` - The NFT collection mint address
    /// * `min_lock_duration` - Minimum lock time in seconds
    /// * `max_lock_duration` - Maximum lock time in seconds
    /// * `enabled` - Whether staking is enabled for this collection
    ///
    /// # Security
    /// Only authority can call this
    pub fn add_collection(
        ctx: Context<AddCollection>,
        collection: Pubkey,
        min_lock_duration: i64,
        max_lock_duration: i64,
        enabled: bool,
    ) -> Result<()> {
        instructions::add_collection::handler(
            ctx,
            collection,
            min_lock_duration,
            max_lock_duration,
            enabled,
        )
    }

    /// Stake an NFT
    ///
    /// # Arguments
    /// * `lock_duration` - How long to lock in seconds
    /// * `associated_pool` - Optional DLMM pool address for targeted benefits
    ///
    /// # Security
    /// - Verifies NFT ownership
    /// - Validates collection is whitelisted
    /// - Validates lock duration within bounds
    /// - Transfers NFT to escrow PDA (user cannot move it)
    pub fn stake_nft(
        ctx: Context<StakeNft>,
        lock_duration: i64,
        associated_pool: Option<Pubkey>,
    ) -> Result<()> {
        instructions::stake_nft::handler(ctx, lock_duration, associated_pool)
    }

    /// Unstake an NFT after lock period expires
    ///
    /// # Security
    /// - Verifies lock period has passed
    /// - Verifies owner matches
    /// - Returns NFT to owner
    /// - Closes stake account (rent refund)
    pub fn unstake_nft(ctx: Context<UnstakeNft>) -> Result<()> {
        instructions::unstake_nft::handler(ctx)
    }

    /// Claim accumulated rewards
    ///
    /// # Arguments
    /// * `amount` - Amount of rewards to mark as claimed
    ///
    /// # Note
    /// This is a tracking function. Actual token distribution
    /// should be handled by your adapter after reading the event.
    pub fn claim_rewards(ctx: Context<ClaimRewards>, amount: u64) -> Result<()> {
        instructions::claim_rewards::handler(ctx, amount)
    }
}
