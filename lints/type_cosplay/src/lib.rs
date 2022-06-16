#![feature(rustc_private)]
#![warn(unused_extern_crates)]

extern crate rustc_hir;
extern crate rustc_span;

use std::fmt::{Debug, Formatter, Result};

use solana_lints::paths;
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
                if let ItemKind::Struct(variant_data, _) = &item.kind {
                    let struct_field_ty_repr = StructFieldTypeRepr::from(variant_data.fields());

                    // if the array has a matching struct (ie, all fields are the same, in order, and no
                    // AccountDiscriminant field)
                    if struct_field_ty_reprs.has_match(cx, &struct_field_ty_repr) {
                        span_lint(
                            cx,
                            TYPE_COSPLAY,
                            item.span,
                            "TODO"
                        )
                    } else {
                        println!("Pushing struct");
                        struct_field_ty_reprs.0.push(struct_field_ty_repr);
                    }
                }
            });
        }
    }
}

// NOTE: only for structs
fn eq_ty(left: &Ty, right: &Ty) -> bool {
    match (&left.kind, &right.kind) {
        (&TyKind::Path(ref l), &TyKind::Path(ref r)) => eq_qpath(l, r),
        _ => false,
    }
}

// NOTE: only for QPath::Resolved
fn eq_qpath(left: &QPath, right: &QPath) -> bool {
    match (left, right) {
        (&QPath::Resolved(ref lty, lpath), &QPath::Resolved(ref rty, rpath)) => {
            both(lty, rty, |l, r| eq_ty(l, r)) && eq_path(lpath, rpath)
        },
        _ => false,
    }
}

// need to check recursively all the way down to primitive type
fn eq_path(left: &Path, right: &Path) -> bool {
    match(left.res, right.res) {
        // TODO: not sure if we can just compare raw
        (Res::Def(_, l), Res::Def(_, r)) => l == r,
        _ => false,
    }
}

// https://github.com/rust-lang/rust-clippy/blob/17b7ab004fd67f186b5822bf6c42c16896802c4b/clippy_utils/src/hir_utils.rs#L502
/// Checks if the two `Option`s are both `None` or some equal values as per
/// `eq_fn`.
pub fn both<X>(l: &Option<X>, r: &Option<X>, mut eq_fn: impl FnMut(&X, &X) -> bool) -> bool {
    l.as_ref()
        .map_or_else(|| r.is_none(), |x| r.as_ref().map_or(false, |y| eq_fn(x, y)))
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
                println!("structs not equal: {:?} {:?}", item.0[0].hir_id.owner, other.0[0].hir_id.owner);
                return false;
            }
        })
    }
}

/// A representation of a struct as a vector of its field types
struct StructFieldTypeRepr<'hir>(Vec<&'hir Ty<'hir>>);

impl<'hir> StructFieldTypeRepr<'hir> {
    // later add check: && !match_type(cx, x, &paths::ACCOUNT_DISCRIMINANT)
    pub fn eq(&self, other: &StructFieldTypeRepr) -> bool {
        // all -- if all fields are equal, returns true; as soon as unequal fields
        // are found, returns false immediately

        // BUG: doesn't zip through all elems?
        self.0.iter().for_each(|x| println!("{:?}", x));
        other.0.iter().for_each(|x| println!("{:?}", x));

        // add another len check
        self.0.iter().zip(other.0.iter())
            .all(|(x, y)| {
                if eq_ty(x, y) {
                    println!("{:#?} and {:#?} are equal types", x, y);
                    return true;
                } else {
                    return false;
                }
            })
    }

    // change name
    pub fn from(field_defs: &'hir [FieldDef<'hir>]) -> StructFieldTypeRepr {
        StructFieldTypeRepr(field_defs.iter().map(|def| def.ty).collect())
    }
}

impl<'hir> Debug for StructFieldTypeRepr<'hir> {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result {
        f.debug_tuple("StructFieldTypeRepr")
            .field(&self.0)
            .finish()
    }
}

#[cfg(test)]
mod test {
    use super::*;

    struct User {
        authority: u8,
        ocean: u16
    }
    struct Metadata {
        brown: u8,
        james: u16,
    }

    #[test]
    fn test_eq_structs() {
        // let struct1 = User { 8, 10 };
        // let struct2 = Metadata { 4, 45 };


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
