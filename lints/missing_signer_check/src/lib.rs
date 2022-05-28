#![feature(rustc_private)]
#![warn(unused_extern_crates)]

extern crate rustc_hir;
extern crate rustc_span;

use clippy_utils::{diagnostics::span_lint, ty::match_type};
use if_chain::if_chain;
use rustc_hir::{intravisit::FnKind, Body, Expr, ExprKind, FnDecl, HirId};
use rustc_lint::{LateContext, LateLintPass};
use rustc_span::Span;
use solana_lints::{paths, utils::visit_expr_no_bodies};

dylint_linting::declare_late_lint! {
    /// **What it does:**
    ///
    /// **Why is this bad?**
    ///
    /// **Known problems:** None.
    ///
    /// **Example:**
    ///
    /// ```rust
    /// // example code where a warning is issued
    /// ```
    /// Use instead:
    /// ```rust
    /// // example code that does not raise a warning
    /// ```
    pub MISSING_SIGNER_CHECK,
    Warn,
    "description goes here"
}

impl<'tcx> LateLintPass<'tcx> for MissingSignerCheck {
    fn check_fn(
        &mut self,
        cx: &LateContext<'tcx>,
        fn_kind: FnKind<'tcx>,
        _: &'tcx FnDecl<'tcx>,
        body: &'tcx Body<'tcx>,
        span: Span,
        hir_id: HirId,
    ) {
        let local_def_id = cx.tcx.hir().local_def_id(hir_id);
        if_chain! {
            if matches!(fn_kind, FnKind::ItemFn(..));
            let fn_sig = cx.tcx.fn_sig(local_def_id.to_def_id()).skip_binder();
            if fn_sig
                .inputs()
                .iter()
                .any(|ty| match_type(cx, *ty, &paths::ANCHOR_LANG_CONTEXT));
            if !contains_is_signer_use(cx, body);
            then {
                span_lint(
                    cx,
                    MISSING_SIGNER_CHECK,
                    span,
                    "this function lacks a use of `is_signer`",
                )
            }
        }
    }
}

fn contains_is_signer_use<'tcx>(cx: &LateContext<'tcx>, body: &'tcx Body<'tcx>) -> bool {
    visit_expr_no_bodies(&body.value, |expr| is_is_signer_use(cx, expr))
}

fn is_is_signer_use<'tcx>(cx: &LateContext<'tcx>, expr: &Expr<'tcx>) -> bool {
    if_chain! {
        if let ExprKind::Field(object, field_name) = expr.kind;
        if field_name.as_str() == "is_signer";
        let ty = cx.typeck_results().expr_ty(object);
        if match_type(cx, ty, &paths::SOLANA_PROGRAM_ACCOUNT_INFO);
        then {
            true
        } else {
            false
        }
    }
}

trait Conclusive: Default {
    fn concluded(&self) -> bool;
}

impl<T> Conclusive for Option<T> {
    fn concluded(&self) -> bool {
        self.is_some()
    }
}

impl Conclusive for bool {
    fn concluded(&self) -> bool {
        *self
    }
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
9c9,11
<     pub fn log_message(ctx: Context<LogMessage>) -> ProgramResult {
---
>     pub fn log_message(
>         ctx: Context<LogMessage>,
>     ) -> anchor_lang::solana_program::entrypoint::ProgramResult {
18a21,22
> 
> fn main() {}
Only in ../../../ui/insecure/src: lib.stderr
diff -r ./recommended/src/lib.rs ../../../ui/recommended/src/lib.rs
9c9,11
<     pub fn log_message(ctx: Context<LogMessage>) -> ProgramResult {
---
>     pub fn log_message(
>         ctx: Context<LogMessage>,
>     ) -> anchor_lang::solana_program::entrypoint::ProgramResult {
18a21,22
> 
> fn main() {}
Only in ../../../ui/recommended/src: lib.stderr
diff -r ./secure/src/lib.rs ../../../ui/secure/src/lib.rs
9c9,11
<     pub fn log_message(ctx: Context<LogMessage>) -> ProgramResult {
---
>     pub fn log_message(
>         ctx: Context<LogMessage>,
>     ) -> anchor_lang::solana_program::entrypoint::ProgramResult {
21a24,25
> 
> fn main() {}
Only in ../../../ui/secure/src: lib.stderr
",
        ));

    Ok(())
}
