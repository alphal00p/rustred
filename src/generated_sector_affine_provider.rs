//! Topology-neutral conditional-provider tier for sealed generated-affine
//! sector owners.
//!
//! This wrapper deliberately sits immediately outside the unchanged global
//! V1 provider and immediately inside the V1 coordinate-equality wrapper.
//! It never copies a private relation and never inserts an affine rule into
//! the globally valid candidate database: a query is routed solely by its
//! [`SectorMask`] to one replayed owner allocation.

use std::fmt;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::sync::Arc;

use crate::generated_sector_affine_effective_coverage::{
    GeneratedSectorAffineConcretePointOutcome, GeneratedSectorAffineEffectiveCoverageCertificate,
    GeneratedSectorAffineEffectiveCoverageError, GeneratedSectorAffinePointDisposition,
    GeneratedSectorAffineRuleApplicationError, GeneratedSectorAffineRuleApplicationLimits,
};
use crate::reduction_engine::{ConcreteRuleDecision, ConcreteRuleProvider};
use crate::{
    ConcreteIntegralKey, IntegralFamily, ParametricCoefficientContext, SectorFoundationError,
    SectorMask,
};

pub(crate) const GENERATED_SECTOR_AFFINE_CONDITIONAL_RULE_PROVIDER_V1_SCHEMA: &str =
    "rustred-generated-sector-affine-conditional-rule-provider-v1";

/// Retention and runtime bounds for the first affine-provider tier.
///
/// `application` is the complete per-owner-query envelope.  Installed owners
/// are replayed once by construction, so the provider uses the owner's
/// already-replayed application seam; consequently its default replay count
/// is exactly zero.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct GeneratedSectorAffineConditionalRuleProviderLimits {
    pub(crate) application: GeneratedSectorAffineRuleApplicationLimits,
    pub(crate) max_installed_owners: usize,
    pub(crate) max_installed_sectors: usize,
    pub(crate) max_retained_outer_owner_bytes: usize,
    /// Heap retained by the sorted `(sector, owner capability)` index.
    pub(crate) max_retained_owner_index_bytes: usize,
    /// Largest provider-owned temporary allocation used while validating the
    /// borrowed owner set. Owner-certificate replay scratch is bounded by the
    /// replayed certificates themselves and is not charged a second time.
    pub(crate) max_temporary_owner_index_bytes: usize,
    /// Maximum provider-visible build footprint: retained outer owner
    /// authority plus the larger of temporary and retained index storage.
    pub(crate) max_peak_visible_build_bytes: usize,
    pub(crate) max_queries: usize,
    pub(crate) max_applications: usize,
    pub(crate) max_delegations: usize,
}

impl Default for GeneratedSectorAffineConditionalRuleProviderLimits {
    fn default() -> Self {
        let mut application = GeneratedSectorAffineRuleApplicationLimits::default();
        application.max_owner_replays = 0;
        Self {
            application,
            max_installed_owners: 1_000_000,
            max_installed_sectors: 1_000_000,
            max_retained_outer_owner_bytes: portable_usize(256 * 1024 * 1024 * 1024),
            max_retained_owner_index_bytes: portable_usize(64 * 1024 * 1024 * 1024),
            max_temporary_owner_index_bytes: portable_usize(16 * 1024 * 1024 * 1024),
            max_peak_visible_build_bytes: portable_usize(320 * 1024 * 1024 * 1024),
            max_queries: 100_000_000,
            max_applications: 100_000_000,
            max_delegations: 100_000_000,
        }
    }
}

const fn portable_usize(value: u64) -> usize {
    if value > usize::MAX as u64 {
        usize::MAX
    } else {
        value as usize
    }
}

/// Exact immutable census of the owner capabilities retained at build time.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct GeneratedSectorAffineConditionalRuleProviderBuildStats {
    installed_owners: usize,
    installed_sectors: usize,
    retained_outer_owner_bytes: usize,
    retained_owner_index_bytes: usize,
    temporary_owner_index_bytes: usize,
    peak_visible_build_bytes: usize,
}

impl GeneratedSectorAffineConditionalRuleProviderBuildStats {
    pub(crate) const fn installed_owners(self) -> usize {
        self.installed_owners
    }

    pub(crate) const fn installed_sectors(self) -> usize {
        self.installed_sectors
    }

    pub(crate) const fn retained_outer_owner_bytes(self) -> usize {
        self.retained_outer_owner_bytes
    }

    pub(crate) const fn retained_owner_index_bytes(self) -> usize {
        self.retained_owner_index_bytes
    }

    pub(crate) const fn temporary_owner_index_bytes(self) -> usize {
        self.temporary_owner_index_bytes
    }

    pub(crate) const fn peak_visible_build_bytes(self) -> usize {
        self.peak_visible_build_bytes
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
struct GeneratedSectorAffineConditionalRuleProviderPreflight {
    installed_owners: usize,
    installed_sectors: usize,
    retained_outer_owner_bytes: usize,
    minimum_retained_owner_index_bytes: usize,
    minimum_sector_key_bytes: usize,
    temporary_owner_index_bytes: usize,
}

type InstalledAffineOwner = (
    SectorMask,
    Arc<GeneratedSectorAffineEffectiveCoverageCertificate>,
);

/// Exact successful-query routing census.
///
/// Mutations are committed only after a concrete decision (including a
/// successful inner delegation) has been produced.  Thus every committed
/// query is exactly one affine application or one delegation, and an owner or
/// inner error leaves this wrapper's counters unchanged.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct GeneratedSectorAffineConditionalRuleProviderStats {
    queries: usize,
    owner_queries: usize,
    applications: usize,
    delegations: usize,
    missing_sector_delegations: usize,
    covered_by_global_delegations: usize,
    residual_root_delegations: usize,
    exceptional_delegations: usize,
}

impl GeneratedSectorAffineConditionalRuleProviderStats {
    pub(crate) const fn queries(self) -> usize {
        self.queries
    }

    pub(crate) const fn owner_queries(self) -> usize {
        self.owner_queries
    }

    pub(crate) const fn applications(self) -> usize {
        self.applications
    }

    pub(crate) const fn delegations(self) -> usize {
        self.delegations
    }

    pub(crate) const fn missing_sector_delegations(self) -> usize {
        self.missing_sector_delegations
    }

    pub(crate) const fn covered_by_global_delegations(self) -> usize {
        self.covered_by_global_delegations
    }

    pub(crate) const fn residual_root_delegations(self) -> usize {
        self.residual_root_delegations
    }

    pub(crate) const fn exceptional_delegations(self) -> usize {
        self.exceptional_delegations
    }
}

/// One-owner-per-sector affine overlay over an arbitrary concrete provider.
///
/// The family and coefficient context are borrowed from the enclosing
/// provider stack.  Owner `Arc`s are retained as capabilities; no private
/// local rule payload is extracted into this table.
pub(crate) struct GeneratedSectorAffineConditionalRuleProvider<'family, Inner> {
    family: &'family IntegralFamily,
    context: &'family ParametricCoefficientContext,
    inner: Inner,
    index_arity: usize,
    owners: Vec<InstalledAffineOwner>,
    limits: GeneratedSectorAffineConditionalRuleProviderLimits,
    build_stats: GeneratedSectorAffineConditionalRuleProviderBuildStats,
    stats: GeneratedSectorAffineConditionalRuleProviderStats,
}

