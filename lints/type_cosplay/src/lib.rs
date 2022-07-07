#![feature(rustc_private)]
#![warn(unused_extern_crates)]
#![recursion_limit = "256"]

extern crate rustc_hir;
extern crate rustc_index;
extern crate rustc_middle;
extern crate rustc_span;

use std::collections::HashMap;

use clippy_utils::{diagnostics::span_lint_and_help, match_def_path, ty::match_type};
use rustc_hir::{def::Res, Expr, ExprKind, QPath, TyKind, Mod, HirId, intravisit::{walk_item, walk_expr}};
use rustc_hir::*;
use rustc_span::{Span, def_id::DefId};
use rustc_index::vec::Idx;
use rustc_lint::{LateContext, LateLintPass};
use rustc_middle::ty::{AdtDef, TyKind as MiddleTyKind, Ty as MiddleTy, AdtKind};
use solana_lints::{paths, utils::visit_expr_no_bodies};
use rustc_hir::intravisit::Visitor;

use if_chain::if_chain;

dylint_linting::impl_late_lint! {
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
    "type is equivalent to another type",
    TypeCosplay::default()
}

#[derive(Default)]
struct TypeCosplay {
    deser_types: HashMap<AdtKind, Vec<(DefId, Span)>>,
}

// Returns the item if `map` contains a single key-value pair, and the value contains
// only a single element. If `map` contains multiple elements, return none.
fn contains_single_deserialized_type(map: &HashMap<AdtKind, Vec<(DefId, Span)>>) -> Option<(DefId, Span)> {
    match map.len() {
        1 => {
            // if there is only 1 k-v pair, then there will only be a single value
            let value = map.values().next().unwrap();
            match value.len() {
                1 => Some(value[0]),
                _ => None,
            }
        },
        _ => None,
    }
}

impl<'tcx> LateLintPass<'tcx> for TypeCosplay {
    fn check_expr(&mut self, cx: &LateContext<'tcx>, expr: &'tcx Expr<'tcx>) {
        if_chain! {
            if !expr.span.from_expansion();
            if let ExprKind::Call(fnc_expr, args_exprs) = expr.kind;
            if is_deserialize_function(cx, fnc_expr);
            // walk each argument expression and see if the data field is referenced
            if args_exprs
                .iter()
                .any(|arg| visit_expr_no_bodies(arg, |expr| contains_data_field_reference(cx, expr)));
            // get the type that the function was called on, ie X in X::deser()
            if let ExprKind::Path(qpath) = &fnc_expr.kind;
            if let QPath::TypeRelative(ty, _) = qpath;
            if let TyKind::Path(ty_qpath) = &ty.kind;
            let res = cx.typeck_results().qpath_res(ty_qpath, ty.hir_id);
            if let Res::Def(_, def_id) = res;
            then {
                // insert type into hashmap
                let middle_ty = cx.tcx.type_of(def_id);
                if let MiddleTyKind::Adt(adt_def, _) = middle_ty.kind() {
                    let adt_kind = adt_def.adt_kind();
                    let def_id = adt_def.did();
                    if let Some(vec) = self.deser_types.get_mut(&adt_kind) {
                        vec.push((def_id, ty.span));
                    } else {
                        self.deser_types.insert(adt_kind, vec![(def_id, ty.span)]);
                    }
                }
            }
        }
    }

    fn check_crate_post(&mut self, cx: &LateContext<'tcx>) {
        // map contains a single deserialized type
        if let Some((def_id, _)) = contains_single_deserialized_type(&self.deser_types) {
            let adt_def = cx.tcx.adt_def(def_id);
            if !adt_def.is_enum() {
                check_structs_have_discriminant(cx, &self.deser_types);
            }
        } else {
            // check if an enum type was deserialized
            if let Some(enums) = self.deser_types.get(&AdtKind::Enum) {
                if enums.len() == 1 {
                    check_structs_have_discriminant(cx, &self.deser_types);
                } else {
                    let first_span = enums[0].1;
                    let second_span = enums[1].1;
                    span_lint_and_help(
                        cx,
                        TYPE_COSPLAY,
                        first_span,
                        "warning: multiple enum types deserialized. Should only have one enum type to avoid possible equivalent types",
                        Some(second_span),
                        "help: consider constructing a single enum that contains all type definitions as variants"
                    )
                }
            } else {
                // no deserialized enum, but multiple deserialized structs. Check each struct for a discriminant
                check_structs_have_discriminant(cx, &self.deser_types);
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
            return true;
        } else {
            return false;
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

// TODO: could be a method
fn check_structs_have_discriminant(cx: &LateContext<'_>, deser_types: &HashMap<AdtKind, Vec<(DefId, Span)>>) {
    if let Some(types) = deser_types.get(&AdtKind::Struct) {
        let num_structs = types.len();
        types.iter().for_each(|t| has_discriminant(cx, &cx.tcx.adt_def(t.0), num_structs, t.1));
    }
}

/// Returns true if the `adt` has a field that is an enum and the number of variants of that enum is at least the number of deserialized struct types collected.
fn has_discriminant(cx: &LateContext, adt: &AdtDef, num_struct_types: usize, span: Span) {
    let variant = adt.variants().get(Idx::new(0)).unwrap();
    let has_discriminant = variant.fields.iter().any(|field| {
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
    });

    if !has_discriminant {
        span_lint_and_help(
            cx,
            TYPE_COSPLAY,
            span,
            "warning: type does not have a proper discriminant. It may be indistinguishable when deserialized",
            None,
            "help: add an enum with at least as many variants as there are struct definitions"
        )
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
fn insecure_two() {
    dylint_testing::ui_test_example(env!("CARGO_PKG_NAME"), "insecure-two");
}
