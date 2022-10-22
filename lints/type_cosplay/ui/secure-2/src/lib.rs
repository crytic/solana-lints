use anchor_lang::prelude::*;
use anchor_lang::solana_program::entrypoint::ProgramResult;
use borsh::{BorshDeserialize, BorshSerialize};

declare_id!("Fg6PaFpoGXkYsidMpWTK6W2BeZ7FEfcYkg476zPFsLnS");

#[program]
pub mod type_cosplay_secure {
    use super::*;

    pub fn update_user(ctx: Context<UpdateUser>) -> ProgramResult {
        match AccountDiscriminant::try_from_slice(&ctx.accounts.user.data.borrow()).unwrap() {
            AccountDiscriminant::User(user) => {
                if ctx.accounts.user.owner != ctx.program_id {
                    return Err(ProgramError::IllegalOwner);
                }
                if user.authority != ctx.accounts.authority.key() {
                    return Err(ProgramError::InvalidAccountData);
                }
                msg!("GM {}", user.authority);
                Ok(())
            }
            AccountDiscriminant::Metadata(metadata) => {
                if ctx.accounts.user.owner != ctx.program_id {
                    return Err(ProgramError::IllegalOwner);
                }
                msg!("GM {}", metadata.account);
                Ok(())
            }
        }
    }
}

#[derive(Accounts)]
pub struct UpdateUser<'info> {
    user: AccountInfo<'info>,
    authority: Signer<'info>,
}

#[derive(BorshSerialize, BorshDeserialize)]
pub struct User {
    authority: Pubkey,
}

#[derive(BorshSerialize, BorshDeserialize)]
pub struct Metadata {
    account: Pubkey,
}

#[derive(BorshSerialize, BorshDeserialize)]
pub enum AccountDiscriminant {
    User(User),
    Metadata(Metadata),
}

#[allow(dead_code)]
fn main() {}