impl<Inner> fmt::Debug for GeneratedSectorAffineConditionalRuleProvider<'_, Inner> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("GeneratedSectorAffineConditionalRuleProvider")
            .field(
                "schema",
                &GENERATED_SECTOR_AFFINE_CONDITIONAL_RULE_PROVIDER_V1_SCHEMA,
            )
            .field("index_arity", &self.index_arity)
            .field("installed_sector_count", &self.owners.len())
            .field("limits", &self.limits)
            .field("build_stats", &self.build_stats)
            .field("stats", &self.stats)
            .field("owner_authorities", &"<redacted>")
            .field("inner", &"<redacted>")
            .finish()
    }
}

impl<'family, Inner> GeneratedSectorAffineConditionalRuleProvider<'family, Inner>
where
    Inner: ConcreteRuleProvider,
{
    pub(crate) const SCHEMA: &'static str =
        GENERATED_SECTOR_AFFINE_CONDITIONAL_RULE_PROVIDER_V1_SCHEMA;

    /// Replay-authenticate and retain at most one exact affine owner for each
    /// sector.  All borrowed scope and aggregate retention limits are checked
    /// before the first owner replay.
    pub(crate) fn try_new(
        family: &'family IntegralFamily,
        context: &'family ParametricCoefficientContext,
        owners: &[Arc<GeneratedSectorAffineEffectiveCoverageCertificate>],
        inner: Inner,
        limits: GeneratedSectorAffineConditionalRuleProviderLimits,
    ) -> Result<Self, GeneratedSectorAffineConditionalRuleProviderError<Inner::Error>> {
        catch_unwind(AssertUnwindSafe(|| {
            Self::try_new_inner(family, context, owners, inner, limits)
        }))
        .map_err(|_| GeneratedSectorAffineConditionalRuleProviderError::Panic)?
    }

    /// Preflight the complete borrowed owner set before an enclosing family
    /// provider clones any owner `Arc` into its runtime stack.
    pub(crate) fn preflight_owners(
        family: &IntegralFamily,
        context: &ParametricCoefficientContext,
        owners: &[Arc<GeneratedSectorAffineEffectiveCoverageCertificate>],
        limits: GeneratedSectorAffineConditionalRuleProviderLimits,
    ) -> Result<(), GeneratedSectorAffineConditionalRuleProviderError<Inner::Error>> {
        Self::preflight_owner_slice(family, context, owners, limits).map(|_| ())
    }

    fn try_new_inner(
        family: &'family IntegralFamily,
        context: &'family ParametricCoefficientContext,
        owners: &[Arc<GeneratedSectorAffineEffectiveCoverageCertificate>],
        inner: Inner,
        limits: GeneratedSectorAffineConditionalRuleProviderLimits,
    ) -> Result<Self, GeneratedSectorAffineConditionalRuleProviderError<Inner::Error>> {
        validate_family_context::<Inner::Error>(family, context)?;
        let index_arity = context.index_count();
        let inner_arity = inner.index_arity();
        if inner_arity != index_arity {
            return Err(
                GeneratedSectorAffineConditionalRuleProviderError::InnerProviderArityChanged {
                    expected: index_arity,
                    actual: inner_arity,
                },
            );
        }

        // This is the first operation that inspects the complete owner set.
        // It validates every scalar limit and scope from borrowed metadata,
        // and bounds/fallibly allocates the duplicate-detection scratch. No
        // owner capability or sector payload has been cloned yet.
        let preflight = Self::preflight_owner_slice(family, context, owners, limits)?;

        let mut retained: Vec<InstalledAffineOwner> = Vec::new();
        retained.try_reserve_exact(owners.len()).map_err(|_| {
            GeneratedSectorAffineConditionalRuleProviderError::AllocationFailure {
                resource: "generated affine retained owner index",
                requested: owners.len(),
            }
        })?;

        let entry_bytes = checked_mul::<Inner::Error>(
            "generated affine retained owner index bytes",
            retained.capacity(),
            std::mem::size_of::<InstalledAffineOwner>(),
        )?;
        let initial_index_bytes = checked_add(
            "generated affine retained owner index bytes",
            entry_bytes,
            preflight.minimum_sector_key_bytes,
        )?;
        check_limit::<Inner::Error>(
            "generated affine retained owner index bytes",
            initial_index_bytes,
            limits.max_retained_owner_index_bytes,
        )?;
        let initial_peak = checked_add(
            "generated affine peak visible build bytes",
            preflight.retained_outer_owner_bytes,
            preflight
                .temporary_owner_index_bytes
                .max(initial_index_bytes),
        )?;
        check_limit::<Inner::Error>(
            "generated affine peak visible build bytes",
            initial_peak,
            limits.max_peak_visible_build_bytes,
        )?;

        // Populate the complete immutable index before replay. Each sector
        // clone crosses a fallible allocation boundary and is immediately
        // charged at its observed capacity. The owner Arc clone occurs only
        // after all borrowed preflight checks have passed.
        for owner in owners {
            let sector =
                SectorMask::try_new(owner.source_queue().sector().active_bits().iter().copied())?;
            retained.push((sector, Arc::clone(owner)));
            let observed_index_bytes =
                retained_owner_index_bytes::<Inner::Error>(&retained, retained.capacity())?;
            check_limit::<Inner::Error>(
                "generated affine retained owner index bytes",
                observed_index_bytes,
                limits.max_retained_owner_index_bytes,
            )?;
            let observed_peak = checked_add(
                "generated affine peak visible build bytes",
                preflight.retained_outer_owner_bytes,
                preflight
                    .temporary_owner_index_bytes
                    .max(observed_index_bytes),
            )?;
            check_limit::<Inner::Error>(
                "generated affine peak visible build bytes",
                observed_peak,
                limits.max_peak_visible_build_bytes,
            )?;
        }
        retained.sort_unstable_by(|left, right| left.0.cmp(&right.0));

        let retained_owner_index_bytes =
            retained_owner_index_bytes::<Inner::Error>(&retained, retained.capacity())?;
        let peak_visible_build_bytes = checked_add(
            "generated affine peak visible build bytes",
            preflight.retained_outer_owner_bytes,
            preflight
                .temporary_owner_index_bytes
                .max(retained_owner_index_bytes),
        )?;
        let build_stats = GeneratedSectorAffineConditionalRuleProviderBuildStats {
            installed_owners: preflight.installed_owners,
            installed_sectors: preflight.installed_sectors,
            retained_outer_owner_bytes: preflight.retained_outer_owner_bytes,
            retained_owner_index_bytes,
            temporary_owner_index_bytes: preflight.temporary_owner_index_bytes,
            peak_visible_build_bytes,
        };

        // Replay only after the complete owner set and its aggregate retained
        // surface have passed preflight and allocation. No partial provider is
        // published and no allocation remains after replay succeeds.
        for (sector, owner) in &retained {
            owner.replay(family, context).map_err(|error| {
                GeneratedSectorAffineConditionalRuleProviderError::OwnerReplay {
                    sector: sector.clone(),
                    error,
                }
            })?;
        }

        debug_assert_eq!(retained.len(), build_stats.installed_owners);
        Ok(Self {
            family,
            context,
            inner,
            index_arity,
            owners: retained,
            limits,
            build_stats,
            stats: GeneratedSectorAffineConditionalRuleProviderStats::default(),
        })
    }

    fn preflight_owner_slice(
        family: &IntegralFamily,
        context: &ParametricCoefficientContext,
        owners: &[Arc<GeneratedSectorAffineEffectiveCoverageCertificate>],
        limits: GeneratedSectorAffineConditionalRuleProviderLimits,
    ) -> Result<
        GeneratedSectorAffineConditionalRuleProviderPreflight,
        GeneratedSectorAffineConditionalRuleProviderError<Inner::Error>,
    > {
        validate_family_context::<Inner::Error>(family, context)?;
        let mut preflight = GeneratedSectorAffineConditionalRuleProviderPreflight::default();

        // Scalar and borrowed-scope checks precede every proportional
        // allocation, Arc clone, owner replay, and sector-payload clone.
        for owner in owners {
            preflight.installed_owners = bounded_add(
                "generated affine installed owners",
                preflight.installed_owners,
                1,
                limits.max_installed_owners,
            )?;
            validate_owner_scope::<Inner::Error>(family, context, owner.as_ref())?;
            preflight.installed_sectors = bounded_add(
                "generated affine installed sectors",
                preflight.installed_sectors,
                1,
                limits.max_installed_sectors,
            )?;
            preflight.retained_outer_owner_bytes = bounded_add(
                "generated affine retained outer owner bytes",
                preflight.retained_outer_owner_bytes,
                owner.stats().outer_retained_bytes(),
                limits.max_retained_outer_owner_bytes,
            )?;
        }

        let entry_bytes = checked_mul::<Inner::Error>(
            "generated affine retained owner index bytes",
            owners.len(),
            std::mem::size_of::<InstalledAffineOwner>(),
        )?;
        let mut sector_bytes = 0usize;
        for owner in owners {
            sector_bytes = checked_add(
                "generated affine retained owner index bytes",
                sector_bytes,
                checked_mul::<Inner::Error>(
                    "generated affine retained owner index bytes",
                    owner.source_queue().sector().arity(),
                    std::mem::size_of::<bool>(),
                )?,
            )?;
        }
        preflight.minimum_retained_owner_index_bytes = checked_add(
            "generated affine retained owner index bytes",
            entry_bytes,
            sector_bytes,
        )?;
        preflight.minimum_sector_key_bytes = sector_bytes;
        check_limit::<Inner::Error>(
            "generated affine retained owner index bytes",
            preflight.minimum_retained_owner_index_bytes,
            limits.max_retained_owner_index_bytes,
        )?;

        let minimum_temporary_bytes = checked_mul::<Inner::Error>(
            "generated affine temporary owner index bytes",
            owners.len(),
            std::mem::size_of::<&SectorMask>(),
        )?;
        check_limit::<Inner::Error>(
            "generated affine temporary owner index bytes",
            minimum_temporary_bytes,
            limits.max_temporary_owner_index_bytes,
        )?;
        let minimum_peak = checked_add(
            "generated affine peak visible build bytes",
            preflight.retained_outer_owner_bytes,
            minimum_temporary_bytes.max(preflight.minimum_retained_owner_index_bytes),
        )?;
        check_limit::<Inner::Error>(
            "generated affine peak visible build bytes",
            minimum_peak,
            limits.max_peak_visible_build_bytes,
        )?;

        let mut sectors: Vec<&SectorMask> = Vec::new();
        sectors.try_reserve_exact(owners.len()).map_err(|_| {
            GeneratedSectorAffineConditionalRuleProviderError::AllocationFailure {
                resource: "generated affine temporary owner index",
                requested: owners.len(),
            }
        })?;
        preflight.temporary_owner_index_bytes = checked_mul::<Inner::Error>(
            "generated affine temporary owner index bytes",
            sectors.capacity(),
            std::mem::size_of::<&SectorMask>(),
        )?;
        check_limit::<Inner::Error>(
            "generated affine temporary owner index bytes",
            preflight.temporary_owner_index_bytes,
            limits.max_temporary_owner_index_bytes,
        )?;
        let observed_scratch_peak = checked_add(
            "generated affine peak visible build bytes",
            preflight.retained_outer_owner_bytes,
            preflight
                .temporary_owner_index_bytes
                .max(preflight.minimum_retained_owner_index_bytes),
        )?;
        check_limit::<Inner::Error>(
            "generated affine peak visible build bytes",
            observed_scratch_peak,
            limits.max_peak_visible_build_bytes,
        )?;

        sectors.extend(owners.iter().map(|owner| owner.source_queue().sector()));
        sectors.sort_unstable();
        if let Some(duplicate) = sectors
            .windows(2)
            .find_map(|pair| (pair[0] == pair[1]).then_some(pair[0]))
        {
            let sector = SectorMask::try_new(duplicate.active_bits().iter().copied())?;
            return Err(
                GeneratedSectorAffineConditionalRuleProviderError::DuplicateSector { sector },
            );
        }

        Ok(preflight)
    }

    pub(crate) const fn family(&self) -> &IntegralFamily {
        self.family
    }

    pub(crate) const fn context(&self) -> &ParametricCoefficientContext {
        self.context
    }

    pub(crate) const fn inner(&self) -> &Inner {
        &self.inner
    }

    pub(crate) fn inner_mut(&mut self) -> &mut Inner {
        &mut self.inner
    }

    pub(crate) fn into_inner(self) -> Inner {
        self.inner
    }

    pub(crate) const fn limits(&self) -> GeneratedSectorAffineConditionalRuleProviderLimits {
        self.limits
    }

    pub(crate) const fn build_stats(
        &self,
    ) -> GeneratedSectorAffineConditionalRuleProviderBuildStats {
        self.build_stats
    }

    pub(crate) const fn stats(&self) -> GeneratedSectorAffineConditionalRuleProviderStats {
        self.stats
    }

    pub(crate) fn owners(
        &self,
    ) -> impl ExactSizeIterator<Item = &Arc<GeneratedSectorAffineEffectiveCoverageCertificate>>
    {
        self.owners.iter().map(|(_, owner)| owner)
    }

    /// Replay every retained owner capability and the provider's immutable
    /// installation census. Runtime query counters and the inner provider's
    /// own replay boundary are deliberately outside this check.
    pub(crate) fn replay(
        &self,
    ) -> Result<(), GeneratedSectorAffineConditionalRuleProviderError<Inner::Error>> {
        catch_unwind(AssertUnwindSafe(|| self.replay_inner()))
            .map_err(|_| GeneratedSectorAffineConditionalRuleProviderError::Panic)?
    }

    fn replay_inner(
        &self,
    ) -> Result<(), GeneratedSectorAffineConditionalRuleProviderError<Inner::Error>> {
        validate_family_context::<Inner::Error>(self.family, self.context)?;
        self.validate_inner_arity()?;
        let mut replayed = GeneratedSectorAffineConditionalRuleProviderBuildStats::default();
        for (sector, owner) in &self.owners {
            replayed.installed_owners = bounded_add(
                "generated affine installed owners",
                replayed.installed_owners,
                1,
                self.limits.max_installed_owners,
            )?;
            replayed.installed_sectors = bounded_add(
                "generated affine installed sectors",
                replayed.installed_sectors,
                1,
                self.limits.max_installed_sectors,
            )?;
            validate_owner_scope::<Inner::Error>(self.family, self.context, owner.as_ref())?;
            if owner.source_queue().sector() != sector {
                return Err(
                    GeneratedSectorAffineConditionalRuleProviderError::ReplayMismatch {
                        detail: "retained owner is stored under a different sector",
                    },
                );
            }
            replayed.retained_outer_owner_bytes = bounded_add(
                "generated affine retained outer owner bytes",
                replayed.retained_outer_owner_bytes,
                owner.stats().outer_retained_bytes(),
                self.limits.max_retained_outer_owner_bytes,
            )?;
            owner.replay(self.family, self.context).map_err(|error| {
                GeneratedSectorAffineConditionalRuleProviderError::OwnerReplay {
                    sector: sector.clone(),
                    error,
                }
            })?;
        }
        replayed.retained_owner_index_bytes =
            retained_owner_index_bytes::<Inner::Error>(&self.owners, self.owners.capacity())?;
        check_limit::<Inner::Error>(
            "generated affine retained owner index bytes",
            replayed.retained_owner_index_bytes,
            self.limits.max_retained_owner_index_bytes,
        )?;
        // Temporary index storage is a historical build census. The retained
        // provider exposes no mutable path that can alter it; replay combines
        // it with freshly recomputed retained authority and index storage.
        replayed.temporary_owner_index_bytes = self.build_stats.temporary_owner_index_bytes;
        check_limit::<Inner::Error>(
            "generated affine temporary owner index bytes",
            replayed.temporary_owner_index_bytes,
            self.limits.max_temporary_owner_index_bytes,
        )?;
        replayed.peak_visible_build_bytes = checked_add(
            "generated affine peak visible build bytes",
            replayed.retained_outer_owner_bytes,
            replayed
                .temporary_owner_index_bytes
                .max(replayed.retained_owner_index_bytes),
        )?;
        check_limit::<Inner::Error>(
            "generated affine peak visible build bytes",
            replayed.peak_visible_build_bytes,
            self.limits.max_peak_visible_build_bytes,
        )?;
        if replayed != self.build_stats {
            return Err(
                GeneratedSectorAffineConditionalRuleProviderError::ReplayMismatch {
                    detail: "retained owner installation census differs",
                },
            );
        }
        Ok(())
    }

    fn validate_inner_arity(
        &self,
    ) -> Result<(), GeneratedSectorAffineConditionalRuleProviderError<Inner::Error>> {
        let actual = self.inner.index_arity();
        if actual == self.index_arity {
            Ok(())
        } else {
            Err(
                GeneratedSectorAffineConditionalRuleProviderError::InnerProviderArityChanged {
                    expected: self.index_arity,
                    actual,
                },
            )
        }
    }

    fn delegate(
        &mut self,
        integral: &ConcreteIntegralKey,
        next_stats: GeneratedSectorAffineConditionalRuleProviderStats,
    ) -> Result<ConcreteRuleDecision, GeneratedSectorAffineConditionalRuleProviderError<Inner::Error>>
    {
        let decision = self
            .inner
            .decision_for(integral)
            .map_err(GeneratedSectorAffineConditionalRuleProviderError::Inner)?;
        self.stats = next_stats;
        Ok(decision)
    }
}

