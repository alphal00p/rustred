//! Public vacuum sector-symmetry census and solver-time router.
//!
//! [`SectorSymmetryCensus::discover`] partitions caller-declared sectors of
//! one vacuum family (typically every sector not proved zero by
//! [`crate::sector::zero::Analyzer`]) into exact equivalence classes:
//!
//! - every sector receives the native canonical key of its mass-colored
//!   projector graph ([`CanonicalSupportKey`]);
//! - each class has one deterministic representative, the first sector of the
//!   class under the selected [`RepresentativePolicy`] (sector ids
//!   `sum_{i active} 2^i`, or a caller-supplied preference list), independent
//!   of hashing or input order;
//! - every other member owns an exactly verified route to its representative:
//!   a loop-momentum map `k_member = A k_representative` with unit Jacobian
//!   whose active denominators map by a unit bijection, complete constants
//!   (masses, including parameter-dependent ones such as `-1 - eta`)
//!   included, compiled by [`integral_transport::compile_with_affine_constants`]
//!   under [`SectorSymmetryCensusOptions::affine_constants`];
//! - every representative owns the verified self-maps recovered from the
//!   native canonical-graph automorphism generators.
//!
//! The graph only proposes maps; [`crate::sector::symmetry::verify()`] and
//! the transport compiler establish every retained one. Equal canonical keys
//! do not imply a class: a map with a non-unit Jacobian is never admitted, so
//! such sectors stay separate representatives. A sector-local discovery or
//! transport failure never aborts the census: the affected sector keeps its
//! own class (or the class loses the affected generators) and the failure is
//! reported in [`SectorSymmetryCensus::skips`]. Skips can only split classes
//! or drop symmetry rows; they never introduce an unverified relation.
//!
//! [`SectorSymmetryCensus::discover_subsectors`] also retains the analyzer's
//! zero set ([`SectorSymmetryCensus::is_zero_sector`]).
//! [`SectorSymmetryCensus::transport_to_representative`] returns the exact
//! transported integral (numerators expanded through the verified affine
//! rows); [`SectorSymmetryCensus::normalize`] applies routes recursively until
//! every endpoint lies in a representative sector, dropping zero-set
//! endpoints; [`SectorSymmetryCensus::route_relation`] and
//! [`SectorSymmetryCensus::automorphism_relations`] (and their `_with`
//! variants, see [`RelationEndpoints`]) return exact homogeneous rows
//! `I - T(I) = 0` for solvers. Generator rows are finite exact identities;
//! applied to a seed set closed under the generated images they span the
//! relations of the full finite group. The census is a symmetry proof service
//! only: it makes no IBP-completeness or master-independence claim.

mod error;
mod group;
mod normalize;

#[cfg(test)]
mod tests;

use std::cmp::Ordering;
use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::sync::Arc;

use crate::algebra::Coefficient;
use crate::family::{IntegralFamily, IntegralKey};
use crate::sector::Mask;
use crate::sector::zero::{Analyzer, Decision};

use super::discovery::{self, CanonicalSupportKey, DiscoveryCache, PreparedSupport};
use super::integral_transport::{
    self, AffineConstants, ExpansionLimits, Prepared, TransportedIntegral,
};
use super::{DiscoveryError, Limits, SupportSkipReason, VerifiedMap};

pub use error::SectorSymmetryCensusError;
pub use normalize::{
    DEFAULT_MAX_NORMAL_FORM_TERMS, DEFAULT_MAX_NORMAL_FORM_TRANSPORTS, NormalizationLimits,
    RelationEndpoints, SectorSymmetryNormalForm,
};

type Error = SectorSymmetryCensusError;

/// Default bound on each enumerated line-permutation group.
pub const DEFAULT_MAX_CENSUS_GROUP_ORDER: usize = 1_000_000;

/// Largest top-sector line count enumerated by
/// [`SectorSymmetryCensus::discover_subsectors`] (2^24 masks).
pub const MAX_SUBSECTOR_CENSUS_ACTIVE_LINES: usize = 24;

