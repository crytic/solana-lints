# Trail of Bits Solana lints

Each subdirectory of [`lints`](lints) contains a Solana lint in the form of a [Dylint](https://github.com/trailofbits/dylint) library.

The current lints are:

| Library                                                  | Description                                                                                        |
| -------------------------------------------------------- | -------------------------------------------------------------------------------------------------- |
| [`insecure_account_close`](lints/insecure_account_close) | lint for https://github.com/coral-xyz/sealevel-attacks/tree/master/programs/9-closing-accounts     |
| [`missing_owner_check`](lints/missing_owner_check)       | lint for https://github.com/coral-xyz/sealevel-attacks/tree/master/programs/2-owner-checks         |
| [`missing_signer_check`](lints/missing_signer_check)     | lint for https://github.com/coral-xyz/sealevel-attacks/tree/master/programs/0-signer-authorization |
| [`type_cosplay`](lints/type_cosplay)                     | lint for https://github.com/coral-xyz/sealevel-attacks/tree/master/programs/3-type-cosplay         |

Deprecated lints are listed [here](./DEPRECATED.md).

## Usage

To use these lints, do the following:

- Ensure `cargo-dylint` and `dylint-link` are installed:
  ```sh
  cargo install cargo-dylint dylint-link
  ```
- Ensure `ssh-agent` is running (because this is a private repo):
  ```sh
  eval `ssh-agent` && ssh-add # enter your SSH key passphrase
  ```
- Add the following to your target's `Cargo.toml` file:
  ```toml
  [workspace.metadata.dylint]
  libraries = [
      { git = "ssh://git@github.com/trailofbits/solana-lints", pattern = "lints/*" }
  ]
  ```
- In your target's directory, run Dylint:
  ```sh
  cd target-directory
  cargo dylint --all --workspace
  ```

## Notes

- Each library is in its own workspace so that it can have its own `rust-toolchain`.
- If you get an error like `Found multiple libraries...`, run `cargo dylint --list` and remove the old libraries.

## Writing lints tips

- We leverage `rustc_hir`, `rustc_lint`, `rustc_session`, `rustc_middle`, `rustc_span` Rust private crates to write rules; your IDE may not give you code completions for those but [there are workarounds for it](https://github.com/intellij-rust/intellij-rust/issues/946)
- The `LateLintPass` may contain one or more "visitors" for different AST (or different representation, HIR?) nodes; this can be e.g. `check_expr`, [`check_fn`, `check_item` etc.](https://github.com/rust-lang/rust-clippy/blob/487c2e8d4e543a025597f5727d99d77a72cfc7b6/clippy_lints/src/functions/mod.rs#L237-L266)
- [Use `rust_clippy/clippy_utils` sources (or docs) to find interesting methods](https://github.com/rust-lang/rust-clippy/blob/master/clippy_utils/src/lib.rs) and check out clippy lints to find how to do certain things
- A workspace metadata specification can be a path. This can be useful for developing lints:
  ```toml
  [workspace.metadata.dylint]
  libraries = [
      { path = "path-to-solana-lints", pattern = "lints/lint-under-development" }
  ]
  ```
