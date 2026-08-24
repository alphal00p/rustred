//! Proof-bearing LiteRed-style zero-sector analysis.
//!
//! The production predicate is loop-count and topology independent.  For an
//! effective sector face it extracts the exponent rows of `G = U + F` and
//! performs LiteRed's rank test exactly over `Q`.  A zero result carries a
//! primitive integer right-kernel that is replayed before it is returned.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::sync::Arc;

use symbolica::domains::rational::RationalField;
use symbolica::prelude::*;

use crate::generic_family::BasePolynomial as FamilyBasePolynomial;
use crate::{
    Coefficient, CoefficientLocation, ExactAlgebraError, FeynmanPolynomialError,
    FeynmanPolynomialLimits, GuardOrigin, IntegralFamily, SectorExclusion, SectorFoundationError,
    SectorMask, SectorRestrictions, SymanzikPolynomials,
};

pub const ZERO_SECTOR_CERTIFICATE_SCHEMA: &str = "rustred.zero-sector-certificate.v1";

/// Semantics used for nonzero power shifts during sector analysis.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum PowerShiftPolicy {
    /// A nonzero, nonintegral shift is a formal regulator.  Its support is
    /// included on the generic locus where its numerator is nonzero.
    FormalGeneric,
}

/// Checked construction, enumeration, and rank budgets.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ZeroSectorLimits {
    pub feynman: FeynmanPolynomialLimits,
    pub max_sectors: usize,
    pub max_effective_masks: usize,
    pub max_rank_rows: usize,
    pub max_rank_columns: usize,
    pub max_rank_entries: usize,
    pub max_rank_operations: usize,
    pub max_rref_integer_bits: usize,
    pub max_certificate_entries: usize,
    pub max_kernel_integer_bits: usize,
    pub max_power_shift_pair_checks: usize,
}

impl Default for ZeroSectorLimits {
    fn default() -> Self {
        Self {
            feynman: FeynmanPolynomialLimits::default(),
            max_sectors: 1_048_576,
            max_effective_masks: 1_048_576,
            max_rank_rows: 4_000_000,
            max_rank_columns: 4_097,
            max_rank_entries: 16_000_000,
            max_rank_operations: 64_000_000,
            max_rref_integer_bits: 1_000_000,
            max_certificate_entries: 4_097,
            max_kernel_integer_bits: 1_000_000,
            max_power_shift_pair_checks: 8_388_608,
        }
    }
}

/// Typed failures at the proof boundary.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ZeroSectorError {
    WrongRestrictionsArity {
        expected: usize,
        actual: usize,
    },
    UnsupportedNonzeroIntegerPowerShift {
        denominator: usize,
    },
    UnsupportedShiftedCut {
        denominator: usize,
    },
    UnsupportedIntegerSeparatedPowerShifts {
        left: usize,
        right: usize,
    },
    ResourceLimit {
        resource: &'static str,
        requested: usize,
        limit: usize,
    },
    ResourceCountOverflow {
        resource: &'static str,
    },
    MatrixDimensionOverflow {
        rows: usize,
        columns: usize,
    },
    MatrixShape {
        detail: String,
    },
    ForeignCertificateFamily,
    CertificateSchemaMismatch,
    CertificateReplayFailure {
        detail: String,
    },
    ExactAlgebra(ExactAlgebraError),
    Feynman(FeynmanPolynomialError),
    Sector(SectorFoundationError),
    SymbolicaPanic,
}

impl fmt::Display for ZeroSectorError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::WrongRestrictionsArity { expected, actual } => write!(
                formatter,
                "sector restrictions have arity {actual}, expected {expected}"
            ),
            Self::UnsupportedNonzeroIntegerPowerShift { denominator } => write!(
                formatter,
                "power shift {denominator} is a known nonzero integer; formal-generic sector support is unsound for integer reindexing"
            ),
            Self::UnsupportedShiftedCut { denominator } => write!(
                formatter,
                "cut denominator {denominator} has a nonzero power shift; shifted-cut semantics are not defined"
            ),
            Self::UnsupportedIntegerSeparatedPowerShifts { left, right } => write!(
                formatter,
                "power shifts {left} and {right} differ by a known nonzero integer"
            ),
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
            Self::MatrixDimensionOverflow { rows, columns } => write!(
                formatter,
                "rank matrix shape {rows} x {columns} cannot be represented safely"
            ),
            Self::MatrixShape { detail } => write!(formatter, "invalid rank matrix: {detail}"),
            Self::ForeignCertificateFamily => {
                formatter.write_str("zero-sector certificate belongs to a foreign family")
            }
            Self::CertificateSchemaMismatch => {
                formatter.write_str("zero-sector certificate schema is unsupported")
            }
            Self::CertificateReplayFailure { detail } => {
                write!(formatter, "zero-sector certificate replay failed: {detail}")
            }
            Self::ExactAlgebra(error) => {
                write!(formatter, "exact power-shift algebra failed: {error}")
            }
            Self::Feynman(error) => {
                write!(formatter, "Feynman-polynomial construction failed: {error}")
            }
            Self::Sector(error) => write!(formatter, "sector foundation failed: {error}"),
            Self::SymbolicaPanic => {
                formatter.write_str("Symbolica panicked during checked zero-sector analysis")
            }
        }
    }
}

impl std::error::Error for ZeroSectorError {}

impl From<ExactAlgebraError> for ZeroSectorError {
    fn from(value: ExactAlgebraError) -> Self {
        Self::ExactAlgebra(value)
    }
}

