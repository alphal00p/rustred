//! Generic LiteRed-style composition of zero sectors, symmetries, and IBPs.
//!
//! This provider contains no topology- or loop-specific recurrence. It asks
//! the adaptive elimination layer for generated parametric candidates, then
//! performs the same semantic quotient used by LiteRed before accepting a
//! descending rule: cut-zero and analytically zero terms are removed, exact
//! internal symmetries canonicalize the remaining terms, and the collected
//! equation is solved only when its proof replays.

use std::collections::{BTreeMap, VecDeque};
use std::fmt::{self, Write};
use std::sync::Arc;

use crate::certified_rewrite::{
    ReplayedGeneratedCylindricalPersistentSource, preflight_persistent_numeric_specialization_terms,
};
use crate::reduction_engine::{ConcreteRuleDecision, ConcreteRuleProvider, ConcreteTerminalStatus};
use crate::{
    AdaptiveParametricRuleProvider, AdaptiveRuleSearchError, CertifiedConcreteRewrite,
    CertifiedRewriteError, CertifiedRewriteLimits, CertifiedZeroReduction, ConcreteIntegralKey,
    ConcreteRelation, GeneratedCylindricalPersistentEliminationCertificate,
    GeneratedCylindricalPersistentEliminationError, IntegralFamily, IntegralOrderingPolicy,
    ParametricCoefficientContext, ParametricIbpConfig, ParametricIbpError, ParametricIbpGenerator,
    ParametricRelationError, ParametricRuleError, PowerShiftPolicy, QuotientTermWitness,
    SectorExclusion, SectorFoundationError, SectorMask, SectorRestrictions,
    VerifiedInternalFamilyPermutationSymmetry, ZeroSectorAnalyzer, ZeroSectorDecision,
    ZeroSectorError,
};

pub const CERTIFIED_FAMILY_RULE_PROVIDER_V1_SCHEMA: &str =
    "rustred-certified-family-rule-provider-v1";
/// V2 adds the opt-in authenticated persistent-cylindrical numeric quotient
/// source. V1 and V2 remain exported as legacy identities.
pub const CERTIFIED_FAMILY_RULE_PROVIDER_V2_SCHEMA: &str =
    "rustred-certified-family-rule-provider-v2";
/// V3 replaces the optional single persistent cylindrical source with a
/// deterministic, immutable, resource-bounded collection. V1 and V2 remain
/// exported as legacy identities; new providers are V3.
pub const CERTIFIED_FAMILY_RULE_PROVIDER_V3_SCHEMA: &str =
    "rustred-certified-family-rule-provider-v3";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CertifiedFamilyRuleProviderLimits {
    pub rewrite: CertifiedRewriteLimits,
    pub max_input_symmetries: usize,
    pub max_persistent_cylindrical_sources: usize,
    /// Aggregate logical scope entries inspected by the persistent-source
    /// index: every sector bit plus every partial-assignment entry.
    pub max_persistent_cylindrical_source_scope_entries: usize,
    /// Conservative byte surface of the persistent-source index: one retained
    /// `Arc` handle per source plus every sector bit and assignment entry.
    pub max_persistent_cylindrical_source_index_bytes: usize,
    pub max_symmetry_orbit_states: usize,
    pub max_symmetry_orbit_path_entries: usize,
    pub max_cached_decisions: usize,
    pub max_retained_proof_debug_bytes: usize,
}

impl Default for CertifiedFamilyRuleProviderLimits {
    fn default() -> Self {
        Self {
            rewrite: CertifiedRewriteLimits::default(),
            max_input_symmetries: 1_000_000,
            max_persistent_cylindrical_sources: 1_000_000,
            max_persistent_cylindrical_source_scope_entries: portable_limit(16_000_000_000),
            max_persistent_cylindrical_source_index_bytes: portable_limit(
                256u128 * 1024 * 1024 * 1024,
            ),
            max_symmetry_orbit_states: 1_000_000,
            max_symmetry_orbit_path_entries: 10_000_000,
            max_cached_decisions: 10_000_000,
            max_retained_proof_debug_bytes: 2 * 1024 * 1024 * 1024,
        }
    }
}

pub struct CertifiedFamilyRuleProvider<'relations> {
    family: IntegralFamily,
    restrictions: SectorRestrictions,
    zero: ZeroSectorAnalyzer,
    symmetries: Box<[Arc<VerifiedInternalFamilyPermutationSymmetry>]>,
    adaptive: AdaptiveParametricRuleProvider<'relations>,
    persistent_cylindrical_sources:
        Box<[Arc<GeneratedCylindricalPersistentEliminationCertificate>]>,
    ordering: IntegralOrderingPolicy,
    limits: CertifiedFamilyRuleProviderLimits,
    cache: BTreeMap<ConcreteIntegralKey, ConcreteRuleDecision>,
    retained_proof_debug_bytes: usize,
}

impl<'relations> CertifiedFamilyRuleProvider<'relations> {
    pub const SCHEMA: &'static str = CERTIFIED_FAMILY_RULE_PROVIDER_V3_SCHEMA;