impl<Inner> ConcreteRuleProvider for GeneratedSectorAffineConditionalRuleProvider<'_, Inner>
where
    Inner: ConcreteRuleProvider,
{
    type Error = GeneratedSectorAffineConditionalRuleProviderError<Inner::Error>;

    fn index_arity(&self) -> usize {
        self.index_arity
    }

    fn decision_for(
        &mut self,
        integral: &ConcreteIntegralKey,
    ) -> Result<ConcreteRuleDecision, Self::Error> {
        self.validate_inner_arity()?;
        if integral.powers().len() != self.index_arity {
            return Err(
                GeneratedSectorAffineConditionalRuleProviderError::WrongArity {
                    expected: self.index_arity,
                    actual: integral.powers().len(),
                },
            );
        }

        let mut next_stats = self.stats;
        next_stats.queries = bounded_add(
            "generated affine provider queries",
            next_stats.queries,
            1,
            self.limits.max_queries,
        )?;
        let sector = SectorMask::try_from_indices(integral.powers())?;
        let Ok(owner_position) = self
            .owners
            .binary_search_by(|(candidate, _)| candidate.cmp(&sector))
        else {
            next_stats.delegations = bounded_add(
                "generated affine provider delegations",
                next_stats.delegations,
                1,
                self.limits.max_delegations,
            )?;
            next_stats.missing_sector_delegations = checked_add(
                "generated affine missing-sector delegations",
                next_stats.missing_sector_delegations,
                1,
            )?;
            return self.delegate(integral, next_stats);
        };
        let owner = &self.owners[owner_position].1;

        next_stats.owner_queries = checked_add(
            "generated affine owner queries",
            next_stats.owner_queries,
            1,
        )?;
        // Admit the only path capable of specializing a sealed owner before
        // invoking the specialization engine. The admission is committed
        // only when the outcome is a reduction; dispositions still count as
        // delegations. Conservatively, an exhausted application budget
        // rejects any installed-owner query because its outcome is not known
        // without performing the bounded specialization.
        let admitted_applications = bounded_add(
            "generated affine provider applications",
            next_stats.applications,
            1,
            self.limits.max_applications,
        )?;
        // The owner was replayed during installation.  Calling the inner
        // provider first would make global `Unsupported` terminal and would
        // bypass this affine overlay, so owner classification is necessarily
        // the first semantic query for an installed sector.
        let application = owner
            .concrete_application_for_indices_from_replayed_owner(
                self.family,
                self.context,
                integral.powers(),
                self.limits.application,
            )
            .map_err(|error| {
                GeneratedSectorAffineConditionalRuleProviderError::OwnerApplication {
                    sector: sector.clone(),
                    error,
                }
            })?;

        match application.into_outcome() {
            GeneratedSectorAffineConcretePointOutcome::Reduction(reduction) => {
                next_stats.applications = admitted_applications;
                self.stats = next_stats;
                Ok(ConcreteRuleDecision::ConditionalReduction(reduction))
            }
            GeneratedSectorAffineConcretePointOutcome::Disposition(disposition) => {
                match disposition {
                    GeneratedSectorAffinePointDisposition::OutsideSector => {
                        return Err(
                            GeneratedSectorAffineConditionalRuleProviderError::OwnerOutsideRoutedSector {
                                sector,
                            },
                        );
                    }
                    GeneratedSectorAffinePointDisposition::Rule(_) => {
                        return Err(
                            GeneratedSectorAffineConditionalRuleProviderError::OwnerReturnedUnappliedRule {
                                sector,
                            },
                        );
                    }
                    GeneratedSectorAffinePointDisposition::CoveredByGlobal { .. } => {
                        next_stats.covered_by_global_delegations = checked_add(
                            "generated affine covered-by-global delegations",
                            next_stats.covered_by_global_delegations,
                            1,
                        )?;
                    }
                    GeneratedSectorAffinePointDisposition::ResidualRoot(_) => {
                        next_stats.residual_root_delegations = checked_add(
                            "generated affine residual-root delegations",
                            next_stats.residual_root_delegations,
                            1,
                        )?;
                    }
                    GeneratedSectorAffinePointDisposition::Exceptional(_) => {
                        next_stats.exceptional_delegations = checked_add(
                            "generated affine exceptional delegations",
                            next_stats.exceptional_delegations,
                            1,
                        )?;
                    }
                }
                next_stats.delegations = bounded_add(
                    "generated affine provider delegations",
                    next_stats.delegations,
                    1,
                    self.limits.max_delegations,
                )?;
                self.delegate(integral, next_stats)
            }
        }
    }
}

