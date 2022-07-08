#![feature(rustc_private)]
#![recursion_limit = "256"]
#![warn(unused_extern_crates)]

extern crate rustc_ast;
extern crate rustc_hir;
extern crate rustc_middle;

use clippy_utils::{diagnostics::span_lint, higher};
use if_chain::if_chain;
use rustc_ast::ast::{LitIntType, LitKind};
use rustc_hir::{BinOpKind, Body, BorrowKind, Expr, ExprKind, LangItem, Mutability, QPath, UnOp};
use rustc_lint::{LateContext, LateLintPass};
use rustc_middle::ty::{TyKind, UintTy};
use solana_lints::utils::visit_expr_no_bodies;

dylint_linting::declare_late_lint! {
    /// **What it does:** Checks for attempts to close an account by setting its lamports to 0 but
    /// not also clearing its data. See:
    /// https://docs.solana.com/developing/programming-model/transactions#multiple-instructions-in-a-single-transaction
    pub INSECURE_ACCOUNT_CLOSE,
    Warn,
    "attempt to close an account without also clearing its data"
}

impl<'tcx> LateLintPass<'tcx> for InsecureAccountClose {
    fn check_expr(&mut self, cx: &LateContext<'tcx>, expr: &'tcx Expr<'tcx>) {
        if_chain! {
            if is_account_close(expr);
            let body_owner_hir_id = cx.tcx.hir().enclosing_body_owner(expr.hir_id);
            let body_id = cx.tcx.hir().body_owned_by(body_owner_hir_id);
            let body = cx.tcx.hir().body(body_id);
            if !is_force_defund(cx, body);
            if !contains_manual_clear(body);
            then {
                span_lint(
                    cx,
                    INSECURE_ACCOUNT_CLOSE,
                    expr.span,
                    "attempt to close an account without also clearing its data",
                )
            }
        }
    }
}

fn is_account_close(expr: &Expr<'_>) -> bool {
    if_chain! {
        if let Some(place) = is_zero_assignment(expr);
        if let ExprKind::Unary(UnOp::Deref, inner) = place.kind;
        if let ExprKind::Unary(UnOp::Deref, inner_inner) = inner.kind;
        if let ExprKind::MethodCall(method_name, args, _) = inner_inner.kind;
        if method_name.ident.as_str() == "borrow_mut";
        if let [arg] = args;
        if let ExprKind::Field(_, field_name) = arg.kind;
        if field_name.as_str() == "lamports";
        then {
            true
        } else {
            false
        }
    }
}

// smoelius: If the body contains both an initial-eight-byte `copy_from_slice` and an
// eight-byte array comparison, then assume it belongs to a `force_defund` instruction:
// https://github.com/project-serum/sealevel-attacks/blob/609e5ade229eaa2b030589020e840c9407bda027/programs/9-closing-accounts/secure/src/lib.rs#L33
fn is_force_defund<'tcx>(cx: &LateContext<'tcx>, body: &'tcx Body<'tcx>) -> bool {
    contains_initial_eight_byte_copy_slice(body) && contains_eight_byte_array_comparison(cx, body)
}

fn contains_initial_eight_byte_copy_slice<'tcx>(body: &'tcx Body<'tcx>) -> bool {
    visit_expr_no_bodies(&body.value, |expr| {
        is_initial_eight_byte_copy_from_slice(expr).then_some(())
    })
    .is_some()
}

fn is_initial_eight_byte_copy_from_slice(expr: &Expr<'_>) -> bool {
    if_chain! {
        if let ExprKind::MethodCall(method_name, args, _) = expr.kind;
        if method_name.ident.as_str() == "copy_from_slice";
        if let [_, arg] = args;
        if let ExprKind::AddrOf(BorrowKind::Ref, Mutability::Not, inner) = arg.kind;
        if let ExprKind::Index(_, index) = inner.kind;
        if let ExprKind::Struct(qpath, fields, None) = index.kind;
        if matches!(qpath, QPath::LangItem(LangItem::Range, _, _));
        if let [start, end] = fields;
        if let ExprKind::Lit(ref start_lit) = start.expr.kind;
        if let LitKind::Int(0, LitIntType::Unsuffixed) = start_lit.node;
        if let ExprKind::Lit(ref end_lit) = end.expr.kind;
        if let LitKind::Int(8, LitIntType::Unsuffixed) = end_lit.node;
        then {
            true
        } else {
            false
        }
    }
}