    #[allow(clippy::too_many_arguments)]
    pub fn try_new(
        family: IntegralFamily,
        restrictions: SectorRestrictions,
        symmetries: impl IntoIterator<Item = VerifiedInternalFamilyPermutationSymmetry>,
        adaptive: AdaptiveParametricRuleProvider<'relations>,
        ordering: IntegralOrderingPolicy,
        limits: CertifiedFamilyRuleProviderLimits,
    ) -> Result<Self, CertifiedFamilyRuleProviderError> {
        if restrictions.arity() != family.denominator_count() {
            return Err(CertifiedFamilyRuleProviderError::WrongArity {
                expected: family.denominator_count(),
                actual: restrictions.arity(),
            });
        }
        if adaptive.ordering() != ordering
            || adaptive.context().index_count() != family.denominator_count()
            || !adaptive
                .context()
                .base()
                .has_same_variable_map(family.coefficient_context())
            || adaptive.source_rows()[0].family_fingerprint() != family.fingerprint()
        {
            return Err(CertifiedFamilyRuleProviderError::ForeignAdaptiveProvider);
        }
        // Concrete elimination stores compact generated-row ordinals and
        // regenerates the canonical IBPLI list during replay. Authenticate
        // the adaptive input against that exact list here as well, including
        // every exceptional-domain origin. Same family/context alone is not
        // evidence that a caller-supplied recurrence was generated from the
        // family definition.
        let regenerated = ParametricIbpGenerator::try_with_context(
            &family,
            adaptive.context().clone(),
            ParametricIbpConfig::default(),
        )?
        .generate()?;
        let expected_rows = regenerated.ibp_li().collect::<Vec<_>>();
        if adaptive.source_rows().len() != expected_rows.len() {
            return Err(CertifiedFamilyRuleProviderError::UnauthenticatedSourceRows { row: None });
        }
        for (row, (actual, expected)) in
            adaptive.source_rows().iter().zip(expected_rows).enumerate()
        {
            if !actual.has_identical_guard_provenance(expected) {
                return Err(
                    CertifiedFamilyRuleProviderError::UnauthenticatedSourceRows { row: Some(row) },
                );
            }
        }
        let zero = ZeroSectorAnalyzer::try_new_with_limits(
            &family,
            restrictions.clone(),
            PowerShiftPolicy::FormalGeneric,
            limits.rewrite.zero_sector,
        )?;
        let mut retained = Vec::new();
        let mut retained_proof_debug_bytes = 0usize;
        for symmetry in symmetries {
            let requested = checked_add(retained.len(), 1, "retained input symmetries")?;
            check_limit("input symmetries", requested, limits.max_input_symmetries)?;
            symmetry.replay(&family, &restrictions, limits.rewrite.symmetry)?;
            retained_proof_debug_bytes = charge_debug_bytes(
                retained_proof_debug_bytes,
                &symmetry,
                limits.max_retained_proof_debug_bytes,
            )?;
            retained.push(Arc::new(symmetry));
        }
        Ok(Self {
            family,
            restrictions,
            zero,
            symmetries: retained.into_boxed_slice(),
            adaptive,
            persistent_cylindrical_sources: Box::new([]),
            ordering,
            limits,
            cache: BTreeMap::new(),
            retained_proof_debug_bytes,
        })
    }

    /// Construct a provider which, after canonicalizing the requested source,
    /// may numerically requotient and re-eliminate the translated bound rows
    /// retained by one authenticated cylindrical persistent certificate.
    ///
    /// The ordinary adaptive generated-IBP paths remain unchanged fallbacks.
    /// The additional source applies only to its own exact sector and partial
    /// index locus; no topology or loop-count dispatch enters the decision.
    #[allow(clippy::too_many_arguments)]
    pub fn try_new_with_persistent_cylindrical_source(
        family: IntegralFamily,
        restrictions: SectorRestrictions,
        symmetries: impl IntoIterator<Item = VerifiedInternalFamilyPermutationSymmetry>,
        adaptive: AdaptiveParametricRuleProvider<'relations>,
        persistent_cylindrical_source: Arc<GeneratedCylindricalPersistentEliminationCertificate>,
        ordering: IntegralOrderingPolicy,
        limits: CertifiedFamilyRuleProviderLimits,
    ) -> Result<Self, CertifiedFamilyRuleProviderError> {
        Self::try_new_with_persistent_cylindrical_sources(
            family,
            restrictions,
            symmetries,
            adaptive,
            [persistent_cylindrical_source],
            ordering,
            limits,
        )
    }