/// Deterministic representative choice within one symmetry class.
#[derive(Clone, Debug, Default, PartialEq, Eq, Hash)]
pub enum RepresentativePolicy {
    /// The member with the smallest sector id `sum_{i active} 2^i`.
    #[default]
    LowestSectorId,
    /// The member with the largest sector id.
    HighestSectorId,
    /// The first member in this caller list; members not listed rank after
    /// every listed sector, by lowest sector id. With an external tool's
    /// representatives (Kira, AMFlow) the census picks the same ones, so
    /// master lists compare without re-routing. Masks must have the family
    /// arity; repeated masks keep their first position.
    Preferred(Arc<[Mask]>),
}

impl RepresentativePolicy {
    /// Sort `sectors` into visiting order: each class representative is the
    /// first of its members in this order.
    fn order(&self, sectors: &mut [Mask]) {
        match self {
            Self::LowestSectorId => sectors.sort_by(sector_id_cmp),
            Self::HighestSectorId => sectors.sort_by(|left, right| sector_id_cmp(right, left)),
            Self::Preferred(preferred) => {
                let mut rank = BTreeMap::new();
                for (position, mask) in preferred.iter().enumerate() {
                    rank.entry(mask).or_insert(position);
                }
                sectors.sort_by(|left, right| match (rank.get(left), rank.get(right)) {
                    (Some(left), Some(right)) => left.cmp(right),
                    (Some(_), None) => Ordering::Less,
                    (None, Some(_)) => Ordering::Greater,
                    (None, None) => sector_id_cmp(left, right),
                });
            }
        }
    }
}

/// Compare equal-arity masks as binary sector ids, slot 0 least significant.
pub fn sector_id_cmp(left: &Mask, right: &Mask) -> Ordering {
    left.arity().cmp(&right.arity()).then_with(|| {
        left.active_bits()
            .iter()
            .rev()
            .cmp(right.active_bits().iter().rev())
    })
}

/// Exact verification, transport-admission and group-enumeration bounds.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SectorSymmetryCensusOptions {
    pub verification: Limits,
    pub expansion: ExpansionLimits,
    pub representative: RepresentativePolicy,
    /// Affine constants admitted in compiled routes and automorphisms.
    /// Defaults to [`AffineConstants::Parametric`], so deformed `-1 - eta`
    /// lines route exactly. [`AffineConstants::Rational`] restricts the
    /// census to maps every transport lane accepts (routed candidate
    /// reducers, native-Q trace support); maps that need parametric
    /// constants are then reported as [`SectorSymmetryCensus::skips`].
    pub affine_constants: AffineConstants,
    /// Line-permutation groups larger than this report no order.
    pub max_group_order: usize,
    /// Optional diagnostic thresholds of [`SectorSymmetryCensus::normalize`]
    /// and of relation rows with [`RelationEndpoints::Representatives`].
    pub normalization: NormalizationLimits,
}

impl Default for SectorSymmetryCensusOptions {
    fn default() -> Self {
        Self {
            verification: Limits::default(),
            expansion: ExpansionLimits::default(),
            representative: RepresentativePolicy::default(),
            affine_constants: AffineConstants::Parametric,
            max_group_order: DEFAULT_MAX_CENSUS_GROUP_ORDER,
            normalization: NormalizationLimits::default(),
        }
    }
}

/// Cooperative cancellation and borrowed progress while building a census.
#[derive(Default)]
pub struct SectorSymmetryCensusControl<'a> {
    pub cancelled: Option<&'a dyn Fn() -> bool>,
    pub progress: Option<&'a dyn Fn(SectorSymmetryCensusProgress)>,
}

impl SectorSymmetryCensusControl<'_> {
    fn is_cancelled(&self) -> bool {
        self.cancelled.is_some_and(|cancelled| cancelled())
    }

    fn report(&self, progress: SectorSymmetryCensusProgress) {
        if let Some(report) = self.progress {
            report(progress);
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct SectorSymmetryCensusProgress {
    pub declared_sectors: usize,
    pub analyzed_sectors: usize,
    pub classes: usize,
}

/// Census diagnostics. No count is an independent-master statement.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct SectorSymmetryCensusStatistics {
    /// Nonempty subsectors enumerated by `discover_subsectors` (else zero).
    pub enumerated_sectors: usize,
    /// Enumerated nonempty subsectors proved zero or excluded by the
    /// analyzer. The retained zero set also contains the empty sector when
    /// the analyzer proves it zero.
    pub zero_sectors: usize,
    pub declared_sectors: usize,
    pub classes: usize,
    /// Declared sectors routed to a different representative.
    pub routed_sectors: usize,
    /// Sectors without integer square line geometry or with deficient rank.
    pub unsupported_sectors: usize,
    pub automorphism_generators: usize,
    /// Sum over representatives of `|G| - 1` for the induced active-line
    /// permutation group `G`; `None` if any group exceeded its bound or lost
    /// generators to a skip.
    pub nontrivial_line_automorphisms: Option<usize>,
    /// Canonical keys shared by more than one class (non-unit lattices).
    pub shared_canonical_keys: usize,
    /// Tolerated sector-local failures, see [`SectorSymmetryCensus::skips`].
    pub skips: usize,
}

/// Why a sector belongs to the retained zero set.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ZeroSectorKind {
    /// The exact zero-sector analyzer proved the sector scaleless.
    ProvedZero,
    /// The analyzer's authenticated cuts or sector patterns exclude it.
    Excluded,
}

