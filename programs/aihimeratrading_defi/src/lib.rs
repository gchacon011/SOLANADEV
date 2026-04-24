use anchor_lang::prelude::*;
use anchor_spl::token::{self, Mint, Token, TokenAccount, Transfer};

declare_id!("AihmDf1111111111111111111111111111111111111");

const BASIS_POINTS: u64 = 10_000;
const MAX_SYMBOL_LEN: usize = 16;
const MAX_STRATEGY_URI_LEN: usize = 96;
const MAX_RATIONALE_HASH_LEN: usize = 64;

#[program]
pub mod aihimeratrading_defi {
    use super::*;

    pub fn initialize_protocol(
        ctx: Context<InitializeProtocol>,
        reward_rate_bps: u16,
        platform_fee_bps: u16,
    ) -> Result<()> {
        require!(reward_rate_bps <= 5_000, DefiError::InvalidRewardRate);
        require!(platform_fee_bps <= 1_000, DefiError::InvalidFeeRate);

        let protocol = &mut ctx.accounts.protocol;
        protocol.authority = ctx.accounts.authority.key();
        protocol.mint = ctx.accounts.mint.key();
        protocol.treasury = ctx.accounts.treasury.key();
        protocol.reward_rate_bps = reward_rate_bps;
        protocol.platform_fee_bps = platform_fee_bps;
        protocol.signal_count = 0;
        protocol.bump = ctx.bumps.protocol;
        Ok(())
    }

    pub fn create_signal(
        ctx: Context<CreateSignal>,
        symbol: String,
        strategy_uri: String,
        rationale_hash: String,
        direction: TradeDirection,
        confidence_bps: u16,
        initial_stake: u64,
    ) -> Result<()> {
        require!(symbol.len() <= MAX_SYMBOL_LEN, DefiError::SymbolTooLong);
        require!(strategy_uri.len() <= MAX_STRATEGY_URI_LEN, DefiError::StrategyUriTooLong);
        require!(rationale_hash.len() <= MAX_RATIONALE_HASH_LEN, DefiError::RationaleHashTooLong);
        require!(confidence_bps <= BASIS_POINTS as u16, DefiError::InvalidConfidence);
        require!(initial_stake > 0, DefiError::StakeMustBePositive);

        let protocol = &mut ctx.accounts.protocol;
        let signal = &mut ctx.accounts.signal;
        let clock = Clock::get()?;
        let signal_id = protocol.signal_count;

        signal.protocol = protocol.key();
        signal.creator = ctx.accounts.creator.key();
        signal.mint = ctx.accounts.mint.key();
        signal.vault = ctx.accounts.signal_vault.key();
        signal.id = signal_id;
        signal.symbol = symbol;
        signal.strategy_uri = strategy_uri;
        signal.rationale_hash = rationale_hash;
        signal.direction = direction;
        signal.confidence_bps = confidence_bps;
        signal.total_staked = 0;
        signal.performance_bps = 0;
        signal.is_active = true;
        signal.created_at = clock.unix_timestamp;
        signal.updated_at = clock.unix_timestamp;
        signal.bump = ctx.bumps.signal;
        signal.vault_bump = ctx.bumps.signal_vault;

        protocol.signal_count = protocol
            .signal_count
            .checked_add(1)
            .ok_or(DefiError::MathOverflow)?;

        transfer_tokens(
            ctx.accounts.creator_token_account.to_account_info(),
            ctx.accounts.signal_vault.to_account_info(),
            ctx.accounts.creator.to_account_info(),
            ctx.accounts.token_program.to_account_info(),
            initial_stake,
        )?;

        signal.total_staked = initial_stake;
        emit!(SignalCreated {
            signal: signal.key(),
            creator: signal.creator,
            id: signal_id,
            symbol: signal.symbol.clone(),
            initial_stake,
        });
        Ok(())
    }