impl From<FeynmanPolynomialError> for ZeroSectorError {
    fn from(value: FeynmanPolynomialError) -> Self {
        Self::Feynman(value)
    }
}

impl From<SectorFoundationError> for ZeroSectorError {
    fn from(value: SectorFoundationError) -> Self {
        Self::Sector(value)
    }
}

/// Why one generic-domain condition is present.
#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum ZeroSectorConditionSource {
    Family(CoefficientLocation),
    PowerShiftSupport { denominator: usize },
}

/// One exact polynomial required to remain nonzero.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ZeroSectorDomainCondition {
    polynomial: FamilyBasePolynomial,
    sources: BTreeSet<ZeroSectorConditionSource>,
    origins: BTreeSet<GuardOrigin>,
}

impl ZeroSectorDomainCondition {
    pub fn polynomial(&self) -> &FamilyBasePolynomial {
        &self.polynomial
    }

    pub fn sources(&self) -> &BTreeSet<ZeroSectorConditionSource> {
        &self.sources
    }

    pub fn origins(&self) -> &BTreeSet<GuardOrigin> {
        &self.origins
    }
}

/// Generic locus on which the family and effective power support are valid.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ZeroSectorDomain {
    conditions: Vec<ZeroSectorDomainCondition>,
}

impl ZeroSectorDomain {
    pub fn conditions(&self) -> &[ZeroSectorDomainCondition] {
        &self.conditions
    }

    fn insert(
        &mut self,
        polynomial: FamilyBasePolynomial,
        source: ZeroSectorConditionSource,
        origins: BTreeSet<GuardOrigin>,
    ) {
        if let Some(condition) = self
            .conditions
            .iter_mut()
            .find(|condition| condition.polynomial == polynomial)
        {
            condition.sources.insert(source);
            condition.origins.extend(origins);
        } else {
            self.conditions.push(ZeroSectorDomainCondition {
                polynomial,
                sources: BTreeSet::from([source]),
                origins,
            });
        }
    }
}

/// Bounded work that prevented one effective-mask decision.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ZeroSectorResource {
    resource: &'static str,
    requested: usize,
    limit: usize,
}

impl ZeroSectorResource {
    pub fn resource(&self) -> &'static str {
        self.resource
    }

    pub fn requested(&self) -> usize {
        self.requested
    }

    pub fn limit(&self) -> usize {
        self.limit
    }
}

/// Replayable sufficient proof that one raw sector is zero.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ZeroSectorCertificate {
    schema: &'static str,
    family_fingerprint: Arc<str>,
    g_fingerprint: Arc<str>,
    raw_sector: SectorMask,
    effective_sector: SectorMask,
    active_parameter_order: Box<[usize]>,
    primitive_kernel: Box<[Integer]>,
    rank: usize,
    exponent_row_count: usize,
    domain: ZeroSectorDomain,
    policy: PowerShiftPolicy,
}

impl ZeroSectorCertificate {
    pub fn schema(&self) -> &'static str {
        self.schema
    }

    pub fn family_fingerprint(&self) -> &str {
        &self.family_fingerprint
    }

    pub fn g_fingerprint(&self) -> &str {
        &self.g_fingerprint
    }

    pub fn raw_sector(&self) -> &SectorMask {
        &self.raw_sector
    }

    pub fn effective_sector(&self) -> &SectorMask {
        &self.effective_sector
    }

    pub fn active_parameter_order(&self) -> &[usize] {
        &self.active_parameter_order
    }

    pub fn primitive_kernel(&self) -> &[Integer] {
        &self.primitive_kernel
    }

    pub fn rank(&self) -> usize {
        self.rank
    }

    pub fn exponent_row_count(&self) -> usize {
        self.exponent_row_count
    }

    pub fn domain(&self) -> &ZeroSectorDomain {
        &self.domain
    }

    pub fn policy(&self) -> PowerShiftPolicy {
        self.policy
    }

    /// Reconstruct `G` and replay this certificate with default limits.
    pub fn replay(&self, family: &IntegralFamily) -> Result<(), ZeroSectorError> {
        self.replay_with_limits(family, ZeroSectorLimits::default())
    }

    /// Reconstruct `G` and replay this certificate with explicit limits.
    pub fn replay_with_limits(
        &self,
        family: &IntegralFamily,
        limits: ZeroSectorLimits,
    ) -> Result<(), ZeroSectorError> {
        catch_unwind(AssertUnwindSafe(|| {
            let analyzer = ZeroSectorAnalyzer::build_unrestricted(family, self.policy, limits)?;
            analyzer.replay_certificate_inner(self)
        }))
        .map_err(|_| ZeroSectorError::SymbolicaPanic)?
    }
}

/// Diagnostic full-column-rank result.  It means only that this sufficient
/// zero test did not produce a certificate; it is not a nonzero proof.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FullColumnRankWitness {
    raw_sector: SectorMask,
    effective_sector: SectorMask,
    active_parameter_order: Box<[usize]>,
    rank: usize,
    exponent_row_count: usize,
    column_count: usize,
}

impl FullColumnRankWitness {
    pub fn raw_sector(&self) -> &SectorMask {
        &self.raw_sector
    }

    pub fn effective_sector(&self) -> &SectorMask {
        &self.effective_sector
    }

    pub fn active_parameter_order(&self) -> &[usize] {
        &self.active_parameter_order
    }

    pub fn rank(&self) -> usize {
        self.rank
    }

    pub fn exponent_row_count(&self) -> usize {
        self.exponent_row_count
    }

