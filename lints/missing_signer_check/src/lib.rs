#![feature(rustc_private)]
#![warn(unused_extern_crates)]

dylint_linting::dylint_library!();

extern crate rustc_hir;
extern crate rustc_lint;
extern crate rustc_session;
extern crate rustc_span;

mod missing_signer_check;

#[doc(hidden)]
#[no_mangle]
pub fn register_lints(_sess: &rustc_session::Session, lint_store: &mut rustc_lint::LintStore) {
    lint_store.register_lints(&[missing_signer_check::MISSING_SIGNER_CHECK]);
    lint_store.register_late_pass(|| Box::new(missing_signer_check::MissingSignerCheck));
}

#[test]
fn insecure() {
    dylint_testing::ui_test_example(env!("CARGO_PKG_NAME"), "insecure");
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
        .current_dir(
            tempfile
                .path()
                .join("programs")
                .join("0-signer-authorization"),
        )
        .assert()
        .stdout(predicate::eq(
            "\
diff -r ./insecure/src/lib.rs ../../../ui/insecure/src/lib.rs
18a19,20
> 
> fn main() {}
Only in ../../../ui/insecure/src: lib.stderr
diff -r ./recommended/src/lib.rs ../../../ui/recommended/src/lib.rs
18a19,20
> 
> fn main() {}
Only in ../../../ui/recommended/src: lib.stderr
diff -r ./secure/src/lib.rs ../../../ui/secure/src/lib.rs
21a22,23
> 
> fn main() {}
Only in ../../../ui/secure/src: lib.stderr
",
        ));

    Ok(())
}
