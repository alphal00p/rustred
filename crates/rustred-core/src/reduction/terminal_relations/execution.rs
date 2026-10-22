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
    /// Empty pending requests removed by binding a complete finite provider.
    /// Historical queried keys and every queued equation remain untouched.
    pub provider_keys_filtered: usize,
}
#[derive(Default)]
struct Progress {
    stage: AtomicUsize,
    total: AtomicUsize,
    prepared: AtomicUsize,
    eliminated: AtomicUsize,
    committed: AtomicUsize,
    blocks: AtomicUsize,
    filtered: AtomicUsize,
}
impl Progress {
    fn snapshot(&self) -> TerminalRelationExecutionProgress {
        TerminalRelationExecutionProgress {
            stage: match self.stage.load(Ordering::Relaxed) {
                0 => "preparing rows",
                1 => "mapping columns",
                2 => "parallel elimination",
                3 => "ordered merge",
                4 => "provider request",
                5 => "assistance equation",
                6 => "filtering provider requests",
                7 => "serial row",
                8 => "provider barrier planning",
                _ => "atomic block merge",
            },
            rows_total: self.total.load(Ordering::Relaxed),
            rows_prepared: self.prepared.load(Ordering::Relaxed),
            rows_eliminated: self.eliminated.load(Ordering::Relaxed),
            rows_committed: self.committed.load(Ordering::Relaxed),
            blocks_completed: self.blocks.load(Ordering::Relaxed),
            provider_keys_filtered: self.filtered.load(Ordering::Relaxed),
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct TerminalRelationExecutionOptions {
    pub workers: usize,
    /// Operational window, not a limit on the finite search inventory.
    pub batch_rows: usize,
    /// Smaller blocks expose more stealable jobs; larger blocks amortize U
    /// slices and column registration. A block adding columns is an atomic
    /// cooperative-cancellation boundary; no partial block is checkpointed.
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

/// Borrow the entire frozen registry; own only columns newly encountered in
/// this window. End-relative offsets keep old identities stable when new
/// auxiliary columns prepend, avoiding an O(all historical columns) clone and
/// BTreeMap rebuild for every small/reuse-heavy execution window.
struct ColumnPlan<'a> {
    old: &'a [IntegralKey],
    old_offsets: &'a BTreeMap<IntegralKey, u32>,
    new: Vec<IntegralKey>,
    new_offsets: BTreeMap<IntegralKey, u32>,
}

impl<'a> ColumnPlan<'a> {
    fn new(
        session: &'a TerminalRelationSession,
        rows: &[PreparedRow],
    ) -> Result<Self, TerminalRelationError> {
        let mut known = BTreeSet::new();
        let mut additions = Vec::new();
        for row in rows {
            let mut new: Vec<_> = row
                .original
                .keys()
                .filter(|key| !session.column_offsets.contains_key(*key) && !known.contains(*key))
                .cloned()
                .collect();
            elimination::sort_keys(&mut new);
            known.extend(new.iter().cloned());
            if !new.is_empty() {
                additions.push(new);
            }
        }
        let new: Vec<_> = additions.into_iter().rev().flatten().collect();
        u32::try_from(new.len().saturating_add(session.columns.len())).map_err(invalid)?;
        let new_offsets = new
            .iter()
            .enumerate()
            .map(|(index, key)| (key.clone(), index as u32))
            .collect();
        Ok(Self {
            old: &session.columns,
            old_offsets: &session.column_offsets,
            new,
            new_offsets,
        })
    }

    fn index(&self, key: &IntegralKey) -> u32 {
        self.new_offsets
            .get(key)
            .copied()
            .unwrap_or_else(|| (self.new.len() + self.old.len() - 1) as u32 - self.old_offsets[key])
    }

    fn key(&self, index: u32) -> &IntegralKey {
        let index = index as usize;
        if index < self.new.len() {
            &self.new[index]
        } else {
            &self.old[index - self.new.len()]
        }
    }
}

/// A preflighted commit of one completed local block. Only this block's new
/// columns belong here, never those from later speculative source blocks.
/// Conservative resource bounds select the fast path; failure to establish a
/// bound falls back to normal per-row admission, not a new resource rejection.
struct BulkBlock {
    prefix: Vec<IntegralKey>,
    prefix_offsets: BTreeMap<IntegralKey, u32>,
    width: u32,
    next_seed: usize,
    next_source: usize,
    next_rebuild: usize,
    next_completed_rebuild: u64,
}

impl BulkBlock {
    fn prepare(
        session: &TerminalRelationSession,
        original: &[PreparedRow],
        residuals: &[TerminalRelationRow],
        rebuild: bool,
    ) -> Option<Self> {
        if original.len() <= 1 || original.len() != residuals.len() {
            return None;
        }
        // A new provider request is a hard ordered barrier. Its source may be
        // the final row, but no later ordinary source may enter this commit.
        if original[..original.len() - 1]
            .iter()
            .any(|row| !row.support.is_empty())
        {
            return None;
        }
        let columns = ColumnPlan::new(session, original).ok()?;
        if columns.new.is_empty() {
            // No historical column shifting to amortize: preserve row-level
            // cancellation responsiveness on the ordinary reuse-only path.
            return None;
        }
        let width = columns.new.len().checked_add(session.columns.len())?;
        let independent = (session.reducer.u().nrows() as usize).checked_add(original.len())?;
        let nonzeros = original
            .len()
            .checked_mul(width)?
            .checked_add(session.reducer.u().nvalues())?;
        if width > session.limits.max_columns
            || independent > session.limits.max_rows
            || nonzeros > session.limits.max_nonzeros
            || u32::try_from(independent).is_err()
        {
            return None;
        }
        let mut condition_count = session.conditions.len();
        for row in original {
            condition_count = condition_count.checked_add(row.conditions.len())?;
        }
        if let Some(assistance) = &session.assistance {
            if !assistance.is_complete() {
                return None;
            }
            let mut support_count = assistance
                .pending
                .len()
                .checked_add(assistance.queried.len())?;
            for row in original {
                support_count = support_count.checked_add(row.support.len())?;
            }
            if support_count > session.assistance_key_limit() {
                return None;
            }
        }
        // Exact block residuals can contain frozen-U columns as well as their
        // original support, but never columns from a future source block.
        if residuals.iter().flat_map(|row| row.keys()).any(|key| {
            !columns.new_offsets.contains_key(key) && !session.column_offsets.contains_key(key)
        }) {
            return None;
        }
        let (next_seed, next_source, next_rebuild, next_completed_rebuild) = if rebuild {
            let next = session.rebuild_cursor.checked_add(original.len())?;
            if next > session.rebuild.len() {
                return None;
            }
            (
                session.seed_cursor,
                session.source_cursor,
                next,
                session
                    .completed_rebuild_rows
                    .checked_add(u64::try_from(original.len()).ok()?)?,
            )
        } else {
            let source_count = session.sources.len();
            let next = session.source_cursor.checked_add(original.len())?;
            let seed = session
                .seed_cursor
                .checked_add(next.checked_div(source_count)?)?;
            let source = next.checked_rem(source_count)?;
            if seed > session.seeds.len() || (seed == session.seeds.len() && source != 0) {
                return None;
            }
            (
                seed,
                source,
                session.rebuild_cursor,
                session.completed_rebuild_rows,
            )
        };
        Some(Self {
            prefix: columns.new,
            prefix_offsets: columns.new_offsets,
            width: u32::try_from(width).ok()?,
            next_seed,
            next_source,
            next_rebuild,
            next_completed_rebuild,
        })
    }

    /// Prepare owned CSR storage before any session mutation. Coefficients are
    /// moved from the completed residuals, not cloned into another row matrix.
    fn matrix(
        &self,
        session: &TerminalRelationSession,
        rows: Vec<TerminalRelationRow>,
    ) -> SparseMatrix<RationalPolynomialField<IntegerRing, u16>> {
        let count = rows.len() as u32;
        let mut values = Vec::new();
        let mut ids = Vec::new();
        let mut row_ptrs = vec![0];
        for row in rows {
            let mut entries: Vec<_> = row
                .into_iter()
                .map(|(key, coefficient)| {
                    let index = self
                        .prefix_offsets
                        .get(&key)
                        .copied()
                        .unwrap_or_else(|| self.width - 1 - session.column_offsets[&key]);
                    (index, coefficient)
                })
                .collect();
            entries.sort_by_key(|(index, _)| *index);
            for (index, coefficient) in entries {
                ids.push(index);
                values.push(coefficient);
            }
            row_ptrs.push(ids.len());
        }
        SparseMatrix::from_csr(
            count,
            self.width,
            values,
            row_ptrs,
            ids,
            RationalPolynomialField::new(Z),
        )
    }

    /// No fallible Result or cooperative cancellation exit is allowed after
    /// prefix registration until all rows, conditions, supports and cursors
    /// are committed. Native panic/OOM discards in-flight state and requires
    /// the last durable checkpoint; this is not an in-memory rollback scheme.
    fn commit(
        mut self,
        session: &mut TerminalRelationSession,
        original: &[PreparedRow],
        matrix: SparseMatrix<RationalPolynomialField<IntegerRing, u16>>,
    ) {
        let before = session.reducer.u().nrows() as usize;
        session.reducer.add_cols(&vec![0; self.prefix.len()]);
        for (index, key) in self.prefix.iter().enumerate() {
            session
                .column_offsets
                .insert(key.clone(), self.width - 1 - index as u32);
        }
        self.prefix.append(&mut session.columns);
        session.columns = self.prefix;
        // Symbolica owns all arithmetic and processes the exact same ordered
        // row sequence as repeated add_row calls.
        session.reducer.add_matrix(&matrix);
        let u = session.reducer.u();
        for row in before..u.nrows() as usize {
            let span = u.row_ptrs()[row]..u.row_ptrs()[row + 1];
            if u.col_idcs()[span].iter().all(|index| {
                session
                    .terminals
                    .contains(&session.columns[*index as usize])
            }) {
                session.terminal_relations += 1;
            }
        }
        for row in original {
            if let Some(assistance) = &mut session.assistance {
                assistance.pending.extend(row.support.iter().cloned());
            }
            for condition in &row.conditions {
                if !session.conditions.contains(condition) {
                    session.conditions.push(condition.clone());
                }
            }
        }
        session.seed_cursor = self.next_seed;
        session.source_cursor = self.next_source;
        session.rebuild_cursor = self.next_rebuild;
        session.completed_rebuild_rows = self.next_completed_rebuild;
    }
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
            let progress = Progress::default();
            progress.stage.store(7, Ordering::Relaxed);
            observer(progress.snapshot());
            let result = self.step(session, cancel);
            observer(progress.snapshot());
            return result;
        }
        self.step_rows_observed(session, cancel, &Progress::default(), observer)
    }

    fn step_rows_observed(
        &self,
        session: &mut TerminalRelationSession,
        cancel: &AtomicBool,
        progress: &Progress,
        mut observer: impl FnMut(TerminalRelationExecutionProgress),
    ) -> Result<TerminalRelationStats, TerminalRelationError> {
        std::thread::scope(|scope| {
            let (sender, receiver) = std::sync::mpsc::sync_channel(1);
            let shared = progress;
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

    /// Mutable provider requests and their equations retain their deterministic
    /// serial barriers. Between barriers, ordinary IBPs use the same stealable
    /// exact blocks as unassisted work. No callback is invoked speculatively.
    pub fn step_with_provider<F>(
        &self,
        session: &mut TerminalRelationSession,
        cancel: &AtomicBool,
        provider: F,
    ) -> Result<TerminalRelationStats, TerminalRelationError>
    where
        F: FnMut(&IntegralKey) -> Result<Vec<TerminalEquation>, TerminalRelationError>,
    {
        self.step_with_provider_observed(session, cancel, provider, |_| {})
    }

    pub fn step_with_provider_observed<F>(
        &self,
        session: &mut TerminalRelationSession,
        cancel: &AtomicBool,
        provider: F,
        observer: impl FnMut(TerminalRelationExecutionProgress),
    ) -> Result<TerminalRelationStats, TerminalRelationError>
    where
        F: FnMut(&IntegralKey) -> Result<Vec<TerminalEquation>, TerminalRelationError>,
    {
        if session
            .assistance
            .as_ref()
            .is_some_and(|assistance| assistance.finite_provider.is_some())
        {
            return Err(invalid(
                "complete provider is bound; use step_with_finite_provider",
            ));
        }
        self.step_assisted_observed(session, cancel, provider, &Progress::default(), observer)
    }

    pub fn step_with_finite_provider(
        &self,
        session: &mut TerminalRelationSession,
        cancel: &AtomicBool,
        provider: &TerminalRelationFiniteProvider,
    ) -> Result<TerminalRelationStats, TerminalRelationError> {
        self.step_with_finite_provider_observed(session, cancel, provider, |_| {})
    }

    /// The immutable map is a complete provider, not a memoized subset. It is
    /// rebound after cold restore under the session's existing authority. This
    /// skips known-empty requests before allocating auxiliary query keys.
    pub fn step_with_finite_provider_observed(
        &self,
        session: &mut TerminalRelationSession,
        cancel: &AtomicBool,
        provider: &TerminalRelationFiniteProvider,
        mut observer: impl FnMut(TerminalRelationExecutionProgress),
    ) -> Result<TerminalRelationStats, TerminalRelationError> {
        if cancel.load(Ordering::Relaxed) {
            return Ok(session.statistics());
        }
        let progress = Progress::default();
        progress.stage.store(6, Ordering::Relaxed);
        observer(progress.snapshot());
        progress
            .filtered
            .store(session.bind_finite_provider(provider)?, Ordering::Relaxed);
        self.step_assisted_observed(
            session,
            cancel,
            |key| Ok(provider.rows().get(key).cloned().unwrap_or_default()),
            &progress,
            observer,
        )
    }

    fn step_assisted_observed<F>(
        &self,
        session: &mut TerminalRelationSession,
        cancel: &AtomicBool,
        provider: F,
        progress: &Progress,
        mut observer: impl FnMut(TerminalRelationExecutionProgress),
    ) -> Result<TerminalRelationStats, TerminalRelationError>
    where
        F: FnMut(&IntegralKey) -> Result<Vec<TerminalEquation>, TerminalRelationError>,
    {
        let assistance = session
            .assistance
            .as_ref()
            .ok_or_else(|| invalid("equation assistance is not enabled"))?;
        if cancel.load(Ordering::Relaxed) || session.is_complete() {
            observer(progress.snapshot());
            return Ok(session.statistics());
        }
        if !assistance.is_complete() {
            progress.stage.store(
                if assistance.equation_cursor < assistance.equations.len() {
                    5
                } else {
                    4
                },
                Ordering::Relaxed,
            );
            observer(progress.snapshot());
            let result = session.step_with_bound_provider(cancel, provider);
            observer(progress.snapshot());
            result
        } else if self.options.workers == 1 {
            progress.stage.store(7, Ordering::Relaxed);
            observer(progress.snapshot());
            let result = session.step_ordinary(cancel);
            progress.total.store(1, Ordering::Relaxed);
            progress.committed.store(1, Ordering::Relaxed);
            observer(progress.snapshot());
            result
        } else {
            // A truly dynamic provider can open a serial barrier on every
            // source. Probe just the next source before speculating an entire
            // window: otherwise workers*batch_rows preparations would be
            // discarded for each single committed source. Finite providers
            // need no probe; their complete domain filters support directly.
            if assistance.finite_provider.is_none()
                && session.rebuild_cursor == session.rebuild.len()
                && session.seed_cursor < session.seeds.len()
            {
                progress.stage.store(8, Ordering::Relaxed);
                observer(progress.snapshot());
                let (row, _) = session.sources.row(
                    &session.seeds[session.seed_cursor],
                    session.source_cursor,
                    session.limits.algebra,
                )?;
                let needs_barrier = !session.assistance_support(row.keys())?.is_empty()
                    || !session
                        .assistance_support(session.normalized_row(&row)?.keys())?
                        .is_empty();
                if needs_barrier {
                    progress.stage.store(7, Ordering::Relaxed);
                    observer(progress.snapshot());
                    let result = session.step_ordinary(cancel);
                    progress.total.store(1, Ordering::Relaxed);
                    progress.committed.store(1, Ordering::Relaxed);
                    observer(progress.snapshot());
                    return result;
                }
            }
            self.step_rows_observed(session, cancel, progress, observer)
        }
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
                        let support = if rebuild {
                            // Rebuilds reindex already-authorized equations;
                            // serial replay never asks new provider questions.
                            BTreeSet::new()
                        } else {
                            session.assistance_support(row.keys().chain(original.keys()))?
                        };
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
        // A mutable provider may contribute a row as soon as this source's
        // support is observed. Sources after that deterministic barrier may
        // be prepared speculatively, but eliminating them against the old
        // snapshot is wasteful and they cannot yet be committed.
        if let Some(barrier) = rows.iter().position(|row| !row.support.is_empty()) {
            rows.truncate(barrier + 1);
        }
        // Simulate the serial prepend-on-first-encounter column order without
        // changing the session. Future columns are zero in earlier sources/U.
        progress.stage.store(1, Ordering::Relaxed);
        let columns = ColumnPlan::new(session, &rows)?;
        progress.stage.store(2, Ordering::Relaxed);
        let results: Vec<_> = self.pool.install(|| {
            rows.par_chunks(self.options.block_rows)
                .map(|block| {
                    let result = reduce_block(session, block, &columns, cancel);
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
            if cancel.load(Ordering::Relaxed) {
                return Ok(session.statistics());
            }
            let original = &rows[ordinal..ordinal + block.len()];
            if let Some(bulk) = BulkBlock::prepare(session, original, &block, rebuild) {
                let count = block.len();
                let matrix = bulk.matrix(session, block);
                // Staging owned CSR can take time. Cancellation still leaves
                // the old exact prefix intact until the atomic commit starts.
                if cancel.load(Ordering::Relaxed) {
                    return Ok(session.statistics());
                }
                progress.stage.store(9, Ordering::Relaxed);
                bulk.commit(session, original, matrix);
                ordinal += count;
                progress.committed.store(ordinal, Ordering::Relaxed);
                progress.stage.store(3, Ordering::Relaxed);
                if session
                    .assistance
                    .as_ref()
                    .is_some_and(|assistance| !assistance.is_complete())
                {
                    return Ok(session.statistics());
                }
                continue;
            }
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
                // Provider equations must enter before the next ordinary
                // source, exactly as in the scalar executor. Discard later
                // speculative residuals; they are scratch, not checkpoint
                // authority. Once a complete finite provider is exhausted,
                // off-domain source keys cannot create these barriers.
                if session
                    .assistance
                    .as_ref()
                    .is_some_and(|assistance| !assistance.is_complete())
                {
                    return Ok(session.statistics());
                }
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
    columns: &ColumnPlan<'_>,
    cancel: &AtomicBool,
) -> Result<Vec<TerminalRelationRow>, TerminalRelationError> {
    let u = session.reducer.u();
    let mut needed: BTreeSet<u32> = rows
        .iter()
        .flat_map(|row| row.original.keys().map(|key| columns.index(key)))
        .collect();
    let mut frontier: Vec<_> = needed.iter().copied().collect();
    let mut pivots = BTreeSet::new();
    while let Some(column) = frontier.pop() {
        if cancel.load(Ordering::Relaxed) {
            return Ok(Vec::new());
        }
        if let Some(Some(row)) = column
            .checked_sub(columns.new.len() as u32)
            .and_then(|old| session.reducer.pivots().get(old as usize))
        {
            if pivots.insert(*row) {
                let span = u.row_ptrs()[*row as usize]..u.row_ptrs()[*row as usize + 1];
                for index in &u.col_idcs()[span] {
                    let column = *index + columns.new.len() as u32;
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
        let pivot = remap[&(u.col_idcs()[span.start] + columns.new.len() as u32)];
        pivot_map[pivot as usize] = Some((row_ptrs.len() - 1) as u32);
        for (index, value) in u.col_idcs()[span.clone()].iter().zip(&u.values()[span]) {
            ids.push(remap[&(*index + columns.new.len() as u32)]);
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
            .map(|(key, value)| (remap[&columns.index(key)], value.clone()))
            .collect();
        input.sort_by_key(|(i, _)| *i);
        let (ids, values): (Vec<_>, Vec<_>) = input.into_iter().unzip();
        let residual = if local.add_row(&values, &ids).is_some() {
            let u = local.u();
            let span = u.row_ptrs()[u.nrows() as usize - 1]..u.row_ptrs()[u.nrows() as usize];
            u.col_idcs()[span.clone()]
                .iter()
                .zip(&u.values()[span])
                .map(|(i, c)| (columns.key(indices[*i as usize]).clone(), c.clone()))
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
