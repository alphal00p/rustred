//! Exact completed Route/Apply records. Entry scope stays in the normal request;
//! source guards, active cycles and native memory limits precede every hit.
use super::super::trace::support;
use super::*;
use crate::persistence::application_cache::{
    ApplicationCacheBinding, ApplicationCacheKey, ApplicationCacheRecord,
};
use crate::reduction::{GuardedCacheBacking, GuardedIntermediateCapture};
use crate::solver::CandidateEntryAdmission;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct CandidateRoutedPersistentCacheStatistics {
    pub lookups: usize,
    pub route_hits: usize,
    pub requested_root_hits: usize,
    pub intermediate_hits: usize,
    pub apply_hits: usize,
    pub misses: usize,
    pub captured_records: usize,
    pub skipped_records: usize,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct CandidateRoutedCacheExportStatistics {
    pub exported_records: usize,
    pub skipped_partial_records: usize,
}

#[derive(Default)]
pub(super) struct PersistentRequest<'a, 'storage> {
    // The request reborrows stores only for its own traversal. Their captured
    // data can outlive that borrow, allowing the same observer to cover the
    // subsequent saved terminal substitutions without retaining a store borrow.
    pub backing: Option<&'a mut (dyn GuardedCacheBacking + 'storage)>,
    pub capture: Option<&'a mut (dyn GuardedIntermediateCapture + 'storage)>,
    pub loaded: Option<ApplicationCacheRecord>,
    pub requested_roots: std::collections::BTreeSet<ApplicationCacheKey>,
}

impl PersistentRequest<'_, '_> {
    pub(super) fn report(&mut self, statistics: CandidateRoutedPersistentCacheStatistics) {
        if let Some(capture) = &mut self.capture {
            capture.report_statistics(crate::reduction::GuardedCaptureStatistics {
                captured_records: statistics.captured_records,
                skipped_records: statistics.skipped_records,
                ..Default::default()
            });
        }
    }
}

fn failure(error: impl std::fmt::Display) -> CandidateRoutedError {
    CandidateRoutedError::InvalidInput(format!("persistent routed cache: {error}"))
}

