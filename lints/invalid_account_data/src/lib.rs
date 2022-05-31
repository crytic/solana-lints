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

        // BASIC STRATEGY
        // 1) something with checking function sig, identifying accounts used by fnc
        // 2) visiting each expr in the fnc body to see if owner is referenced and it is a field of an Account
        
        if_chain! {
          if matches!(fn_kind, FnKind::ItemFn(..));

          if !uses_owner_field(cx, body);
          then {
              span_lint(
                  cx,
                  INVALID_ACCOUNT_DATA,
                  span,
                  "this function doesn't use the owner field"
              )
            }
        }
    }
}

fn uses_owner_field<'tcx>(cx: &LateContext<'tcx>, expr: &Expr<'tcx>) -> bool {
    if_chain! {
        if let ExprKind::Field(object, field_name) = expr.kind;
        if field_name.as_str() == "owner";
        // checking the type of the expression, which is an object
        let ty = cx.typeck_results().expr_ty(object);
        // check if ty == AccountInfo
        if match_type(cx, ty, &SOLANA_PROGRAM_ACCOUNT_INFO);
        then {
            true
        } else {
            false
        }
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