    pub fn update_signal(
        ctx: Context<UpdateSignal>,
        strategy_uri: String,
        rationale_hash: String,
        direction: TradeDirection,
        confidence_bps: u16,
    ) -> Result<()> {
        require!(strategy_uri.len() <= MAX_STRATEGY_URI_LEN, DefiError::StrategyUriTooLong);
        require!(rationale_hash.len() <= MAX_RATIONALE_HASH_LEN, DefiError::RationaleHashTooLong);
        require!(confidence_bps <= BASIS_POINTS as u16, DefiError::InvalidConfidence);
        require!(ctx.accounts.signal.is_active, DefiError::SignalInactive);

        let signal = &mut ctx.accounts.signal;
        signal.strategy_uri = strategy_uri;
        signal.rationale_hash = rationale_hash;
        signal.direction = direction;
        signal.confidence_bps = confidence_bps;
        signal.updated_at = Clock::get()?.unix_timestamp;

        emit!(SignalUpdated {
            signal: signal.key(),
            creator: signal.creator,
            confidence_bps,
        });
        Ok(())
    }

    pub fn stake_signal(ctx: Context<StakeSignal>, amount: u64) -> Result<()> {
        require!(amount > 0, DefiError::StakeMustBePositive);
        require!(ctx.accounts.signal.is_active, DefiError::SignalInactive);

        transfer_tokens(
            ctx.accounts.staker_token_account.to_account_info(),
            ctx.accounts.signal_vault.to_account_info(),
            ctx.accounts.staker.to_account_info(),
            ctx.accounts.token_program.to_account_info(),
            amount,
        )?;

        let position = &mut ctx.accounts.position;
        if position.owner == Pubkey::default() {
            position.owner = ctx.accounts.staker.key();
            position.signal = ctx.accounts.signal.key();
            position.staked_amount = 0;
            position.rewards_claimed = 0;
            position.bump = ctx.bumps.position;
        }

        position.staked_amount = position
            .staked_amount
            .checked_add(amount)
            .ok_or(DefiError::MathOverflow)?;
        position.last_action_at = Clock::get()?.unix_timestamp;

        let signal = &mut ctx.accounts.signal;
        signal.total_staked = signal
            .total_staked
            .checked_add(amount)
            .ok_or(DefiError::MathOverflow)?;
        signal.updated_at = Clock::get()?.unix_timestamp;

        emit!(SignalStaked {
            signal: signal.key(),
            staker: ctx.accounts.staker.key(),
            amount,
        });
        Ok(())
    }

    pub fn score_signal(ctx: Context<ScoreSignal>, performance_bps: i16) -> Result<()> {
        require!(performance_bps >= -10_000 && performance_bps <= 10_000, DefiError::InvalidPerformance);
        let signal = &mut ctx.accounts.signal;
        signal.performance_bps = performance_bps;
        signal.updated_at = Clock::get()?.unix_timestamp;
        emit!(SignalScored {
            signal: signal.key(),
            performance_bps,
        });
        Ok(())
    }

    pub fn claim_rewards(ctx: Context<ClaimRewards>) -> Result<()> {
        require!(ctx.accounts.signal.performance_bps > 0, DefiError::NoPositivePerformance);
        require!(ctx.accounts.position.staked_amount > 0, DefiError::NothingStaked);

        let protocol = &ctx.accounts.protocol;
        let signal = &ctx.accounts.signal;
        let position = &mut ctx.accounts.position;

        let gross_reward = position
            .staked_amount
            .checked_mul(signal.performance_bps as u64)
            .ok_or(DefiError::MathOverflow)?
            .checked_mul(protocol.reward_rate_bps as u64)
            .ok_or(DefiError::MathOverflow)?
            .checked_div(BASIS_POINTS)
            .ok_or(DefiError::MathOverflow)?
            .checked_div(BASIS_POINTS)
            .ok_or(DefiError::MathOverflow)?;

        let fee = gross_reward
            .checked_mul(protocol.platform_fee_bps as u64)
            .ok_or(DefiError::MathOverflow)?
            .checked_div(BASIS_POINTS)
            .ok_or(DefiError::MathOverflow)?;
        let net_reward = gross_reward.checked_sub(fee).ok_or(DefiError::MathOverflow)?;
        require!(net_reward > 0, DefiError::RewardTooSmall);

        let seeds: &[&[u8]] = &[b"protocol", protocol.authority.as_ref(), protocol.mint.as_ref(), &[protocol.bump]];
        let signer = &[seeds];

        token::transfer(
            CpiContext::new_with_signer(
                ctx.accounts.token_program.to_account_info(),
                Transfer {
                    from: ctx.accounts.treasury.to_account_info(),
                    to: ctx.accounts.staker_token_account.to_account_info(),
                    authority: ctx.accounts.protocol.to_account_info(),
                },
                signer,
            ),
            net_reward,
        )?;

        position.rewards_claimed = position
            .rewards_claimed
            .checked_add(net_reward)
            .ok_or(DefiError::MathOverflow)?;
        position.last_action_at = Clock::get()?.unix_timestamp;

        emit!(RewardsClaimed {
            signal: signal.key(),
            staker: ctx.accounts.staker.key(),
            amount: net_reward,
        });
        Ok(())
    }

