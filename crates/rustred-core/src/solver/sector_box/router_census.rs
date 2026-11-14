//! Symmetry routing over the public sector-symmetry census.
//!
//! Every nonzero sector is routed to its class representative
//! (`SectorSymmetryCensus::normalize`, recursively through pinched
//! endpoints, with census zero-set endpoints dropped), so integrals of
//! non-representative sectors never become columns. Automorphism rows
//! `I - T_g(I)` of representative sectors are exact over the family
//! parameters, with every endpoint already routed onto representatives.

use std::sync::Arc;

use crate::algebra::Coefficient;
use crate::family::{IntegralFamily, IntegralKey};
use crate::sector::Mask;
use crate::sector::symmetry::{
    RelationEndpoints, SectorSymmetryCensus, SectorSymmetryCensusControl,
    SectorSymmetryCensusError, SectorSymmetryCensusOptions,
};
use crate::sector::zero::Analyzer;

use super::super::SolverError;
use super::router::{Route, SectorRouter};

/// [`SectorRouter`] over a [`SectorSymmetryCensus`] of one family.
pub struct CensusRouter {
    census: Arc<SectorSymmetryCensus>,
    arity: usize,
    fingerprint: [u8; 32],
    one: Coefficient,
}

fn census_error(error: SectorSymmetryCensusError) -> SolverError {
    SolverError::InvalidInput(format!("sector-symmetry census: {error}"))
}

impl CensusRouter {
    /// Wrap an existing census. Sectors the census does not declare are
    /// treated as their own representatives (no route, no automorphisms),
    /// unless they are in its zero set.
    pub fn new(census: Arc<SectorSymmetryCensus>) -> Self {
        let family = census.family().clone();
        let mut hasher = blake3::Hasher::new();
        hasher.update(b"rustred-sector-box-census-router-v1\0");
        hasher.update(family.fingerprint().as_bytes());
        hasher.update(format!("{:?}", census.options()).as_bytes());
        for member in census.members() {
            hasher.update(
                format!("{:?}->{:?};", member.sector(), member.representative()).as_bytes(),
            );
        }
        for class in census.classes() {
            hasher.update(&(class.automorphisms().len() as u64).to_le_bytes());
            for permutation in class.line_permutations() {
                hasher.update(format!("{permutation:?}").as_bytes());
            }
        }
        for (sector, kind) in census.zero_sectors() {
            hasher.update(format!("0:{sector:?}:{kind:?};").as_bytes());
        }
        Self {
            arity: family.denominator_count(),
            one: family.coefficient_context().one(),
            fingerprint: *hasher.finalize().as_bytes(),
            census,
        }
    }

    /// Census of every subsector of `top` with the default options
    /// (lowest-id representatives, parametric affine constants).
    pub fn discover(family: Arc<IntegralFamily>, top: &[bool]) -> Result<Self, SolverError> {
        Self::discover_with(family, top, SectorSymmetryCensusOptions::default())
    }

    pub fn discover_with(
        family: Arc<IntegralFamily>,
        top: &[bool],
        options: SectorSymmetryCensusOptions,
    ) -> Result<Self, SolverError> {
        let analyzer = Analyzer::try_unrestricted(&family)
            .map_err(|error| SolverError::InvalidInput(error.to_string()))?;
        let top = Mask::try_new(top[..family.denominator_count()].iter().copied())
            .map_err(|error| SolverError::InvalidInput(error.to_string()))?;
        let census = SectorSymmetryCensus::discover_subsectors(
            family,
            &top,
            &analyzer,
            options,
            &SectorSymmetryCensusControl::default(),
        )
        .map_err(census_error)?;
        Ok(Self::new(Arc::new(census)))
    }

    pub fn census(&self) -> &SectorSymmetryCensus {
        &self.census
    }

    fn mask(&self, active: impl Iterator<Item = bool>) -> Result<Mask, SolverError> {
        Mask::try_new(active.take(self.arity))
            .map_err(|error| SolverError::InvalidInput(error.to_string()))
    }

    fn key<const N: usize>(&self, n: &[i16; N]) -> Result<IntegralKey, SolverError> {
        IntegralKey::try_new(n[..self.arity].iter().map(|&value| i64::from(value)))
            .map_err(|error| SolverError::InvalidInput(error.to_string()))
    }

    fn powers<const N: usize>(&self, key: &IntegralKey) -> Result<[i16; N], SolverError> {
        let mut powers = [0i16; N];
        for (slot, &value) in key.powers().iter().enumerate() {
            powers[slot] = i16::try_from(value).map_err(|_| {
                SolverError::InvalidInput("routed power exceeds the compact range".into())
            })?;
        }
        Ok(powers)
    }

    fn terms<const N: usize>(
        &self,
        terms: &[(IntegralKey, Coefficient)],
    ) -> Result<Vec<([i16; N], Coefficient)>, SolverError> {
        terms
            .iter()
            .map(|(key, coefficient)| Ok((self.powers(key)?, coefficient.clone())))
            .collect()
    }

    fn declared(&self, mask: &Mask) -> bool {
        self.census.member(mask).is_some()
    }
}

impl<const N: usize> SectorRouter<N> for CensusRouter {
    fn representative(&self, sector: &[bool; N]) -> Option<[bool; N]> {
        let mask = self.mask(sector.iter().copied()).ok()?;
        if self.census.is_zero_sector(&mask) {
            return None;
        }
        match self.census.representative(&mask) {
            Some(representative) => Some(std::array::from_fn(|slot| {
                slot < self.arity && representative.active_bits()[slot]
            })),
            None => Some(*sector),
        }
    }

    fn route(&self, n: &[i16; N]) -> Result<Route<N>, SolverError> {
        let mask = self.mask(n.iter().map(|&value| value > 0))?;
        if self.census.is_zero_sector(&mask) {
            return Ok(Vec::new());
        }
        if !self.declared(&mask) {
            return Ok(vec![(*n, self.one.clone())]);
        }
        let normal = self.census.normalize(&self.key(n)?).map_err(census_error)?;
        self.terms(normal.terms())
    }

    fn automorphism_rows(&self, n: &[i16; N]) -> Result<Vec<Route<N>>, SolverError> {
        let mask = self.mask(n.iter().map(|&value| value > 0))?;
        match self.census.member(&mask) {
            Some(member) if member.is_representative() => {}
            _ => return Ok(Vec::new()),
        }
        self.census
            .automorphism_relations_with(&self.key(n)?, RelationEndpoints::Representatives)
            .map_err(census_error)?
            .iter()
            .map(|relation| self.terms(relation.terms()))
            .collect()
    }

    fn fingerprint(&self) -> [u8; 32] {
        self.fingerprint
    }

    /// One relation per retained automorphism generator of the class (the
    /// census deduplicates equal relations), and none outside
    /// representatives.
    fn automorphism_bound(&self, sector: &[bool; N]) -> usize {
        let Ok(mask) = self.mask(sector.iter().copied()) else {
            return 0;
        };
        match self.census.member(&mask) {
            Some(member) if member.is_representative() => {
                self.census.classes()[member.class()].automorphisms().len()
            }
            _ => 0,
        }
    }
}