    /// Construct a provider with a deterministic collection of authenticated
    /// persistent cylindrical sources.
    ///
    /// Sources are retained by sector, then by decreasing partial-assignment
    /// specificity, then by their canonical assignment entries. Thus a
    /// locus-specific source is attempted before a broader source in the same
    /// sector, while overlapping incomparable loci have a stable order. Two
    /// sources with the same exact sector/assignment scope are rejected: an
    /// exact scope has one unambiguous proof authority.
    #[allow(clippy::too_many_arguments)]
    pub fn try_new_with_persistent_cylindrical_sources(
        family: IntegralFamily,
        restrictions: SectorRestrictions,
        symmetries: impl IntoIterator<Item = VerifiedInternalFamilyPermutationSymmetry>,
        adaptive: AdaptiveParametricRuleProvider<'relations>,
        persistent_cylindrical_sources: impl IntoIterator<
            Item = Arc<GeneratedCylindricalPersistentEliminationCertificate>,
        >,
        ordering: IntegralOrderingPolicy,
        limits: CertifiedFamilyRuleProviderLimits,
    ) -> Result<Self, CertifiedFamilyRuleProviderError> {
        // Count and retain shallow source capabilities before inspecting
        // their scope. This makes the source-count boundary independent of
        // source algebra and of which later preflight would reject a source.
        let mut retained = Vec::new();
        for persistent_source in persistent_cylindrical_sources {
            let requested = checked_add(retained.len(), 1, "persistent cylindrical sources")?;
            check_limit(
                "persistent cylindrical sources",
                requested,
                limits.max_persistent_cylindrical_sources,
            )?;
            if retained.len() == retained.capacity() {
                retained.try_reserve_exact(1).map_err(|_| {
                    CertifiedFamilyRuleProviderError::AllocationFailure {
                        resource: "persistent cylindrical sources",
                        requested,
                    }
                })?;
            }
            retained.push(persistent_source);
        }

        // Reject foreign capability scope before sorting it or constructing
        // the base provider. These are borrowed identity/arity comparisons;
        // exact certificate replay remains below, after every aggregate index
        // preflight and duplicate check succeeds.
        let expected_family_fingerprint = family.fingerprint();
        let expected_context_fingerprint = adaptive.context().fingerprint();
        for persistent_source in &retained {
            let row_system = persistent_source.row_system();
            let start = row_system.start();
            if persistent_source.family_fingerprint() != expected_family_fingerprint
                || row_system.family_fingerprint() != expected_family_fingerprint
                || start.family_fingerprint() != expected_family_fingerprint
                || persistent_source.context_fingerprint() != expected_context_fingerprint
                || row_system.context_fingerprint() != expected_context_fingerprint
                || start.context_fingerprint() != expected_context_fingerprint
                || start.sector().arity() != family.denominator_count()
                || start.assignment().arity() != family.denominator_count()
                || start.schedule().ordering().policy() != ordering
                || start.ordering_policy() != ordering
                || persistent_source.ordering_identity()
                    != start.schedule().ordering().stable_manifest()
            {
                return Err(CertifiedFamilyRuleProviderError::ForeignPersistentCylindricalSource);
            }
        }

        // Bound the complete comparison-key surface before the first sort
        // comparison. Source count bounds the number of handles; these two
        // aggregate budgets additionally bound variable-length sector and
        // assignment payloads and their conservative index byte surface.
        let mut scope_entries = 0usize;
        let mut index_bytes = 0usize;
        for persistent_source in &retained {
            let start = persistent_source.row_system().start();
            let source_scope_entries = checked_add(
                start.sector().arity(),
                start.assignment().entries().len(),
                "persistent cylindrical source scope entries",
            )?;
            scope_entries = checked_add(
                scope_entries,
                source_scope_entries,
                "persistent cylindrical source scope entries",
            )?;
            check_limit(
                "persistent cylindrical source scope entries",
                scope_entries,
                limits.max_persistent_cylindrical_source_scope_entries,
            )?;

            let sector_bytes = checked_mul(
                start.sector().arity(),
                std::mem::size_of::<bool>(),
                "persistent cylindrical source index bytes",
            )?;
            let assignment_bytes = checked_mul(
                start.assignment().entries().len(),
                std::mem::size_of::<(usize, i64)>(),
                "persistent cylindrical source index bytes",
            )?;
            let source_index_bytes = checked_add(
                std::mem::size_of::<Arc<GeneratedCylindricalPersistentEliminationCertificate>>(),
                checked_add(
                    sector_bytes,
                    assignment_bytes,
                    "persistent cylindrical source index bytes",
                )?,
                "persistent cylindrical source index bytes",
            )?;
            index_bytes = checked_add(
                index_bytes,
                source_index_bytes,
                "persistent cylindrical source index bytes",
            )?;
            check_limit(
                "persistent cylindrical source index bytes",
                index_bytes,
                limits.max_persistent_cylindrical_source_index_bytes,
            )?;
        }

        // The scope key is total once duplicate exact scopes are rejected, so
        // the in-place unstable sort is deterministic and avoids a second
        // source-count-sized allocation.
        retained.sort_unstable_by(|left, right| {
            let left_start = left.row_system().start();
            let right_start = right.row_system().start();
            left_start
                .sector()
                .cmp(right_start.sector())
                .then_with(|| {
                    right_start
                        .assignment()
                        .entries()
                        .len()
                        .cmp(&left_start.assignment().entries().len())
                })
                .then_with(|| {
                    left_start
                        .assignment()
                        .entries()
                        .cmp(right_start.assignment().entries())
                })
        });
        for adjacent in retained.windows(2) {
            let left = adjacent[0].row_system().start();
            let right = adjacent[1].row_system().start();
            if left.sector() == right.sector() && left.assignment() == right.assignment() {
                return Err(
                    CertifiedFamilyRuleProviderError::DuplicatePersistentCylindricalSourceScope {
                        sector: left.sector().clone(),
                        assignment: left.assignment().clone(),
                    },
                );
            }
        }

        let mut provider =
            Self::try_new(family, restrictions, symmetries, adaptive, ordering, limits)?;
        for persistent_source in &retained {
            // Keep each capability operation-scoped: construction
            // authenticates it exactly once, then drops the replay token
            // before publishing the immutable collection.
            ReplayedGeneratedCylindricalPersistentSource::authenticate(
                &provider.family,
                provider.parametric_context(),
                persistent_source,
            )?;
            provider.retained_proof_debug_bytes = charge_debug_bytes(
                provider.retained_proof_debug_bytes,
                persistent_source,
                limits.max_retained_proof_debug_bytes,
            )?;
        }
        provider.persistent_cylindrical_sources = retained.into_boxed_slice();
        Ok(provider)
    }