/// Where a tolerated sector-local census failure occurred.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SectorSymmetrySkipStage {
    /// Preparing the sector's projector-graph support failed; the sector
    /// is a singleton class without symmetry proposals and reports no group
    /// order.
    Support,
    /// Recovering the canonical map onto an existing representative with the
    /// same canonical key failed; other candidates were still tried.
    Match { representative: Mask },
    /// The verified map onto an existing representative did not compile into
    /// an admitted transport; other candidates were still tried.
    Route { representative: Mask },
    /// Discovering the representative's automorphism generators failed; the
    /// class keeps none and reports no group order.
    Automorphisms,
    /// One verified automorphism generator did not compile and was dropped;
    /// the class reports no group order.
    AutomorphismGenerator { ordinal: usize },
}

/// The typed failure behind a [`SectorSymmetrySkip`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SectorSymmetrySkipReason {
    Discovery(DiscoveryError),
    Transport(integral_transport::Error),
}

impl std::fmt::Display for SectorSymmetrySkipReason {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Discovery(error) => error.fmt(f),
            Self::Transport(error) => error.fmt(f),
        }
    }
}

/// One tolerated sector-local failure. Cancellation is never tolerated.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SectorSymmetrySkip {
    sector: Mask,
    stage: SectorSymmetrySkipStage,
    reason: SectorSymmetrySkipReason,
}

impl SectorSymmetrySkip {
    /// The declared sector (or class representative) that was affected.
    pub fn sector(&self) -> &Mask {
        &self.sector
    }

    pub fn stage(&self) -> &SectorSymmetrySkipStage {
        &self.stage
    }

    pub fn reason(&self) -> &SectorSymmetrySkipReason {
        &self.reason
    }
}

/// One declared sector and its verified route to the class representative.
#[derive(Debug)]
pub struct SectorSymmetryMember {
    sector: Mask,
    class: usize,
    canonical_key: Option<CanonicalSupportKey>,
    route: Arc<Prepared>,
}

impl SectorSymmetryMember {
    pub fn sector(&self) -> &Mask {
        &self.sector
    }

    /// Index into [`SectorSymmetryCensus::classes`].
    pub fn class(&self) -> usize {
        self.class
    }

    pub fn representative(&self) -> &Mask {
        self.route.target_root()
    }

    pub fn is_representative(&self) -> bool {
        self.route.source_root() == self.route.target_root()
    }

    /// `None` for unsupported or rank-deficient geometry.
    pub fn canonical_key(&self) -> Option<&CanonicalSupportKey> {
        self.canonical_key.as_ref()
    }

    /// Compiled verified route `k_member = A k_representative`; the identity
    /// for a representative.
    pub fn route(&self) -> &Prepared {
        &self.route
    }

    pub fn verified_map(&self) -> &VerifiedMap {
        self.route.verified_map()
    }
}

/// One exact equivalence class of declared sectors.
#[derive(Debug)]
pub struct SectorSymmetryClass {
    representative: Mask,
    canonical_key: Option<CanonicalSupportKey>,
    members: Box<[Mask]>,
    automorphisms: Box<[Arc<Prepared>]>,
    line_permutations: Box<[Box<[usize]>]>,
    line_group_order: Option<usize>,
    unsupported: Option<SupportSkipReason>,
}

impl SectorSymmetryClass {
    pub fn representative(&self) -> &Mask {
        &self.representative
    }

    pub fn canonical_key(&self) -> Option<&CanonicalSupportKey> {
        self.canonical_key.as_ref()
    }

