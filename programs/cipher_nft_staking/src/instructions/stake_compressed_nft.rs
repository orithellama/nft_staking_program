use anchor_lang::prelude::*;

use crate::state::*;
use crate::events::*;
use crate::error::*;

/// Stake a compressed NFT by setting a delegate
/// **Note:** We rely on Bubblegum's built-in validation when setting the delegate.
/// If the user doesn't own the cNFT, the delegate CPI will fail.
#[derive(Accounts)]
#[instruction(
    leaf_index: u32,
    lock_duration: i64
)]
pub struct StakeCompressedNft<'info> {
    /// The NFT owner who is staking
    #[account(mut)]
    pub owner: Signer<'info>,

    /// The merkle tree account 
    /// CHECK: Validated by Bubblegum program during delegate CPI
    pub merkle_tree: UncheckedAccount<'info>,

    /// The tree authority/config PDA
    /// CHECK: Validated by Bubblegum program
    pub tree_authority: UncheckedAccount<'info>,

    /// The leaf owner (should match owner)
    /// CHECK: Validated by Bubblegum program
    pub leaf_owner: UncheckedAccount<'info>,

    /// The leaf delegate PDA that will "lock" the cNFT
    /// CHECK: PDA derived, becomes delegate
    #[account(
        seeds = [b"cnft_delegate", owner.key().as_ref()],
        bump
    )]
    pub leaf_delegate: UncheckedAccount<'info>,

    /// The stake account PDA
    /// KEY: Uses merkle_tree address instead of nft_mint
    #[account(
        init,
        payer = owner,
        space = StakeAccount::LEN,
        seeds = [
            StakeAccount::SEED_PREFIX,
            merkle_tree.key().as_ref(),
            owner.key().as_ref()
        ],
        bump
    )]
    pub stake_account: Account<'info, StakeAccount>,

    /// The collection config (must be whitelisted)
    #[account(
        mut,
        seeds = [CollectionConfig::SEED_PREFIX, collection_config.collection.as_ref()],
        bump = collection_config.bump,
        constraint = collection_config.enabled @ StakingError::CollectionNotWhitelisted
    )]
    pub collection_config: Account<'info, CollectionConfig>,

    /// Global config
    #[account(
        mut,
        seeds = [GlobalConfig::SEED_PREFIX],
        bump = config.bump,
        constraint = !config.paused @ StakingError::ProgramPaused
    )]
    pub config: Account<'info, GlobalConfig>,

    /// Bubblegum program
    /// CHECK: Must match known Bubblegum program ID
    pub bubblegum_program: UncheckedAccount<'info>,

    /// Log wrapper for Bubblegum
    /// CHECK: Required by Bubblegum
    pub log_wrapper: UncheckedAccount<'info>,

    /// Compression program
    /// CHECK: Required by Bubblegum
    pub compression_program: UncheckedAccount<'info>,

    pub system_program: Program<'info, System>,
}

pub fn handler(
    ctx: Context<StakeCompressedNft>,
    root: [u8; 32],
    data_hash: [u8; 32],
    creator_hash: [u8; 32],
    nonce: u64,
    index: u32,
    lock_duration: i64,
    associated_pool: Option<Pubkey>,
) -> Result<()> {
    
    // VALIDATE LOCK DURATION
    let collection_config = &ctx.accounts.collection_config;

    require!(
        lock_duration >= collection_config.min_lock_duration
            && lock_duration <= collection_config.max_lock_duration,
        StakingError::InvalidLockDuration
    );

    
    // SET BUBBLEGUM DELEGATE
    // This "locks" the cNFT by preventing transfers
    // If this CPI fails, it means the user doesn't own the cNFT
    set_bubblegum_delegate(
        &ctx.accounts.bubblegum_program,
        &ctx.accounts.tree_authority,
        &ctx.accounts.leaf_owner,
        &ctx.accounts.leaf_delegate,
        &ctx.accounts.merkle_tree,
        &ctx.accounts.log_wrapper,
        &ctx.accounts.compression_program,
        &ctx.accounts.owner,
        root,
        data_hash,
        creator_hash,
        nonce,
        index,
    )?;

    
    // INITIALIZE STAKE ACCOUNT
    let clock = Clock::get()?;
    let stake_account_key = ctx.accounts.stake_account.key();
    let stake_account = &mut ctx.accounts.stake_account;

    stake_account.owner = ctx.accounts.owner.key();
    stake_account.nft_mint = ctx.accounts.merkle_tree.key(); // Store merkle_tree address
    stake_account.collection = collection_config.collection; // Use collection from config
    stake_account.staked_at = clock.unix_timestamp;
    stake_account.unlock_at = clock
        .unix_timestamp
        .checked_add(lock_duration)
        .ok_or(StakingError::ArithmeticOverflow)?;
    stake_account.lock_duration = lock_duration;
    stake_account.bump = ctx.bumps.stake_account;
    stake_account.is_active = true;
    stake_account._reserved = [0; 6];
    stake_account.associated_pool = associated_pool.unwrap_or(Pubkey::default());
    stake_account.nft_type = 1; // Compressed NFT
    stake_account.leaf_index = index as u64;
    stake_account._padding = [0; 135];

    
    // UPDATE STATS
    let collection_config = &mut ctx.accounts.collection_config;
    collection_config.total_staked = collection_config
        .total_staked
        .checked_add(1)
        .ok_or(StakingError::ArithmeticOverflow)?;
    collection_config.lifetime_stakes = collection_config
        .lifetime_stakes
        .checked_add(1)
        .ok_or(StakingError::ArithmeticOverflow)?;

    let config = &mut ctx.accounts.config;
    config.total_stakes = config
        .total_stakes
        .checked_add(1)
        .ok_or(StakingError::ArithmeticOverflow)?;

    
    // EMIT EVENT
    emit!(NftStaked {
        staker: stake_account.owner,
        nft_mint: stake_account.nft_mint, // merkle_tree address
        collection: stake_account.collection,
        staked_at: stake_account.staked_at,
        unlock_at: stake_account.unlock_at,
        lock_duration: stake_account.lock_duration,
        stake_account: stake_account_key,
    });

    msg!("Compressed NFT staked successfully");
    msg!("   Merkle Tree: {}", stake_account.nft_mint);
    msg!("   Leaf Index: {}", index);
    msg!("   Owner: {}", stake_account.owner);
    msg!("   Lock Duration: {} days", lock_duration / 86400);
    msg!("   Unlock At: {}", stake_account.unlock_at);

    Ok(())
}

