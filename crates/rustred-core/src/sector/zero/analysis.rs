use std::collections::BTreeSet;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::sync::Arc;

use crate::algebra::Coefficient;
use crate::family::IntegralFamily;
use crate::family::symanzik::SymanzikPolynomials;
use crate::sector::{self, Mask, Restrictions};

use super::domain::{ZeroSectorConditionSource, ZeroSectorDomain};
use super::error::ZeroSectorError;
use super::limits::{PowerShiftPolicy, ZeroSectorLimits, check_limit};
use super::model::{
    FullColumnRankWitness, ZERO_SECTOR_CERTIFICATE_SCHEMA, ZeroSectorCertificate,
    ZeroSectorDecision, ZeroSectorResource,
};
use super::rank::{EffectiveRankDecision, replay_integer_kernel};

/// Generic analyzer constructed once per authenticated family/restriction set.
#[derive(Clone, Debug)]
pub struct ZeroSectorAnalyzer {
    family_fingerprint: Arc<str>,
    pub(super) symanzik: SymanzikPolynomials,
    restrictions: Restrictions,
    power_support: Mask,
    domain: ZeroSectorDomain,
    policy: PowerShiftPolicy,
    pub(super) limits: ZeroSectorLimits,
}

impl ZeroSectorAnalyzer {
    pub fn try_new(
        family: &IntegralFamily,
        restrictions: Restrictions,
        policy: PowerShiftPolicy,
    ) -> Result<Self, ZeroSectorError> {
        Self::try_new_with_limits(family, restrictions, policy, ZeroSectorLimits::default())
    }

    pub fn try_new_with_limits(
        family: &IntegralFamily,
        restrictions: Restrictions,
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

    pub(super) fn build_unrestricted(
        family: &IntegralFamily,
        policy: PowerShiftPolicy,
        limits: ZeroSectorLimits,
    ) -> Result<Self, ZeroSectorError> {
        let restrictions = Restrictions::unrestricted(family.denominator_count())?;
        Self::build(family, restrictions, policy, limits)
    }

    fn build(
        family: &IntegralFamily,
        restrictions: Restrictions,
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
            let sources = condition
                .sources()
                .iter()
                .cloned()
                .map(ZeroSectorConditionSource::Family)
                .collect();
            domain.insert(condition.polynomial().clone(), sources);
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
                domain.insert(
                    shift.numerator.clone(),
                    BTreeSet::from([ZeroSectorConditionSource::PowerShiftSupport { denominator }]),
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
            power_support: Mask::try_new(support)?,
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

    pub fn restrictions(&self) -> &Restrictions {
        &self.restrictions
    }

    pub fn power_support(&self) -> &Mask {
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

    pub fn analyze_sector(&self, raw_sector: &Mask) -> ZeroSectorDecision {
        match catch_unwind(AssertUnwindSafe(|| self.analyze_sector_inner(raw_sector))) {
            Ok(Ok(decision)) => decision,
            Ok(Err(error)) => decision_from_error(error),
            Err(_) => ZeroSectorDecision::Failed(ZeroSectorError::SymbolicaPanic),
        }
    }

    fn analyze_sector_inner(
        &self,
        raw_sector: &Mask,
    ) -> Result<ZeroSectorDecision, ZeroSectorError> {
        if let Some(exclusion) = self.restrictions.exclusion(raw_sector)? {
            return Ok(ZeroSectorDecision::Excluded(exclusion));
        }
        let effective_sector = self.effective_sector(raw_sector)?;
        let effective = self.compute_effective_checked(&effective_sector);
        Ok(self.bind_effective(raw_sector, &effective))
    }

    fn effective_sector(&self, raw_sector: &Mask) -> Result<Mask, ZeroSectorError> {
        if raw_sector.arity() != self.power_support.arity() {
            return Err(ZeroSectorError::Sector(sector::Error::WrongArity {
                expected: self.power_support.arity(),
                actual: raw_sector.arity(),
            }));
        }
        Ok(Mask::try_new(
            raw_sector
                .active_bits()
                .iter()
                .zip(self.power_support.active_bits())
                .map(|(&raw, &shifted)| raw || shifted),
        )?)
    }

    fn bind_effective(
        &self,
        raw_sector: &Mask,
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
                detail: "generic-domain conditions changed".to_owned(),
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

    pub(super) fn replay_certificate_inner(
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
