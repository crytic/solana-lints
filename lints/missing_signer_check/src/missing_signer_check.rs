use clippy_utils::{diagnostics::span_lint, ty::match_type};
use if_chain::if_chain;
use rustc_hir::{
    intravisit::{walk_expr, FnKind, Visitor},
    Body, Expr, ExprKind, FnDecl, HirId,
};
use rustc_lint::{LateContext, LateLintPass};
use rustc_session::{declare_lint, declare_lint_pass};
use rustc_span::Span;

declare_lint! {
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
    pub MISSING_SIGNER_CHECK,
    Warn,
    "description goes here"
}

declare_lint_pass!(MissingSignerCheck => [MISSING_SIGNER_CHECK]);

const ANCHOR_LANG_CONTEXT: [&str; 3] = ["anchor_lang", "context", "Context"];
const SOLANA_PROGRAM_ACCOUNT_INFO: [&str; 3] = ["solana_program", "account_info", "AccountInfo"];

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
        let local_def_id = cx.tcx.hir().local_def_id(hir_id);
        if_chain! {
            if matches!(fn_kind, FnKind::ItemFn(..));
            let fn_sig = cx.tcx.fn_sig(local_def_id.to_def_id()).skip_binder();
            if fn_sig
                .inputs()
                .iter()
                .any(|ty| match_type(cx, *ty, &ANCHOR_LANG_CONTEXT));
            if !contains_is_signer_use(cx, body);
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

fn contains_is_signer_use<'tcx>(cx: &LateContext<'tcx>, body: &'tcx Body<'tcx>) -> bool {
    visit_expr_no_bodies(&body.value, |expr| is_is_signer_use(cx, expr))
}

fn is_is_signer_use<'tcx>(cx: &LateContext<'tcx>, expr: &Expr<'tcx>) -> bool {
    if_chain! {
        if let ExprKind::Field(object, field_name) = expr.kind;
        if field_name.as_str() == "is_signer";
        let ty = cx.typeck_results().expr_ty(object);
        if match_type(cx, ty, &SOLANA_PROGRAM_ACCOUNT_INFO);
        then {
            true
        } else {
            false
        }
    }
}

trait Conclusive: Default {
    fn concluded(&self) -> bool;
}

impl<T> Conclusive for Option<T> {
    fn concluded(&self) -> bool {
        self.is_some()
    }
}

impl Conclusive for bool {
    fn concluded(&self) -> bool {
        *self
    }
}

fn visit_expr_no_bodies<'tcx, T>(expr: &'tcx Expr<'tcx>, f: impl FnMut(&'tcx Expr<'tcx>) -> T) -> T
where
    T: Conclusive,
{
    let mut v = V {
        f,
        result: T::default(),
    };
    v.visit_expr(expr);
    v.result
}

struct V<F, T> {
    f: F,
    result: T,
}

impl<'tcx, F, T> Visitor<'tcx> for V<F, T>
where
    F: FnMut(&'tcx Expr<'tcx>) -> T,
    T: Conclusive,
{
    fn visit_expr(&mut self, expr: &'tcx Expr<'tcx>) {
        if !self.result.concluded() {
            self.result = (self.f)(expr);

            if !self.result.concluded() {
                walk_expr(self, expr);
            }
        }
    }
}