    pub const fn family(&self) -> &IntegralFamily {
        &self.family
    }

    pub const fn adaptive(&self) -> &AdaptiveParametricRuleProvider<'relations> {
        &self.adaptive
    }

    /// Return the retained source only when this is exactly a singular-source
    /// provider. Plural providers return `None`; callers inspecting a source
    /// database must use [`Self::persistent_cylindrical_sources`].
    pub const fn persistent_cylindrical_source(
        &self,
    ) -> Option<&Arc<GeneratedCylindricalPersistentEliminationCertificate>> {
        if self.persistent_cylindrical_sources.len() == 1 {
            self.persistent_cylindrical_sources.first()
        } else {
            None
        }
    }

    /// All retained sources in deterministic demand order.
    pub const fn persistent_cylindrical_sources(
        &self,
    ) -> &[Arc<GeneratedCylindricalPersistentEliminationCertificate>] {
        &self.persistent_cylindrical_sources
    }

    pub const fn parametric_context(&self) -> &ParametricCoefficientContext {
        self.adaptive.context()
    }

    pub fn symmetries(&self) -> &[Arc<VerifiedInternalFamilyPermutationSymmetry>] {
        &self.symmetries
    }

    pub const fn limits(&self) -> CertifiedFamilyRuleProviderLimits {
        self.limits
    }

    pub fn cache_len(&self) -> usize {
        self.cache.len()
    }

    pub const fn retained_proof_debug_bytes(&self) -> usize {
        self.retained_proof_debug_bytes
    }

    fn discover(
        &mut self,
        source: &ConcreteIntegralKey,
    ) -> Result<ConcreteRuleDecision, CertifiedFamilyRuleProviderError> {
        if source.powers().len() != self.family.denominator_count() {
            return Err(CertifiedFamilyRuleProviderError::WrongArity {
                expected: self.family.denominator_count(),
                actual: source.powers().len(),
            });
        }
        let source_sector = SectorMask::try_from_indices(source.powers())?;
        match self.zero.analyze_sector(&source_sector) {
            ZeroSectorDecision::ProvedZero(certificate) => {
                return Ok(ConcreteRuleDecision::ProvedZero(
                    CertifiedZeroReduction::try_new(
                        &self.family,
                        source.clone(),
                        Arc::new(certificate),
                        self.limits.rewrite,
                    )?,
                ));
            }
            ZeroSectorDecision::ResourceLimited(resource) => {
                return Err(CertifiedFamilyRuleProviderError::ZeroResource {
                    resource: resource.resource(),
                    requested: resource.requested(),
                    limit: resource.limit(),
                });
            }
            ZeroSectorDecision::Failed(error) => return Err(error.into()),
            ZeroSectorDecision::Excluded(exclusion) => {
                return self.excluded_source_decision(source, exclusion);
            }
            ZeroSectorDecision::NoZeroCertificate(_) => {}
        }

        let (canonical_source, source_path) = self.canonicalize(source)?;
        if canonical_source != *source {
            return Ok(ConcreteRuleDecision::CertifiedRewrite(
                CertifiedConcreteRewrite::from_symmetry(
                    &self.family,
                    source.clone(),
                    canonical_source,
                    source_path,
                    self.restrictions.clone(),
                    self.ordering,
                    self.limits.rewrite,
                )?,
            ));
        }

        // At the canonical source, join the exact bound cylindrical rows with
        // the same zero/symmetry quotient used by the ordinary scout path,
        // then recompute rank and pivots over the concrete base field.  This
        // closes symmetry-identical terms inside an equation before descent is
        // tested, instead of emitting a recursive rewrite cycle.
        // The collection is sector-major, so a demand inspects only its
        // matching partition rather than linearly scanning an entire solved
        // family database. Within that partition the retained order already
        // places more-specific equality loci first.
        let first_persistent_source = self
            .persistent_cylindrical_sources
            .partition_point(|candidate| candidate.row_system().start().sector() < &source_sector);
        let after_persistent_sources = self
            .persistent_cylindrical_sources
            .partition_point(|candidate| candidate.row_system().start().sector() <= &source_sector);
        for persistent_source in self.persistent_cylindrical_sources
            [first_persistent_source..after_persistent_sources]
            .iter()
            .cloned()
        {
            if let Some(rewrite) =
                self.try_persistent_cylindrical_numeric_quotient(source, persistent_source)?
            {
                return Ok(ConcreteRuleDecision::CertifiedRewrite(rewrite));
            }
        }

        // LiteRed specializes `ids` at numeric preparepoints and joins the
        // zero/symmetry relations before Solvej. Keep that order: eliminating
        // over K(n) first is sound but incomplete on rank-changing loci.
        let scout_layers = self.adaptive.scout_point_layers(source)?;
        let source_rows = self.adaptive.source_rows();
        let mut row_requests = Vec::new();
        let mut quotient_terms = 0usize;
        for points in scout_layers {
            for point in points {
                for (source_row_index, relation) in source_rows.iter().enumerate() {
                    check_limit(
                        "concrete quotient source rows",
                        checked_add(row_requests.len(), 1, "concrete quotient source rows")?,
                        self.limits.rewrite.concrete_elimination.max_rows,
                    )?;
                    let raw = match relation.specialize(
                        self.adaptive.context(),
                        &point,
                        self.limits.rewrite.concrete_specialization,
                    ) {
                        Ok(raw) => raw,
                        Err(ParametricRelationError::UnsatisfiableDomain) => continue,
                        Err(error) => return Err(error.into()),
                    };
                    quotient_terms =
                        checked_add(quotient_terms, raw.terms().len(), "concrete quotient terms")?;
                    check_limit(
                        "concrete quotient terms",
                        quotient_terms,
                        self.limits.rewrite.max_quotient_terms,
                    )?;
                    let witnesses = self.witnesses_for_raw(&raw)?;
                    row_requests.push((source_row_index, point.clone(), witnesses));
                }
            }
            match CertifiedConcreteRewrite::from_concrete_quotient_elimination(
                &self.family,
                self.adaptive.context(),
                source.clone(),
                &row_requests,
                self.restrictions.clone(),
                self.ordering,
                self.limits.rewrite,
            ) {
                Ok(rewrite) => return Ok(ConcreteRuleDecision::CertifiedRewrite(rewrite)),
                Err(CertifiedRewriteError::MissingCollectedLhs)
                | Err(CertifiedRewriteError::Sector(SectorFoundationError::NotStrictDescent)) => {}
                Err(error) => return Err(error.into()),
            }
        }

        // A generic K(n) rule is a useful fallback on the ordinary generic
        // locus, but it is deliberately not the sole demand path.
        for candidates in self.adaptive.candidate_layers_for_quotient(source)? {
            for candidate in candidates {
                let raw = match candidate.specialize_raw(self.adaptive.context(), source.powers()) {
                    Ok(raw) => raw,
                    Err(ParametricRuleError::Relation(
                        ParametricRelationError::UnsatisfiableDomain,
                    )) => continue,
                    Err(error) => return Err(error.into()),
                };
                let witnesses = self.witnesses_for_raw(&raw)?;
                match CertifiedConcreteRewrite::from_parametric_quotient(
                    &self.family,
                    self.adaptive.context(),
                    Arc::new(candidate),
                    source.clone(),
                    witnesses,
                    self.restrictions.clone(),
                    self.ordering,
                    self.limits.rewrite,
                ) {
                    Ok(rewrite) => {
                        return Ok(ConcreteRuleDecision::CertifiedRewrite(rewrite));
                    }
                    Err(CertifiedRewriteError::MissingCollectedLhs)
                    | Err(CertifiedRewriteError::Sector(SectorFoundationError::NotStrictDescent)) =>
                        {}
                    Err(error) => return Err(error.into()),
                }
            }
        }
        Ok(ConcreteRuleDecision::Terminal(
            ConcreteTerminalStatus::Uncovered,
        ))
    }

    fn try_persistent_cylindrical_numeric_quotient(
        &self,
        source: &ConcreteIntegralKey,
        persistent_source: Arc<GeneratedCylindricalPersistentEliminationCertificate>,
    ) -> Result<Option<CertifiedConcreteRewrite>, CertifiedFamilyRuleProviderError> {
        let row_system = persistent_source.row_system();
        let start = row_system.start();
        let source_sector = SectorMask::try_from_indices(source.powers())?;
        if start.sector() != &source_sector
            || !partial_assignment_satisfied(start.assignment(), source.powers())
        {
            return Ok(None);
        }

        let available = row_system.stats().retained_rows();
        check_limit(
            "persistent retained rows scanned",
            available,
            self.limits.rewrite.concrete_elimination.max_rows,
        )?;
        preflight_persistent_numeric_specialization_terms(
            &persistent_source,
            self.limits.rewrite.concrete_elimination.max_input_entries,
        )?;
        let replayed_source = ReplayedGeneratedCylindricalPersistentSource::authenticate(
            &self.family,
            self.adaptive.context(),
            &persistent_source,
        )?;
        // Resolve algebra only through the just-authenticated exact source.
        let row_system = replayed_source.source().row_system();
        let start = row_system.start();
        let mut row_requests = Vec::new();
        let mut quotient_terms = 0usize;
        for retained_row_ordinal in 0..available {
            let (_, specialization) = row_system
                .prevalidated_specialization(retained_row_ordinal)
                .ok_or(
                    CertifiedFamilyRuleProviderError::PersistentRetainedRowOutOfRange {
                        row: retained_row_ordinal,
                        available,
                    },
                )?;
            if specialization.assignment() != start.assignment() {
                return Err(CertifiedFamilyRuleProviderError::ForeignPersistentCylindricalSource);
            }
            let raw = match specialization
                .relation_for_bound_reelimination()
                .specialize_with_additional_nonzero_conditions(
                    self.adaptive.context(),
                    source.powers(),
                    specialization
                        .base_assumptions()
                        .iter()
                        .map(|assumption| assumption.condition()),
                    self.limits.rewrite.concrete_specialization,
                ) {
                Ok(raw) => raw,
                Err(ParametricRelationError::UnsatisfiableDomain) => continue,
                Err(error) => return Err(error.into()),
            };
            check_limit(
                "concrete quotient source rows",
                checked_add(row_requests.len(), 1, "concrete quotient source rows")?,
                self.limits.rewrite.concrete_elimination.max_rows,
            )?;
            quotient_terms =
                checked_add(quotient_terms, raw.terms().len(), "concrete quotient terms")?;
            check_limit(
                "concrete quotient terms",
                quotient_terms,
                self.limits.rewrite.max_quotient_terms,
            )?;
            let witnesses = self.witnesses_for_raw(&raw)?;
            row_requests.push((retained_row_ordinal, witnesses));
        }

        match CertifiedConcreteRewrite::from_generated_cylindrical_numeric_quotient_elimination_with_replayed_source(
            &self.family,
            self.adaptive.context(),
            replayed_source,
            source.clone(),
            &row_requests,
            self.restrictions.clone(),
            self.ordering,
            self.limits.rewrite,
        ) {
            Ok(rewrite) => Ok(Some(rewrite)),
            Err(CertifiedRewriteError::MissingCollectedLhs)
            | Err(CertifiedRewriteError::Sector(SectorFoundationError::NotStrictDescent)) => {
                Ok(None)
            }
            Err(error) => Err(error.into()),
        }
    }

    fn witnesses_for_raw(
        &self,
        raw: &ConcreteRelation,
    ) -> Result<Vec<QuotientTermWitness>, CertifiedFamilyRuleProviderError> {
        let mut witnesses = Vec::with_capacity(raw.terms().len());
        for key in raw.terms().keys() {
            let sector = SectorMask::try_from_indices(key.powers())?;
            match self.zero.analyze_sector(&sector) {
                ZeroSectorDecision::ProvedZero(certificate) => witnesses.push(
                    QuotientTermWitness::zero(key.clone(), Arc::new(certificate)),
                ),
                ZeroSectorDecision::ResourceLimited(resource) => {
                    return Err(CertifiedFamilyRuleProviderError::ZeroResource {
                        resource: resource.resource(),
                        requested: resource.requested(),
                        limit: resource.limit(),
                    });
                }
                ZeroSectorDecision::Failed(error) => return Err(error.into()),
                ZeroSectorDecision::Excluded(exclusion) => {
                    if exclusion.violates_cut() {
                        witnesses.push(QuotientTermWitness::cut_zero(key.clone(), exclusion));
                    } else {
                        return Err(CertifiedFamilyRuleProviderError::PatternExcludedSector {
                            source: key.clone(),
                            exclusion,
                        });
                    }
                }
                ZeroSectorDecision::NoZeroCertificate(_) => {
                    let (canonical, path) = self.canonicalize(key)?;
                    witnesses.push(QuotientTermWitness::canonical(key.clone(), canonical, path));
                }
            }
        }
        Ok(witnesses)
    }

    fn excluded_source_decision(
        &self,
        source: &ConcreteIntegralKey,
        exclusion: SectorExclusion,
    ) -> Result<ConcreteRuleDecision, CertifiedFamilyRuleProviderError> {
        if exclusion.violates_cut() {
            return Ok(ConcreteRuleDecision::ProvedZero(
                CertifiedZeroReduction::from_cut_exclusion(
                    &self.family,
                    source.clone(),
                    self.restrictions.clone(),
                    exclusion,
                    self.limits.rewrite,
                )?,
            ));
        }
        Err(CertifiedFamilyRuleProviderError::PatternExcludedSector {
            source: source.clone(),
            exclusion,
        })
    }

    fn canonicalize(
        &self,
        source: &ConcreteIntegralKey,
    ) -> Result<
        (
            ConcreteIntegralKey,
            Vec<Arc<VerifiedInternalFamilyPermutationSymmetry>>,
        ),
        CertifiedFamilyRuleProviderError,
    > {
        check_limit(
            "symmetry orbit states",
            1,
            self.limits.max_symmetry_orbit_states,
        )?;
        let mut best = source.clone();
        let mut best_path = Vec::new();
        let mut visited = BTreeMap::<
            ConcreteIntegralKey,
            Vec<Arc<VerifiedInternalFamilyPermutationSymmetry>>,
        >::new();
        visited.insert(source.clone(), Vec::new());
        let mut retained_path_entries = 0usize;
        let mut queue = VecDeque::from([source.clone()]);
        while let Some(current) = queue.pop_front() {
            let path = visited
                .get(&current)
                .expect("queued symmetry image has a path")
                .clone();
            if self
                .ordering
                .compare(current.powers(), best.powers())?
                .is_lt()
            {
                best = current.clone();
                best_path = path.clone();
            }
            for symmetry in &self.symmetries {
                let image = symmetry.transport_source_key(&current)?;
                if visited.contains_key(&image) {
                    continue;
                }
                let mut image_path = path.clone();
                image_path.push(symmetry.clone());
                check_limit(
                    "symmetry orbit path length",
                    image_path.len(),
                    self.limits.rewrite.max_symmetry_path_length,
                )?;
                check_limit(
                    "symmetry orbit states",
                    checked_add(visited.len(), 1, "symmetry orbit states")?,
                    self.limits.max_symmetry_orbit_states,
                )?;
                retained_path_entries = checked_add(
                    retained_path_entries,
                    image_path.len(),
                    "symmetry orbit path entries",
                )?;
                check_limit(
                    "symmetry orbit path entries",
                    retained_path_entries,
                    self.limits.max_symmetry_orbit_path_entries,
                )?;
                visited.insert(image.clone(), image_path);
                queue.push_back(image);
            }
        }
        Ok((best, best_path))
    }
}

