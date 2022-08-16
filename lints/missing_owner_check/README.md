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

**Known problems:**

Key checks can be strengthened. Currently, the lint only checks that the account's owner
field is referenced somewhere, ie, `AccountInfo.owner`.

**Example:**

See https://github.com/coral-xyz/sealevel-attacks/blob/master/programs/2-owner-checks/insecure/src/lib.rs
for an insecure example.

Use instead:

See https://github.com/coral-xyz/sealevel-attacks/blob/master/programs/2-owner-checks/secure/src/lib.rs
for a secure example.
