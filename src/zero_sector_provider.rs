//! Generic proof-bearing zero-sector policy layered over any scalar provider.
//!
//! LiteRed erases analytically zero sectors before solving or applying IBP
//! relations.  This module exposes the same ordering as a composable concrete
//! provider: a replayable Lee/Symanzik rank certificate (or an explicit cut)
//! takes precedence, while sectors for which no zero proof is available are
//! delegated unchanged.  Pattern exclusions are not zero proofs and remain a
//! typed error.

use std::error::Error;
use std::fmt;
use std::sync::Arc;

use crate::reduction_engine::{ConcreteRuleDecision, ConcreteRuleProvider};
use crate::{
    CertifiedRewriteError, CertifiedRewriteLimits, CertifiedZeroReduction, ConcreteIntegralKey,
    IntegralFamily, PowerShiftPolicy, SectorExclusion, SectorFoundationError, SectorMask,
    SectorRestrictions, ZeroSectorAnalyzer, ZeroSectorDecision, ZeroSectorError,
};

/// Stable schema for the generic zero-before-inner-provider composition.
pub const CERTIFIED_ZERO_SECTOR_RULE_PROVIDER_V1_SCHEMA: &str =
    "rustred-certified-zero-sector-rule-provider-v1";

/// A topology-independent provider wrapper which proves zero sectors before
/// consulting its inner rule source.
pub struct CertifiedZeroSectorRuleProvider<'family, Provider> {
    family: &'family IntegralFamily,
    restrictions: SectorRestrictions,
    zero: ZeroSectorAnalyzer,
    inner: Provider,
    rewrite_limits: CertifiedRewriteLimits,
}

impl<'family, Provider> CertifiedZeroSectorRuleProvider<'family, Provider>
where
    Provider: ConcreteRuleProvider,
{
    pub const SCHEMA: &'static str = CERTIFIED_ZERO_SECTOR_RULE_PROVIDER_V1_SCHEMA;

    pub fn try_new(
        family: &'family IntegralFamily,
        restrictions: SectorRestrictions,
        policy: PowerShiftPolicy,
        inner: Provider,
        rewrite_limits: CertifiedRewriteLimits,
    ) -> Result<Self, CertifiedZeroSectorRuleProviderError<Provider::Error>> {
        validate_arity::<Provider::Error>(family, &restrictions, &inner)?;
        let zero = ZeroSectorAnalyzer::try_new_with_limits(
            family,
            restrictions.clone(),
            policy,
            rewrite_limits.zero_sector,
        )?;
        Ok(Self {
            family,
            restrictions,
            zero,
            inner,
            rewrite_limits,
        })
    }

    pub fn try_unrestricted(
        family: &'family IntegralFamily,
        policy: PowerShiftPolicy,
        inner: Provider,
        rewrite_limits: CertifiedRewriteLimits,
    ) -> Result<Self, CertifiedZeroSectorRuleProviderError<Provider::Error>> {
        let restrictions = SectorRestrictions::unrestricted(family.denominator_count())?;
        Self::try_new(family, restrictions, policy, inner, rewrite_limits)
    }

    pub const fn family(&self) -> &IntegralFamily {
        self.family
    }

    pub const fn restrictions(&self) -> &SectorRestrictions {
        &self.restrictions
    }

    pub const fn analyzer(&self) -> &ZeroSectorAnalyzer {
        &self.zero
    }

    pub const fn inner(&self) -> &Provider {
        &self.inner
    }

    /// Mutating an inner provider through a reduction engine must use the
    /// engine's cache-invalidating provider accessor.
    pub fn inner_mut(&mut self) -> &mut Provider {
        &mut self.inner
    }

    pub fn into_inner(self) -> Provider {
        self.inner
    }

    pub const fn rewrite_limits(&self) -> CertifiedRewriteLimits {
        self.rewrite_limits
    }

    fn decide(
        &mut self,
        integral: &ConcreteIntegralKey,
    ) -> Result<ConcreteRuleDecision, CertifiedZeroSectorRuleProviderError<Provider::Error>> {
        let expected = self.family.denominator_count();
        let inner_arity = self.inner.index_arity();
        if inner_arity != expected {
            return Err(CertifiedZeroSectorRuleProviderError::ProviderArityChanged {
                expected,
                actual: inner_arity,
            });
        }
        if integral.powers().len() != expected {
            return Err(CertifiedZeroSectorRuleProviderError::WrongArity {
                expected,
                actual: integral.powers().len(),
            });
        }

        let sector = SectorMask::try_from_indices(integral.powers())?;
        match self.zero.analyze_sector(&sector) {
            ZeroSectorDecision::ProvedZero(certificate) => Ok(ConcreteRuleDecision::ProvedZero(
                CertifiedZeroReduction::try_new(
                    self.family,
                    integral.clone(),
                    Arc::new(certificate),
                    self.rewrite_limits,
                )?,
            )),
            ZeroSectorDecision::NoZeroCertificate(_) => self
                .inner
                .decision_for(integral)
                .map_err(CertifiedZeroSectorRuleProviderError::Inner),
            ZeroSectorDecision::ResourceLimited(resource) => {
                Err(CertifiedZeroSectorRuleProviderError::ZeroResource {
                    resource: resource.resource(),
                    requested: resource.requested(),
                    limit: resource.limit(),
                })
            }
            ZeroSectorDecision::Failed(error) => {
                Err(CertifiedZeroSectorRuleProviderError::Zero(error))
            }
            ZeroSectorDecision::Excluded(exclusion) => {
                if exclusion.violates_cut() {
                    Ok(ConcreteRuleDecision::ProvedZero(
                        CertifiedZeroReduction::from_cut_exclusion(
                            self.family,
                            integral.clone(),
                            self.restrictions.clone(),
                            exclusion,
                            self.rewrite_limits,
                        )?,
                    ))
                } else {
                    Err(
                        CertifiedZeroSectorRuleProviderError::PatternExcludedSector {
                            source: integral.clone(),
                            exclusion,
                        },
                    )
                }
            }
        }
    }
}