    /// Members in representative-policy order; the representative is first.
    pub fn members(&self) -> &[Mask] {
        &self.members
    }

    /// Compiled verified self-maps of the representative, one per retained
    /// native canonical-graph generator.
    pub fn automorphisms(&self) -> &[Arc<Prepared>] {
        &self.automorphisms
    }

    /// For each automorphism, the image slot of every active representative
    /// slot, listed in increasing active-slot order.
    pub fn line_permutations(&self) -> &[Box<[usize]>] {
        &self.line_permutations
    }

    /// Order of the active-line permutation group generated by
    /// [`Self::line_permutations`], if within the configured bound and no
    /// generator was skipped.
    pub fn line_group_order(&self) -> Option<usize> {
        self.line_group_order
    }

    pub fn unsupported(&self) -> Option<SupportSkipReason> {
        self.unsupported
    }
}

/// Exact homogeneous relation `sum_i c_i I_i = 0`, sorted by key.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SectorSymmetryRelation {
    source: IntegralKey,
    automorphism: Option<usize>,
    terms: Box<[(IntegralKey, Coefficient)]>,
}

impl SectorSymmetryRelation {
    /// The integral `I` of `I - T(I)`.
    pub fn source(&self) -> &IntegralKey {
        &self.source
    }

    /// Generator index within the class, or `None` for a route relation.
    pub fn automorphism(&self) -> Option<usize> {
        self.automorphism
    }

    pub fn terms(&self) -> &[(IntegralKey, Coefficient)] {
        &self.terms
    }
}

/// Verified sector-symmetry partition and router for one vacuum family.
#[derive(Debug)]
pub struct SectorSymmetryCensus {
    family: Arc<IntegralFamily>,
    options: SectorSymmetryCensusOptions,
    members: BTreeMap<Mask, SectorSymmetryMember>,
    classes: Box<[SectorSymmetryClass]>,
    zero_sectors: BTreeMap<Mask, ZeroSectorKind>,
    skips: Box<[SectorSymmetrySkip]>,
    statistics: SectorSymmetryCensusStatistics,
}

struct ClassBuilder {
    representative: Mask,
    canonical_key: Option<CanonicalSupportKey>,
    support: Option<Arc<PreparedSupport>>,
    members: Vec<Mask>,
    unsupported: Option<SupportSkipReason>,
    /// Support preparation failed (a `Support` skip): no group is known.
    support_failed: bool,
}

/// Keep cancellation fatal; hand every other discovery failure back as a
/// tolerated per-sector outcome.
fn tolerate<T>(result: Result<T, DiscoveryError>) -> Result<Result<T, DiscoveryError>, Error> {
    match result {
        Err(DiscoveryError::Cancelled) => Err(Error::Discovery(DiscoveryError::Cancelled)),
        other => Ok(other),
    }
}

