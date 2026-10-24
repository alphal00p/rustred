//! Recursive routing of integrals onto representative sectors.
//!
//! A route maps a member sector `S` onto its representative `R` with the same
//! line count; every transported endpoint lies in `R` or in a strict pinch of
//! `R`. Endpoints are therefore processed by decreasing line count: an
//! endpoint in a non-representative sector receives all of its contributions
//! before it is routed, every key is transported at most once, and the
//! recursion terminates because routed line counts strictly decrease.
//! Endpoints in the census zero set are dropped (they vanish identically);
//! endpoints in sectors the census neither declared nor proved zero are
//! typed [`SectorSymmetryCensusError::UnknownSector`] errors.

use std::cmp::Reverse;
use std::collections::{BTreeMap, HashMap};
use std::sync::Arc;

use crate::algebra::{Coefficient, CoefficientContext, ExactAlgebraLimits};
use crate::family::IntegralKey;

use super::{Error, SectorSymmetryCensus, SectorSymmetryCensusError};

/// Default diagnostic bound on live terms while normalizing one integral or row.
pub const DEFAULT_MAX_NORMAL_FORM_TERMS: usize = 1_000_000;
/// Default diagnostic bound on route applications while normalizing one integral or row.
pub const DEFAULT_MAX_NORMAL_FORM_TRANSPORTS: usize = 1_000_000;

/// Optional diagnostic thresholds of one recursive normalization (one key,
/// one relation row, or one key of a batch). Ordinary execution relies on the
/// inherited Symbolica RSS/time budget, checked at every route application;
/// the count thresholds apply only with `diagnostics_enabled`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct NormalizationLimits {
    /// Enforce the count thresholds below (explicit opt-in).
    pub diagnostics_enabled: bool,
    /// Live pending plus finished terms.
    pub max_terms: usize,
    /// Route applications, including memoised ones.
    pub max_transports: usize,
}

impl Default for NormalizationLimits {
    fn default() -> Self {
        Self {
            diagnostics_enabled: false,
            max_terms: DEFAULT_MAX_NORMAL_FORM_TERMS,
            max_transports: DEFAULT_MAX_NORMAL_FORM_TRANSPORTS,
        }
    }
}

impl NormalizationLimits {
    /// Explicitly enable the diagnostic count thresholds.
    pub fn with_diagnostics(mut self) -> Self {
        self.diagnostics_enabled = true;
        self
    }
}

/// How relation rows treat the endpoints of `T(I)`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum RelationEndpoints {
    /// Every transported endpoint, as transported.
    #[default]
    Transported,
    /// Drop endpoints in sectors of the census zero set.
    DropZero,
    /// Route every endpoint recursively onto representative sectors
    /// ([`SectorSymmetryCensus::normalize`]); zero-set endpoints are dropped.
    /// Only the source `I` of a route row lies outside a representative.
    Representatives,
}

/// Exact `I = sum_j c_j J_j` with every `J_j` in a representative sector.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SectorSymmetryNormalForm {
    source: IntegralKey,
    terms: Box<[(IntegralKey, Coefficient)]>,
    transports: usize,
    dropped_zero_terms: usize,
}

impl SectorSymmetryNormalForm {
    pub fn source(&self) -> &IntegralKey {
        &self.source
    }

    /// Nonzero terms sorted by key; empty if `I` vanishes.
    pub fn terms(&self) -> &[(IntegralKey, Coefficient)] {
        &self.terms
    }

    /// Route applications used (memoised ones included).
    pub fn transports(&self) -> usize {
        self.transports
    }

    /// Pending terms dropped because their sector is in the zero set.
    pub fn dropped_zero_terms(&self) -> usize {
        self.dropped_zero_terms
    }
}

/// Transported endpoints of non-representative keys, shared by one call or
/// one batch.
#[derive(Default)]
pub(super) struct Memo {
    routes: HashMap<IntegralKey, Arc<[(IntegralKey, Coefficient)]>>,
}

pub(super) struct Image {
    pub(super) terms: BTreeMap<IntegralKey, Coefficient>,
    transports: usize,
    dropped_zero_terms: usize,
}

/// `terms[key] += value` (or `-=`), removing exact zeros.
pub(super) fn accumulate(
    context: &CoefficientContext,
    limits: ExactAlgebraLimits,
    terms: &mut BTreeMap<IntegralKey, Coefficient>,
    key: &IntegralKey,
    value: &Coefficient,
    subtract: bool,
) -> Result<(), Error> {
    let sum = match (terms.get(key), subtract) {
        (Some(old), false) => context.try_add(old, value, limits),
        (Some(old), true) => context.try_sub(old, value, limits),
        (None, false) => Ok(value.clone()),
        (None, true) => context.try_neg(value, limits),
    }?;
    if sum.is_zero() {
        terms.remove(key);
    } else {
        terms.insert(key.clone(), sum);
    }
    Ok(())
}