fn typed_failure(error: impl std::error::Error + 'static) -> CandidateRoutedError {
    let mut source: &(dyn std::error::Error + 'static) = &error;
    loop {
        if let Some(resource) =
            source.downcast_ref::<symbolica::utils::resources::ResourceLimitError>()
        {
            return crate::reduction::ReductionError::Resources(resource.clone()).into();
        }
        match source.source() {
            Some(next) => source = next,
            None => return failure(error),
        }
    }
}

fn cache_key<const N: usize>(node: &Node<N>, arity: usize) -> ApplicationCacheKey {
    match node {
        Node::Route(key) => ApplicationCacheKey::route(key),
        Node::Apply(owner, key) => ApplicationCacheKey::apply(owner[..arity].to_vec(), key),
    }
}

impl<const N: usize> RoutedCandidateApplication<N> {
    /// The caller's authenticated digest must cover the entire immutable
    /// application, including routes, scope, zeros and terminal substitutions.
    /// Producer revision strings alone are not a semantic binding.
    pub fn persistent_cache_binding(&self, artifact_digest: [u8; 32]) -> ApplicationCacheBinding {
        ApplicationCacheBinding {
            artifact_digest,
            family_fingerprint: self.family().fingerprint().to_owned(),
            physical_arity: self.family().denominator_count(),
            layer: "saved-application-route-apply-v1".into(),
        }
    }

    fn validate_persistent_binding(
        &self,
        binding: &ApplicationCacheBinding,
        backing: Option<&dyn GuardedCacheBacking>,
        capture: bool,
    ) -> Result<(), CandidateRoutedError> {
        if *binding != self.persistent_cache_binding(binding.artifact_digest)
            || backing.is_some_and(|b| b.binding() != binding)
        {
            return Err(failure(
                "artifact, family, arity or solver-layer binding mismatch",
            ));
        }
        // A permissive session may retain unresolved nodes in its internal DAG.
        // Distributed capture/export is deliberately strict, even when those
        // unknown terms later cancel in the final linear combination.
        if capture && self.unresolved_policy != UnresolvedPolicy::Reject {
            return Err(failure(
                "completed record capture requires UnresolvedPolicy::Reject",
            ));
        }
        Ok(())
    }

    pub fn reduce_linear_combination_with_cache<'a>(
        &mut self,
        terms: impl IntoIterator<Item = (IntegralKey, Coefficient)>,
        admission: CandidateEntryAdmission<'_, N>,
        binding: &ApplicationCacheBinding,
        backing: Option<&'a mut dyn GuardedCacheBacking>,
        capture: Option<&'a mut dyn GuardedIntermediateCapture>,
    ) -> Result<CandidateRoutedReduction, CandidateRoutedError> {
        self.validate_persistent_binding(binding, backing.as_deref(), capture.is_some())?;
        self.reduce_observed_request(
            terms,
            admission,
            None,
            None,
            PersistentRequest {
                backing,
                capture,
                ..Default::default()
            },
        )
    }

    pub fn reduce_routed_linear_combination_with_cache<'a>(
        &mut self,
        terms: impl IntoIterator<Item = (IntegralKey, Coefficient)>,
        admission: CandidateEntryAdmission<'_, N>,
        binding: &ApplicationCacheBinding,
        backing: Option<&'a mut dyn GuardedCacheBacking>,
        capture: Option<&'a mut dyn GuardedIntermediateCapture>,
    ) -> Result<CandidateRoutedReduction, CandidateRoutedError> {
        self.validate_persistent_binding(binding, backing.as_deref(), capture.is_some())?;
        let mut request = super::engine::Request::with_persistent(PersistentRequest {
            backing,
            capture,
            ..Default::default()
        });
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            self.reduce_routed_request(terms, admission, &mut request)
        }));
        self.release_request_pins(&mut request.pins);
        self.work.merge_work(request.stats.work);
        self.cache_work.merge(request.stats.cache);
        if result.is_ok() {
            request.persistent.report(request.stats.persistent_cache);
        }
        match result {
            Ok(result) => result,
            Err(panic) => std::panic::resume_unwind(panic),
        }
    }

    /// Observe the same cached/captured traversal. Telemetry faults detach the
    /// observer; backing/capture errors and typed native failures are unchanged.
    #[allow(clippy::too_many_arguments)] // Existing cache API plus observer/control.
    pub fn reduce_linear_combination_with_cache_and_observation<'a>(
        &mut self,
        terms: impl IntoIterator<Item = (IntegralKey, Coefficient)>,
        admission: CandidateEntryAdmission<'_, N>,
        binding: &ApplicationCacheBinding,
        backing: Option<&'a mut dyn GuardedCacheBacking>,
        capture: Option<&'a mut dyn GuardedIntermediateCapture>,
        observer: &mut CandidateRoutedApplicationObserver<'_>,
        cancel: Option<&std::sync::atomic::AtomicBool>,
    ) -> Result<CandidateRoutedReduction, CandidateRoutedError> {
        self.validate_persistent_binding(binding, backing.as_deref(), capture.is_some())?;
        self.reduce_observed_request(
            terms,
            admission,
            progress::Progress::from_observer(observer),
            cancel,
            PersistentRequest {
                backing,
                capture,
                ..Default::default()
            },
        )
    }

    /// Initial routing, cache lookup/capture and observation share one native
    /// request; no diagnostic traversal or additional admission path is used.
    #[allow(clippy::too_many_arguments)] // Existing cache API plus observer/control.
    pub fn reduce_routed_linear_combination_with_cache_and_observation<'a>(
        &mut self,
        terms: impl IntoIterator<Item = (IntegralKey, Coefficient)>,
        admission: CandidateEntryAdmission<'_, N>,
        binding: &ApplicationCacheBinding,
        backing: Option<&'a mut dyn GuardedCacheBacking>,
        capture: Option<&'a mut dyn GuardedIntermediateCapture>,
        observer: &mut CandidateRoutedApplicationObserver<'_>,
        cancel: Option<&std::sync::atomic::AtomicBool>,
    ) -> Result<CandidateRoutedReduction, CandidateRoutedError> {
        self.validate_persistent_binding(binding, backing.as_deref(), capture.is_some())?;
        self.reduce_observed_routed_request(
            terms,
            admission,
            progress::Progress::from_observer(observer),
            cancel,
            PersistentRequest {
                backing,
                capture,
                ..Default::default()
            },
        )
    }

    fn target_is_zero(&self, key: &IntegralKey) -> bool {
        let shared = &self.context.shared;
        crate::solver::candidate_reduction::evaluator::CandidateEvaluator {
            context: &shared.context,
            root_sector: [true; N],
            ordering: &Default::default(),
            rules: &[],
            whole_piece_alternatives: &[],
            source_conditions: &shared.source_conditions,
            zero_sectors: &shared.zero_sectors,
            limits: self.context.limits,
        }
        .is_zero(key)
    }

    /// Validate one starting key using native structural/source checks and the
    /// explicitly selected coverage policy. This performs no reduction.
    pub fn validate_entry_for_reduction(
        &self,
        target: &IntegralKey,
        admission: CandidateEntryAdmission<'_, N>,
    ) -> Result<(), CandidateRoutedError> {
        self.target_conditions(target)?;
        self.admit_entry(admission, target)
    }

    fn target_conditions(
        &self,
        key: &IntegralKey,
    ) -> Result<Vec<Coefficient>, CandidateRoutedError> {
        let shared = &self.context.shared;
        let evaluator = crate::solver::candidate_reduction::evaluator::CandidateEvaluator {
            context: &shared.context,
            root_sector: [true; N],
            ordering: &Default::default(),
            rules: &[],
            whole_piece_alternatives: &[],
            source_conditions: &shared.source_conditions,
            zero_sectors: &shared.zero_sectors,
            limits: self.context.limits,
        };
        let mut conditions = Vec::new();
        evaluator.validate_target_with_conditions(key, &mut |condition| {
            if !condition.is_constant() {
                let coefficient: Coefficient = condition.clone().into();
                if !conditions.contains(&coefficient) {
                    conditions.push(coefficient);
                }
            }
        })?;
        Ok(conditions)
    }

    pub(super) fn terminal_is_declared(
        &self,
        key: &IntegralKey,
    ) -> Result<bool, CandidateRoutedError> {
        Ok(match &self.owners {
            ApplicationOwners::Eager(programs) => programs
                .owners
                .get(&support::<N>(key))
                .is_some_and(|owner| owner.batches[0].terminals.contains(key)),
            ApplicationOwners::Deferred(programs) => programs.contains_terminal(key),
        })
    }

    /// Native semantic validation for preparation resume and external records.
    /// This authenticates solver structure and declared terminals, not new IBPs.
    pub fn validate_completed_cache_record(
        &self,
        binding: &ApplicationCacheBinding,
        record: &ApplicationCacheRecord,
    ) -> Result<(), CandidateRoutedError> {
        self.validate_persistent_binding(binding, None, false)?;
        record
            .key
            .validate(binding.physical_arity)
            .map_err(typed_failure)?;
        let target = record.key.integral().map_err(typed_failure)?;
        for condition in self.target_conditions(&target)? {
            if !record.nonzero_conditions.contains(&condition) {
                return Err(failure("record omits an active native source condition"));
            }
        }
        let owner_support = support::<N>(&target);
        match record.key.stage() {
            crate::persistence::application_cache::ApplicationCacheStage::GuardedRoot => {
                return Err(failure(
                    "GuardedRoot is not a saved-application operational stage",
                ));
            }
            crate::persistence::application_cache::ApplicationCacheStage::Route => {
                if !self.owners.contains(&owner_support)
                    && !self.routes.contains_key(&owner_support)
                    && !(record.terms.is_empty() && self.target_is_zero(&target))
                {
                    return Err(failure("record has no authenticated owner or route"));
                }
            }
            crate::persistence::application_cache::ApplicationCacheStage::Apply => {
                if !self.owners.contains(&owner_support) {
                    return Err(failure("record has no authenticated Apply owner"));
                }
            }
        }
        let context = self.context.shared.context.base();
        for (terminal, coefficient) in &record.terms {
            if terminal.powers().len() != binding.physical_arity
                || coefficient.is_zero()
                || !self.terminal_is_declared(terminal)?
            {
                return Err(failure("record contains an undeclared or invalid terminal"));
            }
            context
                .validate_with_limits(coefficient, self.context.limits.exact_algebra)
                .map_err(typed_failure)?;
        }
        for guard in &record.nonzero_conditions {
            if guard.is_zero() {
                return Err(failure("record has a vanished nonzero condition"));
            }
            context
                .validate_with_limits(guard, self.context.limits.exact_algebra)
                .map_err(typed_failure)?;
        }
        Ok(())
    }

    pub(super) fn lookup_persistent_node(
        &self,
        node: &Node<N>,
        conditions: &mut Vec<Coefficient>,
        persistent: &mut PersistentRequest<'_, '_>,
        stats: &mut CandidateRoutedPersistentCacheStatistics,
    ) -> Result<bool, CandidateRoutedError> {
        let Some(backing) = &mut persistent.backing else {
            return Ok(false);
        };
        let arity = self.family().denominator_count();
        let key = cache_key(node, arity);
        // A persisted result never supplies a missing owner or transport.
        let physical_support = support::<N>(node.key());
        match node {
            Node::Route(_)
                if !self.owners.contains(&physical_support)
                    && !self.routes.contains_key(&physical_support) =>
            {
                return Ok(false);
            }
            Node::Apply(owner, _) if owner != &physical_support || !self.owners.contains(owner) => {
                return Err(failure("Apply key has no matching authenticated owner"));
            }
            _ => {}
        }
        stats.lookups = stats.lookups.saturating_add(1);
        let context = self.context.shared.context.base();
        let Some(record) = backing.lookup(&key, context).map_err(typed_failure)? else {
            stats.misses = stats.misses.saturating_add(1);
            return Ok(false);
        };
        if record.key != key {
            return Err(failure("lookup returned a different key"));
        }
        record.key.validate(arity).map_err(typed_failure)?;
        for (terminal, coefficient) in &record.terms {
            if terminal.powers().len() != arity
                || coefficient.is_zero()
                || !self.terminal_is_declared(terminal)?
            {
                return Err(failure("record contains an undeclared or invalid terminal"));
            }
            context
                .validate_with_limits(coefficient, self.context.limits.exact_algebra)
                .map_err(typed_failure)?;
        }
        for guard in &record.nonzero_conditions {
            if guard.is_zero() {
                return Err(failure("record has a vanished nonzero condition"));
            }
            context
                .validate_with_limits(guard, self.context.limits.exact_algebra)
                .map_err(typed_failure)?;
            if !conditions.contains(guard) {
                conditions.push(guard.clone());
            }
        }
        if persistent.requested_roots.contains(&key) {
            stats.requested_root_hits = stats.requested_root_hits.saturating_add(1);
        } else {
            stats.intermediate_hits = stats.intermediate_hits.saturating_add(1);
        }
        match node {
            Node::Route(_) => stats.route_hits = stats.route_hits.saturating_add(1),
            Node::Apply(_, _) => stats.apply_hits = stats.apply_hits.saturating_add(1),
        }
        persistent.loaded = Some(record);
        Ok(true)
    }

    pub(super) fn capture_persistent_node(
        &self,
        node: &Node<N>,
        persistent: &mut PersistentRequest<'_, '_>,
        stats: &mut CandidateRoutedPersistentCacheStatistics,
    ) -> Result<(), CandidateRoutedError> {
        let Some(capture) = &mut persistent.capture else {
            return Ok(());
        };
        let context = self.context.shared.context.base();
        let key = cache_key(node, self.family().denominator_count());
        if !capture.wants(&key, context).map_err(typed_failure)? {
            stats.skipped_records = stats.skipped_records.saturating_add(1);
            return Ok(());
        }
        let entry = self
            .cache
            .get(node)
            .ok_or_else(|| failure("completed node is absent"))?;
        for terminal in entry.terms.terminal_keys() {
            if !self.terminal_is_declared(terminal)? {
                return Err(failure(
                    "strict completed node contains an undeclared terminal",
                ));
            }
        }
        let terms = match entry.terms.materialize(context, self.context.limits) {
            Ok(terms) => terms,
            // This optional materialization may exceed the allowance even
            // when later native factorized combinations would cancel it.
            Err(CandidateReductionError::Application(
                crate::reduction::ReductionError::ExactAlgebra(
                    crate::algebra::ExactAlgebraError::ResourceLimit { .. }
                    | crate::algebra::ExactAlgebraError::ExponentLimit { .. }
                    | crate::algebra::ExactAlgebraError::ResourceCountOverflow { .. }
                    | crate::algebra::ExactAlgebraError::ExponentArithmeticOverflow { .. },
                ),
            )) => {
                stats.skipped_records = stats.skipped_records.saturating_add(1);
                return Ok(());
            }
            Err(error) => return Err(error.into()),
        };
        let record = ApplicationCacheRecord {
            key,
            terms,
            nonzero_conditions: entry.conditions.clone(),
        };
        capture
            .store_completed(&record, context)
            .map_err(typed_failure)?;
        stats.captured_records = stats.captured_records.saturating_add(1);
        Ok(())
    }

    /// Materialize one completed resident node, excluding partial results.
    /// No lookup, reduction, or terminal substitution is performed.
    pub fn completed_cache_record(
        &self,
        binding: &ApplicationCacheBinding,
        key: &ApplicationCacheKey,
    ) -> Result<Option<ApplicationCacheRecord>, CandidateRoutedError> {
        self.validate_persistent_binding(binding, None, false)?;
        key.validate(binding.physical_arity)
            .map_err(typed_failure)?;
        let integral = key.integral().map_err(typed_failure)?;
        let node = match key.stage() {
            crate::persistence::application_cache::ApplicationCacheStage::Route => {
                Node::Route(integral)
            }
            crate::persistence::application_cache::ApplicationCacheStage::Apply => Node::Apply(
                std::array::from_fn(|i| key.owner().get(i).copied().unwrap_or(false)),
                integral,
            ),
            _ => return Err(failure("GuardedRoot is not a routed operational stage")),
        };
        let Some(entry) = self.cache.get(&node) else {
            return Ok(None);
        };
        for terminal in entry.terms.terminal_keys() {
            if !self.terminal_is_declared(terminal)? {
                return Ok(None);
            }
        }
        let record = ApplicationCacheRecord {
            key: key.clone(),
            terms: entry
                .terms
                .materialize(self.context.shared.context.base(), self.context.limits)?,
            nonzero_conditions: entry.conditions.clone(),
        };
        self.validate_completed_cache_record(binding, &record)?;
        Ok(Some(record))
    }

    /// Export resident completed nodes, omitting every node with an unresolved
    /// terminal. Exact cancellation may make a parent complete even when an
    /// unfinished child is omitted; its full native guard union is preserved.
    pub fn export_completed(
        &self,
        binding: &ApplicationCacheBinding,
        backing: &mut dyn GuardedCacheBacking,
    ) -> Result<usize, CandidateRoutedError> {
        Ok(self
            .export_completed_with_statistics(binding, backing)?
            .exported_records)
    }

    pub fn export_completed_with_statistics(
        &self,
        binding: &ApplicationCacheBinding,
        backing: &mut dyn GuardedCacheBacking,
    ) -> Result<CandidateRoutedCacheExportStatistics, CandidateRoutedError> {
        self.validate_persistent_binding(binding, Some(backing), false)?;
        let context = self.context.shared.context.base();
        let mut statistics = CandidateRoutedCacheExportStatistics::default();
        for (node, entry) in &self.cache {
            let mut complete = true;
            for terminal in entry.terms.terminal_keys() {
                if !self.terminal_is_declared(terminal)? {
                    complete = false;
                    break;
                }
            }
            if !complete {
                statistics.skipped_partial_records =
                    statistics.skipped_partial_records.saturating_add(1);
                continue;
            }
            let record = ApplicationCacheRecord {
                key: cache_key(node, self.family().denominator_count()),
                terms: entry.terms.materialize(context, self.context.limits)?,
                nonzero_conditions: entry.conditions.clone(),
            };
            backing
                .store_completed(&record, context)
                .map_err(typed_failure)?;
            statistics.exported_records = statistics.exported_records.saturating_add(1);
        }
        Ok(statistics)
    }
}