    pub fn column_count(&self) -> usize {
        self.column_count
    }
}

/// Complete semantics for one raw sector.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ZeroSectorDecision {
    Excluded(SectorExclusion),
    ProvedZero(ZeroSectorCertificate),
    NoZeroCertificate(FullColumnRankWitness),
    ResourceLimited(ZeroSectorResource),
    Failed(ZeroSectorError),
}

/// Stable all-sector result.  Every admissible mask was tested directly;
/// monotone closure is checked only as auxiliary metadata.
#[derive(Clone, Debug)]
pub struct ZeroSectorAnalysis {
    family_fingerprint: Arc<str>,
    symanzik: SymanzikPolynomials,
    decisions: Vec<(SectorMask, ZeroSectorDecision)>,
    distinct_effective_masks: usize,
    monotone_zero_closure_verified: bool,
}

impl ZeroSectorAnalysis {
    pub fn family_fingerprint(&self) -> &str {
        &self.family_fingerprint
    }

    pub fn symanzik(&self) -> &SymanzikPolynomials {
        &self.symanzik
    }

    pub fn decisions(&self) -> &[(SectorMask, ZeroSectorDecision)] {
        &self.decisions
    }

    pub fn decision(&self, sector: &SectorMask) -> Option<&ZeroSectorDecision> {
        self.decisions
            .binary_search_by(|(candidate, _)| candidate.cmp(sector))
            .ok()
            .map(|position| &self.decisions[position].1)
    }

    pub fn distinct_effective_mask_count(&self) -> usize {
        self.distinct_effective_masks
    }

    pub fn monotone_zero_closure_verified(&self) -> bool {
        self.monotone_zero_closure_verified
    }
}

/// Generic analyzer constructed once per authenticated family/restriction set.
#[derive(Clone, Debug)]
pub struct ZeroSectorAnalyzer {
    family_fingerprint: Arc<str>,
    symanzik: SymanzikPolynomials,
    restrictions: SectorRestrictions,
    power_support: SectorMask,
    domain: ZeroSectorDomain,
    policy: PowerShiftPolicy,
    limits: ZeroSectorLimits,
}

impl ZeroSectorAnalyzer {
    pub fn try_new(
        family: &IntegralFamily,
        restrictions: SectorRestrictions,
        policy: PowerShiftPolicy,
    ) -> Result<Self, ZeroSectorError> {
        Self::try_new_with_limits(family, restrictions, policy, ZeroSectorLimits::default())
    }

    pub fn try_new_with_limits(
        family: &IntegralFamily,
        restrictions: SectorRestrictions,
        policy: PowerShiftPolicy,
        limits: ZeroSectorLimits,
    ) -> Result<Self, ZeroSectorError> {
        catch_unwind(AssertUnwindSafe(|| {
            Self::build(family, restrictions, policy, limits)
        }))
        .map_err(|_| ZeroSectorError::SymbolicaPanic)?
    }

    pub fn try_unrestricted(
        family: &IntegralFamily,
        policy: PowerShiftPolicy,
    ) -> Result<Self, ZeroSectorError> {
        Self::try_unrestricted_with_limits(family, policy, ZeroSectorLimits::default())
    }

    pub fn try_unrestricted_with_limits(
        family: &IntegralFamily,
        policy: PowerShiftPolicy,
        limits: ZeroSectorLimits,
    ) -> Result<Self, ZeroSectorError> {
        catch_unwind(AssertUnwindSafe(|| {
            Self::build_unrestricted(family, policy, limits)
        }))
        .map_err(|_| ZeroSectorError::SymbolicaPanic)?
    }

    fn build_unrestricted(
        family: &IntegralFamily,
        policy: PowerShiftPolicy,
        limits: ZeroSectorLimits,
    ) -> Result<Self, ZeroSectorError> {
        let restrictions = SectorRestrictions::unrestricted(family.denominator_count())?;
        Self::build(family, restrictions, policy, limits)
    }

    fn build(
        family: &IntegralFamily,
        restrictions: SectorRestrictions,
        policy: PowerShiftPolicy,
        limits: ZeroSectorLimits,
    ) -> Result<Self, ZeroSectorError> {
        if restrictions.arity() != family.denominator_count() {
            return Err(ZeroSectorError::WrongRestrictionsArity {
                expected: family.denominator_count(),
                actual: restrictions.arity(),
            });
        }
        let symanzik = SymanzikPolynomials::try_from_family_with_limits(family, limits.feynman)?;
        let mut domain = ZeroSectorDomain::default();
        for condition in family.domain().conditions() {
            domain.insert(
                condition.polynomial().clone(),
                ZeroSectorConditionSource::Family(condition.source().clone()),
                condition.origins().clone(),
            );
        }

        let mut support = Vec::with_capacity(family.denominator_count());
        for (denominator, shift) in family.power_shifts().iter().enumerate() {
            let nonzero = !shift.is_zero();
            if nonzero && is_known_nonzero_integer(shift) {
                return Err(ZeroSectorError::UnsupportedNonzeroIntegerPowerShift { denominator });
            }
            if nonzero
                && restrictions
                    .cuts()
                    .required_active()
                    .is_active(denominator)?
            {
                return Err(ZeroSectorError::UnsupportedShiftedCut { denominator });
            }
            if nonzero && !shift.numerator.is_constant() {
                let origin = GuardOrigin::PowerShiftSupport { denominator };
                domain.insert(
                    shift.numerator.clone(),
                    ZeroSectorConditionSource::PowerShiftSupport { denominator },
                    BTreeSet::from([origin]),
                );
            }
            support.push(nonzero);
        }

        let shift_pairs = family
            .power_shifts()
            .len()
            .checked_mul(family.power_shifts().len().saturating_sub(1))
            .and_then(|ordered| ordered.checked_div(2))
            .ok_or(ZeroSectorError::ResourceCountOverflow {
                resource: "power-shift pair checks",
            })?;
        check_limit(
            "power-shift pair checks",
            shift_pairs,
            limits.max_power_shift_pair_checks,
        )?;
        for left in 0..family.power_shifts().len() {
            for right in left + 1..family.power_shifts().len() {
                let difference = family.coefficient_context().try_sub(
                    &family.power_shifts()[left],
                    &family.power_shifts()[right],
                    limits.feynman.exact_algebra,
                )?;
                if is_known_nonzero_integer(&difference) {
                    return Err(ZeroSectorError::UnsupportedIntegerSeparatedPowerShifts {
                        left,
                        right,
                    });
                }
            }
        }

        Ok(Self {
            family_fingerprint: Arc::from(family.fingerprint()),
            symanzik,
            restrictions,
            power_support: SectorMask::try_new(support)?,
            domain,
            policy,
            limits,
        })
    }

