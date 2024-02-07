#![feature(rustc_private)]
#![feature(box_patterns)]
#![warn(unused_extern_crates)]

use clippy_utils::{diagnostics::span_lint, match_def_path};
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
use solana_lints::paths;

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
    ///
    /// Use instead:
    ///
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
    /// ```
    ///
    /// **How the lint is implemented:**
    ///
    /// - For every function containing calls to `solana_program::program::invoke`
    /// - find the definition of `Instruction` argument passed to `invoke`; first argument
    /// - If the `Instruction` argument is result of a function call
    ///   - If the function is whitelisted, do not report; only functions defined in
    ///     `spl_token::instruction` are whitelisted.
    ///   - Else report the call to `invoke` as vulnerable
    /// - Else if the `Instruction` is initialized in the function itself
    ///   - find the assign statement assigning to the `program_id` field, assigning to
    ///     field at `0`th index
    ///   - find all the aliases of `program_id`. Use the rhs of the assignment as initial
    ///     alias and look for all assignments assigning to the locals recursively.
    ///   - If `program_id` is compared using any of aliases ignore the call to `invoke`.
    ///     - Look for calls to `core::cmp::PartialEq{ne, eq}` where one of arg is moved
    ///       from an alias.
    ///     - If one of the arg accesses `program_id` and if the basic block containing the
    ///       comparison dominates the basic block containing call to `invoke` ensuring the
    ///       `program_id` is checked in all execution paths Then ignore the call to `invoke`.
    ///     - Else report the call to `invoke`.
    ///   - Else report the call to `invoke`.
    pub ARBITRARY_CPI,
    Warn,
    "Finds unconstrained inter-contract calls"
}

