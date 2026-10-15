//! Prepared exact application of an immutable saved publication.
//!
//! Owners, verified momentum routes and the actual published entry union are
//! prepared once. Mutable memoization belongs to the returned handle. Compact
//! runtime exports retain native Symbolica coefficients and inherited authority;
//! they are application data, not new source or closure certificates.
#[cfg(test)]
mod admission_diagnostic;
mod cache_pack;
mod charts;
mod codec;
mod options;
mod package;
mod products;
mod progress;
mod scope;
#[cfg(test)]
mod tests;
mod transport;
mod witnesses;

use std::collections::{BTreeMap, BTreeSet, HashSet};
use std::path::Path;
use std::sync::{Arc, atomic::AtomicBool};
use std::time::{Duration, Instant};

use rustred::algebra::Coefficient;
use rustred::family::{IntegralFamily, IntegralKey};
use rustred::solver::{
    CandidateRoutedApplicationStatistics, CandidateRoutedCacheStatistics, CandidateRoutedReduction,
    CandidateStatistics, RoutedCandidateApplication,
};
use serde_json::Value;

use super::routed_campaign::{
    RoutedCampaignRequest,
    entry::{self, RequestedEntryDomain},
    input::Selection,
    prepare,
};
use crate::{AppError, MasterReductionArtifact};

pub use cache_pack::{
    SavedApplicationCacheBundle, SavedApplicationCacheBundleWriter,
    SavedApplicationCacheCopyStatistics, SavedApplicationCacheLookupOutcome,
    SavedApplicationCachePack, SavedApplicationCachePackLimits,
    SavedApplicationCachePackStatistics, SavedApplicationCacheWriter,
};
pub use charts::SavedApplicationProductChartStatistics;
pub use options::SavedApplicationOptions;
pub use package::{
    export_saved_application, load_saved_application, prepare_saved_application,
    saved_application_arity, saved_application_domains,
};
pub use products::{SavedApplicationProduct, SavedApplicationProductStatistics};
pub use witnesses::{
    SavedApplicationWitness, SavedApplicationWitnessKind, saved_application_boundary_witnesses,
};

/// Versioned manifest for application-only native generated data.
pub const SAVED_APPLICATION_SCHEMA: &str = "rustred.saved-application.v1";

/// Wall-clock preparation receipts, separate from application and export work.
/// Phases are disjoint cumulative durations, not per-phase RSS measurements.
#[derive(Clone, Debug, Default)]
pub struct SavedApplicationPreparationStatistics {
    pub total: Duration,
    pub metadata: Duration,
    /// Source native verification/compact export check, or runtime map import.
    pub terminal_maps: Duration,
    /// Owner payload reads, transport decompression and digest admission.
    pub owner_payloads: Duration,
    /// Shared native context/zero proofs and owner admission. Formula
    /// preparation is eager for source publication, deferred for runtime load.
    pub owner_preparation: Duration,
    pub route_preparation: Duration,
    /// Remaining entry validation and assembly outside the measured phases.
    pub other: Duration,
}
impl SavedApplicationPreparationStatistics {
    fn finish(&mut self, total: Duration) {
        self.total = total;
        self.other = total.saturating_sub(
            self.metadata
                + self.terminal_maps
                + self.owner_payloads
                + self.owner_preparation
                + self.route_preparation,
        );
    }
}

/// One original query ID and its exact correlated entry predicates.
#[derive(Clone, Debug)]
pub struct SavedApplicationDomain<const N: usize> {
    pub id: String,
    pub physical_arity: usize,
    /// Native geometry proves this historical request has no integer inputs.
    pub is_empty: bool,
    pub region: rustred::solver::RootRegionInput<N>,
}

/// Integral identity always includes its authenticated family.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct SavedApplicationIntegral {
    family_fingerprint: String,
    integral: IntegralKey,
}
impl SavedApplicationIntegral {
    pub fn family_fingerprint(&self) -> &str {
        &self.family_fingerprint
    }
    pub fn integral(&self) -> &IntegralKey {
        &self.integral
    }
}

