use anchor_lang::prelude::*;

/// Emitted when a new NFT is staked
#[event]
pub struct NftStaked {
    /// The public key of the staker
    pub staker: Pubkey,
    /// The mint address of the staked NFT
    pub nft_mint: Pubkey,
    /// The collection this NFT belongs to
    pub collection: Pubkey,
    /// Unix timestamp when the stake was created
    pub staked_at: i64,
    /// Unix timestamp when the NFT can be unstaked
    pub unlock_at: i64,
    /// Lock duration in seconds
    pub lock_duration: i64,
    /// The PDA address of the stake account
    pub stake_account: Pubkey,
}

/// Emitted when an NFT is unstaked
#[event]
pub struct NftUnstaked {
    /// The public key of the staker
    pub staker: Pubkey,
    /// The mint address of the unstaked NFT
    pub nft_mint: Pubkey,
    /// Unix timestamp when unstaked
    pub unstaked_at: i64,
    /// Total time staked in seconds
    pub total_staked_duration: i64,
}

/// Emitted when a collection is whitelisted
#[event]
pub struct CollectionWhitelisted {
    /// The collection mint address
    pub collection: Pubkey,
    /// Minimum lock duration in seconds
    pub min_lock_duration: i64,
    /// Maximum lock duration in seconds
    pub max_lock_duration: i64,
    /// Whether this collection is enabled
    pub enabled: bool,
}

/// Emitted when global config is updated
#[event]
pub struct ConfigUpdated {
    /// The authority that made the update
    pub authority: Pubkey,
    /// Whether the program is paused
    pub paused: bool,
    /// Unix timestamp of update
    pub updated_at: i64,
}