impl SectorSymmetryCensus {
    /// Classify every declared sector. Duplicates are coalesced; empty masks
    /// and foreign arities are errors. Zero sectors should not be declared:
    /// their supports are scaleless and carry no reduction information. The
    /// zero set of a census built here is empty.
    ///
    /// Sector-local discovery and transport failures are tolerated and
    /// reported by [`Self::skips`]; cancellation, family-level eligibility
    /// and a failing identity transport of a declared sector are errors.
    pub fn discover(
        family: Arc<IntegralFamily>,
        sectors: impl IntoIterator<Item = Mask>,
        options: SectorSymmetryCensusOptions,
        control: &SectorSymmetryCensusControl<'_>,
    ) -> Result<Self, Error> {
        discovery::eligible(&family)?;
        let arity = family.denominator_count();
        let check_arity = |sector: &Mask| {
            if sector.arity() == arity {
                Ok(())
            } else {
                Err(Error::WrongArity {
                    expected: arity,
                    actual: sector.arity(),
                })
            }
        };
        if let RepresentativePolicy::Preferred(preferred) = &options.representative {
            preferred.iter().try_for_each(check_arity)?;
        }
        let mut declared = BTreeSet::new();
        for sector in sectors {
            check_arity(&sector)?;
            if sector.active_count() == 0 {
                return Err(Error::EmptySector);
            }
            declared.insert(sector);
        }
        let mut ordered: Vec<_> = declared.into_iter().collect();
        options.representative.order(&mut ordered);
        let cancelled = || control.is_cancelled();
        let identity = Arc::new(super::verify(
            &family,
            &family,
            discovery::identity_map(&family, options.verification)?,
            options.verification,
        )?);
        let compile = |map: Arc<VerifiedMap>, source: &Mask, target: &Mask| {
            integral_transport::compile_with_affine_constants(
                &family,
                family.clone(),
                map,
                source.clone(),
                target.clone(),
                options.expansion,
                options.affine_constants,
            )
            .map(Arc::new)
        };

        let mut cache = DiscoveryCache::new(family.clone());
        let mut builders: Vec<ClassBuilder> = Vec::new();
        let mut by_key: HashMap<CanonicalSupportKey, Vec<usize>> = HashMap::new();
        let mut members = BTreeMap::new();
        let mut skips = Vec::new();
        let mut skip = |sector: &Mask, stage, reason| {
            skips.push(SectorSymmetrySkip {
                sector: sector.clone(),
                stage,
                reason,
            });
        };
        for (analyzed, sector) in ordered.iter().enumerate() {
            discovery::checkpoint(&cancelled)?;
            let mut support_failed = false;
            let support =
                match tolerate(cache.prepare_support(sector, options.verification, &cancelled))? {
                    Ok(support) => support,
                    Err(error) => {
                        support_failed = true;
                        skip(
                            sector,
                            SectorSymmetrySkipStage::Support,
                            SectorSymmetrySkipReason::Discovery(error),
                        );
                        None
                    }
                };
            let canonical_key = support.as_ref().map(|support| support.canonical_key());
            let mut routed = None;
            if let (Some(support), Some(key)) = (&support, &canonical_key) {
                for &class in by_key.get(key).map(Vec::as_slice).unwrap_or_default() {
                    let representative = &builders[class].representative;
                    let target = builders[class]
                        .support
                        .as_deref()
                        .expect("keyed classes retain their support");
                    let map = match tolerate(discovery::match_supports(
                        support,
                        target,
                        options.verification,
                        &cancelled,
                    ))? {
                        Ok(Some(map)) => map,
                        Ok(None) => continue,
                        Err(error) => {
                            skip(
                                sector,
                                SectorSymmetrySkipStage::Match {
                                    representative: representative.clone(),
                                },
                                SectorSymmetrySkipReason::Discovery(error),
                            );
                            continue;
                        }
                    };
                    match compile(map, sector, representative) {
                        Ok(route) => {
                            routed = Some((class, route));
                            break;
                        }
                        Err(error) => skip(
                            sector,
                            SectorSymmetrySkipStage::Route {
                                representative: representative.clone(),
                            },
                            SectorSymmetrySkipReason::Transport(error),
                        ),
                    }
                }
            }
            let (class, route) = match routed {
                Some((class, route)) => {
                    builders[class].members.push(sector.clone());
                    (class, route)
                }
                None => {
                    let class = builders.len();
                    if let Some(key) = &canonical_key {
                        by_key.entry(key.clone()).or_default().push(class);
                    }
                    builders.push(ClassBuilder {
                        representative: sector.clone(),
                        canonical_key: canonical_key.clone(),
                        unsupported: support
                            .is_none()
                            .then(|| cache.skip_reason(sector))
                            .flatten(),
                        support,
                        members: vec![sector.clone()],
                        support_failed,
                    });
                    (class, compile(identity.clone(), sector, sector)?)
                }
            };
            members.insert(
                sector.clone(),
                SectorSymmetryMember {
                    sector: sector.clone(),
                    class,
                    canonical_key,
                    route,
                },
            );
            control.report(SectorSymmetryCensusProgress {
                declared_sectors: ordered.len(),
                analyzed_sectors: analyzed + 1,
                classes: builders.len(),
            });
        }

        let mut classes = Vec::with_capacity(builders.len());
        let mut statistics = SectorSymmetryCensusStatistics {
            declared_sectors: ordered.len(),
            classes: builders.len(),
            shared_canonical_keys: by_key.values().filter(|classes| classes.len() > 1).count(),
            nontrivial_line_automorphisms: Some(0),
            ..Default::default()
        };
        for builder in builders {
            discovery::checkpoint(&cancelled)?;
            let mut automorphisms = Vec::new();
            let mut line_permutations = Vec::new();
            let mut complete = !builder.support_failed;
            if let Some(support) = &builder.support {
                let root = support.root();
                let slots: Vec<usize> = root
                    .active_bits()
                    .iter()
                    .enumerate()
                    .filter_map(|(slot, &active)| active.then_some(slot))
                    .collect();
                let maps = match tolerate(discovery::discover_prepared(
                    support,
                    options.verification,
                    &cancelled,
                ))? {
                    Ok(maps) => maps,
                    Err(error) => {
                        complete = false;
                        skip(
                            root,
                            SectorSymmetrySkipStage::Automorphisms,
                            SectorSymmetrySkipReason::Discovery(error),
                        );
                        Vec::new()
                    }
                };
                for (ordinal, map) in maps.into_iter().enumerate() {
                    let prepared = match compile(map, root, root) {
                        Ok(prepared) => prepared,
                        Err(error) => {
                            complete = false;
                            skip(
                                root,
                                SectorSymmetrySkipStage::AutomorphismGenerator { ordinal },
                                SectorSymmetrySkipReason::Transport(error),
                            );
                            continue;
                        }
                    };
                    let images = slots
                        .iter()
                        .map(|&slot| {
                            prepared.active_target_axes()[slot]
                                .expect("compiled self-map binds every active slot")
                        })
                        .collect();
                    automorphisms.push(prepared);
                    line_permutations.push(images);
                }
            }
            let line_group_order = complete
                .then(|| {
                    line_group_order(
                        &builder.representative,
                        &line_permutations,
                        options.max_group_order,
                    )
                })
                .flatten();
            statistics.routed_sectors += builder.members.len() - 1;
            statistics.unsupported_sectors += usize::from(builder.support.is_none());
            statistics.automorphism_generators += automorphisms.len();
            statistics.nontrivial_line_automorphisms = statistics
                .nontrivial_line_automorphisms
                .zip(line_group_order)
                .map(|(sum, order)| sum + order - 1);
            classes.push(SectorSymmetryClass {
                representative: builder.representative,
                canonical_key: builder.canonical_key,
                members: builder.members.into_boxed_slice(),
                automorphisms: automorphisms.into_boxed_slice(),
                line_permutations: line_permutations.into_boxed_slice(),
                line_group_order,
                unsupported: builder.unsupported,
            });
        }
        statistics.skips = skips.len();
        Ok(Self {
            family,
            options,
            members,
            classes: classes.into_boxed_slice(),
            zero_sectors: BTreeMap::new(),
            skips: skips.into_boxed_slice(),
            statistics,
        })
    }

