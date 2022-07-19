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
