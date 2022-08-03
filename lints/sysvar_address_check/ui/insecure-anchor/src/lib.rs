use anchor_lang::prelude::*;
use solana_program::sysvar::SysvarId;
use solana_program::pubkey;
use serde::{Serialize, Deserialize};

declare_id!("Fg6PaFpoGXkYsidMpWTK6W2BeZ7FEfcYkg476zPFsLnS");

#[program]
pub mod recommended {
    use super::*;

    pub fn check_sysvar_address(ctx: Context<CheckSysvarAddress>) -> Result<()> {
        msg!("Rent -> {}", ctx.accounts.rent.lamports_per_byte_year);
        Ok(())
    }
}

#[derive(Accounts)]
pub struct CheckSysvarAddress<'info> {
    rent: Account<'info, RentCopy>,
}

#[account]
#[derive(Default, Serialize, Deserialize)]
pub struct RentCopy {
    pub lamports_per_byte_year: u64,
    pub exemption_threshold: f64,
    pub burn_percent: u8,
}

impl SolanaSysvar for RentCopy {}

impl SysvarId for RentCopy {
    fn id() -> Pubkey {
        pubkey!("SysvarRent111111111111111111111111111111111")
    }

    fn check_id(pubkey: &Pubkey) -> bool {
        id() == *pubkey
    }
}

fn main() {}
