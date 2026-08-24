//! Proof-bearing canonicalization of concrete integral keys by verified
//! internal family symmetries.
//!
//! This wrapper is independent of rule discovery. It computes the complete
//! bounded orbit of a requested key under symmetries already retained by one
//! generated row-span certificate, returns a certified rewrite to the unique
//! easiest orbit representative, and otherwise delegates unchanged. The
//! row-span allocation is retained so startup never deep-clones symmetry
//! proofs. A selected path is lazily rebound to the provider's exact
//! cut/pattern policy and cached behind `Arc`, at most once per symmetry.

use std::collections::{BTreeMap, VecDeque};
use std::error::Error;
use std::fmt::{self, Write};
use std::sync::Arc;

use crate::reduction_engine::{ConcreteRuleDecision, ConcreteRuleProvider};
use crate::{
    CertifiedConcreteRewrite, CertifiedRewriteError, CertifiedRewriteLimits, ConcreteIntegralKey,
    GeneratedSymbolicRowSpanCertificate, GeneratedSymbolicRowSpanError, IntegralFamily,
    IntegralOrderingPolicy, InternalSymmetryCompatibilityError, InternalSymmetryKeyTransportError,
    InternalSymmetryReplayError, ParametricCoefficientContext, SectorFoundationError,
    SectorRestrictions, VerifiedInternalFamilyPermutationSymmetry,
    compile_internal_family_permutation_symmetry,
};

pub const CERTIFIED_SYMMETRY_CANONICALIZING_RULE_PROVIDER_V1_SCHEMA: &str =
    "rustred-certified-symmetry-canonicalizing-rule-provider-v1";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CertifiedSymmetryCanonicalizingRuleProviderLimits {
    pub rewrite: CertifiedRewriteLimits,
    pub max_input_symmetries: usize,
    pub max_compatible_symmetries: usize,
    pub max_orbit_states_per_query: usize,
    pub max_orbit_transitions_per_query: usize,
    pub max_orbit_path_entries_per_query: usize,
    pub max_queries: usize,
    pub max_terminal_canonicalizations: usize,
    pub max_retained_cloned_symmetries: usize,
    pub max_retained_cloned_symmetry_debug_bytes: usize,
}

