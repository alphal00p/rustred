//! Complete-only immutable source capture. Const-N ends at this boundary;
//! master normalization/refinement consumes a non-generic inventory.
use super::*;
use rustred::family::{IntegralFamily, IntegralKey};
use std::{collections::BTreeSet, sync::Arc};

#[derive(Clone, Copy, Debug)]
pub(crate) struct CaptureLimits {
    pub max_terminal_keys: usize,
    /// Logical key/powers payload, not allocator/tree overhead or total RSS.
    pub max_terminal_payload_bytes: usize,
}

/// No public/deserialize/JSON constructor. Only a completed backend Session
/// can capture this after exhausting its actual source-classified raw stream.
pub(crate) struct CompletedFiniteSource {
    request: OwnerDomainWalkRequest,
    family: Arc<IntegralFamily>,
    payloads: Arc<[String]>,
    execution_identity: Arc<Value>,
    terminals: BTreeSet<IntegralKey>,
    summary: Value,
    provenance: storage::RawProvenance,
    window_policy: WindowLimits,
}

impl CompletedFiniteSource {
    #[cfg(test)]
    pub(crate) fn with_repeated_raw_runs_for_test(mut self, runs: usize) -> Self {
        self.provenance = self.provenance.with_repeated_raw_runs_for_test(runs);
        self.summary["source_generation_blake3"] = json!(
            blake3::Hash::from(self.provenance.digest())
                .to_hex()
                .to_string()
        );
        self
    }

    pub(crate) fn request(&self) -> &OwnerDomainWalkRequest {
        &self.request
    }
    pub(crate) fn family_owner(&self) -> &Arc<IntegralFamily> {
        &self.family
    }
    pub(crate) fn terminal_keys(&self) -> &BTreeSet<IntegralKey> {
        &self.terminals
    }
    pub(crate) fn owner_payload_digests(&self) -> &[String] {
        &self.payloads
    }
    pub(crate) fn execution_identity(&self) -> &Value {
        &self.execution_identity
    }
    pub(crate) fn summary(&self) -> &Value {
        &self.summary
    }
    pub(crate) fn source_identity(&self) -> [u8; 32] {
        self.provenance.digest()
    }
    pub(crate) fn generation_number(&self) -> u64 {
        self.provenance.generation()
    }
    pub(crate) fn raw_provenance(&self) -> &storage::RawProvenance {
        &self.provenance
    }
    pub(crate) fn window_policy(&self) -> WindowLimits {
        self.window_policy
    }
}

