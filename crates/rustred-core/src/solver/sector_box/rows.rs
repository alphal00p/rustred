//! Zp row factory: IBP templates pre-specialised at one parameter point.
//!
//! Each `SourceSystem` row is compiled once into Zp polynomials in the index
//! variables at the sample point `theta`. Emitting a row at a numeric seed
//! then costs a few modular multiply-adds per term. Integrals of zero sectors
//! are dropped (`vanishes_in_subsector` semantics for numeric integrals) and
//! other integrals are routed through memoised Zp images of the router.
//! A row keeps only its provenance `(seed, source)`, never a polynomial.

use std::cmp::Ordering;
use std::collections::HashMap;
use std::sync::atomic::{AtomicU8, Ordering as AtomicOrdering};
use std::sync::{Arc, RwLock};

use rand::{Rng, SeedableRng, rngs::StdRng};
use rayon::prelude::*;
use symbolica::domains::finite_field::{FiniteFieldCore, FiniteFieldElement, ToFiniteField, Zp64};
use symbolica::domains::{Field, RingOps};
use symbolica::prelude::{Integer, Ring};

use crate::algebra::{Coefficient, CoefficientPolynomial};

use super::super::discovery::ColumnComparator;
use super::super::{Integral, SolverError, SourceSystem, Term};
use super::BoxPhase;
use super::order::BoxOrder;
use super::router::{SectorRouter, integral_bits};

pub(crate) type Zp = FiniteFieldElement<u64>;

/// The Zp image of an exact route, shared between threads.
type ZpRoute<const N: usize> = Arc<Vec<([i16; N], Zp)>>;

/// Origin of one modular row: a seed ordinal and a source-row ordinal.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub(crate) struct Provenance {
    pub seed: u32,
    pub source: u16,
}

/// One instantiated modular row, harder first, distinct nonzero terms.
#[derive(Clone, Debug)]
pub(crate) struct ZpRow<const N: usize> {
    pub provenance: Provenance,
    pub terms: Vec<Term<N, Zp>>,
}

impl<const N: usize> AsRef<[Term<N, Zp>]> for ZpRow<N> {
    fn as_ref(&self) -> &[Term<N, Zp>] {
        &self.terms
    }
}

#[derive(Debug)]
struct Monomial {
    coefficient: Zp,
    /// `(slot, exponent)` of the index variables.
    exponents: Vec<(u8, u8)>,
}

#[derive(Debug)]
struct TemplateTerm<const N: usize> {
    /// Offset of a free coordinate, or the absolute power of a fixed one.
    shift: [i16; N],
    monomials: Vec<Monomial>,
}

/// How row generation treats integrals of one sector.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum SectorState {
    /// Nonzero and its own representative: integrals become columns.
    Representative,
    /// Proved zero: integrals are dropped.
    Zero,
    /// Nonzero, another representative: integrals are routed.
    Routed,
}

impl SectorState {
    fn code(self) -> u8 {
        match self {
            Self::Representative => 1,
            Self::Zero => 2,
            Self::Routed => 3,
        }
    }

    fn from_code(code: u8) -> Option<Self> {
        match code {
            1 => Some(Self::Representative),
            2 => Some(Self::Zero),
            3 => Some(Self::Routed),
            _ => None,
        }
    }
}

/// Lock-free memo of sector states for small arities.
#[derive(Debug)]
enum ZeroTable {
    Dense(Vec<AtomicU8>),
    Sparse(RwLock<HashMap<u64, SectorState>>),
}

const DENSE_ZERO_TABLE_ARITY: usize = 20;

pub(crate) struct ZpRowFactory<const N: usize> {
    field: Zp64,
    arity: usize,
    fixed: [bool; N],
    templates: Vec<Vec<TemplateTerm<N>>>,
    order: Arc<BoxOrder<N>>,
    router: Arc<dyn SectorRouter<N>>,
    identity: bool,
    zero: ZeroTable,
    routes: RwLock<HashMap<[i16; N], ZpRoute<N>>>,
    /// Representative sector of routed sectors, memoised.
    representatives: RwLock<HashMap<u64, u64>>,
    /// Whether the router supplies automorphism rows.
    automorphisms: bool,
    /// Parameter values (in coefficient-variable order) of this sample.
    point: Vec<Zp>,
}