fn validate_family_context<InnerError>(
    family: &IntegralFamily,
    context: &ParametricCoefficientContext,
) -> Result<(), GeneratedSectorAffineConditionalRuleProviderError<InnerError>>
where
    InnerError: std::error::Error + Send + Sync + 'static,
{
    if context.index_count() != family.denominator_count() {
        return Err(
            GeneratedSectorAffineConditionalRuleProviderError::WrongArity {
                expected: family.denominator_count(),
                actual: context.index_count(),
            },
        );
    }
    if !context
        .base()
        .has_same_variable_map(family.coefficient_context())
    {
        return Err(GeneratedSectorAffineConditionalRuleProviderError::WrongContext);
    }
    Ok(())
}

fn validate_owner_scope<InnerError>(
    family: &IntegralFamily,
    context: &ParametricCoefficientContext,
    owner: &GeneratedSectorAffineEffectiveCoverageCertificate,
) -> Result<(), GeneratedSectorAffineConditionalRuleProviderError<InnerError>>
where
    InnerError: std::error::Error + Send + Sync + 'static,
{
    let queue = owner.source_queue();
    let inventory = owner.inventory();
    if queue.family_fingerprint() != family.fingerprint_ref()
        || inventory.family_fingerprint() != family.fingerprint_ref()
    {
        return Err(
            GeneratedSectorAffineConditionalRuleProviderError::ForeignOwner {
                sector: queue.sector().clone(),
                component: "family",
            },
        );
    }
    if queue.context_fingerprint() != context.fingerprint()
        || inventory.context_fingerprint() != context.fingerprint()
    {
        return Err(
            GeneratedSectorAffineConditionalRuleProviderError::ForeignOwner {
                sector: queue.sector().clone(),
                component: "coefficient context",
            },
        );
    }
    if queue.sector().arity() != context.index_count() {
        return Err(
            GeneratedSectorAffineConditionalRuleProviderError::ForeignOwner {
                sector: queue.sector().clone(),
                component: "sector arity",
            },
        );
    }
    Ok(())
}

