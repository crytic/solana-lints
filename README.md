# Trail of Bits Solana lints

Solana Breakpoint 2022 [slides] [video]

Each subdirectory of [`lints`] contains a Solana lint in the form of a [Dylint] library.

The lints are inspired by the [Sealevel Attacks]. (See also @pencilflip's [Twitter thread].)

The current lints are:

| Library                                                          | Description                                                                                                                              |
| ---------------------------------------------------------------- | ---------------------------------------------------------------------------------------------------------------------------------------- |
| [`arbitrary_cpi`](lints/arbitrary_cpi)                           | lint for [5-arbitrary-cpi](https://github.com/coral-xyz/sealevel-attacks/tree/master/programs/5-arbitrary-cpi)                           |
| [`bump_seed_canonicalization`](lints/bump_seed_canonicalization) | lint for [6-bump-seed-canonicalization](https://github.com/coral-xyz/sealevel-attacks/tree/master/programs/7-bump-seed-canonicalization) |
| [`insecure_account_close`](lints/insecure_account_close)         | lint for [9-closing-accounts](https://github.com/coral-xyz/sealevel-attacks/tree/master/programs/9-closing-accounts)                     |
| [`missing_owner_check`](lints/missing_owner_check)               | lint for [2-owner-checks](https://github.com/coral-xyz/sealevel-attacks/tree/master/programs/2-owner-checks)                             |
| [`missing_signer_check`](lints/missing_signer_check)             | lint for [0-signer-authorization](https://github.com/coral-xyz/sealevel-attacks/tree/master/programs/0-signer-authorization)             |
| [`sysvar_get`](lints/sysvar_get)                                 | Reports uses of `Sysvar::from_account_info` instead of `Sysvar::get`                                                                     |
| [`type_cosplay`](lints/type_cosplay)                             | lint for [3-type-cosplay](https://github.com/coral-xyz/sealevel-attacks/tree/master/programs/3-type-cosplay)                             |

## Usage

To use these lints, do the following:

1. Install `cargo-dylint` and `dylint-link`:

   ```sh
   cargo install cargo-dylint dylint-link
   ```

2. Add the following to your workspace's `Cargo.toml` file:

   ```toml
   [workspace.metadata.dylint]
   libraries = [
       { git = "https://github.com/crytic/solana-lints", pattern = "lints/*" },
   ]
   ```

3. Run `cargo-dylint`:
   ```sh
   cargo dylint --all --workspace
   ```

[`lints`]: lints
[dylint]: https://github.com/trailofbits/dylint
[sealevel attacks]: https://github.com/coral-xyz/sealevel-attacks
[slides]: docs/Dylint%20Can%20Help%20you%20Write%20More%20Secure%20Solana%20Contracts.pdf
[twitter thread]: https://threadreaderapp.com/thread/1483880018858201090.html
[video]: https://www.youtube.com/watch?v=AulT4TaPf1M
