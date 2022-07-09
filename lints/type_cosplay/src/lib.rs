#![feature(rustc_private)]
#![warn(unused_extern_crates)]

extern crate rustc_hir;
extern crate rustc_index;
extern crate rustc_middle;
extern crate rustc_span;

use std::collections::HashMap;

use clippy_utils::{diagnostics::span_lint_and_help, match_def_path, ty::match_type};
use rustc_hir::{def::Res, Expr, ExprKind, QPath, TyKind};
use rustc_index::vec::Idx;
use rustc_lint::{LateContext, LateLintPass};
use rustc_middle::ty::{AdtDef, AdtKind, TyKind as MiddleTyKind};
use rustc_span::{def_id::DefId, Span};
use solana_lints::{paths, utils::visit_expr_no_bodies};

use if_chain::if_chain;

dylint_linting::impl_late_lint! {
    /// **What it does:**
    ///
    /// **Why is this bad?**
    ///
    /// **Known problems:** When only one enum is serialized, may miss certain edge cases.
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

impl<'tcx> LateLintPass<'tcx> for TypeCosplay {
    fn check_expr(&mut self, cx: &LateContext<'tcx>, expr: &'tcx Expr<'tcx>) {
        if_chain! {
            if !expr.span.from_expansion();
            if let ExprKind::Call(fnc_expr, args_exprs) = expr.kind;
            if is_deserialize_function(cx, fnc_expr);
            // walk each argument expression and see if the data field is referenced
            if args_exprs.iter().any(|arg| {
                visit_expr_no_bodies(arg, |expr| contains_data_field_reference(cx, expr))
            });
            // get the type that the function was called on, ie X in X::deser()
            if let ExprKind::Path(qpath) = &fnc_expr.kind;
            if let QPath::TypeRelative(ty, _) = qpath;
            if let TyKind::Path(ty_qpath) = &ty.kind;
            let res = cx.typeck_results().qpath_res(ty_qpath, ty.hir_id);
            if let Res::Def(_, def_id) = res;
            then {
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
        if self.deser_types.len() == 1 {
            let (k, v) = self.deser_types.iter().next().unwrap();
            match k {
                AdtKind::Enum => check_enums(cx, v),
                _ => check_structs_have_discriminant(cx, v), // NOTE: also catches unions
            }
        } else {
            // Retrieve spans: iter through map, grab first elem of each key-pair, then get span
            let mut spans = vec![];
            self.deser_types.iter().for_each(|(_, v)| {
                spans.push(v[0].1);
            });
            span_lint_and_help(
                cx,
                TYPE_COSPLAY,
                spans[0],
                "Deserializing from different ADT types.",
                Some(spans[1]),
                "help: deserialize from only structs with a discriminant, or an enum encapsulating all structs."
            )
        }
    }
}

fn is_deserialize_function(cx: &LateContext<'_>, expr: &Expr<'_>) -> bool {
    match cx.typeck_results().type_dependent_def_id(expr.hir_id) {
        Some(def_id) => match_def_path(cx, def_id, &paths::BORSH_TRY_FROM_SLICE),
        None => false,
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

fn check_enums(cx: &LateContext<'_>, enums: &Vec<(DefId, Span)>) {
    #[allow(clippy::comparison_chain)]
    if enums.len() > 1 {
        // TODO: can implement loop to print all spans if > 2 enums
        let first_span = enums[0].1;
        let second_span = enums[1].1;
        span_lint_and_help(
            cx,
            TYPE_COSPLAY,
            first_span,
            "warning: multiple enum types deserialized. Should only have one enum type to avoid possible equivalent types",
            Some(second_span),
            "help: consider constructing a single enum that contains all type definitions as variants"
        );
    } else if enums.len() == 1 {
        // future check - check that single enum is safe
        // check serialization
    }
}

fn check_structs_have_discriminant(cx: &LateContext<'_>, types: &Vec<(DefId, Span)>) {
    let num_structs = types.len();
    types
        .iter()
        .for_each(|t| has_discriminant(cx, cx.tcx.adt_def(t.0), num_structs, t.1));
}

/// Checks if `adt` has a proper discriminant. We define a proper discriminant as being an enum with
/// the number of variants at least the number of deserialized structs. Further the discriminant should
/// be the first field in the adt.
fn has_discriminant(cx: &LateContext, adt: AdtDef, num_struct_types: usize, span: Span) {
    let variant = adt.variants().get(Idx::new(0)).unwrap();
    let first_field_def = &variant.fields[0];
    let ty = cx.tcx.type_of(first_field_def.did);
    if_chain! {
        if let MiddleTyKind::Adt(adt_def, _) = ty.kind();
        if adt_def.is_enum();
        if adt_def.variants().len() >= num_struct_types;
        then {
            // struct has a proper discriminant
        } else {
            span_lint_and_help(
                cx,
                TYPE_COSPLAY,
                span,
                "warning: type does not have a proper discriminant. It may be indistinguishable when deserialized.",
                None,
                "help: add an enum with at least as many variants as there are struct definitions"
            )
        }
    }
}

#[test]
fn insecure_1() {
    dylint_testing::ui_test_example(env!("CARGO_PKG_NAME"), "insecure");
}

#[test]
fn insecure_2() {
    dylint_testing::ui_test_example(env!("CARGO_PKG_NAME"), "insecure-2");
}

#[test]
fn insecure_3() {
    dylint_testing::ui_test_example(env!("CARGO_PKG_NAME"), "insecure-3");
}

#[test]
fn secure() {
    dylint_testing::ui_test_example(env!("CARGO_PKG_NAME"), "secure");
}

#[test]
fn secure_two() {
    dylint_testing::ui_test_example(env!("CARGO_PKG_NAME"), "secure-2");
}

#[test]
fn recommended() {
    dylint_testing::ui_test_example(env!("CARGO_PKG_NAME"), "recommended");
}
