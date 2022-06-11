#![feature(rustc_private)]
#![warn(unused_extern_crates)]

extern crate rustc_hir;
extern crate rustc_span;

use clippy_utils::{diagnostics::span_lint, ty::match_type, SpanlessEq};
use if_chain::if_chain;
use rustc_hir::{intravisit::{FnKind, Visitor, walk_expr}, Body, Expr, ExprKind, FnDecl, HirId, def_id::DefId};
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
        // visitor collects accounts referenced in fnc body
        let accounts = get_referenced_accounts(cx, body);
        println!("{:#?}", accounts.len());
        for account_expr in accounts {
            if !contains_owner_use(cx, body, account_expr.hir_id) {
                span_lint(
                    cx,
                    INVALID_ACCOUNT_DATA,
                    span,
                    "this function doesn't use the owner field"
                )
                // return?? (if return, then we essentially short circuit)
            }
        }
    }
}

struct AccountUses<'cx, 'tcx> {
    cx: &'cx LateContext<'tcx>,
    uses: Vec<&'tcx Expr<'tcx>>,
}

fn get_referenced_accounts<'tcx>(cx: &LateContext<'tcx>, body: &'tcx Body<'tcx>) -> Vec<&'tcx Expr<'tcx>> {
    let mut accounts = AccountUses {
        cx,
        uses: Vec::new(),
    };

    // start the walk by visiting entire body block
    accounts.visit_expr(&body.value);
    accounts.uses
}

impl<'cx, 'tcx> Visitor<'tcx> for AccountUses<'cx, 'tcx> {
    fn visit_expr(&mut self, expr: &'tcx Expr<'tcx>) {
        let ty = self.cx.typeck_results().expr_ty(expr);
        if match_type(self.cx, ty, &paths::SOLANA_PROGRAM_ACCOUNT_INFO) {
            // TODO: may be a better place to put this struct
            let mut spanless_eq = SpanlessEq::new(self.cx);

            // TODO: check that what is being added to vector is as expected
            // if none of exprs are matching, then add to list
            if !self.uses.iter().any(|e| spanless_eq.eq_expr(e, expr)) {
                self.uses.push(expr);
            }
        }
        walk_expr(self, expr)
    }
}

fn contains_owner_use<'tcx>(
    cx: &LateContext<'tcx>, 
    body: &'tcx Body<'tcx>,
    hir_id: HirId
) -> bool {
    visit_expr_no_bodies(&body.value, |expr| uses_owner_field(cx, expr, hir_id))
}

/// Checks if the expression is an owner field reference on an object with hir_id
fn uses_owner_field<'tcx>(cx: &LateContext<'tcx>, expr: &Expr<'tcx>, hir_id: HirId) -> bool {
    if_chain! {
        if let ExprKind::Field(object, field_name) = expr.kind;
        // TODO: add check for key, is_signer
        if field_name.as_str() == "owner";
        if hir_id == expr.hir_id;
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

// #[test]
// fn recommended() {
//     dylint_testing::ui_test_example(env!("CARGO_PKG_NAME"), "recommended");
// }

#[test]
fn secure() {
    dylint_testing::ui_test_example(env!("CARGO_PKG_NAME"), "secure");
}