fn contains_eight_byte_array_comparison<'tcx>(
    cx: &LateContext<'tcx>,
    body: &'tcx Body<'tcx>,
) -> bool {
    visit_expr_no_bodies(&body.value, |expr| {
        is_eight_byte_array_comparison(cx, expr).then_some(())
    })
    .is_some()
}

fn is_eight_byte_array_comparison<'tcx>(cx: &LateContext<'tcx>, expr: &Expr<'tcx>) -> bool {
    if_chain! {
        if let ExprKind::Binary(op, left, right) = expr.kind;
        if op.node == BinOpKind::Eq || op.node == BinOpKind::Ne;
        if is_eight_byte_array(cx, left) || is_eight_byte_array(cx, right);
        then {
            true
        } else {
            false
        }
    }
}

fn is_eight_byte_array<'tcx>(cx: &LateContext<'tcx>, expr: &Expr<'tcx>) -> bool {
    let ty = cx.typeck_results().expr_ty(expr);
    if_chain! {
        if let TyKind::Array(ty, length) = ty.kind();
        if *ty.kind() == TyKind::Uint(UintTy::U8);
        if let Some(length) = length.try_eval_usize(cx.tcx, cx.param_env);
        if length == 8;
        then {
            true
        } else {
            false
        }
    }
}

fn contains_manual_clear<'tcx>(body: &'tcx Body<'tcx>) -> bool {
    visit_expr_no_bodies(&body.value, |expr| is_manual_clear(expr).then_some(())).is_some()
}

fn is_manual_clear(expr: &Expr<'_>) -> bool {
    if_chain! {
        if let Some(higher::ForLoop { body, .. }) = higher::ForLoop::hir(expr);
        if contains_zero_assignment(body);
        then {
            true
        } else {
            false
        }
    }
}

fn contains_zero_assignment<'tcx>(expr: &'tcx Expr<'tcx>) -> bool {
    visit_expr_no_bodies(expr, is_zero_assignment).is_some()
}

fn is_zero_assignment<'tcx>(expr: &'tcx Expr<'tcx>) -> Option<&'tcx Expr<'tcx>> {
    if_chain! {
        if let ExprKind::Assign(place, value, _) = expr.kind;
        if let ExprKind::Lit(ref lit) = value.kind;
        if let LitKind::Int(0, LitIntType::Unsuffixed) = lit.node;
        then {
            Some(place)
        } else {
            None
        }
    }
}

#[test]
fn insecure() {
    dylint_testing::ui_test_example(env!("CARGO_PKG_NAME"), "insecure");
}

// smoelius: From what I can tell, the programs that `sealevel-attacks` calls `insecure-still` and
// `insecure-still-still` follow Solana's official guidance by zeroing-out the closed account's
// data. So the next two tests verify that no warnings are emitted.
//   See the following link for some discussion: https://github.com/project-serum/anchor/issues/613

#[test]
fn insecure_still() {
    dylint_testing::ui_test_example(env!("CARGO_PKG_NAME"), "insecure-still");
}

#[test]
fn insecure_still_still() {
    dylint_testing::ui_test_example(env!("CARGO_PKG_NAME"), "insecure-still-still");
}

#[test]
fn recommended() {
    dylint_testing::ui_test_example(env!("CARGO_PKG_NAME"), "recommended");
}

#[test]
fn secure() {
    dylint_testing::ui_test_example(env!("CARGO_PKG_NAME"), "secure");
}
