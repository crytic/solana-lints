#![feature(rustc_private)]
#![warn(unused_extern_crates)]

extern crate rustc_ast;
extern crate rustc_hir;
extern crate rustc_span;

use std::collections::{HashMap, VecDeque};
use std::default::Default;

use rustc_ast::{
    token::{Delimiter, Token, TokenKind},
    tokenstream::{DelimSpan, TokenStream, TokenTree, TreeAndSpacing},
    AttrKind, Attribute, MacArgs,
};
use rustc_hir::{def::Res, FieldDef, GenericArg, QPath, TyKind, VariantData};
use rustc_lint::{LateContext, LateLintPass};
use rustc_span::{
    def_id::DefId,
    symbol::{Ident, Symbol},
    Span, DUMMY_SP,
};

use clippy_utils::{diagnostics::span_lint_and_help, ty::match_type};
use if_chain::if_chain;
use solana_lints::paths;

const ANCHOR_ACCOUNT_GENERIC_ARG_COUNT: usize = 2;

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
    streams: Streams,
}

impl<'tcx> LateLintPass<'tcx> for DuplicateMutableAccounts {
    fn check_struct_def(&mut self, cx: &LateContext<'tcx>, variant_data: &'tcx VariantData<'tcx>) {
        if let VariantData::Struct(fields, _) = variant_data {
            fields.iter().for_each(|field| {
                if_chain! {
                    if let Some(def_id) = get_def_id(field.ty);
                    let middle_ty = cx.tcx.type_of(def_id);
                    if match_type(cx, middle_ty, &paths::ANCHOR_ACCOUNT);
                    if let Some(account_id) = get_anchor_account_type_def_id(field);
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
        if_chain! {
            if let AttrKind::Normal(attr_item, _) = &attribute.kind;
            let name = attribute.name_or_empty();
            if name.as_str() == "account";
            if let MacArgs::Delimited(_, _, token_stream) = &attr_item.args;
            then {
                self.streams.0.push(token_stream.clone());
            }
        }
    }

    fn check_crate_post(&mut self, cx: &LateContext<'tcx>) {
        // println!("{:#?}", self);
        for v in self.accounts.values() {
            if v.len() > 1 {
                let mut deq = VecDeque::from(v.to_owned());
                for _ in 0..deq.len() - 1 {
                    let (first, first_span) = deq.pop_front().unwrap();
                    for (other, other_span) in &deq {
                        let stream = create_key_check_constraint_tokenstream(first, *other);
                        let symmetric_stream =
                            create_key_check_constraint_tokenstream(*other, first);

                        if !(self.streams.contains(&stream)
                            || self.streams.contains(&symmetric_stream))
                        {
                            span_lint_and_help(
                                cx,
                                DUPLICATE_MUTABLE_ACCOUNTS,
                                first_span,
                                &format!("{} and {} have identical account types but do not have a key check constraint", first, other),
                                Some(*other_span),
                                &format!("add an anchor key check constraint: #[account(constraint = {}.key() != {}.key())]", first, other)
                            );
                        }
                    }
                }
            }
        }
    }
}

/// Returns the `DefId` of the anchor account type, ie, `T` in `Account<'info, T>`.
/// Returns `None` if the type of `field` is not an anchor account.
fn get_anchor_account_type_def_id(field: &FieldDef) -> Option<DefId> {
    if_chain! {
        if let TyKind::Path(qpath) = &field.ty.kind;
        if let QPath::Resolved(_, path) = qpath;
        if !path.segments.is_empty();
        if let Some(generic_args) = path.segments[0].args;
        if generic_args.args.len() == ANCHOR_ACCOUNT_GENERIC_ARG_COUNT;
        if let GenericArg::Type(hir_ty) = &generic_args.args[1];
        then {
            get_def_id(hir_ty)
        } else {
            None
        }
    }
}

/// Returns the `DefId` of `ty`, an hir type. Returns `None` if cannot resolve type.
fn get_def_id(ty: &rustc_hir::Ty) -> Option<DefId> {
    if_chain! {
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

/// Returns a `TokenStream` of form: `a`.key() != `b`.key().
fn create_key_check_constraint_tokenstream(a: Symbol, b: Symbol) -> TokenStream {
    // TODO: may be more efficient way to do this, since the stream is effectively fixed
    // and determined. Only two tokens are variable.
    let constraint = vec![
        TreeAndSpacing::from(create_token_from_ident(a.as_str())),
        TreeAndSpacing::from(TokenTree::Token(Token::new(TokenKind::Dot, DUMMY_SP))),
        TreeAndSpacing::from(create_token_from_ident("key")),
        TreeAndSpacing::from(TokenTree::Delimited(
            DelimSpan::dummy(),
            Delimiter::Parenthesis,
            TokenStream::new(vec![]),
        )),
        TreeAndSpacing::from(TokenTree::Token(Token::new(TokenKind::Ne, DUMMY_SP))),
        TreeAndSpacing::from(create_token_from_ident(b.as_str())),
        TreeAndSpacing::from(TokenTree::Token(Token::new(TokenKind::Dot, DUMMY_SP))),
        TreeAndSpacing::from(create_token_from_ident("key")),
        TreeAndSpacing::from(TokenTree::Delimited(
            DelimSpan::dummy(),
            Delimiter::Parenthesis,
            TokenStream::new(vec![]),
        )),
    ];

    TokenStream::new(constraint)
}

/// Returns a `TokenTree::Token` which has `TokenKind::Ident`, with the string set to `s`.
fn create_token_from_ident(s: &str) -> TokenTree {
    let ident = Ident::from_str(s);
    TokenTree::Token(Token::from_ast_ident(ident))
}

#[derive(Debug, Default)]
pub struct Streams(Vec<TokenStream>);

impl Streams {
    /// Returns true if `self` has a TokenStream that `other` is a substream of
    fn contains(&self, other: &TokenStream) -> bool {
        self.0
            .iter()
            .any(|token_stream| Self::is_substream(token_stream, other))
    }

    /// Returns true if `other` is a substream of `stream`. By substream we mean in the
    /// sense of a substring.
    // NOTE: a possible optimization is when a match is found, to remove the matched
    // TokenTrees from the TokenStream, since the constraint has been "checked" so it never
    // needs to be validated again. This cuts down the number of comparisons.
    fn is_substream(stream: &TokenStream, other: &TokenStream) -> bool {
        let other_len = other.len();
        for i in 0..stream.len() {
            for (j, other_token) in other.trees().enumerate() {
                match stream.trees().nth(i + j) {
                    Some(token_tree) => {
                        if !token_tree.eq_unspanned(other_token) {
                            break;
                        }
                        // reached last index, so we have a match
                        if j == other_len - 1 {
                            return true;
                        }
                    }
                    None => return false, // reached end of stream
                }
            }
        }
        false
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
fn secure() {
    dylint_testing::ui_test_example(env!("CARGO_PKG_NAME"), "secure");
}

#[test]
fn recommended() {
    dylint_testing::ui_test_example(env!("CARGO_PKG_NAME"), "recommended");
}

#[test]
fn recommended_2() {
    dylint_testing::ui_test_example(env!("CARGO_PKG_NAME"), "recommended-2");
}