impl<const N: usize> ZpRowFactory<N> {
    /// Compile the templates at the parameter point drawn from `seed`.
    ///
    /// Every family condition (source conditions and the caller's extra
    /// conditions, e.g. zero-census conditions) must be nonzero at the point,
    /// otherwise the sample is unlucky.
    pub(crate) fn new(
        sources: &SourceSystem<N>,
        arity: usize,
        order: Arc<BoxOrder<N>>,
        router: Arc<dyn SectorRouter<N>>,
        prime: u64,
        seed: u64,
        extra_conditions: &[CoefficientPolynomial],
    ) -> Result<Self, SolverError> {
        Self::build(
            sources,
            arity,
            order,
            router,
            prime,
            seed,
            None,
            extra_conditions,
        )
    }

    /// Compile the templates at explicit parameter values (in coefficient
    /// variable order: the family parameters), for independent checks at a
    /// chosen point. As in [`Self::new`], every family condition and every
    /// `extra_conditions` polynomial must be nonzero at the point.
    #[allow(dead_code)] // used by the materialising lanes (a later step)
    pub(crate) fn at_parameters(
        sources: &SourceSystem<N>,
        arity: usize,
        order: Arc<BoxOrder<N>>,
        router: Arc<dyn SectorRouter<N>>,
        prime: u64,
        parameters: &[Zp],
        extra_conditions: &[CoefficientPolynomial],
    ) -> Result<Self, SolverError> {
        Self::build(
            sources,
            arity,
            order,
            router,
            prime,
            0,
            Some(parameters),
            extra_conditions,
        )
    }

    #[allow(clippy::too_many_arguments)]
    fn build(
        sources: &SourceSystem<N>,
        arity: usize,
        order: Arc<BoxOrder<N>>,
        router: Arc<dyn SectorRouter<N>>,
        prime: u64,
        seed: u64,
        parameters: Option<&[Zp]>,
        extra_conditions: &[CoefficientPolynomial],
    ) -> Result<Self, SolverError> {
        if prime < 3 || !Integer::from(prime).is_prime(0) {
            return Err(SolverError::InvalidInput(
                "sector-box modular sample requires an odd prime".into(),
            ));
        }
        let field = Zp64::new(prime);
        let variables = sources.coefficient_variables().len();
        let indices = sources.index_variables();
        let mut is_index = vec![None; variables];
        for (axis, &variable) in indices.iter().enumerate() {
            is_index[variable] = Some(axis);
        }
        // Parameters (including d) are sampled; index variables are not.
        let mut random = StdRng::seed_from_u64(seed);
        let point: Vec<Zp> = (0..variables)
            .map(|variable| {
                if is_index[variable].is_some() {
                    field.zero()
                } else if let Some(parameters) = parameters {
                    parameters.get(variable).copied().unwrap_or(field.zero())
                } else {
                    Integer::from(random.random_range(1..prime)).to_finite_field(&field)
                }
            })
            .collect();
        let fixed = std::array::from_fn(|axis| sources.fixed()[axis].is_some());
        let mut templates = Vec::with_capacity(sources.rows().len());
        for row in sources.rows() {
            let mut terms = Vec::with_capacity(row.len());
            for term in row {
                let shift = std::array::from_fn(|axis| term.integral[axis].value());
                let monomials =
                    specialise(&term.coefficient, &field, &point, &is_index, indices.len());
                if !monomials.is_empty() {
                    terms.push(TemplateTerm { shift, monomials });
                }
            }
            templates.push(terms);
        }
        for condition in sources.conditions().iter().chain(extra_conditions) {
            // A parameter-only condition is decided by the point alone. An
            // index-dependent one is generic over seeds and is not tested.
            let value = specialise(condition, &field, &point, &is_index, indices.len());
            if value.iter().all(|monomial| monomial.exponents.is_empty()) {
                let constant = value.iter().fold(field.zero(), |sum, monomial| {
                    field.add(&sum, &monomial.coefficient)
                });
                if field.is_zero(&constant) && !condition.is_zero() {
                    return Err(SolverError::UnluckyBoxSample {
                        phase: BoxPhase::Conditions,
                    });
                }
            }
        }
        let zero = if arity <= DENSE_ZERO_TABLE_ARITY {
            ZeroTable::Dense((0..1usize << arity).map(|_| AtomicU8::new(0)).collect())
        } else {
            ZeroTable::Sparse(RwLock::new(HashMap::new()))
        };
        Ok(Self {
            identity: router.is_identity(),
            automorphisms: !router.is_identity(),
            field,
            arity,
            fixed,
            templates,
            order,
            router,
            zero,
            routes: RwLock::new(HashMap::new()),
            representatives: RwLock::new(HashMap::new()),
            point,
        })
    }

    pub(crate) fn field(&self) -> &Zp64 {
        &self.field
    }

    pub(crate) fn source_count(&self) -> usize {
        self.templates.len()
    }

    /// Whether the sector with these active-slot bits is zero.
    pub(crate) fn is_zero_sector(&self, bits: u64) -> bool {
        self.sector_state(bits) == SectorState::Zero
    }

