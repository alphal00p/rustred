//! Private source-authenticated join between the actual prepared reducer and
//! trusted-generated raw closure. Caller digests never label a separate engine.
use super::super::{
    RoutedCampaignRequest,
    input::{PreferredResidualPolicy, Selection},
    prepare,
};
use super::storage::{
    self, CursorRecord, GenerationState, RawCensus, RawCensusLimits, RawCensusSummary,
    RawExecutionBinding, RawKeyKind, ScopeBinding, SelectionRecord, StoreWriter,
    ValidatedGeneration, WorkReader,
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
    collections::BTreeMap,
    sync::Arc,
    sync::atomic::{AtomicBool, Ordering},
};

#[derive(Clone)]
pub(crate) struct PreparedExecution<const N: usize> {
    reducer: Arc<RoutedCandidateReducer<N>>,
    execution: RawExecutionBinding,
    owner_payload_digests: Arc<[String]>,
    execution_identity: Arc<Value>,
}
/// Immutable source-authenticated generated provenance. Sharing this token
/// keeps the SAME actual prepared reducer allocation alive, not just its hash.
#[derive(Clone)]
pub(crate) struct PreparedGeneration<const N: usize> {
    prepared: PreparedExecution<N>,
    generation: ValidatedGeneration,
}
pub(crate) struct PreparedCommit<const N: usize> {
    pub generation: PreparedGeneration<N>,
    pub cleanup_warning: Option<String>,
}
impl<const N: usize> PreparedExecution<N> {
    #[cfg(test)]
    pub fn prepare(
        request: &RoutedCampaignRequest,
        cancel: &AtomicBool,
        observer: &impl Fn(Value),
    ) -> Result<Option<Self>, AppError> {
        Self::prepare_with_source_limit(
            request,
            super::super::input::MAX_STEERING_BYTES,
            cancel,
            observer,
        )
    }
    #[cfg(test)]
    pub fn prepare_with_source_limit(
        request: &RoutedCampaignRequest,
        max_source_bytes: usize,
        cancel: &AtomicBool,
        observer: &impl Fn(Value),
    ) -> Result<Option<Self>, AppError> {
        Self::prepare_with_operation_limits(request, None, max_source_bytes, cancel, observer)
    }
    /// Keep the checkpoint's original source binding, while changing ONLY
    /// resource quotas. A quota failure aborts an operation, never selects a
    /// different rule/route or turns an unresolved leaf into a terminal.
    /// Exponent representations, inputs, guards and programs remain unchanged.
    pub fn prepare_with_operation_limits(
        request: &RoutedCampaignRequest,
        operation_limits: Option<super::lifecycle::operation_limits::OperationLimits>,
        max_source_bytes: usize,
        cancel: &AtomicBool,
        observer: &impl Fn(Value),
    ) -> Result<Option<Self>, AppError> {
        let (selection, arity, limits) =
            Selection::parse_with_byte_limit(&request.selection_json, max_source_bytes)?;
        let mut execution_request = request.clone();
        if let Some(overrides) = operation_limits {
            overrides.apply(&mut execution_request);
        }
        let mut fingerprints = None;
        let reducer = prepare::prepare_with_fingerprints(
            &execution_request,
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
        let identity = execution_identity(request, &selection, arity, &fingerprints)?;
        if cancel.load(Ordering::Acquire) {
            return Ok(None);
        }
        Ok(Some(Self {
            reducer: Arc::new(reducer),
            execution,
            owner_payload_digests: fingerprints.into(),
            execution_identity: Arc::new(identity),
        }))
    }
    pub fn execution(&self) -> &RawExecutionBinding {
        &self.execution
    }
    pub fn reducer(&self) -> &RoutedCandidateReducer<N> {
        &self.reducer
    }
    /// Exact ordered callback vector from THIS native preparation, not a later
    /// re-read or a caller-supplied identity for a different reducer.
    pub fn owner_payload_digests(&self) -> &Arc<[String]> {
        &self.owner_payload_digests
    }
    pub fn execution_identity(&self) -> &Arc<Value> {
        &self.execution_identity
    }
    pub fn authenticate_generation(
        &self,
        generation: ValidatedGeneration,
        scope: &ScopeBinding,
        cancel: &AtomicBool,
    ) -> Result<PreparedGeneration<N>, AppError> {
        if generation.execution() != &self.execution || &generation.state().scope != scope {
            return Err(AppError::input(
                "finite generation differs from actual prepared execution/scope",
            ));
        }
        // Small current selection metadata only. Raw terminal/zero page bytes
        // and source classification are deliberately LAZY until exact census
        // iteration/publication; restore does not claim they were all checked.
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
        check(cancel)?;
        Ok(PreparedGeneration {
            prepared: self.clone(),
            generation,
        })
    }
    /// LOW-LEVEL generated metadata join: authenticate the native Work seal's
    /// predecessor/reducer and atomically persist raw census plus supplied
    /// metadata. This does NOT authenticate a prefix-window transition or prove
    /// query coverage. The backend must separately bind the exact traced starts
    /// and admission region to a typed window successor before calling this.
    pub fn commit_delta_with_generated_metadata(
        &self,
        authenticated: &PreparedGeneration<N>,
        writer: &mut StoreWriter,
        delta: CandidateRoutedCompletedDelta<'_, N>,
        scope: ScopeBinding,
        cursors: Vec<CursorRecord>,
        cancel: &AtomicBool,
    ) -> Result<PreparedCommit<N>, AppError> {
        self.commit_delta_with_directory_accounting(
            authenticated,
            writer,
            delta,
            scope,
            cursors,
            cancel,
            &mut |_| Ok(()),
        )
    }
    pub fn commit_delta_with_directory_accounting(
        &self,
        authenticated: &PreparedGeneration<N>,
        writer: &mut StoreWriter,
        delta: CandidateRoutedCompletedDelta<'_, N>,
        scope: ScopeBinding,
        cursors: Vec<CursorRecord>,
        cancel: &AtomicBool,
        account_directory: &mut dyn FnMut(
            storage::MetadataMemory,
        ) -> Result<(), storage::StorageError>,
    ) -> Result<PreparedCommit<N>, AppError> {
        let previous = writer
            .current()
            .map_err(storage_failure)?
            .ok_or_else(|| AppError::input("initial pending generation missing"))?;
        if !delta.is_for_reducer(&self.reducer)
            || !Arc::ptr_eq(&self.reducer, &authenticated.prepared.reducer)
            || previous.digest() != authenticated.generation.digest()
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
        // Only small selected-rule metadata is cumulative in memory. Native
        // raw key sets are borrowed/paged by the writer; old pages stay intact.
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
        if selected.len() > limits.max_census_records {
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
        let outcome = writer
            .commit_completed(delta, &self.reducer, state, cancel, account_directory)
            .map_err(storage_failure)?;
        // Native delta classification/selection belongs to the SAME engine;
        // unchanged predecessor pages remain authenticated lazily. No old raw
        // key or selected-rule rescanning is necessary on hot advancement.
        Ok(PreparedCommit {
            generation: PreparedGeneration {
                prepared: self.clone(),
                generation: outcome.generation,
            },
            cleanup_warning: outcome.cleanup_warning,
        })
    }
}
impl<const N: usize> PreparedGeneration<N> {
    pub fn generation(&self) -> &ValidatedGeneration {
        &self.generation
    }
    pub fn bind<'a>(
        &'a self,
        reader: &'a WorkReader,
        budget: CandidateRoutedWorkBudget,
        census: CandidateRoutedTraceCensusLimits,
        export: CandidateRoutedDeltaExportLimits,
        cancel: &AtomicBool,
    ) -> Result<CandidateRoutedGeneratedBaseline<'a, N>, AppError> {
        check(cancel)?;
        if reader.generation() != self.generation.digest()
            || reader.snapshot().execution() != &self.prepared.execution
        {
            return Err(AppError::input(
                "prepared generation token differs from reader snapshot",
            ));
        }
        self.prepared
            .reducer
            .bind_trusted_generated_baseline(
                &self.prepared.execution,
                reader.directory(),
                reader,
                budget,
                census,
                export,
            )
            .map_err(generated_failure)
    }
    /// Full exact raw census authority requires successful exhaustion of this
    /// stream. Cold token construction alone has not checked all raw pages.
    pub fn visit_raw_census(
        &self,
        limits: RawCensusLimits,
        cancel: &AtomicBool,
        visitor: impl FnMut(RawKeyKind, &[i64]) -> Result<(), storage::StorageError>,
    ) -> Result<RawCensusSummary, AppError> {
        self.generation
            .visit_raw_census(
                limits,
                cancel,
                |kind, powers| {
                    let key = IntegralKey::try_new(powers.iter().copied()).map_err(|e| {
                        rustred::persistence::finite_work::GeneratedWorkError::new(
                            rustred::persistence::finite_work::GeneratedWorkErrorKind::Corrupt,
                            e,
                        )
                    })?;
                    match kind {
                        RawKeyKind::Terminal => {
                            self.prepared.reducer.validate_generated_terminal(&key)
                        }
                        RawKeyKind::Zero => self.prepared.reducer.validate_generated_zero(&key),
                    }
                    .map_err(Into::into)
                },
                visitor,
            )
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
    let old_count = before.census.selections.len();
    let census = report
        .complete_census()
        .ok_or_else(|| AppError::input("incomplete census preflight"))?;
    let new_count = census.selections().len();
    if add(old_count, new_count)? > limits.max_census_records
        || scope.rows.len() > limits.max_scope_rows
        || cursors.len() != scope.rows.len()
    {
        return Err(AppError::limit(
            "finite census/scope staging record allowance",
        ));
    }
    let mut payload = before.payload_bytes().map_err(storage_failure)?;
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
/// absent. The original per-formula policy stays in this v1 source identity for
/// checkpoint continuity. An independently recorded resource-only override may
/// change execution quotas, never mathematical inputs or rule selection.
fn execution_binding(
    request: &RoutedCampaignRequest,
    selection: &Selection,
    arity: usize,
    fingerprints: &[String],
) -> Result<RawExecutionBinding, AppError> {
    let identity = execution_identity(request, selection, arity, fingerprints)?;
    let bytes = serde_json::to_vec(&identity).map_err(failure)?;
    Ok(RawExecutionBinding {
        physical_arity: u32::try_from(arity).map_err(failure)?,
        program_identity: *blake3::hash(&bytes).as_bytes(),
    })
}
fn execution_identity(
    request: &RoutedCampaignRequest,
    selection: &Selection,
    arity: usize,
    fingerprints: &[String],
) -> Result<Value, AppError> {
    semantic_identity(
        selection,
        arity,
        fingerprints,
        &format!("{:?}", request.reduction_limits.exact_algebra),
        &format!("{:?}", request.reduction_limits.indexed_algebra),
        &format!("{:?}", request.trace_limits.expansion),
    )
}
fn semantic_identity(
    selection: &Selection,
    arity: usize,
    fingerprints: &[String],
    exact: &str,
    indexed: &str,
    expansion: &str,
) -> Result<Value, AppError> {
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
        "exact_algebra":exact,
        "indexed_algebra":indexed,
        "transport_expansion":expansion,
    });
    Ok(identity)
}

/// Recompute the SAME semantic identity against an immutable raw commitment.
/// No native engine is relabelled or imported and no closure is minted here.
pub(crate) fn validate_portable_execution(
    selection_json: &str,
    fingerprints: &[String],
    identity: &Value,
    expected: &RawExecutionBinding,
    max_source_bytes: usize,
) -> Result<(), AppError> {
    let (selection, arity, _) = Selection::parse_with_byte_limit(selection_json, max_source_bytes)?;
    let policy = |name| {
        identity[name]
            .as_str()
            .ok_or_else(|| AppError::input("finite execution policy absent"))
    };
    let actual = semantic_identity(
        &selection,
        arity,
        fingerprints,
        policy("exact_algebra")?,
        policy("indexed_algebra")?,
        policy("transport_expansion")?,
    )?;
    if &actual != identity
        || u32::try_from(arity).ok() != Some(expected.physical_arity)
        || blake3::hash(&serde_json::to_vec(&actual).map_err(failure)?).as_bytes()
            != &expected.program_identity
    {
        return Err(AppError::input(
            "portable selection differs from captured native execution identity",
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests;
