//! san_delegation: scoped, capped, expiring execution delegations for solana-agent (devnet).
//! Original code (not derived from mpl-agent source). Fixes vs mpl-agent-tools ExecutionDelegateRecordV1:
//! program allowlist, lamport spend cap, expiry, auto-void on Core asset transfer, and one
//! owner-wide revocation nonce that voids every delegation (and anything derived from it).
use anchor_lang::prelude::*;

declare_id!("EeZBH7PggMHuQoYjK9Lz6bsM7GvbmKk9pBn9dDxV57Tm");

pub const MPL_CORE: Pubkey = pubkey!("CoREENxT6tW1HoK8ypY1SxRMZTcVPm7R94rH4PZNhX7d");
pub const MAX_PROGRAMS: usize = 4;

/// Core AssetV1 layout: [0]=key (1 = AssetV1), [1..33]=owner.
fn core_owner(asset: &AccountInfo) -> Result<Pubkey> {
    require_keys_eq!(*asset.owner, MPL_CORE, SanError::NotCoreAsset);
    let d = asset.try_borrow_data()?;
    require!(d.len() >= 33 && d[0] == 1, SanError::NotCoreAsset);
    Ok(Pubkey::try_from(&d[1..33]).unwrap())
}

#[program]
pub mod san_delegation {
    use super::*;

    pub fn grant(ctx: Context<Grant>, programs: Vec<Pubkey>, spend_cap_lamports: u64, expires_at: i64) -> Result<()> {
        require!(!programs.is_empty() && programs.len() <= MAX_PROGRAMS, SanError::BadAllowlist);
        require!(expires_at > Clock::get()?.unix_timestamp, SanError::Expired);
        let owner = core_owner(&ctx.accounts.asset)?;
        require_keys_eq!(owner, ctx.accounts.owner.key(), SanError::NotOwner);
        let s = &mut ctx.accounts.owner_state;
        if s.asset == Pubkey::default() { s.asset = ctx.accounts.asset.key(); s.bump = ctx.bumps.owner_state; }
        let d = &mut ctx.accounts.delegation;
        let mut list = [Pubkey::default(); MAX_PROGRAMS];
        list[..programs.len()].copy_from_slice(&programs);
        d.set_inner(Delegation { asset: s.asset, delegate: ctx.accounts.delegate.key(), owner_at_grant: owner,
            programs: list, n_programs: programs.len() as u8, spend_cap_lamports, spent_lamports: 0,
            expires_at, nonce: s.revoke_nonce, bump: ctx.bumps.delegation });
        Ok(())
    }

    /// Delegate records intended use; fails unless every scope check passes.
    pub fn consume(ctx: Context<Consume>, program: Pubkey, lamports: u64) -> Result<()> {
        let d = &mut ctx.accounts.delegation;
        require_keys_eq!(core_owner(&ctx.accounts.asset)?, d.owner_at_grant, SanError::Transferred);
        require!(d.nonce >= ctx.accounts.owner_state.revoke_nonce, SanError::Revoked);
        require!(Clock::get()?.unix_timestamp < d.expires_at, SanError::Expired);
        require!(d.programs[..d.n_programs as usize].contains(&program), SanError::NotAllowlisted);
        d.spent_lamports = d.spent_lamports.checked_add(lamports).ok_or(SanError::CapExceeded)?;
        require!(d.spent_lamports <= d.spend_cap_lamports, SanError::CapExceeded);
        Ok(())
    }

    /// Owner bumps the nonce: every existing delegation for this asset is void at once.
    pub fn revoke_all(ctx: Context<RevokeAll>) -> Result<()> {
        require_keys_eq!(core_owner(&ctx.accounts.asset)?, ctx.accounts.owner.key(), SanError::NotOwner);
        ctx.accounts.owner_state.revoke_nonce += 1;
        Ok(())
    }

    /// Current owner (or the delegate) closes a delegation and reclaims rent to the caller.
    pub fn close(ctx: Context<Close>) -> Result<()> {
        let k = ctx.accounts.closer.key();
        let ok = k == ctx.accounts.delegation.delegate || k == core_owner(&ctx.accounts.asset)?;
        require!(ok, SanError::NotOwner);
        Ok(())
    }
}

#[account]
#[derive(InitSpace)]
pub struct OwnerState { pub asset: Pubkey, pub revoke_nonce: u64, pub bump: u8 }

#[account]
#[derive(InitSpace)]
pub struct Delegation {
    pub asset: Pubkey, pub delegate: Pubkey, pub owner_at_grant: Pubkey,
    pub programs: [Pubkey; MAX_PROGRAMS], pub n_programs: u8,
    pub spend_cap_lamports: u64, pub spent_lamports: u64, pub expires_at: i64, pub nonce: u64, pub bump: u8,
}

#[derive(Accounts)]
pub struct Grant<'info> {
    #[account(mut)] pub owner: Signer<'info>,
    /// CHECK: validated as an MPL Core asset in core_owner().
    pub asset: UncheckedAccount<'info>,
    /// CHECK: any pubkey may be a delegate.
    pub delegate: UncheckedAccount<'info>,
    #[account(init_if_needed, payer = owner, space = 8 + OwnerState::INIT_SPACE, seeds = [b"owner_state", asset.key().as_ref()], bump)]
    pub owner_state: Account<'info, OwnerState>,
    #[account(init, payer = owner, space = 8 + Delegation::INIT_SPACE, seeds = [b"delegation", asset.key().as_ref(), delegate.key().as_ref()], bump)]
    pub delegation: Account<'info, Delegation>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct Consume<'info> {
    pub delegate: Signer<'info>,
    /// CHECK: validated in core_owner().
    pub asset: UncheckedAccount<'info>,
    #[account(seeds = [b"owner_state", asset.key().as_ref()], bump = owner_state.bump)]
    pub owner_state: Account<'info, OwnerState>,
    #[account(mut, has_one = delegate, has_one = asset, seeds = [b"delegation", asset.key().as_ref(), delegate.key().as_ref()], bump = delegation.bump)]
    pub delegation: Account<'info, Delegation>,
}

#[derive(Accounts)]
pub struct RevokeAll<'info> {
    pub owner: Signer<'info>,
    /// CHECK: validated in core_owner().
    pub asset: UncheckedAccount<'info>,
    #[account(mut, seeds = [b"owner_state", asset.key().as_ref()], bump = owner_state.bump)]
    pub owner_state: Account<'info, OwnerState>,
}

#[derive(Accounts)]
pub struct Close<'info> {
    #[account(mut)] pub closer: Signer<'info>,
    /// CHECK: validated in core_owner().
    pub asset: UncheckedAccount<'info>,
    #[account(mut, close = closer, has_one = asset)]
    pub delegation: Account<'info, Delegation>,
}

#[error_code]
pub enum SanError {
    #[msg("not an MPL Core asset")] NotCoreAsset,
    #[msg("signer is not the asset owner")] NotOwner,
    #[msg("allowlist must have 1-4 programs")] BadAllowlist,
    #[msg("delegation expired")] Expired,
    #[msg("asset transferred: delegation void")] Transferred,
    #[msg("delegation revoked")] Revoked,
    #[msg("program not allowlisted")] NotAllowlisted,
    #[msg("spend cap exceeded")] CapExceeded,
}