    /// Routing state of the sector with these active-slot bits, memoised.
    pub(crate) fn sector_state(&self, bits: u64) -> SectorState {
        match &self.zero {
            ZeroTable::Dense(table) => {
                let cell = &table[bits as usize];
                if let Some(state) = SectorState::from_code(cell.load(AtomicOrdering::Relaxed)) {
                    return state;
                }
                let state = self.decide(bits);
                cell.store(state.code(), AtomicOrdering::Relaxed);
                state
            }
            ZeroTable::Sparse(memo) => {
                if let Some(&state) = memo.read().expect("sector memo lock").get(&bits) {
                    return state;
                }
                let state = self.decide(bits);
                memo.write().expect("sector memo lock").insert(bits, state);
                state
            }
        }
    }

    fn decide(&self, bits: u64) -> SectorState {
        let sector: [bool; N] =
            std::array::from_fn(|slot| slot < self.arity && bits >> slot & 1 == 1);
        match self.router.representative(&sector) {
            None => SectorState::Zero,
            Some(representative) if representative == sector => SectorState::Representative,
            Some(_) => SectorState::Routed,
        }
    }

    /// Kira id of the representative sector of `bits`; `None` when zero.
    pub(crate) fn representative_bits(&self, bits: u64) -> Option<u64> {
        match self.sector_state(bits) {
            SectorState::Zero => None,
            SectorState::Representative => Some(bits),
            SectorState::Routed => {
                if let Some(&representative) = self
                    .representatives
                    .read()
                    .expect("representative memo lock")
                    .get(&bits)
                {
                    return Some(representative);
                }
                let sector: [bool; N] =
                    std::array::from_fn(|slot| slot < self.arity && bits >> slot & 1 == 1);
                let representative =
                    super::router::sector_bits(&self.router.representative(&sector)?);
                self.representatives
                    .write()
                    .expect("representative memo lock")
                    .insert(bits, representative);
                Some(representative)
            }
        }
    }

    /// Upper bound on the router's automorphism rows per seed of sector
    /// `bits` (cost model; 0 for identity routing).
    #[allow(dead_code)] // used by seeding growth (a later step)
    pub(crate) fn automorphism_row_bound(&self, bits: u64) -> usize {
        if !self.automorphisms {
            return 0;
        }
        let sector: [bool; N] =
            std::array::from_fn(|slot| slot < self.arity && bits >> slot & 1 == 1);
        self.router.automorphism_bound(&sector)
    }

    /// Representative-sector integrals of the exact route of one integral
    /// (itself for a representative sector, none for a zero sector).
    pub(crate) fn route_keys(&self, powers: &[i16; N]) -> Result<Vec<[i16; N]>, SolverError> {
        match self.sector_state(integral_bits(powers)) {
            SectorState::Zero => Ok(Vec::new()),
            SectorState::Representative => Ok(vec![*powers]),
            SectorState::Routed => Ok(self
                .router
                .route(powers)?
                .into_iter()
                .map(|(key, _)| key)
                .filter(|key| self.sector_state(integral_bits(key)) != SectorState::Zero)
                .collect()),
        }
    }

    /// Instantiate every source row at `seed`. `seed_id` is its ordinal.
    /// Automorphism rows are added only when `automorphisms` is set (tube
    /// seeds go without them).
    pub(crate) fn rows_at(
        &self,
        seed_id: u32,
        seed: &[i16; N],
        automorphisms: bool,
        out: &mut Vec<ZpRow<N>>,
    ) -> Result<usize, SolverError> {
        let values = self.seed_values(seed);
        let mut empty = 0;
        for source in 0..self.templates.len() {
            let terms = self.instantiate(source, seed, &values)?;
            if terms.is_empty() {
                empty += 1;
                continue;
            }
            out.push(ZpRow {
                provenance: Provenance {
                    seed: seed_id,
                    source: source as u16,
                },
                terms,
            });
        }
        if self.automorphisms && automorphisms {
            for (ordinal, terms) in self.automorphism_rows(seed)?.into_iter().enumerate() {
                if terms.is_empty() {
                    empty += 1;
                    continue;
                }
                out.push(ZpRow {
                    provenance: Provenance {
                        seed: seed_id,
                        source: self.automorphism_source(ordinal)?,
                    },
                    terms,
                });
            }
        }
        Ok(empty)
    }

    fn automorphism_source(&self, ordinal: usize) -> Result<u16, SolverError> {
        u16::try_from(self.templates.len() + ordinal).map_err(|_| {
            SolverError::InvalidInput("too many automorphism rows for one seed".into())
        })
    }