impl<Provider> ConcreteRuleProvider for CertifiedZeroSectorRuleProvider<'_, Provider>
where
    Provider: ConcreteRuleProvider,
{
    type Error = CertifiedZeroSectorRuleProviderError<Provider::Error>;

    fn index_arity(&self) -> usize {
        self.family.denominator_count()
    }

    fn decision_for(
        &mut self,
        integral: &ConcreteIntegralKey,
    ) -> Result<ConcreteRuleDecision, Self::Error> {
        self.decide(integral)
    }
}

fn validate_arity<ProviderError>(
    family: &IntegralFamily,
    restrictions: &SectorRestrictions,
    inner: &impl ConcreteRuleProvider<Error = ProviderError>,
) -> Result<(), CertifiedZeroSectorRuleProviderError<ProviderError>>
where
    ProviderError: Error + Send + Sync + 'static,
{
    let expected = family.denominator_count();
    if restrictions.arity() != expected {
        return Err(
            CertifiedZeroSectorRuleProviderError::WrongRestrictionsArity {
                expected,
                actual: restrictions.arity(),
            },
        );
    }
    let actual = inner.index_arity();
    if actual != expected {
        return Err(CertifiedZeroSectorRuleProviderError::WrongProviderArity { expected, actual });
    }
    Ok(())
}

#[derive(Debug)]
pub enum CertifiedZeroSectorRuleProviderError<ProviderError>
where
    ProviderError: Error + Send + Sync + 'static,
{
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
    PatternExcludedSector {
        source: ConcreteIntegralKey,
        exclusion: SectorExclusion,
    },
    ZeroResource {
        resource: &'static str,
        requested: usize,
        limit: usize,
    },
    Inner(ProviderError),
    Zero(ZeroSectorError),
    Rewrite(CertifiedRewriteError),
    Sector(SectorFoundationError),
}

impl<ProviderError> fmt::Display for CertifiedZeroSectorRuleProviderError<ProviderError>
where
    ProviderError: Error + Send + Sync + 'static,
{
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::WrongArity { expected, actual } => write!(
                formatter,
                "zero-sector provider request arity is {actual}, expected {expected}"
            ),
            Self::WrongRestrictionsArity { expected, actual } => write!(
                formatter,
                "zero-sector restrictions arity is {actual}, expected {expected}"
            ),
            Self::WrongProviderArity { expected, actual } => write!(
                formatter,
                "inner zero-sector provider arity is {actual}, expected {expected}"
            ),
            Self::ProviderArityChanged { expected, actual } => write!(
                formatter,
                "inner zero-sector provider arity changed to {actual}, expected {expected}"
            ),
            Self::PatternExcludedSector { source, exclusion } => write!(
                formatter,
                "integral {source:?} belongs to a pattern-excluded sector without a zero proof: {exclusion:?}"
            ),
            Self::ZeroResource {
                resource,
                requested,
                limit,
            } => write!(
                formatter,
                "zero-sector {resource} needs {requested} units, exceeding the configured limit {limit}"
            ),
            Self::Inner(error) => write!(formatter, "inner rule provider failed: {error}"),
            Self::Zero(error) => error.fmt(formatter),
            Self::Rewrite(error) => error.fmt(formatter),
            Self::Sector(error) => error.fmt(formatter),
        }
    }
}

impl<ProviderError> Error for CertifiedZeroSectorRuleProviderError<ProviderError>
where
    ProviderError: Error + Send + Sync + 'static,
{
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Inner(error) => Some(error),
            Self::Zero(error) => Some(error),
            Self::Rewrite(error) => Some(error),
            Self::Sector(error) => Some(error),
            _ => None,
        }
    }
}

impl<ProviderError> From<ZeroSectorError> for CertifiedZeroSectorRuleProviderError<ProviderError>
where
    ProviderError: Error + Send + Sync + 'static,
{
    fn from(value: ZeroSectorError) -> Self {
        Self::Zero(value)
    }
}

impl<ProviderError> From<CertifiedRewriteError>
    for CertifiedZeroSectorRuleProviderError<ProviderError>
where
    ProviderError: Error + Send + Sync + 'static,
{
    fn from(value: CertifiedRewriteError) -> Self {
        Self::Rewrite(value)
    }
}

impl<ProviderError> From<SectorFoundationError>
    for CertifiedZeroSectorRuleProviderError<ProviderError>
where
    ProviderError: Error + Send + Sync + 'static,
{
    fn from(value: SectorFoundationError) -> Self {
        Self::Sector(value)
    }
}