    /// Enumerate every nonempty subsector of `top`, keep those the exact
    /// zero-sector analyzer does not prove zero or exclude, and classify them.
    /// The analyzer's zero decisions (and the empty sector, if proved zero)
    /// are retained as the census zero set.
    pub fn discover_subsectors(
        family: Arc<IntegralFamily>,
        top: &Mask,
        analyzer: &Analyzer,
        options: SectorSymmetryCensusOptions,
        control: &SectorSymmetryCensusControl<'_>,
    ) -> Result<Self, Error> {
        if analyzer.family_fingerprint() != family.fingerprint() {
            return Err(Error::ForeignAnalyzer);
        }
        let arity = family.denominator_count();
        if top.arity() != arity {
            return Err(Error::WrongArity {
                expected: arity,
                actual: top.arity(),
            });
        }
        let slots: Vec<usize> = top
            .active_bits()
            .iter()
            .enumerate()
            .filter_map(|(slot, &active)| active.then_some(slot))
            .collect();
        if slots.len() > MAX_SUBSECTOR_CENSUS_ACTIVE_LINES {
            return Err(Error::TooManySubsectors {
                active: slots.len(),
                limit: MAX_SUBSECTOR_CENSUS_ACTIVE_LINES,
            });
        }
        let mut nonzero = Vec::new();
        let mut zero_sectors = BTreeMap::new();
        let mut bits = vec![false; arity];
        let count = 1_usize << slots.len();
        for subset in 0..count {
            if control.is_cancelled() {
                return Err(Error::Discovery(DiscoveryError::Cancelled));
            }
            for (position, &slot) in slots.iter().enumerate() {
                bits[slot] = subset & (1 << position) != 0;
            }
            let mask = Mask::try_new(bits.iter().copied())?;
            match analyzer.analyze(&mask)? {
                Decision::Inconclusive(_) if subset != 0 => nonzero.push(mask),
                // The empty sector is never declared; it joins the zero set
                // only on an analyzer proof.
                Decision::Inconclusive(_) => {}
                Decision::ProvedZero(_) => {
                    zero_sectors.insert(mask, ZeroSectorKind::ProvedZero);
                }
                Decision::Excluded(_) => {
                    zero_sectors.insert(mask, ZeroSectorKind::Excluded);
                }
            }
        }
        let empty_zero = usize::from(
            zero_sectors.contains_key(&Mask::try_new(std::iter::repeat_n(false, arity))?),
        );
        let mut census = Self::discover(family, nonzero, options, control)?;
        census.statistics.enumerated_sectors = count - 1;
        census.statistics.zero_sectors = zero_sectors.len() - empty_zero;
        census.zero_sectors = zero_sectors;
        Ok(census)
    }

