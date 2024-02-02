# bump_seed_canonicalization

**What it does:**

Finds uses of solana_program::pubkey::PubKey::create_program_address that do not check the bump_seed

**Why is this bad?**

Generally for every seed there should be a canonical address, so the user should not be
able to pick the bump_seed, since that would result in a different address.

See https://github.com/crytic/building-secure-contracts/tree/master/not-so-smart-contracts/solana/improper_pda_validation

**Known problems:**

False positives, since the bump_seed check may be within some other function (does not
trace through function calls). The bump seed may be also be safely stored in an account but
passed from another function.

False negatives, since our analysis is not path-sensitive (the bump_seed check may not
occur in all possible execution paths)

**Example:**

See https://github.com/coral-xyz/sealevel-attacks/blob/master/programs/7-bump-seed-canonicalization/insecure/src/lib.rs for an insecure example

Use instead:

See https://github.com/coral-xyz/sealevel-attacks/blob/master/programs/7-bump-seed-canonicalization/recommended/src/lib.rs for recommended way to use bump.

**How the lint is implemented:**

- For every function containing calls to `solana_program::pubkey::Pubkey::create_program_address`
- find the `bump` location from the first argument to `create_program_address` call.
    - first argument is the seeds array(`&[&[u8]]`). In general, the seeds are structured with bump as last element:
    `&[seed1, seed2, ..., &[bump]]` e.g `&[b"vault", &[bump]]`.
    - find the locations of bump.
    - If bump is assigned by accessing a struct field
        - if bump is assigned from a struct implementing `AnchorDeserialize` trait
            - report a warning to use `#[account(...)` macro
        - else report "bump may not be constrainted" warning
    - else check if the bump is checked using a comparison operation
        - report a warning if the bump is not checked
