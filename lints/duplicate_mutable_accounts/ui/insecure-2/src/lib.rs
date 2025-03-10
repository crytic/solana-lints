use anchor_lang::prelude::*;

declare_id!("Fg6PaFpoGXkYsidMpWTK6W2BeZ7FEfcYkg476zPFsLnS");

#[program]
pub mod duplicate_mutable_accounts_insecure {
    use super::*;

    pub fn update(
        ctx: Context<Update>,
        a: u64,
        b: u64,
        c: u64,
    ) -> anchor_lang::solana_program::entrypoint::ProgramResult {
        let user_a = &mut ctx.accounts.user_a;
        let user_b = &mut ctx.accounts.user_b;
        let user_c = &mut ctx.accounts.user_c;

        user_a.data = a;
        user_b.data = b;
        user_c.data = c;
        Ok(())
    }
}

#[derive(Accounts)]
pub struct Update<'info> {
    user_a: Account<'info, User>,
    user_b: Account<'info, User>,
    user_c: Account<'info, User>,
}

#[account]
pub struct User {
    data: u64,
}

fn main() {}