    pub fn family(&self) -> &Arc<IntegralFamily> {
        &self.family
    }

    pub fn options(&self) -> &SectorSymmetryCensusOptions {
        &self.options
    }

    pub fn statistics(&self) -> &SectorSymmetryCensusStatistics {
        &self.statistics
    }

    /// Classes in representative-policy order of their representatives.
    pub fn classes(&self) -> &[SectorSymmetryClass] {
        &self.classes
    }

    /// Every declared sector, in `Mask` order.
    pub fn members(&self) -> impl ExactSizeIterator<Item = &SectorSymmetryMember> {
        self.members.values()
    }

    pub fn member(&self, sector: &Mask) -> Option<&SectorSymmetryMember> {
        self.members.get(sector)
    }

    pub fn class_of(&self, sector: &Mask) -> Option<&SectorSymmetryClass> {
        self.member(sector)
            .map(|member| &self.classes[member.class])
    }

    pub fn representative(&self, sector: &Mask) -> Option<&Mask> {
        self.member(sector)
            .map(SectorSymmetryMember::representative)
    }

    /// Tolerated sector-local failures in discovery order. Empty for a
    /// census in which every proposed symmetry was established.
    pub fn skips(&self) -> &[SectorSymmetrySkip] {
        &self.skips
    }

    /// Whether `sector` is in the retained zero set (proved zero or excluded
    /// by the analyzer of [`Self::discover_subsectors`]).
    pub fn is_zero_sector(&self, sector: &Mask) -> bool {
        self.zero_sectors.contains_key(sector)
    }

    pub fn zero_sector_kind(&self, sector: &Mask) -> Option<ZeroSectorKind> {
        self.zero_sectors.get(sector).copied()
    }

    /// The retained zero set, in `Mask` order.
    pub fn zero_sectors(&self) -> impl ExactSizeIterator<Item = (&Mask, ZeroSectorKind)> {
        self.zero_sectors.iter().map(|(mask, kind)| (mask, *kind))
    }

    /// Exact transport of one integral of a declared sector onto its class
    /// representative. Numerators are expanded through the verified affine
    /// rows; pinched endpoints lie in subsectors of the representative.
    pub fn transport_to_representative(
        &self,
        key: &IntegralKey,
    ) -> Result<TransportedIntegral, Error> {
        let member = self.member_of(key)?;
        Ok(member.route.transport(key, self.options.expansion)?)
    }

    /// `I - T(I) = 0` routing a non-representative integral to its
    /// representative; `None` for an integral of a representative sector.
    /// Endpoints are kept as transported.
    pub fn route_relation(
        &self,
        key: &IntegralKey,
    ) -> Result<Option<SectorSymmetryRelation>, Error> {
        self.route_relation_with(key, RelationEndpoints::Transported)
    }

    /// [`Self::route_relation`] with the selected endpoint treatment.
    pub fn route_relation_with(
        &self,
        key: &IntegralKey,
        endpoints: RelationEndpoints,
    ) -> Result<Option<SectorSymmetryRelation>, Error> {
        let member = self.member_of(key)?;
        if member.is_representative() {
            return Ok(None);
        }
        let transported = member.route.transport(key, self.options.expansion)?;
        let mut memo = normalize::Memo::default();
        self.relation(key, &transported, None, endpoints, &mut memo)
    }

    /// Nonzero exact rows `I - T_g(I) = 0`, one per representative generator
    /// `g`, deduplicated in generator order. Requires a representative sector.
    /// Endpoints are kept as transported.
    pub fn automorphism_relations(
        &self,
        key: &IntegralKey,
    ) -> Result<Vec<SectorSymmetryRelation>, Error> {
        self.automorphism_relations_with(key, RelationEndpoints::Transported)
    }

