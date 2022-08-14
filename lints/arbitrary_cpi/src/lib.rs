#![feature(rustc_private)]
#![feature(box_patterns)]
#![warn(unused_extern_crates)]

use if_chain::if_chain;
use rustc_hir::Body;
use rustc_lint::{LateContext, LateLintPass};

use rustc_middle::{
    mir,
    mir::{
        BasicBlock, Local, Operand, Place, ProjectionElem, Rvalue, StatementKind, TerminatorKind,
    },
    ty::TyKind,
};
use rustc_span::symbol::Symbol;

use clippy_utils::{diagnostics::span_lint, match_def_path};
mod paths;

extern crate rustc_hir;
extern crate rustc_middle;
extern crate rustc_span;

dylint_linting::declare_late_lint! {
    /// **What it does:**
    /// Finds uses of solana_program::program::invoke that do not check the program_id
    ///
    /// **Why is this bad?**
    /// A contract could call into an attacker-controlled contract instead of the intended one
    ///
    /// **Known problems:**
    /// False positives, since the program_id check may be within some other function (does not
    /// trace through function calls)
    /// False negatives, since our analysis is not path-sensitive (the program_id check may not
    /// occur in all possible execution paths)
    ///
    /// **Example:**
    ///
    /// ```rust
    /// // example code where a warning is issued
    /// let ix = Instruction {
    ///   program_id: *program_id,
    ///   accounts: vec![AccountMeta::new_readonly(*program_id, false)],
    ///   data: vec![0; 16],
    /// };
    /// invoke(&ix, accounts.clone());
    ///
    /// ```
    /// Use instead:
    /// ```rust
    /// // example code that does not raise a warning
    /// if (*program_id == ...) {
    ///     ...
    /// }
    /// let ix = Instruction {
    ///   program_id: *program_id,
    ///   accounts: vec![AccountMeta::new_readonly(*program_id, false)],
    ///   data: vec![0; 16],
    /// };
    /// invoke(&ix, accounts.clone());

    pub ARBITRARY_CPI,
    Warn,
    "Finds unconstrained inter-contract calls"
}

impl<'tcx> LateLintPass<'tcx> for ArbitraryCpi {
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
                    ..
                } = &t.kind;
                if let mir::Operand::Constant(box func) = func_operand;
                if let TyKind::FnDef(def_id, _callee_substs) = func.literal.ty().kind();
                then {
                    // Static call
                    let callee_did = *def_id;
                    if match_def_path(cx, callee_did, &paths::SOLANA_INVOKE) {
                        let inst_arg = &args[0];
                        if let Operand::Move(p) = inst_arg {
                            let (is_whitelist, programid_places) =
                                Self::find_program_id_for_instru(cx, body_mir, block_id, p);
                            let likely_programid_locals: Vec<Local> =
                                programid_places.iter().map(|pl| pl.local).collect();
                            if !is_whitelist
                                && !Self::is_programid_checked(
                                    cx,
                                    body_mir,
                                    block_id,
                                    likely_programid_locals.as_ref(),
                                )
                            {
                                span_lint(
                                    cx,
                                    ARBITRARY_CPI,
                                    t.source_info.span,
                                    "program_id may not be checked",
                                );
                            }
                        }
                    }
                }
            }
        }
    }
}

