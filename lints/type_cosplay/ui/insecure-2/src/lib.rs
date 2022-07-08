use anchor_lang::prelude::*;
use borsh::{BorshDeserialize, BorshSerialize};

declare_id!("Fg6PaFpoGXkYsidMpWTK6W2BeZ7FEfcYkg476zPFsLnS");

// NOTE: this is insecure because the program tries to deserialize from multiple enum types.
// This is a vulnerability because it partitions the "set of types" and is thus not exhaustive
// in discriminating between all types. Deserializing from a single enum is safe because the
// program will have to match the deserialized result against an enum variant in order to reconstruct
// the ADT. This effectively serves as a discriminant, because even if two types deserialize the same,
// the programmer will explicitly match it to a type.
// If multiple enums are used to encompass the types, this defense will basically be breached.
// There may be two different types that are equivalent. If we deserialize from an enum,
// type B could be passed in, when what was expected was type A.

// NOTE: what about the case where we only deserialize from one enum? Could this still be a vulnerability?
// ex. only deser from UserInfo. Someone sets data as MetadataInfo::Metadata(...). will this match?
// what if MetadataInfo::User(...)?

#[program]
pub mod type_cosplay_insecure {
    use super::*;

    pub fn update_user(
        ctx: Context<UpdateUser>,
    ) -> anchor_lang::solana_program::entrypoint::ProgramResult {
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

fn main() {}