    pub fn family_fingerprint(&self) -> &str {
        &self.family_fingerprint
    }

    pub fn symanzik(&self) -> &SymanzikPolynomials {
        &self.symanzik
    }

    pub fn restrictions(&self) -> &SectorRestrictions {
        &self.restrictions
    }

    pub fn power_support(&self) -> &SectorMask {
        &self.power_support
    }

    pub fn domain(&self) -> &ZeroSectorDomain {
        &self.domain
    }

    pub fn policy(&self) -> PowerShiftPolicy {
        self.policy
    }

    pub fn limits(&self) -> ZeroSectorLimits {
        self.limits
    }

    pub fn analyze_sector(&self, raw_sector: &SectorMask) -> ZeroSectorDecision {
        match catch_unwind(AssertUnwindSafe(|| self.analyze_sector_inner(raw_sector))) {
            Ok(Ok(decision)) => decision,
            Ok(Err(error)) => decision_from_error(error),
            Err(_) => ZeroSectorDecision::Failed(ZeroSectorError::SymbolicaPanic),
        }
    }

    fn analyze_sector_inner(
        &self,
        raw_sector: &SectorMask,
    ) -> Result<ZeroSectorDecision, ZeroSectorError> {
        if let Some(exclusion) = self.restrictions.exclusion(raw_sector)? {
            return Ok(ZeroSectorDecision::Excluded(exclusion));
        }
        let effective_sector = self.effective_sector(raw_sector)?;
        let effective = self.compute_effective_checked(&effective_sector);
        Ok(self.bind_effective(raw_sector, &effective))
    }

    pub fn analyze_all(&self) -> Result<ZeroSectorAnalysis, ZeroSectorError> {
        catch_unwind(AssertUnwindSafe(|| self.analyze_all_inner()))
            .map_err(|_| ZeroSectorError::SymbolicaPanic)?
    }

    fn analyze_all_inner(&self) -> Result<ZeroSectorAnalysis, ZeroSectorError> {
        let arity = self.power_support.arity();
        if arity >= usize::BITS as usize {
            return Err(ZeroSectorError::ResourceCountOverflow {
                resource: "raw sector count",
            });
        }
        let sector_count =
            1_usize
                .checked_shl(arity as u32)
                .ok_or(ZeroSectorError::ResourceCountOverflow {
                    resource: "raw sector count",
                })?;
        check_limit("raw sectors", sector_count, self.limits.max_sectors)?;

        let mut decisions = Vec::with_capacity(sector_count);
        let mut cache = BTreeMap::<SectorMask, EffectiveRankDecision>::new();
        for value in 0..sector_count {
            let bits = (0..arity)
                .map(|position| value & (1_usize << (arity - 1 - position)) != 0)
                .collect::<Vec<_>>();
            let raw_sector = SectorMask::try_new(bits)?;
            if let Some(exclusion) = self.restrictions.exclusion(&raw_sector)? {
                decisions.push((raw_sector, ZeroSectorDecision::Excluded(exclusion)));
                continue;
            }
            let effective_sector = self.effective_sector(&raw_sector)?;
            if !cache.contains_key(&effective_sector) {
                let requested = checked_add(cache.len(), 1, "effective mask cache")?;
                if requested > self.limits.max_effective_masks {
                    return Err(ZeroSectorError::ResourceLimit {
                        resource: "effective mask cache",
                        requested,
                        limit: self.limits.max_effective_masks,
                    });
                }
                let computed = self.compute_effective_checked(&effective_sector);
                cache.insert(effective_sector.clone(), computed);
            }
            let effective = cache.get(&effective_sector).ok_or_else(|| {
                ZeroSectorError::CertificateReplayFailure {
                    detail: "effective-mask cache insertion was lost".to_owned(),
                }
            })?;
            decisions.push((
                raw_sector.clone(),
                self.bind_effective(&raw_sector, effective),
            ));
        }

        let monotone_zero_closure_verified = verify_direct_monotone_closure(&decisions)?;
        Ok(ZeroSectorAnalysis {
            family_fingerprint: self.family_fingerprint.clone(),
            symanzik: self.symanzik.clone(),
            decisions,
            distinct_effective_masks: cache.len(),
            monotone_zero_closure_verified,
        })
    }

