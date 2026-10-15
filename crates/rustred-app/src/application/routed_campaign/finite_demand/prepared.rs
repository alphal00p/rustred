//! Private source-authenticated join between the actual prepared reducer and
//! trusted-generated raw closure. Caller digests never label a separate engine.
use super::super::{
    RoutedCampaignRequest,
    input::{PreferredResidualPolicy, Selection},
    prepare,
};
use super::storage::{
    self, CursorRecord, GenerationState, RawCensus, RawExecutionBinding, ScopeBinding,
    SelectionRecord, StoreWriter, WorkReader,
};
use crate::AppError;
use rustred::{
    family::IntegralKey,
    solver::{
        CandidateRoutedCompletedDelta, CandidateRoutedDeltaExportLimits,
        CandidateRoutedGeneratedBaseline, CandidateRoutedTraceCensusLimits,
        CandidateRoutedWorkBudget, RoutedCandidateReducer,
    },
};
use serde_json::{Value, json};
use std::{
    collections::{BTreeMap, BTreeSet},
    sync::atomic::{AtomicBool, Ordering},
};

pub(crate) struct PreparedExecution<const N: usize> {
    reducer: RoutedCandidateReducer<N>,
    execution: RawExecutionBinding,
}
impl<const N: usize> PreparedExecution<N> {
    pub fn prepare(
        request: &RoutedCampaignRequest,
        cancel: &AtomicBool,
        observer: &impl Fn(Value),
    ) -> Result<Option<Self>, AppError> {
        let (selection, arity, limits) = Selection::parse(&request.selection_json)?;
        let mut fingerprints = None;
        let reducer = prepare::prepare_with_fingerprints(
            request,
            &selection,
            limits,
            cancel,
            observer,
            Some(&mut |digests| {
                if fingerprints.replace(digests).is_some() {
                    return Err("duplicate prepared payload binding".into());
                }
                Ok(())
            }),
        )?;
        let Some(reducer) = reducer else {
            return Ok(None);
        };
        let fingerprints =
            fingerprints.ok_or_else(|| AppError::input("prepared payload binding missing"))?;
        let execution = execution_binding(request, &selection, arity, &fingerprints)?;
        if cancel.load(Ordering::Acquire) {
            return Ok(None);
        }
        Ok(Some(Self { reducer, execution }))
    }
    pub fn execution(&self) -> &RawExecutionBinding {
        &self.execution
    }
    pub fn reducer(&self) -> &RoutedCandidateReducer<N> {
        &self.reducer
    }
    pub fn bind_generation<'a>(
        &'a self,
        reader: &'a WorkReader,
        scope: &ScopeBinding,
        budget: CandidateRoutedWorkBudget,
        census: CandidateRoutedTraceCensusLimits,
        export: CandidateRoutedDeltaExportLimits,
        cancel: &AtomicBool,
    ) -> Result<CandidateRoutedGeneratedBaseline<'a, N>, AppError> {
        let generation = reader.snapshot();
        if generation.execution() != &self.execution || &generation.state().scope != scope {
            return Err(AppError::input(
                "finite generation differs from actual prepared execution/scope",
            ));
        }
        // Bounded current metadata, not historical seed/Work/algebra replay.
        for row in &generation.state().census.selections {
            check(cancel)?;
            self.reducer
                .validate_generated_selection(
                    &row.owner,
                    usize::try_from(row.batch).map_err(failure)?,
                    usize::try_from(row.baseline).map_err(failure)?,
                    usize::try_from(row.selected).map_err(failure)?,
                )
                .map_err(generated_failure)?;
        }
        for powers in &generation.state().census.terminals {
            check(cancel)?;
            self.reducer
                .validate_generated_terminal(
                    &IntegralKey::try_new(powers.iter().copied()).map_err(failure)?,
                )
                .map_err(generated_failure)?;
        }
        for powers in &generation.state().census.zeros {
            check(cancel)?;
            self.reducer
                .validate_generated_zero(
                    &IntegralKey::try_new(powers.iter().copied()).map_err(failure)?,
                )
                .map_err(generated_failure)?;
        }
        check(cancel)?;
        self.reducer
            .bind_trusted_generated_baseline(
                &self.execution,
                reader.directory(),
                reader,
                budget,
                census,
                export,
            )
            .map_err(generated_failure)
    }
    /// LOW-LEVEL generated metadata join: authenticate the native Work seal's
    /// predecessor/reducer and atomically persist raw census plus supplied
    /// metadata. This does NOT authenticate a prefix-window transition or prove
    /// query coverage. The backend must separately bind the exact traced starts
    /// and admission region to a typed window successor before calling this.
    pub fn commit_delta_with_generated_metadata(
        &self,
        writer: &mut StoreWriter,
        delta: CandidateRoutedCompletedDelta<'_, N>,
        scope: ScopeBinding,
        cursors: Vec<CursorRecord>,
        cancel: &AtomicBool,
    ) -> Result<storage::CommitOutcome, AppError> {
        let previous = writer
            .current()
            .map_err(storage_failure)?
            .ok_or_else(|| AppError::input("initial pending generation missing"))?;
        if !delta.is_for_reducer(&self.reducer)
            || delta.execution() != &self.execution
            || previous.execution() != &self.execution
            || previous.digest() != delta.predecessor()
        {
            return Err(AppError::input(
                "finite native delta is stale or belongs to another prepared engine",
            ));
        }
        let report = delta.report();
        let census = report
            .complete_census()
            .ok_or_else(|| AppError::input("finite delta census is incomplete"))?;
        let limits = writer.limits();
        let before = previous.state();
        preflight_merge(before, &scope, &cursors, report, limits, cancel)?;
        let mut terminals: BTreeSet<_> = before.census.terminals.iter().cloned().collect();
        let mut zeros: BTreeSet<_> = before.census.zeros.iter().cloned().collect();
        let mut selected: BTreeMap<_, u64> = before
            .census
            .selections
            .iter()
            .map(|r| {
                (
                    (r.owner.clone(), r.batch, r.baseline, r.selected),
                    r.multiplicity,
                )
            })
            .collect();
        // The bounded first format stages current census metadata, not Work.
        // It is deliberately O(history metadata), not yet a full-R6 strategy.
        for key in report.trace().declared_terminals() {
            check(cancel)?;
            terminals.insert(key.powers().to_vec());
        }
        for key in report.trace().visited_zeros() {
            check(cancel)?;
            zeros.insert(key.powers().to_vec());
        }
        for (id, &count) in census.selections() {
            check(cancel)?;
            let key = (
                id.owner.clone(),
                u64::try_from(id.batch_ordinal).map_err(failure)?,
                u64::try_from(id.baseline_rule_ordinal).map_err(failure)?,
                u64::try_from(id.selected_rule_ordinal).map_err(failure)?,
            );
            let value = selected.entry(key).or_default();
            *value = value
                .checked_add(count)
                .ok_or_else(|| AppError::limit("cumulative selection multiplicity overflow"))?;
        }
        if terminals
            .len()
            .checked_add(zeros.len())
            .and_then(|n| n.checked_add(selected.len()))
            .is_none_or(|n| n > limits.max_census_records)
        {
            return Err(AppError::limit("cumulative raw census allowance"));
        }
        let state = GenerationState {
            scope,
            cursors,
            completed_work: before
                .completed_work
                .checked_add(u64::try_from(delta.work().len()).map_err(failure)?)
                .ok_or_else(|| AppError::limit("cumulative completed Work overflow"))?,
            input_occurrences: before
                .input_occurrences
                .checked_add(u64::try_from(report.snapshot().input_targets).map_err(failure)?)
                .ok_or_else(|| AppError::limit("cumulative input occurrence overflow"))?,
            census: RawCensus {
                terminals: terminals.into_iter().collect(),
                zeros: zeros.into_iter().collect(),
                selections: selected
                    .into_iter()
                    .map(
                        |((owner, batch, baseline, selected), multiplicity)| SelectionRecord {
                            owner,
                            batch,
                            baseline,
                            selected,
                            multiplicity,
                        },
                    )
                    .collect(),
            },
        };
        check(cancel)?;
        writer
            .commit_completed(delta, &self.reducer, state, cancel)
            .map_err(storage_failure)
    }
}
fn failure(error: impl ToString) -> AppError {
    AppError::input(error.to_string())
}
fn storage_failure(error: storage::StorageError) -> AppError {
    use crate::AppErrorKind as A;
    use storage::ErrorKind as K;
    let kind = match error.kind {
        K::Resource => A::Limit,
        K::Cancelled => A::Cancelled,
        K::Corrupt => A::Serialization,
        K::Io | K::Locked | K::Indeterminate => A::Execution,
        K::Input | K::Stale => A::Input,
    };
    AppError::new(kind, error.to_string())
}
fn generated_failure(error: rustred::persistence::finite_work::GeneratedWorkError) -> AppError {
    use crate::AppErrorKind as A;
    use rustred::persistence::finite_work::GeneratedWorkErrorKind as K;
    let kind = match error.kind {
        K::Resource => A::Limit,
        K::Cancelled => A::Cancelled,
        K::Corrupt => A::Serialization,
        K::Io => A::Execution,
    };
    AppError::new(kind, error.to_string())
}

