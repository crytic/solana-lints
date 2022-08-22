fn try_accounts(
    program_id: &anchor_lang::solana_program::pubkey::Pubkey,
    accounts: &mut &[anchor_lang::solana_program::account_info::AccountInfo<'info>],
    ix_data: &[u8],
    __bumps: &mut std::collections::BTreeMap<String, u8>,
) -> anchor_lang::Result<Self> {
    if accounts.is_empty() {
        return Err(anchor_lang::error::ErrorCode::AccountNotEnoughKeys.into());
    }
    let user = &accounts[0]; // user is first account. need to do init stuff
    *accounts = &accounts[1..];
    // consumes accounts[1] into a Signer account (info.is_signer == true)
    let authority: Signer =
        anchor_lang::Accounts::try_accounts(program_id, accounts, ix_data, __bumps)
            .map_err(|e| e.with_account_name("authority"))?;
    // consumes accounts[2] into a Program<System> account. Program validates System is really System program account.
    let system_program: anchor_lang::accounts::program::Program<System> =
        anchor_lang::Accounts::try_accounts(program_id, accounts, ix_data, __bumps)
            .map_err(|e| e.with_account_name("system_program"))?;
    let __anchor_rent = Rent::get()?;

    // here is where a lot of "init" stuff is done:
    let user = {
        let actual_field = user.to_account_info();
        let actual_owner = actual_field.owner;
        let space = 8 + 32; // from the macro discrim + account bytes

        // creates Account<User> that is "initialized"
        let pa: anchor_lang::accounts::account::Account<User> = if !false // not sure what this !false does
            || actual_owner == &anchor_lang::solana_program::system_program::ID
        // this check never hits???
        {
            let payer = authority.to_account_info(); // from macro payer = authority
            let __current_lamports = user.lamports();

            // if no lamports, create a new account with system program
            if __current_lamports == 0 {
                let lamports = __anchor_rent.minimum_balance(space); // get min lamports needed to be rent-exempt
                                                                     // prepares account used in CPI. Calling system program, using payer and user accounts.
                let cpi_accounts = anchor_lang::system_program::CreateAccount {
                    from: payer.to_account_info(),
                    to: user.to_account_info(),
                };
                let cpi_context = anchor_lang::context::CpiContext::new(
                    system_program.to_account_info(),
                    cpi_accounts,
                );
                // creates user account with system program
                anchor_lang::system_program::create_account(
                    cpi_context.with_signer(&[]),
                    lamports,
                    space as u64,
                    program_id,
                )?;
            } else {
                // user account has some lamports. allocate space and assign owner
                let required_lamports = __anchor_rent
                    .minimum_balance(space)
                    .max(1)
                    .saturating_sub(__current_lamports);
                // transfer required lamports to the user account
                if required_lamports > 0 {
                    let cpi_accounts = anchor_lang::system_program::Transfer {
                        from: payer.to_account_info(),
                        to: user.to_account_info(),
                    };
                    let cpi_context = anchor_lang::context::CpiContext::new(
                        system_program.to_account_info(),
                        cpi_accounts,
                    );
                    anchor_lang::system_program::transfer(cpi_context, required_lamports)?;
                }
                // allocate `space` for `user` account
                let cpi_accounts = anchor_lang::system_program::Allocate {
                    account_to_allocate: user.to_account_info(),
                };
                let cpi_context = anchor_lang::context::CpiContext::new(
                    system_program.to_account_info(),
                    cpi_accounts,
                );
                anchor_lang::system_program::allocate(cpi_context.with_signer(&[]), space as u64)?;
                // assign owner of user account as program_id
                let cpi_accounts = anchor_lang::system_program::Assign {
                    account_to_assign: user.to_account_info(),
                };
                let cpi_context = anchor_lang::context::CpiContext::new(
                    system_program.to_account_info(),
                    cpi_accounts,
                );
                anchor_lang::system_program::assign(cpi_context.with_signer(&[]), program_id)?;
            }
            // deserializes AccountInfo.data into T type (User). Does not check account discriminator
            anchor_lang::accounts::account::Account::try_from_unchecked(&user)?
        } else {
            // never executes. but same as above, except checks account discriminator
            anchor_lang::accounts::account::Account::try_from(&user)?
        };
        // never executes(why??)
        if false {
            if space != actual_field.data_len() {
                return Err(anchor_lang::error::Error::from(
                    anchor_lang::error::ErrorCode::ConstraintSpace,
                )
                .with_account_name("user")
                .with_values((space, actual_field.data_len())));
            }
            if actual_owner != program_id {
                return Err(anchor_lang::error::Error::from(
                    anchor_lang::error::ErrorCode::ConstraintOwner,
                )
                .with_account_name("user")
                .with_pubkeys((*actual_owner, *program_id)));
            }
            {
                let required_lamports = __anchor_rent.minimum_balance(space);
                if pa.to_account_info().lamports() < required_lamports {
                    return Err(anchor_lang::error::Error::from(
                        anchor_lang::error::ErrorCode::ConstraintRentExempt,
                    )
                    .with_account_name("user"));
                }
            }
        }
        pa
    };
    // various checks
    if !user.to_account_info().is_writable {
        return Err(
            anchor_lang::error::Error::from(anchor_lang::error::ErrorCode::ConstraintMut)
                .with_account_name("user"),
        );
    }
    if !user.to_account_info().is_signer {
        return Err(anchor_lang::error::Error::from(
            anchor_lang::error::ErrorCode::ConstraintSigner,
        )
        .with_account_name("user"));
    }
    if !__anchor_rent.is_exempt(
        user.to_account_info().lamports(),
        user.to_account_info().try_data_len()?,
    ) {
        return Err(anchor_lang::error::Error::from(
            anchor_lang::error::ErrorCode::ConstraintRentExempt,
        )
        .with_account_name("user"));
    }
    if !authority.to_account_info().is_writable {
        return Err(
            anchor_lang::error::Error::from(anchor_lang::error::ErrorCode::ConstraintMut)
                .with_account_name("authority"),
        );
    }
    Ok(Init {
        user,
        authority,
        system_program,
    })
}
