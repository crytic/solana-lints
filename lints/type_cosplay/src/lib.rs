#![feature(rustc_private)]
#![warn(unused_extern_crates)]

extern crate rustc_hir;
extern crate rustc_index;
extern crate rustc_middle;
extern crate rustc_span;
extern crate rustc_typeck;

use std::fmt::{Debug, Formatter, Result};

use clippy_utils::{diagnostics::span_lint_and_note, ty::match_type, SpanlessEq};
use rustc_hir::{def::Res, HirId, Item, ItemKind, Mod, Path, QPath, Ty};
use rustc_index::vec::Idx;
use rustc_lint::{LateContext, LateLintPass};
use rustc_middle::ty::subst::GenericArg;
use rustc_middle::ty::List;
use rustc_middle::ty::TyKind;
use rustc_middle::ty::{AdtDef, FieldDef, Ty as MiddleTy, VariantDef};
use rustc_span::Symbol;
use solana_lints::paths;

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

// can get type_of(adt_def) then call kind() to get substs?

// collect struct items
// for each struct pair, call has_discriminant and eq_ty on each pair
// if any pair returns true for eq_ty and false for has_discriminant
// then: return true and span lint
// else: return false and push struct to array

impl<'tcx> LateLintPass<'tcx> for TypeCosplay {
    fn check_mod(&mut self, cx: &LateContext<'tcx>, module: &'tcx Mod<'tcx>, span: Span, _: HirId) {
        if !span.from_expansion() {
            let mut struct_items: Vec<&Item> = vec![];
            module.item_ids.iter().for_each(|id| {
                let item = cx.tcx.hir().item(*id);

                if let ItemKind::Struct(variant_data, _) = &item.kind {
                    let adt_def = cx.tcx.adt_def(item.def_id);

                    // TODO: can make if/else just one line later
					let mut other_item = None;
                    let has_eq_types = struct_items.iter().any(|other| {
                        if eq_ty_recur(cx, &adt_def, &cx.tcx.adt_def(other.def_id))
                            && !has_discriminant(cx, &adt_def)
                        {
							other_item = Some(other.span); // save span for lint message
                            println!("Equal and no discrim: {:?} and {:?}", adt_def, other);
                            return true;
                        } else {
                            return false;
                        }
                    });

                    if has_eq_types {
						span_lint_and_note(
							cx,
							TYPE_COSPLAY,
							item.span,
							"equivalent struct types that when deserialized will be indistinguishable", 
							other_item,
							"consider adding a discriminant field");
                    	// should we also push in this case?
                    } else {
                        println!("Pushing struct: {:?}", adt_def);
                        struct_items.push(item);
                    }
                }
            });
        }
    }
}

/// Walks `left` and `right` in a DFS manner, checking if each field of the struct is equivalent
/// If a field type is an ADT, that "sub-ADT" is walked in a recursive manner
fn eq_ty_recur(cx: &LateContext, left: &AdtDef, right: &AdtDef) -> bool {
    // grab the first variant of the adtdef (structs only have one variant)
    let l_variants = left.variants();
    let r_variants = right.variants();

    l_variants.len() == r_variants.len()
        && l_variants
            .iter()
            .zip(r_variants.iter())
            .all(|(lvar, rvar)| eq_variant(cx, lvar, rvar))
}

/// Returns true if all the fields of a variant have the same type
fn eq_variant(cx: &LateContext, left: &VariantDef, right: &VariantDef) -> bool {
    left.fields.len() == right.fields.len()
        && left
            .fields
            .iter()
            .zip(right.fields.iter())
            .all(|(lfield, rfield)| eq_field(cx, lfield, rfield))
}

