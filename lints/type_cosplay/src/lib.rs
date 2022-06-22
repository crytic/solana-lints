#![feature(rustc_private)]
#![warn(unused_extern_crates)]

extern crate rustc_hir;
extern crate rustc_span;
extern crate rustc_middle;
extern crate rustc_typeck;

use std::fmt::{Debug, Formatter, Result};

use solana_lints::paths;
use clippy_utils::{diagnostics::span_lint, ty::match_type, SpanlessEq};
use rustc_lint::{LateContext, LateLintPass};
use rustc_hir::{Item, ItemKind, def::Res, Path, QPath, HirId, Mod, Ty};
use rustc_middle::ty::{Ty as MiddleTy, FieldDef, AdtDef};
use rustc_middle::ty::subst::GenericArg;
use rustc_middle::ty::TyKind;
use rustc_span::Symbol;
use rustc_middle::ty::List;

use rustc_span::Span;
use if_chain::if_chain;

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

// TODO:
// manual implementation of eq_ty and match_type
// test everything
// reformat code - data structures, iterators, visitor

impl<'tcx> LateLintPass<'tcx> for TypeCosplay {
    fn check_mod(
        &mut self,
        cx: &LateContext<'tcx>,
        module: &'tcx Mod<'tcx>,
        span: Span,
        _: HirId
    ) {
        if !span.from_expansion() {
            // this code could be replaced with visit_item?
            // collect only struct items
            let mut struct_field_ty_reprs: StructFieldTypeReprArray = StructFieldTypeReprArray::new();
            module.item_ids.iter().for_each(|id| {
                let item = cx.tcx.hir().item(*id);

                // filter out only struct items and convert to field_ty_repr
                // TODO: add union match?
                if let ItemKind::Struct(variant_data, _) = &item.kind {
                    let adt_def = cx.tcx.adt_def(item.def_id);
                    println!("{:?}", cx.tcx.type_of(adt_def.did)); // rustc version? did is defined as method, not field

                    // first convert struct to an array of field_defs
                    // then recursively get the ty::Ty of each field_def, until there are no more Adt types (only primitives)

                    let result: Vec<StructFieldTypeRepr> = get_field_types_recursive(cx, adt_def);

                    // let struct_field_ty_repr = StructFieldTypeRepr::from_field_defs(variant_data.fields());

                    // if the array has a matching struct (ie, all fields are the same, in order, and no
                    // AccountDiscriminant field)
                    // if struct_field_ty_reprs.has_match(cx, &struct_field_ty_repr) {
                    //     span_lint(
                    //         cx,
                    //         TYPE_COSPLAY,
                    //         item.span,
                    //         "TODO"
                    //     )
                    // } else {
                    //     println!("Pushing struct");
                    //     struct_field_ty_reprs.0.push(struct_field_ty_repr);
                    // }
                }
            });
        }
    }
}

fn get_field_types_recursive<'tcx>(cx: &LateContext, adt_def: &AdtDef) -> Vec<StructFieldTypeRepr<'tcx>> {
    let mut result = vec![];

    for variant in adt_def.variants {
        let field_tys: Vec<MiddleTy> = variant.fields.iter().map(|field_def| {
            let field_type = cx.tcx.type_of(field_def.did);
            if let TyKind::Adt(sub_adt_def, _) = field_type.kind() {
                get_field_types_recursive(cx, sub_adt_def)
            } else {
                StructFieldTypeRepr(field_type)
            }
        }).collect()
        result.push(StructFieldTypeRepr(field_tys));
    }
}

/// A vector of StructFieldTypeRepr
struct StructFieldTypeReprArray<'hir>(Vec<StructFieldTypeRepr<'hir>>);

// TODO: can maybe get rid of this and just have it be a raw vector
impl<'hir> StructFieldTypeReprArray<'hir> {
    pub fn new() -> Self {
        StructFieldTypeReprArray(vec![])
    }

    // returns true if all fields are the same type and same order
    pub fn has_match(&self, cx: &LateContext, other: &StructFieldTypeRepr) -> bool {
        // any -- if any two structs are equal, return true immediately; if NO structs match, then return false
        self.0.iter().any(|item| {
            if item.eq(other) {
                println!("{:#?}\n, {:#?}", item, other);
                return true;
            } else {
                // println!("structs not equal: {:?} {:?}", item.0[0].hir_id.owner, other.0[0].hir_id.owner);
                return false;
            }
        })
    }
}

/// A representation of a struct as a vector of its field types
struct StructFieldTypeRepr<'hir>(Vec<&'hir MiddleTy<'hir>>);

impl<'hir> StructFieldTypeRepr<'hir> {
    // later add check: && !match_type(cx, x, &paths::ACCOUNT_DISCRIMINANT)
    pub fn eq(&self, other: &StructFieldTypeRepr) -> bool {
        // all -- if all fields are equal, returns true; as soon as unequal fields
        // are found, returns false immediately
        self.0.len() == other.0.len() && self.0.iter().zip(other.0.iter())
            .all(|(x, y)| {
                // if eq_ty(x, y) {
                //     println!("{:#?} and {:#?} are equal types", x, y);
                    return true;
                // } else {
                //     return false;
                // }
            })
    }

    // pub fn from_field_defs(field_defs: &'hir [FieldDef<'hir>]) -> StructFieldTypeRepr {
    //     StructFieldTypeRepr(field_defs.iter().map(|def| {
            
    //     }).collect())
    // }
}

impl<'hir> Debug for StructFieldTypeRepr<'hir> {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result {
        f.debug_tuple("StructFieldTypeRepr")
            .field(&self.0)
            .finish()
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
