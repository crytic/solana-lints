#![feature(rustc_private)]
#![warn(unused_extern_crates)]
#![recursion_limit = "256"]

extern crate rustc_data_structures;
extern crate rustc_hir;
extern crate rustc_index;
extern crate rustc_middle;
extern crate rustc_span;

use clippy_utils::{
    diagnostics::span_lint_and_help,
    get_trait_def_id, match_def_path,
    ty::{implements_trait, match_type},
};
use if_chain::if_chain;
use rustc_data_structures::fx::FxHashMap;
use rustc_hir::{def::Res, Expr, ExprKind, QPath, TyKind};
use rustc_index::vec::Idx;
use rustc_lint::{LateContext, LateLintPass};
use rustc_middle::ty::{AdtDef, AdtKind, TyKind as MiddleTyKind};
use rustc_span::{def_id::DefId, Span};
use solana_lints::{paths, utils::visit_expr_no_bodies};

dylint_linting::impl_late_lint! {
    /// **What it does:** Checks that all deserialized types have a proper discriminant so that
    /// all types are guaranteed to deserialize differently.
    ///
    /// Instead of searching for equivalent types and checking to make sure those specific
    /// types have a discriminant, this lint takes a more strict approach and instead enforces
    /// all deserialized types it collects, to have a discriminant, regardless of whether the
    /// types are equivalent or not.
    ///
    /// We define a proper discriminant as an enum with as many variants as there are struct
    /// types in the program. Further, the discriminant should be the first field of every
    /// struct in order to avoid overwrite by arbitrary length fields, like vectors.
    ///
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
    ///
    /// Furthermore, one may have alternative definitions of a discriminant, such as using a bool,
    /// or u8, and not an enum. This will flag a false positive.
    ///
    /// ## Note on Tests
    ///
    /// ### insecure
    ///
    /// This is the canonical example of type-cosplay. The program tries to deserialize
    /// bytes from `AccountInfo.data` into the `User` type. However, a malicious user could pass in
    /// an account that has in it's data field the `Metadata` type. This type is equivalent to the
    /// `User` type, and the data bytes will thus successfully deserialize as a `User` type. The
    /// program performs no checks whatsoever, and will continue on operating with a pubkey that it
    /// believes to be a `User` pubkey, not a `Metadata` pubkey.
    ///
    /// ### insecure-2
    ///
    /// This is insecure because the program tries to deserialize from multiple enum types.
    /// Here, `UserInfo` and `MetadataInfo` enums are both being deserialized. Note that both of these
    /// enums contain a single variant, with the struct type nested inside it. This evades the in-built
    /// discriminant of an enum. A `Metadata` type could be deserialized into a `UserInfo::User(User)`,
    /// and a `User` could be deserialized into a `MetadataInfo::Metadata(Metadata)`.
    ///
    /// Only deserializing from a single enum is safe since enums contain a natural, in-built discriminator.
    /// If _all_ types are nested under a variant of this enum, then when deserializing, the enum variant
    /// must be matched first, thus guaranteeing differentiation between types.
    ///
    /// However, deserializing from multiple enums partitions the "set of types" and is thus not exhaustive
    /// in discriminating between all types. If multiple enums are used to encompass the types, there may
    /// be two equivalent types that are variants under different enums, as seen in this example.
    ///
    /// ### insecure-3
    ///
    /// This example is insecure because `AccountWithDiscriminant` could be deserialized as a
    /// `User`, if the variant is `Extra(Extra)`. The first byte would be 0, to indicate the discriminant
    /// in both cases, and the next 32 bytes would be the pubkey. The problem here is similar to
    /// the insecure-2 example--not all types are nested under a single enum type. Except here,
    /// instead of using another enum, the program also tries to deserialize `User`.
    ///
    /// This illustrates that in order to properly take advantage of the enums natural built-in
    /// discriminator, you must nest _all_ types in your program as variants of this enum, and
    /// only serialize and deserialize this enum type.
    ///
    /// ### insecure-anchor
    ///
    /// Insecure because `User` type derives Discriminator trait (via `#[account]`),
    /// thus one may expect this code to be secure. However, the program tries to deserialize with
    /// `try_from_slice`, the default borsh deserialization method, which does _not_ check for the
    /// discriminator. Thus, one could potentially serialize a `Metadata` struct, and then later
    /// deserialize without any problem into a `User` struct, leading to a type-cosplay vulnerability.
    ///
    /// ### recommended
    ///
    /// The recommended way to address the type-cosplay issue. It adds an `#[account]` macro to each
    /// struct, which adds a discriminant to each struct. It doesn't actually perform any deserializations,
    /// which is why the `recommended-2` was created.
    ///
    /// ### recommended-2
    ///
    /// This is secure code because all structs have an `#[account]` macro attributed
    /// on them, thus deriving the `Discriminator` trait for each. Further, unlike the insecure-anchor
    /// example, the program uses the proper deserialization method, `try_deserialize`, to deserialize
    /// bytes as `User`. This is "proper" because in the derived implementation of `try_deserialize`,
    /// the discriminator of the type is checked first.
    ///
    /// _Note: this example differs from the Sealevel [recommended](https://github.com/coral-xyz/sealevel-attacks/blob/master/programs/3-type-cosplay/recommended/src/lib.rs) example in that it actually attempts_
    /// _to perform a deserialization in the function body, and then uses the struct. It provides_
    /// _a more realistic and concrete example of what might happen in real programs_
    ///
    /// ### secure
    ///
    /// This example is from the Sealevel [example](https://github.com/coral-xyz/sealevel-attacks/blob/master/programs/3-type-cosplay/secure/src/lib.rs). It fixes the insecure case by adding a `discriminant`
    /// field to each struct, and further this discriminant is "proper" because it contains the
    /// necessary amount of variants in order to differentiate each type. In the code, there is
    /// an explicit check to make sure the discriminant is as expected.
    ///
    /// ### secure-2
    ///
    /// This example fixes both the insecure and insecure-2 examples. It is secure because it only deserializes
    /// from a single enum, and that enum encapsulates all of the user-defined types. Since enums contain
    /// an implicit discriminant, this program will always be secure as long as all types are defined under the enum.
    pub TYPE_COSPLAY,
    Warn,
    "type is equivalent to another type",
    TypeCosplay::default()
}