impl SectorSymmetryCensus {
    /// Express one integral exactly through integrals of representative
    /// sectors by applying member routes recursively; zero-set endpoints are
    /// dropped. A representative-sector integral is its own normal form; an
    /// integral of a zero-set sector has the empty normal form. Integrals of
    /// sectors that are neither declared nor in the zero set are
    /// [`SectorSymmetryCensusError::UnknownSector`] errors.
    ///
    /// Automorphisms are not applied: representative-sector integrals related
    /// by [`Self::automorphism_relations`] stay distinct terms.
    pub fn normalize(&self, key: &IntegralKey) -> Result<SectorSymmetryNormalForm, Error> {
        self.normalize_with_memo(key, &mut Memo::default())
    }

    /// [`Self::normalize`] for many keys, sharing transported routes across
    /// the batch. Results are in input order; the first error aborts.
    pub fn normalize_many<'a>(
        &self,
        keys: impl IntoIterator<Item = &'a IntegralKey>,
    ) -> Result<Vec<SectorSymmetryNormalForm>, Error> {
        let mut memo = Memo::default();
        keys.into_iter()
            .map(|key| self.normalize_with_memo(key, &mut memo))
            .collect()
    }

    fn normalize_with_memo(
        &self,
        key: &IntegralKey,
        memo: &mut Memo,
    ) -> Result<SectorSymmetryNormalForm, Error> {
        let one = self.family.coefficient_context().one();
        let image = self.normalize_terms(std::iter::once((key.clone(), one)), memo)?;
        Ok(SectorSymmetryNormalForm {
            source: key.clone(),
            terms: image.terms.into_iter().collect(),
            transports: image.transports,
            dropped_zero_terms: image.dropped_zero_terms,
        })
    }

    /// Normalize a finite combination `sum_i c_i I_i`.
    pub(super) fn normalize_terms(
        &self,
        input: impl IntoIterator<Item = (IntegralKey, Coefficient)>,
        memo: &mut Memo,
    ) -> Result<Image, Error> {
        let context = self.family.coefficient_context();
        let algebra = self.options.expansion.exact_algebra;
        let limits = self.options.normalization;
        // Highest line count first; ties in key order (deterministic).
        let mut pending = Pending::new();
        for (key, coefficient) in input {
            let lines = self.sector_of(&key)?.active_count();
            pending.add(context, algebra, lines, &key, &coefficient)?;
        }
        let mut image = Image {
            terms: BTreeMap::new(),
            transports: 0,
            dropped_zero_terms: 0,
        };
        while let Some(((_, key), coefficient)) = pending.0.pop_first() {
            let sector = self.sector_of(&key)?;
            if self.is_zero_sector(&sector) {
                image.dropped_zero_terms += 1;
                continue;
            }
            let member = self
                .members
                .get(&sector)
                .ok_or_else(|| SectorSymmetryCensusError::UnknownSector(sector.clone()))?;
            if member.is_representative() {
                accumulate(
                    context,
                    algebra,
                    &mut image.terms,
                    &key,
                    &coefficient,
                    false,
                )?;
            } else {
                image.transports += 1;
                admit(
                    "normal-form transports",
                    image.transports,
                    limits.max_transports,
                    limits.diagnostics_enabled,
                )?;
                let route = match memo.routes.get(&key) {
                    Some(route) => route.clone(),
                    None => {
                        let transported = member.route.transport(&key, self.options.expansion)?;
                        let route: Arc<[(IntegralKey, Coefficient)]> = transported
                            .terms()
                            .iter()
                            .map(|endpoint| {
                                (endpoint.key().clone(), endpoint.coefficient().clone())
                            })
                            .collect();
                        memo.routes.insert(key.clone(), route.clone());
                        route
                    }
                };
                for (endpoint, value) in route.iter() {
                    let product = context.try_mul(&coefficient, value, algebra)?;
                    let lines = self.sector_of(endpoint)?.active_count();
                    debug_assert!(lines <= sector.active_count());
                    pending.add(context, algebra, lines, endpoint, &product)?;
                }
            }
            admit(
                "normal-form terms",
                pending.0.len() + image.terms.len(),
                limits.max_terms,
                limits.diagnostics_enabled,
            )?;
        }
        Ok(image)
    }
}

/// Pending terms keyed by (decreasing line count, key).
struct Pending(BTreeMap<(Reverse<usize>, IntegralKey), Coefficient>);

impl Pending {
    fn new() -> Self {
        Self(BTreeMap::new())
    }

    fn add(
        &mut self,
        context: &CoefficientContext,
        limits: ExactAlgebraLimits,
        lines: usize,
        key: &IntegralKey,
        value: &Coefficient,
    ) -> Result<(), Error> {
        let slot = (Reverse(lines), key.clone());
        let sum = match self.0.get(&slot) {
            Some(old) => context.try_add(old, value, limits)?,
            None => value.clone(),
        };
        if sum.is_zero() {
            self.0.remove(&slot);
        } else {
            self.0.insert(slot, sum);
        }
        Ok(())
    }
}

fn admit(
    resource: &'static str,
    requested: usize,
    limit: usize,
    diagnostics_enabled: bool,
) -> Result<(), Error> {
    symbolica::utils::resources::checkpoint().map_err(Error::Resource)?;
    if diagnostics_enabled && requested > limit {
        Err(Error::ResourceLimit {
            resource,
            requested,
            limit,
        })
    } else {
        Ok(())
    }
}
