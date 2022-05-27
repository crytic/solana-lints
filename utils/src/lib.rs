#![feature(rustc_private)]
#![warn(unused_extern_crates)]

extern crate rustc_hir;

use rustc_hir::{
    intravisit::{walk_expr, Visitor},
    Expr,
};

pub const ANCHOR_LANG_CONTEXT: [&str; 3] = ["anchor_lang", "context", "Context"];
pub const SOLANA_PROGRAM_ACCOUNT_INFO: [&str; 3] =
    ["solana_program", "account_info", "AccountInfo"];

pub trait Conclusive: Default {
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

pub fn visit_expr_no_bodies<'tcx, T>(
    expr: &'tcx Expr<'tcx>,
    f: impl FnMut(&'tcx Expr<'tcx>) -> T,
) -> T
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