impl ConcreteRuleProvider for CertifiedFamilyRuleProvider<'_> {
    type Error = CertifiedFamilyRuleProviderError;

    fn index_arity(&self) -> usize {
        self.family.denominator_count()
    }

    fn decision_for(
        &mut self,
        integral: &ConcreteIntegralKey,
    ) -> Result<ConcreteRuleDecision, Self::Error> {
        if let Some(decision) = self.cache.get(integral) {
            return Ok(decision.clone());
        }
        let decision = self.discover(integral)?;
        check_limit(
            "cached provider decisions",
            checked_add(self.cache.len(), 1, "cached provider decisions")?,
            self.limits.max_cached_decisions,
        )?;
        let retained = charge_debug_bytes(
            self.retained_proof_debug_bytes,
            &decision,
            self.limits.max_retained_proof_debug_bytes,
        )?;
        self.cache.insert(integral.clone(), decision.clone());
        self.retained_proof_debug_bytes = retained;
        Ok(decision)
    }
}

#[derive(Debug)]
pub enum CertifiedFamilyRuleProviderError {
    WrongArity {
        expected: usize,
        actual: usize,
    },
    ForeignAdaptiveProvider,
    ForeignPersistentCylindricalSource,
    DuplicatePersistentCylindricalSourceScope {
        sector: SectorMask,
        assignment: crate::PartialIndexAssignment,
    },
    UnauthenticatedSourceRows {
        row: Option<usize>,
    },
    PatternExcludedSector {
        source: ConcreteIntegralKey,
        exclusion: SectorExclusion,
    },
    PersistentRetainedRowOutOfRange {
        row: usize,
        available: usize,
    },
    ZeroResource {
        resource: &'static str,
        requested: usize,
        limit: usize,
    },
    ResourceCountOverflow {
        resource: &'static str,
    },
    AllocationFailure {
        resource: &'static str,
        requested: usize,
    },
    ResourceLimit {
        resource: &'static str,
        requested: usize,
        limit: usize,
    },
    Adaptive(AdaptiveRuleSearchError),
    Rewrite(CertifiedRewriteError),
    Rule(ParametricRuleError),
    Sector(SectorFoundationError),
    SymmetryReplay(crate::InternalSymmetryReplayError),
    SymmetryKey(crate::InternalSymmetryKeyTransportError),
    Zero(ZeroSectorError),
    Ibp(ParametricIbpError),
    Persistent(GeneratedCylindricalPersistentEliminationError),
}

