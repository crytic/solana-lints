#![feature(rustc_private)]
#![warn(unused_extern_crates)]

extern crate rustc_ast;
extern crate rustc_ast_pretty;
extern crate rustc_data_structures;
extern crate rustc_errors;
extern crate rustc_hir;
extern crate rustc_hir_pretty;
extern crate rustc_index;
extern crate rustc_infer;
extern crate rustc_lexer;
extern crate rustc_middle;
extern crate rustc_mir_dataflow;
extern crate rustc_parse;
extern crate rustc_parse_format;
extern crate rustc_span;
extern crate rustc_target;
extern crate rustc_trait_selection;
extern crate rustc_typeck;

use rustc_lint::{LateLintPass, LateContext};
use clippy_utils::{diagnostics::span_lint, higher};
use if_chain::if_chain;
use rustc_ast::ast::{LitIntType, LitKind};
use rustc_middle::{
    mir::interpret::ConstValue,
    ty::{ConstKind, TyKind, UintTy},
};
use rustc_hir::{intravisit::FnKind, Body, Expr, ExprKind, FnDecl, HirId};
use rustc_span::Span;

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
    pub INVALID_ACCOUNT_DATA,
    Warn,
    "description goes here"
}

impl<'tcx> LateLintPass<'tcx> for InvalidAccountData {
    // A list of things you might check can be found here:
    // https://doc.rust-lang.org/stable/nightly-rustc/rustc_lint/trait.LateLintPass.html
    fn check_fn(
        &mut self,
        cx: &LateContext<'tcx>,
        fn_kind: FnKind<'tcx>,
        _: &'tcx FnDecl<'tcx>,
        body: &'tcx Body<'tcx>,
        span: Span,
        hir_id: HirId,
    ) {
        // check which accounts are referenced (used) by the function
        // call this set of accounts s.
        // For each account in s, check if the owner field is referenced somewhere in the function
        
        let params = body.params;
    }
}

#[test]
fn ui() {
    dylint_testing::ui_test(
        env!("CARGO_PKG_NAME"),
        &std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("ui"),
    );
}

/*#[test]
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
}*/
