#![feature(rustc_private)]
#![feature(box_patterns)]
#![warn(unused_extern_crates)]

use if_chain::if_chain;
use rustc_hir::Body;
use rustc_lint::{LateContext, LateLintPass};

use rustc_middle::{
    mir,
    mir::{
        AggregateKind, BasicBlock, BinOp, Local, Operand, Place, ProjectionElem, Rvalue,
        StatementKind, TerminatorKind,
    },
    ty::Ty,
    ty::TyKind,
};

use clippy_utils::{
    diagnostics::span_lint, get_trait_def_id, match_def_path, ty::implements_trait,
};
mod paths;

extern crate rustc_hir;
extern crate rustc_middle;

dylint_linting::declare_late_lint! {
    /// **What it does:**
    /// Finds uses of solana_program::pubkey::PubKey::create_program_address that do not check the bump_seed
    ///
    /// **Why is this bad?**
    /// Generally for every seed there should be a canonical address, so the user should not be
    /// able to pick the bump_seed, since that would result in a different address.
    ///
    /// **Known problems:**
    /// False positives, since the bump_seed check may be within some other function (does not
    /// trace through function calls). The bump seed may be also be safely stored in an account but
    /// passed from another function.
    ///
    /// False negatives, since our analysis is not path-sensitive (the bump_seed check may not
    /// occur in all possible execution paths)
    ///
    /// **Example:**
    ///
    pub BUMP_SEED_CANONICALIZATION,
    Warn,
    "Finds calls to create_program_address that do not check the bump_seed"
}

impl<'tcx> LateLintPass<'tcx> for BumpSeedCanonicalization {
    fn check_body(&mut self, cx: &LateContext<'tcx>, body: &'tcx Body<'tcx>) {
        let hir_map = cx.tcx.hir();
        let body_did = hir_map.body_owner_def_id(body.id()).to_def_id();
        if !cx.tcx.def_kind(body_did).is_fn_like() || !cx.tcx.is_mir_available(body_did) {
            return;
        }
        let body_mir = cx.tcx.optimized_mir(body_did);
        let terminators = body_mir
            .basic_blocks()
            .iter_enumerated()
            .map(|(block_id, block)| (block_id, &block.terminator));
        for (_idx, (block_id, terminator)) in terminators.enumerate() {
            if_chain! {
                if let t = terminator.as_ref().unwrap();
                if let TerminatorKind::Call {
                    func: func_operand,
                    args,
                    destination: _,
                    cleanup: _,
                    ..
                } = &t.kind;
                if let mir::Operand::Constant(box func) = func_operand;
                if let TyKind::FnDef(def_id, _callee_substs) = func.literal.ty().kind();
                then {
                    // Static call
                    let callee_did = *def_id;
                    if match_def_path(cx, callee_did, &paths::CREATE_PROGRAM_ADDRESS) {
                        let seed_arg = &args[0];
                        if let Operand::Move(p) = seed_arg {
                            let (dataflow_state, likely_bump_places): (
                                BackwardDataflowState,
                                Vec<Place>,
                            ) = Self::find_bump_seed_for_seed_array(cx, body_mir, block_id, p);
                            let likely_bump_locals: Vec<Local> =
                                likely_bump_places.iter().map(|pl| pl.local).collect();
                            match dataflow_state {
                                BackwardDataflowState::Bump => {
                                    // If the bump seed is just passed in but didn't come from a
                                    // structure, look for equality checks that might show that
                                    // they try to constrain it.
                                    if !Self::is_bump_seed_checked(
                                        cx,
                                        body_mir,
                                        likely_bump_locals.as_ref(),
                                    ) {
                                        span_lint(
                                            cx,
                                            BUMP_SEED_CANONICALIZATION,
                                            t.source_info.span,
                                            "Bump seed may not be constrained. If stored in an account, use anchor's #[account(seed=..., bump=...)] macro instead",
                                        );
                                    }
                                }
                                BackwardDataflowState::NonAnchorStructContainingBump => {
                                    // Value came from a non-anchor struct. We will warn here
                                    // just to be safe, since we can't tell if this bump seed
                                    // is checked or not.
                                    span_lint(
                                            cx,
                                            BUMP_SEED_CANONICALIZATION,
                                            t.source_info.span,
                                            "Bump seed comes from structure, ensure it is constrained to a single value and not user-controlled.",
                                        );
                                }
                                BackwardDataflowState::AnchorStructContainingBump => {
                                    // Value came from an anchor struct. They should be using
                                    // the account macro for this.
                                    span_lint(
                                            cx,
                                            BUMP_SEED_CANONICALIZATION,
                                            t.source_info.span,
                                            "Bump seed comes from anchor Account, use anchor's #[account(seed=..., bump=...)] macro instead",
                                        );
                                }
                                _ => {}
                            }
                        }
                    }
                }
            }
        }
    }
}

