//! The Zp probe compiler of the selected system: values of the requested
//! `(target, master)` coefficients at one parameter point modulo one prime.
//!
//! A probe replays the selected rows (re-instantiated from provenance, see
//! [`super::system`]) at a parameter point modulo a prime with the modular
//! pivot order prescribed: no pivot search, forward elimination along the
//! known pivots, back substitution restricted to the closure of the
//! requested integrals, and output of the `(target, master)` coefficients
//! only. This is the finite-field black box of Kira 2 (arXiv:2008.06494
//! §2.2: forward elimination, trimming, back substitution, requested master
//! coefficients only).
//!
//! The system is compiled once per prime ([`LaneRequest::compile`]): fixed
//! parameters are substituted, every coefficient becomes a sum over a shared
//! table of monomials in the free parameters, and constant denominators are
//! folded into the numerators. Probes then run in bunches of `K` points
//! sharing one traversal of the system (FireFly 2, arXiv:2004.01463 §3.1);
//! the sparsity pattern of a bunch is the union over its lanes, values are
//! exact per lane.
//!
//! Guard (per lane): every lane must reproduce the modular pivot of every
//! row and the modular master set. A lane that does not (a zero pivot, a
//! nonzero entry left of a prescribed pivot, a pole of an input
//! coefficient, a nonzero non-master coefficient) is reported as a
//! [`LaneFailure`] and its values must not be used. Zeros of denominators
//! of the reduction show up exactly as such failures.
//!
//! [`reference_probe`] computes the same values independently: Symbolica's
//! sparse row reducer with pivot search, the session's replay guard and the
//! modular pass's back substitution, with coefficients evaluated directly
//! from the exact rows. [`probe_explicit`] runs both at one explicit point
//! and refuses a disagreement.

use std::cmp::Reverse;
use std::collections::{BinaryHeap, HashMap};
use std::sync::Arc;

use symbolica::domains::RingOps;
use symbolica::domains::finite_field::{FiniteFieldCore, ToFiniteField, Zp64};
use symbolica::domains::rational::Rational;
use symbolica::prelude::{Integer, Ring};

use crate::algebra::reconstruct::rational_image;
use crate::algebra::{Coefficient, CoefficientPolynomial};

use super::super::discovery::ColumnComparator;
use super::super::{Integral, SolverError, Term};
use super::BoxPhase;
use super::rows::Zp;
use super::select::{Echelon, Replay, closure, reduced_rows};
use super::system::{ExactSystem, TargetRoute};

/// Why a lane of a bunch was discarded.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum LaneFailure {
    /// An input coefficient (or a fixed parameter) has a pole at the point.
    Pole,
    /// A prescribed pivot vanished.
    Pivot,
    /// A non-pivot column left of a prescribed pivot is nonzero.
    Structure,
    /// A candidate outside the modular master set has a nonzero coefficient.
    Master,
}

impl LaneFailure {
    pub fn label(self) -> &'static str {
        match self {
            Self::Pole => "pole",
            Self::Pivot => "pivot",
            Self::Structure => "structure",
            Self::Master => "master",
        }
    }
}

/// The request of one materialisation or probe: the selected system, the
/// targets routed onto its columns and the parameter split.
pub(crate) struct LaneRequest<const N: usize> {
    pub system: Arc<ExactSystem<N>>,
    pub targets: Vec<[i16; N]>,
    pub routes: Vec<TargetRoute>,
    /// Fixed value of each family parameter, `None` for free parameters.
    pub fixed: Vec<Option<Rational>>,
}

impl<const N: usize> LaneRequest<N> {
    pub(crate) fn free(&self) -> Vec<usize> {
        (0..self.fixed.len())
            .filter(|&index| self.fixed[index].is_none())
            .collect()
    }

