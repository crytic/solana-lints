#![feature(rustc_private)]
#![warn(unused_extern_crates)]

extern crate rustc_hir;
extern crate rustc_span;

use solana_lints::{paths};
use clippy_utils::{diagnostics::span_lint, ty::match_type, SpanlessEq};
use rustc_lint::{LateContext, LateLintPass};
use rustc_hir::{Item, ItemKind, def::Res, Path, QPath, TyKind, HirId, Mod, FieldDef, Ty};
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
            let struct_field_ty_reprs: StructFieldTypeReprArray = StructFieldTypeReprArray::new();
            module.item_ids.iter().for_each(|id| {
                let item = cx.tcx.hir().item(*id);

                // filter out only struct items and convert to field_ty_repr
                if let ItemKind::Struct(variant_data, _) = &item.kind {
                    let struct_field_ty_repr = convert_struct_to_field_tys(variant_data.fields());
                    // println!("{:#?}", struct_field_ty_repr);

                    // if the array has a matching struct (ie, all fields are the same, in order, and no
                    // AccountDiscriminant field)
                    if struct_field_ty_reprs.has_match(cx, struct_field_ty_repr) {
                        span_lint(
                            cx,
                            TYPE_COSPLAY,
                            item.span,
                            "TODO"
                        )
                    } else {
                        struct_field_ty_reprs.0.push(struct_field_ty_repr);
                    }
                }
            });
        }
    }
}

/// A vector of StructFieldTypeRepr
struct StructFieldTypeReprArray<'hir>(Vec<StructFieldTypeRepr<'hir>>);

// TODO: probably better way to format data structures
impl<'hir> StructFieldTypeReprArray<'hir> {
    pub fn new() -> Self {
        StructFieldTypeReprArray(vec![])
    }

    // returns true if all fields are the same type and same order
    pub fn has_match(&self, cx: &LateContext, other: StructFieldTypeRepr) -> bool {
        if self.0.len() == 0 {
            false
        } else {
            // TODO: turn into iterator
            let mut equals = false;
            for item in self.0 {
                equals = item.0.iter().zip(other.0.iter())
                    .all(|(x, y)| eq_ty(x, y) && !match_type(cx, x, &paths::ACCOUNT_DISCRIMINANT));
                
                if equals {
                    return equals;
                }
            }
            equals
        }
    }
}

/// A representation of a struct as a vector of its field types
struct StructFieldTypeRepr<'hir>(Vec<&'hir Ty<'hir>>);

fn convert_struct_to_field_tys<'hir>(field_defs: &'hir [FieldDef<'hir>]) -> StructFieldTypeRepr {
    StructFieldTypeRepr(field_defs.iter().map(|def| def.ty).collect())
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

// fn check_item(&mut self, cx: &LateContext<'tcx>, item: &'tcx Item<'tcx>) {
//     // collect struct definitions
//     //  while collecting, if encountered an equivalent struct def, flag lint
//     // with the exception of if one of the field types == AccountDiscriminant
//     let structs = vec![];

//     if_chain! {
//         if !item.span.from_expansion();
//         if let ItemKind::Struct(variant_data, _) = &item.kind;
//         let field_def_ids = variant_data.fields().iter().for_each(|field| {
//             if_chain! {
//                 if let TyKind::Path(qpath) = field.ty.kind;
//                 if let QPath::Resolved(_, path) = qpath;
//                 if let Res::Def(_, def_id) = path.res;
//                 then {
//                     def_id
//                 } else {
//                     return;
//                 }
//             }
//         }).collect();
//         let _ = println!("{:#?}", variant_data.fields());

//         // if there exists another struct such that they match, then flag lint
//         if structs.has_match(field_def_ids);
//         then {
//             // let _ = println!("{:#?}", item);
            
//         }
        
//     }
// }
