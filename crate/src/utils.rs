use anchor_syn::parser::accounts as accounts_parser;
use anchor_syn::AccountsStruct;
use clippy_utils::{get_trait_def_id, ty::implements_trait};
use if_chain::if_chain;
use rustc_hir::{
    intravisit::{walk_expr, Visitor},
    Expr, Item, ItemKind,
};
use rustc_lint::LateContext;
use rustc_middle::ty::{self, GenericArgKind};
use syn::{parse_str, ItemStruct};

use crate::paths;

pub trait Conclusive: Default {
    fn concluded(&self) -> bool;
}

impl<T> Conclusive for Option<T> {
    fn concluded(&self) -> bool {
        self.is_some()
    }
}

impl Conclusive for bool {
    fn concluded(&self) -> bool {
        *self
    }
}

pub fn visit_expr_no_bodies<'tcx, T>(
    expr: &'tcx Expr<'tcx>,
    f: impl FnMut(&'tcx Expr<'tcx>) -> T,
) -> T
where
    T: Conclusive,
{
    let mut v = V {
        f,
        result: T::default(),
    };
    v.visit_expr(expr);
    v.result
}

struct V<F, T> {
    f: F,
    result: T,
}

impl<'tcx, F, T> Visitor<'tcx> for V<F, T>
where
    F: FnMut(&'tcx Expr<'tcx>) -> T,
    T: Conclusive,
{
    fn visit_expr(&mut self, expr: &'tcx Expr<'tcx>) {
        if !self.result.concluded() {
            self.result = (self.f)(expr);

            if !self.result.concluded() {
                walk_expr(self, expr);
            }
        }
    }
}

/// Return `Some(accounts_struct)` if the item is an Anchor Accounts struct derived using `#[derive(Accounts)]` macro else None
/// - If Item is a Struct and implements `anchor_lang::ToAccountInfos` trait.
///     - Get the pre-expansion source code and parse it using anchor's accounts parser
///     - If parsing succeeds then
///         - Return `Some(anchor_syn::AccountsStruct)`
///     - Else return None
/// - Else return None
pub fn get_anchor_accounts_struct<'tcx>(
    cx: &LateContext<'tcx>,
    item: &'tcx Item<'tcx>,
) -> Option<AccountsStruct> {
    // identify Anchor structs:
    // `#[derive(Accounts)]` macro implements `anchor_lang::ToAccountInfos` trait for the struct
    if_chain! {
        // Anchor generated IDL structs also implement these traits. Check if the item is defined in this crate.
        if !item.span.from_expansion();
        if let ItemKind::Struct(_, _) = item.kind;
        // Accounts structs implement `anchor_lang::ToAccountInfos` trait.
        // see: https://github.com/coral-xyz/anchor/blob/8eee184938f71e3b85414d469db55fd882b380b2/lang/syn/src/codegen/accounts/mod.rs#L19
        if let Some(trait_id) = get_trait_def_id(cx.tcx, &paths::ANCHOR_LANG_TO_ACCOUNT_INFOS_TRAIT);
        // `implements_trait` takes generic arguments. providing empty args or dummy args works fine outside tests but
        // fails in tests because of a debug assertion. ToAccountInfos is used instead of Accounts trait because Accounts trait
        // takes a type generic argument. It is not possible to find the generic args in `check_item`, have to use `check_impl` probably.
        // Assumption: Accounts structs have a lifetime argument and it should be same for the trait implementations as well.
        if let ty::Adt(_, substs) = cx
            .tcx
            .type_of(item.owner_id.to_def_id())
            .skip_binder()
            .kind();
        if let Some(lifetime_arg) = substs
            .iter()
            .find(|arg| matches!(arg.unpack(), GenericArgKind::Lifetime(_)));
        if implements_trait(
            cx,
            cx.tcx.type_of(item.owner_id.to_def_id()).skip_binder(),
            trait_id,
            &[lifetime_arg],
        );
        // Get the pre-expansion source code of the struct and parse it using anchor's parser.
        if let Ok(struct_str) = cx.tcx.sess.source_map().span_to_snippet(item.span);
        if let Ok(syn_struct) = parse_str::<ItemStruct>(&struct_str);
        if let Ok(accounts_struct) = accounts_parser::parse(&syn_struct);
        then {
            Some(accounts_struct)
        } else {
            None
        }
    }
}

/// Return true if the current program is an anchor program
///
/// Anchor generated programs will have
/// mod instruction {}
/// mod program {}
/// mod __private { mod __global {} mod __idl}
/// fn dispatch
/// fn entry
///
/// `is_anchor_program` uses the presence of `mod __private { mod __global {} mod __idl}` to determine if
/// the Solana program is written using Anchor
/// - If the root module has `__private` module and it contains `__global`, `__idl` modules
///   - return True
/// - else false
pub fn is_anchor_program(cx: &LateContext<'_>) -> bool {
    let hir_map = cx.tcx.hir();
    hir_map
        .root_module()
        .item_ids
        .iter()
        .map(|item_id| hir_map.item(*item_id))
        .any(|item| {
            if_chain! {
                if let ItemKind::Mod(private_mod) = item.kind;
                if item.ident.as_str() == "__private";
                if private_mod
                    .item_ids
                    .iter()
                    .map(|inner_item_id| hir_map.item(*inner_item_id))
                    .any(|inner_item| {
                        matches!(inner_item.kind, ItemKind::Mod(_))
                            && inner_item.ident.as_str() == "__global"
                    });
                if private_mod
                    .item_ids
                    .iter()
                    .map(|inner_item_id| hir_map.item(*inner_item_id))
                    .any(|inner_item| {
                        matches!(inner_item.kind, ItemKind::Mod(_))
                            && inner_item.ident.as_str() == "__idl"
                    });
                then {
                    true
                } else {
                    false
                }
            }
        })
}
