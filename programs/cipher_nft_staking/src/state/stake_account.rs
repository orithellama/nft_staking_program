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
/// **Orbit Finance DLMM Integration:**
/// Orbit Finance DLMM reads this account to check: is_active && !is_unlocked(now)
/// If true -> user is eligible for fee claims
#[account]
#[derive(Debug)]
pub struct StakeAccount {
    /// The owner/staker of this NFT
    pub owner: Pubkey, // 32

    /// The NFT mint address (used for PDA derivation)
    /// For compressed NFTs: stores the merkle tree address
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

    /// Optional: Associated pool address from Orbit Finance DLMM
    /// If set, this stake provides benefits for that specific pool
    pub associated_pool: Pubkey, // 32

    /// NFT type: 0 = Traditional, 1 = Compressed
    pub nft_type: u8, // 1

    /// For compressed NFTs: the leaf index in the merkle tree
    /// For traditional NFTs: unused (0)
    pub leaf_index: u64, // 8

    /// Reserved space for future fields
    pub _padding: [u8; 135], // 135 (119 + 16 from removed fields)
}

impl StakeAccount {
    /// Size calculation:
    /// 8 (discriminator) + 32 + 32 + 32 + 8 + 8 + 8 + 1 + 1 + 6 + 32 + 1 + 8 + 135 = 312 bytes
    pub const LEN: usize = 8 + 32 + 32 + 32 + 8 + 8 + 8 + 1 + 1 + 6 + 32 + 1 + 8 + 135;

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
    ///
    /// **Safety:** Uses saturating_sub to handle edge cases gracefully
    pub fn total_time_staked(&self, current_timestamp: i64) -> i64 {
        current_timestamp.saturating_sub(self.staked_at)
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