    /// Compile the system and the routes at `prime` with the fixed
    /// parameters substituted; `None` when a fixed value or a constant
    /// coefficient denominator vanishes modulo `prime`.
    pub(crate) fn compile(&self, prime: u64) -> Option<Compiled> {
        let field = Zp64::new(prime);
        let free = self.free();
        let mut fixed_values = Vec::with_capacity(self.fixed.len());
        for value in &self.fixed {
            fixed_values.push(match value {
                Some(value) => Some(rational_image(value, &field)?),
                None => None,
            });
        }
        let mut monomials: Vec<Vec<u16>> = Vec::new();
        let mut index: HashMap<Vec<u16>, u32> = HashMap::new();
        let mut compile_poly = |polynomial: &CoefficientPolynomial| -> CompiledPoly {
            let mut terms: Vec<(Zp, u32)> = Vec::new();
            for term in polynomial {
                let mut value: Zp = term.coefficient.to_finite_field(&field);
                for (variable, &exponent) in term.exponents.iter().enumerate() {
                    if exponent == 0 {
                        continue;
                    }
                    if let Some(Some(fixed)) = fixed_values.get(variable) {
                        value = field.mul(&value, &field.pow(fixed, u64::from(exponent)));
                    }
                }
                if field.is_zero(&value) {
                    continue;
                }
                let key: Vec<u16> = free
                    .iter()
                    .map(|&variable| term.exponents.get(variable).copied().unwrap_or(0))
                    .collect();
                let next = index.len() as u32;
                let id = *index.entry(key.clone()).or_insert_with(|| {
                    monomials.push(key);
                    next
                });
                match terms.iter_mut().find(|(_, m)| *m == id) {
                    Some((c, _)) => field.add_assign(c, &value),
                    None => terms.push((value, id)),
                }
            }
            terms.retain(|(c, _)| !field.is_zero(c));
            CompiledPoly { terms }
        };
        let mut compile_entry = |coefficient: &Coefficient| -> Option<CompiledEntry> {
            let numerator = compile_poly(&coefficient.numerator);
            let denominator = compile_poly(&coefficient.denominator);
            if denominator.terms.is_empty() {
                return None;
            }
            Some(CompiledEntry {
                numerator,
                denominator: Some(denominator),
            })
        };
        let mut rows = Vec::with_capacity(self.system.rows.len());
        for row in &self.system.rows {
            let mut compiled = Vec::with_capacity(row.len());
            for (column, coefficient) in row {
                compiled.push((*column, compile_entry(coefficient)?));
            }
            rows.push(compiled);
        }
        let mut targets = Vec::with_capacity(self.routes.len());
        for route in &self.routes {
            let mut compiled = Vec::with_capacity(route.len());
            for (column, coefficient) in route {
                compiled.push((*column, compile_entry(coefficient)?));
            }
            targets.push(compiled);
        }
        // Fold constant denominators into the numerators.
        let constant: Option<u32> = index.get(&vec![0u16; free.len()]).copied();
        for entry in rows
            .iter_mut()
            .flatten()
            .chain(targets.iter_mut().flatten())
            .map(|(_, entry)| entry)
        {
            let Some(denominator) = &entry.denominator else {
                continue;
            };
            if denominator.terms.len() == 1 && Some(denominator.terms[0].1) == constant {
                let inverse = field.try_inv(&denominator.terms[0].0)?;
                for (c, _) in entry.numerator.terms.iter_mut() {
                    field.mul_assign(c, &inverse);
                }
                entry.denominator = None;
            }
        }
        let mut max_exponent = vec![0u16; free.len()];
        for monomial in &monomials {
            for (slot, &exponent) in monomial.iter().enumerate() {
                max_exponent[slot] = max_exponent[slot].max(exponent);
            }
        }
        Some(Compiled {
            field,
            monomials,
            max_exponent,
            rows,
            targets,
        })
    }
}

/// One coefficient compiled at a prime: `sum c * monomial(free values)`.
#[derive(Clone, Debug, Default)]
struct CompiledPoly {
    terms: Vec<(Zp, u32)>,
}

#[derive(Clone, Debug)]
struct CompiledEntry {
    numerator: CompiledPoly,
    /// `None` when the denominator is a nonzero constant (folded in).
    denominator: Option<CompiledPoly>,
}

/// The system and the target routes compiled at one prime.
pub(crate) struct Compiled {
    field: Zp64,
    /// Exponents of the free parameters of every distinct monomial.
    monomials: Vec<Vec<u16>>,
    max_exponent: Vec<u16>,
    rows: Vec<Vec<(u32, CompiledEntry)>>,
    targets: Vec<Vec<(u32, CompiledEntry)>>,
}

/// Outcome of one bunch: lane status and `targets x masters` values.
struct Bunch<const K: usize> {
    failures: [Option<LaneFailure>; K],
    /// Row-major `target * masters + master`; empty if every lane failed.
    values: Vec<[Zp; K]>,
}

fn lanes_zero<const K: usize>(field: &Zp64, values: &[Zp; K]) -> bool {
    values.iter().all(|value| field.is_zero(value))
}

