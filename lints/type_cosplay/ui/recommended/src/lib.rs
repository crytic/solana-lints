use anchor_lang::prelude::*;
use anchor_lang::solana_program::entrypoint::ProgramResult;
use borsh::{BorshDeserialize, BorshSerialize};

declare_id!("Fg6PaFpoGXkYsidMpWTK6W2BeZ7FEfcYkg476zPFsLnS");

#[program]
pub mod type_cosplay_recommended {
    use super::*;

    pub fn update_user(ctx: Context<UpdateUser>) -> ProgramResult {
        msg!("GM {}", ctx.accounts.user.authority);
        Ok(())
    }
}

#[derive(Accounts)]
pub struct UpdateUser<'info> {
    #[account(has_one = authority)]
    user: Account<'info, User>,
    authority: Signer<'info>,
}

#[account]
pub struct User {
    authority: Pubkey,
}

#[account]
pub struct Metadata {
    account: Pubkey,
}

#[allow(dead_code)]
fn main() {}
