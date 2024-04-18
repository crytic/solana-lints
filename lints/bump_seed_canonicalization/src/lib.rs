#![feature(rustc_private)]
#![feature(box_patterns)]
#![warn(unused_extern_crates)]

use clippy_utils::{
    diagnostics::span_lint, get_trait_def_id, match_def_path, ty::implements_trait,
};
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
use rustc_target::abi::FieldIdx;
use solana_lints::paths;

extern crate rustc_hir;
extern crate rustc_middle;
extern crate rustc_target;

dylint_linting::declare_late_lint! {
    /// **What it does:**
    ///
    /// Finds uses of solana_program::pubkey::PubKey::create_program_address that do not check the bump_seed
    ///
    /// **Why is this bad?**
    ///
    /// Generally for every seed there should be a canonical address, so the user should not be
    /// able to pick the bump_seed, since that would result in a different address.
    ///
    /// See https://github.com/crytic/building-secure-contracts/tree/master/not-so-smart-contracts/solana/improper_pda_validation
    ///
    /// **Works on:**
    ///
    /// - [ ] Anchor
    /// - [x] Non Anchor
    ///
    /// **Known problems:**
    ///
    /// False positives, since the bump_seed check may be within some other function (does not
    /// trace through function calls). The bump seed may be also be safely stored in an account but
    /// passed from another function.
    ///
    /// False negatives, since our analysis is not path-sensitive (the bump_seed check may not
    /// occur in all possible execution paths)
    ///
    /// **Example:**
    ///
    /// See https://github.com/coral-xyz/sealevel-attacks/blob/master/programs/7-bump-seed-canonicalization/insecure/src/lib.rs for an insecure example
    ///
    /// Use instead:
    ///
    /// See https://github.com/coral-xyz/sealevel-attacks/blob/master/programs/7-bump-seed-canonicalization/recommended/src/lib.rs for recommended way to use bump.
    ///
    /// **How the lint is implemented:**
    ///
    /// - For every function containing calls to `solana_program::pubkey::Pubkey::create_program_address`
    /// - find the `bump` location from the first argument to `create_program_address` call.
    ///   - first argument is the seeds array(`&[&[u8]]`). In general, the seeds are structured with bump as last element:
    ///     `&[seed1, seed2, ..., &[bump]]` e.g `&[b"vault", &[bump]]`.
    ///   - find the locations of bump.
    ///   - If bump is assigned by accessing a struct field
    ///     - if bump is assigned from a struct implementing `AnchorDeserialize` trait
    ///       - report a warning to use `#[account(...)` macro
    ///     - else report "bump may not be constrainted" warning
    ///   - else if the bump is checked using a comparison operation; do not report
    ///   - else report a warning
    pub BUMP_SEED_CANONICALIZATION,
    Warn,
    "Finds calls to create_program_address that do not check the bump_seed"
}

