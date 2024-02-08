#![feature(rustc_private)]
#![warn(unused_extern_crates)]

extern crate rustc_hir;
extern crate rustc_middle;
extern crate rustc_span;

use clippy_utils::{diagnostics::span_lint, ty::match_type};
use if_chain::if_chain;
use rustc_hir::{def_id::LocalDefId, intravisit::FnKind, Body, Expr, ExprKind, FnDecl};
use rustc_lint::{LateContext, LateLintPass};
use rustc_middle::ty::{self, GenericArg, GenericArgKind};
use rustc_span::Span;
use solana_lints::{paths, utils::visit_expr_no_bodies};

dylint_linting::declare_late_lint! {
    /// **What it does:**
    ///
    /// This lint reports functions which use `AccountInfo` type and have zero signer checks.
    ///
    /// **Why is this bad?**
    ///
    /// The missing-signer-check vulnerability occurs when a program does not check that all the authorative
    /// accounts have signed the instruction. The issue is lack of proper access controls. Verifying signatures is a way to
    /// ensure the required entities has approved the operation. If a program does not check the signer field,
    /// then anyone can create the instruction, call the program and perform a privileged operation.
    ///
    /// For example if the Token program does not check that the owner of the tokens is a signer in the transfer instruction then anyone can
    /// transfer the tokens and steal them.
    ///
    /// **Known problems:**
    /// None.
    ///
    /// **Example:**
    ///
    /// See https://github.com/coral-xyz/sealevel-attacks/blob/master/programs/0-signer-authorization/insecure/src/lib.rs
    /// for an insecure example.
    ///
    /// Use instead:
    ///
    /// See https://github.com/coral-xyz/sealevel-attacks/blob/master/programs/0-signer-authorization/recommended/src/lib.rs for a secure example
    ///
    /// **How the lint is implemented:**
    ///
    /// - For each free function, function not associated with any type or trait.
    /// - If the function has an expression of type `AccountInfo` AND
    /// - If the function does **not** take a `Context<T>` type argument where `T` has a `Signer` type field AND
    /// - If the function does **not** has an expression `x.is_signer` where the expression `x` is of type `AccountInfo`.
    ///   - Report the function
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
        local_def_id: LocalDefId,
    ) {
        if_chain! {
            // fn is a free-standing function (parent is a `mod`). fn is not a method associated with a trait or type.
            if matches!(fn_kind, FnKind::ItemFn(..));
            // The function has an expression with AccountInfo type.
            if body_uses_account_info(cx, body);
            // The function does not take a Context<T> argument where T has a Signer type field.
            if !context_contains_signer_field(cx, local_def_id);
            // The function does not have an expression `x.is_signer` where `x` has AccountInfo type.
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

/// Return true if any of the expression in body has type `AccountInfo` (`solana_program::account_info::AccountInfo`)
fn body_uses_account_info<'tcx>(cx: &LateContext<'tcx>, body: &'tcx Body<'tcx>) -> bool {
    visit_expr_no_bodies(body.value, |expr| {
        let ty = cx.typeck_results().expr_ty(expr);
        match_type(cx, ty, &paths::SOLANA_PROGRAM_ACCOUNT_INFO)
    })
}

/// Given def id of a function return true if
/// - function takes a Context<T> type argument and
/// - T has only one variant (T is a struct) and
/// - T has a Signer type field
fn context_contains_signer_field(cx: &LateContext<'_>, local_def_id: LocalDefId) -> bool {
    let fn_sig = cx
        .tcx
        .fn_sig(local_def_id.to_def_id())
        .skip_binder()
        .skip_binder();
    if_chain! {
        // iterate over the arguments and find Context<> type argument
        if let Some(ty) = fn_sig
            .inputs()
            .iter()
            .find(|ty| match_type(cx, **ty, &paths::ANCHOR_LANG_CONTEXT));
        if let ty::Adt(_, substs) = ty.kind();
        // Context takes T as generic arg. iterate over the type arguments and
        // check any of them is a type arg and has `Signer` type field.
        if substs.iter().any(|arg| arg_contains_signer_field(cx, arg));
        then {
            true
        } else {
            false
        }
    }
}

/// Given a generic type argument, return true if its a struct that contains `Signer` type field.
fn arg_contains_signer_field<'tcx>(cx: &LateContext<'tcx>, arg: GenericArg<'tcx>) -> bool {
    if_chain! {
        // GenericArg is a type argument (not lifetime)
        if let GenericArgKind::Type(ty) = arg.unpack();
        if let ty::Adt(adt_def, substs) = ty.kind();
        if let [variant] = adt_def.variants().iter().collect::<Vec<_>>().as_slice();
        // iterate over the fields and check if any of the field's type is `Signer`
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

/// Return true if any of expressions in `body` are `x.is_signer` where `x`'s type is `AccountInfo`
fn body_contains_is_signer_use<'tcx>(cx: &LateContext<'tcx>, body: &'tcx Body<'tcx>) -> bool {
    visit_expr_no_bodies(body.value, |expr| is_is_signer_use(cx, expr))
}

/// Return true if the `expr` is `x.is_signer` where `x`'s type is `AccountInfo`.
fn is_is_signer_use<'tcx>(cx: &LateContext<'tcx>, expr: &Expr<'tcx>) -> bool {
    if_chain! {
        // `expr` is `x.{field_name}`
        if let ExprKind::Field(object, field_name) = expr.kind;
        if field_name.as_str() == "is_signer";
        // type of `x` is AccountInfo
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
