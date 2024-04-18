# missing_signer_check

**What it does:**

This lint reports functions which use `AccountInfo` type and have zero signer checks.

**Why is this bad?**

The missing-signer-check vulnerability occurs when a program does not check that all the authorative
accounts have signed the instruction. The issue is lack of proper access controls. Verifying signatures is a way to
ensure the required entities has approved the operation. If a program does not check the signer field,
then anyone can create the instruction, call the program and perform a privileged operation.

For example if the Token program does not check that the owner of the tokens is a signer in the transfer instruction then anyone can
transfer the tokens and steal them.

**Works on:**

- [x] Anchor
- [x] Non Anchor

**Known problems:**
None.

**Example:**

See https://github.com/coral-xyz/sealevel-attacks/blob/master/programs/0-signer-authorization/insecure/src/lib.rs
for an insecure example.

Use instead:

See https://github.com/coral-xyz/sealevel-attacks/blob/master/programs/0-signer-authorization/recommended/src/lib.rs for a secure example

**How the lint is implemented:**

- For each free function, function not associated with any type or trait.
- If the function has an expression of type `AccountInfo` AND
- If the function does **not** take a `Context<T>` type argument where `T` has a `Signer` type field AND
- If the function does **not** has an expression `x.is_signer` where the expression `x` is of type `AccountInfo`.
  - Report the function
