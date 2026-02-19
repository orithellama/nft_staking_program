use anchor_lang::prelude::*;

use crate::state::*;
use crate::events::*;
use crate::error::*;

/// Unstake a compressed NFT after lock period expires
///
/// **Security:**
/// - Verifies lock period has expired
/// - Verifies owner matches
/// - Removes Bubblegum delegate (unlocks the cNFT)
/// - Closes stake account (rent refund)
#[derive(Accounts)]
pub struct UnstakeCompressedNft<'info> {
    /// The NFT owner who is unstaking
    #[account(mut)]
    pub owner: Signer<'info>,

    /// The merkle tree account
    /// CHECK: Validated via stake_account
    pub merkle_tree: UncheckedAccount<'info>,

    /// The tree authority/config PDA
    /// CHECK: Validated by Bubblegum program
    pub tree_authority: UncheckedAccount<'info>,

    /// The leaf delegate PDA that currently locks the cNFT
    /// CHECK: PDA derived, will be removed as delegate
    #[account(
        seeds = [b"cnft_delegate", owner.key().as_ref()],
        bump
    )]
    pub leaf_delegate: UncheckedAccount<'info>,

    /// The stake account
    #[account(
        mut,
        close = owner,
        seeds = [
            StakeAccount::SEED_PREFIX,
            merkle_tree.key().as_ref(),
            owner.key().as_ref()
        ],
        bump = stake_account.bump,
        constraint = stake_account.nft_mint == merkle_tree.key() @ StakingError::StakeAccountMismatch,
        constraint = stake_account.is_active @ StakingError::StakeAccountMismatch,
        constraint = stake_account.nft_type == 1 @ StakingError::InvalidNftType
    )]
    pub stake_account: Account<'info, StakeAccount>,

    /// The collection config
    #[account(
        mut,
        seeds = [CollectionConfig::SEED_PREFIX, stake_account.collection.as_ref()],
        bump = collection_config.bump
    )]
    pub collection_config: Account<'info, CollectionConfig>,

    /// Global config
    #[account(
        mut,
        seeds = [GlobalConfig::SEED_PREFIX],
        bump = config.bump
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
    ctx: Context<UnstakeCompressedNft>,
    root: [u8; 32],
    data_hash: [u8; 32],
    creator_hash: [u8; 32],
    nonce: u64,
) -> Result<()> {
    // ========================================
    // STEP 1: VERIFY OWNER
    // ========================================

    ctx.accounts.stake_account.verify_owner(&ctx.accounts.owner.key())?;

    // ========================================
    // STEP 2: CHECK LOCK PERIOD
    // ========================================

    let clock = Clock::get()?;
    require!(
        ctx.accounts.stake_account.is_unlocked(clock.unix_timestamp),
        StakingError::StillLocked
    );

    // ========================================
    // STEP 3: REMOVE BUBBLEGUM DELEGATE
    // ========================================
    // This "unlocks" the cNFT

    let leaf_index = ctx.accounts.stake_account.leaf_index as u32;

    remove_bubblegum_delegate(
        &ctx.accounts.bubblegum_program,
        &ctx.accounts.tree_authority,
        &ctx.accounts.leaf_delegate,
        &ctx.accounts.merkle_tree,
        &ctx.accounts.log_wrapper,
        &ctx.accounts.compression_program,
        root,
        data_hash,
        creator_hash,
        nonce,
        leaf_index,
        &ctx.accounts.owner.key(),
        ctx.bumps.leaf_delegate,
    )?;

    // ========================================
    // STEP 4: UPDATE STATS
    // ========================================

    let collection_config = &mut ctx.accounts.collection_config;
    collection_config.total_staked = collection_config
        .total_staked
        .checked_sub(1)
        .ok_or(StakingError::ArithmeticOverflow)?;

    let config = &mut ctx.accounts.config;
    config.total_stakes = config
        .total_stakes
        .checked_sub(1)
        .ok_or(StakingError::ArithmeticOverflow)?;

    // ========================================
    // STEP 5: EMIT EVENT
    // ========================================

    let total_staked_duration = ctx.accounts.stake_account.total_time_staked(clock.unix_timestamp);

    emit!(NftUnstaked {
        staker: ctx.accounts.stake_account.owner,
        nft_mint: ctx.accounts.stake_account.nft_mint, // merkle_tree address
        unstaked_at: clock.unix_timestamp,
        total_staked_duration,
    });

    msg!("Compressed NFT unstaked successfully");
    msg!("   Merkle Tree: {}", ctx.accounts.stake_account.nft_mint);
    msg!("   Leaf Index: {}", ctx.accounts.stake_account.leaf_index);
    msg!("   Total staked: {} days", total_staked_duration / 86400);

    // Stake account automatically closed (rent refunded to owner)
    Ok(())
}