impl Default for CertifiedSymmetryCanonicalizingRuleProviderLimits {
    fn default() -> Self {
        Self {
            rewrite: CertifiedRewriteLimits::default(),
            max_input_symmetries: 1_000_000,
            max_compatible_symmetries: 1_000_000,
            max_orbit_states_per_query: 1_000_000,
            max_orbit_transitions_per_query: 100_000_000,
            max_orbit_path_entries_per_query: 10_000_000,
            max_queries: 100_000_000,
            max_terminal_canonicalizations: 10_000_000,
            max_retained_cloned_symmetries: 1_000_000,
            max_retained_cloned_symmetry_debug_bytes: 2 * 1024 * 1024 * 1024,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct CertifiedSymmetryCanonicalizingRuleProviderStats {
    queries: usize,
    delegated_queries: usize,
    symmetry_rewrites: usize,
    orbit_states: usize,
    orbit_transitions: usize,
    retained_cloned_symmetries: usize,
    retained_cloned_symmetry_debug_bytes: usize,
}

impl CertifiedSymmetryCanonicalizingRuleProviderStats {
    pub const fn queries(self) -> usize {
        self.queries
    }
    pub const fn delegated_queries(self) -> usize {
        self.delegated_queries
    }
    pub const fn symmetry_rewrites(self) -> usize {
        self.symmetry_rewrites
    }
    pub const fn orbit_states(self) -> usize {
        self.orbit_states
    }
    pub const fn orbit_transitions(self) -> usize {
        self.orbit_transitions
    }
    pub const fn retained_cloned_symmetries(self) -> usize {
        self.retained_cloned_symmetries
    }
    pub const fn retained_cloned_symmetry_debug_bytes(self) -> usize {
        self.retained_cloned_symmetry_debug_bytes
    }
}

pub struct CertifiedSymmetryCanonicalizingRuleProvider<'family, Provider> {
    family: &'family IntegralFamily,
    context: &'family ParametricCoefficientContext,
    restrictions: SectorRestrictions,
    row_span: Option<Arc<GeneratedSymbolicRowSpanCertificate>>,
    compatible_ordinals: Box<[usize]>,
    cached_symmetries: Vec<Option<Arc<VerifiedInternalFamilyPermutationSymmetry>>>,
    ordering: IntegralOrderingPolicy,
    inner: Provider,
    limits: CertifiedSymmetryCanonicalizingRuleProviderLimits,
    stats: CertifiedSymmetryCanonicalizingRuleProviderStats,
}

impl<'family, Provider> CertifiedSymmetryCanonicalizingRuleProvider<'family, Provider>
where
    Provider: ConcreteRuleProvider,
{
    pub const SCHEMA: &'static str = CERTIFIED_SYMMETRY_CANONICALIZING_RULE_PROVIDER_V1_SCHEMA;

    pub fn try_new(
        family: &'family IntegralFamily,
        context: &'family ParametricCoefficientContext,
        restrictions: SectorRestrictions,
        row_span: Arc<GeneratedSymbolicRowSpanCertificate>,
        ordering: IntegralOrderingPolicy,
        inner: Provider,
        limits: CertifiedSymmetryCanonicalizingRuleProviderLimits,
    ) -> Result<Self, CertifiedSymmetryCanonicalizingRuleProviderError<Provider::Error>> {
        Self::preflight_binding(family, context, &restrictions, Some(&row_span), &inner)?;
        row_span.replay(family, context)?;
        Self::try_new_impl(
            family,
            context,
            restrictions,
            Some(row_span),
            ordering,
            inner,
            None,
            limits,
        )
    }

    /// Install the exact row-span allocation already replayed by a
    /// family-level certificate. Public callers use [`Self::try_new`] and
    /// receive an independent replay.
    pub(crate) fn try_new_with_replayed_row_span(
        family: &'family IntegralFamily,
        context: &'family ParametricCoefficientContext,
        restrictions: SectorRestrictions,
        row_span: Arc<GeneratedSymbolicRowSpanCertificate>,
        ordering: IntegralOrderingPolicy,
        inner: Provider,
        replayed_row_span: &Arc<GeneratedSymbolicRowSpanCertificate>,
        limits: CertifiedSymmetryCanonicalizingRuleProviderLimits,
    ) -> Result<Self, CertifiedSymmetryCanonicalizingRuleProviderError<Provider::Error>> {
        Self::try_new_impl(
            family,
            context,
            restrictions,
            Some(row_span),
            ordering,
            inner,
            Some(replayed_row_span),
            limits,
        )
    }

    /// Construct the exact identity wrapper used when a family inventory has
    /// no generated-stage work and therefore owns no row-span certificate.
    pub(crate) fn try_new_without_symmetries(
        family: &'family IntegralFamily,
        context: &'family ParametricCoefficientContext,
        restrictions: SectorRestrictions,
        ordering: IntegralOrderingPolicy,
        inner: Provider,
        limits: CertifiedSymmetryCanonicalizingRuleProviderLimits,
    ) -> Result<Self, CertifiedSymmetryCanonicalizingRuleProviderError<Provider::Error>> {
        Self::try_new_impl(
            family,
            context,
            restrictions,
            None,
            ordering,
            inner,
            None,
            limits,
        )
    }

    fn try_new_impl(
        family: &'family IntegralFamily,
        context: &'family ParametricCoefficientContext,
        restrictions: SectorRestrictions,
        row_span: Option<Arc<GeneratedSymbolicRowSpanCertificate>>,
        ordering: IntegralOrderingPolicy,
        inner: Provider,
        replayed_row_span: Option<&Arc<GeneratedSymbolicRowSpanCertificate>>,
        limits: CertifiedSymmetryCanonicalizingRuleProviderLimits,
    ) -> Result<Self, CertifiedSymmetryCanonicalizingRuleProviderError<Provider::Error>> {
        Self::preflight_binding(family, context, &restrictions, row_span.as_deref(), &inner)?;
        if let Some(replayed_row_span) = replayed_row_span {
            if !row_span
                .as_ref()
                .is_some_and(|row_span| Arc::ptr_eq(row_span, replayed_row_span))
            {
                return Err(
                    CertifiedSymmetryCanonicalizingRuleProviderError::ReplayedRowSpanAllocationMismatch,
                );
            }
        }
        check_limit(
            "input symmetry certificates",
            row_span
                .as_ref()
                .map_or(0, |row_span| row_span.symmetries().len()),
            limits.max_input_symmetries,
        )?;
        let mut compatible_ordinals = Vec::new();
        for (ordinal, symmetry) in row_span
            .iter()
            .flat_map(|row_span| row_span.symmetries())
            .enumerate()
        {
            match symmetry.validate_restriction_compatibility(family, &restrictions) {
                Ok(()) => {
                    check_limit(
                        "compatible symmetry certificates",
                        compatible_ordinals.len().checked_add(1).ok_or(
                            CertifiedSymmetryCanonicalizingRuleProviderError::ResourceCountOverflow {
                                resource: "compatible symmetry certificates",
                            },
                        )?,
                        limits.max_compatible_symmetries,
                    )?;
                    compatible_ordinals.push(ordinal);
                }
                Err(
                    InternalSymmetryCompatibilityError::CutTransportMismatch { .. }
                    | InternalSymmetryCompatibilityError::SectorPatternTransportMismatch { .. },
                ) => {}
                Err(error) => {
                    return Err(
                        CertifiedSymmetryCanonicalizingRuleProviderError::Compatibility(error),
                    );
                }
            }
        }
        let cached_symmetries = vec![
            None;
            row_span
                .as_ref()
                .map_or(0, |row_span| row_span.symmetries().len())
        ];
        Ok(Self {
            family,
            context,
            restrictions,
            row_span,
            compatible_ordinals: compatible_ordinals.into_boxed_slice(),
            cached_symmetries,
            ordering,
            inner,
            limits,
            stats: CertifiedSymmetryCanonicalizingRuleProviderStats::default(),
        })
    }

    fn preflight_binding(
        family: &IntegralFamily,
        context: &ParametricCoefficientContext,
        restrictions: &SectorRestrictions,
        row_span: Option<&GeneratedSymbolicRowSpanCertificate>,
        inner: &Provider,
    ) -> Result<(), CertifiedSymmetryCanonicalizingRuleProviderError<Provider::Error>> {
        if restrictions.arity() != family.denominator_count() {
            return Err(
                CertifiedSymmetryCanonicalizingRuleProviderError::WrongRestrictionsArity {
                    expected: family.denominator_count(),
                    actual: restrictions.arity(),
                },
            );
        }
        if inner.index_arity() != family.denominator_count() {
            return Err(
                CertifiedSymmetryCanonicalizingRuleProviderError::WrongProviderArity {
                    expected: family.denominator_count(),
                    actual: inner.index_arity(),
                },
            );
        }
        if let Some(row_span) = row_span {
            if row_span.family_fingerprint() != family.fingerprint() {
                return Err(CertifiedSymmetryCanonicalizingRuleProviderError::WrongFamily);
            }
            if row_span.context_fingerprint() != context.fingerprint() {
                return Err(CertifiedSymmetryCanonicalizingRuleProviderError::WrongContext);
            }
        }
        Ok(())
    }

    pub const fn family(&self) -> &IntegralFamily {
        self.family
    }
    pub const fn context(&self) -> &ParametricCoefficientContext {
        self.context
    }
    pub const fn restrictions(&self) -> &SectorRestrictions {
        &self.restrictions
    }
    pub fn row_span_arc(&self) -> Option<&Arc<GeneratedSymbolicRowSpanCertificate>> {
        self.row_span.as_ref()
    }
    pub fn compatible_symmetry_ordinals(&self) -> &[usize] {
        &self.compatible_ordinals
    }
    pub const fn ordering(&self) -> IntegralOrderingPolicy {
        self.ordering
    }
    pub const fn inner(&self) -> &Provider {
        &self.inner
    }
    pub fn inner_mut(&mut self) -> &mut Provider {
        &mut self.inner
    }
    pub fn into_inner(self) -> Provider {
        self.inner
    }
    pub const fn limits(&self) -> CertifiedSymmetryCanonicalizingRuleProviderLimits {
        self.limits
    }
    pub const fn stats(&self) -> CertifiedSymmetryCanonicalizingRuleProviderStats {
        self.stats
    }

    pub fn canonical_key(
        &self,
        source: &ConcreteIntegralKey,
    ) -> Result<
        ConcreteIntegralKey,
        CertifiedSymmetryCanonicalizingRuleProviderError<Provider::Error>,
    > {
        self.orbit(source).map(|orbit| orbit.best)
    }

    pub fn canonicalize_terminals<T: Clone + PartialEq>(
        &self,
        terminals: impl IntoIterator<Item = (ConcreteIntegralKey, T)>,
    ) -> Result<
        Vec<(ConcreteIntegralKey, T)>,
        CertifiedSymmetryCanonicalizingRuleProviderError<Provider::Error>,
    > {
        let mut canonical = BTreeMap::new();
        let mut canonicalizations = 0usize;
        for (source, terminal) in terminals {
            canonicalizations = canonicalizations.checked_add(1).ok_or(
                CertifiedSymmetryCanonicalizingRuleProviderError::ResourceCountOverflow {
                    resource: "terminal canonicalizations",
                },
            )?;
            check_limit(
                "terminal canonicalizations",
                canonicalizations,
                self.limits.max_terminal_canonicalizations,
            )?;
            let key = self.canonical_key(&source)?;
            if let Some(existing) = canonical.get(&key) {
                if existing != &terminal {
                    return Err(CertifiedSymmetryCanonicalizingRuleProviderError::ConflictingCanonicalTerminal {
                        canonical: key,
                    });
                }
            } else {
                canonical.insert(key, terminal);
            }
        }
        Ok(canonical.into_iter().collect())
    }

    pub fn replay(
        &self,
    ) -> Result<(), CertifiedSymmetryCanonicalizingRuleProviderError<Provider::Error>> {
        if let Some(row_span) = &self.row_span {
            row_span.replay(self.family, self.context)?;
        }
        self.replay_impl(None)
    }

    /// Replay wrapper-local binding after a family-level owner has just
    /// replayed the exact shared row-span allocation.
    pub(crate) fn replay_with_replayed_row_span(
        &self,
        replayed_row_span: &Arc<GeneratedSymbolicRowSpanCertificate>,
    ) -> Result<(), CertifiedSymmetryCanonicalizingRuleProviderError<Provider::Error>> {
        self.replay_impl(Some(replayed_row_span))
    }

    fn replay_impl(
        &self,
        replayed_row_span: Option<&Arc<GeneratedSymbolicRowSpanCertificate>>,
    ) -> Result<(), CertifiedSymmetryCanonicalizingRuleProviderError<Provider::Error>> {
        if self.row_span.as_ref().is_some_and(|row_span| {
            row_span.family_fingerprint() != self.family.fingerprint()
                || row_span.context_fingerprint() != self.context.fingerprint()
        }) || self.restrictions.arity() != self.family.denominator_count()
            || self.cached_symmetries.len() != self.symmetries().len()
        {
            return Err(CertifiedSymmetryCanonicalizingRuleProviderError::ReplayMismatch);
        }
        if let Some(replayed_row_span) = replayed_row_span {
            if !self
                .row_span
                .as_ref()
                .is_some_and(|row_span| Arc::ptr_eq(row_span, replayed_row_span))
            {
                return Err(
                    CertifiedSymmetryCanonicalizingRuleProviderError::ReplayedRowSpanAllocationMismatch,
                );
            }
        }
        let mut expected = Vec::new();
        for (ordinal, symmetry) in self.symmetries().iter().enumerate() {
            match symmetry.validate_restriction_compatibility(self.family, &self.restrictions) {
                Ok(()) => expected.push(ordinal),
                Err(
                    InternalSymmetryCompatibilityError::CutTransportMismatch { .. }
                    | InternalSymmetryCompatibilityError::SectorPatternTransportMismatch { .. },
                ) => {}
                Err(error) => return Err(error.into()),
            }
        }
        if expected.as_slice() != self.compatible_ordinals.as_ref() {
            return Err(CertifiedSymmetryCanonicalizingRuleProviderError::ReplayMismatch);
        }
        for (ordinal, cached) in self.cached_symmetries.iter().enumerate() {
            if let Some(symmetry) = cached {
                if !self.compatible_ordinals.contains(&ordinal) {
                    return Err(CertifiedSymmetryCanonicalizingRuleProviderError::ReplayMismatch);
                }
                symmetry.replay(
                    self.family,
                    &self.restrictions,
                    self.limits.rewrite.symmetry,
                )?;
                let source = &self.symmetries()[ordinal];
                if symmetry.denominator_permutation() != source.denominator_permutation() {
                    return Err(CertifiedSymmetryCanonicalizingRuleProviderError::ReplayMismatch);
                }
            }
        }
        Ok(())
    }

    fn orbit(
        &self,
        source: &ConcreteIntegralKey,
    ) -> Result<Orbit, CertifiedSymmetryCanonicalizingRuleProviderError<Provider::Error>> {
        self.validate_request(source)?;
        check_limit(
            "symmetry orbit states",
            1,
            self.limits.max_orbit_states_per_query,
        )?;
        let mut best = source.clone();
        let mut best_path = Vec::new();
        let mut visited = BTreeMap::<ConcreteIntegralKey, Vec<usize>>::new();
        visited.insert(source.clone(), Vec::new());
        let mut queue = VecDeque::from([source.clone()]);
        let mut path_entries = 0usize;
        let mut transitions = 0usize;
        while let Some(current) = queue.pop_front() {
            let path = visited
                .get(&current)
                .expect("queued orbit state has a path")
                .clone();
            if self
                .ordering
                .compare(current.powers(), best.powers())?
                .is_lt()
            {
                best = current.clone();
                best_path = path.clone();
            }
            for &ordinal in self.compatible_ordinals.iter() {
                transitions = transitions.checked_add(1).ok_or(
                    CertifiedSymmetryCanonicalizingRuleProviderError::ResourceCountOverflow {
                        resource: "symmetry orbit transitions",
                    },
                )?;
                check_limit(
                    "symmetry orbit transitions",
                    transitions,
                    self.limits.max_orbit_transitions_per_query,
                )?;
                let image = self.symmetries()[ordinal].transport_source_key(&current)?;
                if visited.contains_key(&image) {
                    continue;
                }
                let mut image_path = path.clone();
                image_path.push(ordinal);
                check_limit(
                    "symmetry orbit path length",
                    image_path.len(),
                    self.limits.rewrite.max_symmetry_path_length,
                )?;
                check_limit(
                    "symmetry orbit states",
                    visited.len().checked_add(1).ok_or(
                        CertifiedSymmetryCanonicalizingRuleProviderError::ResourceCountOverflow {
                            resource: "symmetry orbit states",
                        },
                    )?,
                    self.limits.max_orbit_states_per_query,
                )?;
                path_entries = path_entries.checked_add(image_path.len()).ok_or(
                    CertifiedSymmetryCanonicalizingRuleProviderError::ResourceCountOverflow {
                        resource: "symmetry orbit path entries",
                    },
                )?;
                check_limit(
                    "symmetry orbit path entries",
                    path_entries,
                    self.limits.max_orbit_path_entries_per_query,
                )?;
                visited.insert(image.clone(), image_path);
                queue.push_back(image);
            }
        }
        Ok(Orbit {
            best,
            best_path,
            states: visited.len(),
            transitions,
        })
    }

    fn validate_request(
        &self,
        source: &ConcreteIntegralKey,
    ) -> Result<(), CertifiedSymmetryCanonicalizingRuleProviderError<Provider::Error>> {
        let expected = self.family.denominator_count();
        let actual_inner = self.inner.index_arity();
        if actual_inner != expected {
            return Err(
                CertifiedSymmetryCanonicalizingRuleProviderError::ProviderArityChanged {
                    expected,
                    actual: actual_inner,
                },
            );
        }
        if source.powers().len() != expected {
            return Err(
                CertifiedSymmetryCanonicalizingRuleProviderError::WrongArity {
                    expected,
                    actual: source.powers().len(),
                },
            );
        }
        Ok(())
    }

    /// Compile and charge every uncached proof selected by one orbit path
    /// without mutating provider state. The caller commits the staged cache
    /// only after the complete certified rewrite has been built successfully.
    fn prepare_symmetry_path(
        &self,
        ordinals: &[usize],
    ) -> Result<
        PreparedSymmetryPath,
        CertifiedSymmetryCanonicalizingRuleProviderError<Provider::Error>,
    > {
        let mut missing = BTreeMap::new();
        for &ordinal in ordinals {
            if self.cached_symmetries[ordinal].is_none() {
                missing.insert(ordinal, ());
            }
        }
        let retained = self
            .stats
            .retained_cloned_symmetries
            .checked_add(missing.len())
            .ok_or(
                CertifiedSymmetryCanonicalizingRuleProviderError::ResourceCountOverflow {
                    resource: "retained cloned symmetry certificates",
                },
            )?;
        check_limit(
            "retained cloned symmetry certificates",
            retained,
            self.limits.max_retained_cloned_symmetries,
        )?;

        let mut retained_bytes = self.stats.retained_cloned_symmetry_debug_bytes;
        let mut staged = BTreeMap::new();
        for &ordinal in missing.keys() {
            let source = &self.symmetries()[ordinal];
            let symmetry = compile_internal_family_permutation_symmetry(
                self.family,
                &self.restrictions,
                source.affine_map().clone(),
            )?;
            symmetry.replay(
                self.family,
                &self.restrictions,
                self.limits.rewrite.symmetry,
            )?;
            let bytes = debug_bytes(
                &symmetry,
                self.limits
                    .max_retained_cloned_symmetry_debug_bytes
                    .saturating_sub(retained_bytes),
            )?;
            retained_bytes = retained_bytes.checked_add(bytes).ok_or(
                CertifiedSymmetryCanonicalizingRuleProviderError::ResourceCountOverflow {
                    resource: "retained cloned symmetry debug bytes",
                },
            )?;
            check_limit(
                "retained cloned symmetry debug bytes",
                retained_bytes,
                self.limits.max_retained_cloned_symmetry_debug_bytes,
            )?;
            staged.insert(ordinal, Arc::new(symmetry));
        }

        let path = ordinals
            .iter()
            .map(|&ordinal| {
                self.cached_symmetries[ordinal]
                    .as_ref()
                    .or_else(|| staged.get(&ordinal))
                    .expect("every selected symmetry is cached or staged")
                    .clone()
            })
            .collect();
        Ok(PreparedSymmetryPath {
            path,
            staged: staged.into_iter().collect(),
            retained,
            retained_bytes,
        })
    }

    fn commit_symmetry_path(&mut self, prepared: PreparedSymmetryPath) {
        for (ordinal, symmetry) in prepared.staged {
            debug_assert!(self.cached_symmetries[ordinal].is_none());
            self.cached_symmetries[ordinal] = Some(symmetry);
        }
        self.stats.retained_cloned_symmetries = prepared.retained;
        self.stats.retained_cloned_symmetry_debug_bytes = prepared.retained_bytes;
    }

    fn symmetries(&self) -> &[VerifiedInternalFamilyPermutationSymmetry] {
        self.row_span
            .as_ref()
            .map_or(&[], |row_span| row_span.symmetries())
    }
}

impl<Provider> ConcreteRuleProvider for CertifiedSymmetryCanonicalizingRuleProvider<'_, Provider>
where
    Provider: ConcreteRuleProvider,
{
    type Error = CertifiedSymmetryCanonicalizingRuleProviderError<Provider::Error>;

    fn index_arity(&self) -> usize {
        self.family.denominator_count()
    }

    fn decision_for(
        &mut self,
        integral: &ConcreteIntegralKey,
    ) -> Result<ConcreteRuleDecision, Self::Error> {
        let queries = self.stats.queries.checked_add(1).ok_or(
            CertifiedSymmetryCanonicalizingRuleProviderError::ResourceCountOverflow {
                resource: "symmetry provider queries",
            },
        )?;
        check_limit(
            "symmetry provider queries",
            queries,
            self.limits.max_queries,
        )?;
        let orbit = self.orbit(integral)?;
        let states = self.stats.orbit_states.checked_add(orbit.states).ok_or(
            CertifiedSymmetryCanonicalizingRuleProviderError::ResourceCountOverflow {
                resource: "aggregate symmetry orbit states",
            },
        )?;
        let transitions = self
            .stats
            .orbit_transitions
            .checked_add(orbit.transitions)
            .ok_or(
                CertifiedSymmetryCanonicalizingRuleProviderError::ResourceCountOverflow {
                    resource: "aggregate symmetry orbit transitions",
                },
            )?;
        if orbit.best == *integral {
            let decision = self
                .inner
                .decision_for(integral)
                .map_err(CertifiedSymmetryCanonicalizingRuleProviderError::Inner)?;
            self.stats.queries = queries;
            self.stats.delegated_queries = self.stats.delegated_queries.checked_add(1).ok_or(
                CertifiedSymmetryCanonicalizingRuleProviderError::ResourceCountOverflow {
                    resource: "symmetry delegated queries",
                },
            )?;
            self.stats.orbit_states = states;
            self.stats.orbit_transitions = transitions;
            return Ok(decision);
        }
        let prepared = self.prepare_symmetry_path(&orbit.best_path)?;
        let rewrite = CertifiedConcreteRewrite::from_symmetry(
            self.family,
            integral.clone(),
            orbit.best,
            prepared.path.clone(),
            self.restrictions.clone(),
            self.ordering,
            self.limits.rewrite,
        )?;
        self.commit_symmetry_path(prepared);
        self.stats.queries = queries;
        self.stats.symmetry_rewrites = self.stats.symmetry_rewrites.checked_add(1).ok_or(
            CertifiedSymmetryCanonicalizingRuleProviderError::ResourceCountOverflow {
                resource: "symmetry rewrites",
            },
        )?;
        self.stats.orbit_states = states;
        self.stats.orbit_transitions = transitions;
        Ok(ConcreteRuleDecision::CertifiedRewrite(rewrite))
    }
}

struct Orbit {
    best: ConcreteIntegralKey,
    best_path: Vec<usize>,
    states: usize,
    transitions: usize,
}

struct PreparedSymmetryPath {
    path: Vec<Arc<VerifiedInternalFamilyPermutationSymmetry>>,
    staged: Vec<(usize, Arc<VerifiedInternalFamilyPermutationSymmetry>)>,
    retained: usize,
    retained_bytes: usize,
}

fn check_limit<ProviderError>(
    resource: &'static str,
    requested: usize,
    limit: usize,
) -> Result<(), CertifiedSymmetryCanonicalizingRuleProviderError<ProviderError>>
where
    ProviderError: Error + Send + Sync + 'static,
{
    if requested <= limit {
        Ok(())
    } else {
        Err(
            CertifiedSymmetryCanonicalizingRuleProviderError::ResourceLimit {
                resource,
                requested,
                limit,
            },
        )
    }
}

fn debug_bytes<ProviderError>(
    value: &impl fmt::Debug,
    limit: usize,
) -> Result<usize, CertifiedSymmetryCanonicalizingRuleProviderError<ProviderError>>
where
    ProviderError: Error + Send + Sync + 'static,
{
    struct Counter {
        bytes: usize,
        limit: usize,
    }
    impl Write for Counter {
        fn write_str(&mut self, value: &str) -> fmt::Result {
            self.bytes = self.bytes.checked_add(value.len()).ok_or(fmt::Error)?;
            if self.bytes > self.limit {
                Err(fmt::Error)
            } else {
                Ok(())
            }
        }
    }
    let mut counter = Counter { bytes: 0, limit };
    if write!(&mut counter, "{value:?}").is_err() {
        return Err(
            CertifiedSymmetryCanonicalizingRuleProviderError::ResourceLimit {
                resource: "retained cloned symmetry debug bytes",
                requested: limit.saturating_add(1),
                limit,
            },
        );
    }
    Ok(counter.bytes)
}

#[derive(Debug)]
pub enum CertifiedSymmetryCanonicalizingRuleProviderError<ProviderError>
where
    ProviderError: Error + Send + Sync + 'static,
{
    WrongFamily,
    WrongContext,
    ReplayedRowSpanAllocationMismatch,
    WrongArity {
        expected: usize,
        actual: usize,
    },
    WrongRestrictionsArity {
        expected: usize,
        actual: usize,
    },
    WrongProviderArity {
        expected: usize,
        actual: usize,
    },
    ProviderArityChanged {
        expected: usize,
        actual: usize,
    },
    ConflictingCanonicalTerminal {
        canonical: ConcreteIntegralKey,
    },
    ReplayMismatch,
    ResourceCountOverflow {
        resource: &'static str,
    },
    ResourceLimit {
        resource: &'static str,
        requested: usize,
        limit: usize,
    },
    Compatibility(InternalSymmetryCompatibilityError),
    SymmetryReplay(InternalSymmetryReplayError),
    SymmetryKey(InternalSymmetryKeyTransportError),
    Rewrite(CertifiedRewriteError),
    Ordering(SectorFoundationError),
    RowSpan(GeneratedSymbolicRowSpanError),
    Inner(ProviderError),
}

impl<ProviderError> fmt::Display for CertifiedSymmetryCanonicalizingRuleProviderError<ProviderError>
where
    ProviderError: Error + Send + Sync + 'static,
{
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::WrongFamily => formatter.write_str("symmetry provider family mismatch"),
            Self::WrongContext => formatter.write_str("symmetry provider context mismatch"),
            Self::ReplayedRowSpanAllocationMismatch => formatter.write_str(
                "symmetry provider did not receive the exact replayed row-span allocation",
            ),
            Self::WrongArity { expected, actual } => write!(
                formatter,
                "symmetry provider request arity is {actual}, expected {expected}"
            ),
            Self::WrongRestrictionsArity { expected, actual } => write!(
                formatter,
                "symmetry provider restriction arity is {actual}, expected {expected}"
            ),
            Self::WrongProviderArity { expected, actual }
            | Self::ProviderArityChanged { expected, actual } => write!(
                formatter,
                "symmetry provider inner arity is {actual}, expected {expected}"
            ),
            Self::ConflictingCanonicalTerminal { canonical } => write!(
                formatter,
                "conflicting explicit master terminals canonicalize to {canonical:?}"
            ),
            Self::ReplayMismatch => formatter.write_str("symmetry provider replay mismatch"),
            Self::ResourceCountOverflow { resource } => write!(
                formatter,
                "symmetry provider {resource} count overflowed usize"
            ),
            Self::ResourceLimit {
                resource,
                requested,
                limit,
            } => write!(
                formatter,
                "symmetry provider {resource} requested {requested}, configured limit is {limit}"
            ),
            Self::Compatibility(error) => error.fmt(formatter),
            Self::SymmetryReplay(error) => error.fmt(formatter),
            Self::SymmetryKey(error) => error.fmt(formatter),
            Self::Rewrite(error) => error.fmt(formatter),
            Self::Ordering(error) => error.fmt(formatter),
            Self::RowSpan(error) => error.fmt(formatter),
            Self::Inner(error) => fmt::Display::fmt(error, formatter),
        }
    }
}

