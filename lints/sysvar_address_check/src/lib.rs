#![feature(rustc_private)]
#![warn(unused_extern_crates)]
#![recursion_limit = "256"]

extern crate rustc_hir;
extern crate rustc_middle;
extern crate rustc_span;

use rustc_hir::{
    def::Res,
    intravisit::{walk_expr, FnKind, Visitor},
    Body, Expr, ExprKind, FieldDef, FnDecl, GenericArg, HirId, QPath, TyKind as HirTyKind,
};
use rustc_lint::{LateContext, LateLintPass};
use rustc_middle::ty::TyKind;
use rustc_span::Span;

use clippy_utils::{
    diagnostics::span_lint_and_help,
    get_trait_def_id, match_def_path,
    ty::{implements_trait, match_type},
};
use if_chain::if_chain;
use solana_lints::paths;

dylint_linting::declare_late_lint! {
    /// **What it does:** This lint checks to ensure that programs using Solana types that derive the
    /// `Sysvar` trait (e.g. Rent, Clock) check the address of the account. The recommended way to
    /// deal with these types is using the `from_account_info()` method from the `Sysvar` trait.
    /// This method performs the ID check and only deserializes from an `AccountInfo` if the check
    /// passes, and is thus secure.

    /// This lint catches direct calls to deserialize (via `bincode::deserialize`) a byte array into
    /// a type deriving Sysvar. Furthermore, if using the Anchor framework, this lint will catch
    /// uses of `Account<'info, T>`, where `T` derives `Sysvar`. This is insecure since Anchor
    /// will not perform the ID check in this case.
    ///
    /// **Why is this bad?** If a program deserializes an `AccountInfo.data` directly, without
    /// checking the ID first, a malicious user could pass in an `AccountInfo` with spoofed data
    /// and the same structure as a `Sysvar` type. Then the program would be dealing with incorrect
    /// data.
    ///
    /// **Known problems:** This lint will flag any calls to deserialize some bytes into a type deriving
    /// `Sysvar`, regardless of whether the ID check is done or not. Thus, if a program manually does the ID
    /// check and deserialization, the lint will still flag this as insecure, thus possibly generating
    /// some false positives. However, one should really prefer to use `from_account_info()`.
    ///
    /// **Example:**
    ///
    /// ```rust
    /// pub fn check_sysvar_address(ctx: Context<CheckSysvarAddress>) -> Result<()> {
    ///     let rent: Rent = bincode::deserialize(&ctx.accounts.rent.data.borrow()).unwrap();
    ///     msg!("Rent -> {}", rent.lamports_per_byte_year);
    ///     Ok(())
    /// }
    /// ```
    /// Use instead:
    /// ```rust
    /// pub fn check_sysvar_address(ctx: Context<CheckSysvarAddress>) -> Result<()> {
    ///     let rent = Rent::from_account_info(&ctx.accounts.rent).unwrap();
    ///     msg!("Rent -> {}", rent.lamports_per_byte_year);
    ///     Ok(())
    /// }
    /// ```
    pub SYSVAR_ADDRESS_CHECK,
    Warn,
    "missing address check for Sysvar types"
}

impl<'tcx> LateLintPass<'tcx> for SysvarAddressCheck {
    fn check_field_def(&mut self, cx: &LateContext<'tcx>, field: &'tcx FieldDef<'tcx>) {
        // if field is Anchor Account<'info, T>
        // grab type T and if it derives Sysvar trait, flag lint
        if_chain! {
            // check field is type Account<'info, T>
            if let HirTyKind::Path(qpath) = &field.ty.kind;
            let res = cx.qpath_res(qpath, field.hir_id);
            if let Res::Def(_, def_id) = res;
            let middle_ty = cx.tcx.type_of(def_id);
            if match_type(cx, middle_ty, &paths::ANCHOR_ACCOUNT);
            // grab type T
            if let QPath::Resolved(_, path) = qpath;
            if !path.segments.is_empty();
            if let Some(generic_args) = &path.segments[0].args;
            if generic_args.args.len() > 1;
            if let GenericArg::Type(ty) = &generic_args.args[1];
            if let HirTyKind::Path(ty_qpath) = &ty.kind;
            let ty_res = cx.qpath_res(ty_qpath, ty.hir_id);
            if let Res::Def(_, type_def_id) = ty_res;
            let account_type = cx.tcx.type_of(type_def_id);
            // check if T derives Sysvar trait
            if let Some(trait_id) = get_trait_def_id(cx, &paths::SOLANA_SYSVAR_TRAIT);
            if implements_trait(cx, account_type, trait_id, &[]);
            then {
                span_lint_and_help(
                    cx,
                    SYSVAR_ADDRESS_CHECK,
                    field.span,
                    &format!(
                        "Anchor Account type T is '{}', which derives the Sysvar trait",
                        account_type
                    ),
                    None,
                    &format!(
                        "Account type does not perform an ID check. Use Sysvar<'info, {}> instead",
                        account_type
                    ),
                );
            }
        }
    }

