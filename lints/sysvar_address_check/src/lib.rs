#![feature(rustc_private)]
#![warn(unused_extern_crates)]
#![recursion_limit = "256"]

extern crate rustc_hir;
extern crate rustc_middle;
extern crate rustc_span;

use rustc_lint::{LateContext, LateLintPass};
use rustc_hir::*;
use rustc_hir::def::Res;
use rustc_span::Span;
use rustc_hir::{intravisit::{Visitor, walk_expr, FnKind}};
use rustc_middle::ty::TyKind;

use clippy_utils::{match_def_path};
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
    span: Span,
    _: HirId
    ) {
        // 1. grab types
        // check for calls to bincode::deserialize and grab argument
        // grab the type and check it derives Sysvar
        // if so store the AccountInfo
        // 2. check for key checks
        // there is some check on AccountInfo.key == ID of Sysvar program
        let mut accounts = AccountUses {
            cx,
            uses: Vec::new(),
        };
        accounts.visit_expr(&body.value);
        
    }

    // fn check_block(&mut self, cx: &LateContext<'tcx>, block: &'tcx Block<'tcx>) {
    //     // if !block.span.from_expansion() {
    //     //     println!("{:#?}", block);
    //     // }
    //     for stmt in block.stmts {
    //         if_chain! {
    //             if !stmt.span.from_expansion();
    //             if let StmtKind::Local(local) = stmt.kind;
    //             if let Some(expr) = local.init;
    //             let _ = println!("{:#?}", expr);
    //             if is_deserialize_call(cx, expr);
    //             // if let Some(ty) = local.ty;
    //             then {
    //                 // derives_sysvar(ty);
    //                 // println!("{:#?}", local);
    //             }
    //         }

    //     }
    // }
}

struct AccountUses<'cx, 'tcx> {
    cx: &'cx LateContext<'tcx>,
    uses: Vec<&'tcx Expr<'tcx>>,
}

impl<'cx, 'tcx> Visitor<'tcx> for AccountUses<'cx, 'tcx> {
    fn visit_expr(&mut self, expr: &'tcx Expr<'tcx>) {
        if_chain! {
            // check if bincode::deserialize call
            if let ExprKind::Call(fnc_expr, args_expr) = expr.kind;
            if let ExprKind::Path(qpath) = &fnc_expr.kind;
            let res = self.cx.qpath_res(qpath, fnc_expr.hir_id);
            if let Res::Def(_, def_id) = res;
            if match_def_path(self.cx, def_id, &paths::BINCODE_DESERIALIZE);
            // check type of expr
            let ty = self.cx.typeck_results().expr_ty(expr);
            // assumes type is always Result type, which should be the case
            if let TyKind::Adt(_, substs) = ty.kind();
            if !substs.is_empty();
            let deser_type = substs[0];
            let _ = println!("{:#?}", deser_type);
            // // temp code for grabbing DefId of generic arg: Rent
            // if let QPath::Resolved(_, path) = qpath;
            // if let Some(generic_args) = path.segments[1].args;
            // if let GenericArg::Type(ty) = &generic_args.args[0];
            // if let TyKind::Path(qpath_sub) = &ty.kind;
            // let res_sub = self.cx.qpath_res(qpath_sub, ty.hir_id);
            // if let Res::Def(_, ty_id) = res_sub;
            then {
                println!("{:#?}", def_id);
                
            }
        }
        walk_expr(self, expr);
    }
}

fn is_deserialize_call<'tcx>(cx: &LateContext<'tcx>, expr: &'tcx Expr<'tcx>) -> bool {
    let mut accounts = AccountUses {
        cx,
        uses: Vec::new(),
    };

    accounts.visit_expr(expr);
    true
}

#[test]
fn insecure() {
    dylint_testing::ui_test_example(env!("CARGO_PKG_NAME"), "insecure");
}

#[test]
fn insecure_2() {
    dylint_testing::ui_test_example(env!("CARGO_PKG_NAME"), "insecure-2");
}
