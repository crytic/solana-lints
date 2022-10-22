use anchor_lang::prelude::*;
use anchor_lang::solana_program::entrypoint::ProgramResult;

declare_id!("Fg6PaFpoGXkYsidMpWTK6W2BeZ7FEfcYkg476zPFsLnS");

#[program]
pub mod closing_accounts_recommended {
    use super::*;

    pub fn close(ctx: Context<Close>) -> ProgramResult {
        Ok(())
    }
}

#[derive(Accounts)]
pub struct Close<'info> {
    #[account(mut, close = destination)]
    account: Account<'info, Data>,
    destination: AccountInfo<'info>,
}

#[account]
pub struct Data {
    data: u64,
}

#[allow(dead_code)]
fn main() {}