    /// Whether a provenance source is an automorphism row (not an IBP).
    pub(crate) fn is_automorphism_source(&self, source: u16) -> bool {
        usize::from(source) >= self.templates.len()
    }

    /// Zp images of the router's automorphism rows of `seed`, with every
    /// term routed (in practice already representative) and merged.
    fn automorphism_rows(&self, seed: &[i16; N]) -> Result<Vec<Vec<Term<N, Zp>>>, SolverError> {
        let field = &self.field;
        let order = &*self.order;
        let mut rows = Vec::new();
        for relation in self.router.automorphism_rows(seed)? {
            let mut terms: Vec<Term<N, Zp>> = Vec::with_capacity(relation.len());
            for (powers, coefficient) in relation {
                let value = self.evaluate(&coefficient)?;
                if field.is_zero(&value) {
                    continue;
                }
                match self.sector_state(integral_bits(&powers)) {
                    SectorState::Zero => {}
                    SectorState::Representative => terms.push(Term {
                        integral: Integral::numeric(powers)?,
                        coefficient: value,
                    }),
                    SectorState::Routed => {
                        for (image, factor) in self.route(&powers)?.iter() {
                            terms.push(Term {
                                integral: Integral::numeric(*image)?,
                                coefficient: field.mul(&value, factor),
                            });
                        }
                    }
                }
            }
            terms.sort_unstable_by(|left, right| order.compare(&left.integral, &right.integral));
            rows.push(merge_terms(field, order, terms));
        }
        Ok(rows)
    }

    /// One row from its provenance; empty when it vanishes.
    pub(crate) fn row(
        &self,
        seed: &[i16; N],
        source: u16,
    ) -> Result<Vec<Term<N, Zp>>, SolverError> {
        if self.is_automorphism_source(source) {
            let ordinal = usize::from(source) - self.templates.len();
            return Ok(self
                .automorphism_rows(seed)?
                .into_iter()
                .nth(ordinal)
                .unwrap_or_default());
        }
        let values = self.seed_values(seed);
        self.instantiate(source as usize, seed, &values)
    }

    /// Rows of many seeds, generated in parallel by the current Rayon pool
    /// and merged deterministically in seed order; `automorphisms(seed_id)`
    /// decides whether a seed gets automorphism rows. Returns `(rows, empty)`.
    pub(crate) fn rows_for(
        &self,
        seeds: &[(u32, [i16; N])],
        automorphisms: &(dyn Fn(u32) -> bool + Sync),
    ) -> Result<(Vec<ZpRow<N>>, usize), SolverError> {
        const CHUNK: usize = 64;
        let chunks: Vec<Result<(Vec<ZpRow<N>>, usize), SolverError>> = seeds
            .par_chunks(CHUNK)
            .map(|chunk| {
                let mut rows = Vec::with_capacity(chunk.len() * self.templates.len());
                let mut empty = 0;
                for (seed_id, seed) in chunk {
                    empty += self.rows_at(*seed_id, seed, automorphisms(*seed_id), &mut rows)?;
                }
                Ok((rows, empty))
            })
            .collect();
        let mut rows = Vec::with_capacity(seeds.len() * self.templates.len());
        let mut empty = 0;
        for chunk in chunks {
            let (chunk_rows, chunk_empty) = chunk?;
            rows.extend(chunk_rows);
            empty += chunk_empty;
        }
        Ok((rows, empty))
    }

    /// Seed coordinates as Zp elements, with signs.
    fn seed_values(&self, seed: &[i16; N]) -> [Zp; N] {
        std::array::from_fn(|axis| {
            let value = seed[axis];
            let magnitude = self.field.to_element(u64::from(value.unsigned_abs()));
            if value < 0 {
                self.field.neg(&magnitude)
            } else {
                magnitude
            }
        })
    }

