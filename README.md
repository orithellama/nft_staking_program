# NFT Staking Program

A secure NFT staking program on Solana (Anchor) that locks NFTs in escrow PDAs for a fixed period. Other on-chain or off-chain systems can read stake state to gate access to benefits such as fee claims.

This program can be integrated with Balddev DEX.

## Program ID

```text
7dMir6E96FwiYQQ9mdsL6AKUmgzzrERwqj7mkhthxQgV
```

## Overview

The program uses a binary lock model:
- NFT is either actively staked (locked) or unstaked (unlocked)
- No reward multipliers or complex reward math inside this program
- External systems decide how stake status affects eligibility or incentives

## Features

- NFT escrow via per-user PDA authority
- Collection-level allowlisting and enable/disable controls
- Configurable min/max lock durations per collection
- Emergency pause for staking operations
- No admin withdrawal path for user NFTs
- Event emission for indexing and analytics
- Support for traditional and compressed NFT tracking fields

## Program Instructions

### 1. `initialize_config`
Initializes global config. Callable once.

Parameters:
- `protocol_fee_bps` (`u16`) - protocol fee in bps (max 1000 = 10%)

Access:
- First successful caller initializes config authority

### 2. `update_config`
Updates global config.

Parameters:
- `new_authority` (`Option<Pubkey>`) - transfer authority
- `paused` (`Option<bool>`) - pause/unpause staking ops
- `protocol_fee_bps` (`Option<u16>`) - update fee bps

Access:
- Current authority only

### 3. `add_collection`
Adds or updates config for a verified NFT collection.

Parameters:
- `collection` (`Pubkey`) - verified collection address
- `min_lock_duration` (`i64`) - minimum lock (seconds)
- `max_lock_duration` (`i64`) - maximum lock (seconds)
- `enabled` (`bool`) - staking enabled for this collection

Access:
- Authority only

### 4. `stake_nft`
Locks a user NFT in escrow and creates stake state.

Parameters:
- `lock_duration` (`i64`) - lock period in seconds
- `associated_pool` (`Option<Pubkey>`) - optional pool/market reference for downstream integrations

Requirements:
- Signer owns the NFT
- Collection is allowlisted and enabled
- Lock duration is within collection bounds
- NFT metadata verification passes

### 5. `unstake_nft`
Unlocks and returns NFT after lock expiry.

Requirements:
- Signer matches stake owner
- Current timestamp is at or after `unlock_at`

## Account Model

### `GlobalConfig` (190 bytes)

```rust
{
    authority: Pubkey,
    paused: bool,
    bump: u8,
    _reserved: [u8; 6],
    total_stakes: u64,
    collection_count: u32,
    protocol_fee_bps: u16,
    _padding: [u8; 128]
}
```

PDA seeds:
- `["config"]`

### `CollectionConfig` (210 bytes)

```rust
{
    collection: Pubkey,
    enabled: bool,
    bump: u8,
    min_lock_duration: i64,
    max_lock_duration: i64,
    _reserved: [u8; 8],
    total_staked: u64,
    lifetime_stakes: u64,
    _padding: [u8; 128]
}
```

PDA seeds:
- `["collection", collection_pubkey]`

### `StakeAccount` (312 bytes)

```rust
{
    owner: Pubkey,
    nft_mint: Pubkey,
    collection: Pubkey,
    staked_at: i64,
    unlock_at: i64,
    lock_duration: i64,
    bump: u8,
    is_active: bool,
    _reserved: [u8; 6],
    associated_pool: Pubkey,
    nft_type: u8,
    leaf_index: u64,
    _padding: [u8; 135]
}
```

PDA seeds:
- `["stake", nft_mint, owner]`

### Escrow Authority PDA

Per-user authority PDA that owns the escrow token account.

PDA seeds:
- `["escrow_authority", owner]`

## Balddev DEX Integration

This staking program can be integrated into Balddev DEX by checking whether a user has an active, still-locked stake.

Example check logic:

```rust
let stake_account = StakeAccount::try_deserialize(&stake_account_data)?;
let is_staked = stake_account.is_active && !stake_account.is_unlocked(current_time);

if is_staked {
    // eligible in Balddev DEX
} else {
    // staking requirement not met
}
```

Integration patterns:

1. On-chain check
- Balddev DEX instruction reads `StakeAccount` directly
- Highest consistency, higher compute/account overhead

2. Off-chain indexer check
- Index `NftStaked` and `NftUnstaked` events
- Maintain stake status in a database/cache
- Lower on-chain cost, requires reliable indexer infra

## Events

### `NftStaked`
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

### `NftUnstaked`
```rust
{
    staker: Pubkey,
    nft_mint: Pubkey,
    unstaked_at: i64,
    total_staked_duration: i64
}
```

### `CollectionWhitelisted`
```rust
{
    collection: Pubkey,
    min_lock_duration: i64,
    max_lock_duration: i64,
    enabled: bool
}
```

### `ConfigUpdated`
```rust
{
    authority: Pubkey,
    paused: bool,
    updated_at: i64
}
```

## Error Codes

```rust
CollectionNotWhitelisted
StillLocked
InvalidLockDuration
InvalidNftOwner
InvalidMetadata
StakeAccountMismatch
InvalidAuthority
CollectionConfigNotFound
ArithmeticOverflow
InvalidMultiplier
StakeAlreadyExists
ProgramPaused
InvalidTokenAccount
MetadataVerificationFailed
InvalidNftType
InvalidMerkleProof
CompressedNftVerificationFailed
InvalidDelegate
InvalidMerkleTree
```

## Build and Test

```bash
anchor build
anchor test
```

Deploy:

```bash
# devnet
anchor deploy --provider.cluster devnet

# mainnet
anchor deploy --provider.cluster mainnet
```

## Security

See `SECURITY.txt` for threat model, assumptions, and operational guidance.

## License

See `LICENSE`.
