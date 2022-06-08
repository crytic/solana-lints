#![feature(rustc_private)]
#![warn(unused_extern_crates)]

extern crate rustc_hir;
extern crate rustc_span;

use clippy_utils::{diagnostics::span_lint, ty::match_type};
use if_chain::if_chain;
use rustc_hir::{intravisit::{FnKind, Visitor, walk_expr}, Body, Expr, ExprKind, FnDecl, HirId, def_id::DefId};
use rustc_lint::{LateContext, LateLintPass};
use rustc_span::Span;
use solana_lints::{paths, utils::visit_expr_no_bodies};
use std::collections::HashSet;

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
        println!("{:#?}", accounts);
        for account_id in accounts {
            if !contains_owner_use(cx, body, account_id) {
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
    uses: HashSet<DefId>,
}

fn get_referenced_accounts<'tcx>(cx: &LateContext<'tcx>, body: &'tcx Body<'tcx>) -> HashSet<DefId> {
    let mut accounts = AccountUses {
        cx,
        uses: HashSet::new(),
    };

    // start the walk by visiting entire body block
    accounts.visit_expr(&body.value);
    accounts.uses
}

impl<'cx, 'tcx> Visitor<'tcx> for AccountUses<'cx, 'tcx> {
    // TODO: check if collects unique ids or not. Ex. ctx.accounts.token is referenced 2x in body
    // make Vec a HashSet
    fn visit_expr(&mut self, expr: &'tcx Expr<'tcx>) {
        let ty = self.cx.typeck_results().expr_ty(expr);
        if match_type(self.cx, ty, &paths::SOLANA_PROGRAM_ACCOUNT_INFO) {
            if let Some(def_id) = self.cx.typeck_results().type_dependent_def_id(expr.hir_id) {
                //println!("{:#?}", def_id);
                self.uses.insert(def_id);
            }
        }
        walk_expr(self, expr)
    }
}

fn contains_owner_use<'tcx>(
    cx: &LateContext<'tcx>, 
    body: &'tcx Body<'tcx>,
    local_def_id: DefId
) -> bool {
    visit_expr_no_bodies(&body.value, |expr| uses_owner_field(cx, expr, local_def_id))
}

/// Checks if the expression is an owner field reference on an object with local_def_id
fn uses_owner_field<'tcx>(cx: &LateContext<'tcx>, expr: &Expr<'tcx>, local_def_id: DefId) -> bool {
    if_chain! {
        if let ExprKind::Field(object, field_name) = expr.kind;
        // TODO: add check for key, is_signer
        if field_name.as_str() == "owner";
        if let Some(obj_def_id) = cx.typeck_results().type_dependent_def_id(object.hir_id);
        if obj_def_id == local_def_id;
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

// #[test]
// fn secure() {
//     dylint_testing::ui_test_example(env!("CARGO_PKG_NAME"), "secure");
// }
