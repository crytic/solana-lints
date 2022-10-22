use anchor_lang::prelude::*;
use anchor_lang::solana_program;
use anchor_lang::solana_program::entrypoint::ProgramResult;
use anchor_lang::solana_program::instruction::Instruction;

declare_id!("Fg6PaFpoGXkYsidMpWTK6W2BeZ7FEfcYkg476zPFsLnS");

#[program]
pub mod arbitrary_cpi_insecure {
    use super::*;

    pub fn cpi(ctx: Context<Cpi>, _amount: u64) -> ProgramResult {
        let ins = Instruction {
            program_id: *ctx.accounts.token_program.key,
            accounts: vec![],
            data: vec![],
        };
        solana_program::program::invoke(
            &ins,
            &[
                ctx.accounts.source.clone(),
                ctx.accounts.destination.clone(),
                ctx.accounts.authority.clone(),
            ],
        )
    }
}

#[derive(Accounts)]
pub struct Cpi<'info> {
    source: AccountInfo<'info>,
    destination: AccountInfo<'info>,
    authority: AccountInfo<'info>,
    token_program: AccountInfo<'info>,
}

#[allow(dead_code)]
fn main() {}