impl fmt::Display for CertifiedFamilyRuleProviderError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::WrongArity { expected, actual } => {
                write!(formatter, "provider arity is {actual}, expected {expected}")
            }
            Self::ForeignAdaptiveProvider => {
                formatter.write_str("adaptive provider belongs to a foreign family/context/order")
            }
            Self::ForeignPersistentCylindricalSource => formatter.write_str(
                "persistent cylindrical source belongs to a foreign family, context, or ordering",
            ),
            Self::DuplicatePersistentCylindricalSourceScope { sector, assignment } => write!(
                formatter,
                "duplicate persistent cylindrical source scope at sector {} and assignment {:?}",
                sector.to_bit_string(),
                assignment.entries()
            ),
            Self::UnauthenticatedSourceRows { row: Some(row) } => write!(
                formatter,
                "adaptive source row {row} is not the authenticated generated IBPLI row"
            ),
            Self::UnauthenticatedSourceRows { row: None } => formatter.write_str(
                "adaptive source-row count does not match the authenticated generated IBPLI list",
            ),
            Self::PatternExcludedSector { source, exclusion } => write!(
                formatter,
                "integral {source:?} lies in a pattern-excluded sector outside provider scope ({exclusion:?})"
            ),
            Self::PersistentRetainedRowOutOfRange { row, available } => write!(
                formatter,
                "persistent retained row {row} is outside {available} available rows"
            ),
            Self::ZeroResource {
                resource,
                requested,
                limit,
            }
            | Self::ResourceLimit {
                resource,
                requested,
                limit,
            } => write!(
                formatter,
                "{resource} requested {requested}, configured limit is {limit}"
            ),
            Self::ResourceCountOverflow { resource } => {
                write!(formatter, "{resource} count overflowed usize")
            }
            Self::AllocationFailure {
                resource,
                requested,
            } => write!(
                formatter,
                "could not reserve bounded {resource} storage for {requested} entries"
            ),
            Self::Adaptive(error) => error.fmt(formatter),
            Self::Rewrite(error) => error.fmt(formatter),
            Self::Rule(error) => error.fmt(formatter),
            Self::Sector(error) => error.fmt(formatter),
            Self::SymmetryReplay(error) => error.fmt(formatter),
            Self::SymmetryKey(error) => error.fmt(formatter),
            Self::Zero(error) => error.fmt(formatter),
            Self::Ibp(error) => error.fmt(formatter),
            Self::Persistent(error) => error.fmt(formatter),
        }
    }
}