    fn effective_sector(&self, raw_sector: &SectorMask) -> Result<SectorMask, ZeroSectorError> {
        if raw_sector.arity() != self.power_support.arity() {
            return Err(ZeroSectorError::Sector(SectorFoundationError::WrongArity {
                expected: self.power_support.arity(),
                actual: raw_sector.arity(),
            }));
        }
        Ok(SectorMask::try_new(
            raw_sector
                .active_bits()
                .iter()
                .zip(self.power_support.active_bits())
                .map(|(&raw, &shifted)| raw || shifted),
        )?)
    }

    fn compute_effective_checked(&self, effective: &SectorMask) -> EffectiveRankDecision {
        match catch_unwind(AssertUnwindSafe(|| self.compute_effective(effective))) {
            Ok(Ok(decision)) => decision,
            Ok(Err(ZeroSectorError::ResourceLimit {
                resource,
                requested,
                limit,
            })) => EffectiveRankDecision::Resource(ZeroSectorResource {
                resource,
                requested,
                limit,
            }),
            Ok(Err(error)) => EffectiveRankDecision::Failed(error),
            Err(_) => EffectiveRankDecision::Failed(ZeroSectorError::SymbolicaPanic),
        }
    }

    fn compute_effective(
        &self,
        effective: &SectorMask,
    ) -> Result<EffectiveRankDecision, ZeroSectorError> {
        let matrix = self.exponent_matrix(effective)?;
        if matrix.rows.is_empty() {
            check_limit(
                "certificate kernel entries",
                matrix.columns,
                self.limits.max_certificate_entries,
            )?;
            check_limit(
                "certificate kernel integer bits",
                1,
                self.limits.max_kernel_integer_bits,
            )?;
            let mut kernel = vec![Integer::zero(); matrix.columns];
            if let Some(first) = kernel.first_mut() {
                *first = Integer::one();
            }
            return Ok(EffectiveRankDecision::Zero {
                active_parameter_order: matrix.active_parameter_order,
                primitive_kernel: kernel.into_boxed_slice(),
                rank: 0,
                exponent_row_count: 0,
            });
        }

        let rank_operations = checked_mul(
            checked_mul(matrix.rows.len(), matrix.columns, "rank operations")?,
            matrix.rows.len().min(matrix.columns),
            "rank operations",
        )?;
        check_limit(
            "rank operations",
            rank_operations,
            self.limits.max_rank_operations,
        )?;
        let minor_bit_bound = matrix.preflight_rref_bits(self.limits.max_rref_integer_bits)?;
        let row_count = matrix.rows.len();
        let mut reduced = matrix.to_symbolica_matrix()?;
        let rank = reduced.row_reduce(matrix.columns as u32);
        validate_rational_matrix_bits(&reduced, self.limits.max_rref_integer_bits)?;
        if rank == matrix.columns {
            return Ok(EffectiveRankDecision::Full {
                active_parameter_order: matrix.active_parameter_order,
                rank,
                exponent_row_count: row_count,
                column_count: matrix.columns,
            });
        }
        check_limit(
            "certificate kernel entries",
            matrix.columns,
            self.limits.max_certificate_entries,
        )?;
        let kernel_bit_bound = checked_mul(
            matrix.columns,
            minor_bit_bound,
            "certificate kernel integer bit bound",
        )?;
        check_limit(
            "certificate kernel integer bits",
            kernel_bit_bound,
            self.limits.max_kernel_integer_bits,
        )?;
        let rational_kernel = deterministic_rref_kernel(&reduced, rank, matrix.columns)?;
        let primitive_kernel =
            primitive_integer_kernel(&rational_kernel, self.limits.max_kernel_integer_bits)?;
        replay_integer_kernel(&matrix.rows, &primitive_kernel)?;
        Ok(EffectiveRankDecision::Zero {
            active_parameter_order: matrix.active_parameter_order,
            primitive_kernel: primitive_kernel.into_boxed_slice(),
            rank,
            exponent_row_count: row_count,
        })
    }

    fn exponent_matrix(&self, effective: &SectorMask) -> Result<ExponentMatrix, ZeroSectorError> {
        if effective.arity() != self.symanzik.context().parameter_count() {
            return Err(ZeroSectorError::Sector(SectorFoundationError::WrongArity {
                expected: self.symanzik.context().parameter_count(),
                actual: effective.arity(),
            }));
        }
        let active_parameter_order = effective
            .active_bits()
            .iter()
            .enumerate()
            .filter_map(|(parameter, &active)| active.then_some(parameter))
            .collect::<Vec<_>>();
        let columns = checked_add(active_parameter_order.len(), 1, "rank matrix columns")?;
        check_limit("rank matrix columns", columns, self.limits.max_rank_columns)?;
        let mut rows = Vec::new();
        for (_, exponents) in self.symanzik.g().terms() {
            if exponents
                .iter()
                .zip(effective.active_bits())
                .any(|(&exponent, &active)| exponent > 0 && !active)
            {
                continue;
            }
            let requested = checked_add(rows.len(), 1, "rank matrix rows")?;
            check_limit("rank matrix rows", requested, self.limits.max_rank_rows)?;
            let mut row = Vec::with_capacity(columns);
            row.extend(
                active_parameter_order
                    .iter()
                    .map(|&parameter| exponents[parameter]),
            );
            row.push(1);
            rows.push(row);
        }
        let entries = checked_mul(rows.len(), columns, "rank matrix entries")?;
        check_limit("rank matrix entries", entries, self.limits.max_rank_entries)?;
        if rows.len() > u32::MAX as usize
            || columns > u32::MAX as usize
            || entries > u32::MAX as usize
        {
            return Err(ZeroSectorError::MatrixDimensionOverflow {
                rows: rows.len(),
                columns,
            });
        }
        Ok(ExponentMatrix {
            rows,
            active_parameter_order: active_parameter_order.into_boxed_slice(),
            columns,
        })
    }

