//! Replayable, topology-independent orchestration of family sector analysis.
//!
//! This is RustRed's bounded inventory layer corresponding to the orchestration
//! part of LiteRed's `AnalyzeSectors`.  It enumerates every raw sector through
//! [`ZeroSectorAnalyzer::analyze_all`], retains each exact proof/diagnostic, and
//! derives a deterministic queue of still-unresolved sectors.  A full-column-
//! rank witness means only that the current sufficient zero criterion did not
//! prove zero; it is never relabelled as an analytic nonzero proof or a master.

use std::fmt;
use std::sync::Arc;

use crate::{
    FullColumnRankWitness, IntegralComplexityKey, IntegralFamily, IntegralOrderingPolicy,
    PowerShiftPolicy, SectorExclusion, SectorFoundationError, SectorMask, SectorRestrictions,
    ZeroSectorAnalyzer, ZeroSectorCertificate, ZeroSectorDecision, ZeroSectorDomain,
    ZeroSectorError, ZeroSectorLimits, ZeroSectorResource,
};

pub const FAMILY_SECTOR_INVENTORY_V1_SCHEMA: &str = "rustred.family-sector-inventory.v1";
pub const FORMAL_GENERIC_POWER_SHIFT_POLICY_V1_ID: &str =
    "rustred.power-shift-support.formal-generic.v1";

/// Outer transcript and dependency-check budgets.  `zero_sectors` is retained
/// verbatim in the certificate and is also the exact analyzer budget on replay.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FamilySectorInventoryLimits {
    pub zero_sectors: ZeroSectorLimits,
    pub max_inventory_entries: usize,
    pub max_unresolved_solve_entries: usize,
    pub max_dependency_checks: usize,
    pub max_binding_bytes: usize,
}

impl Default for FamilySectorInventoryLimits {
    fn default() -> Self {
        Self {
            zero_sectors: ZeroSectorLimits::default(),
            max_inventory_entries: 1_048_576,
            max_unresolved_solve_entries: 1_048_576,
            max_dependency_checks: 67_108_864,
            max_binding_bytes: 16 * 1024 * 1024,
        }
    }
}

/// Exact outcome retained for one raw, unshifted sector mask.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum FamilySectorInventoryStatus {
    /// User cut/pattern metadata excluded this sector.  This is not an
    /// analytic zero proof.
    Excluded(SectorExclusion),
    /// The Symbolica-native `U+F` rank criterion supplied a replayable kernel.
    ProvedZero(ZeroSectorCertificate),
    /// The sufficient zero test had full column rank.  The integral remains
    /// analytically unresolved and may enter the sector-solving queue.
    UnresolvedNoZeroCertificate(FullColumnRankWitness),
    /// Bounded analysis of this sector could not complete.
    ResourceLimited(ZeroSectorResource),
    /// A typed non-resource analysis failure was retained without inference.
    Failed(ZeroSectorError),
}

impl FamilySectorInventoryStatus {
    pub fn is_excluded(&self) -> bool {
        matches!(self, Self::Excluded(_))
    }

    pub fn is_proved_zero(&self) -> bool {
        matches!(self, Self::ProvedZero(_))
    }

    /// The current sufficient zero test returned no zero certificate.  This
    /// is not a proof that the integral is analytically nonzero.
    pub fn is_unresolved_after_zero_test(&self) -> bool {
        matches!(self, Self::UnresolvedNoZeroCertificate(_))
    }
}