const NONE: u32 = u32::MAX;

impl Compiled {
    pub(crate) fn field(&self) -> &Zp64 {
        &self.field
    }

    /// Monomial values per lane at the given free-parameter points.
    fn monomial_values<const K: usize>(&self, points: &[Vec<Zp>; K]) -> Vec<[Zp; K]> {
        let field = &self.field;
        let mut powers: Vec<Vec<[Zp; K]>> = Vec::with_capacity(self.max_exponent.len());
        for (slot, &top) in self.max_exponent.iter().enumerate() {
            let mut table = vec![[field.one(); K]];
            for e in 1..=usize::from(top) {
                let previous = table[e - 1];
                table.push(std::array::from_fn(|lane| {
                    field.mul(&previous[lane], &points[lane][slot])
                }));
            }
            powers.push(table);
        }
        self.monomials
            .iter()
            .map(|monomial| {
                let mut value = [field.one(); K];
                for (slot, &exponent) in monomial.iter().enumerate() {
                    if exponent > 0 {
                        let factor = &powers[slot][usize::from(exponent)];
                        for lane in 0..K {
                            value[lane] = field.mul(&value[lane], &factor[lane]);
                        }
                    }
                }
                value
            })
            .collect()
    }

    fn evaluate<const K: usize>(
        &self,
        entry: &CompiledEntry,
        monomials: &[[Zp; K]],
        failures: &mut [Option<LaneFailure>; K],
    ) -> [Zp; K] {
        let field = &self.field;
        let mut value = [field.zero(); K];
        for (c, m) in &entry.numerator.terms {
            let monomial = &monomials[*m as usize];
            for lane in 0..K {
                field.add_mul_assign(&mut value[lane], c, &monomial[lane]);
            }
        }
        if let Some(denominator) = &entry.denominator {
            let mut d = [field.zero(); K];
            for (c, m) in &denominator.terms {
                let monomial = &monomials[*m as usize];
                for lane in 0..K {
                    field.add_mul_assign(&mut d[lane], c, &monomial[lane]);
                }
            }
            for lane in 0..K {
                match field.try_inv(&d[lane]) {
                    Some(inverse) => value[lane] = field.mul(&value[lane], &inverse),
                    None => {
                        failures[lane].get_or_insert(LaneFailure::Pole);
                    }
                }
            }
        }
        value
    }

