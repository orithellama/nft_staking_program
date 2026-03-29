use anchor_lang::prelude::*;
use crate::state::*;
use crate::events::*;

/// Initialize the global config account
///
/// **Security:**
/// - Can only be called once (account init prevents re-initialization)
/// - Authority is set to balddev Squads v4 multisig
/// - Starts in unpaused state
#[derive(Accounts)]
pub struct InitializeConfig<'info> {
    /// The authority that will control the config
    #[account(mut)]
    pub authority: Signer<'info>,

    /// The global config PDA
    #[account(
        init,
        payer = authority,
        space = GlobalConfig::LEN,
        seeds = [GlobalConfig::SEED_PREFIX],
        bump
    )]
    pub config: Account<'info, GlobalConfig>,

    pub system_program: Program<'info, System>,
}

pub fn handler(ctx: Context<InitializeConfig>, protocol_fee_bps: u16) -> Result<()> {
    // Validate protocol fee (max 10% = 1000 bps)
    require!(
        protocol_fee_bps <= 1000,
        crate::error::StakingError::InvalidMultiplier
    );

    let config = &mut ctx.accounts.config;

    config.authority = ctx.accounts.authority.key();
    config.paused = false;
    config.bump = ctx.bumps.config;
    config._reserved = [0; 6];
    config.total_stakes = 0;
    config.collection_count = 0;
    config.protocol_fee_bps = protocol_fee_bps;
    config._padding = [0; 128];

    emit!(ConfigUpdated {
        authority: config.authority,
        paused: config.paused,
        updated_at: Clock::get()?.unix_timestamp,
    });

    msg!("Global config initialized by {}", config.authority);
    msg!("   Protocol fee: {} bps ({}%)", protocol_fee_bps, protocol_fee_bps as f64 / 100.0);

    Ok(())
}