    pub fn deactivate_signal(ctx: Context<DeactivateSignal>) -> Result<()> {
        let signal = &mut ctx.accounts.signal;
        signal.is_active = false;
        signal.updated_at = Clock::get()?.unix_timestamp;
        emit!(SignalDeactivated { signal: signal.key() });
        Ok(())
    }

    pub fn withdraw_stake(ctx: Context<WithdrawStake>, amount: u64) -> Result<()> {
        require!(amount > 0, DefiError::StakeMustBePositive);
        require!(ctx.accounts.position.staked_amount >= amount, DefiError::InsufficientStake);

        let signal = &mut ctx.accounts.signal;
        let position = &mut ctx.accounts.position;
        position.staked_amount = position.staked_amount.checked_sub(amount).ok_or(DefiError::MathOverflow)?;
        position.last_action_at = Clock::get()?.unix_timestamp;
        signal.total_staked = signal.total_staked.checked_sub(amount).ok_or(DefiError::MathOverflow)?;
        signal.updated_at = Clock::get()?.unix_timestamp;

        let signal_key = signal.key();
        let seeds: &[&[u8]] = &[b"vault", signal_key.as_ref(), &[signal.vault_bump]];
        let signer = &[seeds];

        token::transfer(
            CpiContext::new_with_signer(
                ctx.accounts.token_program.to_account_info(),
                Transfer {
                    from: ctx.accounts.signal_vault.to_account_info(),
                    to: ctx.accounts.staker_token_account.to_account_info(),
                    authority: ctx.accounts.signal_vault.to_account_info(),
                },
                signer,
            ),
            amount,
        )?;

        emit!(StakeWithdrawn {
            signal: signal.key(),
            staker: ctx.accounts.staker.key(),
            amount,
        });
        Ok(())
    }
}

fn transfer_tokens<'info>(
    from: AccountInfo<'info>,
    to: AccountInfo<'info>,
    authority: AccountInfo<'info>,
    token_program: AccountInfo<'info>,
    amount: u64,
) -> Result<()> {
    token::transfer(
        CpiContext::new(token_program, Transfer { from, to, authority }),
        amount,
    )
}

#[derive(Accounts)]
pub struct InitializeProtocol<'info> {
    #[account(mut)]
    pub authority: Signer<'info>,
    pub mint: Account<'info, Mint>,
    #[account(
        init,
        payer = authority,
        space = 8 + Protocol::INIT_SPACE,
        seeds = [b"protocol", authority.key().as_ref(), mint.key().as_ref()],
        bump
    )]
    pub protocol: Account<'info, Protocol>,
    #[account(
        init,
        payer = authority,
        token::mint = mint,
        token::authority = protocol,
        seeds = [b"treasury", protocol.key().as_ref()],
        bump
    )]
    pub treasury: Account<'info, TokenAccount>,
    pub token_program: Program<'info, Token>,
    pub system_program: Program<'info, System>,
    pub rent: Sysvar<'info, Rent>,
}

