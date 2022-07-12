use anchor_lang::prelude::*;
use borsh::{BorshDeserialize, BorshSerialize};

declare_id!("Fg6PaFpoGXkYsidMpWTK6W2BeZ7FEfcYkg476zPFsLnS");

// This example is secure because it only deserializes from a single enum, and that enum
// encapsulates all of the user-defined types. Since enums contain an implicit discriminant,
// this program will always be secure as long as all types are defined under the enum.
#[program]
pub mod type_cosplay_secure {
    use super::*;

    pub fn update_user(
        ctx: Context<UpdateUser>,
    ) -> anchor_lang::solana_program::entrypoint::ProgramResult {
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

fn main() {}