impl<'tcx> LateLintPass<'tcx> for BumpSeedCanonicalization {
    fn check_body(&mut self, cx: &LateContext<'tcx>, body: &'tcx Body<'tcx>) {
        let hir_map = cx.tcx.hir();
        let body_did = hir_map.body_owner_def_id(body.id()).to_def_id();
        // The body is the body of function whose mir is available
        // fn_like includes fn, const fn, async fn but not closures.
        if !cx.tcx.def_kind(body_did).is_fn_like() || !cx.tcx.is_mir_available(body_did) {
            return;
        }
        let body_mir = cx.tcx.optimized_mir(body_did);
        // list of block id and the terminator of the basic blocks in the CFG
        let terminators = body_mir
            .basic_blocks
            .iter_enumerated()
            .map(|(block_id, block)| (block_id, &block.terminator));
        for (block_id, terminator) in terminators {
            if_chain! {
                if let t = terminator.as_ref().unwrap();
                // The terminator is call to a function
                if let TerminatorKind::Call {
                    func: func_operand,
                    args,
                    destination: _,
                    ..
                } = &t.kind;
                if let mir::Operand::Constant(box func) = func_operand;
                if let TyKind::FnDef(def_id, _callee_substs) = func.const_.ty().kind();
                then {
                    // Static call
                    let callee_did = *def_id;
                    // called function is `solana_program::pubkey::Pubkey::create_program_address`
                    if match_def_path(
                        cx,
                        callee_did,
                        &paths::SOLANA_PROGRAM_CREATE_PROGRAM_ADDRESS,
                    ) {
                        // get the seeds argument; seeds is the first argument
                        let seed_arg = &args[0];
                        if let Operand::Move(p) = seed_arg {
                            // find all alias of bump in the seeds array: &[seed1, ..., &[bump]].
                            let (dataflow_state, likely_bump_places): (
                                BackwardDataflowState,
                                Vec<Place>,
                            ) = Self::find_bump_seed_for_seed_array(cx, body_mir, block_id, p);
                            let likely_bump_locals: Vec<Local> =
                                likely_bump_places.iter().map(|pl| pl.local).collect();
                            match dataflow_state {
                                // found the location of bump
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
                                // bump value is accessed from a struct which does not implement AnchorDeserialize trait
                                // non anchor struct => not part of state
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
                                // TODO: Should we report this???
                                // bump for one anchor account might be stored in a different account, it might not be
                                // always possible to use the #[account(...)] macro
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

/// Return true if the `deser_ty` implements `anchor::AccountDeserialize` trait else false
fn is_anchor_account_struct<'tcx>(cx: &LateContext<'tcx>, deser_ty: Ty<'tcx>) -> bool {
    let mut account_deserialize = false;
    if let Some(anchor_trait_id) = get_trait_def_id(cx, &paths::ANCHOR_LANG_ACCOUNT_DESERIALIZE) {
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
    /// Given the `seeds_arg`, a location passed to first argument of `create_program_address`,
    /// find all locations/alias of bump: `&[seed1, .., &[bump]]`
    fn find_bump_seed_for_seed_array<'tcx>(
        cx: &LateContext<'tcx>,
        body: &'tcx mir::Body<'tcx>,
        block: BasicBlock,
        mut seeds_arg: &Place<'tcx>,
    ) -> (BackwardDataflowState, Vec<Place<'tcx>>) {
        let preds = body.basic_blocks.predecessors();
        let mut cur_block = block;
        let mut state = BackwardDataflowState::SeedsArray;
        let mut likely_bump_seed_aliases = Vec::<Place>::new();
        loop {
            // check every statement
            for stmt in body.basic_blocks[cur_block].statements.iter().rev() {
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
                                // if seed_arg = x then trace for assignments of x
                                seeds_arg = rvalue_place;
                                // state is Bump => seed_arg stores the bump
                                if state == BackwardDataflowState::Bump {
                                    likely_bump_seed_aliases.push(*rvalue_place);
                                }
                                if_chain! {
                                    // if seed_arg stores bump and rvalue is such that `x.y` (field access)
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
                                        // if the bump is accessed from a Anchor struct (representing program state)
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
                            // rhs is array
                            Rvalue::Aggregate(box AggregateKind::Array(_), elements) => match state
                            {
                                BackwardDataflowState::SeedsArray if elements.len() > 1 => {
                                    // if seeds_arg stores the `seeds` location, find the location of bump
                                    // bump is the last element: [seed1, seed2, ..., bump]
                                    if let Operand::Move(pl) = elements.into_iter().last().unwrap()
                                    {
                                        // update the seeds_arg to point to pl and update the state
                                        seeds_arg = pl;
                                        state = BackwardDataflowState::FirstSeed;
                                    }
                                }
                                BackwardDataflowState::FirstSeed if elements.len() == 1 => {
                                    // seeds_arg points to bump array [ seed1, ..., &[bump]. seeds_arg stores
                                    // the location of &[bump]. update it to store the location of bump.
                                    if let Operand::Move(pl) = &elements[FieldIdx::from_u32(0)] {
                                        // store the location of bump
                                        seeds_arg = &pl;
                                        likely_bump_seed_aliases.push(*seeds_arg);
                                        // seeds_arg is a location of bump
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
        let mut cur_block = block;
        if let Some(search_loc) = search_place.local_or_deref_local() {
            if search_list.contains(&search_loc) {
                return true;
            }
        }
        // look for chain of assign statements whose value is eventually assigned to the `search_place` and
        // see if any of the intermediate local is in the search_list.
        // TODO: move this and ArbitraryCPI::is_moved_from to utils.
        loop {
            for stmt in body.basic_blocks[cur_block].statements.iter().rev() {
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

    // This function takes the list of bump_locals and a starting block, and searches for a
    // check elsewhere in the Body that would compare the program_id with something else.
    fn is_bump_seed_checked<'tcx>(
        cx: &LateContext,
        body: &'tcx mir::Body<'tcx>,
        bump_locals: &[Local],
    ) -> bool {
        for (block_id, block) in body.basic_blocks.iter_enumerated() {
            for stmt in &block.statements {
                if_chain! {
                    // look for assign statements
                    if let StatementKind::Assign(box (_, rvalue)) = &stmt.kind;
                    // check if rhs is comparison between bump and some other value.
                    if let Rvalue::BinaryOp(BinOp::Eq | BinOp::Ne, box (op0, op1)) = rvalue;
                    if let Operand::Copy(arg0_pl) | Operand::Move(arg0_pl) = op0;
                    if let Operand::Copy(arg1_pl) | Operand::Move(arg1_pl) = op1;
                    then {
                        // Check if one of the args in comparison came from a local of bump
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
