# sysvar_address_check

**What it does:**

**Why is this bad?**

**Known problems:** None.

**Example:**

```rust
// example code where a warning is issued
```

Use instead:

```rust
// example code that does not raise a warning
```

**insecure-2:** an actual insecure example since it does deserialization without checking ID. Essentially what
the sealevel-insecure example was trying to demonstrate
**insecure-anchor:** an insecure example that more directly uses anchor, ie, uses Account<'info, T>, where T
derives Sysvar. This doesn't do the ID check that the anchor Sysvar type does.
**recommended:** the secure way to use anchor to check IDs. Directly fixes "insecure-anchor" by using Sysvar type.
**secure-2:** Fixes "insecure-2" case. Uses the `from_account_info()` method, which performs the ID check.

NOT TESTED:
insecure: sealevel-example. not tested because it's technically not insecure, since it never attempts
a deserialization
secure: poor sealevel example. Does key check, but never deserializes, so technically the vulnerability is
non-existent in this example.