impl<'tcx> LateLintPass<'tcx> for ArbitraryCpi {
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
                // The terminator is a call to a function; the function is defined and is not function pointer or function object
                // i.e The function is not copied or moved. Generic functions, trait methods are not Constant.
                if let TerminatorKind::Call {
                    func: func_operand,
                    args,
                    ..
                } = &t.kind;
                if let mir::Operand::Constant(box func) = func_operand;
                if let TyKind::FnDef(def_id, _callee_substs) = func.const_.ty().kind();
                then {
                    // Static call
                    let callee_did = *def_id;
                    // Calls `invoke`
                    if match_def_path(cx, callee_did, &paths::SOLANA_PROGRAM_INVOKE) {
                        // Get the `Instruction`, instruction is the first argument of `invoke` function.
                        let inst_arg = &args[0];
                        if let Operand::Move(p) = inst_arg {
                            // Check if the Instruction is returned from a whitelisted function (is_whitelist = true)
                            // if `Instruction` is defined in this function, find all the locals/places the program_id is defined
                            let (is_whitelist, programid_places) =
                                Self::find_program_id_for_instru(cx, body_mir, block_id, p);
                            let likely_programid_locals: Vec<Local> =
                                programid_places.iter().map(|pl| pl.local).collect();
                            // if not whitelisted, check if the program_id is compared using one of the locals.
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
        let mut cur_block = block;
        loop {
            // Walk the bb in reverse, starting with the terminator
            if let Some(t) = &body.basic_blocks[cur_block].terminator {
                // the terminator is a call; the return value of the call is assigned to `inst_arg.local`
                match &t.kind {
                    TerminatorKind::Call {
                        func: mir::Operand::Constant(box func),
                        destination: dest,
                        args,
                        ..
                    } if dest.local_or_deref_local() == Some(inst_arg.local) => {
                        if_chain! {
                            // function definition
                            if let TyKind::FnDef(def_id, _callee_substs) = func.const_.ty().kind();
                            // non-zero args are passed in the call
                            if !args.is_empty();
                            if let Operand::Copy(arg0_pl) | Operand::Move(arg0_pl) = &args[0];
                            then {
                                // in order to trace back to the call which creates the
                                // instruction, we have to trace through a call to Try::branch
                                // Expressions such as `call()?` with try operator will have Try::branch
                                // call and the first argument is the return value of actual call `call()`.
                                // If the call is Try::branch, look for the first arg which will have the return
                                // value of `call()`.
                                if match_def_path(cx, *def_id, &paths::CORE_BRANCH) {
                                    inst_arg = arg0_pl;
                                } else {
                                    // If this is not Try::branch, check if its a call to a function in `spl_token::instruction` module
                                    let path = cx.get_def_path(*def_id);
                                    let token_path =
                                        paths::SPL_TOKEN_INSTRUCTION.map(Symbol::intern);
                                    // if the instruction is constructed by a function in `spl_token::instruction`, assume program_id is checked
                                    if path.iter().take(2).eq(&token_path) {
                                        return (true, Vec::new());
                                    }
                                    // if the called function is not the whitelisted one, then we assume it to be vulnerable
                                    return (false, Vec::new());
                                }
                            }
                        }
                    }
                    _ => {}
                }
            }
            // check every statement
            for stmt in body.basic_blocks[cur_block].statements.iter().rev() {
                match &stmt.kind {
                    // if the statement assigns to `inst_arg`, update `inst_arg` to the rhs
                    StatementKind::Assign(box (assign_place, rvalue))
                        if assign_place.local == inst_arg.local =>
                    {
                        // Check if assign_place is assignment to a field. if not then this is not the initialization of the struct
                        // have to check further
                        if_chain! {
                            if assign_place.projection.len() == 1;
                            if let proj = assign_place.projection[0];
                            // the projection could be deref etc
                            if let ProjectionElem::Field(f, ty) = proj;
                            then {
                                // stmt is an assignment to a field.
                                // there will be 3 statements(for 3 fields), ensure this statement is assignment
                                // to the first field `program_id`
                                // Also, do not update inst_arg, as this is just field assignment.
                                if_chain! {
                                    // program_id is the first field; index = 0
                                    if f.index() == 0;
                                    if let Some(adtdef) = ty.ty_adt_def();
                                    if match_def_path(
                                        cx,
                                        adtdef.did(),
                                        &["solana_program", "pubkey", "Pubkey"],
                                    );
                                    then {
                                        if let Rvalue::Use(Operand::Copy(pl) | Operand::Move(pl))
                                        | Rvalue::Ref(_, _, pl) = rvalue
                                        {
                                            // found the program_id. now look for all assignments/aliases to program_id.
                                            let likely_program_id_aliases = Self::find_program_id_aliases(body, cur_block, pl);
                                            return (false, likely_program_id_aliases);
                                        }
                                    }
                                }
                            } else {
                                // inst_arg is defined using this statement. rvalue could store the actual value.
                                if let Rvalue::Use(Operand::Copy(pl) | Operand::Move(pl))
                                | Rvalue::Ref(_, _, pl) = rvalue
                                {
                                    inst_arg = pl;
                                }
                            }
                        }
                    }
                    _ => {}
                }
            }
            match preds.get(cur_block) {
                // traverse the CFG. Only predecessor is being considered.
                Some(cur_preds) if !cur_preds.is_empty() => cur_block = cur_preds[0],
                _ => {
                    break;
                }
            }
        }
        // we did not find the statement assigning to the program_id of `Instruction`. report as vulnerable
        (false, Vec::new())
    }

    fn find_program_id_aliases<'tcx>(
        body: &'tcx mir::Body<'tcx>,
        block: BasicBlock,
        mut id_arg: &Place<'tcx>,
    ) -> Vec<Place<'tcx>> {
        let preds = body.basic_blocks.predecessors();
        let mut cur_block = block;
        let mut likely_program_id_aliases = Vec::<Place>::new();
        likely_program_id_aliases.push(*id_arg);
        loop {
            // check every stmt
            for stmt in body.basic_blocks[cur_block].statements.iter().rev() {
                match &stmt.kind {
                    // if the statement assigns to `inst_arg`, update `inst_arg` to the rhs
                    StatementKind::Assign(box (assign_place, rvalue))
                        if assign_place.local_or_deref_local() == id_arg.local_or_deref_local() =>
                    {
                        if let Rvalue::Use(Operand::Copy(pl) | Operand::Move(pl))
                        | Rvalue::Ref(_, _, pl) = rvalue
                        {
                            id_arg = pl;
                            likely_program_id_aliases.push(*pl);
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
        likely_program_id_aliases
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
        let mut cur_block = block;
        loop {
            // check every statement
            if_chain! {
                // is terminator a call `core::cmp::PartialEq{ne, eq}`?
                if let Some(t) = &body.basic_blocks[cur_block].terminator;
                if let TerminatorKind::Call {
                    func: func_operand,
                    args,
                    ..
                } = &t.kind;
                if let mir::Operand::Constant(box func) = func_operand;
                if let TyKind::FnDef(def_id, _callee_substs) = func.const_.ty().kind();
                if match_def_path(cx, *def_id, &["core", "cmp", "PartialEq", "ne"])
                    || match_def_path(cx, *def_id, &["core", "cmp", "PartialEq", "eq"]);
                // check if any of the args accesses program_id
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
                        return body.basic_blocks.dominators().dominates(cur_block, block);
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
}

// We do not test the sealevel-attacks 'insecure' example, because it calls
// spl_token::instruction::transfer, which in newer versions of the crate, includes a program_id
// check.

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
