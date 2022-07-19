#![feature(rustc_private)]
#![warn(unused_extern_crates)]

extern crate rustc_ast;
extern crate rustc_hir;
extern crate rustc_middle;
extern crate rustc_span;

mod alternate_constraint;
mod anchor_constraint;

use crate::alternate_constraint::Values;
use crate::anchor_constraint::{
    create_key_check_constraint_tokenstream, get_anchor_account_type_def_id, get_def_id, Streams,
};

use std::collections::{HashMap, VecDeque};
use std::default::Default;

use rustc_ast::{AttrKind, Attribute, MacArgs};
use rustc_hir::{intravisit::FnKind, Body, FnDecl, HirId, VariantData};
use rustc_lint::{LateContext, LateLintPass};
use rustc_span::{def_id::DefId, symbol::Symbol, Span};

use clippy_utils::{diagnostics::span_lint_and_help, ty::match_type};
use if_chain::if_chain;
use solana_lints::paths;

const ANCHOR_ACCOUNT_GENERIC_ARG_COUNT: usize = 2;

dylint_linting::impl_late_lint! {
    /// **What it does:** Checks to make sure there is a key check on identical Anchor accounts.
    /// The key check serves to make sure that two identical accounts do not have the same key,
    /// ie, they are unique. An Anchor account (`Account<'info, T>`) is identical to another if
    /// the generic parameter `T` is the same type for each account.
    ///
    /// **Why is this bad?** If a program contains two identical, mutable Anchor accounts, and
    /// performs some operation on those accounts, then a user could pass in the same account
    /// twice. Then any previous operations may be overwritten by the last operation, which may
    /// not be what the program wanted if it expected different accounts.
    ///
    /// **Known problems:** If a program is not using the anchor #[account] macro constraints,
    /// and is instead using checks in the function bodies, and the program uses boolean operator
    /// && or || to link constraints in a single if statement, the lint will flag this as a false
    /// positive since the lint only catches statements with `==` or `!=`.
    /// Another issue is if a program uses an if statement such as `a.key() == b.key()` and then
    /// continues to modify the accounts, then this will not be caught. The reason is because the
    /// lint regards expressions with `==` as a secure check, since it assumes the program will
    /// then return an error (see the secure example). However, it does not explicitly check that
    /// an error is returned.
    ///
    /// In general, this lint will catch all vulnerabilities if the anchor macro constraints are
    /// used (see the recommended example). It is not as robust if alternative methods are utilized.
    /// Thus it is encouraged to use the anchor `#[account]` macro constraints.
    ///
    /// **Example:**
    ///
    /// ```rust
    /// #[derive(Accounts)]
    /// pub struct Update<'info> {
    ///     user_a: Account<'info, User>,
    ///     user_b: Account<'info, User>,
    /// }
    /// ```
    /// Use instead:
    /// ```rust
    /// #[derive(Accounts)]
    /// pub struct Update<'info> {
    ///     #[account(constraint = user_a.key() != user_b.key())]
    ///     user_a: Account<'info, User>,
    ///     user_b: Account<'info, User>,
    /// }
    /// ```
    pub DUPLICATE_MUTABLE_ACCOUNTS,
    Warn,
    "does not check if multiple identical Anchor accounts have different keys",
    DuplicateMutableAccounts::default()
}

#[derive(Default, Debug)]
struct DuplicateMutableAccounts {
    accounts: HashMap<DefId, Vec<(Symbol, Span)>>,
    streams: Streams,
    spans: Vec<(Span, Span)>,
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
            // get all mutable references to Accounts and if_statements in body
            let mut values = Values::new(cx);
            values.get_referenced_accounts_and_if_statements(body);

            // NOTE: could do this check in check_post_crate if exprs are replaced with HirId, then use
            // the HirId to fetch the expr
            values.accounts.values().for_each(|exprs| {
                if exprs.len() > 1 {
                    for current in 0..exprs.len() - 1 {
                        for next in current + 1..exprs.len() {
                            if !values.check_key_constraint(exprs[current], exprs[next]) {
                                // store for later spanning
                                self.spans.push((exprs[current].span, exprs[next].span));
                            }
                        }
                    }
                }
            });
        }
    }

    fn check_crate_post(&mut self, cx: &LateContext<'tcx>) {
        // if collected some anchor macro constraints then perform v1 lint
        if self.streams.0.is_empty() {
            // TODO: Not a fan of having it span lints for this check when there are no checks whatsoever.
            // I'd rather have it span lints to recommended anchor macros, if no checks are found at all
            for (first, second) in &self.spans {
                span_lint_and_help(
                    cx,
                    DUPLICATE_MUTABLE_ACCOUNTS,
                    *first,
                    &format!("the expressions on line {:?} and {:?} have identical Account types, yet do not contain a proper key check.", first, second),
                    Some(*second),
                    "add a key check to make sure the accounts have different keys, e.g., x.key() != y.key()",
                );
            }
        } else {
            for v in self.accounts.values() {
                if v.len() > 1 {
                    let mut deq = VecDeque::from(v.clone());
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
