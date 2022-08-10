#![feature(rustc_private)]
#![warn(unused_extern_crates)]
#![recursion_limit = "256"]

extern crate rustc_data_structures;
extern crate rustc_hir;
extern crate rustc_index;
extern crate rustc_middle;
extern crate rustc_span;

use clippy_utils::{diagnostics::span_lint_and_help, match_def_path, ty::implements_trait, get_trait_def_id};
use rustc_data_structures::fx::FxHashMap;
use rustc_hir::{def::Res, Expr, ExprKind, QPath, TyKind};
use rustc_index::vec::Idx;
use rustc_lint::{LateContext, LateLintPass};
use rustc_middle::ty::{AdtDef, AdtKind, TyKind as MiddleTyKind};
use rustc_span::{def_id::DefId, Span};
use solana_lints::paths;

use if_chain::if_chain;

dylint_linting::impl_late_lint! {
    /// **What it does:** Checks that all deserialized types have a proper discriminant so that
    /// all types are guaranteed to deserialize differently.

    /// Instead of searching for equivalent types and checking to make sure those specific
    /// types have a discriminant, this lint takes a more strict approach and instead enforces
    /// all deserialized types it collects, to have a discriminant, regardless of whether the
    /// types are equivalent or not.

    /// We define a proper discriminant as an enum with as many variants as there are struct
    /// types in the program. Further, the discriminant should be the first field of every
    /// struct in order to avoid overwrite by arbitrary length fields, like vectors.

    /// A second case of a proper discriminant is when a single enum contains as variants all the struct
    /// types that will be deserialized. This "umbrella" enum essentially has a built-in
    /// discriminant. If it is the only type that is deserialized, then all struct types
    /// are guaranteed to be unique since the program will have to match a specific variant.
    ///
    /// **Why is this bad?**
    /// The type cosplay issue is when one account type can be substituted for another account type.
    /// This occurs when a type deserializes exactly the same as another type, such that you can't
    /// tell the difference between deserialized type `X` and deserialized type `Y`. This allows a
    /// malicious user to substitute `X` for `Y` or vice versa, and the code may perform unauthorized
    /// actions with the bytes.
    ///
    /// **Known problems:** In the case when only one enum is deserialized, this lint by default
    /// regards that as secure. However, this is not always the case. For example, if the program
    /// defines another enum and serializes, but never deserializes it, a user could create this enum,
    /// and, if it deserializes the same as the first enum, then this may be a possible vulnerability.

    /// Furthermore, one may have alternative definitions of a discriminant, such as using a bool,
    /// or u8, and not an enum. This will flag a false positive.
    pub TYPE_COSPLAY,
    Warn,
    "type is equivalent to another type",
    TypeCosplay::default()
}

#[derive(Default)]
struct TypeCosplay {
    deser_types: FxHashMap<AdtKind, Vec<(DefId, Span)>>,
}

// get type X
// check if implements Discriminator
// check corresponding function call type:
// if !try_deserialize
// emit lint with warning to use try_deserialize (because any types deriving Discriminator should since it guaranteed to check discrim)

impl<'tcx> LateLintPass<'tcx> for TypeCosplay {
    fn check_expr(&mut self, cx: &LateContext<'tcx>, expr: &'tcx Expr<'tcx>) {
        if_chain! {
            if let ExprKind::Call(fnc_expr, _args_exprs) = expr.kind;
            // TODO: is the following if statement really needed?? don't think it's ever used.
            // all it does is check if AccountInfo.data is referenced...but what bytes we deser
            // from shouldn't impact if this is a type-cosplay issue or not.
            // walk each argument expression and see if the data field is referenced
            // if args_exprs.iter().any(|arg| {
            //     visit_expr_no_bodies(arg, |expr| contains_data_field_reference(cx, expr))
            // });
            // get the type that the function was called on, ie X in X::deser()
            if let ExprKind::Path(qpath) = &fnc_expr.kind;
            if let QPath::TypeRelative(ty, _) = qpath;
            if let TyKind::Path(ty_qpath) = &ty.kind;
            let res = cx.typeck_results().qpath_res(ty_qpath, ty.hir_id);
            if let Res::Def(_, def_id) = res;
            let middle_ty = cx.tcx.type_of(def_id);
            if let Some(trait_did) = get_trait_def_id(cx, &paths::ANCHOR_DISCRIMINATOR_TRAIT);
            then {
                if implements_trait(cx, middle_ty, trait_did, &[]) {
                    if let Some(def_id) = cx.typeck_results().type_dependent_def_id(fnc_expr.hir_id) {
                        if !match_def_path(cx, def_id, &paths::ANCHOR_TRY_DESERIALIZE) {
                            span_lint_and_help(
                                cx,
                                TYPE_COSPLAY,
                                fnc_expr.span,
                                &format!("{} type implements the anchor_lang::Discriminator trait. If you are using #[account] to derive Discriminator, use try_deserialize() instead.",
                                    middle_ty),
                                None,
                                "otherwise, make sure you are accounting for this type's discriminator in your deserialization function"
                            );
                        }
                    }
                } else {
                    // currently only checks borsh::try_from_slice()
                    if is_deserialize_function(cx, fnc_expr) {
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
        }
    }

    fn check_crate_post(&mut self, cx: &LateContext<'tcx>) {
        // NOTE: the case where len == 0 does nothing, since no types are deserialized
        #[allow(clippy::comparison_chain)]
        if self.deser_types.len() == 1 {
            let (k, v) = self.deser_types.iter().next().unwrap();
            match k {
                AdtKind::Enum => check_enums(cx, v),
                _ => check_structs_have_discriminant(cx, v), // NOTE: also catches unions
            }
        } else if self.deser_types.len() > 1 {
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
                "deserialize from only structs with a discriminant, or an enum encapsulating all structs."
            );
        }
    }
}

fn is_deserialize_function(cx: &LateContext<'_>, expr: &Expr<'_>) -> bool {
    match cx.typeck_results().type_dependent_def_id(expr.hir_id) {
        Some(def_id) => match_def_path(cx, def_id, &paths::BORSH_TRY_FROM_SLICE),
        None => false,
    }
}

// fn contains_data_field_reference(cx: &LateContext<'_>, expr: &Expr<'_>) -> bool {
//     if_chain! {
//         if let ExprKind::Field(obj_expr, ident) = expr.kind;
//         if ident.as_str() == "data";
//         let ty = cx.typeck_results().expr_ty(obj_expr);
//         if match_type(cx, ty, &paths::SOLANA_PROGRAM_ACCOUNT_INFO);
//         then {
//             true
//         } else {
//             false
//         }
//     }
// }

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
            "multiple enum types deserialized. Should only have one enum type to avoid possible equivalent types",
            Some(second_span),
            "consider constructing a single enum that contains all type definitions as variants"
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
                "type does not have a proper discriminant. It may be indistinguishable when deserialized.",
                None,
                "add an enum with at least as many variants as there are struct definitions"
            );
        }
    }
}

#[test]
fn insecure() {
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
fn insecure_anchor() {
    dylint_testing::ui_test_example(env!("CARGO_PKG_NAME"), "insecure-anchor");
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
