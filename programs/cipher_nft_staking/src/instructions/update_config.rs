use anchor_lang::prelude::*;
use crate::state::*;
use crate::events::*;
use crate::error::*;

/// Update global config
///
/// **Security:** Only authority can call this
#[derive(Accounts)]
pub struct UpdateConfig<'info> {
    /// The config authority
    pub authority: Signer<'info>,

    /// The global config
    #[account(
        mut,
        seeds = [GlobalConfig::SEED_PREFIX],
        bump = config.bump,
        constraint = config.authority == authority.key() @ StakingError::InvalidAuthority
    )]
    pub config: Account<'info, GlobalConfig>,
}

pub fn handler(
    ctx: Context<UpdateConfig>,
    new_authority: Option<Pubkey>,
    paused: Option<bool>,
    protocol_fee_bps: Option<u16>,
) -> Result<()> {
    let config = &mut ctx.accounts.config;

    // Update authority (ownership transfer)
    if let Some(new_auth) = new_authority {
        msg!("⚠️  Authority transfer: {} -> {}", config.authority, new_auth);
        config.authority = new_auth;
    }

    // Update pause state
    if let Some(pause_state) = paused {
        config.paused = pause_state;
        msg!("⚠️  Program {}", if pause_state { "PAUSED" } else { "UNPAUSED" });
    }

    // Update protocol fee
    if let Some(fee) = protocol_fee_bps {
        require!(fee <= 1000, StakingError::InvalidMultiplier); // Max 10%
        config.protocol_fee_bps = fee;
        msg!("Protocol fee updated: {} bps", fee);
    }

    emit!(ConfigUpdated {
        authority: config.authority,
        paused: config.paused,
        updated_at: Clock::get()?.unix_timestamp,
    });

    Ok(())
}
