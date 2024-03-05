use solana_program::msg;
use solana_program::{
    account_info::{next_account_info, AccountInfo},
    entrypoint,
    entrypoint::ProgramResult,
    program_error::ProgramError,
    pubkey::Pubkey,
};

entrypoint!(process_instruction);
fn process_instruction(
    _program_id: &Pubkey,
    accounts: &[AccountInfo],
    instruction_data: &[u8],
) -> ProgramResult {
    if instruction_data.len() != 0 {
        return Err(ProgramError::InvalidInstructionData);
    }
    log_message(accounts)
}

pub fn log_message(accounts: &[AccountInfo]) -> ProgramResult {
    let authority = next_account_info(&mut accounts.iter())?;
    msg!("GM {:?}", authority);
    Ok(())
}

#[allow(dead_code)]
fn main() {}