    /// [`Self::automorphism_relations`] with the selected endpoint treatment.
    pub fn automorphism_relations_with(
        &self,
        key: &IntegralKey,
        endpoints: RelationEndpoints,
    ) -> Result<Vec<SectorSymmetryRelation>, Error> {
        let member = self.member_of(key)?;
        if !member.is_representative() {
            return Err(Error::NotRepresentative(member.sector.clone()));
        }
        let class = &self.classes[member.class];
        let mut relations: Vec<SectorSymmetryRelation> = Vec::new();
        let mut memo = normalize::Memo::default();
        for (ordinal, automorphism) in class.automorphisms.iter().enumerate() {
            let transported = automorphism.transport(key, self.options.expansion)?;
            if let Some(relation) =
                self.relation(key, &transported, Some(ordinal), endpoints, &mut memo)?
                && !relations.iter().any(|old| old.terms == relation.terms)
            {
                relations.push(relation);
            }
        }
        Ok(relations)
    }

    /// The declared member owning `key`'s sector. Zero-set and undeclared
    /// sectors are distinct typed errors.
    fn member_of(&self, key: &IntegralKey) -> Result<&SectorSymmetryMember, Error> {
        let sector = self.sector_of(key)?;
        if let Some(member) = self.members.get(&sector) {
            return Ok(member);
        }
        if self.is_zero_sector(&sector) {
            Err(Error::ZeroSector(sector))
        } else {
            Err(Error::UnknownSector(sector))
        }
    }

    fn sector_of(&self, key: &IntegralKey) -> Result<Mask, Error> {
        let arity = self.family.denominator_count();
        if key.powers().len() != arity {
            return Err(Error::WrongArity {
                expected: arity,
                actual: key.powers().len(),
            });
        }
        Ok(Mask::try_from_indices(key.powers())?)
    }

    fn relation(
        &self,
        source: &IntegralKey,
        transported: &TransportedIntegral,
        automorphism: Option<usize>,
        endpoints: RelationEndpoints,
        memo: &mut normalize::Memo,
    ) -> Result<Option<SectorSymmetryRelation>, Error> {
        let context = self.family.coefficient_context();
        let limits = self.options.expansion.exact_algebra;
        let mut terms = BTreeMap::from([(source.clone(), context.one())]);
        let mut subtract = |key: &IntegralKey, coefficient: &Coefficient| {
            normalize::accumulate(context, limits, &mut terms, key, coefficient, true)
        };
        match endpoints {
            RelationEndpoints::Transported => {
                for endpoint in transported.terms() {
                    subtract(endpoint.key(), endpoint.coefficient())?;
                }
            }
            RelationEndpoints::DropZero => {
                for endpoint in transported.terms() {
                    if !self.is_zero_sector(&self.sector_of(endpoint.key())?) {
                        subtract(endpoint.key(), endpoint.coefficient())?;
                    }
                }
            }
            RelationEndpoints::Representatives => {
                let image = self.normalize_terms(
                    transported
                        .terms()
                        .iter()
                        .map(|endpoint| (endpoint.key().clone(), endpoint.coefficient().clone())),
                    memo,
                )?;
                for (key, coefficient) in &image.terms {
                    subtract(key, coefficient)?;
                }
            }
        }
        if terms.is_empty() {
            return Ok(None);
        }
        Ok(Some(SectorSymmetryRelation {
            source: source.clone(),
            automorphism,
            terms: terms.into_iter().collect(),
        }))
    }
}

fn line_group_order(
    representative: &Mask,
    line_permutations: &[Box<[usize]>],
    max_order: usize,
) -> Option<usize> {
    let slots: Vec<usize> = representative
        .active_bits()
        .iter()
        .enumerate()
        .filter_map(|(slot, &active)| active.then_some(slot))
        .collect();
    let positions: Vec<Box<[usize]>> = line_permutations
        .iter()
        .map(|images| {
            images
                .iter()
                .map(|image| {
                    slots
                        .binary_search(image)
                        .expect("compiled self-map stays on the representative")
                })
                .collect()
        })
        .collect();
    group::permutation_group_order(slots.len(), &positions, max_order)
}
