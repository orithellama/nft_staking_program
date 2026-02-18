# Cipher NFT Staking Program

A secure, simple NFT staking program built on Solana for the Cipher/Orbit Finance ecosystem.

## Overview

This program enables users to stake their NFTs by locking them in escrow PDAs for a specified duration. During the lock period, the NFT cannot be moved, providing a verifiable on-chain proof that the user has committed to the ecosystem. This staking status can be read by other programs (like DLMM) to determine eligibility for fee claims or other benefits.

**Key Design Principle**: Simple binary lock/unlock system, no complex multipliers or reward calculations. The program tracks whether an NFT is locked and when it unlocks, nothing more.

## Program ID

```
CiPherNFTStake11111111111111111111111111111
```

## Features

- **NFT Escrow via PDA**: NFTs are held in per-user escrow PDAs and cannot be moved until the lock period expires
- **Collection Whitelisting**: Only admin-approved NFT collections can be staked
- **Configurable Lock Durations**: Each collection can have min/max lock period requirements
- **Emergency Pause**: Admin can pause staking in case of emergencies (does not affect existing stakes)
- **No Admin Backdoors**: Authority cannot access or withdraw user NFTs
- **Per-User Escrow Authority**: Each user has their own escrow authority PDA, eliminating any collision risks
- **Full Event Emission**: All actions emit events for easy off-chain indexing

## Program Instructions

### Admin Instructions

#### 1. `initialize_config`
Initialize the global configuration. Must be called once before any other instruction.

**Parameters:**
- `protocol_fee_bps` (u16) - Optional protocol fee in basis points (max 1000 = 10%)

**Access:** Anyone (first call initializes, subsequent calls fail)

#### 2. `update_config`
Update global configuration settings.

**Parameters:**
- `new_authority` (Option<Pubkey>) - Transfer authority to new address
- `paused` (Option<bool>) - Pause/unpause staking operations
- `protocol_fee_bps` (Option<u16>) - Update protocol fee

**Access:** Current authority only

#### 3. `add_collection`
Add or update a collection's staking configuration.

**Parameters:**
- `collection` (Pubkey) - The NFT collection's verified collection address
- `min_lock_duration` (i64) - Minimum lock time in seconds
- `max_lock_duration` (i64) - Maximum lock time in seconds
- `enabled` (bool) - Whether staking is enabled for this collection

**Access:** Authority only

**Example:**
```rust
// Enable staking for a collection with 7-30 day lock periods
add_collection(
    collection: collection_mint,
    min_lock_duration: 604800,  // 7 days
    max_lock_duration: 2592000, // 30 days
    enabled: true
)
```

### User Instructions

#### 4. `stake_nft`
Stake an NFT by locking it in escrow.

**Parameters:**
- `lock_duration` (i64) - How long to lock the NFT (seconds)
- `associated_pool` (Option<Pubkey>) - Optional pool address for targeted benefits

**Requirements:**
- User must own the NFT
- NFT must be from a whitelisted and enabled collection
- Lock duration must be within collection's min/max range
- NFT must have verified collection metadata

**What Happens:**
1. Verifies NFT ownership and collection
2. Transfers NFT to escrow PDA (user loses custody)
3. Creates stake account with lock details
4. Emits `NftStaked` event

#### 5. `unstake_nft`
Unstake an NFT after the lock period expires.

**Requirements:**
- User must own the stake account
- Current time must be >= unlock_at timestamp

**What Happens:**
1. Verifies lock period has expired
2. Transfers NFT back to user
3. Closes stake account (refunds rent to user)
4. Emits `NftUnstaked` event

#### 6. `claim_rewards`
Track rewards claimed by a user (metadata only).

**Parameters:**
- `amount` (u64) - Amount of rewards to mark as claimed

**Note:** This is a tracking function only. Actual token distribution must be handled off-chain by your adapter after reading the event.

## Account Structures

### GlobalConfig (190 bytes)
```rust
{
    authority: Pubkey,        // Admin who can update config
    paused: bool,             // Emergency pause flag
    protocol_fee_bps: u16,    // Optional protocol fee
    total_stakes: u64,        // Total active stakes
    collection_count: u64,    // Number of whitelisted collections
    bump: u8
}
```

**PDA Seeds:** `["config"]`

### CollectionConfig (210 bytes)
```rust
{
    collection: Pubkey,       // NFT collection address
    enabled: bool,            // Whether staking is enabled
    min_lock_duration: i64,   // Min lock time in seconds
    max_lock_duration: i64,   // Max lock time in seconds
    total_staked: u64,        // Current stakes from this collection
    lifetime_stakes: u64,     // All-time stakes from this collection
    bump: u8
}
```

**PDA Seeds:** `["collection", collection_pubkey]`

