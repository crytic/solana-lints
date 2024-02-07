# arbitrary_cpi

**What it does:**
Finds uses of solana_program::program::invoke that do not check the program_id

**Why is this bad?**
A contract could call into an attacker-controlled contract instead of the intended one

**Known problems:**
False positives, since the program_id check may be within some other function (does not
trace through function calls)
False negatives, since our analysis is not path-sensitive (the program_id check may not
occur in all possible execution paths)

**Example:**

```rust
// example code where a warning is issued
let ix = Instruction {
  program_id: *program_id,
  accounts: vec![AccountMeta::new_readonly(*program_id, false)],
  data: vec![0; 16],
};
invoke(&ix, accounts.clone());

```

Use instead:

```rust
// example code that does not raise a warning
if (*program_id == ...) {
    ...
}
let ix = Instruction {
  program_id: *program_id,
  accounts: vec![AccountMeta::new_readonly(*program_id, false)],
  data: vec![0; 16],
};
invoke(&ix, accounts.clone());
```

**How the lint is implemented:**

- For every function containing calls to `solana_program::program::invoke`
- find the definition of `Instruction` argument passed to `invoke`; first argument
- If the `Instruction` argument is result of a function call
  - If the function is whitelisted, do not report; only functions defined in
    `spl_token::instruction` are whitelisted.
  - Else report the call to `invoke` as vulnerable
- Else if the `Instruction` is initialized in the function itself
  - find the assign statement assigning to the `program_id` field, assigning to
    field at `0`th index
  - find all the aliases of `program_id`. Use the rhs of the assignment as initial
    alias and look for all assignments assigning to the locals recursively.
  - If `program_id` is compared using any of aliases ignore the call to `invoke`.
    - Look for calls to `core::cmp::PartialEq{ne, eq}` where one of arg is moved
      from an alias.
    - If one of the arg accesses `program_id` and if the basic block containing the
      comparison dominates the basic block containing call to `invoke` ensuring the
      `program_id` is checked in all execution paths Then ignore the call to `invoke`.
    - Else report the call to `invoke`.
  - Else report the call to `invoke`.