/// Exact generic-parameter coefficients in the stored unit-mass, common formal
/// loop-measure convention; every retained condition must be nonzero. External
/// mass rescaling and family-specific measure factors belong to the caller.
#[derive(Clone, Debug)]
pub struct SavedApplicationReduction {
    terms: BTreeMap<SavedApplicationIntegral, Coefficient>,
    conditions: Vec<Coefficient>,
    statistics: CandidateRoutedApplicationStatistics,
    terminal_substitution_additions: usize,
}
impl SavedApplicationReduction {
    pub fn terms(&self) -> &BTreeMap<SavedApplicationIntegral, Coefficient> {
        &self.terms
    }
    pub fn nonzero_conditions(&self) -> &[Coefficient] {
        &self.conditions
    }
    pub fn statistics(&self) -> &CandidateRoutedApplicationStatistics {
        &self.statistics
    }
    /// Additional coalescing work after the routed owner's reported statistics.
    /// It shares that request's configured coalescing allowance.
    pub fn terminal_substitution_additions(&self) -> usize {
        self.terminal_substitution_additions
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct TerminalRow {
    terms: BTreeMap<SavedApplicationIntegral, Coefficient>,
    conditions: Arc<Vec<Coefficient>>,
}

struct TerminalMaps {
    families: BTreeMap<String, Arc<IntegralFamily>>,
    rows: BTreeMap<SavedApplicationIntegral, TerminalRow>,
    remaining: BTreeSet<SavedApplicationIntegral>,
}

impl TerminalMaps {
    fn from_artifact(artifact: &MasterReductionArtifact) -> Result<Self, AppError> {
        let collection = artifact.collection();
        let families: BTreeMap<_, _> = collection
            .families()
            .map(|family| (family.fingerprint().to_owned(), Arc::clone(family)))
            .collect();
        let key = |key: &rustred::reduction::terminal_normalization::VacuumIntegralKey| {
            SavedApplicationIntegral {
                family_fingerprint: key.family_fingerprint().to_owned(),
                integral: key.integral().clone(),
            }
        };
        let mut rows = BTreeMap::new();
        for source in collection.raw_terminals() {
            let family = &families[source.family_fingerprint()];
            let row = artifact.apply_terminal(family, source.integral())?;
            rows.insert(
                key(source),
                TerminalRow {
                    terms: row
                        .terms()
                        .iter()
                        .map(|(k, c)| (key(k), c.clone()))
                        .collect(),
                    conditions: Arc::new(row.nonzero_conditions().to_vec()),
                },
            );
        }
        Ok(Self {
            families,
            rows,
            remaining: collection.remaining_terminals().iter().map(key).collect(),
        })
    }

    fn validate(&self, options: SavedApplicationOptions) -> Result<(), AppError> {
        if self.families.is_empty() {
            return Err(AppError::input("terminal application has no family"));
        }
        let first = self.families.values().next().unwrap();
        for family in self.families.values() {
            if !first
                .coefficient_context()
                .has_same_variable_map(family.coefficient_context())
            {
                return Err(AppError::input(
                    "terminal application coefficient maps differ",
                ));
            }
        }
        let check_key = |key: &SavedApplicationIntegral| {
            let family = self.families.get(&key.family_fingerprint).ok_or_else(|| {
                AppError::input("terminal application references an unknown family")
            })?;
            if key.integral.powers().len() != family.denominator_count() {
                return Err(AppError::input("terminal application key has wrong arity"));
            }
            Ok(())
        };
        for key in &self.remaining {
            check_key(key)?;
        }
        let mut validated_conditions = HashSet::new();
        for (key, row) in &self.rows {
            check_key(key)?;
            for (output, coefficient) in &row.terms {
                check_key(output)?;
                if !self.remaining.contains(output) || coefficient.is_zero() {
                    return Err(AppError::input(
                        "terminal substitution has a nonterminal or zero entry",
                    ));
                }
                first
                    .coefficient_context()
                    .validate_with_limits(coefficient, options.reduction_limits.exact_algebra)
                    .map_err(failure)?;
            }
            // Decoder-interned guard vectors are immutable and context-bound
            // identically for every row. Validate the shared allocation once.
            if validated_conditions.insert(Arc::as_ptr(&row.conditions)) {
                for condition in row.conditions.iter() {
                    first
                        .coefficient_context()
                        .validate_with_limits(condition, options.reduction_limits.exact_algebra)
                        .map_err(failure)?;
                    if condition.is_zero() {
                        return Err(AppError::input(
                            "terminal application has an impossible nonzero condition",
                        ));
                    }
                }
            }
        }
        Ok(())
    }
}

/// Immutable prepared configuration with one explicitly owned mutable cache.
/// Cache reuse never bypasses entry admission, including for zero coefficients.
pub struct PreparedSavedApplication<const N: usize> {
    application: RoutedCandidateApplication<N>,
    entry_domain: RequestedEntryDomain<N>,
    terminals: TerminalMaps,
    terminal_products: Option<products::TerminalProducts>,
    terminal_product_charts: Option<charts::TerminalProductCharts>,
    preparation_statistics: SavedApplicationPreparationStatistics,
    options: SavedApplicationOptions,
    metadata: Value,
    inputs: package::ApplicationInputs,
}

impl<const N: usize> PreparedSavedApplication<N> {
    pub fn family(&self) -> &IntegralFamily {
        self.application.family()
    }
    pub fn output_family(&self, fingerprint: &str) -> Option<&IntegralFamily> {
        self.terminals
            .families
            .get(fingerprint)
            .map(Arc::as_ref)
            .or_else(|| {
                self.terminal_products
                    .as_ref()?
                    .families
                    .get(fingerprint)
                    .map(Arc::as_ref)
            })
            .or_else(|| {
                self.terminal_product_charts
                    .as_ref()?
                    .families
                    .get(fingerprint)
                    .map(Arc::as_ref)
            })
    }
    /// Share an authenticated family with a lower-factor provider without
    /// rebuilding its exact matrices or duplicating its Symbolica context.
    pub fn output_family_shared(&self, fingerprint: &str) -> Option<Arc<IntegralFamily>> {
        self.terminals
            .families
            .get(fingerprint)
            .cloned()
            .or_else(|| {
                self.terminal_products
                    .as_ref()?
                    .families
                    .get(fingerprint)
                    .cloned()
            })
            .or_else(|| {
                self.terminal_product_charts
                    .as_ref()?
                    .families
                    .get(fingerprint)
                    .cloned()
            })
    }
    /// Frozen provenance and authority, with no claim of master independence.
    pub fn metadata(&self) -> &Value {
        &self.metadata
    }
    pub fn preparation_statistics(&self) -> &SavedApplicationPreparationStatistics {
        &self.preparation_statistics
    }
    /// Original exact union, suitable for enumerating admitted test witnesses.
    pub fn entry_regions(&self) -> &[rustred::solver::RootRegionInput<N>] {
        self.entry_domain.regions()
    }
    pub fn queries_json(&self) -> &str {
        &self.inputs.queries
    }
    /// Authenticated runtime selection; routes and physical slots retain their
    /// original exact definitions, independent of topology display labels.
    pub fn selection_json(&self) -> &str {
        &self.inputs.selection
    }
    pub fn terminal_sources(&self) -> impl ExactSizeIterator<Item = &SavedApplicationIntegral> {
        self.terminals.rows.keys()
    }
    pub fn remaining_terminals(&self) -> &BTreeSet<SavedApplicationIntegral> {
        &self.terminals.remaining
    }
    pub fn validate_entry(&self, key: &IntegralKey) -> Result<(), AppError> {
        self.entry_domain.validate(key)
    }
    pub fn cache_statistics(&self) -> CandidateStatistics {
        self.application.cache_statistics()
    }
    /// Cumulative eviction/recomputation work, including failed requests.
    /// Clearing retained results does not reset these counters.
    pub fn cache_work_statistics(&self) -> CandidateRoutedCacheStatistics {
        self.application.cache_work_statistics()
    }
    pub fn owner_preparation_statistics(
        &self,
    ) -> rustred::solver::DeferredCandidateOwnerStatistics {
        self.application.owner_preparation_statistics()
    }
    pub fn clear_cache(&mut self) -> Result<(), AppError> {
        self.application.clear_cache().map_err(failure)
    }

    /// Apply the saved, verified unit-Jacobian entry map without any IBP
    /// application. Literal owners use the identity map. Every nonzero mapped
    /// endpoint must lie in the original published union; a route cannot turn
    /// the published R/D maxima into universal entry bounds.
    pub fn route_entry_to_owner(
        &self,
        target: &IntegralKey,
    ) -> Result<CandidateRoutedReduction, AppError> {
        let routed = self
            .application
            .route_entry_to_owner(target)
            .map_err(failure)?;
        for (endpoint, coefficient) in routed.terms() {
            if !coefficient.is_zero() {
                self.entry_domain.validate(endpoint)?;
            }
        }
        Ok(routed)
    }

    /// Probe the exact saved union after native, verified owner transport.
    /// No formulas are prepared or applied. An out-of-union endpoint returns
    /// false, as does an unsupported entry support without a saved owner route.
    /// That is a rejected matching candidate, not a broken application. Malformed
    /// keys, invalid transports and resource failures stay errors. Actual route
    /// and reduction methods still reject missing owners with an error.
    pub fn routed_entry_is_admitted(&self, target: &IntegralKey) -> Result<bool, AppError> {
        let routed = match self.application.route_entry_to_owner(target) {
            Ok(routed) => routed,
            Err(rustred::solver::CandidateRoutedError::MissingOwner { .. }) => return Ok(false),
            Err(error) => return Err(failure(error)),
        };
        for (endpoint, coefficient) in routed.terms() {
            if !coefficient.is_zero() && !self.entry_domain.is_admitted(endpoint)? {
                return Ok(false);
            }
        }
        Ok(true)
    }

    /// Transport equivalent entries to saved owners and reduce the exact
    /// weighted combination. Each endpoint crosses scope admission before
    /// cancellation or cache lookup. Entry transports and downstream reduction
    /// share one operational request and its arithmetic/transport allowances.
    pub fn reduce_routed_linear_combination(
        &mut self,
        terms: impl IntoIterator<Item = (IntegralKey, Coefficient)>,
    ) -> Result<SavedApplicationReduction, AppError> {
        let reduced = self
            .application
            .reduce_routed_linear_combination_with_entry_admission(
                terms,
                entry::admission(Some(&self.entry_domain)),
            )
            .map_err(failure)?;
        self.apply_terminal_substitutions(reduced)
    }

    pub fn reduce_linear_combination(
        &mut self,
        terms: impl IntoIterator<Item = (IntegralKey, Coefficient)>,
    ) -> Result<SavedApplicationReduction, AppError> {
        self.reduce_linear_combination_progress(terms, None)
    }

    fn reduce_linear_combination_progress(
        &mut self,
        terms: impl IntoIterator<Item = (IntegralKey, Coefficient)>,
        mut progress: Option<&mut progress::SavedProgress<'_>>,
    ) -> Result<SavedApplicationReduction, AppError> {
        let mut inputs = Vec::new();
        for (key, coefficient) in terms {
            if inputs.len() >= self.options.trace_limits.max_input_targets {
                return Err(AppError::limit(
                    "saved application input term limit exceeded",
                ));
            }
            // Admission is deliberately before cancellation and core-cache lookup.
            self.entry_domain.validate(&key)?;
            inputs.push((key, coefficient));
        }
        let reduced = if let Some(progress) = progress.as_deref_mut() {
            self.application
                .reduce_linear_combination_with_entry_admission_and_observer(
                    inputs,
                    entry::admission(Some(&self.entry_domain)),
                    progress.interval,
                    |snapshot| progress.routed(snapshot),
                )
        } else {
            self.application
                .reduce_linear_combination_with_entry_admission(
                    inputs,
                    entry::admission(Some(&self.entry_domain)),
                )
        }
        .map_err(failure)?;
        self.apply_terminal_substitutions_progress(reduced, progress)
    }

    fn apply_terminal_substitutions(
        &self,
        reduced: CandidateRoutedReduction,
    ) -> Result<SavedApplicationReduction, AppError> {
        self.apply_terminal_substitutions_progress(reduced, None)
    }

    fn apply_terminal_substitutions_progress(
        &self,
        reduced: CandidateRoutedReduction,
        mut progress: Option<&mut progress::SavedProgress<'_>>,
    ) -> Result<SavedApplicationReduction, AppError> {
        if let Some(progress) = progress.as_deref_mut() {
            progress.terminal_start(reduced.terms().len());
        }
        let mut output = BTreeMap::new();
        // Keep deterministic encounter order in the output, but use native
        // Symbolica Hash/Eq for membership. Borrowing avoids a second owned
        // copy of every potentially large polynomial in the membership set.
        let mut conditions = Vec::new();
        let mut seen_conditions = HashSet::new();
        let mut seen_guard_vectors = HashSet::new();
        append_conditions(
            &mut conditions,
            &mut seen_conditions,
            reduced.nonzero_conditions(),
        );
        let family = self.family();
        let context = family.coefficient_context();
        let routed_additions = reduced.statistics().work.coalescing_additions();
        let mut additions = routed_additions;
        for (integral, factor) in reduced.terms() {
            let key = SavedApplicationIntegral {
                family_fingerprint: family.fingerprint().to_owned(),
                integral: integral.clone(),
            };
            let row = self.terminals.rows.get(&key).ok_or_else(|| {
                AppError::execution(
                    "saved owner returned a terminal absent from the published substitutions",
                )
            })?;
            if seen_guard_vectors.insert(Arc::as_ptr(&row.conditions)) {
                append_conditions(&mut conditions, &mut seen_conditions, &row.conditions);
            }
            for (key, coefficient) in &row.terms {
                let value = context
                    .try_mul(
                        factor,
                        coefficient,
                        self.options.reduction_limits.exact_algebra,
                    )
                    .map_err(failure)?;
                if value.is_zero() {
                    continue;
                }
                let sum = match output.remove(key) {
                    Some(previous) => {
                        additions = additions
                            .checked_add(1)
                            .filter(|&n| {
                                n <= self.options.reduction_limits.max_coalescing_additions
                            })
                            .ok_or_else(|| {
                                AppError::limit("terminal substitution addition limit exceeded")
                            })?;
                        context
                            .try_add(
                                &previous,
                                &value,
                                self.options.reduction_limits.exact_algebra,
                            )
                            .map_err(failure)?
                    }
                    None => value,
                };
                if !sum.is_zero() {
                    output.insert(key.clone(), sum);
                }
            }
            if let Some(progress) = progress.as_deref_mut() {
                progress.terminal_row(additions - routed_additions);
            }
        }
        Ok(SavedApplicationReduction {
            terms: output,
            conditions,
            statistics: reduced.statistics().clone(),
            terminal_substitution_additions: additions - routed_additions,
        })
    }

    /// Write a self-contained directory, refusing to replace an existing path.
    /// No campaign ledger, refinement matrix or numerical master is exported.
    pub fn export_runtime(&self, destination: &Path) -> Result<Value, AppError> {
        package::export(self, destination)
    }

    fn from_parts(
        inputs: package::ApplicationInputs,
        terminals: TerminalMaps,
        metadata: Value,
        options: SavedApplicationOptions,
    ) -> Result<Self, AppError> {
        Self::from_parts_with_mode(inputs, terminals, metadata, options, false)
    }

    fn from_parts_deferred(
        inputs: package::ApplicationInputs,
        terminals: TerminalMaps,
        metadata: Value,
        options: SavedApplicationOptions,
    ) -> Result<Self, AppError> {
        Self::from_parts_with_mode(inputs, terminals, metadata, options, true)
    }

    fn from_parts_with_mode(
        inputs: package::ApplicationInputs,
        terminals: TerminalMaps,
        mut metadata: Value,
        options: SavedApplicationOptions,
        deferred: bool,
    ) -> Result<Self, AppError> {
        let started = Instant::now();
        options.validate_preparation()?;
        let (selection, arity, limits) = Selection::parse(&inputs.selection)?;
        if arity > N {
            return Err(AppError::input(format!(
                "saved application arity {arity} exceeds storage arity {N}"
            )));
        }
        let (entry_domain, empty) = scope::execution_domain::<N>(&inputs.queries, arity)?;
        metadata["empty_starting_domains"] = serde_json::json!(empty);
        metadata["nonempty_starting_domains"] = serde_json::json!(entry_domain.regions().len());
        terminals.validate(options)?;
        let mut request = RoutedCampaignRequest::new(inputs.selection.clone(), String::new());
        request.owner_base = inputs.owner_base.clone();
        request.reduction_limits = options.reduction_limits;
        request.trace_limits = options.trace_limits;
        request.workers = options.preparation_workers;
        let expected = inputs
            .payloads
            .iter()
            .map(|p| p.blake3.clone())
            .collect::<Vec<_>>();
        let mut authenticate = |actual: Vec<String>| {
            if actual == expected {
                Ok(())
            } else {
                Err(
                    "saved application owner payloads differ from the authenticated selection"
                        .to_owned(),
                )
            }
        };
        let mut reader = |path: &Path, bytes| inputs.read_payload(path, bytes);
        let owner_started = Instant::now();
        let timing = std::cell::RefCell::new((
            None::<Instant>,
            None::<Instant>,
            SavedApplicationPreparationStatistics::default(),
        ));
        let observer = |event: Value| {
            let now = Instant::now();
            let mut timing = timing.borrow_mut();
            if event["event"] == "preparation" && event["phase"] == "native_owner_load" {
                timing.2.owner_payloads = now.duration_since(owner_started);
                timing.0 = Some(now);
            } else if event["event"] == "loaded" {
                if let Some(native_started) = timing.0 {
                    timing.2.owner_preparation = now.duration_since(native_started);
                }
                timing.1 = Some(now);
            } else if event["event"] == "routes_verified" {
                if let Some(route_started) = timing.1 {
                    timing.2.route_preparation = now.duration_since(route_started);
                }
            }
        };
        let mut application = if deferred {
            let family = terminals
                .families
                .get(&selection.family_fingerprint)
                .cloned()
                .ok_or_else(|| {
                    AppError::input("deferred primary family is absent from terminal maps")
                })?;
            prepare::prepare_deferred_with_reader::<N>(
                family,
                &request,
                &selection,
                limits,
                &AtomicBool::new(false),
                &observer,
                Some(&mut authenticate),
                &mut reader,
            )?
        } else {
            prepare::prepare_with_reader::<N>(
                &request,
                &selection,
                limits,
                &AtomicBool::new(false),
                &observer,
                Some(&mut authenticate),
                &mut reader,
            )?
            .map(|reducer| reducer.application())
        }
        .ok_or_else(|| AppError::execution("saved application preparation cancelled"))?;
        let family = application.family();
        if !terminals.families.contains_key(family.fingerprint()) {
            return Err(AppError::input(
                "saved owners and terminal maps have different families",
            ));
        }
        if !family
            .coefficient_context()
            .has_same_variable_map(terminals.families[family.fingerprint()].coefficient_context())
        {
            return Err(AppError::input(
                "saved owner and terminal coefficient contexts differ",
            ));
        }
        if metadata["family_fingerprint"].as_str() != Some(family.fingerprint()) {
            return Err(AppError::input(
                "saved application manifest family differs from its owners",
            ));
        }
        application
            .set_cache_representation(options.cache_representation)
            .map_err(failure)?;
        application
            .set_cache_policy(options.cache_policy)
            .map_err(failure)?;
        metadata["owner_preparation"] =
            serde_json::json!(if deferred { "on_first_use" } else { "eager" });
        metadata["preparation_workers"] = serde_json::json!(options.preparation_workers);
        let mut preparation_statistics = timing.into_inner().2;
        preparation_statistics.finish(started.elapsed());
        Ok(Self {
            application,
            entry_domain,
            terminals,
            terminal_products: None,
            terminal_product_charts: None,
            preparation_statistics,
            options,
            metadata,
            inputs,
        })
    }
}

fn failure(error: impl std::fmt::Display) -> AppError {
    AppError::execution(error.to_string())
}

fn append_conditions<'a>(
    output: &mut Vec<Coefficient>,
    seen: &mut HashSet<&'a Coefficient>,
    conditions: &'a [Coefficient],
) {
    for condition in conditions {
        if seen.insert(condition) {
            output.push(condition.clone());
        }
    }
}
