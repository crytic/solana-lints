# missing_owner_check

**What it does:**

This lint checks that for each account referenced in a program, that there is a
corresponding owner check on that account. Specifically, this means that the owner
field is referenced on that account.

**Why is this bad?**

The missing-owner-check vulnerability occurs when a program uses an account, but does
not check that it is owned by the expected program. This could lead to vulnerabilities
where a malicious actor passes in an account owned by program `X` when what was expected
was an account owned by program `Y`. The code may then perform unexpected operations
on that spoofed account.

For example, suppose a program expected an account to be owned by the SPL Token program.
If no owner check is done on the account, then a malicious actor could pass in an
account owned by some other program. The code may then perform some actions on the
unauthorized account that is not owned by the SPL Token program.

**Works on:**

- [x] Anchor
- [x] Non Anchor

**Known problems:**

Key checks can be strengthened. Currently, the lint only checks that the account's owner
field is referenced somewhere, ie, `AccountInfo.owner`.

**Example:**

See https://github.com/coral-xyz/sealevel-attacks/blob/master/programs/2-owner-checks/insecure/src/lib.rs
for an insecure example.

Use instead:

See https://github.com/coral-xyz/sealevel-attacks/blob/master/programs/2-owner-checks/secure/src/lib.rs
for a secure example.

**How the lint is implemented:**

check_fn:

- for every function defined in the package
- exclude functions generated from macro expansion.
- Get a list of unique and unsafe AccountInfo's referenced in the body
  - for each expression in the function body
  - Ignore `.clone()` expressions as the expression referencing original account will be checked
  - Check if the expression's type is Solana's `AccountInfo` (`solana_account_info::AccountInfo`)
  - Ignore local variable expressions (`x` where x is defined in the function `let x = y`)
    - Removes duplcate warnings: both `x` and `y` are reported by the lint. reporting `y` is sufficient.
    - Also the owner could be checked on `y`. reporting `x` which a copy/ref of `y` would be false-positive.
    - Determined using the expression kind (`.kind`): expr.kind = ExprKind::Path(QPath::Resolved(None, path)); path.segments.len() == 1
  - Ignore safe `.to_account_info()` expressions
    - `.to_account_info()` method can be called to convert different Anchor account types to `AccountInfo`
    - The Anchor account types such as `Account` implement `Owner` trait: The owner of the account is checked during deserialization
    - The expressions `x.to_account_info()` where `x` has one of following types are ignored:
      - `Account` requires its type argument to implement `anchor_lang::Owner`.
      - `Program`'s implementation of `try_from` checks the account's program id. So there is
        no ambiguity in regard to the account's owner.
      - `SystemAccount`'s implementation of `try_from` checks that the account's owner is the System Program.
      - `AccountLoader` requires its type argument to implement `anchor_lang::Owner`.
      - `Signer` are mostly accounts with a private key and most of the times owned by System Program.
      - `Sysvar` type arguments checks the account key.
  - Ignore `x.to_account_info()` expressions called on Anchor `AccountInfo` to remove duplicates.
    - the lint checks the original expression `x`; no need for checking both.
- For each of the collected expressions, check if `owner` is accessed or if the `key` is compared
  - Ignore the `account_expr` if any of the expressions in the function is `{account_expr}.owner`
  - Ignore the `account_expr` if `key` is compared
    - if there is a comparison expression (`==` or `!=`) and one of the expressions being compared accesses key on `account_expr`:
      - lhs or rhs of the comparison is `{account_expr}.key()`; The key for Anchor's `AccountInfo` is accessed using `.key()`
      - Or lhs or rhs is `{account_expr}.key`; The key of Solana `AccountInfo` are accessed using `.key`
  - Else
    - If the expression is `.to_account_info()` and the receiver is a field access on a struct: `x.y.to_account_info()`
    - Or If the expression is a field access on a struct `x.y`
      - Then store the struct(x) def id and the accessed field name (y) in `MissingOwnerCheck.account_exprs`.
    - Else report the expression.

check_item: Collect Anchor `Accounts` structs

- for each item defined in the crate
  - If Item is a Struct and implements `anchor_lang::ToAccountInfos` trait.
    - Get the pre-expansion source code and parse it using anchor's accounts parser
    - If parsing succeeds
      - Then store the struct def id and the resultant AccountsStruct in `MissingOwnerCheck.anchor_accounts`

check_crate_post:

- for each account expression in `MissingOwnerCheck.account_exprs`
  - If the struct accessed in the expression is in `MissingOwnerCheck.anchor_accounts`
    - find the `#[account(...)]` constraints applied on the accessed field
    - If any of the following constraints are applied on the field/account
      - Then ignore the expression.
      - Constraints:
        - `#[account(signer)]` - Signer accounts are assumed to be EOA accounts and are ignored.
        - `#[account(init, ...)]` - init creates a new account and sets its owner to current program or the given program.
        - `#[account(seeds = ..., ...)]` - Anchor derives a PDA using the seeds. This is essentially a `key` check
        - `#[account(address = ...)]` - Validates the key of the account.
        - `#[account(owner = ...)]` - Checks the owner.
        - `#[account(executable)]` - The account is an executable; All executables are owned by `BPFLoaders`.
      - Else report the expression.
