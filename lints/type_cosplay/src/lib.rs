#![feature(rustc_private)]
#![warn(unused_extern_crates)]

extern crate rustc_ast;
extern crate rustc_ast_pretty;
extern crate rustc_data_structures;
extern crate rustc_errors;
extern crate rustc_hir;
extern crate rustc_hir_pretty;
extern crate rustc_index;
extern crate rustc_infer;
extern crate rustc_lexer;
extern crate rustc_middle;
extern crate rustc_mir_dataflow;
extern crate rustc_parse;
extern crate rustc_parse_format;
extern crate rustc_span;
extern crate rustc_target;
extern crate rustc_trait_selection;
extern crate rustc_typeck;

use rustc_lint::{LateContext, LateLintPass};
use rustc_hir::{Item, VariantData};

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
    // if there are 2 structs that have the same fields, then flag lint
    // CONSIDERATIONS:
    // If it is semantically meant for 2 types to have the same fields, then adding
    // an arbitrary "dummy" field is weird. It makes more sense to add a "discriminant"
    // field, as is suggested. But then there needs to also be a check on this discriminant
    // field, as in the secure example. Should we enforce this check? Or is that another lint's job?

    // or post?
    // fn check_struct_def_post(&mut self, cx: &LateContext<'tcx>, variant_data: &'tcx VariantData<'tcx>) {
    //     println!("{:?}", variant_data);
    // }

    fn check_item(&mut self, cx: &LateContext<'tcx>, item: &'tcx Item<'tcx>) {
        // collect struct definitions
        //  while collecting, if encountered an equivalent struct def, flag lint
        if let ItemKind::Struct(_, _) = item.kind {
            println!("{:?}", item);
        }
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

