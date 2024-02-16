use anchor_lang::prelude::*;
use anchor_lang::solana_program::entrypoint::ProgramResult;

declare_id!("Fg6PaFpoGXkYsidMpWTK6W2BeZ7FEfcYkg476zPFsLnS");

fn check_accounts(accounts: &LogMessage) {
    msg!("#[account(seeds)]: {}", accounts.canonical_pda.key());
    msg!(
        "#[account(address)]: {}",
        accounts.address_acc.to_account_info().key()
    );
}

#[program]
pub mod owner_checks_secure {
    use super::*;

    pub fn log_message(ctx: Context<LogMessage>) -> ProgramResult {
        msg!("#[account(signer)]: {}", ctx.accounts.authority.key());
        msg!("#[account(init)]: {}", ctx.accounts.init_account.key());
        msg!(
            "#[account(owner)]: {}",
            ctx.accounts.owner_acc.to_account_info().key()
        );
        msg!(
            "#[account(executable)]: {}",
            ctx.accounts.executable_acc.to_account_info().key()
        );
        check_accounts(ctx.accounts);
        // Uncommenting the following line will fail the test.
        // Lint reports this because constraints only check that the account is writable.
        // msg!("#[account(mut)]: {}", ctx.accounts.receiver.key());
        Ok(())
    }
}

// close and realloc constraints require the type to be Account/AccountLoader
// zero constraint just checks that discriminator is zero. doesn't seem to write to it. Lint should report this case.
// init_if_needed constraint is reported by the lint.
// Seems like spl constraints require the Account to be TokenAccount or Mint. Not considering them
#[derive(Accounts)]
pub struct LogMessage<'info> {
    #[account(signer)]
    pub authority: AccountInfo<'info>,
    #[account(init, payer = payer, space = 8 + 8)]
    pub init_account: AccountInfo<'info>,
    #[account(seeds = [b"example_seed"], bump)]
    pub canonical_pda: AccountInfo<'info>,
    #[account(address = crate::ID)]
    pub address_acc: UncheckedAccount<'info>,
    #[account(owner = crate::ID)]
    pub owner_acc: UncheckedAccount<'info>,
    #[account(executable)]
    pub executable_acc: UncheckedAccount<'info>,
    #[account(mut)]
    pub payer: Signer<'info>,
    pub system_program: Program<'info, System>,
    #[account(mut)]
    pub receiver: AccountInfo<'info>,
}

#[allow(dead_code)]
fn main() {}
