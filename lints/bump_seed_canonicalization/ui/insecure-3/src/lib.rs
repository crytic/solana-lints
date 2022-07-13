use anchor_lang::prelude::*;
use anchor_lang::solana_program::entrypoint::ProgramResult;
use borsh::{BorshDeserialize, BorshSerialize};

declare_id!("Fg6PaFpoGXkYsidMpWTK6W2BeZ7FEfcYkg476zPFsLnS");

#[program]
pub mod bump_seed_canonicalization_insecure {
    use super::{AnotherStruct, BumpSeed, Context, Key, ProgramError, ProgramResult, Pubkey};

    pub fn set_value(ctx: Context<BumpSeed>, key: u64, s: AnotherStruct) -> ProgramResult {
        let address = Pubkey::create_program_address(
            &[key.to_le_bytes().as_ref(), &[s.bump]],
            ctx.program_id,
        )?;
        if address != ctx.accounts.data.key() {
            return Err(ProgramError::InvalidArgument);
        }

        ctx.accounts.data.value = 1;

        Ok(())
    }
}

#[derive(Accounts)]
pub struct BumpSeed<'info> {
    data: Account<'info, Data>,
}

#[account]
pub struct Data {
    value: u64,
}

#[derive(Clone, BorshDeserialize, BorshSerialize)]
pub struct AnotherStruct {
    bump: u8,
}

fn main() {}