fn bounded_add<InnerError>(
    resource: &'static str,
    current: usize,
    addend: usize,
    limit: usize,
) -> Result<usize, GeneratedSectorAffineConditionalRuleProviderError<InnerError>>
where
    InnerError: std::error::Error + Send + Sync + 'static,
{
    let requested = checked_add(resource, current, addend)?;
    if requested > limit {
        Err(
            GeneratedSectorAffineConditionalRuleProviderError::ResourceLimit {
                resource,
                requested,
                limit,
            },
        )
    } else {
        Ok(requested)
    }
}

fn check_limit<InnerError>(
    resource: &'static str,
    requested: usize,
    limit: usize,
) -> Result<(), GeneratedSectorAffineConditionalRuleProviderError<InnerError>>
where
    InnerError: std::error::Error + Send + Sync + 'static,
{
    if requested > limit {
        Err(
            GeneratedSectorAffineConditionalRuleProviderError::ResourceLimit {
                resource,
                requested,
                limit,
            },
        )
    } else {
        Ok(())
    }
}

fn retained_owner_index_bytes<InnerError>(
    owners: &[InstalledAffineOwner],
    owner_capacity: usize,
) -> Result<usize, GeneratedSectorAffineConditionalRuleProviderError<InnerError>>
where
    InnerError: std::error::Error + Send + Sync + 'static,
{
    let mut retained = checked_mul::<InnerError>(
        "generated affine retained owner index bytes",
        owner_capacity,
        std::mem::size_of::<InstalledAffineOwner>(),
    )?;
    for (sector, _) in owners {
        let sector_bytes = sector.owned_retained_byte_bound().ok_or(
            GeneratedSectorAffineConditionalRuleProviderError::ResourceCountOverflow {
                resource: "generated affine retained owner index bytes",
            },
        )?;
        retained = checked_add(
            "generated affine retained owner index bytes",
            retained,
            sector_bytes,
        )?;
    }
    Ok(retained)
}

fn checked_mul<InnerError>(
    resource: &'static str,
    left: usize,
    right: usize,
) -> Result<usize, GeneratedSectorAffineConditionalRuleProviderError<InnerError>>
where
    InnerError: std::error::Error + Send + Sync + 'static,
{
    left.checked_mul(right).ok_or(
        GeneratedSectorAffineConditionalRuleProviderError::ResourceCountOverflow { resource },
    )
}

fn checked_add<InnerError>(
    resource: &'static str,
    current: usize,
    addend: usize,
) -> Result<usize, GeneratedSectorAffineConditionalRuleProviderError<InnerError>>
where
    InnerError: std::error::Error + Send + Sync + 'static,
{
    current.checked_add(addend).ok_or(
        GeneratedSectorAffineConditionalRuleProviderError::ResourceCountOverflow { resource },
    )
}

pub(crate) enum GeneratedSectorAffineConditionalRuleProviderError<InnerError>
where
    InnerError: std::error::Error + Send + Sync + 'static,
{
    WrongContext,
    WrongArity {
        expected: usize,
        actual: usize,
    },
    InnerProviderArityChanged {
        expected: usize,
        actual: usize,
    },
    DuplicateSector {
        sector: SectorMask,
    },
    ForeignOwner {
        sector: SectorMask,
        component: &'static str,
    },
    OwnerReplay {
        sector: SectorMask,
        error: GeneratedSectorAffineEffectiveCoverageError,
    },
    OwnerApplication {
        sector: SectorMask,
        error: GeneratedSectorAffineRuleApplicationError,
    },
    OwnerOutsideRoutedSector {
        sector: SectorMask,
    },
    OwnerReturnedUnappliedRule {
        sector: SectorMask,
    },
    ReplayMismatch {
        detail: &'static str,
    },
    ResourceLimit {
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
    Sector(SectorFoundationError),
    Inner(InnerError),
    Panic,
}

impl<InnerError> fmt::Debug for GeneratedSectorAffineConditionalRuleProviderError<InnerError>
where
    InnerError: std::error::Error + Send + Sync + 'static,
{
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::WrongContext => formatter.write_str("WrongContext"),
            Self::WrongArity { expected, actual } => formatter
                .debug_struct("WrongArity")
                .field("expected", expected)
                .field("actual", actual)
                .finish(),
            Self::InnerProviderArityChanged { expected, actual } => formatter
                .debug_struct("InnerProviderArityChanged")
                .field("expected", expected)
                .field("actual", actual)
                .finish(),
            Self::DuplicateSector { sector } => formatter
                .debug_struct("DuplicateSector")
                .field("sector", sector)
                .finish(),
            Self::ForeignOwner { sector, component } => formatter
                .debug_struct("ForeignOwner")
                .field("sector", sector)
                .field("component", component)
                .field("owner", &"<redacted>")
                .finish(),
            Self::OwnerReplay { sector, .. } => formatter
                .debug_struct("OwnerReplay")
                .field("sector", sector)
                .field("owner", &"<redacted>")
                .finish(),
            Self::OwnerApplication { sector, .. } => formatter
                .debug_struct("OwnerApplication")
                .field("sector", sector)
                .field("owner", &"<redacted>")
                .finish(),
            Self::OwnerOutsideRoutedSector { sector } => formatter
                .debug_struct("OwnerOutsideRoutedSector")
                .field("sector", sector)
                .finish(),
            Self::OwnerReturnedUnappliedRule { sector } => formatter
                .debug_struct("OwnerReturnedUnappliedRule")
                .field("sector", sector)
                .finish(),
            Self::ReplayMismatch { detail } => formatter
                .debug_struct("ReplayMismatch")
                .field("detail", detail)
                .field("owner", &"<redacted>")
                .finish(),
            Self::ResourceLimit {
                resource,
                requested,
                limit,
            } => formatter
                .debug_struct("ResourceLimit")
                .field("resource", resource)
                .field("requested", requested)
                .field("limit", limit)
                .finish(),
            Self::ResourceCountOverflow { resource } => formatter
                .debug_struct("ResourceCountOverflow")
                .field("resource", resource)
                .finish(),
            Self::AllocationFailure {
                resource,
                requested,
            } => formatter
                .debug_struct("AllocationFailure")
                .field("resource", resource)
                .field("requested", requested)
                .finish(),
            Self::Sector(error) => formatter.debug_tuple("Sector").field(error).finish(),
            Self::Inner(_) => formatter.write_str("Inner(<redacted>)"),
            Self::Panic => formatter.write_str("Panic(<redacted>)"),
        }
    }
}