/// Helper: Remove Bubblegum delegate via CPI to UNLOCK the cNFT
///
/// **PRODUCTION IMPLEMENTATION using Raw CPI with PDA Signing**
///
/// This removes the delegation by transferring the cNFT back to its owner
/// while signed as the delegate. This unlocks the cNFT, allowing the owner
/// to transfer, burn, or modify it again.
///
/// Bubblegum Program ID: BGUMAp9Gvew5e5YBEuRAGHsivpB3ZwickQYqasdBZH8Ka
/// Transfer instruction discriminator: [163, 52, 200, 231, 140, 3, 69, 186]
fn remove_bubblegum_delegate<'info>(
    bubblegum_program: &UncheckedAccount<'info>,
    tree_authority: &UncheckedAccount<'info>,
    leaf_delegate: &UncheckedAccount<'info>,
    merkle_tree: &UncheckedAccount<'info>,
    log_wrapper: &UncheckedAccount<'info>,
    compression_program: &UncheckedAccount<'info>,
    root: [u8; 32],
    data_hash: [u8; 32],
    creator_hash: [u8; 32],
    nonce: u64,
    leaf_index: u32,
    owner: &Pubkey,
    delegate_bump: u8,
) -> Result<()> {
    use anchor_lang::solana_program::{instruction::Instruction, program::invoke_signed};

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
    let discriminator: [u8; 8] = [163, 52, 200, 231, 140, 3, 69, 186]; // transfer instruction
    let mut instruction_data = Vec::with_capacity(116);
    instruction_data.extend_from_slice(&discriminator);
    instruction_data.extend_from_slice(&root);
    instruction_data.extend_from_slice(&data_hash);
    instruction_data.extend_from_slice(&creator_hash);
    instruction_data.extend_from_slice(&nonce.to_le_bytes());
    instruction_data.extend_from_slice(&leaf_index.to_le_bytes());

    // Build account metas in the order Bubblegum expects for transfer:
    // 0. tree_authority (tree config PDA)
    // 1. leaf_owner (current owner)
    // 2. leaf_delegate (our program PDA - must sign)
    // 3. new_leaf_owner (same as current owner, transfers to self removes delegation)
    // 4. merkle_tree
    // 5. log_wrapper (SPL Noop)
    // 6. compression_program (SPL Account Compression)
    // 7. system_program
    use anchor_lang::solana_program::instruction::AccountMeta;
    let account_metas = vec![
        AccountMeta::new_readonly(*tree_authority.key, false),
        AccountMeta::new_readonly(*owner, false), // current owner
        AccountMeta::new_readonly(*leaf_delegate.key, true), // delegate MUST sign
        AccountMeta::new_readonly(*owner, false), // new owner (same = removes delegation)
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

    // Prepare PDA signer seeds
    let seeds = &[
        b"cnft_delegate".as_ref(),
        owner.as_ref(),
        &[delegate_bump],
    ];
    let signer_seeds = &[&seeds[..]];

    // Prepare account infos for CPI
    let account_infos = vec![
        tree_authority.to_account_info(),
        leaf_delegate.to_account_info(), // Our PDA must be included
        merkle_tree.to_account_info(),
        log_wrapper.to_account_info(),
        compression_program.to_account_info(),
    ];

    // Execute CPI WITH PDA SIGNING (delegate signs to authorize transfer)
    invoke_signed(&ix, &account_infos, signer_seeds)?;

    msg!("Bubblegum delegate removed - cNFT is UNLOCKED");
    msg!("   Former Delegate: {}", leaf_delegate.key());
    msg!("   Merkle Tree: {}", merkle_tree.key());
    msg!("   Leaf Index: {}", leaf_index);

    Ok(())
}