    /// One bunch of `K` points of the free parameters.
    fn probe<const K: usize, const N: usize>(
        &self,
        system: &ExactSystem<N>,
        points: &[Vec<Zp>; K],
    ) -> Bunch<K> {
        let field = &self.field;
        let zero = [field.zero(); K];
        let ncols = system.columns.len();
        let monomials = self.monomial_values(points);
        let mut failures: [Option<LaneFailure>; K] = [None; K];
        let mut accumulator = vec![zero; ncols];
        let mut queued = vec![false; ncols];
        let mut heap: BinaryHeap<Reverse<u32>> = BinaryHeap::new();
        let mut pivot_row = vec![NONE; ncols];
        let mut u_ptr: Vec<usize> = vec![0];
        let mut u_col: Vec<u32> = Vec::new();
        let mut u_val: Vec<[Zp; K]> = Vec::new();
        // Forward elimination along the prescribed pivots.
        for (row, entries) in self.rows.iter().enumerate() {
            let expected = system.pivots[row];
            for (column, entry) in entries {
                let value = self.evaluate(entry, &monomials, &mut failures);
                let c = *column as usize;
                let slot = &mut accumulator[c];
                for lane in 0..K {
                    field.add_assign(&mut slot[lane], &value[lane]);
                }
                if !queued[c] {
                    queued[c] = true;
                    heap.push(Reverse(*column));
                }
            }
            let mut found = false;
            while let Some(Reverse(column)) = heap.pop() {
                let c = column as usize;
                queued[c] = false;
                let value = std::mem::replace(&mut accumulator[c], zero);
                if lanes_zero(field, &value) {
                    continue;
                }
                let pivot = pivot_row[c];
                if pivot != NONE {
                    let (start, end) = (u_ptr[pivot as usize], u_ptr[pivot as usize + 1]);
                    for position in start + 1..end {
                        let next = u_col[position] as usize;
                        let u = &u_val[position];
                        let slot = &mut accumulator[next];
                        for lane in 0..K {
                            field.sub_mul_assign(&mut slot[lane], &value[lane], &u[lane]);
                        }
                        if !queued[next] {
                            queued[next] = true;
                            heap.push(Reverse(next as u32));
                        }
                    }
                } else if column == expected {
                    let mut inverse = [field.one(); K];
                    for lane in 0..K {
                        match field.try_inv(&value[lane]) {
                            Some(value) => inverse[lane] = value,
                            None => {
                                failures[lane].get_or_insert(LaneFailure::Pivot);
                            }
                        }
                    }
                    u_col.push(column);
                    u_val.push([field.one(); K]);
                    while let Some(Reverse(next)) = heap.pop() {
                        let n = next as usize;
                        queued[n] = false;
                        let value = std::mem::replace(&mut accumulator[n], zero);
                        if lanes_zero(field, &value) {
                            continue;
                        }
                        u_col.push(next);
                        u_val.push(std::array::from_fn(|lane| {
                            field.mul(&value[lane], &inverse[lane])
                        }));
                    }
                    u_ptr.push(u_col.len());
                    pivot_row[c] = (u_ptr.len() - 2) as u32;
                    found = true;
                    break;
                } else {
                    // Nonzero left of the prescribed pivot: these lanes
                    // specialise to another pivot structure.
                    for lane in 0..K {
                        if !field.is_zero(&value[lane]) {
                            failures[lane].get_or_insert(LaneFailure::Structure);
                        }
                    }
                }
            }
            if !found {
                // The prescribed pivot vanished in every lane.
                for failure in failures.iter_mut() {
                    failure.get_or_insert(LaneFailure::Pivot);
                }
                return Bunch {
                    failures,
                    values: Vec::new(),
                };
            }
        }
        // Back substitution restricted to the closure of the targets.
        let mut seen = vec![false; ncols];
        let mut stack: Vec<u32> = Vec::new();
        for route in &self.targets {
            for (column, _) in route {
                if !seen[*column as usize] {
                    seen[*column as usize] = true;
                    stack.push(*column);
                }
            }
        }
        let mut closure_pivots: Vec<u32> = Vec::new();
        let mut candidates: Vec<u32> = Vec::new();
        while let Some(column) = stack.pop() {
            let pivot = pivot_row[column as usize];
            if pivot == NONE {
                candidates.push(column);
                continue;
            }
            closure_pivots.push(column);
            let (start, end) = (u_ptr[pivot as usize], u_ptr[pivot as usize + 1]);
            for &next in &u_col[start + 1..end] {
                if !seen[next as usize] {
                    seen[next as usize] = true;
                    stack.push(next);
                }
            }
        }
        candidates.sort_unstable();
        closure_pivots.sort_unstable();
        let mut candidate_slot = vec![NONE; ncols];
        for (slot, &column) in candidates.iter().enumerate() {
            candidate_slot[column as usize] = slot as u32;
        }
        let mut reduced_slot = vec![NONE; ncols];
        let mut reduced: Vec<Vec<(u32, [Zp; K])>> = Vec::with_capacity(closure_pivots.len());
        let mut dense = vec![zero; candidates.len()];
        let mut touched_flag = vec![false; candidates.len()];
        let mut touched: Vec<u32> = Vec::new();
        for &column in closure_pivots.iter().rev() {
            let pivot = pivot_row[column as usize] as usize;
            for position in u_ptr[pivot] + 1..u_ptr[pivot + 1] {
                let next = u_col[position] as usize;
                let u = &u_val[position];
                let slot = candidate_slot[next];
                if slot != NONE {
                    let target = &mut dense[slot as usize];
                    for lane in 0..K {
                        field.sub_assign(&mut target[lane], &u[lane]);
                    }
                    if !touched_flag[slot as usize] {
                        touched_flag[slot as usize] = true;
                        touched.push(slot);
                    }
                } else {
                    let tail = reduced_slot[next];
                    debug_assert!(tail != NONE, "closure pivots are reduced easiest first");
                    for (candidate, weight) in &reduced[tail as usize] {
                        let target = &mut dense[*candidate as usize];
                        for lane in 0..K {
                            field.sub_mul_assign(&mut target[lane], &u[lane], &weight[lane]);
                        }
                        if !touched_flag[*candidate as usize] {
                            touched_flag[*candidate as usize] = true;
                            touched.push(*candidate);
                        }
                    }
                }
            }
            touched.sort_unstable();
            let mut row = Vec::with_capacity(touched.len());
            for &slot in &touched {
                touched_flag[slot as usize] = false;
                let value = std::mem::replace(&mut dense[slot as usize], zero);
                if !lanes_zero(field, &value) {
                    row.push((slot, value));
                }
            }
            touched.clear();
            reduced_slot[column as usize] = reduced.len() as u32;
            reduced.push(row);
        }
        // Targets over the masters.
        let masters = &system.masters;
        let mut master_position = vec![NONE; candidates.len()];
        for (slot, column) in candidates.iter().enumerate() {
            if let Ok(position) = masters.binary_search(column) {
                master_position[slot] = position as u32;
            }
        }
        let mut values = vec![zero; self.targets.len() * masters.len()];
        let mut stray = [false; K];
        let one = [field.one(); K];
        for (target, route) in self.targets.iter().enumerate() {
            let base = target * masters.len();
            for (column, entry) in route {
                let factor = self.evaluate(entry, &monomials, &mut failures);
                let c = *column as usize;
                let terms: Vec<(u32, &[Zp; K])> = if candidate_slot[c] != NONE {
                    vec![(candidate_slot[c], &one)]
                } else {
                    reduced[reduced_slot[c] as usize]
                        .iter()
                        .map(|(slot, weight)| (*slot, weight))
                        .collect()
                };
                for (slot, weight) in terms {
                    let position = master_position[slot as usize];
                    if position == NONE {
                        for lane in 0..K {
                            if !field.is_zero(&weight[lane]) && !field.is_zero(&factor[lane]) {
                                stray[lane] = true;
                            }
                        }
                        continue;
                    }
                    let out = &mut values[base + position as usize];
                    for lane in 0..K {
                        field.add_mul_assign(&mut out[lane], &factor[lane], &weight[lane]);
                    }
                }
            }
        }
        for lane in 0..K {
            if stray[lane] {
                failures[lane].get_or_insert(LaneFailure::Master);
            }
        }
        Bunch { failures, values }
    }

