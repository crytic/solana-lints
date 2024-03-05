use anchor_lang::prelude::*;
use anchor_lang::solana_program::entrypoint::ProgramResult;

declare_id!("Fg6PaFpoGXkYsidMpWTK6W2BeZ7FEfcYkg476zPFsLnS");

#[program]
pub mod signer_authorization_secure {
    use super::*;

    pub fn log_message(ctx: Context<LogMessage>) -> ProgramResult {
        if !ctx.accounts.authority.is_signer {
            return Err(ProgramError::MissingRequiredSignature);
        }
        msg!("GM {}", ctx.accounts.authority.key().to_string());
        Ok(())
    }
}

#[derive(Accounts)]
pub struct LogMessage<'info> {
    authority: AccountInfo<'info>,
}

// This is a false positive as the lint does not check for `is_signer` checks if the
// program is an anchor program. The lint should be updated to remove the false positive.
#[allow(dead_code)]
fn main() {}