impl<const N: usize> Session<N> {
    pub(crate) fn capture_completed_source(
        &self,
        limits: CaptureLimits,
        cancel: &AtomicBool,
    ) -> Result<CompletedFiniteSource, AppError> {
        check(cancel)?;
        let progress = self.progress()?;
        if self.stopped
            || !progress.required_complete
            || !progress.all_rows_exhausted
            || progress.auxiliary_rows != 0
        {
            return Err(AppError::input(
                "finite publication requires every exact required row exhausted and no auxiliary admission",
            ));
        }
        let generation = self.generation()?;
        if generation.digest() != self.authenticated.generation().digest()
            || generation.state().scope != self.scope
        {
            return Err(AppError::internal_invariant(
                "finite publication capture generation differs",
            ));
        }
        let mut terminals = BTreeSet::new();
        let mut terminal_payload = 0usize;
        let raw =
            self.authenticated
                .visit_raw_census(self.policy.storage.raw, cancel, |kind, key| {
                    if kind == storage::RawKeyKind::Terminal {
                        let bytes = key
                            .len()
                            .checked_mul(std::mem::size_of::<i64>())
                            .and_then(|n| n.checked_add(std::mem::size_of::<IntegralKey>()))
                            .and_then(|n| n.checked_add(terminal_payload));
                        if terminals.len() >= limits.max_terminal_keys
                            || bytes.is_none_or(|n| n > limits.max_terminal_payload_bytes)
                        {
                            return Err(rustred::persistence::finite_work::GeneratedWorkError::new(
                            rustred::persistence::finite_work::GeneratedWorkErrorKind::Resource,
                            "finite publication terminal capture payload",
                        )
                        .into());
                        }
                        let key = IntegralKey::try_new(key.iter().copied()).map_err(|e| {
                            storage::StorageError::from(
                            rustred::persistence::finite_work::GeneratedWorkError::new(
                                rustred::persistence::finite_work::GeneratedWorkErrorKind::Corrupt,
                                e,
                            ),
                        )
                        })?;
                        if !terminals.insert(key) {
                            return Err(rustred::persistence::finite_work::GeneratedWorkError::new(
                            rustred::persistence::finite_work::GeneratedWorkErrorKind::Corrupt,
                            "raw terminal union stream repeated a key",
                        )
                        .into());
                        }
                        terminal_payload = bytes.unwrap();
                    }
                    Ok(())
                })?;
        if u64::try_from(terminals.len()).ok() != Some(raw.terminals) {
            return Err(AppError::internal_invariant(
                "finite captured raw terminal count differs",
            ));
        }
        let programs = self.prepared.reducer().programs();
        let arity = usize::try_from(generation.execution().physical_arity)
            .map_err(|e| AppError::input(e.to_string()))?;
        let mut rules = 0usize;
        let mut terminal_records = 0usize;
        let mut batches = Vec::new();
        for (owner, batch, batch_rules, batch_terminals) in programs.installed_inventory() {
            check(cancel)?;
            rules = rules
                .checked_add(batch_rules)
                .ok_or_else(|| AppError::limit("installed rule count"))?;
            terminal_records = terminal_records
                .checked_add(batch_terminals)
                .ok_or_else(|| AppError::limit("installed terminal count"))?;
            batches.push(json!({"owner":owner[..arity].iter().map(|&b|if b {'1'}else{'0'}).collect::<String>(),
                "batch":batch,"rules":batch_rules,"declared_terminals":batch_terminals}));
        }
        let selections = &generation.state().census.selections;
        let multiplicity = selections
            .iter()
            .try_fold(0u64, |sum, r| sum.checked_add(r.multiplicity))
            .ok_or_else(|| AppError::limit("finite captured rule multiplicity"))?;
        let selected = selections
            .iter()
            .map(|r| (&r.owner, r.batch, r.selected))
            .collect::<BTreeSet<_>>();
        let summary = json!({"schema":"rustred.finite-demand-inventory.v1","complete":true,
            "authority":"trusted_generated","independently_verified":false,
            "family_fingerprint":programs.context().family().fingerprint(),
            "family_count":1,"cross_family_merged":false,"storage_capacity":N,
            "source_generation":generation.generation(),
            "source_generation_blake3":blake3::Hash::from(generation.digest()).to_hex().to_string(),
            "installed":{"owners":programs.owner_count(),"batches":batches.len(),"rules":rules,
                "declared_terminal_records":terminal_records,"owner_qualified_unique_declared_terminals":programs.terminal_count(),
                "batch_inventory":batches,"scope":"actually installed immutable owner programs, including preferred programs and overlays"},
            "encountered":{"rules":selected.len(),"composite_rule_identities":selections.len(),
                "rule_applications":multiplicity,"terminals":raw.terminals,"visited_zeros":raw.zeros,
                "complete_inventory":true,"scope":"completed native Work over the exact original and appended required input union",
                "symbolic_classification_occurrences":null,"coefficient_nonzero_reachability_claim":false},
            "input_occurrences":generation.state().input_occurrences,"completed_work":generation.state().completed_work,
            "required_rows":progress.required_rows,"all_rows_exhausted":true,"auxiliary_rows":0,
            "master_minimality_claim":false,"family_closure_claim":false,
            "raw_census_stream":{"complete":true,"zeros_materialized":false}});
        check(cancel)?;
        Ok(CompletedFiniteSource {
            request: self.source.clone(),
            family: Arc::clone(programs.context().family_owner()),
            payloads: Arc::clone(self.prepared.owner_payload_digests()),
            terminals,
            summary,
            execution_identity: Arc::clone(self.prepared.execution_identity()),
            provenance: storage::RawProvenance::capture(&generation),
            window_policy: self.policy.windows,
        })
    }
}