/// Helper: Set Bubblegum delegate via CPI to LOCK the cNFT on-chain
///
/// This implements Bubblegum's `delegate` instruction which locks the cNFT by
/// setting our program PDA as the delegate. While delegated, the owner cannot
/// transfer, burn, or modify the NFT - equivalent to escrow locking.
///
/// Bubblegum Program ID: BGUMAp9Gvew5e5YBEuRAGHsivpB3ZwickQYqasdBZH8Ka
/// Delegate instruction discriminator: [90, 147, 75, 178, 85, 88, 4, 137]
fn set_bubblegum_delegate<'info>(
    bubblegum_program: &UncheckedAccount<'info>,
    tree_authority: &UncheckedAccount<'info>,
    leaf_owner: &UncheckedAccount<'info>,
    leaf_delegate: &UncheckedAccount<'info>,
    merkle_tree: &UncheckedAccount<'info>,
    log_wrapper: &UncheckedAccount<'info>,
    compression_program: &UncheckedAccount<'info>,
    owner_signer: &Signer<'info>,
    root: [u8; 32],
    data_hash: [u8; 32],
    creator_hash: [u8; 32],
    nonce: u64,
    index: u32,
) -> Result<()> {
    use anchor_lang::solana_program::{instruction::Instruction, program::invoke};

    // Verify Bubblegum program ID
    const BUBBLEGUM_PROGRAM_ID: &str = "BGUMAp9Gvew5e5YBEuRAGHsivpB3ZwickQYqasdBZH8Ka";
    let expected_program_id = BUBBLEGUM_PROGRAM_ID.parse::<Pubkey>().unwrap();
    require_keys_eq!(
        *bubblegum_program.key,
        expected_program_id,
        StakingError::InvalidDelegate
    );

    // Build instruction data with merkle proof parameters
    // Format: [discriminator: 8][root: 32][data_hash: 32][creator_hash: 32][nonce: 8][index: 4]
    let discriminator: [u8; 8] = [90, 147, 75, 178, 85, 88, 4, 137]; // delegate instruction
    let mut instruction_data = Vec::with_capacity(116);
    instruction_data.extend_from_slice(&discriminator);
    instruction_data.extend_from_slice(&root);
    instruction_data.extend_from_slice(&data_hash);
    instruction_data.extend_from_slice(&creator_hash);
    instruction_data.extend_from_slice(&nonce.to_le_bytes());
    instruction_data.extend_from_slice(&index.to_le_bytes());

    // Build account metas in the order Bubblegum expects:
    // 0. tree_authority (tree config PDA)
    // 1. leaf_owner (current owner)
    // 2. previous_leaf_delegate (none, same as leaf_owner)
    // 3. new_leaf_delegate (our program PDA)
    // 4. merkle_tree
    // 5. log_wrapper (SPL Noop)
    // 6. compression_program (SPL Account Compression)
    // 7. system_program
    use anchor_lang::solana_program::instruction::AccountMeta;
    let account_metas = vec![
        AccountMeta::new_readonly(*tree_authority.key, false),
        AccountMeta::new_readonly(*leaf_owner.key, true), // signer
        AccountMeta::new_readonly(*leaf_owner.key, false), // previous delegate = owner
        AccountMeta::new_readonly(*leaf_delegate.key, false), // new delegate
        AccountMeta::new(*merkle_tree.key, false),
        AccountMeta::new_readonly(*log_wrapper.key, false),
        AccountMeta::new_readonly(*compression_program.key, false),
        AccountMeta::new_readonly(anchor_lang::solana_program::system_program::ID, false),
    ];

    // Build instruction
    let ix = Instruction {
        program_id: *bubblegum_program.key,
        accounts: account_metas,
        data: instruction_data,
    };

    // Prepare account infos for CPI
    let account_infos = vec![
        tree_authority.to_account_info(),
        leaf_owner.to_account_info(),
        leaf_delegate.to_account_info(),
        merkle_tree.to_account_info(),
        log_wrapper.to_account_info(),
        compression_program.to_account_info(),
        owner_signer.to_account_info(),
    ];

    // Execute CPI (owner signs, no PDA signing needed for setting delegate)
    invoke(&ix, &account_infos)?;

    msg!("Bubblegum delegate set - cNFT is LOCKED ON-CHAIN");
    msg!("   Delegate: {}", leaf_delegate.key());
    msg!("   Merkle Tree: {}", merkle_tree.key());
    msg!("   Leaf Index: {}", index);

    Ok(())
}