    fn bind_effective(
        &self,
        raw_sector: &SectorMask,
        effective: &EffectiveRankDecision,
    ) -> ZeroSectorDecision {
        let effective_sector = match self.effective_sector(raw_sector) {
            Ok(sector) => sector,
            Err(error) => return ZeroSectorDecision::Failed(error),
        };
        match effective {
            EffectiveRankDecision::Zero {
                active_parameter_order,
                primitive_kernel,
                rank,
                exponent_row_count,
            } => {
                let certificate = ZeroSectorCertificate {
                    schema: ZERO_SECTOR_CERTIFICATE_SCHEMA,
                    family_fingerprint: self.family_fingerprint.clone(),
                    g_fingerprint: Arc::from(self.symanzik.g().stable_string()),
                    raw_sector: raw_sector.clone(),
                    effective_sector,
                    active_parameter_order: active_parameter_order.clone(),
                    primitive_kernel: primitive_kernel.clone(),
                    rank: *rank,
                    exponent_row_count: *exponent_row_count,
                    domain: self.domain.clone(),
                    policy: self.policy,
                };
                match self.verify_bound_certificate(&certificate) {
                    Ok(()) => ZeroSectorDecision::ProvedZero(certificate),
                    Err(error) => ZeroSectorDecision::Failed(error),
                }
            }
            EffectiveRankDecision::Full {
                active_parameter_order,
                rank,
                exponent_row_count,
                column_count,
            } => ZeroSectorDecision::NoZeroCertificate(FullColumnRankWitness {
                raw_sector: raw_sector.clone(),
                effective_sector,
                active_parameter_order: active_parameter_order.clone(),
                rank: *rank,
                exponent_row_count: *exponent_row_count,
                column_count: *column_count,
            }),
            EffectiveRankDecision::Resource(resource) => {
                ZeroSectorDecision::ResourceLimited(resource.clone())
            }
            EffectiveRankDecision::Failed(error) => ZeroSectorDecision::Failed(error.clone()),
        }
    }

    fn verify_bound_certificate(
        &self,
        certificate: &ZeroSectorCertificate,
    ) -> Result<(), ZeroSectorError> {
        if certificate.family_fingerprint.as_ref() != self.family_fingerprint.as_ref() {
            return Err(ZeroSectorError::ForeignCertificateFamily);
        }
        if certificate.schema != ZERO_SECTOR_CERTIFICATE_SCHEMA {
            return Err(ZeroSectorError::CertificateSchemaMismatch);
        }
        if certificate.g_fingerprint.as_ref() != self.symanzik.g().stable_string() {
            return Err(ZeroSectorError::CertificateReplayFailure {
                detail: "G fingerprint changed".to_owned(),
            });
        }
        if self.effective_sector(&certificate.raw_sector)? != certificate.effective_sector {
            return Err(ZeroSectorError::CertificateReplayFailure {
                detail: "effective support does not match raw sector and power shifts".to_owned(),
            });
        }
        if certificate.domain != self.domain {
            return Err(ZeroSectorError::CertificateReplayFailure {
                detail: "generic-domain guards changed".to_owned(),
            });
        }
        let matrix = self.exponent_matrix(&certificate.effective_sector)?;
        if matrix.active_parameter_order != certificate.active_parameter_order {
            return Err(ZeroSectorError::CertificateReplayFailure {
                detail: "active parameter order changed".to_owned(),
            });
        }
        if matrix.rows.len() != certificate.exponent_row_count || certificate.rank >= matrix.columns
        {
            return Err(ZeroSectorError::CertificateReplayFailure {
                detail: "stored rank or exponent-row count is inconsistent".to_owned(),
            });
        }
        replay_integer_kernel(&matrix.rows, &certificate.primitive_kernel)
    }

    fn replay_certificate_inner(
        &self,
        certificate: &ZeroSectorCertificate,
    ) -> Result<(), ZeroSectorError> {
        self.verify_bound_certificate(certificate)?;
        let effective = self.compute_effective_checked(&certificate.effective_sector);
        match effective {
            EffectiveRankDecision::Zero {
                active_parameter_order,
                primitive_kernel,
                rank,
                exponent_row_count,
            } if active_parameter_order == certificate.active_parameter_order
                && primitive_kernel == certificate.primitive_kernel
                && rank == certificate.rank
                && exponent_row_count == certificate.exponent_row_count =>
            {
                Ok(())
            }
            EffectiveRankDecision::Resource(resource) => Err(ZeroSectorError::ResourceLimit {
                resource: resource.resource,
                requested: resource.requested,
                limit: resource.limit,
            }),
            EffectiveRankDecision::Failed(error) => Err(error),
            _ => Err(ZeroSectorError::CertificateReplayFailure {
                detail: "recomputed deterministic rank certificate differs".to_owned(),
            }),
        }
    }
}

#[derive(Clone, Debug)]
enum EffectiveRankDecision {
    Zero {
        active_parameter_order: Box<[usize]>,
        primitive_kernel: Box<[Integer]>,
        rank: usize,
        exponent_row_count: usize,
    },
    Full {
        active_parameter_order: Box<[usize]>,
        rank: usize,
        exponent_row_count: usize,
        column_count: usize,
    },
    Resource(ZeroSectorResource),
    Failed(ZeroSectorError),
}