impl<InnerError> fmt::Display for GeneratedSectorAffineConditionalRuleProviderError<InnerError>
where
    InnerError: std::error::Error + Send + Sync + 'static,
{
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::WrongContext => formatter.write_str(
                "generated affine provider coefficient context does not match its family",
            ),
            Self::WrongArity { expected, actual } => write!(
                formatter,
                "generated affine provider expected arity {expected}, got {actual}"
            ),
            Self::InnerProviderArityChanged { expected, actual } => write!(
                formatter,
                "generated affine inner provider expected arity {expected}, got {actual}"
            ),
            Self::DuplicateSector { sector } => {
                write!(
                    formatter,
                    "duplicate generated affine owner for sector {sector:?}"
                )
            }
            Self::ForeignOwner { sector, component } => write!(
                formatter,
                "generated affine owner for sector {sector:?} has foreign {component} authority"
            ),
            Self::OwnerReplay { sector, .. } => write!(
                formatter,
                "generated affine owner replay failed for sector {sector:?}"
            ),
            Self::OwnerApplication { sector, error } => write!(
                formatter,
                "generated affine owner application failed for sector {sector:?}: {error}"
            ),
            Self::OwnerOutsideRoutedSector { sector } => write!(
                formatter,
                "generated affine owner returned OutsideSector after routing sector {sector:?}"
            ),
            Self::OwnerReturnedUnappliedRule { sector } => write!(
                formatter,
                "generated affine owner returned an unapplied rule for sector {sector:?}"
            ),
            Self::ReplayMismatch { detail } => {
                write!(
                    formatter,
                    "generated affine provider replay mismatch: {detail}"
                )
            }
            Self::ResourceLimit {
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
                "failed to allocate {requested} entries for {resource}"
            ),
            Self::Sector(error) => error.fmt(formatter),
            Self::Inner(_) => formatter.write_str("generated affine inner provider failed"),
            Self::Panic => formatter
                .write_str("generated affine provider panicked while handling sealed authority"),
        }
    }
}

impl<InnerError> std::error::Error for GeneratedSectorAffineConditionalRuleProviderError<InnerError>
where
    InnerError: std::error::Error + Send + Sync + 'static,
{
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::OwnerReplay { error, .. } => Some(error),
            Self::OwnerApplication { error, .. } => Some(error),
            Self::Sector(error) => Some(error),
            Self::Inner(error) => Some(error),
            _ => None,
        }
    }
}

impl<InnerError> From<SectorFoundationError>
    for GeneratedSectorAffineConditionalRuleProviderError<InnerError>