    fn instantiate(
        &self,
        source: usize,
        seed: &[i16; N],
        values: &[Zp; N],
    ) -> Result<Vec<Term<N, Zp>>, SolverError> {
        let field = &self.field;
        let mut terms: Vec<Term<N, Zp>> = Vec::with_capacity(self.templates[source].len());
        let mut routed = false;
        for template in &self.templates[source] {
            let mut powers = [0i16; N];
            for axis in 0..N {
                powers[axis] = if self.fixed[axis] {
                    template.shift[axis]
                } else {
                    seed[axis] + template.shift[axis]
                };
            }
            let state = if self.identity {
                if self.is_zero_sector(integral_bits(&powers)) {
                    continue;
                }
                SectorState::Representative
            } else {
                match self.sector_state(integral_bits(&powers)) {
                    SectorState::Zero => continue,
                    state => state,
                }
            };
            let mut coefficient = field.zero();
            for monomial in &template.monomials {
                let mut value = monomial.coefficient;
                for &(slot, exponent) in &monomial.exponents {
                    for _ in 0..exponent {
                        value = field.mul(&value, &values[slot as usize]);
                    }
                }
                field.add_assign(&mut coefficient, &value);
            }
            if field.is_zero(&coefficient) {
                continue;
            }
            if state == SectorState::Representative {
                terms.push(Term {
                    integral: Integral::numeric(powers)?,
                    coefficient,
                });
            } else {
                routed = true;
                for (image, factor) in self.route(&powers)?.iter() {
                    terms.push(Term {
                        integral: Integral::numeric(*image)?,
                        coefficient: field.mul(&coefficient, factor),
                    });
                }
            }
        }
        let order = &*self.order;
        terms.sort_unstable_by(|left, right| order.compare(&left.integral, &right.integral));
        if routed {
            // Routing can map several terms onto one integral.
            return Ok(merge_terms(field, order, terms));
        }
        debug_assert!(
            terms.windows(2).all(|pair| {
                order.compare(&pair[0].integral, &pair[1].integral) == Ordering::Less
            })
        );
        Ok(terms)
    }

    /// Zp image of one integral after zero pruning and routing: itself for
    /// a representative sector, its route for a routed one, nothing for a
    /// zero sector.
    #[allow(dead_code)] // used by the materialising lanes (a later step)
    pub(crate) fn route_image(
        &self,
        powers: &[i16; N],
    ) -> Result<Vec<([i16; N], Zp)>, SolverError> {
        match self.sector_state(integral_bits(powers)) {
            SectorState::Zero => Ok(Vec::new()),
            SectorState::Representative => Ok(vec![(*powers, self.field.one())]),
            SectorState::Routed => Ok(self.route(powers)?.as_ref().clone()),
        }
    }

    /// Memoised Zp image of the router's exact route.
    fn route(&self, powers: &[i16; N]) -> Result<ZpRoute<N>, SolverError> {
        if let Some(image) = self.routes.read().expect("route memo lock").get(powers) {
            return Ok(image.clone());
        }
        let exact = self.router.route(powers)?;
        let mut image = Vec::with_capacity(exact.len());
        for (target, coefficient) in exact {
            match self.sector_state(integral_bits(&target)) {
                SectorState::Zero => continue,
                SectorState::Representative => {}
                SectorState::Routed => {
                    return Err(SolverError::InvalidInput(format!(
                        "route of {powers:?} ends in a non-representative sector at {target:?}"
                    )));
                }
            }
            let value = self.evaluate(&coefficient)?;
            if !self.field.is_zero(&value) {
                image.push((target, value));
            }
        }
        let image = Arc::new(image);
        self.routes
            .write()
            .expect("route memo lock")
            .insert(*powers, image.clone());
        Ok(image)
    }

    /// Evaluate an exact parameter coefficient at this sample.
    pub(crate) fn evaluate(&self, coefficient: &Coefficient) -> Result<Zp, SolverError> {
        let numerator = coefficient.numerator.evaluate_with_coeff_map(
            |value| value.to_finite_field(&self.field),
            &self.point[..coefficient.numerator.nvars().min(self.point.len())],
            &self.field,
        );
        let denominator = coefficient.denominator.evaluate_with_coeff_map(
            |value| value.to_finite_field(&self.field),
            &self.point[..coefficient.denominator.nvars().min(self.point.len())],
            &self.field,
        );
        if self.field.is_zero(&denominator) {
            return Err(SolverError::UnluckyBoxSample {
                phase: BoxPhase::Conditions,
            });
        }
        Ok(self.field.div(&numerator, &denominator))
    }
}

/// Merge equal integrals of sorted terms and drop zeros.
fn merge_terms<const N: usize>(
    field: &Zp64,
    order: &BoxOrder<N>,
    terms: Vec<Term<N, Zp>>,
) -> Vec<Term<N, Zp>> {
    let mut merged: Vec<Term<N, Zp>> = Vec::with_capacity(terms.len());
    for term in terms {
        match merged.last_mut() {
            Some(last) if order.compare(&last.integral, &term.integral) == Ordering::Equal => {
                field.add_assign(&mut last.coefficient, &term.coefficient);
            }
            _ => merged.push(term),
        }
    }
    merged.retain(|term| !field.is_zero(&term.coefficient));
    merged
}

