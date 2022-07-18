#![feature(rustc_private)]
#![warn(unused_extern_crates)]

extern crate rustc_hir;
extern crate rustc_middle;
extern crate rustc_span;

use clippy_utils::{
    diagnostics::{span_lint_and_help, span_lint_and_note},
    ty::match_type,
    SpanlessEq,
};
use if_chain::if_chain;
use rustc_hir::{
    def_id::DefId,
    intravisit::{walk_expr, FnKind, Visitor},
    BinOpKind, Body, Expr, ExprKind, FnDecl, HirId, Mutability,
};
use rustc_lint::{LateContext, LateLintPass};
use rustc_middle::ty::TyKind;
use rustc_span::Span;
use solana_lints::{paths, utils::visit_expr_no_bodies};

use std::collections::HashMap;

const ANCHOR_ACCOUNT_GENERIC_ARG_COUNT: usize = 2;

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
    pub DUP_MUTABLE_ACCOUNTS_2,
    Warn,
    "description goes here"
}

impl<'tcx> LateLintPass<'tcx> for DupMutableAccounts2 {
    fn check_fn(
        &mut self,
        cx: &LateContext<'tcx>,
        _: FnKind<'tcx>,
        _: &'tcx FnDecl<'tcx>,
        body: &'tcx Body<'tcx>,
        span: Span,
        _: HirId,
    ) {
        if !span.from_expansion() {
            // get all mutable references to Accounts and if_statements in body
            let mut values = Values::new(cx);
            values.get_referenced_accounts_and_if_statements(cx, body);
            // println!("{:#?}", values.if_statements);

            values.accounts.values().for_each(|exprs| {
                if exprs.len() > 1 {
                    for current in 0..exprs.len() - 1 {
                        for next in current + 1..exprs.len() {
                            if !values.check_key_constraint(exprs[current], exprs[next]) {
                                span_lint_and_help(
                                    cx,
                                    DUP_MUTABLE_ACCOUNTS_2,
                                    exprs[current].span,
                                    "the following expressions have equivalent Account types, yet do not contain a proper key check.",
                                    Some(exprs[next].span),
                                    "add a key check to make sure the accounts have different keys, e.g., x.key() != y.key()",
                                );
                            }
                        }
                    }
                }
            });
        }
    }
}

struct Values<'cx, 'tcx> {
    cx: &'cx LateContext<'tcx>,
    accounts: HashMap<DefId, Vec<&'tcx Expr<'tcx>>>,
    if_statements: Vec<(&'tcx Expr<'tcx>, &'tcx Expr<'tcx>)>,
}

impl<'cx, 'tcx> Values<'cx, 'tcx> {
    fn new(cx: &'cx LateContext<'tcx>) -> Self {
        Values {
            cx,
            accounts: HashMap::new(),
            if_statements: Vec::new(),
        }
    }

    fn get_referenced_accounts_and_if_statements(
        &mut self,
        cx: &'cx LateContext<'tcx>,
        body: &'tcx Body<'tcx>,
    ) -> &Self {
        self.visit_expr(&body.value);
        self
    }

    /// Checks if there is a valid key constraint for `first_account` and `second_account`.
    /// NOTE: currently only considers `first.key() == second.key()` or the symmetric relation as valid constraints.
    /// TODO: if == relation used, should return some error in the THEN block
    fn check_key_constraint(&self, first_account: &Expr<'_>, second_account: &Expr<'_>) -> bool {
        for (left, right) in &self.if_statements {
            if_chain! {
                if let ExprKind::MethodCall(path_seg_left, exprs_left, _span) = left.kind;
                if let ExprKind::MethodCall(path_seg_right, exprs_right, _span) = right.kind;
                if path_seg_left.ident.name.as_str() == "key" && path_seg_right.ident.name.as_str() == "key";
                if !exprs_left.is_empty() && !exprs_right.is_empty();
                let mut spanless_eq = SpanlessEq::new(self.cx);
                if (spanless_eq.eq_expr(&exprs_left[0], first_account) && spanless_eq.eq_expr(&exprs_right[0], second_account)) 
                || (spanless_eq.eq_expr(&exprs_left[0], second_account) && spanless_eq.eq_expr(&exprs_right[0], first_account));
                then {
                    return true;
                }
            }
        }
        return false;
    }
}

impl<'cx, 'tcx> Visitor<'tcx> for Values<'cx, 'tcx> {
    fn visit_expr(&mut self, expr: &'tcx Expr<'tcx>) {
        if_chain! {
            // get mutable reference expressions
            if let ExprKind::AddrOf(_, mutability, mut_expr) = expr.kind;
            if let Mutability::Mut = mutability;
            // check type of expr == Account<'info, T>
            let middle_ty = self.cx.typeck_results().expr_ty(mut_expr);
            if match_type(self.cx, middle_ty, &paths::ANCHOR_ACCOUNT);
            // grab T generic parameter
            if let TyKind::Adt(_adt_def, substs) = middle_ty.kind();
            if substs.len() == ANCHOR_ACCOUNT_GENERIC_ARG_COUNT;
            let account_type = substs[1].expect_ty(); // TODO: could just store middle::Ty instead of DefId?
            if let Some(adt_def) = account_type.ty_adt_def();
            then {
                let def_id = adt_def.did();
                if let Some(exprs) = self.accounts.get_mut(&def_id) {
                    let mut spanless_eq = SpanlessEq::new(self.cx);
                    // check that expr is not a duplicate within its particular key-pair
                    if exprs.iter().all(|e| !spanless_eq.eq_expr(e, mut_expr)) {
                        exprs.push(mut_expr);
                    }
                } else {
                    self.accounts.insert(def_id, vec![mut_expr]);
                }
            }
        }

        // get if statements
        if_chain! {
            if let ExprKind::If(wrapped_if_expr, then, _else_opt) = expr.kind;
            if let ExprKind::DropTemps(if_expr) = wrapped_if_expr.kind;
            if let ExprKind::Binary(op, left, right) = if_expr.kind;
            // TODO: leaves out || or &&. Could implement something that pulls apart
            // an if expr that is of this form into individual == or != comparisons
            if let BinOpKind::Ne | BinOpKind::Eq = op.node;
            then {
                // println!("{:#?}, {:#?}", expr, then);
                self.if_statements.push((left, right));
            }
        }
        walk_expr(self, expr);
    }
}

// /// Performs a walk on `body`, checking whether there exists an expression that contains
// /// a `key()` method call on `account_expr`.
// fn contains_key_call<'tcx>(
//     cx: &LateContext<'tcx>,
//     body: &'tcx Body<'tcx>,
//     account_expr: &Expr<'tcx>,
// ) -> bool {
//     visit_expr_no_bodies(&body.value, |expr| {
//         if_chain! {
//             if let ExprKind::MethodCall(path_seg, exprs, _span) = expr.kind;
//             if path_seg.ident.name.as_str() == "key";
//             if !exprs.is_empty();
//             let mut spanless_eq = SpanlessEq::new(cx);
//             if spanless_eq.eq_expr(&exprs[0], account_expr);
//             then {
//                 true
//             } else {
//                 false
//             }
//         }
//     })
// }

#[test]
fn insecure() {
    dylint_testing::ui_test_example(env!("CARGO_PKG_NAME"), "insecure");
}

#[test]
fn secure() {
    dylint_testing::ui_test_example(env!("CARGO_PKG_NAME"), "secure");
}
