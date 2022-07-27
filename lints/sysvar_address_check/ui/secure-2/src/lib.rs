use anchor_lang::prelude::*;

declare_id!("Fg6PaFpoGXkYsidMpWTK6W2BeZ7FEfcYkg476zPFsLnS");

#[program]
pub mod insecure {
    use super::*;

    pub fn check_sysvar_address(ctx: Context<CheckSysvarAddress>) -> Result<()> {
        let rent = Rent::from_account_info(&ctx.accounts.rent).unwrap();
        msg!("Rent -> {}", rent.lamports_per_byte_year);
        Ok(())
    }
}

#[derive(Accounts)]
pub struct CheckSysvarAddress<'info> {
    rent: AccountInfo<'info>,
}

fn main() {}