struct ExponentMatrix {
    rows: Vec<Vec<u16>>,
    active_parameter_order: Box<[usize]>,
    columns: usize,
}

impl ExponentMatrix {
    /// Bound exact Gaussian-elimination temporaries from the Leibniz bound
    /// `r! M^r <= (r M)^r` for every integer minor.  Canonical RREF entries
    /// are ratios of minors; one rational product/addition can temporarily
    /// need at most twice that many bits plus carry bits.
    fn preflight_rref_bits(&self, limit: usize) -> Result<usize, ZeroSectorError> {
        let rank_dimension = self.rows.len().min(self.columns);
        let maximum_entry = self.rows.iter().flatten().copied().max().unwrap_or(1);
        let entry_bits = usize::try_from(u16::BITS - maximum_entry.leading_zeros())
            .map_err(|_| ZeroSectorError::ResourceCountOverflow {
                resource: "rank matrix entry bit length",
            })?
            .max(1);
        let dimension_bits = ceil_log2(rank_dimension.max(1));
        let minor_bits = checked_add(
            checked_mul(
                rank_dimension,
                checked_add(entry_bits, dimension_bits, "RREF minor bit bound")?,
                "RREF minor bit bound",
            )?,
            1,
            "RREF minor bit bound",
        )?;
        let temporary_bits = checked_add(
            checked_mul(2, minor_bits, "RREF integer bit bound")?,
            2,
            "RREF integer bit bound",
        )?;
        check_limit("RREF integer bits", temporary_bits, limit)?;
        Ok(minor_bits)
    }

    fn to_symbolica_matrix(&self) -> Result<Matrix<RationalField>, ZeroSectorError> {
        let entries = self
            .rows
            .iter()
            .flatten()
            .map(|&entry| Rational::from(i64::from(entry)))
            .collect::<Vec<_>>();
        Matrix::from_linear(entries, self.rows.len() as u32, self.columns as u32, Q)
            .map_err(|detail| ZeroSectorError::MatrixShape { detail })
    }
}

fn deterministic_rref_kernel(
    reduced: &Matrix<RationalField>,
    rank: usize,
    columns: usize,
) -> Result<Vec<Rational>, ZeroSectorError> {
    let mut pivot_for_row = Vec::with_capacity(rank);
    let mut pivot_columns = vec![false; columns];
    for row in 0..rank {
        let pivot = (0..columns)
            .find(|&column| !Q.is_zero(&reduced[(row as u32, column as u32)]))
            .ok_or_else(|| ZeroSectorError::CertificateReplayFailure {
                detail: format!("RREF row {row} has no pivot"),
            })?;
        if pivot_columns[pivot] {
            return Err(ZeroSectorError::CertificateReplayFailure {
                detail: format!("RREF pivot column {pivot} is repeated"),
            });
        }
        if !Q.is_one(&reduced[(row as u32, pivot as u32)]) {
            return Err(ZeroSectorError::CertificateReplayFailure {
                detail: format!("RREF pivot at row {row}, column {pivot} is not normalized"),
            });
        }
        pivot_columns[pivot] = true;
        pivot_for_row.push(pivot);
    }
    let free = pivot_columns
        .iter()
        .position(|&pivot| !pivot)
        .ok_or_else(|| ZeroSectorError::CertificateReplayFailure {
            detail: "rank-deficient matrix has no free column".to_owned(),
        })?;
    let mut kernel = vec![Rational::zero(); columns];
    kernel[free] = Rational::one();
    for (row, &pivot) in pivot_for_row.iter().enumerate() {
        kernel[pivot] = Q.neg(&reduced[(row as u32, free as u32)]);
    }
    Ok(kernel)
}

fn primitive_integer_kernel(
    kernel: &[Rational],
    max_integer_bits: usize,
) -> Result<Vec<Integer>, ZeroSectorError> {
    let mut common_denominator = Integer::one();
    for value in kernel {
        let denominator = value.denominator_ref();
        let gcd = common_denominator.gcd(denominator);
        let reduced = exact_integer_quotient(&common_denominator, &gcd)?;
        common_denominator = Z.mul(&reduced, denominator);
        check_integer_bits(
            &common_denominator,
            "certificate kernel integer bits",
            max_integer_bits,
        )?;
    }
    let mut integers = Vec::with_capacity(kernel.len());
    for value in kernel {
        let scale = exact_integer_quotient(&common_denominator, value.denominator_ref())?;
        let integer = Z.mul(value.numerator_ref(), &scale);
        check_integer_bits(
            &integer,
            "certificate kernel integer bits",
            max_integer_bits,
        )?;
        integers.push(integer);
    }
    let mut content = Integer::zero();
    for value in &integers {
        if !value.is_zero() {
            content = if content.is_zero() {
                value.abs()
            } else {
                content.gcd(&value.abs())
            };
        }
    }
    if content.is_zero() {
        return Err(ZeroSectorError::CertificateReplayFailure {
            detail: "RREF produced a zero kernel".to_owned(),
        });
    }
    for value in &mut integers {
        *value = exact_integer_quotient(value, &content)?;
        check_integer_bits(value, "certificate kernel integer bits", max_integer_bits)?;
    }
    if integers
        .iter()
        .find(|value| !value.is_zero())
        .is_some_and(Integer::is_negative)
    {
        for value in &mut integers {
            *value = Z.neg(&*value);
        }
    }
    Ok(integers)
}