impl From<ZeroSectorDecision> for FamilySectorInventoryStatus {
    fn from(value: ZeroSectorDecision) -> Self {
        match value {
            ZeroSectorDecision::Excluded(exclusion) => Self::Excluded(exclusion),
            ZeroSectorDecision::ProvedZero(certificate) => Self::ProvedZero(certificate),
            ZeroSectorDecision::NoZeroCertificate(witness) => {
                Self::UnresolvedNoZeroCertificate(witness)
            }
            ZeroSectorDecision::ResourceLimited(resource) => Self::ResourceLimited(resource),
            ZeroSectorDecision::Failed(error) => Self::Failed(error),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FamilySectorInventoryEntry {
    sector: SectorMask,
    status: FamilySectorInventoryStatus,
}

impl FamilySectorInventoryEntry {
    pub fn sector(&self) -> &SectorMask {
        &self.sector
    }

    pub fn status(&self) -> &FamilySectorInventoryStatus {
        &self.status
    }
}

/// One unresolved sector and its exact corner-ordering key.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct UnresolvedSectorSolveOrderEntry {
    sector: SectorMask,
    corner_key: IntegralComplexityKey,
}

impl UnresolvedSectorSolveOrderEntry {
    pub fn sector(&self) -> &SectorMask {
        &self.sector
    }

    pub fn corner_key(&self) -> &IntegralComplexityKey {
        &self.corner_key
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct FamilySectorInventoryStats {
    inventory_entries: usize,
    excluded: usize,
    proved_zero: usize,
    unresolved: usize,
    resource_limited: usize,
    failed: usize,
    unresolved_solve_entries: usize,
    dependency_checks: usize,
    distinct_effective_masks: usize,
    binding_bytes: usize,
}

impl FamilySectorInventoryStats {
    pub fn inventory_entries(self) -> usize {
        self.inventory_entries
    }

    pub fn excluded(self) -> usize {
        self.excluded
    }

    pub fn proved_zero(self) -> usize {
        self.proved_zero
    }

    pub fn unresolved(self) -> usize {
        self.unresolved
    }

    pub fn resource_limited(self) -> usize {
        self.resource_limited
    }

    pub fn failed(self) -> usize {
        self.failed
    }

    pub fn unresolved_solve_entries(self) -> usize {
        self.unresolved_solve_entries
    }

    pub fn dependency_checks(self) -> usize {
        self.dependency_checks
    }

    pub fn distinct_effective_masks(self) -> usize {
        self.distinct_effective_masks
    }

    pub fn binding_bytes(self) -> usize {
        self.binding_bytes
    }
}

/// Complete replayable family-sector inventory.
#[derive(Clone, Debug)]
pub struct FamilySectorInventoryCertificate {
    schema: &'static str,
    family_fingerprint: Arc<str>,
    symanzik_g_fingerprint: Arc<str>,
    restrictions: SectorRestrictions,
    power_shift_policy: PowerShiftPolicy,
    power_shift_policy_id: &'static str,
    power_support: SectorMask,
    zero_sector_domain: ZeroSectorDomain,
    ordering: IntegralOrderingPolicy,
    entries: Box<[FamilySectorInventoryEntry]>,
    unresolved_solve_order: Box<[UnresolvedSectorSolveOrderEntry]>,
    monotone_zero_closure_verified: bool,
    limits: FamilySectorInventoryLimits,
    stats: FamilySectorInventoryStats,
}

impl FamilySectorInventoryCertificate {
    pub fn schema(&self) -> &'static str {
        self.schema
    }

    pub fn family_fingerprint(&self) -> &str {
        &self.family_fingerprint
    }

    pub fn symanzik_g_fingerprint(&self) -> &str {
        &self.symanzik_g_fingerprint
    }

    pub fn restrictions(&self) -> &SectorRestrictions {
        &self.restrictions
    }

    pub fn power_shift_policy(&self) -> PowerShiftPolicy {
        self.power_shift_policy
    }

    pub fn power_shift_policy_id(&self) -> &'static str {
        self.power_shift_policy_id
    }

    pub fn power_support(&self) -> &SectorMask {
        &self.power_support
    }

    pub fn zero_sector_domain(&self) -> &ZeroSectorDomain {
        &self.zero_sector_domain
    }

    pub fn ordering(&self) -> IntegralOrderingPolicy {
        self.ordering
    }

    pub fn entries(&self) -> &[FamilySectorInventoryEntry] {
        &self.entries
    }

    pub fn status(&self, sector: &SectorMask) -> Option<&FamilySectorInventoryStatus> {
        self.entries
            .binary_search_by(|entry| entry.sector.cmp(sector))
            .ok()
            .map(|position| &self.entries[position].status)
    }

    /// Dependency-safe queue of sectors for which the sufficient zero test
    /// returned full rank.  These are unresolved, not certified nonzero and
    /// never inferred masters.
    pub fn unresolved_solve_order(&self) -> &[UnresolvedSectorSolveOrderEntry] {
        &self.unresolved_solve_order
    }

    pub fn monotone_zero_closure_verified(&self) -> bool {
        self.monotone_zero_closure_verified
    }

    pub fn limits(&self) -> FamilySectorInventoryLimits {
        self.limits
    }

    pub fn stats(&self) -> FamilySectorInventoryStats {
        self.stats
    }

    /// Rebuild the exact analyzer with the retained restrictions, power-shift
    /// policy, and `ZeroSectorLimits`; run the all-sector analysis once; and
    /// compare the complete deterministic payload.
    pub fn replay(&self, family: &IntegralFamily) -> Result<(), FamilySectorInventoryError> {
        if self.schema != FAMILY_SECTOR_INVENTORY_V1_SCHEMA {
            return Err(FamilySectorInventoryError::SchemaMismatch);
        }
        if family.fingerprint() != self.family_fingerprint.as_ref() {
            return Err(FamilySectorInventoryError::ForeignFamily);
        }
        let replayed = FamilySectorInventoryCompiler::compile(
            family,
            self.restrictions.clone(),
            self.power_shift_policy,
            self.ordering,
            self.limits,
        )?;
        if self.payload_eq(&replayed) {
            Ok(())
        } else {
            Err(FamilySectorInventoryError::ReplayMismatch)
        }
    }

    pub(crate) fn payload_eq(&self, other: &Self) -> bool {
        self.schema == other.schema
            && self.family_fingerprint == other.family_fingerprint
            && self.symanzik_g_fingerprint == other.symanzik_g_fingerprint
            && self.restrictions == other.restrictions
            && self.power_shift_policy == other.power_shift_policy
            && self.power_shift_policy_id == other.power_shift_policy_id
            && self.power_support == other.power_support
            && self.zero_sector_domain == other.zero_sector_domain
            && self.ordering == other.ordering
            && self.entries == other.entries
            && self.unresolved_solve_order == other.unresolved_solve_order
            && self.monotone_zero_closure_verified == other.monotone_zero_closure_verified
            && self.limits == other.limits
            && self.stats == other.stats
    }
}

pub struct FamilySectorInventoryCompiler;

impl FamilySectorInventoryCompiler {
    pub fn compile(
        family: &IntegralFamily,
        restrictions: SectorRestrictions,
        power_shift_policy: PowerShiftPolicy,
        ordering: IntegralOrderingPolicy,
        limits: FamilySectorInventoryLimits,
    ) -> Result<FamilySectorInventoryCertificate, FamilySectorInventoryError> {
        if restrictions.arity() != family.denominator_count() {
            return Err(FamilySectorInventoryError::WrongRestrictionsArity {
                expected: family.denominator_count(),
                actual: restrictions.arity(),
            });
        }
        let raw_sector_count = raw_sector_count(family.denominator_count())?;
        check_limit(
            "family sector inventory entries",
            raw_sector_count,
            limits.max_inventory_entries,
        )?;

        // One analyzer construction and exactly one cached all-sector pass.
        let analyzer = ZeroSectorAnalyzer::try_new_with_limits(
            family,
            restrictions.clone(),
            power_shift_policy,
            limits.zero_sectors,
        )?;
        let analysis = analyzer.analyze_all()?;
        if analysis.decisions().len() != raw_sector_count
            || analysis.family_fingerprint() != family.fingerprint()
        {
            return Err(FamilySectorInventoryError::InternalInvariant(
                "all-sector analyzer returned an incomplete or foreign inventory",
            ));
        }

        let mut stats = FamilySectorInventoryStats {
            inventory_entries: analysis.decisions().len(),
            distinct_effective_masks: analysis.distinct_effective_mask_count(),
            ..FamilySectorInventoryStats::default()
        };
        let mut entries = Vec::with_capacity(analysis.decisions().len());
        for (sector, decision) in analysis.decisions() {
            let status = FamilySectorInventoryStatus::from(decision.clone());
            match &status {
                FamilySectorInventoryStatus::Excluded(_) => stats.excluded += 1,
                FamilySectorInventoryStatus::ProvedZero(_) => stats.proved_zero += 1,
                FamilySectorInventoryStatus::UnresolvedNoZeroCertificate(_) => {
                    stats.unresolved += 1
                }
                FamilySectorInventoryStatus::ResourceLimited(_) => stats.resource_limited += 1,
                FamilySectorInventoryStatus::Failed(_) => stats.failed += 1,
            }
            entries.push(FamilySectorInventoryEntry {
                sector: sector.clone(),
                status,
            });
        }
        if !entries
            .windows(2)
            .all(|pair| pair[0].sector < pair[1].sector)
        {
            return Err(FamilySectorInventoryError::InternalInvariant(
                "inventory masks are not strictly ordered",
            ));
        }

        let unresolved_count = stats.unresolved;
        check_limit(
            "unresolved sector solve entries",
            unresolved_count,
            limits.max_unresolved_solve_entries,
        )?;
        let mut unresolved_solve_order = entries
            .iter()
            .filter(|entry| {
                matches!(
                    entry.status,
                    FamilySectorInventoryStatus::UnresolvedNoZeroCertificate(_)
                )
            })
            .map(|entry| {
                let corner_key = ordering.complexity_key(&entry.sector.corner_indices())?;
                Ok(UnresolvedSectorSolveOrderEntry {
                    sector: entry.sector.clone(),
                    corner_key,
                })
            })
            .collect::<Result<Vec<_>, SectorFoundationError>>()?;
        unresolved_solve_order.sort_by(|left, right| {
            left.corner_key
                .cmp(&right.corner_key)
                .then_with(|| left.sector.cmp(&right.sector))
        });

        let dependency_checks = pair_count(unresolved_solve_order.len())?;
        check_limit(
            "unresolved sector dependency checks",
            dependency_checks,
            limits.max_dependency_checks,
        )?;
        verify_unresolved_solve_order(&unresolved_solve_order)?;
        stats.unresolved_solve_entries = unresolved_solve_order.len();
        stats.dependency_checks = dependency_checks;

        let symanzik_g_fingerprint = analyzer.symanzik().g().stable_string();
        let family_fingerprint = family.fingerprint();
        let power_support = analyzer.power_support().clone();
        let binding_bytes = binding_bytes(
            &family_fingerprint,
            &symanzik_g_fingerprint,
            &restrictions,
            &power_support,
            ordering,
            power_shift_policy,
        )?;
        check_limit(
            "family sector inventory binding bytes",
            binding_bytes,
            limits.max_binding_bytes,
        )?;
        stats.binding_bytes = binding_bytes;

        let certificate = FamilySectorInventoryCertificate {
            schema: FAMILY_SECTOR_INVENTORY_V1_SCHEMA,
            family_fingerprint: family_fingerprint.into(),
            symanzik_g_fingerprint: symanzik_g_fingerprint.into(),
            restrictions,
            power_shift_policy,
            power_shift_policy_id: power_shift_policy_id(power_shift_policy),
            power_support,
            zero_sector_domain: analyzer.domain().clone(),
            ordering,
            entries: entries.into_boxed_slice(),
            unresolved_solve_order: unresolved_solve_order.into_boxed_slice(),
            monotone_zero_closure_verified: analysis.monotone_zero_closure_verified(),
            limits,
            stats,
        };
        // Verify the constructed transcript without rerunning the analyzer.
        certificate.verify_payload()?;
        Ok(certificate)
    }
}

impl FamilySectorInventoryCertificate {
    fn verify_payload(&self) -> Result<(), FamilySectorInventoryError> {
        if self.entries.len() != self.stats.inventory_entries
            || self.unresolved_solve_order.len() != self.stats.unresolved_solve_entries
            || self.stats.unresolved != self.unresolved_solve_order.len()
            || self.entries.len()
                != self.stats.excluded
                    + self.stats.proved_zero
                    + self.stats.unresolved
                    + self.stats.resource_limited
                    + self.stats.failed
            || !self
                .entries
                .windows(2)
                .all(|pair| pair[0].sector < pair[1].sector)
        {
            return Err(FamilySectorInventoryError::InternalInvariant(
                "inventory census or ordering mismatch",
            ));
        }
        let recomputed_checks = pair_count(self.unresolved_solve_order.len())?;
        if recomputed_checks != self.stats.dependency_checks {
            return Err(FamilySectorInventoryError::InternalInvariant(
                "dependency-check census mismatch",
            ));
        }
        verify_unresolved_solve_order(&self.unresolved_solve_order)?;
        for solve in &self.unresolved_solve_order {
            if !matches!(
                self.status(&solve.sector),
                Some(FamilySectorInventoryStatus::UnresolvedNoZeroCertificate(_))
            ) || self
                .ordering
                .complexity_key(&solve.sector.corner_indices())?
                != solve.corner_key
            {
                return Err(FamilySectorInventoryError::InternalInvariant(
                    "solve queue is not bound to unresolved entries and exact corner keys",
                ));
            }
        }
        Ok(())
    }
}

fn verify_unresolved_solve_order(
    entries: &[UnresolvedSectorSolveOrderEntry],
) -> Result<(), FamilySectorInventoryError> {
    if !entries
        .windows(2)
        .all(|pair| pair[0].corner_key < pair[1].corner_key)
    {
        return Err(FamilySectorInventoryError::InvalidSolveOrder);
    }
    for earlier in 0..entries.len() {
        for later in earlier + 1..entries.len() {
            if entries[later]
                .sector
                .is_strict_subsector_of(&entries[earlier].sector)?
            {
                return Err(FamilySectorInventoryError::InvalidSolveOrder);
            }
        }
    }
    Ok(())
}

fn raw_sector_count(arity: usize) -> Result<usize, FamilySectorInventoryError> {
    if arity >= usize::BITS as usize {
        return Err(FamilySectorInventoryError::ResourceCountOverflow {
            resource: "family sector inventory entries",
        });
    }
    1usize
        .checked_shl(arity as u32)
        .ok_or(FamilySectorInventoryError::ResourceCountOverflow {
            resource: "family sector inventory entries",
        })
}

fn pair_count(count: usize) -> Result<usize, FamilySectorInventoryError> {
    count
        .checked_mul(count.saturating_sub(1))
        .and_then(|ordered| ordered.checked_div(2))
        .ok_or(FamilySectorInventoryError::ResourceCountOverflow {
            resource: "unresolved sector dependency checks",
        })
}

fn binding_bytes(
    family_fingerprint: &str,
    g_fingerprint: &str,
    restrictions: &SectorRestrictions,
    power_support: &SectorMask,
    ordering: IntegralOrderingPolicy,
    power_shift_policy: PowerShiftPolicy,
) -> Result<usize, FamilySectorInventoryError> {
    [
        FAMILY_SECTOR_INVENTORY_V1_SCHEMA.len(),
        family_fingerprint.len(),
        g_fingerprint.len(),
        restrictions.cuts().to_bit_string().len(),
        restrictions.pattern().to_stable_string().len(),
        power_support.to_bit_string().len(),
        ordering.stable_id().len(),
        power_shift_policy_id(power_shift_policy).len(),
    ]
    .into_iter()
    .try_fold(0usize, |total, bytes| {
        total
            .checked_add(bytes)
            .ok_or(FamilySectorInventoryError::ResourceCountOverflow {
                resource: "family sector inventory binding bytes",
            })
    })
}

const fn power_shift_policy_id(policy: PowerShiftPolicy) -> &'static str {
    match policy {
        PowerShiftPolicy::FormalGeneric => FORMAL_GENERIC_POWER_SHIFT_POLICY_V1_ID,
    }
}

fn check_limit(
    resource: &'static str,
    requested: usize,
    limit: usize,
) -> Result<(), FamilySectorInventoryError> {
    if requested > limit {
        Err(FamilySectorInventoryError::ResourceLimit {
            resource,
            requested,
            limit,
        })
    } else {
        Ok(())
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum FamilySectorInventoryError {
    WrongRestrictionsArity {
        expected: usize,
        actual: usize,
    },
    ForeignFamily,
    SchemaMismatch,
    ReplayMismatch,
    InvalidSolveOrder,
    InternalInvariant(&'static str),
    ResourceLimit {
        resource: &'static str,
        requested: usize,
        limit: usize,
    },
    ResourceCountOverflow {
        resource: &'static str,
    },
    ZeroSector(ZeroSectorError),
    Sector(SectorFoundationError),
}

impl fmt::Display for FamilySectorInventoryError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::WrongRestrictionsArity { expected, actual } => write!(
                formatter,
                "sector restrictions have arity {actual}, expected {expected}"
            ),
            Self::ForeignFamily => formatter.write_str("sector inventory belongs to another family"),
            Self::SchemaMismatch => formatter.write_str("sector inventory schema mismatch"),
            Self::ReplayMismatch => formatter.write_str("sector inventory replay mismatch"),
            Self::InvalidSolveOrder => formatter.write_str(
                "unresolved sector solve order violates the exact ordering or subsector dependencies",
            ),
            Self::InternalInvariant(detail) => {
                write!(formatter, "sector inventory invariant failed: {detail}")
            }
            Self::ResourceLimit {
                resource,
                requested,
                limit,
            } => write!(
                formatter,
                "{resource} needs {requested} units, exceeding the configured limit {limit}"
            ),
            Self::ResourceCountOverflow { resource } => {
                write!(formatter, "{resource} count overflowed usize")
            }
            Self::ZeroSector(error) => error.fmt(formatter),
            Self::Sector(error) => error.fmt(formatter),
        }
    }
}

impl std::error::Error for FamilySectorInventoryError {}

impl From<ZeroSectorError> for FamilySectorInventoryError {
    fn from(value: ZeroSectorError) -> Self {
        Self::ZeroSector(value)
    }
}

impl From<SectorFoundationError> for FamilySectorInventoryError {
    fn from(value: SectorFoundationError) -> Self {
        Self::Sector(value)
    }
}

#[cfg(test)]
mod replay_tamper_tests {
    use super::*;
    use crate::{AffineDenominator, CutConstraint, SectorPattern, algebra::CoefficientContext};

