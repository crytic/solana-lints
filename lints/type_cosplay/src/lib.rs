#![feature(rustc_private)]
#![warn(unused_extern_crates)]
#![recursion_limit = "256"]

extern crate rustc_hir;
extern crate rustc_index;
extern crate rustc_middle;
extern crate rustc_span;
extern crate rustc_typeck;

use std::fmt::{Debug, Formatter, Result};

use clippy_utils::{diagnostics::span_lint_and_note, match_def_path, ty::match_type, SpanlessEq};
use rustc_hir::{def::Res, Expr, ExprKind, HirId, Item, ItemKind, Mod, Path, QPath, Ty, TyKind};
use rustc_index::vec::Idx;
use rustc_lint::{LateContext, LateLintPass};
use rustc_middle::ty::subst::GenericArg;
use rustc_middle::ty::List;
use rustc_middle::ty::{AdtDef, FieldDef, Ty as MiddleTy, VariantDef, TyKind as MiddleTyKind};
use rustc_span::Symbol;
use solana_lints::{paths, utils::visit_expr_no_bodies};

use if_chain::if_chain;
use rustc_span::Span;

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
    pub TYPE_COSPLAY,
    Warn,
    "type is equivalent to another type"
}

impl<'tcx> LateLintPass<'tcx> for TypeCosplay {
    fn check_expr(&mut self, cx: &LateContext<'tcx>, expr: &'tcx Expr<'tcx>) {
        let mut enum_type = None;
        let mut num_struct_types = 0;
        if_chain! {
            if !expr.span.from_expansion();
            if let ExprKind::Call(fnc_expr, args_exprs) = expr.kind;
            if is_deserialize_function(cx, fnc_expr);
            // walk each argument expression and see if the data field is referenced
            if args_exprs.iter().any(|arg| visit_expr_no_bodies(arg, |expr| contains_data_field_reference(cx, expr)));
            // Checking the type that the deser function was called on is an enum
            if let ExprKind::Path(qpath) = &fnc_expr.kind;
            if let QPath::TypeRelative(ty, _) = qpath;
            if let TyKind::Path(ty_qpath) = &ty.kind;
            let res = cx.typeck_results().qpath_res(ty_qpath, ty.hir_id);
            if let Res::Def(_, def_id) = res;
            let middle_ty = cx.tcx.type_of(def_id);
            then {
                if middle_ty.is_enum() {
                    enum_type = match enum_type {
                        Some(t) => {
                            if t != middle_ty {
                                // span_lint -- warning, multiple enum types detected. Should only have 1 enum
                                // type to avoid possible equivalent types
                            }
                            Some(t)
                        }
                        None => Some(middle_ty)
                    }
                } else {
                    if_chain! {
                        if let MiddleTyKind::Adt(adt_def, _) = middle_ty.kind();
                        // num_struct_types += 1;
                        if !has_discriminant(cx, &adt_def, num_struct_types);
                        then {
                            // span_lint
                        }
                    }
                }
            }
        }
    }
}

fn is_deserialize_function(cx: &LateContext<'_>, expr: &Expr<'_>) -> bool {
    if_chain! {
        if let Some(def_id) = cx.typeck_results().type_dependent_def_id(expr.hir_id);
        // for now, only testing borsh deserialize function
        if match_def_path(cx, def_id, &paths::BORSH_TRY_FROM_SLICE);
        then {
            true
        } else {
            false
        }
    }
}

fn contains_data_field_reference(cx: &LateContext<'_>, expr: &Expr<'_>) -> bool {
    if_chain! {
        if let ExprKind::Field(obj_expr, ident) = expr.kind;
        if ident.as_str() == "data";
        let ty = cx.typeck_results().expr_ty(obj_expr);
        if match_type(cx, ty, &paths::SOLANA_PROGRAM_ACCOUNT_INFO);
        then {
            true
        } else {
            false
        }
    }
}

fn has_discriminant(cx: &LateContext, adt: &AdtDef, num_struct_types: usize) -> bool {
    // check if it is an enum with #variants >= #equivalent struct types in code
    let variant = adt.variants().get(Idx::new(0)).unwrap();

	variant
		.fields
		.iter()
		.any(|field| {
			let ty = cx.tcx.type_of(field.did);
			if_chain! {
				if let MiddleTyKind::Adt(adt_def, _) = ty.kind();
				if adt_def.is_enum();
				if adt_def.variants().len() >= num_struct_types;
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
fn recommended() {
    dylint_testing::ui_test_example(env!("CARGO_PKG_NAME"), "recommended");
}

#[test]
fn secure() {
    dylint_testing::ui_test_example(env!("CARGO_PKG_NAME"), "secure");
}

#[test]
fn types() {
    dylint_testing::ui_test_example(env!("CARGO_PKG_NAME"), "types");
}
