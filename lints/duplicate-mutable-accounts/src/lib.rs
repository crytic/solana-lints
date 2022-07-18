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
        } else {
            // perform alternate constraint check, e.g., check fn bodies, then check key checks
            self.check_fn()
        }

        // TODO: how to enforce that this is only called when necessary?
        fn check_fn(
            &mut self,
            cx: &LateContext<'tcx>,
            _: FnKind<'tcx>,
            _: &'tcx FnDecl<'tcx>,
            body: &'tcx Body<'tcx>,
            span: Span,
            _: HirId,
        ) {
            if !span.from_expansion() {
                let accounts = get_referenced_accounts(cx, body);
                
                accounts.values().for_each(|exprs| {
                    // TODO: figure out handling of >2 accounts
                    match exprs.len() {
                        2 => {
                            let first = exprs[0];
                            let second = exprs[1];
                            if !contains_key_call(cx, body, first) {
                                span_lint_and_help(
                                    cx,
                                    DUP_MUTABLE_ACCOUNTS_2,
                                    first.span,
                                    "this expression does not have a key check but has the same account type as another expression",
                                    Some(second.span),
                                    "add a key check to make sure the accounts have different keys, e.g., x.key() != y.key()",
                                );
                            }
                            if !contains_key_call(cx, body, second) {
                                span_lint_and_help(
                                    cx,
                                    DUP_MUTABLE_ACCOUNTS_2,
                                    second.span,
                                    "this expression does not have a key check but has the same account type as another expression",
                                    Some(first.span),
                                    "add a key check to make sure the accounts have different keys, e.g., x.key() != y.key()",
                                );
                            }
                        },
                        n if n > 2 => {
                            span_lint_and_note(
                                cx,
                                DUP_MUTABLE_ACCOUNTS_2,
                                exprs[0].span,
                                &format!("the following expression has the same account type as {} other accounts", exprs.len()),
                                None,
                                "might not check that each account has a unique key"
                            )
                        },
                        _ => {}
                    }
                });
            }
        }
    }
}

mod anchor_constraint_check {
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
        /// Returns true if `self` contains `other`, by comparing if there is an
        /// identical `TokenStream` in `self` regardless of span.
        fn contains(&self, other: &TokenStream) -> bool {
            self.0.iter().any(|stream| stream.eq_unspanned(other))
        }
    }
}


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

mod alternate_constraint_check {
    struct AccountUses<'cx, 'tcx> {
        cx: &'cx LateContext<'tcx>,
        uses: HashMap<DefId, Vec<&'tcx Expr<'tcx>>>,
    }
    
    fn get_referenced_accounts<'tcx>(
        cx: &LateContext<'tcx>,
        body: &'tcx Body<'tcx>,
    ) -> HashMap<DefId, Vec<&'tcx Expr<'tcx>>> {
        let mut accounts = AccountUses {
            cx,
            uses: HashMap::new(),
        };
    
        accounts.visit_expr(&body.value);
        accounts.uses
    }
    
    impl<'cx, 'tcx> Visitor<'tcx> for AccountUses<'cx, 'tcx> {
        fn visit_expr(&mut self, expr: &'tcx Expr<'tcx>) {
            if_chain! {
                // get mutable reference expressions
                if let ExprKind::AddrOf(_, mutability, mut_expr) = expr.kind;
                if let Mutability::Mut = mutability;
                // check type of expr == Account<'info, T>
                let middle_ty = self.cx.typeck_results().expr_ty(mut_expr);
                if match_type(self.cx, middle_ty, &paths::ANCHOR_ACCOUNT);
                // grab T generic parameter
                if let TyKind::Adt(_adt_def, substs) = middle_ty.kind();
                if substs.len() == ANCHOR_ACCOUNT_GENERIC_ARG_COUNT;
                let account_type = substs[1].expect_ty(); // TODO: could just store middle::Ty instead of DefId?
                if let Some(adt_def) = account_type.ty_adt_def();
                then {
                    let def_id = adt_def.did();
                    if let Some(exprs) = self.uses.get_mut(&def_id) {
                        let mut spanless_eq = SpanlessEq::new(self.cx);
                        // check that expr is not a duplicate within its particular key-pair
                        if exprs.iter().all(|e| !spanless_eq.eq_expr(e, mut_expr)) {
                            exprs.push(mut_expr);
                        }
                    } else {
                        self.uses.insert(def_id, vec![mut_expr]);
                    }
                }
            }
            walk_expr(self, expr);
        }
    }
    
    /// Performs a walk on `body`, checking whether there exists an expression that contains
    /// a `key()` method call on `account_expr`.
    fn contains_key_call<'tcx>(
        cx: &LateContext<'tcx>,
        body: &'tcx Body<'tcx>,
        account_expr: &Expr<'tcx>,
    ) -> bool {
        visit_expr_no_bodies(&body.value, |expr| {
            if_chain! {
                if let ExprKind::MethodCall(path_seg, exprs, _span) = expr.kind;
                if path_seg.ident.name.as_str() == "key";
                if !exprs.is_empty();
                let mut spanless_eq = SpanlessEq::new(cx);
                if spanless_eq.eq_expr(&exprs[0], account_expr);
                then {
                    true
                } else {
                    false
                }
            }
        })
    }
}

// /// Splits `stream` into a vector of substreams, separated by `delimiter`.
// fn split(stream: CursorRef, delimiter: TokenKind) -> Vec<TokenStream> {
//     let mut split_streams: Vec<TokenStream> = Vec::new();
//     let mut temp: Vec<TreeAndSpacing> = Vec::new();
//     let delim = TokenTree::Token(Token::new(delimiter, DUMMY_SP));

//     stream.for_each(|t| {
//         if t.eq_unspanned(&delim) {
//             split_streams.push(TokenStream::new(temp.clone()));
//             temp.clear();
//         } else {
//             temp.push(TreeAndSpacing::from(t.clone()));
//         }
//     });
//     split_streams.push(TokenStream::new(temp));
//     split_streams
// }

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
