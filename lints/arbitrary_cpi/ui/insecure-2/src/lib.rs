use anchor_lang::prelude::*;
use anchor_lang::solana_program;
use anchor_lang::solana_program::entrypoint::ProgramResult;
use anchor_lang::solana_program::instruction::Instruction;
use anchor_spl::token::{self, Token};

declare_id!("Fg6PaFpoGXkYsidMpWTK6W2BeZ7FEfcYkg476zPFsLnS");

#[program]
pub mod arbitrary_cpi_insecure {
    use super::*;

    pub fn cpi(ctx: Context<Cpi>, _amount: u64) -> ProgramResult {
        // Instruction {...}; Lint reports the ins
        let ins = Instruction {
            program_id: *ctx.accounts.some_program.key,
            accounts: vec![],
            data: vec![],
        };
        solana_program::program::invoke_signed(
            &ins,
            &[
                ctx.accounts.source.clone(),
                ctx.accounts.destination.clone(),
                ctx.accounts.authority.clone(),
            ],
            &[&[]],
        )?;

        // CpiContext::new(); Lint reports this
        let accounts = token::Transfer {
            from: ctx.accounts.source.to_account_info(),
            to: ctx.accounts.destination.to_account_info(),
            authority: ctx.accounts.authority.to_account_info(),
        };
        let cpi_ctx = CpiContext::new(ctx.accounts.some_program.to_account_info(), accounts);
        token::transfer(cpi_ctx, 100)?;

        // CpiContext::new_with_signer(); Lint reports this
        let accounts2 = token::Transfer {
            from: ctx.accounts.destination.to_account_info(),
            to: ctx.accounts.source.to_account_info(),
            authority: ctx.accounts.authority.to_account_info(),
        };

        let cpi_ctx_signers = CpiContext::new_with_signer(
            ctx.accounts.some_program.to_account_info(),
            accounts2,
            &[&[]],
        );
        token::transfer(cpi_ctx_signers, 1000)?;

        // CpiContext::new_with_signer()
        // The lint does not report this because `token_program` is of type `Program`
        let t = ctx.accounts.token_program.to_account_info();

        let accounts3 = token::Transfer {
            from: ctx.accounts.destination.to_account_info(),
            to: ctx.accounts.source.to_account_info(),
            authority: ctx.accounts.authority.to_account_info(),
        };

        let cpi_ctx_signers_crct = CpiContext::new_with_signer(t, accounts3, &[&[]]);
        token::transfer(cpi_ctx_signers_crct, 1000)?;

        Ok(())
    }
}

#[derive(Accounts)]
pub struct Cpi<'info> {
    source: AccountInfo<'info>,
    destination: AccountInfo<'info>,
    authority: AccountInfo<'info>,
    token_program: Program<'info, Token>,
    some_program: AccountInfo<'info>,
}

#[allow(dead_code)]
fn main() {}
