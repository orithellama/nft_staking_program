use anchor_lang::prelude::*;

/// Individual NFT stake account
///
/// One account per staked NFT. Stores all information about the stake:
/// - Ownership
/// - Lock period
/// - Lock status
/// - Metadata reference
///
/// **Security:**
/// - NFT is held in escrow PDA (different from this account)
/// - Cannot be moved until unlock_at timestamp passes
/// - Owner verification required for all operations
///
/// **DLMM Integration:**
/// Your DLMM program reads this account to check: is_active && !is_unlocked(now)
/// If true → user is eligible for fee claims
#[account]
#[derive(Debug)]
pub struct StakeAccount {
    /// The owner/staker of this NFT
    pub owner: Pubkey, // 32

    /// The NFT mint address (used for PDA derivation)
    pub nft_mint: Pubkey, // 32

    /// The verified collection this NFT belongs to
    pub collection: Pubkey, // 32

    /// Unix timestamp when the stake was created
    pub staked_at: i64, // 8

    /// Unix timestamp when the NFT can be unstaked
    pub unlock_at: i64, // 8

    /// Lock duration in seconds
    pub lock_duration: i64, // 8

    /// Bump seed for this PDA
    pub bump: u8, // 1

    /// Whether this stake is currently active
    pub is_active: bool, // 1

    /// Reserved for alignment
    pub _reserved: [u8; 6], // 6

    /// Total rewards claimed so far
    pub rewards_claimed: u64, // 8

    /// Last time rewards were claimed
    pub last_claim_at: i64, // 8

    /// Optional: Associated pool address from DLMM
    /// If set, this stake provides benefits for that specific pool
    pub associated_pool: Pubkey, // 32

    /// Reserved space for future fields
    pub _padding: [u8; 128], // 128
}

impl StakeAccount {
    /// Size calculation:
    /// 8 (discriminator) + 32 + 32 + 32 + 8 + 8 + 8 + 1 + 1 + 6 + 8 + 8 + 32 + 128 = 312 bytes
    pub const LEN: usize = 8 + 32 + 32 + 32 + 8 + 8 + 8 + 1 + 1 + 6 + 8 + 8 + 32 + 128;

    /// PDA seeds for stake account
    pub const SEED_PREFIX: &'static [u8] = b"stake";

    /// Check if the NFT can be unstaked
    ///
    /// **Security:** Always verifies current timestamp against unlock_at
    pub fn is_unlocked(&self, current_timestamp: i64) -> bool {
        current_timestamp >= self.unlock_at
    }

    /// Calculate time remaining until unlock (in seconds)
    pub fn time_until_unlock(&self, current_timestamp: i64) -> i64 {
        if self.is_unlocked(current_timestamp) {
            0
        } else {
            self.unlock_at - current_timestamp
        }
    }

    /// Calculate total time staked (in seconds)
    pub fn total_time_staked(&self, current_timestamp: i64) -> i64 {
        current_timestamp - self.staked_at
    }

    /// Verify that the signer owns this stake
    ///
    /// **Security Critical:** Must be called before any owner-privileged operations
    pub fn verify_owner(&self, signer: &Pubkey) -> Result<()> {
        require_keys_eq!(
            self.owner,
            *signer,
            crate::error::StakingError::InvalidNftOwner
        );
        Ok(())
    }
}
