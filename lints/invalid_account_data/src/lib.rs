#![feature(rustc_private)]
#![warn(unused_extern_crates)]

extern crate rustc_hir;
extern crate rustc_middle;
extern crate rustc_span;

use clippy_utils::{diagnostics::span_lint, ty::match_type};
use if_chain::if_chain;
use rustc_hir::{intravisit::FnKind, Body, Expr, ExprKind, FnDecl, HirId};
use rustc_lint::{LateContext, LateLintPass};
use rustc_middle::ty::{
    self,
    subst::{GenericArg, GenericArgKind},
};
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
    pub INVALID_ACCOUNT_DATA,
    Warn,
    "description goes here"
}

impl<'tcx> LateLintPass<'tcx> for InvalidAccountData {
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

        // 1. ctx.accounts.token (here, the tokens acc is referenced)
        // 2. Check if token.owner is referenced elsewhere in body
        // 3. If not, emit lint
        let local_def_id = cx.tcx.hir().local_def_id(hir_id);
        
        if_chain! {
            if matches!(fn_kind, FnKind::ItemFn(..));
            let fn_sig = cx.tcx.fn_sig(local_def_id.to_def_id()).skip_binder();
            if let Some(ty) = fn_sig
                .inputs()
                .iter()
                .find(|ty| match_type(cx, **ty, &paths::ANCHOR_LANG_CONTEXT));
            // what are the substs?
            // Adt(struct Context<...>, ??)
            if let ty::Adt(_, substs) = ty.kind();
            

            //if !uses_owner_field(cx, body);
            then {
                span_lint(
                    cx,
                    INVALID_ACCOUNT_DATA,
                    span,
                    "this function doesn't use the owner field"
                )
            }
        }
        span_lint(
            cx,
            INVALID_ACCOUNT_DATA,
            span,
            "this function doesn't use the owner field"
        )
    }
}

fn uses_owner_field<'tcx>(cx: &LateContext<'tcx>, expr: &Expr<'tcx>) -> bool {
    if_chain! {
        if let ExprKind::Field(object, field_name) = expr.kind;
        if field_name.as_str() == "owner";
        // checking the type of the expression, which is an object
        let ty = cx.typeck_results().expr_ty(object);
        // check if ty == AccountInfo
        if match_type(cx, ty, &paths::SOLANA_PROGRAM_ACCOUNT_INFO);
        then {
            true
        } else {
            false
        }
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
