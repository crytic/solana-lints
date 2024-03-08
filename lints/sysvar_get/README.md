# sysvar_get

**What it does:**

Lint warns uses of `Sysvar::from_account_info` and suggests to use `Sysvar::get` instead for
the sysvars implementing `Sysvar::get` function. The following sysvars implement `Sysvar::get`:

- Clock
- EpochRewards
- EpochSchedule
- Fees
- LastRestartSlot
- Rent

**Why is this bad?**

The `Sysvar::from_account_info` is less efficient than `Sysvar::get` because:

- The `from_account_info` requires that Sysvar account is passed to the program wasting the limited space
  available to the transactions.
- The `from_account_info` deserializes the Sysvar account data wasting the computation budget.

The `Sysvar::from_account_info` should be used if and only if the program interacts with an old program that
requires the sysvar account to be passed in CPI call. The program could avoid deserialization overhead by using
the passed Sysvar account in CPI (after verifying the ID) and using the `Sysvar::get`.

References:
[`solana_program/sysvar` docs](https://docs.rs/solana-program/latest/solana_program/sysvar/index.html#:~:text=programs%20should%20prefer%20to%20call%20Sysvar%3A%3Aget),
[Anchor docs](https://docs.rs/anchor-lang/latest/anchor_lang/accounts/sysvar/struct.Sysvar.html#:~:text=If%20possible%2C%20sysvars%20should%20not%20be%20used%20via%20accounts)

**Known problems:**

None

**Example:**

```rust
    let clock_account = next_account_info(account_info_iter)?;
    let clock = clock::Clock::from_account_info(&clock_account)?;
```

Use instead:

```rust
    let clock = clock::Clock::get()?;
```

**How the lint is implemented:**

- For every item
  - If item is a struct and has `#[derive(Accounts)]` macro
  - For each field in the struct
    - If field is of type Ty::Sysvar(T) and T is one of `Clock`, `EpochRewards`, `EpochSchedule`, `Fees`, `LastRestartSlot`, `Rent`
      - Then report the field and suggest to T::get().
- For every function
  - If an expr in function calls T::x() where x is `solana_program::Sysvar::from_account_info` and
    T is one of sysvars that implements `Sysvar::get()` method.
    - report the call expr and suggest to use T::get().