#[derive(Accounts)]
pub struct CreateSignal<'info> {
    #[account(mut)]
    pub creator: Signer<'info>,
    pub mint: Account<'info, Mint>,
    #[account(mut, has_one = mint)]
    pub protocol: Account<'info, Protocol>,
    #[account(
        init,
        payer = creator,
        space = 8 + Signal::INIT_SPACE,
        seeds = [b"signal", protocol.key().as_ref(), creator.key().as_ref(), &protocol.signal_count.to_le_bytes()],
        bump
    )]
    pub signal: Account<'info, Signal>,
    #[account(
        init,
        payer = creator,
        token::mint = mint,
        token::authority = signal_vault,
        seeds = [b"vault", signal.key().as_ref()],
        bump
    )]
    pub signal_vault: Account<'info, TokenAccount>,
    #[account(mut, constraint = creator_token_account.owner == creator.key(), constraint = creator_token_account.mint == mint.key())]
    pub creator_token_account: Account<'info, TokenAccount>,
    pub token_program: Program<'info, Token>,
    pub system_program: Program<'info, System>,
    pub rent: Sysvar<'info, Rent>,
}

#[derive(Accounts)]
pub struct UpdateSignal<'info> {
    pub creator: Signer<'info>,
    #[account(mut, has_one = creator)]
    pub signal: Account<'info, Signal>,
}

#[derive(Accounts)]
pub struct StakeSignal<'info> {
    #[account(mut)]
    pub staker: Signer<'info>,
    pub mint: Account<'info, Mint>,
    #[account(mut, has_one = mint)]
    pub signal: Account<'info, Signal>,
    #[account(mut, address = signal.vault)]
    pub signal_vault: Account<'info, TokenAccount>,
    #[account(
        init_if_needed,
        payer = staker,
        space = 8 + UserPosition::INIT_SPACE,
        seeds = [b"position", signal.key().as_ref(), staker.key().as_ref()],
        bump
    )]
    pub position: Account<'info, UserPosition>,
    #[account(mut, constraint = staker_token_account.owner == staker.key(), constraint = staker_token_account.mint == mint.key())]
    pub staker_token_account: Account<'info, TokenAccount>,
    pub token_program: Program<'info, Token>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct ScoreSignal<'info> {
    pub authority: Signer<'info>,
    #[account(has_one = authority)]
    pub protocol: Account<'info, Protocol>,
    #[account(mut, has_one = protocol)]
    pub signal: Account<'info, Signal>,
}

#[derive(Accounts)]
pub struct ClaimRewards<'info> {
    #[account(mut)]
    pub staker: Signer<'info>,
    pub mint: Account<'info, Mint>,
    #[account(has_one = mint, has_one = treasury)]
    pub protocol: Account<'info, Protocol>,
    #[account(has_one = protocol)]
    pub signal: Account<'info, Signal>,
    #[account(mut, has_one = signal, has_one = owner @ DefiError::Unauthorized)]
    pub position: Account<'info, UserPosition>,
    /// CHECK: checked by UserPosition has_one = owner and signer equality below
    #[account(address = staker.key())]
    pub owner: UncheckedAccount<'info>,
    #[account(mut, address = protocol.treasury)]
    pub treasury: Account<'info, TokenAccount>,
    #[account(mut, constraint = staker_token_account.owner == staker.key(), constraint = staker_token_account.mint == mint.key())]
    pub staker_token_account: Account<'info, TokenAccount>,
    pub token_program: Program<'info, Token>,
}

#[derive(Accounts)]
pub struct DeactivateSignal<'info> {
    pub creator: Signer<'info>,
    #[account(mut, has_one = creator)]
    pub signal: Account<'info, Signal>,
}

#[derive(Accounts)]
pub struct WithdrawStake<'info> {
    #[account(mut)]
    pub staker: Signer<'info>,
    pub mint: Account<'info, Mint>,
    #[account(mut, has_one = mint)]
    pub signal: Account<'info, Signal>,
    #[account(mut, address = signal.vault)]
    pub signal_vault: Account<'info, TokenAccount>,
    #[account(mut, has_one = signal, has_one = owner @ DefiError::Unauthorized)]
    pub position: Account<'info, UserPosition>,
    /// CHECK: checked by UserPosition has_one = owner and signer equality below
    #[account(address = staker.key())]
    pub owner: UncheckedAccount<'info>,
    #[account(mut, constraint = staker_token_account.owner == staker.key(), constraint = staker_token_account.mint == mint.key())]
    pub staker_token_account: Account<'info, TokenAccount>,
    pub token_program: Program<'info, Token>,
}