where
    InnerError: std::error::Error + Send + Sync + 'static,
{
    fn from(value: SectorFoundationError) -> Self {
        Self::Sector(value)
    }
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::{AtomicUsize, Ordering};

    use super::*;
    use crate::generated_sector_affine_effective_coverage::{
        GeneratedSectorAffineEffectiveCoverageCompiler,
        GeneratedSectorAffineEffectiveCoverageConfig, GeneratedSectorAffineEffectiveCoverageLimits,
    };
    use crate::{
        AffineDenominator, CoefficientContext, ConcreteTerminalStatus,
        GeneratedResidualAffineCaseInventoryCompiler, GeneratedResidualAffineCaseInventoryLimits,
        GeneratedSectorDiscoveryCompiler, GeneratedSectorDiscoveryLimits,
        GeneratedSectorLiveLeafQueueCompiler, GeneratedSectorLiveLeafQueueLimits,
        IntegralOrderingPolicy, ParametricIbpGenerator,
    };

    fn equal_mass_two_loop_family(name: &str) -> IntegralFamily {
        let coefficients = CoefficientContext::new(["d", "m2"]);
        let zero = coefficients.zero();
        let one = coefficients.one();
        let minus_m2 = coefficients.parse("-m2").unwrap();
        IntegralFamily::new(
            name,
            vec!["k1".into(), "k2".into()],
            Vec::new(),
            coefficients.clone(),
            coefficients.parameter("d").unwrap(),
            vec![
                AffineDenominator::new(
                    minus_m2.clone(),
                    vec![one.clone(), zero.clone(), zero.clone()],
                ),
                AffineDenominator::new(
                    minus_m2.clone(),
                    vec![zero.clone(), zero.clone(), one.clone()],
                ),
                AffineDenominator::new(minus_m2, vec![one.clone(), coefficients.integer(2), one]),
            ],
            Vec::new(),
            vec![zero.clone(), zero.clone(), zero],
        )
        .unwrap()
    }

    fn owner_fixture(
        bits: &str,
        name: &str,
    ) -> (
        IntegralFamily,
        ParametricCoefficientContext,
        GeneratedSectorAffineEffectiveCoverageCertificate,
    ) {
        let family = equal_mass_two_loop_family(name);
        let context = ParametricIbpGenerator::try_new(&family)
            .unwrap()
            .context()
            .clone();
        let mut discovery_limits = GeneratedSectorDiscoveryLimits::default();
        discovery_limits.adaptive.max_search_depth = 0;
        let discovery = GeneratedSectorDiscoveryCompiler::compile(
            &family,
            &context,
            SectorMask::try_from_bit_string(bits).unwrap(),
            IntegralOrderingPolicy::RustRedUnshiftedV1,
            discovery_limits,
        )
        .unwrap();
        let mut queue_limits = GeneratedSectorLiveLeafQueueLimits::default();
        queue_limits.translation_radius = 0;
        queue_limits.max_translation_points = 1;
        let queue = Arc::new(
            GeneratedSectorLiveLeafQueueCompiler::compile(
                &family,
                &context,
                &discovery,
                queue_limits,
            )
            .unwrap(),
        );
        let inventory = Arc::new(
            GeneratedResidualAffineCaseInventoryCompiler::compile(
                &family,
                &context,
                queue,
                GeneratedResidualAffineCaseInventoryLimits::default(),
            )
            .unwrap(),
        );
        let owner = GeneratedSectorAffineEffectiveCoverageCompiler::compile(
            &family,
            &context,
            inventory,
            GeneratedSectorAffineEffectiveCoverageConfig::new(0),
            GeneratedSectorAffineEffectiveCoverageLimits::default(),
        )
        .unwrap();
        (family, context, owner)
    }

    #[derive(Debug)]
    struct TestInnerError;

    impl fmt::Display for TestInnerError {
        fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
            formatter.write_str("test inner failed")
        }
    }

    impl std::error::Error for TestInnerError {}

    struct CountingInner {
        arity: usize,
        calls: Arc<AtomicUsize>,
        fail: bool,
    }

    impl CountingInner {
        fn new(arity: usize, calls: Arc<AtomicUsize>) -> Self {
            Self {
                arity,
                calls,
                fail: false,
            }
        }

        fn failing(arity: usize, calls: Arc<AtomicUsize>) -> Self {
            Self {
                arity,
                calls,
                fail: true,
            }
        }
    }

    impl ConcreteRuleProvider for CountingInner {
        type Error = TestInnerError;

        fn index_arity(&self) -> usize {
            self.arity
        }

        fn decision_for(
            &mut self,
            _integral: &ConcreteIntegralKey,
        ) -> Result<ConcreteRuleDecision, Self::Error> {
            self.calls.fetch_add(1, Ordering::SeqCst);
            if self.fail {
                Err(TestInnerError)
            } else {
                Ok(ConcreteRuleDecision::Terminal(
                    ConcreteTerminalStatus::Uncovered,
                ))
            }
        }
    }

    fn provider_limits_for_one_owner(
        owner: &GeneratedSectorAffineEffectiveCoverageCertificate,
    ) -> GeneratedSectorAffineConditionalRuleProviderLimits {
        let mut limits = GeneratedSectorAffineConditionalRuleProviderLimits::default();
        limits.max_installed_owners = 1;
        limits.max_installed_sectors = 1;
        limits.max_retained_outer_owner_bytes = owner.stats().outer_retained_bytes();
        limits
    }

    #[test]
    fn schema_and_default_installed_application_replay_budget_are_stable() {
        assert_eq!(
            GENERATED_SECTOR_AFFINE_CONDITIONAL_RULE_PROVIDER_V1_SCHEMA,
            "rustred-generated-sector-affine-conditional-rule-provider-v1"
        );
        assert_eq!(
            GeneratedSectorAffineConditionalRuleProviderLimits::default()
                .application
                .max_owner_replays,
            0
        );
    }

    #[test]
    fn sealed_rule_routes_before_inner_and_replays_after_provider_drop() {
        let (family, context, owner) = owner_fixture("001", "affine-provider-generated-rule-route");
        let owner = Arc::new(owner);
        let calls = Arc::new(AtomicUsize::new(0));
        let mut limits = provider_limits_for_one_owner(owner.as_ref());
        limits.max_queries = 1;
        limits.max_applications = 1;
        limits.max_delegations = 0;
        let mut provider = GeneratedSectorAffineConditionalRuleProvider::try_new(
            &family,
            &context,
            std::slice::from_ref(&owner),
            CountingInner::new(3, Arc::clone(&calls)),
            limits,
        )
        .unwrap();
        assert_eq!(provider.build_stats().installed_owners(), 1);
        assert_eq!(provider.build_stats().installed_sectors(), 1);
        assert_eq!(
            provider.build_stats().retained_outer_owner_bytes(),
            owner.stats().outer_retained_bytes()
        );
        let source = ConcreteIntegralKey::try_new([-4, -4, 2]).unwrap();
        let ConcreteRuleDecision::ConditionalReduction(reduction) =
            provider.decision_for(&source).unwrap()
        else {
            panic!("the sealed affine source must yield a conditional reduction")
        };
        assert_eq!(calls.load(Ordering::SeqCst), 0);
        assert_eq!(provider.stats().queries(), 1);
        assert_eq!(provider.stats().owner_queries(), 1);
        assert_eq!(provider.stats().applications(), 1);
        assert_eq!(provider.stats().delegations(), 0);
        assert_eq!(reduction.source(), &source);
        assert!(reduction.coordinate_rule().is_none());
        provider.replay().unwrap();
        assert_eq!(provider.stats().queries(), 1);
        let debug = format!("{provider:?}");
        assert!(debug.contains("<redacted>"));
        assert!(!debug.contains("split_recentered_relation"));

        drop(provider);
        assert_eq!(limits.application.max_owner_replays, 0);
        reduction.replay(&family, &context).unwrap();
    }

    #[test]
    fn exceptional_and_missing_sector_paths_delegate_exactly_once() {
        let (family, context, owner) =
            owner_fixture("001", "affine-provider-generated-delegations");
        let owner = Arc::new(owner);

        let exceptional_calls = Arc::new(AtomicUsize::new(0));
        let mut exceptional = GeneratedSectorAffineConditionalRuleProvider::try_new(
            &family,
            &context,
            std::slice::from_ref(&owner),
            CountingInner::new(3, Arc::clone(&exceptional_calls)),
            provider_limits_for_one_owner(owner.as_ref()),
        )
        .unwrap();
        let exceptional_source = ConcreteIntegralKey::try_new([-4, -4, 1]).unwrap();
        assert!(matches!(
            exceptional.decision_for(&exceptional_source).unwrap(),
            ConcreteRuleDecision::Terminal(ConcreteTerminalStatus::Uncovered)
        ));
        assert_eq!(exceptional_calls.load(Ordering::SeqCst), 1);
        assert_eq!(exceptional.stats().queries(), 1);
        assert_eq!(exceptional.stats().owner_queries(), 1);
        assert_eq!(exceptional.stats().delegations(), 1);
        assert_eq!(exceptional.stats().exceptional_delegations(), 1);

        let missing_calls = Arc::new(AtomicUsize::new(0));
        let mut missing = GeneratedSectorAffineConditionalRuleProvider::try_new(
            &family,
            &context,
            std::slice::from_ref(&owner),
            CountingInner::new(3, Arc::clone(&missing_calls)),
            provider_limits_for_one_owner(owner.as_ref()),
        )
        .unwrap();
        let missing_source = ConcreteIntegralKey::try_new([1, 1, 1]).unwrap();
        assert!(matches!(
            missing.decision_for(&missing_source).unwrap(),
            ConcreteRuleDecision::Terminal(ConcreteTerminalStatus::Uncovered)
        ));
        assert_eq!(missing_calls.load(Ordering::SeqCst), 1);
        assert_eq!(missing.stats().queries(), 1);
        assert_eq!(missing.stats().owner_queries(), 0);
        assert_eq!(missing.stats().delegations(), 1);
        assert_eq!(missing.stats().missing_sector_delegations(), 1);
    }

    #[test]
    fn duplicate_foreign_and_corrupt_owner_authorities_are_rejected() {
        let (family, context, owner) =
            owner_fixture("001", "affine-provider-generated-owner-validation");
        let owner = Arc::new(owner);
        let calls = Arc::new(AtomicUsize::new(0));
        let duplicate_owners = [Arc::clone(&owner), Arc::clone(&owner)];
        assert!(matches!(
            GeneratedSectorAffineConditionalRuleProvider::try_new(
                &family,
                &context,
                &duplicate_owners,
                CountingInner::new(3, Arc::clone(&calls)),
                GeneratedSectorAffineConditionalRuleProviderLimits::default(),
            ),
            Err(GeneratedSectorAffineConditionalRuleProviderError::DuplicateSector { .. })
        ));

        let (foreign_family, foreign_context, foreign_owner) =
            owner_fixture("001", "affine-provider-generated-foreign-owner");
        let foreign_owner = Arc::new(foreign_owner);
        assert!(matches!(
            GeneratedSectorAffineConditionalRuleProvider::try_new(
                &family,
                &context,
                &[foreign_owner],
                CountingInner::new(3, Arc::clone(&calls)),
                GeneratedSectorAffineConditionalRuleProviderLimits::default(),
            ),
            Err(GeneratedSectorAffineConditionalRuleProviderError::ForeignOwner { .. })
        ));
        drop((foreign_family, foreign_context));

        let (corrupt_family, corrupt_context, mut corrupt_owner) =
            owner_fixture("001", "affine-provider-generated-corrupt-owner");
        assert!(corrupt_owner.test_only_corrupt_first_pass_group_ordinal());
        assert!(matches!(
            GeneratedSectorAffineConditionalRuleProvider::try_new(
                &corrupt_family,
                &corrupt_context,
                &[Arc::new(corrupt_owner)],
                CountingInner::new(3, calls),
                GeneratedSectorAffineConditionalRuleProviderLimits::default(),
            ),
            Err(GeneratedSectorAffineConditionalRuleProviderError::OwnerReplay { .. })
        ));
    }

    #[test]
    fn exact_build_limits_pass_and_each_positive_one_below_fails() {
        let (family, context, owner) =
            owner_fixture("001", "affine-provider-generated-build-budgets");
        let owner = Arc::new(owner);
        let baseline = GeneratedSectorAffineConditionalRuleProvider::try_new(
            &family,
            &context,
            std::slice::from_ref(&owner),
            CountingInner::new(3, Arc::new(AtomicUsize::new(0))),
            provider_limits_for_one_owner(owner.as_ref()),
        )
        .unwrap();
        let baseline_stats = baseline.build_stats();
        assert!(baseline_stats.retained_owner_index_bytes() > 0);
        assert!(baseline_stats.temporary_owner_index_bytes() > 0);
        assert!(baseline_stats.peak_visible_build_bytes() > 0);
        drop(baseline);

        let mut exact = provider_limits_for_one_owner(owner.as_ref());
        exact.max_retained_owner_index_bytes = baseline_stats.retained_owner_index_bytes();
        exact.max_temporary_owner_index_bytes = baseline_stats.temporary_owner_index_bytes();
        exact.max_peak_visible_build_bytes = baseline_stats.peak_visible_build_bytes();
        let provider = GeneratedSectorAffineConditionalRuleProvider::try_new(
            &family,
            &context,
            std::slice::from_ref(&owner),
            CountingInner::new(3, Arc::new(AtomicUsize::new(0))),
            exact,
        )
        .unwrap();
        assert_eq!(provider.build_stats().installed_owners(), 1);
        assert_eq!(provider.build_stats().installed_sectors(), 1);
        drop(provider);

        for one_below in [
            GeneratedSectorAffineConditionalRuleProviderLimits {
                max_installed_owners: 0,
                ..exact
            },
            GeneratedSectorAffineConditionalRuleProviderLimits {
                max_installed_sectors: 0,
                ..exact
            },
            GeneratedSectorAffineConditionalRuleProviderLimits {
                max_retained_outer_owner_bytes: owner
                    .stats()
                    .outer_retained_bytes()
                    .saturating_sub(1),
                ..exact
            },
            GeneratedSectorAffineConditionalRuleProviderLimits {
                max_retained_owner_index_bytes: baseline_stats
                    .retained_owner_index_bytes()
                    .saturating_sub(1),
                ..exact
            },
            GeneratedSectorAffineConditionalRuleProviderLimits {
                max_temporary_owner_index_bytes: baseline_stats
                    .temporary_owner_index_bytes()
                    .saturating_sub(1),
                ..exact
            },
            GeneratedSectorAffineConditionalRuleProviderLimits {
                max_peak_visible_build_bytes: baseline_stats
                    .peak_visible_build_bytes()
                    .saturating_sub(1),
                ..exact
            },
        ] {
            assert!(matches!(
                GeneratedSectorAffineConditionalRuleProvider::try_new(
                    &family,
                    &context,
                    std::slice::from_ref(&owner),
                    CountingInner::new(3, Arc::new(AtomicUsize::new(0))),
                    one_below,
                ),
                Err(GeneratedSectorAffineConditionalRuleProviderError::ResourceLimit { .. })
            ));
        }
    }

    #[test]
    fn query_limits_are_exact_and_failed_queries_do_not_commit_wrapper_stats() {
        let (family, context, owner) =
            owner_fixture("001", "affine-provider-generated-query-budgets");
        let owner = Arc::new(owner);
        let source = ConcreteIntegralKey::try_new([-4, -4, 2]).unwrap();

        let mut exact = provider_limits_for_one_owner(owner.as_ref());
        exact.max_queries = 1;
        exact.max_applications = 1;
        exact.max_delegations = 0;
        let mut provider = GeneratedSectorAffineConditionalRuleProvider::try_new(
            &family,
            &context,
            std::slice::from_ref(&owner),
            CountingInner::new(3, Arc::new(AtomicUsize::new(0))),
            exact,
        )
        .unwrap();
        assert!(matches!(
            provider.decision_for(&source).unwrap(),
            ConcreteRuleDecision::ConditionalReduction(_)
        ));
        assert_eq!(provider.stats().queries(), 1);
        assert!(matches!(
            provider.decision_for(&source),
            Err(
                GeneratedSectorAffineConditionalRuleProviderError::ResourceLimit {
                    requested: 2,
                    limit: 1,
                    ..
                }
            )
        ));
        assert_eq!(provider.stats().queries(), 1);

        let mut no_applications = exact;
        no_applications.max_applications = 0;
        let no_application_calls = Arc::new(AtomicUsize::new(0));
        let mut no_application = GeneratedSectorAffineConditionalRuleProvider::try_new(
            &family,
            &context,
            std::slice::from_ref(&owner),
            CountingInner::new(3, Arc::clone(&no_application_calls)),
            no_applications,
        )
        .unwrap();
        assert!(matches!(
            no_application.decision_for(&source),
            Err(
                GeneratedSectorAffineConditionalRuleProviderError::ResourceLimit {
                    requested: 1,
                    limit: 0,
                    ..
                }
            )
        ));
        assert_eq!(no_application.stats(), Default::default());
        assert_eq!(no_application_calls.load(Ordering::SeqCst), 0);

        let missing_source = ConcreteIntegralKey::try_new([1, 1, 1]).unwrap();
        let mut no_delegations = exact;
        no_delegations.max_applications = 0;
        no_delegations.max_delegations = 0;
        let no_delegation_calls = Arc::new(AtomicUsize::new(0));
        let mut no_delegation = GeneratedSectorAffineConditionalRuleProvider::try_new(
            &family,
            &context,
            std::slice::from_ref(&owner),
            CountingInner::new(3, Arc::clone(&no_delegation_calls)),
            no_delegations,
        )
        .unwrap();
        assert!(matches!(
            no_delegation.decision_for(&missing_source),
            Err(
                GeneratedSectorAffineConditionalRuleProviderError::ResourceLimit {
                    requested: 1,
                    limit: 0,
                    ..
                }
            )
        ));
        assert_eq!(no_delegation.stats(), Default::default());
        assert_eq!(no_delegation_calls.load(Ordering::SeqCst), 0);

        let failing_calls = Arc::new(AtomicUsize::new(0));
        let mut failing = GeneratedSectorAffineConditionalRuleProvider::try_new(
            &family,
            &context,
            std::slice::from_ref(&owner),
            CountingInner::failing(3, Arc::clone(&failing_calls)),
            GeneratedSectorAffineConditionalRuleProviderLimits::default(),
        )
        .unwrap();
        assert!(matches!(
            failing.decision_for(&missing_source),
            Err(GeneratedSectorAffineConditionalRuleProviderError::Inner(_))
        ));
        assert_eq!(failing_calls.load(Ordering::SeqCst), 1);
        assert_eq!(failing.stats(), Default::default());
    }
}