/// Substitute the parameter point into `polynomial` and keep a Zp polynomial
/// in the index variables, merging equal index monomials.
fn specialise(
    polynomial: &CoefficientPolynomial,
    field: &Zp64,
    point: &[Zp],
    is_index: &[Option<usize>],
    _index_count: usize,
) -> Vec<Monomial> {
    let mut merged: Vec<Monomial> = Vec::new();
    for term in polynomial {
        let mut value = term.coefficient.to_finite_field(field);
        let mut exponents = Vec::new();
        for (variable, &exponent) in term.exponents.iter().enumerate() {
            if exponent == 0 {
                continue;
            }
            match is_index.get(variable).copied().flatten() {
                Some(axis) => exponents.push((axis as u8, exponent as u8)),
                None => {
                    for _ in 0..exponent {
                        value = field.mul(&value, &point[variable]);
                    }
                }
            }
        }
        if field.is_zero(&value) {
            continue;
        }
        match merged
            .iter_mut()
            .find(|monomial| monomial.exponents == exponents)
        {
            Some(monomial) => field.add_assign(&mut monomial.coefficient, &value),
            None => merged.push(Monomial {
                coefficient: value,
                exponents,
            }),
        }
    }
    merged.retain(|monomial| !field.is_zero(&monomial.coefficient));
    merged
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use symbolica::domains::finite_field::FiniteFieldCore;

    use super::*;
    use crate::algebra::{CoefficientContext, ExactAlgebraLimits};
    use crate::family::{AffineDenominator, IntegralFamily};
    use crate::solver::sector_box::IdentityRouter;
    use crate::solver::sector_box::order::BoxOrder;
    use crate::solver::{IntegralOrder, Seed};

    const N: usize = 10;

    /// RustFlow's native P14 wheel with `D0 -> D0 - eta` (the rim variant).
    fn p14_rim() -> Arc<IntegralFamily> {
        const ROWS: [[i64; 10]; 10] = [
            [0, 0, 0, 0, 0, 0, 0, 1, 0, 0],
            [1, -2, 0, -2, 1, 0, 2, 0, 0, 1],
            [0, 0, 0, 0, 1, 2, 2, 1, 2, 1],
            [0, 0, 0, 0, 0, 0, 0, 0, 0, 1],
            [1, -2, -2, -2, 1, 2, 2, 1, 2, 1],
            [0, 0, 0, 0, 1, 2, 0, 1, 0, 0],
            [1, 0, 0, 0, 0, 0, 0, 0, 0, 0],
            [0, 0, 0, 0, 1, 0, 0, 0, 0, 0],
            [0, 1, 0, 0, 0, 0, 0, 0, 0, 0],
            [0, 0, 1, 0, 0, 0, 0, 0, 0, 0],
        ];
        let context = CoefficientContext::try_new(["d", "eta"]).unwrap();
        let denominators = ROWS
            .iter()
            .enumerate()
            .map(|(slot, row)| {
                let mut constant = context.integer(if slot < 8 { -1 } else { 0 });
                if slot == 0 {
                    constant = context
                        .try_sub(
                            &constant,
                            &context.parameter("eta").unwrap(),
                            ExactAlgebraLimits::default(),
                        )
                        .unwrap();
                }
                AffineDenominator::new(constant, row.map(|value| context.integer(value)).to_vec())
            })
            .collect();
        Arc::new(
            IntegralFamily::new(
                "p14_rim",
                (0..4).map(|loop_| format!("k{loop_}")).collect(),
                vec![],
                context.clone(),
                context.parameter("d").unwrap(),
                denominators,
                vec![],
                vec![context.zero(); 10],
            )
            .unwrap(),
        )
    }

    /// Factory rows equal RustRed's exact source instantiation (with the
    /// same zero-sector pruning) evaluated at the factory's sample point.
    #[test]
    fn zp_rows_equal_exact_instantiation_at_the_sample_point() {
        let family = p14_rim();
        let router = IdentityRouter::shared(&family).unwrap();
        let sources = SourceSystem::<N>::from_family_with_capacity(&family, false).unwrap();
        let order = Arc::new(BoxOrder::new(&super::super::ColumnOrder::Numeric, N, &[]).unwrap());
        let factory = ZpRowFactory::new(
            &sources,
            N,
            order.clone(),
            router.clone(),
            super::super::DEFAULT_PRIME,
            11,
            &[],
        )
        .unwrap();
        let zero: Vec<[bool; N]> = (0u64..256)
            .filter(|&bits| factory.is_zero_sector(bits))
            .map(|bits| std::array::from_fn(|slot| bits >> slot & 1 == 1))
            .collect();
        let seeds: [[i16; N]; 6] = [
            [1, 1, 1, 1, 0, 0, 1, 0, 0, 0],
            [1, 1, 2, 1, 0, 0, 1, 0, 0, 0],
            [2, 1, 1, 1, 0, -1, 1, 0, 0, -1],
            [1, 1, 1, 1, 0, 0, 1, 1, -1, 0],
            [1, 1, 1, 1, 1, 1, 1, 1, 0, 0],
            [1, 3, 1, 1, 2, 1, 1, 1, -1, -1],
        ];
        let field = factory.field().clone();
        for seed in seeds {
            let sector: [bool; N] = std::array::from_fn(|slot| seed[slot] > 0);
            let exact_order = IntegralOrder::new(sector, [false; N]);
            let start = Seed {
                integral: Integral::numeric(seed).unwrap(),
                shifts: [0; N],
            };
            for (source, template) in sources.rows().iter().enumerate() {
                let exact = super::super::super::instantiate::instantiate(
                    template,
                    &start,
                    sources.index_variables(),
                    sources.fixed(),
                    &exact_order,
                    &zero,
                    None,
                )
                .unwrap();
                let mut expected: Vec<([i16; N], u64)> = exact
                    .iter()
                    .map(|term| {
                        let value = factory.evaluate(&term.coefficient).unwrap();
                        (
                            std::array::from_fn(|axis| term.integral[axis].value()),
                            field.from_element(&value),
                        )
                    })
                    .filter(|(_, value)| *value != 0)
                    .collect();
                expected.sort_unstable();
                let mut actual: Vec<([i16; N], u64)> = factory
                    .row(&seed, source as u16)
                    .unwrap()
                    .iter()
                    .map(|term| {
                        (
                            std::array::from_fn(|axis| term.integral[axis].value()),
                            field.from_element(&term.coefficient),
                        )
                    })
                    .collect();
                actual.sort_unstable();
                assert_eq!(actual, expected, "seed {seed:?}, source {source}");
            }
        }
    }

    /// Review finding (PR 4): `at_parameters` honours the caller's extra
    /// conditions like `new`: an explicit point where one vanishes is an
    /// unlucky sample, a point where none vanishes is accepted.
    #[test]
    fn explicit_points_check_the_extra_conditions() {
        let family = p14_rim();
        let router = IdentityRouter::shared(&family).unwrap();
        let sources = SourceSystem::<N>::from_family_with_capacity(&family, false).unwrap();
        let order = Arc::new(BoxOrder::new(&super::super::ColumnOrder::Numeric, N, &[]).unwrap());
        let context = family.coefficient_context();
        let d_minus_4 = context
            .try_sub(
                &context.parameter("d").unwrap(),
                &context.integer(4),
                ExactAlgebraLimits::default(),
            )
            .unwrap()
            .numerator;
        let field = symbolica::domains::finite_field::Zp64::new(super::super::DEFAULT_PRIME);
        let at = |d: u64| {
            ZpRowFactory::at_parameters(
                &sources,
                N,
                order.clone(),
                router.clone(),
                super::super::DEFAULT_PRIME,
                &[field.to_element(d), field.to_element(3)],
                std::slice::from_ref(&d_minus_4),
            )
        };
        assert!(matches!(
            at(4),
            Err(SolverError::UnluckyBoxSample {
                phase: BoxPhase::Conditions
            })
        ));
        assert!(at(5).is_ok());
    }

    /// Review finding (PR 4): the routed path. A three-loop banana with four
    /// unit-mass lines and two massless ISPs, `(k1+k2)^2` and `(k2+k3)^2`,
    /// under the census router: its line permutations need numerator
    /// substitutions, i.e. multi-term routes. Every factory row equals RustRed's
    /// exact instantiation with each term composed with the router's exact
    /// route, evaluated at the factory's sample point.
    mod routed {
        use std::collections::BTreeMap;
        use std::sync::Arc;

        use symbolica::domains::RingOps;
        use symbolica::domains::finite_field::FiniteFieldCore;

        use super::super::*;
        use crate::algebra::CoefficientContext;
        use crate::family::{AffineDenominator, IntegralFamily};
        use crate::solver::sector_box::CensusRouter;
        use crate::solver::sector_box::order::BoxOrder;
        use crate::solver::{IntegralOrder, Seed};

        const N: usize = 6;

        /// Coordinates `k1.k1, k1.k2, k1.k3, k2.k2, k2.k3, k3.k3`.
        fn banana() -> Arc<IntegralFamily> {
            const ROWS: [[i64; 6]; 6] = [
                [1, 0, 0, 0, 0, 0],
                [0, 0, 0, 1, 0, 0],
                [0, 0, 0, 0, 0, 1],
                [1, 2, 2, 1, 2, 1],
                [1, 2, 0, 1, 0, 0],
                [0, 0, 0, 1, 2, 1],
            ];
            let context = CoefficientContext::try_new(["d"]).unwrap();
            let denominators = ROWS
                .iter()
                .enumerate()
                .map(|(slot, row)| {
                    AffineDenominator::new(
                        context.integer(if slot < 4 { -1 } else { 0 }),
                        row.map(|value| context.integer(value)).to_vec(),
                    )
                })
                .collect();
            Arc::new(
                IntegralFamily::new(
                    "banana3",
                    ["k1", "k2", "k3"].map(String::from).to_vec(),
                    vec![],
                    context.clone(),
                    context.parameter("d").unwrap(),
                    denominators,
                    vec![],
                    vec![context.zero(); N],
                )
                .unwrap(),
            )
        }

        #[test]
        fn routed_rows_equal_exact_instantiation_composed_with_routes() {
            let family = banana();
            let router: Arc<dyn SectorRouter<N>> = Arc::new(
                CensusRouter::discover(family.clone(), &[true, true, true, true, false, false])
                    .unwrap(),
            );
            let sources = SourceSystem::<N>::from_family_with_capacity(&family, false).unwrap();
            let order = Arc::new(
                BoxOrder::new(&super::super::super::ColumnOrder::Numeric, N, &[]).unwrap(),
            );
            let factory = ZpRowFactory::new(
                &sources,
                N,
                order,
                router.clone(),
                super::super::super::DEFAULT_PRIME,
                11,
                &[],
            )
            .unwrap();
            let field = factory.field().clone();
            let zero: Vec<[bool; N]> = (0u64..64)
                .filter(|&bits| factory.is_zero_sector(bits))
                .map(|bits| std::array::from_fn(|slot| bits >> slot & 1 == 1))
                .collect();
            // Top, dotted and numerator seeds of the banana and of routed
            // three-line subsectors.
            let seeds: [[i16; N]; 8] = [
                [1, 1, 1, 1, 0, 0],
                [2, 1, 1, 1, 0, -1],
                [1, 1, 1, 1, -1, -1],
                [1, 1, 1, 0, -1, 0],
                [1, 1, 0, 1, 0, -2],
                [0, 1, 1, 1, -1, 0],
                [1, 0, 1, 1, -1, -1],
                [2, 1, 0, 1, -1, 0],
            ];
            let mut multi_term_routes = 0usize;
            let mut compared = 0usize;
            for seed in seeds {
                let sector: [bool; N] = std::array::from_fn(|slot| seed[slot] > 0);
                let exact_order = IntegralOrder::new(sector, [false; N]);
                let start = Seed {
                    integral: Integral::numeric(seed).unwrap(),
                    shifts: [0; N],
                };
                for (source, template) in sources.rows().iter().enumerate() {
                    let exact = super::super::super::super::instantiate::instantiate(
                        template,
                        &start,
                        sources.index_variables(),
                        sources.fixed(),
                        &exact_order,
                        &zero,
                        None,
                    )
                    .unwrap();
                    // Compose every exact term with its exact route.
                    let mut expected: BTreeMap<[i16; N], Zp> = BTreeMap::new();
                    for term in &exact {
                        let powers: [i16; N] =
                            std::array::from_fn(|axis| term.integral[axis].value());
                        let value = factory.evaluate(&term.coefficient).unwrap();
                        let route = router.route(&powers).unwrap();
                        if route.len() > 1 {
                            multi_term_routes += 1;
                        }
                        for (target, weight) in route {
                            let weight = factory.evaluate(&weight).unwrap();
                            let entry = expected.entry(target).or_insert_with(|| field.zero());
                            field.add_assign(entry, &field.mul(&value, &weight));
                        }
                    }
                    let expected: Vec<([i16; N], u64)> = expected
                        .into_iter()
                        .filter(|(_, value)| !field.is_zero(value))
                        .map(|(target, value)| (target, field.from_element(&value)))
                        .collect();
                    let mut actual: Vec<([i16; N], u64)> = factory
                        .row(&seed, source as u16)
                        .unwrap()
                        .iter()
                        .map(|term| {
                            (
                                std::array::from_fn(|axis| term.integral[axis].value()),
                                field.from_element(&term.coefficient),
                            )
                        })
                        .collect();
                    actual.sort_unstable();
                    assert_eq!(actual, expected, "seed {seed:?}, source {source}");
                    compared += 1;
                    // Every term lies in a representative sector.
                    for (target, _) in &actual {
                        let sector: [bool; N] = std::array::from_fn(|slot| target[slot] > 0);
                        assert_eq!(router.representative(&sector), Some(sector));
                    }
                }
            }
            assert!(compared > 0);
            assert!(
                multi_term_routes > 0,
                "the test must exercise multi-term routes"
            );
        }
    }
}
