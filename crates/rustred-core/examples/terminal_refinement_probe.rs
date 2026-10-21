//! Read-only, fixed-prefix comparison of exact terminal-refinement execution.
//! Usage: terminal_refinement_probe STATE.rrbin WORKERS ROWS [BLOCK_ROWS]
//! Source loading and result encoding are outside the solver-only timer.
use rustred::reduction::terminal_relations::{
    TerminalRelationExecutionOptions, TerminalRelationExecutor, TerminalRelationLimits,
    TerminalRelationSession,
};
use std::sync::atomic::AtomicBool;
use std::time::Instant;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = std::env::args().skip(1).collect();
    if !(3..=4).contains(&args.len()) {
        return Err("expected STATE.rrbin WORKERS ROWS [BLOCK_ROWS]".into());
    }
    let workers: usize = args[1].parse()?;
    let rows: usize = args[2].parse()?;
    if rows == 0 {
        return Err("rows must be positive".into());
    }
    let load = Instant::now();
    let bytes = std::fs::read(&args[0])?;
    let limits = TerminalRelationLimits {
        max_seeds: usize::MAX,
        max_columns: usize::MAX,
        max_rows: usize::MAX,
        max_nonzeros: usize::MAX,
        ..Default::default()
    };
    let mut session =
        TerminalRelationSession::from_native_bytes(&bytes, limits, Default::default())?;
    let load_seconds = load.elapsed().as_secs_f64();
    let mut options = TerminalRelationExecutionOptions::new(workers);
    options.batch_rows = rows;
    if let Some(block) = args.get(3) {
        options.block_rows = block.parse()?;
    }
    let executor = TerminalRelationExecutor::new(options)?;
    let count = |s: &TerminalRelationSession| {
        let s = s.statistics();
        s.completed_source_rows + s.completed_rebuild_rows
    };
    let before = count(&session);
    let timer = Instant::now();
    while count(&session) - before < rows as u64 && !session.is_complete() {
        executor.step(&mut session, &AtomicBool::new(false))?;
    }
    let solve_seconds = timer.elapsed().as_secs_f64();
    let actual = count(&session) - before;
    let stats = session.statistics();
    let output = session.to_native_bytes(Default::default())?;
    let mut result = blake3::Hasher::new();
    // This diagnostic digest is not an artifact codec or algebraic authority.
    for (source, terms) in session.apply_all_terminals()? {
        result.update(format!("{:?}", source.powers()).as_bytes());
        for (key, value) in terms {
            result.update(format!("{:?}={value};", key.powers()).as_bytes());
        }
    }
    println!(
        "{{\"workers\":{workers},\"rows\":{actual},\"block_rows\":{},\"load_seconds\":{load_seconds},\"solver_seconds\":{solve_seconds},\"independent_rows\":{},\"nonzeros\":{},\"remaining\":{},\"state_blake3\":\"{}\",\"terminal_map_digest\":\"{}\"}}",
        options.block_rows,
        stats.independent_rows,
        stats.nonzeros,
        stats.remaining_terminals,
        blake3::hash(&output).to_hex(),
        result.finalize().to_hex()
    );
    Ok(())
}
