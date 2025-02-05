#![feature(rustc_private)]
#![feature(box_patterns)]
#![warn(unused_extern_crates)]

use clippy_utils::{diagnostics::span_lint, match_any_def_paths, match_def_path};
use if_chain::if_chain;
use rustc_hir::Body;
use rustc_lint::{LateContext, LateLintPass};
use rustc_middle::{
    mir,
    mir::{
        AggregateKind, BasicBlock, Local, Operand, Place, Rvalue, Statement, StatementKind,
        TerminatorKind,
    },
    ty::{self, TyKind},
};
use solana_lints::paths;

extern crate rustc_hir;
extern crate rustc_middle;

dylint_linting::declare_late_lint! {
    /// **What it does:**
    /// Finds uses of solana_program::program::invoke that do not check the program_id
    ///
    /// **Why is this bad?**
    /// A contract could call into an attacker-controlled contract instead of the intended one
    ///
    /// **Works on:**
    ///
    /// - [x] Anchor
    /// - [x] Non Anchor
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
    /// - For every function
    ///   - For every statement in the function initializing `Instruction {..}`
    ///     - Get the place being assigned to `program_id` field
    ///     - find all the aliases of `program_id`. Use the rhs of the assignment as initial
    ///       alias and look for all assignments assigning to the locals recursively.
    ///     - If `program_id` is compared using any of aliases ignore the call to `invoke`.
    ///       - Look for calls to `core::cmp::PartialEq{ne, eq}` where one of arg is moved
    ///         from an alias.
    ///       - If one of the arg accesses `program_id` and if the basic block containing the
    ///         comparison dominates the basic block containing call to `invoke` ensuring the
    ///         `program_id` is checked in all execution paths Then ignore the call to `invoke`.
    ///       - Else report the statement initializing `Instruction`.
    ///     - Else report the statement initializing `Instruction`.
    ///   - For every call to `CpiContext::new` or `CpiContext::new_with_signer`
    ///     - Get the place of the first argument (program's account info)
    ///     - find all aliases of `program's` place.
    ///     - If the `program` is a result of calling `to_account_info` on Anchor `Program`/`Interface`
    ///       - continue
    ///     - Else report the call to `CpiContext::new`/`CpiContext::new_with_signer`
    pub ARBITRARY_CPI,
    Warn,
    "Finds unconstrained inter-contract calls"
}

impl<'tcx> LateLintPass<'tcx> for ArbitraryCpi {
    fn check_body(&mut self, cx: &LateContext<'tcx>, body: &Body<'tcx>) {
        if body.value.span.from_expansion() {
            return;
        }
        let hir_map = cx.tcx.hir();
        let body_did = hir_map.body_owner_def_id(body.id()).to_def_id();
        // The body is the body of function whose mir is available
        // fn_like includes fn, const fn, async fn but not closures.
        if !cx.tcx.def_kind(body_did).is_fn_like() || !cx.tcx.is_mir_available(body_did) {
            return;
        }
        let body_mir = cx.tcx.optimized_mir(body_did);
        // list of block id and the terminator of the basic blocks in the CFG
        for (block_id, block_data) in body_mir.basic_blocks.iter_enumerated() {
            // find the Instruction {...} initialization statements and check if program id is validated
            // Note: Because the lint looks for `Instruction {..}` construction instead of going through `invoke`, `invoke_signed`,
            // the lint will not report when the `Instruction` of `invoke` calls is returned by a function defined in a dependency.
            // However, if the `Instruction` is returned by a function defined in this crate then that function will get checked by the
            // lint and the statement initializing the `Instruction` will be reported.
            for stmt in &block_data.statements {
                if_chain! {
                    if let Some(program_id_place) = is_instruction_init_stmt(cx, stmt);
                    if !is_program_id_verified(cx, body_mir, block_id, &program_id_place);
                    then {
                        span_lint(
                            cx,
                            ARBITRARY_CPI,
                            stmt.source_info.span,
                            "program_id may not be checked",
                        )
                    }
                }
            }
            // if the terminator is a call to CpiContext::new or CpiContext::new_with_signer:
            //  - if program id is not verified
            //      - report error
            if let Some(t) = &block_data.terminator {
                if_chain! {
                    if let TerminatorKind::Call {
                        func: func_operand,
                        args,
                        ..
                    } = &t.kind;
                    if let mir::Operand::Constant(box func) = func_operand;
                    if let TyKind::FnDef(def_id, _callee_substs) = func.const_.ty().kind();
                    if match_any_def_paths(
                        cx,
                        *def_id,
                        &[
                            &paths::ANCHOR_CPI_CONTEXT_NEW,
                            &paths::ANCHOR_CPI_CONTEXT_NEW_SIGNER,
                        ],
                    )
                    .is_some();
                    if let Operand::Move(program_place) = &args[0].node;
                    if !is_program_safe_account_info(cx, body_mir, block_id, program_place);
                    then {
                        span_lint(
                            cx,
                            ARBITRARY_CPI,
                            t.source_info.span,
                            "program_id may not be checked",
                        )
                    }
                }
            }
        }
    }
}