/// Returns true if `left` has the same middle type as `right`. If the FieldDef is an ADT,
/// compare those types recursively.
fn eq_field(cx: &LateContext, left: &FieldDef, right: &FieldDef) -> bool {
    let l_tykind = cx.tcx.type_of(left.did).kind();
    let r_tykind = cx.tcx.type_of(right.did).kind();

    match (l_tykind, r_tykind) {
        (TyKind::Adt(l_adt_def, _), TyKind::Adt(r_adt_def, _)) => {
            eq_ty_recur(cx, l_adt_def, r_adt_def)
        }
        // If ONLY one of the fields is an ADT, there is a chance this is a tuple struct
        // that has an inner type that serializes the same way. Eg. PubKey(u8) and u8.
        // Thus, we grab the inner value in the ADT and compare
        (TyKind::Adt(adt_def, _), other) | (other, TyKind::Adt(adt_def, _)) => {
            if_chain! {
                // check that there is only 1 variant
                if adt_def.variants().len() == 1;
                let variant = adt_def.variants().get(Idx::new(0)).unwrap();
                // check that the 1 variant is a tuple struct, ie, has 1 field
                if variant.fields.len() == 1;
                let field = &variant.fields[0];
                if cx.tcx.type_of(field.did).kind() == other;
                then {
                    return true;
                } else {
                    return false;
                }
            }
        }
        _ => l_tykind == r_tykind,
    }
}

/// Returns true if the ADT has a field of type AccountDiscriminant
fn has_discriminant(cx: &LateContext, adt: &AdtDef) -> bool {
    // Various ways to check, from easy to hard:
    // 1. simply check field name
    // 2. check if it is an enum with #variants == #equivalent struct types in code
    let variant = adt.variants().get(Idx::new(0)).unwrap();
    variant
        .fields
        .iter()
        .any(|field| field.name.as_str() == "discriminant")
}

/// A vector of StructFieldTypeRepr
// struct StructFieldTypeReprArray<'hir>(Vec<StructFieldTypeRepr<'hir>>);

// // TODO: can maybe get rid of this and just have it be a raw vector
// impl<'hir> StructFieldTypeReprArray<'hir> {
//     pub fn new() -> Self {
//         StructFieldTypeReprArray(vec![])
//     }

//     // returns true if all fields are the same type and same order
//     pub fn has_match(&self, cx: &LateContext, other: &StructFieldTypeRepr) -> bool {
//         // any -- if any two structs are equal, return true immediately; if NO structs match, then return false
//         self.0.iter().any(|item| {
//             if item.eq(other) {
//                 println!("{:#?}\n, {:#?}", item, other);
//                 return true;
//             } else {
//                 // println!("structs not equal: {:?} {:?}", item.0[0].hir_id.owner, other.0[0].hir_id.owner);
//                 return false;
//             }
//         })
//     }
// }

// /// A representation of a struct as a vector of its field types
// struct StructFieldTypeRepr<'hir>(Vec<&'hir MiddleTy<'hir>>);

// impl<'hir> StructFieldTypeRepr<'hir> {
//     // later add check: && !match_type(cx, x, &paths::ACCOUNT_DISCRIMINANT)
//     pub fn eq(&self, other: &StructFieldTypeRepr) -> bool {
//         // all -- if all fields are equal, returns true; as soon as unequal fields
//         // are found, returns false immediately
//         self.0.len() == other.0.len()
//             && self.0.iter().zip(other.0.iter()).all(|(x, y)| {
//                 // if eq_ty(x, y) {
//                 //     println!("{:#?} and {:#?} are equal types", x, y);
//                 return true;
//                 // } else {
//                 //     return false;
//                 // }
//             })
//     }

//     // pub fn from_field_defs(field_defs: &'hir [FieldDef<'hir>]) -> StructFieldTypeRepr {
//     //     StructFieldTypeRepr(field_defs.iter().map(|def| {

//     //     }).collect())
//     // }
// }

// impl<'hir> Debug for StructFieldTypeRepr<'hir> {
//     fn fmt(&self, f: &mut Formatter<'_>) -> Result {
//         f.debug_tuple("StructFieldTypeRepr").field(&self.0).finish()
//     }
// }

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