/// Conservative payload bound before cloning cumulative metadata. Current
/// immutable state is charged separately by max_state_bytes; staging plus the
/// output payload must fit max_transient_bytes. Tree/allocator overhead is not
/// RSS and is separately bounded by max_census_records/external RAM guards.
fn preflight_merge<const N: usize>(
    before: &GenerationState,
    scope: &ScopeBinding,
    cursors: &[CursorRecord],
    report: &rustred::solver::CandidateRoutedCampaignReport<N>,
    limits: storage::StorageLimits,
    cancel: &AtomicBool,
) -> Result<(), AppError> {
    let add = |a: usize, b: usize| {
        a.checked_add(b)
            .ok_or_else(|| AppError::limit("finite census staging payload overflow"))
    };
    let mul = |a: usize, b: usize| {
        a.checked_mul(b)
            .ok_or_else(|| AppError::limit("finite census staging payload overflow"))
    };
    let old_count = add(
        add(before.census.terminals.len(), before.census.zeros.len())?,
        before.census.selections.len(),
    )?;
    let census = report
        .complete_census()
        .ok_or_else(|| AppError::input("incomplete census preflight"))?;
    let new_count = add(
        add(
            report.trace().declared_terminals().len(),
            report.trace().visited_zeros().len(),
        )?,
        census.selections().len(),
    )?;
    if add(old_count, new_count)? > limits.max_census_records
        || scope.rows.len() > limits.max_scope_rows
        || cursors.len() != scope.rows.len()
    {
        return Err(AppError::limit(
            "finite census/scope staging record allowance",
        ));
    }
    let mut payload = before.payload_bytes().map_err(storage_failure)?;
    for key in report
        .trace()
        .declared_terminals()
        .iter()
        .chain(report.trace().visited_zeros())
    {
        check(cancel)?;
        payload = add(
            payload,
            add(
                std::mem::size_of::<Vec<i64>>(),
                mul(key.powers().len(), std::mem::size_of::<i64>())?,
            )?,
        )?;
    }
    for id in census.selections().keys() {
        check(cancel)?;
        payload = add(
            payload,
            add(std::mem::size_of::<SelectionRecord>(), id.owner.len())?,
        )?;
    }
    // Conservatively retain the old scope charge as well; it avoids allocating
    // an intermediate scope clone merely to estimate an append transition.
    payload = add(
        payload,
        mul(
            scope.rows.capacity(),
            std::mem::size_of::<storage::ScopeRow>(),
        )?,
    )?;
    payload = add(
        payload,
        mul(cursors.len(), std::mem::size_of::<CursorRecord>())?,
    )?;
    for (row, cursor) in scope.rows.iter().zip(cursors) {
        check(cancel)?;
        payload = add(
            payload,
            add(row.origin.capacity(), cursor.encoded_cursor.capacity())?,
        )?;
    }
    if payload > limits.max_state_bytes || mul(payload, 2)? > limits.max_transient_bytes {
        return Err(AppError::limit("finite census staging payload allowance"));
    }
    check(cancel)
}
fn check(cancel: &AtomicBool) -> Result<(), AppError> {
    if cancel.load(Ordering::Acquire) {
        Err(AppError::new(
            crate::AppErrorKind::Cancelled,
            "finite generated preparation/commit cancelled",
        ))
    } else {
        Ok(())
    }
}