#[account]
#[derive(InitSpace)]
pub struct Protocol {
    pub authority: Pubkey,
    pub mint: Pubkey,
    pub treasury: Pubkey,
    pub reward_rate_bps: u16,
    pub platform_fee_bps: u16,
    pub signal_count: u64,
    pub bump: u8,
}

#[account]
#[derive(InitSpace)]
pub struct Signal {
    pub protocol: Pubkey,
    pub creator: Pubkey,
    pub mint: Pubkey,
    pub vault: Pubkey,
    pub id: u64,
    #[max_len(MAX_SYMBOL_LEN)]
    pub symbol: String,
    #[max_len(MAX_STRATEGY_URI_LEN)]
    pub strategy_uri: String,
    #[max_len(MAX_RATIONALE_HASH_LEN)]
    pub rationale_hash: String,
    pub direction: TradeDirection,
    pub confidence_bps: u16,
    pub total_staked: u64,
    pub performance_bps: i16,
    pub is_active: bool,
    pub created_at: i64,
    pub updated_at: i64,
    pub bump: u8,
    pub vault_bump: u8,
}

#[account]
#[derive(InitSpace)]
pub struct UserPosition {
    pub owner: Pubkey,
    pub signal: Pubkey,
    pub staked_amount: u64,
    pub rewards_claimed: u64,
    pub last_action_at: i64,
    pub bump: u8,
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone, Copy, PartialEq, Eq, InitSpace)]
pub enum TradeDirection {
    Long,
    Short,
    Neutral,
}

#[event]
pub struct SignalCreated {
    pub signal: Pubkey,
    pub creator: Pubkey,
    pub id: u64,
    pub symbol: String,
    pub initial_stake: u64,
}

#[event]
pub struct SignalUpdated {
    pub signal: Pubkey,
    pub creator: Pubkey,
    pub confidence_bps: u16,
}

#[event]
pub struct SignalStaked {
    pub signal: Pubkey,
    pub staker: Pubkey,
    pub amount: u64,
}

#[event]
pub struct SignalScored {
    pub signal: Pubkey,
    pub performance_bps: i16,
}

#[event]
pub struct RewardsClaimed {
    pub signal: Pubkey,
    pub staker: Pubkey,
    pub amount: u64,
}

#[event]
pub struct SignalDeactivated {
    pub signal: Pubkey,
}

#[event]
pub struct StakeWithdrawn {
    pub signal: Pubkey,
    pub staker: Pubkey,
    pub amount: u64,
}

#[error_code]
pub enum DefiError {
    #[msg("Symbol is too long")]
    SymbolTooLong,
    #[msg("Strategy URI is too long")]
    StrategyUriTooLong,
    #[msg("Rationale hash is too long")]
    RationaleHashTooLong,
    #[msg("Confidence must be between 0 and 10000 basis points")]
    InvalidConfidence,
    #[msg("Stake amount must be positive")]
    StakeMustBePositive,
    #[msg("Math overflow")]
    MathOverflow,
    #[msg("Signal is inactive")]
    SignalInactive,
    #[msg("Only the authorized account can perform this action")]
    Unauthorized,
    #[msg("Invalid reward rate")]
    InvalidRewardRate,
    #[msg("Invalid platform fee rate")]
    InvalidFeeRate,
    #[msg("Performance must be between -10000 and 10000 basis points")]
    InvalidPerformance,
    #[msg("Signal has no positive performance")]
    NoPositivePerformance,
    #[msg("Nothing staked")]
    NothingStaked,
    #[msg("Reward is too small")]
    RewardTooSmall,
    #[msg("Insufficient staked amount")]
    InsufficientStake,
}
