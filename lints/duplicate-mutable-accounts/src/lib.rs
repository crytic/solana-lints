#![feature(rustc_private)]
#![warn(unused_extern_crates)]

extern crate rustc_ast;
extern crate rustc_hir;
extern crate rustc_span;

use rustc_ast::{
    token::{Delimiter, Token, TokenKind},
    tokenstream::{CursorRef, DelimSpan, TokenStream, TokenTree, TreeAndSpacing},
    AttrKind, Attribute, MacArgs,
};
use rustc_hir::def::Res;
use rustc_hir::*;
use rustc_lint::{LateContext, LateLintPass};
use rustc_span::{
    def_id::DefId,
    symbol::{Ident, Symbol},
    Span, DUMMY_SP,
};
use std::collections::{HashMap, VecDeque};
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
    streams: Streams,
}

impl<'tcx> LateLintPass<'tcx> for DuplicateMutableAccounts {
    // fn check_mod(
    //     &mut self,
    //     cx: &LateContext<'tcx>,
    //     _: &'tcx Mod<'tcx>,
    //     span: Span,
    //     _: HirId
    // ) {
    //     println!("new");
    //     for _ in 0..3 {
    //         println!("linting");
    //         span_lint_and_help(
    //             cx,
    //             DUPLICATE_MUTABLE_ACCOUNTS,
    //             span,
    //             "dummy",
    //             None,
    //             ""
    //         );
    //     }
    // }

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
            let name = attr_item.path.segments[0].ident.name; // TODO: can use name_or_empty
            // for some reason #[account] doesn't match when no args, maybe take away
            // the code to check name, and just check it has constraint args?
            if name.as_str() == "account";
            if let MacArgs::Delimited(_, _, token_stream) = &attr_item.args;
            then {
                for split in split(token_stream.trees(), TokenKind::Comma) {
                    // println!("{:#?}", split);
                    self.streams.0.push(split);
                }

            }
        }
    }

    fn check_crate_post(&mut self, cx: &LateContext<'tcx>) {
        // println!("{:#?}", self);
        for (_k, v) in self.accounts.iter() {
            if v.len() > 1 {
                // generate static set of possible constraints
                let gen_constraints = generate_possible_expected_constraints(v);

                for ((one, symmetric), symbols) in gen_constraints {
                    // println!("{:#?}\n {:#?}", one, symmetric);
                    if !(self.streams.contains(one) || self.streams.contains(symmetric)) {
                        println!("lint for {} {}", symbols.0, symbols.1);

                        // stupid way to get spans for offending types
                        let mut spans: Vec<Span> = Vec::new();
                        for (sym, span) in v {
                            if &symbols.0 == sym || &symbols.1 == sym {
                                spans.push(span.clone());
                            }
                        }

                        // TODO: for some reason, will only print out 2 messages, not 3
                        // println!("{:?}", spans);
                        span_lint_and_help(
                            cx,
                            DUPLICATE_MUTABLE_ACCOUNTS,
                            spans[0],
                            "identical account types without a key check constraint",
                            Some(spans[1]),
                            &format!("add an anchor key check constraint: #[account(constraint = {}.key() != {}.key())]", symbols.0, symbols.1)
                        );
                    }
                }
            }
        }
    }
}

fn get_anchor_account_type(segment: &PathSegment<'_>) -> Option<DefId> {
    if_chain! {
        // TODO: the following logic to get def_id is a repeated pattern
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

// collect elements into a TokenStream until encounter delim then stop collecting. create a new vec.
// continue until reaching end of stream
fn split(stream: CursorRef, delimiter: TokenKind) -> Vec<TokenStream> {
    let mut split_streams: Vec<TokenStream> = Vec::new();
    let mut temp: Vec<TreeAndSpacing> = Vec::new();
    let delim = TokenTree::Token(Token::new(delimiter, DUMMY_SP));

    stream.for_each(|t| {
        if t.eq_unspanned(&delim) {
            split_streams.push(TokenStream::new(temp.clone()));
            temp.clear();
        } else {
            temp.push(TreeAndSpacing::from(t.to_owned()));
        }
    });
    split_streams.push(TokenStream::new(temp));
    split_streams
}

/// Generates a static set of a possible expected key check constraints necessary for `values`.
fn generate_possible_expected_constraints(
    values: &Vec<(Symbol, Span)>,
) -> Vec<((TokenStream, TokenStream), (Symbol, Symbol))> {
    // TODO: may start with a VecDeque in the first place?
    let mut deq = VecDeque::from(values.clone());
    let mut gen_set = Vec::new();

    for _ in 0..deq.len() - 1 {
        let first = deq.pop_front().unwrap().0;
        // generate stream for all other values in vec
        for (other, _) in &deq {
            let stream = create_key_check_constraint_tokenstream(&first, other);
            let symmetric_stream = create_key_check_constraint_tokenstream(other, &first);
            // println!("{:#?}", stream);

            gen_set.push(((stream, symmetric_stream), (first, other.clone())));
        }
    }
    gen_set
}

// TODO: figure out more efficient way to do this
fn create_key_check_constraint_tokenstream(a: &Symbol, b: &Symbol) -> TokenStream {
    let constraint = vec![
        // TODO: test string matching by changing some string
        TreeAndSpacing::from(create_token("constraint")),
        TreeAndSpacing::from(TokenTree::Token(Token::new(TokenKind::Eq, DUMMY_SP))),
        TreeAndSpacing::from(create_token(a.as_str())),
        TreeAndSpacing::from(TokenTree::Token(Token::new(TokenKind::Dot, DUMMY_SP))),
        TreeAndSpacing::from(create_token("key")),
        TreeAndSpacing::from(TokenTree::Delimited(
            DelimSpan::dummy(),
            Delimiter::Parenthesis,
            TokenStream::new(vec![]),
        )),
        TreeAndSpacing::from(TokenTree::Token(Token::new(TokenKind::Ne, DUMMY_SP))),
        TreeAndSpacing::from(create_token(b.as_str())),
        TreeAndSpacing::from(TokenTree::Token(Token::new(TokenKind::Dot, DUMMY_SP))),
        TreeAndSpacing::from(create_token("key")),
        TreeAndSpacing::from(TokenTree::Delimited(
            DelimSpan::dummy(),
            Delimiter::Parenthesis,
            TokenStream::new(vec![]),
        )),
    ];

    TokenStream::new(constraint)
}

fn create_token(s: &str) -> TokenTree {
    let ident = Ident::from_str(s);
    TokenTree::Token(Token::from_ast_ident(ident))
}

#[derive(Debug, Default)]
pub struct Streams(Vec<TokenStream>);

impl Streams {
    fn contains(&self, other: TokenStream) -> bool {
        self.0.iter().any(|stream| stream.eq_unspanned(&other))
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
