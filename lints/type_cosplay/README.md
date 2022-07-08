# type_cosplay

**What it does:**

**Why is this bad?**

**Known problems:** When only one enum is serialized, may miss certain edge cases.

**Example:**

```rust
// example code where a warning is issued
```

Use instead:

```rust
// example code that does not raise a warning
```

Returns true if the `adt` has a field that is an enum and the number of variants of that enum is at least the number of deserialized struct types collected.
