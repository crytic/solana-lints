use anchor_lang::prelude::*;
use borsh::{BorshDeserialize, BorshSerialize};

declare_id!("Fg6PaFpoGXkYsidMpWTK6W2BeZ7FEfcYkg476zPFsLnS");

// This example is insecure because AccountWithDiscriminant could be deserialized as a
// User, if the variant is Extra(Extra). The first byte would be 0, to indicate the discriminant
// in both cases, and the next 32 bytes would be the pubkey.
#[program]
pub mod type_cosplay_secure {
    use super::*;

    pub fn update_user(
        ctx: Context<UpdateUser>,
    ) -> anchor_lang::solana_program::entrypoint::ProgramResult {
        let user = User::try_from_slice(&ctx.accounts.user.data.borrow()).unwrap();
        if ctx.accounts.user.owner != ctx.program_id {
            return Err(ProgramError::IllegalOwner);
        }
        if user.authority != ctx.accounts.authority.key() {
            return Err(ProgramError::InvalidAccountData);
        }
        if user.discriminant != AccountDiscriminant::User {
            return Err(ProgramError::InvalidAccountData);
        }
        msg!("GM {}", user.authority);

        let extra = Instruction::try_from_slice(&ctx.accounts.user.data.borrow()).unwrap();
        Ok(())
    }
}

#[derive(Accounts)]
pub struct UpdateUser<'info> {
    user: AccountInfo<'info>,
    authority: Signer<'info>,
}

#[derive(BorshSerialize, BorshDeserialize)]
pub struct User {
    discriminant: AccountDiscriminant,
    authority: Pubkey,
}

#[derive(BorshSerialize, BorshDeserialize)]
pub struct Metadata {
    discriminant: AccountDiscriminant,
    account: Pubkey,
}

#[derive(BorshSerialize, BorshDeserialize)]
pub struct Extra {
    account: Pubkey,
}

#[derive(BorshSerialize, BorshDeserialize, PartialEq)]
pub enum AccountDiscriminant {
    User,
    Metadata,
}

#[derive(BorshSerialize, BorshDeserialize)]
pub enum AccountWithDiscriminant {
    Extra(Extra),
    Metadata,
}

fn main() {}
