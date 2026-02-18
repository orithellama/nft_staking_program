use anchor_lang::prelude::*;
use crate::state::*;
use crate::events::*;
use crate::error::*;

/// Claim accumulated staking rewards
///
/// **Note:** This is a placeholder for reward claiming logic.
/// In practice, rewards would be distributed from a treasury/vault
/// or calculated based on DLMM trading fees.
///
/// **Integration with DLMM:**
/// Your adapter can call this to mark rewards as claimed,
/// then distribute tokens from the DLMM reward pools.
#[derive(Accounts)]
pub struct ClaimRewards<'info> {
    /// The NFT owner claiming rewards
    #[account(mut)]
    pub owner: Signer<'info>,

    /// The stake account
    #[account(
        mut,
        seeds = [StakeAccount::SEED_PREFIX, stake_account.nft_mint.as_ref(), owner.key().as_ref()],
        bump = stake_account.bump,
        constraint = stake_account.is_active @ StakingError::StakeAccountMismatch
    )]
    pub stake_account: Account<'info, StakeAccount>,

    /// Global config
    #[account(
        seeds = [GlobalConfig::SEED_PREFIX],
        bump = config.bump
    )]
    pub config: Account<'info, GlobalConfig>,
}

pub fn handler(ctx: Context<ClaimRewards>, amount: u64) -> Result<()> {
    
    // VERIFY OWNER
    ctx.accounts.stake_account.verify_owner(&ctx.accounts.owner.key())?;

    // VALIDATE CLAIM
    require!(amount > 0, StakingError::NoRewards);

    // UPDATE STAKE ACCOUNT
    let clock = Clock::get()?;
    let stake_account = &mut ctx.accounts.stake_account;

    stake_account.rewards_claimed = stake_account
        .rewards_claimed
        .checked_add(amount)
        .ok_or(StakingError::ArithmeticOverflow)?;

    stake_account.last_claim_at = clock.unix_timestamp;

    
    // EMIT EVENT
    emit!(RewardsClaimed {
        staker: stake_account.owner,
        nft_mint: stake_account.nft_mint,
        amount,
        claimed_at: clock.unix_timestamp,
    });

    msg!("Rewards claimed!");
    msg!("   Amount: {}", amount);
    msg!("   Total claimed: {}", stake_account.rewards_claimed);

    // after this instruction succeeds, reading the event.

    Ok(())
}