impl<ProviderError> Error for CertifiedSymmetryCanonicalizingRuleProviderError<ProviderError> where
    ProviderError: Error + Send + Sync + 'static
{
}

impl<ProviderError> From<InternalSymmetryCompatibilityError>
    for CertifiedSymmetryCanonicalizingRuleProviderError<ProviderError>
where
    ProviderError: Error + Send + Sync + 'static,
{
    fn from(value: InternalSymmetryCompatibilityError) -> Self {
        Self::Compatibility(value)
    }
}
impl<ProviderError> From<InternalSymmetryReplayError>
    for CertifiedSymmetryCanonicalizingRuleProviderError<ProviderError>
where
    ProviderError: Error + Send + Sync + 'static,
{
    fn from(value: InternalSymmetryReplayError) -> Self {
        Self::SymmetryReplay(value)
    }
}
impl<ProviderError> From<InternalSymmetryKeyTransportError>
    for CertifiedSymmetryCanonicalizingRuleProviderError<ProviderError>
where
    ProviderError: Error + Send + Sync + 'static,
{
    fn from(value: InternalSymmetryKeyTransportError) -> Self {
        Self::SymmetryKey(value)
    }
}
impl<ProviderError> From<CertifiedRewriteError>
    for CertifiedSymmetryCanonicalizingRuleProviderError<ProviderError>
where
    ProviderError: Error + Send + Sync + 'static,
{
    fn from(value: CertifiedRewriteError) -> Self {
        Self::Rewrite(value)
    }
}
impl<ProviderError> From<SectorFoundationError>
    for CertifiedSymmetryCanonicalizingRuleProviderError<ProviderError>
where
    ProviderError: Error + Send + Sync + 'static,
{
    fn from(value: SectorFoundationError) -> Self {
        Self::Ordering(value)
    }
}

impl<ProviderError> From<GeneratedSymbolicRowSpanError>
    for CertifiedSymmetryCanonicalizingRuleProviderError<ProviderError>
where
    ProviderError: Error + Send + Sync + 'static,
{
    fn from(value: GeneratedSymbolicRowSpanError) -> Self {
        Self::RowSpan(value)
    }
}