    fn tadpole() -> IntegralFamily {
        let coefficients = CoefficientContext::new(["d", "m2"]);
        IntegralFamily::new(
            "family-sector-inventory-tamper",
            vec!["k".into()],
            Vec::new(),
            coefficients.clone(),
            coefficients.parameter("d").unwrap(),
            vec![AffineDenominator::new(
                coefficients.parse("-m2").unwrap(),
                vec![coefficients.one()],
            )],
            Vec::new(),
            vec![coefficients.zero()],
        )
        .unwrap()
    }

    fn certificate() -> (IntegralFamily, FamilySectorInventoryCertificate) {
        let family = tadpole();
        let restrictions = SectorRestrictions::try_new(
            CutConstraint::none(1).unwrap(),
            SectorPattern::any(1).unwrap(),
        )
        .unwrap();
        let certificate = FamilySectorInventoryCompiler::compile(
            &family,
            restrictions,
            PowerShiftPolicy::FormalGeneric,
            IntegralOrderingPolicy::RustRedUnshiftedV1,
            FamilySectorInventoryLimits::default(),
        )
        .unwrap();
        (family, certificate)
    }

    #[test]
    fn replay_rejects_status_order_and_policy_binding_tampering() {
        let (family, certificate) = certificate();
        certificate.replay(&family).unwrap();

        let mut status = certificate.clone();
        status.entries[0].status =
            FamilySectorInventoryStatus::Failed(ZeroSectorError::CertificateReplayFailure {
                detail: "forged".to_owned(),
            });
        assert!(matches!(
            status.replay(&family),
            Err(FamilySectorInventoryError::ReplayMismatch)
        ));

        let mut order = certificate.clone();
        order.unresolved_solve_order[0].sector = SectorMask::try_from_bit_string("0").unwrap();
        assert!(matches!(
            order.replay(&family),
            Err(FamilySectorInventoryError::ReplayMismatch)
        ));

        let mut policy = certificate.clone();
        policy.power_shift_policy_id = "forged-policy";
        assert!(matches!(
            policy.replay(&family),
            Err(FamilySectorInventoryError::ReplayMismatch)
        ));
    }
}