/// Read-only semantic cross-check of portable metadata against the original
/// immutable generation. Returns no cursor, resume token or Work capability.
pub(crate) fn validate_portable_scope(
    archive: &storage::RawProvenance,
    source: &Value,
    max_source_bytes: usize,
    planning_limits: Option<super::super::windows::WindowPlanningLimits>,
    cancel: &AtomicBool,
) -> Result<(), AppError> {
    let capacity = source["inventory"]["storage_capacity"]
        .as_u64()
        .ok_or_else(|| AppError::input("finite source storage capacity absent"))?;
    // Use the same configured capacities as native preparation. This dispatches
    // only source/count checks, never the master/reduction tail. One-slot native
    // fixtures are also readable in a capacity-dispatch test build.
    #[cfg(all(test, feature = "capacity-dispatch"))]
    if capacity == 1 {
        return validate_scope::<1>(archive, source, max_source_bytes, planning_limits, cancel);
    }
    macro_rules! dispatch {($($n:literal),*)=>{match capacity {
        $($n=>validate_scope::<$n>(archive,source,max_source_bytes,planning_limits,cancel),)*
        _=>Err(AppError::input("finite publication source storage capacity unsupported by this build")),
    }}}
    rustred::with_app_runtime_arities!(dispatch)
}
fn validate_scope<const N: usize>(
    archive: &storage::RawProvenance,
    source: &Value,
    max_source_bytes: usize,
    planning_limits: Option<super::super::windows::WindowPlanningLimits>,
    cancel: &AtomicBool,
) -> Result<(), AppError> {
    let text = |name| {
        source[name]
            .as_str()
            .ok_or_else(|| AppError::input("finite portable source string absent"))
    };
    let request_binding = text("request_binding")?.to_string();
    if original_source_identity(&request_binding)? != archive.state().scope.original_source {
        return Err(AppError::input(
            "portable original request differs from generation",
        ));
    }
    let arity = archive.execution().physical_arity as usize;
    if !rustred::fits_storage(arity, N) {
        return Err(AppError::input("portable physical/storage arity differs"));
    }
    let amendments = source["amendments"]
        .as_array()
        .ok_or_else(|| AppError::input("finite amendments absent"))?
        .iter()
        .enumerate()
        .map(|(i, a)| {
            Ok(crate::OwnerDomainWalkAmendment {
                path: PathBuf::from(format!("captured-source-{i}")),
                text: a
                    .as_str()
                    .ok_or_else(|| AppError::input("finite amendment string absent"))?
                    .to_owned(),
            })
        })
        .collect::<Result<Vec<_>, AppError>>()?;
    let count = archive.state().scope.rows.len();
    let parsed = finite_scope::parse_bound_sources(
        text("original_queries")?,
        &amendments,
        request_binding,
        arity,
        count.max(1),
        max_source_bytes,
        count,
        max_source_bytes,
    )?;
    if parsed.rows.len() != count || archive.state().cursors.len() != count {
        return Err(AppError::input(
            "portable source row count differs from generation",
        ));
    }
    let limits: WindowLimits = serde_json::from_value(source["window_policy"].clone())
        .map_err(|e| AppError::input(e.to_string()))?;
    for (ordinal, row) in parsed.rows.iter().enumerate() {
        check(cancel)?;
        let source_digest = parsed.sources[row.source]["blake3"]
            .as_str()
            .ok_or_else(|| AppError::input("portable source digest absent"))?;
        let origin = origin_identity(
            row.source,
            source_digest,
            ordinal,
            &row.query.id,
            row.query.auxiliary,
            row.query.role_declared,
        )?;
        // Original/amended source validity was authenticated by capture. Their
        // geometric fields are unchanged by phase selection; only native padded
        // storage is added here, exactly as query_domain/rescue::domain do.
        let region = rustred::solver::RootRegionInput {
            support: rustred::storage_array::<_, N>(&row.query.owner, false)
                .ok_or_else(|| AppError::input("portable owner arity"))?,
            lower: rustred::storage_array::<_, N>(&row.query.lower, 0)
                .ok_or_else(|| AppError::input("portable lower arity"))?
                .to_vec(),
            upper: rustred::storage_array::<_, N>(&row.query.upper, Some(0))
                .ok_or_else(|| AppError::input("portable upper arity"))?
                .to_vec(),
            rank: row.query.rank,
            powers: row.query.powers,
        };
        let expected = ScopeRow {
            origin: origin.clone(),
            role: if row.query.auxiliary {
                ScopeRole::Auxiliary
            } else {
                ScopeRole::Required
            },
            exact_domain: domain_identity(arity, &region)?,
        };
        let cursor = &archive.state().cursors[ordinal];
        if expected.role != ScopeRole::Required
            || archive.state().scope.rows[ordinal] != expected
            || PrefixWindows::<N>::source_binding(&origin, &region, arity, limits)
                .map_err(window_error)?
                != cursor.source_policy
        {
            return Err(AppError::input(
                "portable source geometry/role/window policy contradicts native generation",
            ));
        }
        let windows = PrefixWindows::try_new_with_planning_limits(
            &origin,
            region,
            arity,
            limits,
            planning_limits.unwrap_or_else(|| limits.into()),
            cancel,
        )
        .map_err(window_error)?;
        windows
            .validate_completed_commitment(
                &cursor.encoded_cursor,
                cursor.completed_windows,
                cursor.input_occurrences,
                cancel,
            )
            .map_err(window_error)?;
    }
    Ok(())
}
