use std::collections::BTreeSet;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::sync::Arc;

use crate::algebra::Coefficient;
use crate::family::IntegralFamily;
use crate::family::symanzik::SymanzikPolynomials;
use crate::sector::{self, Mask, Restrictions};

use super::domain::{ConditionSource, Domain};
use super::error::Error;
use super::limits::{Limits, check_limit};
use super::model::{Certificate, Decision, FullColumnRank};
use super::rank::EffectiveRankDecision;

/// Generic topology-neutral zero-sector analyzer for one authenticated family.
///
/// Nonzero nonintegral power shifts are interpreted as formal generic
/// regulators and are therefore included in effective sector support.
#[derive(Debug)]
pub struct Analyzer {
    family_fingerprint: Arc<String>,
    pub(super) symanzik: SymanzikPolynomials,
    restrictions: Restrictions,
    power_support: Mask,
    domain: Arc<Domain>,
    pub(super) limits: Limits,
}

impl Analyzer {
    /// Construct an analyzer with explicit sector restrictions and default
    /// resource limits.
    pub fn try_new(family: &IntegralFamily, restrictions: Restrictions) -> Result<Self, Error> {
        Self::try_new_with_limits(family, restrictions, Limits::default())
    }

    /// Construct an analyzer with explicit sector restrictions and resource
    /// limits.
    pub fn try_new_with_limits(
        family: &IntegralFamily,
        restrictions: Restrictions,
        limits: Limits,
    ) -> Result<Self, Error> {
        catch_unwind(AssertUnwindSafe(|| {
            Self::build(family, restrictions, limits)
        }))
        .map_err(|_| Error::SymbolicaPanic)?
    }

    /// Construct an unrestricted analyzer with default resource limits.
    pub fn try_unrestricted(family: &IntegralFamily) -> Result<Self, Error> {
        Self::try_unrestricted_with_limits(family, Limits::default())
    }

    /// Construct an unrestricted analyzer with explicit resource limits.
    pub fn try_unrestricted_with_limits(
        family: &IntegralFamily,
        limits: Limits,
    ) -> Result<Self, Error> {
        catch_unwind(AssertUnwindSafe(|| {
            let restrictions = Restrictions::unrestricted(family.denominator_count())?;
            Self::build(family, restrictions, limits)
        }))
        .map_err(|_| Error::SymbolicaPanic)?
    }

    fn build(
        family: &IntegralFamily,
        restrictions: Restrictions,
        limits: Limits,
    ) -> Result<Self, Error> {
        if restrictions.arity() != family.denominator_count() {
            return Err(Error::WrongRestrictionsArity {
                expected: family.denominator_count(),
                actual: restrictions.arity(),
            });
        }
        let symanzik = SymanzikPolynomials::try_from_family_with_limits(family, limits.feynman)?;
        let mut domain = Domain::default();
        for condition in family.domain().conditions() {
            let sources = condition
                .sources()
                .iter()
                .cloned()
                .map(ConditionSource::Family)
                .collect();
            domain.insert(condition.polynomial().clone(), sources);
        }

        let mut support = Vec::with_capacity(family.denominator_count());
        for (denominator, shift) in family.power_shifts().iter().enumerate() {
            let nonzero = !shift.is_zero();
            if nonzero && is_known_nonzero_integer(shift) {
                return Err(Error::UnsupportedNonzeroIntegerPowerShift { denominator });
            }
            if nonzero
                && restrictions
                    .cuts()
                    .required_active()
                    .is_active(denominator)?
            {
                return Err(Error::UnsupportedShiftedCut { denominator });
            }
            if nonzero && !shift.numerator.is_constant() {
                domain.insert(
                    shift.numerator.clone(),
                    BTreeSet::from([ConditionSource::PowerShiftSupport { denominator }]),
                );
            }
            support.push(nonzero);
        }

        let shift_pairs = family
            .power_shifts()
            .len()
            .checked_mul(family.power_shifts().len().saturating_sub(1))
            .and_then(|ordered| ordered.checked_div(2))
            .ok_or(Error::ResourceCountOverflow {
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
                    return Err(Error::UnsupportedIntegerSeparatedPowerShifts { left, right });
                }
            }
        }

        Ok(Self {
            family_fingerprint: family.fingerprint_owner(),
            symanzik,
            restrictions,
            power_support: Mask::try_new(support)?,
            domain: Arc::new(domain),
            limits,
        })
    }

    /// Generic coefficient locus required by family construction and formal
    /// power-shift support.
    pub fn domain(&self) -> &Domain {
        &self.domain
    }

    /// Analyze one raw unshifted sector. Resource exhaustion, malformed input,
    /// and native CAS failure remain typed errors rather than decisions.
    pub fn analyze(&self, raw_sector: &Mask) -> Result<Decision, Error> {
        catch_unwind(AssertUnwindSafe(|| self.analyze_inner(raw_sector)))
            .map_err(|_| Error::SymbolicaPanic)?
    }

    fn analyze_inner(&self, raw_sector: &Mask) -> Result<Decision, Error> {
        if let Some(exclusion) = self.restrictions.exclusion(raw_sector)? {
            return Ok(Decision::Excluded(exclusion));
        }
        let effective_sector = self.effective_sector(raw_sector)?;
        let effective = self.compute_effective(&effective_sector)?;
        Ok(self.bind(raw_sector, effective_sector, effective))
    }

    fn effective_sector(&self, raw_sector: &Mask) -> Result<Mask, Error> {
        if raw_sector.arity() != self.power_support.arity() {
            return Err(Error::Sector(sector::Error::WrongArity {
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

    fn bind(
        &self,
        raw_sector: &Mask,
        effective_sector: Mask,
        effective: EffectiveRankDecision,
    ) -> Decision {
        match effective {
            EffectiveRankDecision::Zero {
                active_parameter_order,
                primitive_kernel,
                rank,
                exponent_row_count,
            } => Decision::ProvedZero(Certificate {
                family_fingerprint: self.family_fingerprint.clone(),
                raw_sector: raw_sector.clone(),
                effective_sector,
                active_parameter_order,
                primitive_kernel,
                rank,
                exponent_row_count,
                domain: self.domain.clone(),
            }),
            EffectiveRankDecision::Full {
                active_parameter_order,
                rank,
                exponent_row_count,
                column_count,
            } => Decision::Inconclusive(FullColumnRank {
                raw_sector: raw_sector.clone(),
                effective_sector,
                active_parameter_order,
                rank,
                exponent_row_count,
                column_count,
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