impl std::error::Error for CertifiedFamilyRuleProviderError {}

impl From<AdaptiveRuleSearchError> for CertifiedFamilyRuleProviderError {
    fn from(value: AdaptiveRuleSearchError) -> Self {
        Self::Adaptive(value)
    }
}
impl From<CertifiedRewriteError> for CertifiedFamilyRuleProviderError {
    fn from(value: CertifiedRewriteError) -> Self {
        Self::Rewrite(value)
    }
}
impl From<ParametricRuleError> for CertifiedFamilyRuleProviderError {
    fn from(value: ParametricRuleError) -> Self {
        Self::Rule(value)
    }
}
impl From<ParametricRelationError> for CertifiedFamilyRuleProviderError {
    fn from(value: ParametricRelationError) -> Self {
        Self::Rewrite(CertifiedRewriteError::Relation(value))
    }
}
impl From<SectorFoundationError> for CertifiedFamilyRuleProviderError {
    fn from(value: SectorFoundationError) -> Self {
        Self::Sector(value)
    }
}
impl From<crate::InternalSymmetryReplayError> for CertifiedFamilyRuleProviderError {
    fn from(value: crate::InternalSymmetryReplayError) -> Self {
        Self::SymmetryReplay(value)
    }
}
impl From<crate::InternalSymmetryKeyTransportError> for CertifiedFamilyRuleProviderError {
    fn from(value: crate::InternalSymmetryKeyTransportError) -> Self {
        Self::SymmetryKey(value)
    }
}
impl From<ZeroSectorError> for CertifiedFamilyRuleProviderError {
    fn from(value: ZeroSectorError) -> Self {
        Self::Zero(value)
    }
}
impl From<ParametricIbpError> for CertifiedFamilyRuleProviderError {
    fn from(value: ParametricIbpError) -> Self {
        Self::Ibp(value)
    }
}
impl From<GeneratedCylindricalPersistentEliminationError> for CertifiedFamilyRuleProviderError {
    fn from(value: GeneratedCylindricalPersistentEliminationError) -> Self {
        Self::Persistent(value)
    }
}

