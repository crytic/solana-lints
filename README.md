# Trail of Bits Solana lints

Each subdirectory of [`lints`](lints) contains a Solana lint in the form of a [Dylint](https://github.com/trailofbits/dylint) library.

The lints are inspired by the [Sealevel Attacks](https://github.com/coral-xyz/sealevel-attacks).

The current lints are:

| Library                                                  | Description                                                                                                                  |
| -------------------------------------------------------- | ---------------------------------------------------------------------------------------------------------------------------- |
| [`arbitrary_cpi`](lints/arbitrary_cpi)                   | lint for [5-arbitrary-cpi](https://github.com/coral-xyz/sealevel-attacks/tree/master/programs/5-arbitrary-cpi)               |
| [`insecure_account_close`](lints/insecure_account_close) | lint for [9-closing-accounts](https://github.com/coral-xyz/sealevel-attacks/tree/master/programs/9-closing-accounts)         |
| [`missing_owner_check`](lints/missing_owner_check)       | lint for [2-owner-checks](https://github.com/coral-xyz/sealevel-attacks/tree/master/programs/2-owner-checks)                 |
| [`missing_signer_check`](lints/missing_signer_check)     | lint for [0-signer-authorization](https://github.com/coral-xyz/sealevel-attacks/tree/master/programs/0-signer-authorization) |
| [`type_cosplay`](lints/type_cosplay)                     | lint for [3-type-cosplay](https://github.com/coral-xyz/sealevel-attacks/tree/master/programs/3-type-cosplay)                 |

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
       { git = "https://github.com/trailofbits/solana-lints", pattern = "lints/*" },
   ]
   ```

3. Run `cargo-dylint`:
   ```sh
   cargo dylint --all --workspace
   ```