/// Ordered semantic selection plus exact admitted payload bytes. Paths,
/// worker count, query domains and downstream refinement are intentionally
/// absent. Per-formula native policies are conservative identity boundaries;
/// aggregate trace/counter/cache allowances are not mathematical conventions.
fn execution_binding(
    request: &RoutedCampaignRequest,
    selection: &Selection,
    arity: usize,
    fingerprints: &[String],
) -> Result<RawExecutionBinding, AppError> {
    let expected = selection
        .owners
        .len()
        .checked_add(selection.domain_rule_overlays.len())
        .and_then(|n| n.checked_add(selection.preferred_owner_programs.len()))
        .ok_or_else(|| AppError::limit("prepared payload count overflow"))?;
    if fingerprints.len() != expected
        || fingerprints
            .iter()
            .any(|s| s.len() != 64 || !s.bytes().all(|b| b.is_ascii_hexdigit()))
    {
        return Err(AppError::input(
            "invalid exact prepared payload digest vector",
        ));
    }
    let identity = json!({
        "policy":"rustred-raw-finite-execution-v1", "arity":arity,
        "family":selection.family_fingerprint, "payloads":fingerprints,
        "owners":selection.owners.iter().map(|o| &o.mask).collect::<Vec<_>>(),
        "overlays":selection.domain_rule_overlays.iter().map(|o| &o.owner_mask).collect::<Vec<_>>(),
        "preferences":selection.preferred_owner_programs.iter().map(|o| json!({
            "owner":o.owner_mask,"rules":o.rule_ordinals,"residual":match o.residual_policy {
                PreferredResidualPolicy::DeferToBaseline => "defer-to-baseline" }})).collect::<Vec<_>>(),
        "routes":selection.initial_frontier_routes.iter().map(|r| json!({"source":r.source_mask,
            "owner":r.owner_mask,"transport":r.requires_transport,"source_map":r.source_to_representative,
            "owner_map":r.owner_to_representative})).collect::<Vec<_>>(),
        "exact_algebra":format!("{:?}",request.reduction_limits.exact_algebra),
        "indexed_algebra":format!("{:?}",request.reduction_limits.indexed_algebra),
        "transport_expansion":format!("{:?}",request.trace_limits.expansion),
    });
    let bytes = serde_json::to_vec(&identity).map_err(failure)?;
    Ok(RawExecutionBinding {
        physical_arity: u32::try_from(arity).map_err(failure)?,
        program_identity: *blake3::hash(&bytes).as_bytes(),
    })
}

#[cfg(test)]
mod tests;
