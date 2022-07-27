#![feature(rustc_private)]
#![warn(unused_extern_crates)]

extern crate rustc_hir;
extern crate rustc_middle;
extern crate rustc_span;

use rustc_hir::def::Res;
use rustc_hir::intravisit::{walk_expr, FnKind, Visitor};
use rustc_hir::{Body, Expr, ExprKind, FnDecl, HirId};
use rustc_lint::{LateContext, LateLintPass};
use rustc_middle::ty::TyKind;
use rustc_span::Span;

use clippy_utils::{
    diagnostics::span_lint_and_help, get_trait_def_id, match_def_path, ty::implements_trait,
};
use if_chain::if_chain;
mod paths;

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
    pub SYSVAR_ADDRESS_CHECK,
    Warn,
    "description goes here"
}

impl<'tcx> LateLintPass<'tcx> for SysvarAddressCheck {
    fn check_fn(
        &mut self,
        cx: &LateContext<'tcx>,
        _: FnKind<'tcx>,
        _: &'tcx FnDecl<'tcx>,
        body: &'tcx Body<'tcx>,
        _: Span,
        _: HirId,
    ) {
        // 1. grab types
        // check for calls to bincode::deserialize and grab argument
        // grab the type and check it derives Sysvar
        // if so store the AccountInfo
        // 2. check for key checks
        // there is some check on AccountInfo.key == ID of Sysvar program

        // 1. walk function body and search for calls to bincode::deserialize
        // 2. retrieve the type of this expression (which is what is being deserialized to),
        // and check that the type implements the Sysvar trait
        // 3. if so, flag the lint and issue warning that user should not deserialize directly,
        // but instead use from_account_info() method from Sysvar trait
        let mut accounts = AccountUses { cx };
        accounts.visit_expr(&body.value);
    }
}

struct AccountUses<'cx, 'tcx> {
    cx: &'cx LateContext<'tcx>,
}

impl<'cx, 'tcx> Visitor<'tcx> for AccountUses<'cx, 'tcx> {
    fn visit_expr(&mut self, expr: &'tcx Expr<'tcx>) {
        if_chain! {
            // check if bincode::deserialize call
            if let ExprKind::Call(fnc_expr, _args_expr) = expr.kind;
            if let ExprKind::Path(qpath) = &fnc_expr.kind;
            let res = self.cx.qpath_res(qpath, fnc_expr.hir_id);
            if let Res::Def(_, def_id) = res;
            if match_def_path(self.cx, def_id, &paths::BINCODE_DESERIALIZE);
            // check type of expr
            let ty = self.cx.typeck_results().expr_ty(expr);
            // assumes type is always Result type, which should be the case
            if let TyKind::Adt(_, substs) = ty.kind();
            if !substs.is_empty();
            let deser_type = substs[0].expect_ty();
            // check type implements Sysvar
            if let Some(trait_id) = get_trait_def_id(self.cx, &paths::ANCHOR_SYSVAR_TRAIT);
            if implements_trait(self.cx, deser_type, trait_id, &[]);
            then {
                // grab AccountInfo
                span_lint_and_help(
                    self.cx,
                    SYSVAR_ADDRESS_CHECK,
                    expr.span,
                    "raw deserialization of a type that implements Sysvar",
                    None,
                    "use from_account_info() instead",
                );
            }
        }
        walk_expr(self, expr);
    }
}

// Not checking sealevel insecure case because in its current form, it is technically not even
// insecure. It does not deserialize from `rent.data`, thus possibly incorrectly assuming that
// this is a Rent struct. It is insecure in the sense there is no key check.
// #[test]
// fn insecure() {
//     dylint_testing::ui_test_example(env!("CARGO_PKG_NAME"), "insecure");
// }

// Not testing sealevel secure case because this lint will flag any attempt to do a "raw"
// deserialization. The canonical way should be using from_account_info().

#[test]
fn insecure_2() {
    dylint_testing::ui_test_example(env!("CARGO_PKG_NAME"), "insecure-2");
}

#[test]
fn secure_2() {
    dylint_testing::ui_test_example(env!("CARGO_PKG_NAME"), "secure-2");
}