    /// Probe any number of points of one free parameter (`free = 1`) or of
    /// none (`free = 0`), in bunches of `K`; the last bunch is padded with
    /// its first point. Returns `targets x masters` values per point.
    pub(crate) fn probe_points<const K: usize, const N: usize>(
        &self,
        system: &ExactSystem<N>,
        xs: &[Zp],
        free: usize,
    ) -> Vec<Result<Vec<Zp>, LaneFailure>> {
        debug_assert!(
            free <= 1,
            "the bunch kernel probes at most one free parameter"
        );
        let entries = system.masters.len() * self.targets.len();
        let mut out = Vec::with_capacity(xs.len());
        for chunk in xs.chunks(K) {
            let points: [Vec<Zp>; K] = std::array::from_fn(|lane| {
                let x = chunk.get(lane).copied().unwrap_or(chunk[0]);
                vec![x; free]
            });
            let bunch = self.probe::<K, N>(system, &points);
            for lane in 0..chunk.len() {
                out.push(match bunch.failures[lane] {
                    Some(failure) => Err(failure),
                    None => Ok((0..entries)
                        .map(|entry| bunch.values[entry][lane])
                        .collect()),
                });
            }
        }
        out
    }
}

/// The independent reference probe at one point (all parameters given):
/// Symbolica's sparse row reducer with pivot search, the session's replay
/// guard and the modular-pass back substitution. Returns `targets x
/// masters` values, or the failure.
pub(crate) fn reference_probe<const N: usize>(
    request: &LaneRequest<N>,
    field: &Zp64,
    point: &[Zp],
) -> Result<Vec<Zp>, LaneFailure> {
    let system = &request.system;
    let evaluate = |coefficient: &Coefficient| -> Result<Zp, LaneFailure> {
        let map = |value: &Integer| value.to_finite_field(field);
        let numerator = coefficient.numerator.evaluate_with_coeff_map(
            map,
            &point[..coefficient.numerator.nvars()],
            field,
        );
        let denominator = coefficient.denominator.evaluate_with_coeff_map(
            map,
            &point[..coefficient.denominator.nvars()],
            field,
        );
        let inverse = field.try_inv(&denominator).ok_or(LaneFailure::Pole)?;
        Ok(field.mul(&numerator, &inverse))
    };
    let mut rows: Vec<Vec<Term<N, Zp>>> = Vec::with_capacity(system.rows.len());
    for row in &system.rows {
        let mut terms = Vec::with_capacity(row.len());
        for (column, coefficient) in row {
            let value = evaluate(coefficient)?;
            if !field.is_zero(&value) {
                terms.push(Term {
                    integral: system.columns[*column as usize],
                    coefficient: value,
                });
            }
        }
        rows.push(terms);
    }
    let expected: Vec<Integral<N>> = system
        .pivots
        .iter()
        .map(|&column| system.columns[column as usize])
        .collect();
    let replay = Replay::new(system.order.clone(), field.clone(), &rows, &expected)
        .map_err(|_| LaneFailure::Pivot)?;
    let mut demand = Vec::new();
    for route in &request.routes {
        for (column, _) in route {
            if let Some(index) = replay.column_index(&system.columns[*column as usize]) {
                demand.push(index);
            }
        }
    }
    demand.sort_unstable();
    demand.dedup();
    let reached = closure(&replay, &demand);
    let reduced = reduced_rows(&replay, &reached, &demand);
    let mut by_column: HashMap<Integral<N>, &Vec<(usize, Zp)>> = HashMap::new();
    for (index, &column) in demand.iter().enumerate() {
        by_column.insert(replay.column(column), &reduced[index]);
    }
    let masters: Vec<Integral<N>> = system
        .masters
        .iter()
        .map(|&column| system.columns[column as usize])
        .collect();
    let mut values = vec![field.zero(); request.routes.len() * masters.len()];
    for (target, route) in request.routes.iter().enumerate() {
        for (column, coefficient) in route {
            let factor = evaluate(coefficient)?;
            let integral = system.columns[*column as usize];
            let terms: Vec<(Integral<N>, Zp)> = match by_column.get(&integral) {
                Some(row) => row
                    .iter()
                    .map(|(candidate, weight)| (replay.column(*candidate), *weight))
                    .collect(),
                // In no selected row: a master by itself.
                None => vec![(integral, field.one())],
            };
            for (integral, weight) in terms {
                if field.is_zero(&weight) || field.is_zero(&factor) {
                    continue;
                }
                let position = masters
                    .binary_search_by(|master| system.order.compare(master, &integral))
                    .map_err(|_| LaneFailure::Master)?;
                let out = &mut values[target * masters.len() + position];
                field.add_mul_assign(out, &factor, &weight);
            }
        }
    }
    Ok(values)
}

