use anchor_lang::prelude::*;
use anchor_lang::solana_program::entrypoint::ProgramResult;

declare_id!("Fg6PaFpoGXkYsidMpWTK6W2BeZ7FEfcYkg476zPFsLnS");

#[program]
pub mod sysvar_get_secure {
    use super::*;

    pub fn log_message(ctx: Context<LogMessage>) -> ProgramResult {
        let clock = Clock::get()?;
        msg!(
            "Authority {} signed at {}",
            ctx.accounts.authority.to_account_info().key,
            clock.slot
        );
        Ok(())
    }
}

#[derive(Accounts)]
pub struct LogMessage<'info> {
    authority: Signer<'info>,
}

#[allow(dead_code)]
fn main() {}