fn is_anchor_account_struct<'tcx>(cx: &LateContext<'tcx>, deser_ty: Ty<'tcx>) -> bool {
    let mut account_deserialize = false;
    if let Some(anchor_trait_id) = get_trait_def_id(cx, &paths::ACCOUNT_DESERIALIZE) {
        account_deserialize = implements_trait(cx, deser_ty, anchor_trait_id, &[]);
    }
    account_deserialize
}

#[derive(Eq, PartialEq)]
enum BackwardDataflowState {
    SeedsArray,
    FirstSeed,
    Bump,
    AnchorStructContainingBump,
    NonAnchorStructContainingBump,
}

impl BumpSeedCanonicalization {
    fn find_bump_seed_for_seed_array<'tcx>(
        cx: &LateContext<'tcx>,
        body: &'tcx mir::Body<'tcx>,
        block: BasicBlock,
        mut seeds_arg: &Place<'tcx>,
    ) -> (BackwardDataflowState, Vec<Place<'tcx>>) {
        let preds = body.basic_blocks.predecessors();
        let bbs = body.basic_blocks();
        let mut cur_block = block;
        let mut state = BackwardDataflowState::SeedsArray;
        let mut likely_bump_seed_aliases = Vec::<Place>::new();
        loop {
            // check every statement
            for stmt in bbs[cur_block].statements.iter().rev() {
                if let StatementKind::Assign(box (assign_place, rvalue)) = &stmt.kind {
                    // trace assignments so we have a list of locals that contain the bump_seed
                    if assign_place.local_or_deref_local() == seeds_arg.local_or_deref_local() {
                        // println!("match: {:?}", stmt);
                        match rvalue {
                            Rvalue::Use(
                                Operand::Copy(rvalue_place) | Operand::Move(rvalue_place),
                            )
                            | Rvalue::Ref(_, _, rvalue_place)
                            | Rvalue::Cast(
                                _,
                                Operand::Copy(rvalue_place) | Operand::Move(rvalue_place),
                                _,
                            ) => {
                                seeds_arg = rvalue_place;
                                if state == BackwardDataflowState::Bump {
                                    likely_bump_seed_aliases.push(*rvalue_place);
                                }
                                if_chain! {
                                    if state == BackwardDataflowState::Bump;
                                    if let Some(proj) =
                                        rvalue_place.iter_projections().find_map(|(_, proj)| {
                                            match proj {
                                                ProjectionElem::Field(_, _) => Some(proj),
                                                _ => None,
                                            }
                                        });
                                    if let ProjectionElem::Field(_, _) = proj;
                                    then {
                                        state = if is_anchor_account_struct(
                                            cx,
                                            Place::ty_from(rvalue_place.local, &[], body, cx.tcx)
                                                .ty
                                                .peel_refs(),
                                        ) {
                                            BackwardDataflowState::AnchorStructContainingBump
                                        } else {
                                            BackwardDataflowState::NonAnchorStructContainingBump
                                        };
                                    }
                                }
                            }
                            Rvalue::Aggregate(box AggregateKind::Array(_), elements) => match state
                            {
                                BackwardDataflowState::SeedsArray if elements.len() > 1 => {
                                    if let Operand::Move(pl) = elements.last().unwrap() {
                                        seeds_arg = pl;
                                        state = BackwardDataflowState::FirstSeed;
                                    }
                                }
                                BackwardDataflowState::FirstSeed if elements.len() == 1 => {
                                    if let Operand::Move(pl) = &elements[0] {
                                        seeds_arg = pl;
                                        likely_bump_seed_aliases.push(*seeds_arg);
                                        state = BackwardDataflowState::Bump;
                                    }
                                }
                                _ => {}
                            },
                            _ => {}
                        }
                    }
                }
            }
            match preds.get(cur_block) {
                Some(cur_preds) if !cur_preds.is_empty() => cur_block = cur_preds[0],
                _ => {
                    break;
                }
            }
        }
        (state, likely_bump_seed_aliases)
    }

    // helper function
    // Given the Place search_place, check if it was defined using one of the locals in search_list
    fn is_moved_from<'tcx>(
        _: &LateContext,
        body: &'tcx mir::Body<'tcx>,
        block: BasicBlock,
        mut search_place: &Place<'tcx>,
        search_list: &[Local],
    ) -> bool {
        let preds = body.basic_blocks.predecessors();
        let bbs = body.basic_blocks();
        let mut cur_block = block;
        if let Some(search_loc) = search_place.local_or_deref_local() {
            if search_list.contains(&search_loc) {
                return true;
            }
        }
        loop {
            for stmt in bbs[cur_block].statements.iter().rev() {
                match &stmt.kind {
                    StatementKind::Assign(box (assign_place, rvalue))
                        if assign_place.local_or_deref_local()
                            == search_place.local_or_deref_local() =>
                    {
                        match rvalue {
                            Rvalue::Use(
                                Operand::Copy(rvalue_place) | Operand::Move(rvalue_place),
                            )
                            | Rvalue::Ref(_, _, rvalue_place) => {
                                // println!("Found assignment {:?}", stmt);
                                search_place = rvalue_place;
                                if let Some(search_loc) = search_place.local_or_deref_local() {
                                    if search_list.contains(&search_loc) {
                                        return true;
                                    }
                                }
                            }
                            _ => {}
                        }
                    }
                    _ => {}
                }
            }
            match preds.get(cur_block) {
                Some(cur_preds) if !cur_preds.is_empty() => cur_block = cur_preds[0],
                _ => {
                    break;
                }
            }
        }
        false
    }

    fn is_bump_seed_checked<'tcx>(
        cx: &LateContext,
        body: &'tcx mir::Body<'tcx>,
        bump_locals: &[Local],
    ) -> bool {
        for (block_id, block) in body.basic_blocks().iter_enumerated() {
            for stmt in &block.statements {
                if_chain! {
                    if let StatementKind::Assign(box (_, rvalue)) = &stmt.kind;
                    if let Rvalue::BinaryOp(BinOp::Eq | BinOp::Ne, box (op0, op1)) = rvalue;
                    if let Operand::Copy(arg0_pl) | Operand::Move(arg0_pl) = op0;
                    if let Operand::Copy(arg1_pl) | Operand::Move(arg1_pl) = op1;
                    then {
                        if Self::is_moved_from(cx, body, block_id, arg0_pl, bump_locals)
                            || Self::is_moved_from(cx, body, block_id, arg1_pl, bump_locals)
                        {
                            // we found the check
                            return true;
                        }
                    }
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
fn secure() {
    dylint_testing::ui_test_example(env!("CARGO_PKG_NAME"), "secure");
}

#[test]
fn insecure_2() {
    dylint_testing::ui_test_example(env!("CARGO_PKG_NAME"), "insecure-2");
}

#[test]
fn insecure_3() {
    dylint_testing::ui_test_example(env!("CARGO_PKG_NAME"), "insecure-3");
}

#[test]
fn recommended() {
    dylint_testing::ui_test_example(env!("CARGO_PKG_NAME"), "recommended");
}
