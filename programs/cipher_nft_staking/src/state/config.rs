use anchor_lang::prelude::*;

/// Global configuration for the NFT staking program
///
/// This account stores program-wide settings and is controlled by the authority.
/// Only one instance exists per program.
///
/// **Security:** Authority should be a multisig or governance program.
#[account]
#[derive(Debug)]
pub struct GlobalConfig {
    /// The authority that can update config and manage collections
    /// Should be a multisig for security
    pub authority: Pubkey, // 32

    /// Whether the program is paused (emergency stop)
    pub paused: bool, // 1

    /// Bump seed for PDA derivation
    pub bump: u8, // 1

    /// Reserved for future upgrades (alignment padding)
    pub _reserved: [u8; 6], // 6

    /// Total number of active stakes across all collections
    pub total_stakes: u64, // 8

    /// Total number of whitelisted collections
    pub collection_count: u32, // 4

    /// Protocol fee in basis points (e.g., 100 = 1%)
    /// Applied to rewards (optional, can be 0)
    pub protocol_fee_bps: u16, // 2

    /// Reserved space for future fields
    pub _padding: [u8; 128], // 128
}

impl GlobalConfig {
    /// Size calculation:
    /// 8 (discriminator) + 32 + 1 + 1 + 6 + 8 + 4 + 2 + 128 = 190 bytes
    pub const LEN: usize = 8 + 32 + 1 + 1 + 6 + 8 + 4 + 2 + 128;

    /// PDA seeds for global config
    pub const SEED_PREFIX: &'static [u8] = b"config";
}