fn validate_rational_matrix_bits(
    matrix: &Matrix<RationalField>,
    limit: usize,
) -> Result<(), ZeroSectorError> {
    for row in matrix.row_iter() {
        for value in row {
            check_integer_bits(value.numerator_ref(), "RREF integer bits", limit)?;
            check_integer_bits(value.denominator_ref(), "RREF integer bits", limit)?;
        }
    }
    Ok(())
}

fn check_integer_bits(
    integer: &Integer,
    resource: &'static str,
    limit: usize,
) -> Result<(), ZeroSectorError> {
    let requested = integer_bit_length(integer)?;
    check_limit(resource, requested, limit)
}

fn integer_bit_length(integer: &Integer) -> Result<usize, ZeroSectorError> {
    let bits = match integer {
        Integer::Single(value) => u64::from(i64::BITS - value.unsigned_abs().leading_zeros()),
        Integer::Double(value) => u64::from(i128::BITS - value.unsigned_abs().leading_zeros()),
        Integer::Large(value) => u64::from(value.significant_bits()),
    };
    usize::try_from(bits).map_err(|_| ZeroSectorError::ResourceCountOverflow {
        resource: "integer bit length",
    })
}

fn ceil_log2(value: usize) -> usize {
    if value <= 1 {
        0
    } else {
        (usize::BITS - (value - 1).leading_zeros()) as usize
    }
}

fn exact_integer_quotient(
    numerator: &Integer,
    denominator: &Integer,
) -> Result<Integer, ZeroSectorError> {
    if denominator.is_zero() {
        return Err(ZeroSectorError::CertificateReplayFailure {
            detail: "integer certificate normalization divided by zero".to_owned(),
        });
    }
    let (quotient, remainder) = numerator.quot_rem(denominator);
    if remainder.is_zero() {
        Ok(quotient)
    } else {
        Err(ZeroSectorError::CertificateReplayFailure {
            detail: "integer certificate normalization was inexact".to_owned(),
        })
    }
}

fn replay_integer_kernel(rows: &[Vec<u16>], kernel: &[Integer]) -> Result<(), ZeroSectorError> {
    let columns = rows.first().map_or(kernel.len(), Vec::len);
    if kernel.len() != columns || kernel.iter().all(Integer::is_zero) {
        return Err(ZeroSectorError::CertificateReplayFailure {
            detail: format!(
                "kernel has {} entries for {columns} columns, or is identically zero",
                kernel.len()
            ),
        });
    }
    for (row_index, row) in rows.iter().enumerate() {
        if row.len() != columns {
            return Err(ZeroSectorError::MatrixShape {
                detail: format!("exponent row {row_index} has inconsistent length"),
            });
        }
        let mut sum = Integer::zero();
        for (&entry, value) in row.iter().zip(kernel) {
            let product = Z.mul(&Integer::from(i64::from(entry)), value);
            Z.add_assign(&mut sum, &product);
        }
        if !sum.is_zero() {
            return Err(ZeroSectorError::CertificateReplayFailure {
                detail: format!("kernel product is nonzero on exponent row {row_index}"),
            });
        }
    }
    Ok(())
}

fn is_known_nonzero_integer(coefficient: &Coefficient) -> bool {
    if !coefficient.is_constant() || coefficient.is_zero() {
        return false;
    }
    let numerator = coefficient.numerator.get_constant();
    let denominator = coefficient.denominator.get_constant();
    if denominator.is_zero() {
        return false;
    }
    let (quotient, remainder) = numerator.quot_rem(&denominator);
    remainder.is_zero() && !quotient.is_zero()
}

fn decision_from_error(error: ZeroSectorError) -> ZeroSectorDecision {
    match error {
        ZeroSectorError::ResourceLimit {
            resource,
            requested,
            limit,
        } => ZeroSectorDecision::ResourceLimited(ZeroSectorResource {
            resource,
            requested,
            limit,
        }),
        other => ZeroSectorDecision::Failed(other),
    }
}

fn verify_direct_monotone_closure(
    decisions: &[(SectorMask, ZeroSectorDecision)],
) -> Result<bool, ZeroSectorError> {
    let lookup = decisions
        .iter()
        .map(|(mask, decision)| (mask.clone(), decision))
        .collect::<BTreeMap<_, _>>();
    for (mask, decision) in decisions {
        if !matches!(decision, ZeroSectorDecision::ProvedZero(_)) {
            continue;
        }
        for position in 0..mask.arity() {
            if !mask.is_active(position)? {
                continue;
            }
            let subsector = mask.with_activity(position, false)?;
            match lookup.get(&subsector) {
                Some(ZeroSectorDecision::Excluded(_)) => {}
                Some(ZeroSectorDecision::ProvedZero(_)) => {}
                _ => return Ok(false),
            }
        }
    }
    Ok(true)
}

fn check_limit(
    resource: &'static str,
    requested: usize,
    limit: usize,
) -> Result<(), ZeroSectorError> {
    if requested > limit {
        Err(ZeroSectorError::ResourceLimit {
            resource,
            requested,
            limit,
        })
    } else {
        Ok(())
    }
}

fn checked_add(
    left: usize,
    right: usize,
    resource: &'static str,
) -> Result<usize, ZeroSectorError> {
    left.checked_add(right)
        .ok_or(ZeroSectorError::ResourceCountOverflow { resource })
}

fn checked_mul(
    left: usize,
    right: usize,
    resource: &'static str,
) -> Result<usize, ZeroSectorError> {
    left.checked_mul(right)
        .ok_or(ZeroSectorError::ResourceCountOverflow { resource })
}