/// The `(target, master)` values at one explicit point
/// (every parameter fixed) modulo `prime`, from the bunch kernel and
/// cross-checked with the reference probe. Canonical residues.
pub(crate) fn probe_explicit<const N: usize>(
    request: &LaneRequest<N>,
    prime: u64,
) -> Result<Vec<u64>, SolverError> {
    if !request.free().is_empty() {
        return Err(SolverError::InvalidInput(
            "an explicit sample fixes every parameter".into(),
        ));
    }
    if !(3..1 << 63).contains(&prime) || !Integer::from(prime).is_prime(0) {
        return Err(SolverError::InvalidInput(format!(
            "explicit sample prime {prime} is not an odd prime below 2^63"
        )));
    }
    let unlucky = |failure: LaneFailure| SolverError::UnluckyBoxSample {
        phase: match failure {
            LaneFailure::Pole => BoxPhase::Conditions,
            LaneFailure::Master => BoxPhase::MasterTest,
            _ => BoxPhase::Replay,
        },
    };
    let compiled = request
        .compile(prime)
        .ok_or(SolverError::UnluckyBoxSample {
            phase: BoxPhase::Conditions,
        })?;
    let field = compiled.field().clone();
    let kernel = compiled
        .probe_points::<1, N>(&request.system, &[field.zero()], 0)
        .pop()
        .expect("one point")
        .map_err(unlucky)?;
    let point: Vec<Zp> = request
        .fixed
        .iter()
        .map(|value| rational_image(value.as_ref().expect("all fixed"), &field))
        .collect::<Option<_>>()
        .ok_or(SolverError::UnluckyBoxSample {
            phase: BoxPhase::Conditions,
        })?;
    let reference = reference_probe(request, &field, &point).map_err(unlucky)?;
    if kernel != reference {
        return Err(SolverError::Certification(format!(
            "explicit sample at prime {prime}: bunch kernel and reference probe disagree"
        )));
    }
    Ok(kernel
        .iter()
        .map(|value| field.from_element(value))
        .collect())
}
