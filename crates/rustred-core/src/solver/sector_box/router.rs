//! Sector routing: which sectors are zero, which are representatives, and
//! how integrals of other sectors are rewritten before they become columns.

use std::collections::HashMap;
use std::sync::{Arc, RwLock};

use crate::algebra::Coefficient;
use crate::family::IntegralFamily;
use crate::sector::Mask;
use crate::sector::zero::{Analyzer, Decision};

use super::super::SolverError;

/// An exact linear combination of integrals: `(powers, coefficient)` terms.
pub type Route<const N: usize> = Vec<([i16; N], Coefficient)>;

/// Routing contract of the sector-box engine.
///
/// `route` is exact. Its image lies in the integral's representative sector
/// and its subsectors. Row generation memoises Zp images of the routes.
pub trait SectorRouter<const N: usize>: Send + Sync {
    /// `None` for a zero sector; otherwise the representative sector.
    fn representative(&self, sector: &[bool; N]) -> Option<[bool; N]>;
    /// Exact rewrite of `n` into representative sectors. Empty for an
    /// integral of a zero sector.
    fn route(&self, n: &[i16; N]) -> Result<Route<N>, SolverError>;
    /// Exact relations `I - T(I) = 0` from automorphisms of `n`'s sector,
    /// each as a list of terms (empty for routers without symmetries).
    fn automorphism_rows(&self, n: &[i16; N]) -> Result<Vec<Route<N>>, SolverError>;
    /// Binds the family and the routing policy, for session keys.
    fn fingerprint(&self) -> [u8; 32];
    /// Upper bound on the number of [`Self::automorphism_rows`] of any
    /// integral of `sector` (0 for routers without symmetries). Used by the
    /// pre-flight cost model.
    fn automorphism_bound(&self, _sector: &[bool; N]) -> usize {
        0
    }
    /// Whether every nonzero sector is its own representative and `route`
    /// is the identity. Row generation then skips route lookups.
    fn is_identity(&self) -> bool {
        false
    }
}

/// The zero census only (`sector::zero::Analyzer`): every nonzero sector is
/// its own representative and no integral is rewritten.
///
/// Zero decisions are memoised per sector. A sector is zero when the
/// analyzer proves it scaleless or excludes it from the family domain.
pub struct IdentityRouter {
    analyzer: Analyzer,
    arity: usize,
    fingerprint: [u8; 32],
    one: Coefficient,
    zero: RwLock<HashMap<u64, bool>>,
}

impl IdentityRouter {
    pub fn new(family: &IntegralFamily) -> Result<Self, SolverError> {
        let analyzer = Analyzer::try_unrestricted(family)
            .map_err(|error| SolverError::InvalidInput(error.to_string()))?;
        let mut hasher = blake3::Hasher::new();
        hasher.update(b"rustred-sector-box-identity-router-v1\0");
        hasher.update(family.fingerprint().as_bytes());
        Ok(Self {
            analyzer,
            arity: family.denominator_count(),
            fingerprint: *hasher.finalize().as_bytes(),
            one: family.coefficient_context().one(),
            zero: RwLock::new(HashMap::new()),
        })
    }

    pub fn shared(family: &IntegralFamily) -> Result<Arc<Self>, SolverError> {
        Self::new(family).map(Arc::new)
    }

    /// Exact zero decision of one sector, memoised.
    pub fn is_zero_bits(&self, bits: u64) -> Result<bool, SolverError> {
        if let Some(&zero) = self.zero.read().expect("zero memo lock").get(&bits) {
            return Ok(zero);
        }
        let mask = Mask::try_new((0..self.arity).map(|slot| bits >> slot & 1 == 1))
            .map_err(|error| SolverError::InvalidInput(error.to_string()))?;
        let zero = bits == 0
            || matches!(
                self.analyzer
                    .analyze(&mask)
                    .map_err(|error| SolverError::InvalidInput(error.to_string()))?,
                Decision::ProvedZero(_) | Decision::Excluded(_)
            );
        self.zero
            .write()
            .expect("zero memo lock")
            .insert(bits, zero);
        Ok(zero)
    }
}

pub(crate) fn sector_bits<const N: usize>(sector: &[bool; N]) -> u64 {
    sector
        .iter()
        .enumerate()
        .filter(|(_, active)| **active)
        .map(|(slot, _)| 1u64 << slot)
        .sum()
}

pub(crate) fn integral_bits<const N: usize>(n: &[i16; N]) -> u64 {
    n.iter()
        .enumerate()
        .filter(|(_, value)| **value > 0)
        .map(|(slot, _)| 1u64 << slot)
        .sum()
}

impl<const N: usize> SectorRouter<N> for IdentityRouter {
    fn representative(&self, sector: &[bool; N]) -> Option<[bool; N]> {
        // An analyzer failure is not a zero proof: keep the sector.
        match self.is_zero_bits(sector_bits(sector)) {
            Ok(true) => None,
            _ => Some(*sector),
        }
    }

    fn route(&self, n: &[i16; N]) -> Result<Route<N>, SolverError> {
        if self.is_zero_bits(integral_bits(n))? {
            return Ok(Vec::new());
        }
        Ok(vec![(*n, self.one.clone())])
    }

    fn automorphism_rows(&self, _n: &[i16; N]) -> Result<Vec<Route<N>>, SolverError> {
        Ok(Vec::new())
    }

    fn fingerprint(&self) -> [u8; 32] {
        self.fingerprint
    }

    fn is_identity(&self) -> bool {
        true
    }
}
