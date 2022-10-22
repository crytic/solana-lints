use anchor_lang::prelude::*;
use anchor_lang::solana_program::entrypoint::ProgramResult;
use borsh::{BorshDeserialize, BorshSerialize};

declare_id!("Fg6PaFpoGXkYsidMpWTK6W2BeZ7FEfcYkg476zPFsLnS");

// NOTE: what about the case where we only deserialize from one enum? Could this still be a vulnerability?
// ex. only deser from UserInfo. Someone sets data as MetadataInfo::Metadata(...). will this match?
// what if MetadataInfo::User(...)?

#[program]
pub mod type_cosplay_insecure {
    use super::*;

    pub fn update_user(ctx: Context<UpdateUser>) -> ProgramResult {
        let user = UserInfo::try_from_slice(&ctx.accounts.user.data.borrow()).unwrap();
        match user {
            UserInfo::User(u) => {
                if ctx.accounts.user.owner != ctx.program_id {
                    return Err(ProgramError::IllegalOwner);
                }
                if u.authority != ctx.accounts.authority.key() {
                    return Err(ProgramError::InvalidAccountData);
                }
                msg!("GM {}", u.authority);
                // Ok(())
            }
        };

        let metadata = MetadataInfo::try_from_slice(&ctx.accounts.user.data.borrow()).unwrap();
        match metadata {
            MetadataInfo::Metadata(m) => {
                if ctx.accounts.user.owner != ctx.program_id {
                    return Err(ProgramError::IllegalOwner);
                }
                if m.account != ctx.accounts.authority.key() {
                    return Err(ProgramError::InvalidAccountData);
                }
                msg!("GM {}", m.account);
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
pub enum UserInfo {
    User(User),
}

#[derive(BorshSerialize, BorshDeserialize)]
pub enum MetadataInfo {
    Metadata(Metadata),
}

#[allow(dead_code)]
fn main() {}
