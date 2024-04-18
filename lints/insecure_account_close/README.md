# insecure_account_close

**What it does:**

Checks for attempts to close an account by setting its lamports to `0` but
not also clearing its data.

**Why is this bad?**

See: https://docs.solana.com/developing/programming-model/transactions#multiple-instructions-in-a-single-transaction

> An example of where this could be a problem is if a token program, upon transferring the token out of an account, sets the account's lamports to zero, assuming it will be deleted by the runtime. If the program does not zero out the account's data, a malicious user could trail this instruction with another that transfers the tokens a second time.

**Works on:**

- [x] Anchor
- [x] Non Anchor

**Known problems:**

None

**Example:**

See https://github.com/coral-xyz/sealevel-attacks/tree/master/programs/9-closing-accounts for examples of insecure, secure and recommended
approach to close an account.

**How the lint is implemented:**

- For every expression like `(*(*some_expr).lamports.borrow_mut()) = 0;`; assigning `0` to account's lamports
- If the body enclosing the expression `is_force_defund`, ignore the expression
  - The body contains expressions `some_expr.copy_from_slice(&another_expr[0..8])`
    and comparison expression comparing an `[u8; 8]` value.
- Else If the body contains a manual clear of the account data
  - If the body has a for loop like pattern and the loop body has an expression
    assigning zero
    - Assume the loop is clearing the account data and the expression is safe
- Else
  - report the expression as vulnerable
