# bump_seed_canonicalization

**What it does:**
Finds uses of solana_program::pubkey::PubKey::create_program_address that do not check the bump_seed

**Why is this bad?**
Generally for every seed there should be a canonical address, so the user should not be
able to pick the bump_seed, since that would result in a different address.

**Known problems:**
False positives, since the bump_seed check may be within some other function (does not
trace through function calls). The bump seed may be also be safely stored in an account but
passed from another function.

False negatives, since our analysis is not path-sensitive (the bump_seed check may not
occur in all possible execution paths)

**Example:**