impl ArbitraryCpi {
    // This function is passed the Place corresponding to the 1st argument to invoke, and is
    // responsible for tracing the definition of that local back to the point where the instruction
    // is defined.
    //
    // We handle two cases:
    // 1. The instruction is initialized within this Body, in which case the returned
    //    likely_program_id_places will contain all the Places containing a program_id,
    //    and we can then look for comparisons with those places to see if the program id is
    //    checked.
    //
    // 2. The instruction passed to invoke is returned from a function call. In the general case,
    //    we want to raise a warning since the program ID still might not be checked (the function
    //    that is called may or may not check it). However, if this came from a call to
    //    spl_token::instruction::... then it will be checked and we will ignore it
    //
    //  Returns (is_whitelisted, likely_program_id_places)
    fn find_program_id_for_instru<'tcx>(
        cx: &LateContext,
        body: &'tcx mir::Body<'tcx>,
        block: BasicBlock,
        mut inst_arg: &Place<'tcx>,
    ) -> (bool, Vec<Place<'tcx>>) {
        let preds = body.basic_blocks.predecessors();
        let bbs = body.basic_blocks();
        let mut cur_block = block;
        let mut found_program_id = false;
        let mut likely_program_id_aliases = Vec::<Place>::new();
        loop {
            // Walk the bb in reverse, starting with the terminator
            if let Some(t) = &bbs[cur_block].terminator {
                match &t.kind {
                    TerminatorKind::Call {
                        func: mir::Operand::Constant(box func),
                        destination: dest,
                        args,
                        ..
                    } if dest.local_or_deref_local() == Some(inst_arg.local)
                        && !found_program_id =>
                    {
                        if_chain! {
                            if let TyKind::FnDef(def_id, _callee_substs) = func.literal.ty().kind();
                            if !args.is_empty();
                            if let Operand::Copy(arg0_pl) | Operand::Move(arg0_pl) = &args[0];
                            then {
                                // in order to trace back to the call which creates the
                                // instruction, we have to trace through a call to Try::branch
                                if match_def_path(cx, *def_id, &paths::TRY_BRANCH) {
                                    inst_arg = arg0_pl;
                                } else {
                                    let path = cx.get_def_path(*def_id);
                                    let token_path = paths::SPL_TOKEN.map(Symbol::intern);
                                    if path.iter().take(2).eq(&token_path) {
                                        return (true, likely_program_id_aliases);
                                    }
                                }
                            }
                        }
                    }
                    _ => {}
                }
            }
            // check every statement
            for stmt in bbs[cur_block].statements.iter().rev() {
                match &stmt.kind {
                    StatementKind::Assign(box (assign_place, rvalue))
                        if assign_place.local_or_deref_local()
                            == inst_arg.local_or_deref_local() =>
                    {
                        match rvalue {
                            Rvalue::Use(
                                Operand::Copy(rvalue_place) | Operand::Move(rvalue_place),
                            ) => {
                                // println!("Found assignment {:?}", stmt);
                                inst_arg = rvalue_place;
                                if found_program_id {
                                    likely_program_id_aliases.push(*rvalue_place);
                                }
                            }
                            Rvalue::Ref(_, _, pl) => {
                                // println!("Found assignment (ref) {:?}", pl);
                                inst_arg = pl;
                                if found_program_id {
                                    likely_program_id_aliases.push(*inst_arg);
                                }
                            }
                            _ => {}
                        }
                    }
                    StatementKind::Assign(box (assign_place, rvalue))
                        if assign_place.local == inst_arg.local =>
                    {
                        if_chain! {
                            // If we've found the Instruction that was passed to invoke, then
                            // field at index 0 will be the program_id
                            if assign_place.projection.len() == 1;
                            if let proj = assign_place.projection[0];
                            if let ProjectionElem::Field(f, ty) = proj;
                            if f.index() == 0;
                            if let Some(adtdef) = ty.ty_adt_def();
                            if match_def_path(
                                cx,
                                adtdef.did(),
                                &["solana_program", "pubkey", "Pubkey"],
                            );
                            then {
                                // We found the field
                                if let Rvalue::Use(Operand::Copy(pl) | Operand::Move(pl))
                                | Rvalue::Ref(_, _, pl) = rvalue
                                {
                                    inst_arg = pl;
                                    likely_program_id_aliases.push(*pl);
                                    // println!("Found program ID: {:?}", rvalue);
                                    found_program_id = true;
                                    break;
                                }
                            }
                        };
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
        // println!("Likely aliases: {:?}", likely_program_id_aliases);
        (false, likely_program_id_aliases)
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

    // This function takes the list of programid_locals and a starting block, and searches for a
    // check elsewhere in the Body that would compare the program_id with something else.
    fn is_programid_checked<'tcx>(
        cx: &LateContext,
        body: &'tcx mir::Body<'tcx>,
        block: BasicBlock,
        programid_locals: &[Local],
    ) -> bool {
        let preds = body.basic_blocks.predecessors();
        let bbs = body.basic_blocks();
        let mut cur_block = block;
        loop {
            // check every statement
            if_chain! {
                if let Some(t) = &bbs[cur_block].terminator;
                if let TerminatorKind::Call {
                    func: func_operand,
                    args,
                    ..
                } = &t.kind;
                if let mir::Operand::Constant(box func) = func_operand;
                if let TyKind::FnDef(def_id, _callee_substs) = func.literal.ty().kind();
                if match_def_path(cx, *def_id, &["core", "cmp", "PartialEq", "ne"])
                    || match_def_path(cx, *def_id, &["core", "cmp", "PartialEq", "eq"]);
                if let Operand::Copy(arg0_pl) | Operand::Move(arg0_pl) = args[0];
                if let Operand::Copy(arg1_pl) | Operand::Move(arg1_pl) = args[1];
                then {
                    // if either arg0 or arg1 came from one of the programid_locals, then we know
                    // this eq/ne check was operating on the program_id.
                    if Self::is_moved_from(cx, body, cur_block, &arg0_pl, programid_locals)
                        || Self::is_moved_from(cx, body, cur_block, &arg1_pl, programid_locals)
                    {
                        // we found the check. if it dominates the call to invoke, then the check
                        // is assumed to be sufficient!
                        return body
                            .basic_blocks
                            .dominators()
                            .is_dominated_by(block, cur_block);
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
        false
    }
}

// We do not test the sealevel-attacks 'insecure' example, because it calls
// spl_token::instruction::transfer, which in newer versions of the crate, includes a program_id
// check.

#[test]
fn insecure_2() {
    dylint_testing::ui_test_example(env!("CARGO_PKG_NAME"), "insecure_2");
}

#[test]
fn secure() {
    dylint_testing::ui_test_example(env!("CARGO_PKG_NAME"), "secure");
}

#[test]
fn recommended() {
    dylint_testing::ui_test_example(env!("CARGO_PKG_NAME"), "recommended");
}
