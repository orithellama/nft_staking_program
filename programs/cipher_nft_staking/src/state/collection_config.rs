use anchor_lang::prelude::*;

/// Configuration for a specific NFT collection
///
/// Each whitelisted collection has its own config that defines:
/// - Allowed lock durations
/// - Whether staking is enabled
///
/// **Integration with Orbit Finance DLMM:** Orbit Finance DLMM checks if a user has an active
/// stake by reading the StakeAccount. If staked -> eligible for fee claims.
#[account]
#[derive(Debug)]
pub struct CollectionConfig {
    /// The NFT collection's verified collection address
    pub collection: Pubkey, // 32

    /// Whether staking is enabled for this collection
    pub enabled: bool, // 1

    /// Bump seed for PDA derivation
    pub bump: u8, // 1

    /// Minimum lock duration in seconds (e.g., 7 days = 604800)
    pub min_lock_duration: i64, // 8

    /// Maximum lock duration in seconds (e.g., 365 days = 31536000)
    pub max_lock_duration: i64, // 8

    /// Reserved space for alignment
    pub _reserved: [u8; 8], // 8

    /// Total number of NFTs currently staked from this collection
    pub total_staked: u64, // 8

    /// Total all-time stakes from this collection
    pub lifetime_stakes: u64, // 8

    /// Reserved space for future fields
    pub _padding: [u8; 128], // 128
}

impl CollectionConfig {
    /// Size calculation:
    /// 8 (discriminator) + 32 + 1 + 1 + 8 + 8 + 8 + 8 + 8 + 128 = 210 bytes
    pub const LEN: usize = 8 + 32 + 1 + 1 + 8 + 8 + 8 + 8 + 8 + 128;

    /// PDA seeds for collection config
    pub const SEED_PREFIX: &'static [u8] = b"collection";
}
