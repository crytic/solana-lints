use anchor_lang::prelude::*;
use borsh::{BorshDeserialize, BorshSerialize};

declare_id!("Fg6PaFpoGXkYsidMpWTK6W2BeZ7FEfcYkg476zPFsLnS");

#[program]
pub mod type_cosplay_recommended {
    use super::*;

    pub fn update_user(
        ctx: Context<UpdateUser>,
    ) -> anchor_lang::solana_program::entrypoint::ProgramResult {
        let account_info: &AccountInfo = ctx.accounts.user.as_ref();
        let mut data = &*account_info.data.take();
        let user = User::try_deserialize(&mut data).unwrap();

        msg!("User: {:?}", user.authority);
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

fn main() {}
