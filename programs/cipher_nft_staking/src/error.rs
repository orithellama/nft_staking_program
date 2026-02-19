use anchor_lang::prelude::*;

/// Custom error codes for NFT staking program
#[error_code]
pub enum StakingError {
    #[msg("The NFT collection is not whitelisted for staking")]
    CollectionNotWhitelisted,

    #[msg("The lock period has not expired yet")]
    StillLocked,

    #[msg("Invalid lock duration - must be between min and max")]
    InvalidLockDuration,

    #[msg("NFT is not owned by the signer")]
    InvalidNftOwner,

    #[msg("Invalid NFT metadata")]
    InvalidMetadata,

    #[msg("Stake account does not match NFT")]
    StakeAccountMismatch,

    #[msg("Config authority mismatch")]
    InvalidAuthority,

    #[msg("Collection config not found")]
    CollectionConfigNotFound,

    #[msg("Arithmetic overflow")]
    ArithmeticOverflow,

    #[msg("Invalid reward multiplier")]
    InvalidMultiplier,

    #[msg("Stake account already exists")]
    StakeAlreadyExists,

    #[msg("Program is paused")]
    ProgramPaused,

    #[msg("Invalid token account")]
    InvalidTokenAccount,

    #[msg("NFT metadata verification failed")]
    MetadataVerificationFailed,

    // Compressed NFT errors
    #[msg("Invalid NFT type (must be 0=Traditional or 1=Compressed)")]
    InvalidNftType,

    #[msg("Invalid merkle proof - compressed NFT ownership verification failed")]
    InvalidMerkleProof,

    #[msg("Compressed NFT verification failed")]
    CompressedNftVerificationFailed,

    #[msg("Delegate authority mismatch or delegation failed")]
    InvalidDelegate,

    #[msg("Merkle tree account is invalid or inaccessible")]
    InvalidMerkleTree,
}
