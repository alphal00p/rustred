//! Proof-backed zero and product terminals for bounded K=6 discovery.
//!
//! These terminals discharge only their exact concrete domains. In
//! particular, none of the unresolved scalar graph corners is promoted to a
//! master while the three-loop fixed point remains open.

use std::collections::BTreeSet;

use crate::algebra::IndexedCoefficientContext;
use crate::family::IntegralKey;
use crate::foundry::artifact::ArtifactError;
use crate::foundry::search::{
    ReachabilityTerminal, ReachabilityTerminalKind, ReachabilityTerminalProvider,
};
use crate::sector::symmetry::Canonicalizer;
use crate::sector::{Mask, SectorInteriorDomain};

use super::factorization::K6FactorizationSupport;
use super::manifest::ZERO_ORBITS;
use super::momentum_rank::active_momentum_rank;

/// Exact terminal owner shared by the K=6 cell foundry and bounded reachability
/// census.
pub(crate) struct K6ReachabilityTerminals {
    factorization: K6FactorizationSupport,
    zero_sectors: Box<[Mask]>,
}

impl K6ReachabilityTerminals {
    pub(crate) fn try_new() -> Result<Self, ArtifactError> {
        let factorization = K6FactorizationSupport::try_new()?;
        let zero_sectors = exact_zero_sectors(factorization.canonicalizer())?;
        for sector in &zero_sectors {
            let rank = active_momentum_rank(factorization.family(), sector)
                .map_err(|_| ArtifactError::InvalidZeroTerminal)?;
            if rank >= factorization.family().loop_count() {
                return Err(ArtifactError::InvalidZeroTerminal);
            }
        }
        Ok(Self {
            factorization,
            zero_sectors: zero_sectors.into_boxed_slice(),
        })
    }

    pub(crate) fn context(&self) -> &IndexedCoefficientContext {
        self.factorization.context()
    }

    pub(crate) fn family_fingerprint(&self) -> &str {
        self.factorization.family().fingerprint()
    }

    pub(crate) fn canonicalizer(&self) -> &Canonicalizer {
        self.factorization.canonicalizer()
    }

    pub(crate) fn zero_sectors(&self) -> &[Mask] {
        &self.zero_sectors
    }

    pub(crate) fn factorization_rule_count(&self) -> usize {
        self.factorization.factorization_rules().len()
    }
}

impl ReachabilityTerminalProvider for K6ReachabilityTerminals {
    fn classify(&self, target: &IntegralKey) -> Option<ReachabilityTerminal> {
        if let Some(ordinal) = self
            .zero_sectors
            .iter()
            .position(|sector| same_sector(sector, target))
        {
            return Some(ReachabilityTerminal::new(
                ReachabilityTerminalKind::ZeroSector,
                ordinal,
            ));
        }
        self.factorization
            .factorization_rules()
            .iter()
            .position(|rule| domain_contains(rule.application_domain(), target))
            .map(|ordinal| {
                ReachabilityTerminal::new(ReachabilityTerminalKind::Factorization, ordinal)
            })
    }
}

/// Expand the exact zero-orbit manifest to every raw sector needed by residual
/// source projection. The returned masks are independently rechecked by
/// [`K6ReachabilityTerminals::try_new`] before they can terminate discovery.
pub(crate) fn exact_zero_sectors(
    canonicalizer: &Canonicalizer,
) -> Result<Vec<Mask>, ArtifactError> {
    let zero_representatives = ZERO_ORBITS
        .iter()
        .map(|orbit| orbit.representative)
        .collect::<BTreeSet<_>>();
    (0_u64..64)
        .map(|bits| {
            let powers: [i64; 6] = std::array::from_fn(|slot| i64::from(((bits >> slot) & 1) != 0));
            let key = IntegralKey::try_new(powers)?;
            let canonical = canonicalizer.canonicalize(&key)?;
            Ok::<_, ArtifactError>(
                zero_representatives
                    .contains(canonical.canonical().powers())
                    .then(|| Mask::try_from_indices(key.powers()))
                    .transpose()?,
            )
        })
        .collect::<Result<Vec<_>, _>>()
        .map(|sectors| sectors.into_iter().flatten().collect())
}

fn same_sector(sector: &Mask, target: &IntegralKey) -> bool {
    sector
        .active_bits()
        .iter()
        .zip(target.powers())
        .all(|(&active, &power)| active == (power >= 1))
}

fn domain_contains(domain: &SectorInteriorDomain, target: &IntegralKey) -> bool {
    domain
        .bounds()
        .iter()
        .zip(target.powers())
        .all(|(&bounds, &power)| bounds.contains(power))
}
