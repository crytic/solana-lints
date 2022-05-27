#![feature(bool_to_option)]
#![feature(rustc_private)]
#![recursion_limit = "256"]
#![warn(unused_extern_crates)]

dylint_linting::dylint_library!();

extern crate rustc_ast;
extern crate rustc_hir;
extern crate rustc_lint;
extern crate rustc_middle;
extern crate rustc_session;

mod insecure_account_close;

#[doc(hidden)]
#[no_mangle]
pub fn register_lints(_sess: &rustc_session::Session, lint_store: &mut rustc_lint::LintStore) {
    lint_store.register_lints(&[insecure_account_close::INSECURE_ACCOUNT_CLOSE]);
    lint_store.register_late_pass(|| Box::new(insecure_account_close::InsecureAccountClose));
}

#[test]
fn insecure() {
    dylint_testing::ui_test_example(env!("CARGO_PKG_NAME"), "insecure");
}

// smoelius: From what I can tell, the programs that `sealevel-attacks` calls `insecure-still` and
// `insecure-still-still` follow Solana's official guidance by zeroing-out the closed account's
// data. So the next two tests verify that no warnings are emitted.
//   See the following link for some discussion: https://github.com/project-serum/anchor/issues/613

#[test]
fn insecure_still() {
    dylint_testing::ui_test_example(env!("CARGO_PKG_NAME"), "insecure-still");
}

#[test]
fn insecure_still_still() {
    dylint_testing::ui_test_example(env!("CARGO_PKG_NAME"), "insecure-still-still");
}

#[test]
fn recommended() {
    dylint_testing::ui_test_example(env!("CARGO_PKG_NAME"), "recommended");
}

#[test]
fn secure() {
    dylint_testing::ui_test_example(env!("CARGO_PKG_NAME"), "secure");
}

#[test]
fn diff() -> std::io::Result<()> {
    use assert_cmd::prelude::*;
    use predicates::prelude::*;

    let tempfile = tempfile::tempdir_in(".")?;

    std::process::Command::new("git")
        .args([
            "clone",
            "https://github.com/project-serum/sealevel-attacks",
            &tempfile.path().to_string_lossy(),
        ])
        .assert()
        .success();

    std::process::Command::new("diff")
        .args(["-r", ".", "../../../ui"])
        .current_dir(tempfile.path().join("programs").join("9-closing-accounts"))
        .assert()
        .stdout(predicate::eq(
            r#"diff -r ./insecure/Cargo.toml ../../../ui/insecure/Cargo.toml
19c19
< anchor-lang = "0.20.1"
---
> anchor-lang = "0.24.2"
diff -r ./insecure/src/lib.rs ../../../ui/insecure/src/lib.rs
9c9
<     pub fn close(ctx: Context<Close>) -> ProgramResult {
---
>     pub fn close(ctx: Context<Close>) -> anchor_lang::solana_program::entrypoint::ProgramResult {
30a31,32
> 
> fn main() {}
Only in ../../../ui/insecure/src: lib.stderr
diff -r ./insecure-still/Cargo.toml ../../../ui/insecure-still/Cargo.toml
19c19
< anchor-lang = "0.20.1"
---
> anchor-lang = "0.24.2"
diff -r ./insecure-still/src/lib.rs ../../../ui/insecure-still/src/lib.rs
10c10
<     pub fn close(ctx: Context<Close>) -> ProgramResult {
---
>     pub fn close(ctx: Context<Close>) -> anchor_lang::solana_program::entrypoint::ProgramResult {
45a46,47
> 
> fn main() {}
Only in ../../../ui/insecure-still/src: lib.stderr
diff -r ./insecure-still-still/Cargo.toml ../../../ui/insecure-still-still/Cargo.toml
19c19
< anchor-lang = "0.20.1"
---
> anchor-lang = "0.24.2"
diff -r ./insecure-still-still/src/lib.rs ../../../ui/insecure-still-still/src/lib.rs
11c11
<     pub fn close(ctx: Context<Close>) -> ProgramResult {
---
>     pub fn close(ctx: Context<Close>) -> anchor_lang::solana_program::entrypoint::ProgramResult {
45a46,47
> 
> fn main() {}
Only in ../../../ui/insecure-still-still/src: lib.stderr
diff -r ./recommended/Cargo.toml ../../../ui/recommended/Cargo.toml
19c19
< anchor-lang = "0.20.1"
---
> anchor-lang = "0.24.2"
diff -r ./recommended/src/lib.rs ../../../ui/recommended/src/lib.rs
9c9
<     pub fn close(ctx: Context<Close>) -> ProgramResult {
---
>     pub fn close(ctx: Context<Close>) -> anchor_lang::solana_program::entrypoint::ProgramResult {
24a25,26
> 
> fn main() {}
Only in ../../../ui/recommended/src: lib.stderr
diff -r ./secure/src/lib.rs ../../../ui/secure/src/lib.rs
12c12
<     pub fn close(ctx: Context<Close>) -> ProgramResult {
---
>     pub fn close(ctx: Context<Close>) -> anchor_lang::solana_program::entrypoint::ProgramResult {
33c33,35
<     pub fn force_defund(ctx: Context<ForceDefund>) -> ProgramResult {
---
>     pub fn force_defund(
>         ctx: Context<ForceDefund>,
>     ) -> anchor_lang::solana_program::entrypoint::ProgramResult {
71a74,75
> 
> fn main() {}
Only in ../../../ui/secure/src: lib.stderr
"#,
        ));

    Ok(())
}
