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
    Body, Expr, ExprKind, FnDecl, HirId, Mutability,
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
            let accounts = get_referenced_accounts(cx, body);

            accounts.values().for_each(|exprs| {
                // TODO: figure out handling of >2 accounts
                match exprs.len() {
                    2 => {
                        let first = exprs[0];
                        let second = exprs[1];
                        if !contains_key_call(cx, body, first) {
                            span_lint_and_help(
                                cx,
                                DUP_MUTABLE_ACCOUNTS_2,
                                first.span,
                                "this expression does not have a key check but has the same account type as another expression",
                                Some(second.span),
                                "add a key check to make sure the accounts have different keys, e.g., x.key() != y.key()",
                            );
                        }
                        if !contains_key_call(cx, body, second) {
                            span_lint_and_help(
                                cx,
                                DUP_MUTABLE_ACCOUNTS_2,
                                second.span,
                                "this expression does not have a key check but has the same account type as another expression",
                                Some(first.span),
                                "add a key check to make sure the accounts have different keys, e.g., x.key() != y.key()",
                            );
                        }
                    },
                    n if n > 2 => {
                        span_lint_and_note(
                            cx,
                            DUP_MUTABLE_ACCOUNTS_2,
                            exprs[0].span,
                            &format!("the following expression has the same account type as {} other accounts", exprs.len()),
                            None,
                            "might not check that each account has a unique key"
                        )
                    },
                    _ => {}
                }
            });
        }
    }
}

struct AccountUses<'cx, 'tcx> {
    cx: &'cx LateContext<'tcx>,
    uses: HashMap<DefId, Vec<&'tcx Expr<'tcx>>>,
}

fn get_referenced_accounts<'tcx>(
    cx: &LateContext<'tcx>,
    body: &'tcx Body<'tcx>,
) -> HashMap<DefId, Vec<&'tcx Expr<'tcx>>> {
    let mut accounts = AccountUses {
        cx,
        uses: HashMap::new(),
    };

    accounts.visit_expr(&body.value);
    accounts.uses
}

impl<'cx, 'tcx> Visitor<'tcx> for AccountUses<'cx, 'tcx> {
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
                if let Some(exprs) = self.uses.get_mut(&def_id) {
                    let mut spanless_eq = SpanlessEq::new(self.cx);
                    // check that expr is not a duplicate within its particular key-pair
                    if exprs.iter().all(|e| !spanless_eq.eq_expr(e, mut_expr)) {
                        exprs.push(mut_expr);
                    }
                } else {
                    self.uses.insert(def_id, vec![mut_expr]);
                }
            }
        }
        walk_expr(self, expr);
    }
}

/// Performs a walk on `body`, checking whether there exists an expression that contains
/// a `key()` method call on `account_expr`.
fn contains_key_call<'tcx>(
    cx: &LateContext<'tcx>,
    body: &'tcx Body<'tcx>,
    account_expr: &Expr<'tcx>,
) -> bool {
    visit_expr_no_bodies(&body.value, |expr| {
        if_chain! {
            if let ExprKind::MethodCall(path_seg, exprs, _span) = expr.kind;
            if path_seg.ident.name.as_str() == "key";
            if !exprs.is_empty();
            let mut spanless_eq = SpanlessEq::new(cx);
            if spanless_eq.eq_expr(&exprs[0], account_expr);
            then {
                true
            } else {
                false
            }
        }
    })
}

#[test]
fn insecure() {
    dylint_testing::ui_test_example(env!("CARGO_PKG_NAME"), "insecure");
}

#[test]
fn secure() {
    dylint_testing::ui_test_example(env!("CARGO_PKG_NAME"), "secure");
}