### StakeAccount (312 bytes)
```rust
{
    owner: Pubkey,            // Who staked the NFT
    nft_mint: Pubkey,         // The staked NFT's mint
    collection: Pubkey,       // Verified collection
    staked_at: i64,           // Unix timestamp of stake
    unlock_at: i64,           // When NFT can be unstaked
    lock_duration: i64,       // Lock period in seconds
    is_active: bool,          // Whether stake is active
    rewards_claimed: u64,     // Total rewards claimed
    last_claim_at: i64,       // Last claim timestamp
    associated_pool: Pubkey,  // Optional pool association
    bump: u8
}
```

**PDA Seeds:** `["stake", nft_mint, owner]`

### Escrow Authority (No data)
Per-user PDA that owns the escrow token account.

**PDA Seeds:** `["escrow_authority", owner]`

## Integration with DLMM

This program is designed to integrate with the Orbit Finance DLMM (Dynamic Liquidity Market Maker) program to gate fee claims:

```rust
// In DLMM program:
let stake_account = StakeAccount::try_deserialize(&stake_account_data)?;

// Check if user has active stake
let is_staked = stake_account.is_active && !stake_account.is_unlocked(current_time);

if is_staked {
    // User is eligible for fee claims
    distribute_fees(&user);
} else {
    // User must stake NFT first
    return Err(ErrorCode::NoActiveStake);
}
```

### Integration Options

**Option 1: On-Chain Reading**
- DLMM program reads StakeAccount directly during fee claim transactions
- Pro: Always current, no sync issues
- Con: Adds compute units and account requirements

**Option 2: Database Tracking (Recommended)**
- Index NftStaked/NftUnstaked events off-chain
- Store stake status in database
- DLMM adapter checks database before distributing fees
- Pro: Lower cost, more flexible
- Con: Requires event indexer

## Security Features

### NFT Protection
- NFTs are held in Associated Token Accounts owned by per-user escrow authority PDAs
- Escrow authority has no private key - only the program can sign for it
- Each user has unique escrow authority: `["escrow_authority", user_pubkey]`
- Eliminates any theoretical collision scenarios between users

### Access Control
- All owner operations verify signer matches stake account owner
- Collection operations restricted to authority only
- Pause functionality for emergencies

### Validation
- Metadata verification ensures NFT is from claimed collection
- Token standard check prevents staking fungible tokens
- Lock duration validation enforces collection rules
- Double-stake prevention via explicit checks

### Checked Arithmetic
- All math operations use checked arithmetic to prevent overflows
- Stats updates safely handle edge cases

## Error Codes

```rust
InvalidAuthority          // Signer is not the authority
InvalidNftOwner          // Signer does not own the NFT
InvalidMetadata          // NFT metadata is invalid or missing
CollectionNotWhitelisted // Collection is not approved for staking
InvalidLockDuration      // Lock duration outside allowed range
StillLocked              // Attempted unstake before unlock time
ProgramPaused            // Staking operations are paused
InvalidTokenAccount      // Token account validation failed
ArithmeticOverflow       // Math operation would overflow
MetadataVerificationFailed // Collection not verified in metadata
StakeAlreadyExists       // Stake account already exists for this NFT
StakeAccountMismatch     // Stake account data doesn't match expected values
InvalidMultiplier        // [Unused in simplified version]
```

## Events

### NftStaked
```rust
{
    staker: Pubkey,
    nft_mint: Pubkey,
    collection: Pubkey,
    staked_at: i64,
    unlock_at: i64,
    lock_duration: i64,
    stake_account: Pubkey
}
```

### NftUnstaked
```rust
{
    staker: Pubkey,
    nft_mint: Pubkey,
    unstaked_at: i64,
    total_staked_duration: i64,
    rewards_earned: u64
}
```

### RewardsClaimed
```rust
{
    staker: Pubkey,
    nft_mint: Pubkey,
    amount: u64,
    claimed_at: i64
}
```

### CollectionWhitelisted
```rust
{
    collection: Pubkey,
    min_lock_duration: i64,
    max_lock_duration: i64,
    enabled: bool
}
```

### ConfigUpdated
```rust
{
    authority: Pubkey,
    paused: bool,
    updated_at: i64
}
```

## Building

```bash
# Install dependencies
anchor build

# Run tests
anchor test

# Deploy to devnet
anchor deploy --provider.cluster devnet

# Deploy to mainnet
anchor deploy --provider.cluster mainnet
```

## Testing

The program includes comprehensive tests covering:
- Initialization and configuration
- Collection management
- NFT staking and unstaking
- Lock period enforcement
- Access control
- Error conditions

## License

See LICENSE file for details.

## Security

See SECURITY.txt for security considerations and audit status.

## Contributing

This is a production program for the Cipher/Orbit Finance ecosystem. External contributions are not currently accepted but requests can be send to info@cipherlabsx.com.
