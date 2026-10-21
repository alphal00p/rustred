//! Work-stealing preparation and block elimination, with one ordered authority.
//!
//! Every local row is an exact combination of its original source, the frozen
//! global U, and earlier rows in its own block. All independent residuals,
//! including auxiliary columns, return to the coordinator in source order.
//! Symbolica performs every elimination. Only graph reachability, column maps
//! and scheduling live here. In-flight blocks are disposable checkpoint scratch.
use super::*;
use crate::solver::SectorExecutor;
use rayon::prelude::*;
use std::sync::atomic::AtomicUsize;
use std::time::Duration;
use symbolica::tensors::sparse::SparseMatrix;

#[derive(Clone, Copy, Debug)]
pub struct TerminalRelationExecutionProgress {
    pub stage: &'static str,
    pub rows_total: usize,
    pub rows_prepared: usize,
    pub rows_eliminated: usize,
    pub rows_committed: usize,
    pub blocks_completed: usize,
}
#[derive(Default)]
struct Progress {
    stage: AtomicUsize,
    total: AtomicUsize,
    prepared: AtomicUsize,
    eliminated: AtomicUsize,
    committed: AtomicUsize,
    blocks: AtomicUsize,
}
impl Progress {
    fn snapshot(&self) -> TerminalRelationExecutionProgress {
        TerminalRelationExecutionProgress {
            stage: match self.stage.load(Ordering::Relaxed) {
                0 => "preparing rows",
                1 => "mapping columns",
                2 => "parallel elimination",
                _ => "ordered merge",
            },
            rows_total: self.total.load(Ordering::Relaxed),
            rows_prepared: self.prepared.load(Ordering::Relaxed),
            rows_eliminated: self.eliminated.load(Ordering::Relaxed),
            rows_committed: self.committed.load(Ordering::Relaxed),
            blocks_completed: self.blocks.load(Ordering::Relaxed),
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct TerminalRelationExecutionOptions {
    pub workers: usize,
    /// Operational window, not a limit on the finite search inventory.
    pub batch_rows: usize,
    /// Smaller blocks expose more stealable jobs; larger blocks amortize U slices.
    pub block_rows: usize,
}
impl TerminalRelationExecutionOptions {
    pub fn new(workers: usize) -> Self {
        Self {
            workers,
            batch_rows: workers.saturating_mul(128),
            block_rows: 32,
        }
    }
}

pub struct TerminalRelationExecutor {
    pool: SectorExecutor,
    options: TerminalRelationExecutionOptions,
}

#[derive(Debug)]
struct PreparedRow {
    original: TerminalRelationRow,
    conditions: Vec<Coefficient>,
    support: BTreeSet<IntegralKey>,
}

impl TerminalRelationExecutor {
    pub fn new(options: TerminalRelationExecutionOptions) -> Result<Self, TerminalRelationError> {
        if options.workers == 0 || options.batch_rows == 0 || options.block_rows == 0 {
            return Err(invalid(
                "relation workers and batch/block sizes must be positive",
            ));
        }
        let pool = SectorExecutor::new(options.workers).map_err(invalid)?;
        Ok(Self { pool, options })
    }

    pub fn options(&self) -> TerminalRelationExecutionOptions {
        self.options
    }

    /// Preparation is eager and stealable. Publication/cursors advance only in
    /// original order after all started jobs have drained. Cancellation leaves
    /// an exact prefix; no worker may publish or serialize a half-finished row.
    pub fn step(
        &self,
        session: &mut TerminalRelationSession,
        cancel: &AtomicBool,
    ) -> Result<TerminalRelationStats, TerminalRelationError> {
        if session.assistance.is_some() {
            return Err(invalid("assisted sessions require their equation provider"));
        }
        if self.options.workers == 1 || cancel.load(Ordering::Relaxed) || session.is_complete() {
            return session.step(cancel);
        }
        self.step_rows(session, cancel, &Progress::default())
    }

    /// Observe disposable atomic counters on the caller's original thread while
    /// work stealing runs. The scoped coordinator drains all jobs before return.
    pub fn step_observed(
        &self,
        session: &mut TerminalRelationSession,
        cancel: &AtomicBool,
        mut observer: impl FnMut(TerminalRelationExecutionProgress),
    ) -> Result<TerminalRelationStats, TerminalRelationError> {
        if self.options.workers == 1
            || session.assistance.is_some()
            || cancel.load(Ordering::Relaxed)
            || session.is_complete()
            || (session.rebuild_cursor == session.rebuild.len()
                && session.seed_cursor == session.seeds.len())
        {
            return self.step(session, cancel);
        }
        let progress = Progress::default();
        std::thread::scope(|scope| {
            let (sender, receiver) = std::sync::mpsc::sync_channel(1);
            let shared = &progress;
            scope.spawn(move || {
                let _ = sender.send(self.step_rows(session, cancel, shared));
            });
            loop {
                match receiver.recv_timeout(Duration::from_millis(250)) {
                    Ok(result) => {
                        observer(progress.snapshot());
                        return result;
                    }
                    Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {
                        observer(progress.snapshot())
                    }
                    Err(error) => {
                        return Err(invalid(format!("relation execution coordinator: {error}")));
                    }
                }
            }
        })
    }

    /// A mutable equation provider retains its existing deterministic request
    /// order. It must not be called speculatively from multiple workers.
    pub fn step_with_provider<F>(
        &self,
        session: &mut TerminalRelationSession,
        cancel: &AtomicBool,
        provider: F,
    ) -> Result<TerminalRelationStats, TerminalRelationError>
    where
        F: FnMut(&IntegralKey) -> Result<Vec<TerminalEquation>, TerminalRelationError>,
    {
        session.step_with_provider(cancel, provider)
    }

    fn step_rows(
        &self,
        session: &mut TerminalRelationSession,
        cancel: &AtomicBool,
        progress: &Progress,
    ) -> Result<TerminalRelationStats, TerminalRelationError> {
        let rebuild = session.rebuild_cursor < session.rebuild.len();
        if !rebuild && session.seed_cursor == session.seeds.len() {
            session.normalize_generated()?;
            return Ok(session.statistics());
        }
        let first = if rebuild {
            session.rebuild_cursor
        } else {
            session.seed_cursor * session.sources.len() + session.source_cursor
        };
        let total = if rebuild {
            session.rebuild.len()
        } else {
            session
                .seeds
                .len()
                .checked_mul(session.sources.len())
                .ok_or_else(|| invalid("finite source work overflow"))?
        };
        let end = first.saturating_add(self.options.batch_rows).min(total);
        progress.total.store(end - first, Ordering::Relaxed);
        let prepared: Vec<_> = self.pool.install(|| {
            (first..end)
                .into_par_iter()
                .map(|ordinal| {
                    if cancel.load(Ordering::Relaxed) {
                        return None;
                    }
                    Some((|| {
                        let (row, conditions) = if rebuild {
                            (session.rebuild[ordinal].clone(), Vec::new())
                        } else {
                            session.sources.row(
                                &session.seeds[ordinal / session.sources.len()],
                                ordinal % session.sources.len(),
                                session.limits.algebra,
                            )?
                        };
                        let original = session.normalized_row(&row)?;
                        let support =
                            session.assistance_support(row.keys().chain(original.keys()))?;
                        let result = PreparedRow {
                            original,
                            conditions,
                            support,
                        };
                        progress.prepared.fetch_add(1, Ordering::Relaxed);
                        Ok(result)
                    })())
                })
                .collect()
        });
        // A preparation error has the same source-prefix boundary as serial
        // execution. Later speculative work never changes authoritative state.
        let usable = prepared
            .iter()
            .take_while(|row| matches!(row, Some(Ok(_))))
            .count();
        let mut rows = Vec::with_capacity(usable);
        let mut stop_error = None;
        for row in prepared {
            match row {
                Some(Ok(row)) if stop_error.is_none() && rows.len() < usable => rows.push(row),
                Some(Err(error)) if stop_error.is_none() && rows.len() == usable => {
                    stop_error = Some(error);
                    break;
                }
                _ => break,
            }
        }
        if rows.is_empty() {
            return match stop_error {
                Some(error) => Err(error),
                None => Ok(session.statistics()),
            };
        }
        // Simulate the serial prepend-on-first-encounter column order without
        // changing the session. Future columns are zero in earlier sources/U.
        progress.stage.store(1, Ordering::Relaxed);
        let mut known: BTreeSet<_> = session.columns.iter().cloned().collect();
        let mut additions = Vec::new();
        for row in &rows {
            let mut new: Vec<_> = row
                .original
                .keys()
                .filter(|key| !known.contains(*key))
                .cloned()
                .collect();
            elimination::sort_keys(&mut new);
            known.extend(new.iter().cloned());
            if !new.is_empty() {
                additions.push(new);
            }
        }
        let columns: Vec<_> = additions
            .into_iter()
            .rev()
            .flatten()
            .chain(session.columns.iter().cloned())
            .collect();
        u32::try_from(columns.len()).map_err(invalid)?;
        let offsets: BTreeMap<_, _> = columns
            .iter()
            .enumerate()
            .map(|(i, k)| (k.clone(), i as u32))
            .collect();
        let prefix = columns.len() - session.columns.len();
        progress.stage.store(2, Ordering::Relaxed);
        let results: Vec<_> = self.pool.install(|| {
            rows.par_chunks(self.options.block_rows)
                .map(|block| {
                    let result = reduce_block(session, block, &columns, &offsets, prefix, cancel);
                    if let Ok(rows) = &result {
                        progress.eliminated.fetch_add(rows.len(), Ordering::Relaxed);
                    }
                    progress.blocks.fetch_add(1, Ordering::Relaxed);
                    result
                })
                .collect()
        });
        progress.stage.store(3, Ordering::Relaxed);
        let mut ordinal = 0;
        for block in results {
            let block = block?;
            for residual in block {
                if cancel.load(Ordering::Relaxed) {
                    return Ok(session.statistics());
                }
                let original = &rows[ordinal];
                // Admit ORIGINAL support even if a local combination removes a
                // column. Otherwise batching could change precedence or scope.
                session.ensure_columns(&original.original)?;
                session.add_row(residual)?;
                if let Some(assistance) = &mut session.assistance {
                    assistance.pending.extend(original.support.iter().cloned());
                }
                for condition in &original.conditions {
                    if !session.conditions.contains(condition) {
                        session.conditions.push(condition.clone());
                    }
                }
                if rebuild {
                    session.rebuild_cursor += 1;
                    session.completed_rebuild_rows += 1;
                } else {
                    session.source_cursor += 1;
                    if session.source_cursor == session.sources.len() {
                        session.source_cursor = 0;
                        session.seed_cursor += 1;
                    }
                }
                ordinal += 1;
                progress.committed.store(ordinal, Ordering::Relaxed);
            }
        }
        if rebuild && session.rebuild_cursor == session.rebuild.len() {
            session.rebuild.clear();
            session.rebuild_cursor = 0;
        }
        if let Some(error) = stop_error {
            return Err(error);
        }
        Ok(session.statistics())
    }
}

/// Copy only the frozen pivots reachable from this block's support. This is
/// graph traversal, not algebra; values and forward reduction stay Symbolica's.
/// Compact columns avoid full-history matrix/scratch clones on every worker.
fn reduce_block(
    session: &TerminalRelationSession,
    rows: &[PreparedRow],
    columns: &[IntegralKey],
    offsets: &BTreeMap<IntegralKey, u32>,
    prefix: usize,
    cancel: &AtomicBool,
) -> Result<Vec<TerminalRelationRow>, TerminalRelationError> {
    let u = session.reducer.u();
    let mut needed: BTreeSet<u32> = rows
        .iter()
        .flat_map(|row| row.original.keys().map(|k| offsets[k]))
        .collect();
    let mut frontier: Vec<_> = needed.iter().copied().collect();
    let mut pivots = BTreeSet::new();
    while let Some(column) = frontier.pop() {
        if cancel.load(Ordering::Relaxed) {
            return Ok(Vec::new());
        }
        if let Some(Some(row)) = column
            .checked_sub(prefix as u32)
            .and_then(|old| session.reducer.pivots().get(old as usize))
        {
            if pivots.insert(*row) {
                let span = u.row_ptrs()[*row as usize]..u.row_ptrs()[*row as usize + 1];
                for index in &u.col_idcs()[span] {
                    let column = *index + prefix as u32;
                    if needed.insert(column) {
                        frontier.push(column);
                    }
                }
            }
        }
    }
    let indices: Vec<_> = needed.into_iter().collect();
    let remap: BTreeMap<_, _> = indices
        .iter()
        .enumerate()
        .map(|(i, c)| (*c, i as u32))
        .collect();
    let width = u32::try_from(indices.len()).map_err(invalid)?;
    let mut values = Vec::new();
    let mut ids = Vec::new();
    let mut row_ptrs = vec![0];
    let mut pivot_map = vec![None; width as usize];
    for row in pivots {
        let span = u.row_ptrs()[row as usize]..u.row_ptrs()[row as usize + 1];
        let pivot = remap[&(u.col_idcs()[span.start] + prefix as u32)];
        pivot_map[pivot as usize] = Some((row_ptrs.len() - 1) as u32);
        for (index, value) in u.col_idcs()[span.clone()].iter().zip(&u.values()[span]) {
            ids.push(remap[&(*index + prefix as u32)]);
            values.push(value.clone());
        }
        row_ptrs.push(ids.len());
    }
    let matrix = SparseMatrix::from_csr(
        (row_ptrs.len() - 1) as u32,
        width,
        values,
        row_ptrs,
        ids,
        RationalPolynomialField::new(Z),
    );
    let mut local = SparseRowReducer::from_upper_triangular_matrix(matrix, pivot_map);
    let mut result = Vec::with_capacity(rows.len());
    for row in rows {
        if cancel.load(Ordering::Relaxed) {
            break;
        }
        let mut input: Vec<_> = row
            .original
            .iter()
            .map(|(k, c)| (remap[&offsets[k]], c.clone()))
            .collect();
        input.sort_by_key(|(i, _)| *i);
        let (ids, values): (Vec<_>, Vec<_>) = input.into_iter().unzip();
        let residual = if local.add_row(&values, &ids).is_some() {
            let u = local.u();
            let span = u.row_ptrs()[u.nrows() as usize - 1]..u.row_ptrs()[u.nrows() as usize];
            u.col_idcs()[span.clone()]
                .iter()
                .zip(&u.values()[span])
                .map(|(i, c)| (columns[indices[*i as usize] as usize].clone(), c.clone()))
                .collect()
        } else {
            TerminalRelationRow::new()
        };
        result.push(residual);
    }
    Ok(result)
}

#[cfg(test)]
mod tests;
