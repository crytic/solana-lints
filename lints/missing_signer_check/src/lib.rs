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
    ///
    /// Use instead:
    ///
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
        if_chain! {
            if matches!(fn_kind, FnKind::ItemFn(..));
            if body_uses_account_info(cx, body);
            if !context_contains_signer_field(cx, hir_id);
            if !body_contains_is_signer_use(cx, body);
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

fn body_uses_account_info<'tcx>(cx: &LateContext<'tcx>, body: &'tcx Body<'tcx>) -> bool {
    visit_expr_no_bodies(body.value, |expr| {
        let ty = cx.typeck_results().expr_ty(expr);
        match_type(cx, ty, &paths::SOLANA_PROGRAM_ACCOUNT_INFO)
    })
}

fn context_contains_signer_field(cx: &LateContext<'_>, hir_id: HirId) -> bool {
    let local_def_id = cx.tcx.hir().local_def_id(hir_id);
    let fn_sig = cx.tcx.fn_sig(local_def_id.to_def_id()).skip_binder();
    if_chain! {
        if let Some(ty) = fn_sig
            .inputs()
            .iter()
            .find(|ty| match_type(cx, **ty, &paths::ANCHOR_LANG_CONTEXT));
        if let ty::Adt(_, substs) = ty.kind();
        if substs.iter().any(|arg| arg_contains_signer_field(cx, arg));
        then {
            true
        } else {
            false
        }
    }
}

fn arg_contains_signer_field<'tcx>(cx: &LateContext<'tcx>, arg: GenericArg<'tcx>) -> bool {
    if_chain! {
        if let GenericArgKind::Type(ty) = arg.unpack();
        if let ty::Adt(adt_def, substs) = ty.kind();
        if let [variant] = adt_def.variants().iter().collect::<Vec<_>>().as_slice();
        if variant.fields.iter().any(|field_def| {
            match_type(cx, field_def.ty(cx.tcx, substs), &paths::ANCHOR_LANG_SIGNER)
        });
        then {
            true
        } else {
            false
        }
    }
}

fn body_contains_is_signer_use<'tcx>(cx: &LateContext<'tcx>, body: &'tcx Body<'tcx>) -> bool {
    visit_expr_no_bodies(body.value, |expr| is_is_signer_use(cx, expr))
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