/// Return the place of program id if the statement initializes Instruction i.e stmt is _x = Instruction {...}
fn is_instruction_init_stmt<'tcx>(cx: &LateContext, stmt: &Statement<'tcx>) -> Option<Place<'tcx>> {
    if_chain! {
        if let StatementKind::Assign(box (_, rvalue)) = &stmt.kind;
        // The MIR generated for the `insecure-2` and other programs shows that the entire struct is initialized at once.
        // Note: Its unknown in what cases the struct initialization is deaggregated. Assuming here that
        // the struct is initialized at once till a counter example is found.
        if let Rvalue::Aggregate(box AggregateKind::Adt(def_id, variant_idx, _, _, _), fields) =
            rvalue;
        // The Adt is a struct
        if variant_idx.index() == 0;
        // The struct is `solana_program::instruction::Instruction`
        if match_def_path(cx, *def_id, &paths::SOLANA_PROGRAM_INSTRUCTION);
        // program id is the first field. Assuming its operand is at the start of the fields IndexVec.
        if let Some(Operand::Move(pl) | Operand::Copy(pl)) = fields.iter().next();
        then {
            Some(*pl)
        } else {
            None
        }
    }
}

/// Given the place corresponding to `program_id` of CPI call, return true if `program_id` is validated else false
///
/// The `program_id` is the place of operand used to initialize `Instruction`:
///   - `let _x = Instruction { program_id: program_id_place, accounts: _, data: _ }`
fn is_program_id_verified<'tcx>(
    cx: &LateContext,
    body: &'tcx mir::Body<'tcx>,
    block_id: BasicBlock,
    program_id_place: &Place<'tcx>,
) -> bool {
    let program_id_aliases = find_place_aliases(body, block_id, program_id_place);
    let likely_program_id_locals: Vec<Local> =
        program_id_aliases.iter().map(|pl| pl.local).collect();
    is_programid_checked(cx, body, block_id, likely_program_id_locals.as_ref())
}

/// Given the place corresponding to `program` account info, return true if the `AccountInfo` is of a `Program`.
fn is_program_safe_account_info<'tcx>(
    cx: &LateContext<'tcx>,
    body: &'tcx mir::Body<'tcx>,
    block_id: BasicBlock,
    program_place: &Place<'tcx>,
) -> bool {
    let program_aliases = find_place_aliases(body, block_id, program_place);
    // This function at the moment only checks if the program is a result of calling `to_account_info`.
    // The aliases returned by `find_place_aliases` are of form where there is an assignment statement `alias[i] = alias[i+1]`.
    // As we are only looking for `to_account_info` calls, it is sufficient to check for assignment to the last alias.
    let program = program_aliases.last().unwrap();

    for (_, block_data) in body.basic_blocks.iter_enumerated() {
        match &block_data.terminator.as_ref().unwrap().kind {
            TerminatorKind::Call {
                func: mir::Operand::Constant(box func),
                destination: dest,
                args,
                ..
            } if dest.local_or_deref_local() == program.local_or_deref_local() => {
                if_chain! {
                    // the func is a call to `.to_account_info()` on type `Program` or `Interface`
                    if let TyKind::FnDef(def_id, _) = func.const_.ty().kind();
                    if match_def_path(cx, *def_id, &paths::ANCHOR_LANG_TO_ACCOUNT_INFO);
                    if !args.is_empty();
                    if let Operand::Copy(arg0_pl) | Operand::Move(arg0_pl) = &args[0].node;
                    if let ty::Adt(adt_def, _) = arg0_pl.ty(body, cx.tcx).ty.peel_refs().kind();
                    if match_any_def_paths(
                        cx,
                        adt_def.did(),
                        &[&paths::ANCHOR_LANG_PROGRAM, &paths::ANCHOR_LANG_INTERFACE],
                    )
                    .is_some();
                    then {
                        // The program is a result of calling `to_account_info` on `Program` or `Interface`
                        return true;
                    }
                }
            }
            _ => {}
        }
    }
    false
}

/// Given a place, find other places which are an alias to this place
fn find_place_aliases<'tcx>(
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
            if let Operand::Copy(arg0_pl) | Operand::Move(arg0_pl) = args[0].node;
            if let Operand::Copy(arg1_pl) | Operand::Move(arg1_pl) = args[1].node;
            then {
                // if either arg0 or arg1 came from one of the programid_locals, then we know
                // this eq/ne check was operating on the program_id.
                if is_moved_from(cx, body, cur_block, &arg0_pl, programid_locals)
                    || is_moved_from(cx, body, cur_block, &arg1_pl, programid_locals)
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
                        Rvalue::Use(Operand::Copy(rvalue_place) | Operand::Move(rvalue_place))
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
