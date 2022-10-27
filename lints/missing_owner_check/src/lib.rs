#![feature(rustc_private)]
#![warn(unused_extern_crates)]

extern crate rustc_hir;
extern crate rustc_span;

use clippy_utils::{diagnostics::span_lint, ty::match_type, SpanlessEq};
use if_chain::if_chain;
use rustc_hir::{
    intravisit::{walk_expr, FnKind, Visitor},
    Body, Expr, ExprKind, FnDecl, HirId,
};
use rustc_lint::{LateContext, LateLintPass};
use rustc_span::Span;
use solana_lints::{paths, utils::visit_expr_no_bodies};

dylint_linting::declare_late_lint! {
    /// **What it does:**
    ///
    /// This lint checks that for each account referenced in a program, that there is a
    /// corresponding owner check on that account. Specifically, this means that the owner
    /// field is referenced on that account.
    ///
    /// **Why is this bad?**
    ///
    /// The missing-owner-check vulnerability occurs when a program uses an account, but does
    /// not check that it is owned by the expected program. This could lead to vulnerabilities
    /// where a malicious actor passes in an account owned by program `X` when what was expected
    /// was an account owned by program `Y`. The code may then perform unexpected operations
    /// on that spoofed account.
    ///
    /// For example, suppose a program expected an account to be owned by the SPL Token program.
    /// If no owner check is done on the account, then a malicious actor could pass in an
    /// account owned by some other program. The code may then perform some actions on the
    /// unauthorized account that is not owned by the SPL Token program.
    ///
    /// **Known problems:**
    ///
    /// Key checks can be strengthened. Currently, the lint only checks that the account's owner
    /// field is referenced somewhere, ie, `AccountInfo.owner`.
    ///
    /// **Example:**
    ///
    /// See https://github.com/coral-xyz/sealevel-attacks/blob/master/programs/2-owner-checks/insecure/src/lib.rs
    /// for an insecure example.
    ///
    /// Use instead:
    ///
    /// See https://github.com/coral-xyz/sealevel-attacks/blob/master/programs/2-owner-checks/secure/src/lib.rs
    /// for a secure example.
    pub MISSING_OWNER_CHECK,
    Warn,
    "using an account without checking if its owner is as expected"
}

impl<'tcx> LateLintPass<'tcx> for MissingOwnerCheck {
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
            // println!("{:?}\n {:#?}", span, accounts.len());

            for account_expr in accounts {
                if !contains_owner_use(cx, body, account_expr) {
                    span_lint(
                        cx,
                        MISSING_OWNER_CHECK,
                        account_expr.span,
                        "this Account struct is used but there is no check on its owner field",
                    );
                }
            }
        }
    }
}

struct AccountUses<'cx, 'tcx> {
    cx: &'cx LateContext<'tcx>,
    uses: Vec<&'tcx Expr<'tcx>>,
}

fn get_referenced_accounts<'tcx>(
    cx: &LateContext<'tcx>,
    body: &'tcx Body<'tcx>,
) -> Vec<&'tcx Expr<'tcx>> {
    let mut accounts = AccountUses {
        cx,
        uses: Vec::new(),
    };

    accounts.visit_expr(body.value);
    accounts.uses
}

impl<'cx, 'tcx> Visitor<'tcx> for AccountUses<'cx, 'tcx> {
    fn visit_expr(&mut self, expr: &'tcx Expr<'tcx>) {
        if_chain! {
            let ty = self.cx.typeck_results().expr_ty(expr);
            if match_type(self.cx, ty, &paths::SOLANA_PROGRAM_ACCOUNT_INFO);
            let mut spanless_eq = SpanlessEq::new(self.cx);
            if !self.uses.iter().any(|e| spanless_eq.eq_expr(e, expr));
            then {
                // println!("Expression pushed: {:?}", expr);
                self.uses.push(expr);
            }
        }
        walk_expr(self, expr);
    }
}

fn contains_owner_use<'tcx>(
    cx: &LateContext<'tcx>,
    body: &'tcx Body<'tcx>,
    account_expr: &Expr<'tcx>,
) -> bool {
    visit_expr_no_bodies(body.value, |expr| uses_owner_field(cx, expr, account_expr))
}

/// Checks if `expr` is an owner field reference on `account_expr`
fn uses_owner_field<'tcx>(
    cx: &LateContext<'tcx>,
    expr: &Expr<'tcx>,
    account_expr: &Expr<'tcx>,
) -> bool {
    if_chain! {
        if let ExprKind::Field(object, field_name) = expr.kind;
        // TODO: add check for key, is_signer
        if field_name.as_str() == "owner";
        let mut spanless_eq = SpanlessEq::new(cx);
        if spanless_eq.eq_expr(account_expr, object);
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

#[test]
fn secure_fixed() {
    dylint_testing::ui_test_example(env!("CARGO_PKG_NAME"), "secure-fixed");
}