#[derive(Default)]
struct TypeCosplay {
    deser_types: FxHashMap<AdtKind, Vec<(DefId, Span)>>,
}

impl<'tcx> LateLintPass<'tcx> for TypeCosplay {
    fn check_expr(&mut self, cx: &LateContext<'tcx>, expr: &'tcx Expr<'tcx>) {
        if_chain! {
            if !expr.span.from_expansion();
            if let ExprKind::Call(fnc_expr, args_exprs) = expr.kind;
            // TODO: recommended-2 case will exit early since it contains a reference to AccountInfo.data,
            // not a direct argument. In general, any references will fail
            // smoelius: I updated the `recommended-2` test so that the call contains a reference to
            // `AccountInfo.data`. But @victor-wei126's comment is still relevant in that we need a
            // more general solution for finding references to `AccountInfo.data`.
            if args_exprs.iter().any(|arg| {
                visit_expr_no_bodies(arg, |expr| contains_data_field_reference(cx, expr))
            });
            // get the type that the function was called on, ie X in X::call()
            if let ExprKind::Path(qpath) = &fnc_expr.kind;
            if let QPath::TypeRelative(ty, _) = qpath;
            if let TyKind::Path(ty_qpath) = &ty.kind;
            let res = cx.typeck_results().qpath_res(ty_qpath, ty.hir_id);
            if let Res::Def(_, def_id) = res;
            let middle_ty = cx.tcx.type_of(def_id);
            then {
                if_chain! {
                    if let Some(trait_did) = get_trait_def_id(cx, &paths::ANCHOR_LANG_DISCRIMINATOR);
                    if implements_trait(cx, middle_ty, trait_did, &[]);
                    if let Some(def_id) = cx.typeck_results().type_dependent_def_id(fnc_expr.hir_id);
                    if !match_def_path(cx, def_id, &paths::ANCHOR_LANG_TRY_DESERIALIZE);
                    then {
                        span_lint_and_help(
                            cx,
                            TYPE_COSPLAY,
                            fnc_expr.span,
                            &format!("`{}` type implements the `Discriminator` trait. If you are attempting to deserialize\n here and `{}` is annotated with #[account] use try_deserialize() instead.",
                                middle_ty,
                                middle_ty
                            ),
                            None,
                            "otherwise, make sure you are accounting for this type's discriminator in your deserialization function"
                        );
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

fn contains_data_field_reference(cx: &LateContext<'_>, expr: &Expr<'_>) -> bool {
    if_chain! {
        if let ExprKind::Field(obj_expr, ident) = expr.kind;
        if ident.as_str() == "data";
        let ty = cx.typeck_results().expr_ty(obj_expr).peel_refs();
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

#[test]
fn recommended_2() {
    dylint_testing::ui_test_example(env!("CARGO_PKG_NAME"), "recommended-2");
}