struct BoundedCountWriter {
    bytes: usize,
    limit: usize,
}

impl Write for BoundedCountWriter {
    fn write_str(&mut self, value: &str) -> fmt::Result {
        self.bytes = self.bytes.checked_add(value.len()).ok_or(fmt::Error)?;
        if self.bytes > self.limit {
            return Err(fmt::Error);
        }
        Ok(())
    }
}

fn charge_debug_bytes(
    retained: usize,
    value: &impl fmt::Debug,
    limit: usize,
) -> Result<usize, CertifiedFamilyRuleProviderError> {
    let remaining = limit.saturating_sub(retained);
    let mut writer = BoundedCountWriter {
        bytes: 0,
        limit: remaining,
    };
    if write!(&mut writer, "{value:?}").is_err() {
        return Err(CertifiedFamilyRuleProviderError::ResourceLimit {
            resource: "retained provider proof debug bytes",
            requested: limit.saturating_add(1),
            limit,
        });
    }
    let requested = checked_add(
        retained,
        writer.bytes,
        "retained provider proof debug bytes",
    )?;
    check_limit("retained provider proof debug bytes", requested, limit)?;
    Ok(requested)
}

fn checked_add(
    left: usize,
    right: usize,
    resource: &'static str,
) -> Result<usize, CertifiedFamilyRuleProviderError> {
    left.checked_add(right)
        .ok_or(CertifiedFamilyRuleProviderError::ResourceCountOverflow { resource })
}

fn checked_mul(
    left: usize,
    right: usize,
    resource: &'static str,
) -> Result<usize, CertifiedFamilyRuleProviderError> {
    left.checked_mul(right)
        .ok_or(CertifiedFamilyRuleProviderError::ResourceCountOverflow { resource })
}

const fn portable_limit(preferred: u128) -> usize {
    if preferred > usize::MAX as u128 {
        usize::MAX
    } else {
        preferred as usize
    }
}

fn check_limit(
    resource: &'static str,
    requested: usize,
    limit: usize,
) -> Result<(), CertifiedFamilyRuleProviderError> {
    if requested > limit {
        Err(CertifiedFamilyRuleProviderError::ResourceLimit {
            resource,
            requested,
            limit,
        })
    } else {
        Ok(())
    }
}

fn partial_assignment_satisfied(
    assignment: &crate::PartialIndexAssignment,
    indices: &[i64],
) -> bool {
    indices.len() == assignment.arity()
        && assignment
            .entries()
            .iter()
            .all(|&(position, expected)| indices[position] == expected)
}
