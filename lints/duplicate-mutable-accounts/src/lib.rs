#![feature(rustc_private)]
#![warn(unused_extern_crates)]

extern crate rustc_ast;
extern crate rustc_hir;
extern crate rustc_span;

use rustc_ast::{
    token::TokenKind,
    tokenstream::{TokenStream, TokenTree},
    AttrKind, Attribute, MacArgs,
};
use rustc_hir::def::Res;
use rustc_hir::*;
use rustc_lint::{LateContext, LateLintPass};
use rustc_span::{def_id::DefId, symbol::Symbol, Span};
use std::collections::HashMap;
use std::default::Default;

use clippy_utils::{diagnostics::span_lint_and_help, ty::match_type};
use if_chain::if_chain;
use solana_lints::paths;

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
    pub DUPLICATE_MUTABLE_ACCOUNTS,
    Warn,
    "description goes here",
    DuplicateMutableAccounts::default()
}

#[derive(Default, Debug)]
struct DuplicateMutableAccounts {
    accounts: HashMap<DefId, Vec<(Symbol, Span)>>,
    streams: Vec<Stream>,
}

impl<'tcx> LateLintPass<'tcx> for DuplicateMutableAccounts {
    fn check_struct_def(&mut self, cx: &LateContext<'tcx>, variant_data: &'tcx VariantData<'tcx>) {
        if let VariantData::Struct(fields, _) = variant_data {
            fields.iter().for_each(|field| {
                if_chain! {
                    // grab the def_id of the field type
                    let ty = field.ty;
                    if let TyKind::Path(qpath) = &ty.kind;
                    if let QPath::Resolved(_, path) = qpath;
                    if let Res::Def(_, def_id) = path.res;
                    // match the type of the field
                    let ty = cx.tcx.type_of(def_id);
                    // check it is an anchor account type
                    if match_type(cx, ty, &paths::ANCHOR_ACCOUNT);
                    // check the type of T, the second generic arg
                    let account_id = get_anchor_account_type(&path.segments[0]).unwrap();
                    then {
                        if let Some(v) = self.accounts.get_mut(&account_id) {
                            v.push((field.ident.name, field.span));
                        } else {
                            self.accounts.insert(account_id, vec![(field.ident.name, field.span)]);
                        }
                    }
                }
            });
        }
    }

    fn check_attribute(&mut self, _: &LateContext<'tcx>, attribute: &'tcx Attribute) {
        // println!("{:#?}", self.accounts);
        if_chain! {
            if let AttrKind::Normal(attr_item, _) = &attribute.kind;
            let name = attr_item.path.segments[0].ident.name;
            // for some reason #[account] doesn't match when no args, maybe take away
            // the code to check name, and just check it has constraint args?
            if name.as_str() == "account";
            if let MacArgs::Delimited(_, _, token_stream) = &attr_item.args;
            then {
                self.streams.push(Stream(token_stream.clone()));
                // println!("{:#?}", attribute);
            }
        }
    }

    fn check_crate_post(&mut self, cx: &LateContext<'tcx>) {
        // println!("{:#?}", self);
        for (_k, v) in self.accounts.iter() {
            // if multiple same accounts, check that there is a tokenstream such that
            // all necessary expressions are in it: x.key(), y.key(), !=
            // where x and y are the field name from self.accounts
            if v.len() > 1 {
                if !has_satisfying_stream(&self.streams, v) {
                    span_lint_and_help(
                        cx,
                        DUPLICATE_MUTABLE_ACCOUNTS,
                        v[0].1,
                        "identical account types",
                        Some(v[1].1),
                        &format!("add an anchor key check constraint: #[account(constraint = {}.key() != {}.key())]", v[0].0, v[1].0)
                    );
                }
            }
        }
    }
}

fn get_anchor_account_type(segment: &PathSegment<'_>) -> Option<DefId> {
    if_chain! {
        if let Some(generic_args) = segment.args;
        if let GenericArg::Type(ty) = &generic_args.args[1]; // the account type is the second generic arg
        if let TyKind::Path(qpath) = &ty.kind;
        if let QPath::Resolved(_, path) = qpath;
        if let Res::Def(_, def_id) = path.res;
        then {
            Some(def_id)
        } else {
            None
        }
    }
}

fn has_satisfying_stream(streams: &Vec<Stream>, field_names: &Vec<(Symbol, Span)>) -> bool {
    for stream in streams {
        if stream.contains(TokenKind::Ne)
            && field_names
                .iter()
                // TODO: if true, will not match. figure out what the bool signifies
                .all(|(sym, _)| stream.contains(TokenKind::Ident(*sym, false)))
        {
            return true;
        }
    }
    return false;
}

#[derive(Debug)]
pub struct Stream(TokenStream);

impl Stream {
    fn contains(&self, x: TokenKind) -> bool {
        for token_tree in self.0.trees() {
            if let TokenTree::Token(t) = token_tree {
                if t.kind == x {
                    // println!("The type {:?} matches {:?}", t.kind, x);
                    return true;
                }
            }
        }
        return false;
    }
}

#[test]
fn insecure() {
    dylint_testing::ui_test_example(env!("CARGO_PKG_NAME"), "insecure");
}

#[test]
fn secure() {
    dylint_testing::ui_test_example(env!("CARGO_PKG_NAME"), "secure");
}
