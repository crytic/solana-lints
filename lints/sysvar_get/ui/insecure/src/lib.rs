use anchor_lang::prelude::*;
use anchor_lang::solana_program::entrypoint::ProgramResult;
use anchor_lang::solana_program::epoch_rewards::*;
use anchor_lang::solana_program::last_restart_slot::*;
use anchor_lang::solana_program::program_error::ProgramError;
use anchor_lang::solana_program::program_pack::Pack;
use anchor_spl::token::spl_token::state::Account as SplTokenAccount;

declare_id!("Fg6PaFpoGXkYsidMpWTK6W2BeZ7FEfcYkg476zPFsLnS");

#[program]
pub mod sysvar_get_insecure {
    use super::*;

    pub fn log_message(ctx: Context<LogMessage>) -> ProgramResult {
        let token = SplTokenAccount::unpack(&ctx.accounts.token.data.borrow())?;
        if ctx.accounts.authority.key != &token.owner {
            return Err(ProgramError::InvalidAccountData);
        }
        msg!("Your account balance is: {}", token.amount);

        Ok(())
    }

    pub fn rent_clock(_ctx: Context<RentClock>) -> ProgramResult {
        msg!("rent, clock");
        Ok(())
    }

    pub fn use_from(ctx: Context<UseFrom>) -> ProgramResult {
        let _clock = Clock::from_account_info(&ctx.accounts.clock);
        let _rewards = EpochRewards::from_account_info(&ctx.accounts.epoch_rewards);
        let _schedule = EpochSchedule::from_account_info(&ctx.accounts.epoch_schedule);
        let _last_restart_slot =
            LastRestartSlot::from_account_info(&ctx.accounts.last_restart_slot);
        let _rent = Rent::from_account_info(&ctx.accounts.rent);
        Ok(())
    }
}

#[derive(Accounts)]
pub struct LogMessage<'info> {
    token: AccountInfo<'info>,
    authority: Signer<'info>,
    clock: Sysvar<'info, Clock>,
    rent: AccountInfo<'info>,
}

#[derive(Accounts)]
pub struct RentClock<'info> {
    token: AccountInfo<'info>,
    authority: Signer<'info>,
    rent: Sysvar<'info, Rent>,
    clock: Sysvar<'info, Clock>,
}

#[derive(Accounts)]
pub struct UseFrom<'info> {
    clock: AccountInfo<'info>,
    epoch_rewards: AccountInfo<'info>,
    epoch_schedule: AccountInfo<'info>,
    last_restart_slot: AccountInfo<'info>,
    rent: AccountInfo<'info>,
}

#[allow(dead_code)]
fn main() {}
