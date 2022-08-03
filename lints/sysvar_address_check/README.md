# sysvar_address_check

**What it does:** This lint checks to ensure that programs using Solana types that derive the
`Sysvar` trait (e.g. Rent, Clock) check the address of the account. The recommended way to
deal with these types is using the `from_account_info()` method from the `Sysvar` trait.
This method performs the ID check and only deserializes from an `AccountInfo` if the check
passes, and is thus secure.

This lint catches direct calls to deserialize (via `bincode::deserialize`) a byte array into
a type deriving Sysvar. Furthermore, if using the Anchor framework, this lint will catch
uses of `Account<'info, T>`, where `T` derives `Sysvar`. This is insecure since Anchor
will not perform the ID check in this case.

**Why is this bad?** If a program deserializes an `AccountInfo.data` directly, without
checking the ID first, a malicious user could pass in an `AccountInfo` with spoofed data
and the same structure as a `Sysvar` type. Then the program would be dealing with incorrect
data.

**Known problems:** This lint will flag any calls to deserialize some bytes into a type deriving
`Sysvar`, regardless of whether the ID check is done or not. Thus, if a program manually does the ID
check and deserialization, the lint will still flag this as insecure, thus possibly generating
some false positives. However, one should really prefer to use `from_account_info()`.

**Example:**

```rust
pub fn check_sysvar_address(ctx: Context<CheckSysvarAddress>) -> Result<()> {
    let rent: Rent = bincode::deserialize(&ctx.accounts.rent.data.borrow()).unwrap();
    msg!("Rent -> {}", rent.lamports_per_byte_year);
    Ok(())
}
```

Use instead:

```rust
pub fn check_sysvar_address(ctx: Context<CheckSysvarAddress>) -> Result<()> {
    let rent = Rent::from_account_info(&ctx.accounts.rent).unwrap();
    msg!("Rent -> {}", rent.lamports_per_byte_year);
    Ok(())
}
```

## Note on Tests

**insecure-2:** an actual insecure example since it does deserialization without checking ID. Essentially what
the sealevel insecure example was trying to demonstrate (see below).
**secure-2:** Fixes the "insecure-2" case. Uses the `from_account_info()` method, which performs the ID check.
**insecure-anchor:** an insecure example that uses `Account<'info, T>`, where `T`
derives Sysvar. This doesn't do the ID check that the `anchor::Sysvar` type does.
**recommended:** the secure way to use Anchor to check IDs. Directly fixes "insecure-anchor" by using
the `anchor::Sysvar` type.

### NOT TESTED:

**insecure:** sealevel-example; not tested because it's technically not insecure as it never attempts
a deserialization.
**secure:** sealevel-example; oes key check, but never deserializes, so technically the vulnerability is
non-existent in this example.