    fn check_fn(
        &mut self,
        cx: &LateContext<'tcx>,
        _: FnKind<'tcx>,
        _: &'tcx FnDecl<'tcx>,
        body: &'tcx Body<'tcx>,
        _: Span,
        _: HirId,
    ) {
        // 1. walk function body and search for calls to bincode::deserialize
        // 2. retrieve the type of this expression (which is what is being deserialized to),
        // and check that the type implements the Sysvar trait
        // 3. if so, flag the lint and issue warning that user should not deserialize directly,
        // but instead use from_account_info() method from Sysvar trait
        let mut accounts = AccountUses { cx };
        accounts.visit_expr(&body.value);
    }
}

struct AccountUses<'cx, 'tcx> {
    cx: &'cx LateContext<'tcx>,
}

impl<'cx, 'tcx> Visitor<'tcx> for AccountUses<'cx, 'tcx> {
    fn visit_expr(&mut self, expr: &'tcx Expr<'tcx>) {
        if_chain! {
            // check if bincode::deserialize call
            if let ExprKind::Call(fnc_expr, _args_expr) = expr.kind;
            if let ExprKind::Path(qpath) = &fnc_expr.kind;
            let res = self.cx.qpath_res(qpath, fnc_expr.hir_id);
            if let Res::Def(_, def_id) = res;
            if match_def_path(self.cx, def_id, &paths::BINCODE_DESERIALIZE);
            // check type of expr
            let ty = self.cx.typeck_results().expr_ty(expr);
            // assumes type is always Result type, which should be the case
            if let TyKind::Adt(_, substs) = ty.kind();
            if !substs.is_empty();
            let deser_type = substs[0].expect_ty();
            // check type implements Sysvar
            if let Some(trait_id) = get_trait_def_id(self.cx, &paths::SOLANA_SYSVAR_TRAIT);
            if implements_trait(self.cx, deser_type, trait_id, &[]);
            then {
                span_lint_and_help(
                    self.cx,
                    SYSVAR_ADDRESS_CHECK,
                    expr.span,
                    "raw deserialization of a type that implements Sysvar",
                    None,
                    "use from_account_info() instead",
                );
            }
        }
        walk_expr(self, expr);
    }
}

// Not checking sealevel insecure case because in its current form, it is technically not even
// insecure. It does not deserialize from `rent.data`, thus possibly incorrectly assuming that
// this is a Rent struct. It is insecure in the sense there is no key check.

// Not testing sealevel secure case because this lint will flag any attempt to do a "raw"
// deserialization. The canonical way should be using from_account_info().

#[test]
fn insecure_2() {
    dylint_testing::ui_test_example(env!("CARGO_PKG_NAME"), "insecure-2");
}

#[test]
fn secure_2() {
    dylint_testing::ui_test_example(env!("CARGO_PKG_NAME"), "secure-2");
}

#[test]
fn insecure_anchor() {
    dylint_testing::ui_test_example(env!("CARGO_PKG_NAME"), "insecure-anchor");
}

#[test]
fn recommended() {
    dylint_testing::ui_test_example(env!("CARGO_PKG_NAME"), "recommended");
}
