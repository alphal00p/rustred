//! Rules-study (knock-out) export, first on the rules-s5-tables branch: exact row
//! multipliers of solved rules, so a dumped table can carry a certificate.
//!
//! No search, guard or ordering code is touched. Everything here runs after a
//! sector solve on the retained `RuleCandidate::sources`:
//!
//!   rule(n) := I(target) - sum_t c_t(n) I(t)
//!           == sum_j W_j(n) B_{b_j}[at_j(n)]   modulo proved-zero sectors
//!   B_k(n)  == sum_m P_km(n) R_m(n)            exact polynomial identity
//!
//! `R_m` are the ordinary rows of `SourceSystem`, `B_k` the preconditioned
//! sector basis, `at_j` the seed translated into the rule's canonical frame.
//! Both identities are multiplied out exactly before anything is returned.

use std::collections::HashMap;

use symbolica::domains::rational_polynomial::RationalPolynomialField;
use symbolica::prelude::{IntegerRing, Z};
use symbolica::tensors::sparse::{LuLMode, SparseRowReducer};

use crate::algebra::{Coefficient, CoefficientPolynomial};

use super::instantiate::{canonicalize, instantiate, translate};
use super::{
    ExactRow, Integral, RuleCandidate, SectorConfig, SectorRule, SectorSolver, SolverError,
    SourceSystem, Term, extract_exceptions,
};

/// `rows[k]` lists the nonzero `(m, P_km)` of basis row `k`.
pub struct BasisDerivation {
    pub rows: Vec<Vec<(usize, Coefficient)>>,
}

/// One multiplier of a rule certificate.
pub struct FrameEntry<const N: usize> {
    /// Ordinal of the seed in `RuleCandidate::sources`.
    pub source: usize,
    pub basis_row: usize,
    /// Seed in the rule's canonical frame: symbolic axes carry their offset
    /// from `n`, numeric axes the absolute power.
    pub at: Integral<N>,
    pub weight: Coefficient,
}

pub struct RuleFrame<const N: usize> {
    pub entries: Vec<FrameEntry<N>>,
    /// Translation from the raw pivot to the canonical target.
    pub recenter: [i16; N],
    /// `first_hit_row` or `desired_row`.
    pub via: &'static str,
    /// Trace rows whose multiplier came out zero (omitted from `entries`).
    pub zero_weights: usize,
    /// Index-dependent denominator loci of the multipliers that are not rule
    /// exceptions (OR of ANDs, same form as `ExceptionalConditions`). The
    /// certificate says nothing at an integer point on such a locus, even
    /// though the rule itself is regular there. Not intersected with the case.
    pub weight_poles: Vec<Vec<CoefficientPolynomial>>,
    /// Every index-dependent denominator locus of the multipliers, before the
    /// rule's exceptions are removed (`weight_poles` is a subset).
    pub all_weight_poles: Vec<Vec<CoefficientPolynomial>>,
}

/// One multiplier of an ORIGINAL ordinary source row (bebe-cert): the frame
/// weights composed with the checked basis derivation `B_k = sum_m P_km R_m`.
pub struct OriginalEntry<const N: usize> {
    /// Row ordinal in [`SourceSystem::rows`].
    pub row: usize,
    /// The seed (as stored) at which row `row` is instantiated.
    pub seed: super::Seed<N>,
    /// The seed in the rule's canonical frame (as [`FrameEntry::at`]).
    pub at: Integral<N>,
    pub weight: Coefficient,
}

fn failed(message: impl Into<String>) -> SolverError {
    SolverError::ExactReplay(message.into())
}

impl<'a, const N: usize> SectorSolver<'a, N> {
    /// `Self::new` plus the checked polynomial derivation of its basis.
    pub fn with_basis_derivation(
        system: &'a SourceSystem<N>,
        sector: [bool; N],
        config: SectorConfig<N>,
    ) -> Result<(Self, BasisDerivation), SolverError> {
        let (solver, trace) = Self::new_with_provenance(system, sector, config)?;
        let template: Coefficient = system
            .rows()
            .iter()
            .flatten()
            .next()
            .ok_or_else(|| SolverError::InvalidInput("the source system is empty".into()))?
            .coefficient
            .clone()
            .into();
        let one: Coefficient = template.numerator.one().into();
        let zero: Coefficient = template.numerator.zero().into();
        let mut rows = Vec::with_capacity(solver.basis.len());
        for k in 0..solver.basis.len() {
            let original =
                trace.compose(&[(k, one.clone())], &one, |scale| Ok(scale.clone().into()))?;
            let mut sum: HashMap<Integral<N>, Coefficient> = HashMap::new();
            let mut retained = Vec::new();
            for (m, weight) in original.into_iter().enumerate() {
                if weight.is_zero() {
                    continue;
                }
                for term in &system.rows()[m] {
                    let product = &weight * &Coefficient::from(term.coefficient.clone());
                    let slot = sum.entry(term.integral).or_insert_with(|| zero.clone());
                    *slot = &*slot + &product;
                }
                retained.push((m, weight));
            }
            for term in &solver.basis[k] {
                let value = sum.remove(&term.integral).unwrap_or_else(|| zero.clone());
                if value != Coefficient::from(term.coefficient.clone()) {
                    return Err(failed(format!("basis row {k}: derivation mismatch")));
                }
            }
            if sum.values().any(|value| !value.is_zero()) {
                return Err(failed(format!("basis row {k}: derivation has extra terms")));
            }
            rows.push(retained);
        }
        Ok((solver, BasisDerivation { rows }))
    }

    /// Exact multipliers of `rule` over its retained trace; checked by product.
    pub fn rule_frame(&self, rule: &SectorRule<N>) -> Result<RuleFrame<N>, SolverError> {
        let candidate = &rule.candidate;
        let indices = self.system.index_variables();
        let affine = candidate.case.affine();
        // The same projection as discovery (search.rs): none on affine charts.
        let zero_sectors: &[[bool; N]] = if affine.is_some() {
            &[]
        } else {
            &self.config.zero_sectors
        };
        let mut rows: Vec<ExactRow<N>> = Vec::with_capacity(candidate.sources.len());
        for source in &candidate.sources {
            let row = self
                .basis
                .get(source.basis_row)
                .ok_or_else(|| failed("basis row out of range"))?;
            rows.push(instantiate(
                row,
                &source.seed,
                indices,
                self.system.fixed(),
                &self.order,
                zero_sectors,
                affine,
            )?);
        }
        let template = rows
            .iter()
            .flatten()
            .next()
            .ok_or_else(|| failed("empty source trace"))?
            .coefficient
            .clone();
        let one: Coefficient = template.numerator.one().into();
        let zero: Coefficient = template.numerator.zero().into();
        let numeric = candidate.case.is_numerical();
        let mut desired: ExactRow<N> = Vec::new();
        if numeric {
            desired.push(Term {
                integral: candidate.target,
                coefficient: one.clone(),
            });
            desired.extend(candidate.rhs.iter().map(|term| Term {
                integral: term.integral,
                coefficient: -term.coefficient.clone(),
            }));
            desired.sort_unstable_by(|a, b| self.order.compare(&a.integral, &b.integral));
        }
        let mut columns: Vec<_> = rows
            .iter()
            .flatten()
            .chain(desired.iter())
            .map(|term| term.integral)
            .collect();
        columns.sort_unstable_by(|a, b| self.order.compare(a, b));
        columns.dedup();
        let column_of = |key: &Integral<N>| -> usize {
            columns
                .binary_search_by(|column| self.order.compare(column, key))
                .expect("the column union contains every row key")
        };
        let desired_column = columns.len() + rows.len();
        let ncols = u32::try_from(desired_column + 2).map_err(|_| failed("too many columns"))?;
        let field = RationalPolynomialField::<IntegerRing, u16>::new(Z);
        let mut reducer = SparseRowReducer::new(ncols, field, LuLMode::None);
        let mut solution: Option<(Vec<Coefficient>, [i16; N], Coefficient, &'static str)> = None;
        for (ordinal, row) in rows.iter().enumerate() {
            let mut values: Vec<_> = row.iter().map(|term| term.coefficient.clone()).collect();
            let mut positions: Vec<_> = row
                .iter()
                .map(|term| column_of(&term.integral) as u32)
                .collect();
            values.push(one.clone());
            positions.push((columns.len() + ordinal) as u32);
            let Some(pivot) = reducer.add_row(&values, &positions) else {
                continue;
            };
            if numeric {
                continue;
            }
            let Some(raw_target) = columns.get(pivot as usize) else {
                continue;
            };
            if !candidate.case.matches(raw_target) {
                continue;
            }
            let u = reducer.u();
            let last = u.nrows() as usize - 1;
            let (start, end) = (u.row_ptrs()[last], u.row_ptrs()[last + 1]);
            let mut physical: ExactRow<N> = Vec::new();
            let mut weights = vec![zero.clone(); rows.len()];
            for (&column, value) in u.col_idcs()[start..end].iter().zip(&u.values()[start..end]) {
                match columns.get(column as usize) {
                    Some(integral) => physical.push(Term {
                        integral: *integral,
                        coefficient: value.clone(),
                    }),
                    None => weights[column as usize - columns.len()] = value.clone(),
                }
            }
            let (target, rhs) = canonicalize(physical.clone(), indices)?;
            if target != candidate.target || rhs != candidate.rhs {
                continue;
            }
            let shifts: [i16; N] = std::array::from_fn(|axis| {
                if raw_target[axis].is_symbolic() {
                    -raw_target[axis].value()
                } else {
                    0
                }
            });
            let pivot_value = physical.swap_remove(0).coefficient;
            solution = Some((weights, shifts, pivot_value, "first_hit_row"));
            break;
        }
        if numeric {
            let mut values: Vec<_> = desired
                .iter()
                .map(|term| term.coefficient.clone())
                .collect();
            let mut positions: Vec<_> = desired
                .iter()
                .map(|term| column_of(&term.integral) as u32)
                .collect();
            values.push(one.clone());
            positions.push(desired_column as u32);
            reducer.add_row(&values, &positions);
            let u = reducer.u();
            let last = u.nrows() as usize - 1;
            let (start, end) = (u.row_ptrs()[last], u.row_ptrs()[last + 1]);
            let mut weights = vec![zero.clone(); rows.len()];
            let mut scale = None;
            for (&column, value) in u.col_idcs()[start..end].iter().zip(&u.values()[start..end]) {
                let column = column as usize;
                if column < columns.len() {
                    return Err(failed("desired row has a physical remainder"));
                } else if column == desired_column {
                    scale = Some(value.clone());
                } else {
                    weights[column - columns.len()] = -value.clone();
                }
            }
            let scale = scale.ok_or_else(|| failed("desired row lost its scale"))?;
            solution = Some((weights, [0; N], scale, "desired_row"));
        }
        let (weights, recenter, pivot, via) =
            solution.ok_or_else(|| failed("trace has no pivot reproducing the rule"))?;
        let pivot = translate(&pivot, indices, &recenter);
        let weights: Vec<Coefficient> = weights
            .iter()
            .map(|weight| &translate(weight, indices, &recenter) / &pivot)
            .collect();

        // Multiply out in the canonical frame; the rule must come back exactly.
        let mut sum: HashMap<Integral<N>, Coefficient> = HashMap::new();
        for (row, weight) in rows.iter().zip(&weights) {
            if weight.is_zero() {
                continue;
            }
            for term in row {
                let key = term.integral.shifted(recenter)?;
                let product = weight * &translate(&term.coefficient, indices, &recenter);
                let slot = sum.entry(key).or_insert_with(|| zero.clone());
                *slot = &*slot + &product;
            }
        }
        let target = sum
            .remove(&candidate.target)
            .unwrap_or_else(|| zero.clone());
        if target != one {
            return Err(failed("product check: target coefficient differs from 1"));
        }
        for term in &candidate.rhs {
            let value = sum.remove(&term.integral).unwrap_or_else(|| zero.clone());
            if !(&value + &term.coefficient).is_zero() {
                return Err(failed("product check: an RHS coefficient differs"));
            }
        }
        if sum.values().any(|value| !value.is_zero()) {
            return Err(failed("product check: an extra term survives"));
        }

        let weight_candidate = RuleCandidate {
            case: candidate.case.clone(),
            target: candidate.target,
            rhs: weights
                .iter()
                .filter(|weight| !weight.is_zero())
                .map(|weight| Term {
                    integral: candidate.target,
                    coefficient: weight.clone(),
                })
                .collect(),
            sources: Vec::new(),
            stats: Default::default(),
            hidden_poles: Vec::new(),
        };
        let all_weight_poles = extract_exceptions(&weight_candidate, indices, self.order.sector())
            .map_err(|error| failed(format!("weight poles: {error}")))?
            .branches;
        let weight_poles = all_weight_poles
            .iter()
            .filter(|branch| !rule.exceptions.branches.contains(branch))
            .cloned()
            .collect();

        let mut entries = Vec::new();
        let mut zero_weights = 0;
        for (ordinal, (source, weight)) in candidate.sources.iter().zip(weights).enumerate() {
            if weight.is_zero() {
                zero_weights += 1;
                continue;
            }
            entries.push(FrameEntry {
                source: ordinal,
                basis_row: source.basis_row,
                at: source.seed.integral.shifted(recenter)?,
                weight,
            });
        }
        Ok(RuleFrame {
            entries,
            recenter,
            via,
            zero_weights,
            weight_poles,
            all_weight_poles,
        })
    }

    /// The multipliers of `frame` on the ORIGINAL ordinary source rows
    /// (bebe-cert): `w_j * P_{k_j m}` with `P` from `derivation` (of this
    /// solver), instantiated at seed `j` exactly as the source row, summed per
    /// (row, seed). The identity `rule = sum W R_m[seed]` is multiplied out
    /// exactly in the canonical frame (modulo the same proved-zero sector
    /// projection as `rule_frame`) before anything is returned.
    pub fn original_frame(
        &self,
        rule: &SectorRule<N>,
        frame: &RuleFrame<N>,
        derivation: &BasisDerivation,
    ) -> Result<Vec<OriginalEntry<N>>, SolverError> {
        let candidate = &rule.candidate;
        let indices = self.system.index_variables();
        let fixed = self.system.fixed();
        let affine = candidate.case.affine();
        let zero_sectors: &[[bool; N]] = if affine.is_some() {
            &[]
        } else {
            &self.config.zero_sectors
        };
        let recenter = frame.recenter;
        let mut zero: Option<Coefficient> = None;
        let mut order: Vec<(usize, Integral<N>)> = Vec::new();
        let mut combined: HashMap<(usize, Integral<N>), (super::Seed<N>, Coefficient)> =
            HashMap::new();
        for entry in &frame.entries {
            let source = candidate
                .sources
                .get(entry.source)
                .ok_or_else(|| failed("frame source out of range"))?;
            let derived = derivation
                .rows
                .get(entry.basis_row)
                .ok_or_else(|| failed("derivation row out of range"))?;
            for (m, p) in derived {
                let numerator = super::instantiate::instantiate_polynomial(
                    &p.numerator,
                    &source.seed,
                    indices,
                    fixed,
                    affine,
                )?;
                let denominator = super::instantiate::instantiate_polynomial(
                    &p.denominator,
                    &source.seed,
                    indices,
                    fixed,
                    affine,
                )?;
                let value = &numerator / &denominator;
                let value = &entry.weight * &translate(&value, indices, &recenter);
                if value.is_zero() {
                    continue;
                }
                let key = (*m, source.seed.integral);
                match combined.get_mut(&key) {
                    Some((_, slot)) => *slot = &*slot + &value,
                    None => {
                        zero.get_or_insert_with(|| value.numerator.zero().into());
                        order.push(key);
                        combined.insert(key, (source.seed, value));
                    }
                }
            }
        }
        let zero = zero.ok_or_else(|| failed("original frame is empty"))?;
        let one: Coefficient = zero.numerator.one().into();
        let mut out = Vec::with_capacity(order.len());
        let mut sum: HashMap<Integral<N>, Coefficient> = HashMap::new();
        for key in order {
            let (seed, weight) = combined.remove(&key).expect("recorded key");
            if weight.is_zero() {
                continue;
            }
            let row = instantiate(
                &self.system.rows()[key.0],
                &seed,
                indices,
                fixed,
                &self.order,
                zero_sectors,
                affine,
            )?;
            for term in &row {
                let at = term.integral.shifted(recenter)?;
                let product = &weight * &translate(&term.coefficient, indices, &recenter);
                let slot = sum.entry(at).or_insert_with(|| zero.clone());
                *slot = &*slot + &product;
            }
            out.push(OriginalEntry {
                row: key.0,
                seed,
                at: seed.integral.shifted(recenter)?,
                weight,
            });
        }
        let target = sum
            .remove(&candidate.target)
            .unwrap_or_else(|| zero.clone());
        if target != one {
            return Err(failed(
                "original product check: target coefficient differs from 1",
            ));
        }
        for term in &candidate.rhs {
            let value = sum.remove(&term.integral).unwrap_or_else(|| zero.clone());
            if !(&value + &term.coefficient).is_zero() {
                return Err(failed("original product check: an RHS coefficient differs"));
            }
        }
        if sum.values().any(|value| !value.is_zero()) {
            return Err(failed("original product check: an extra term survives"));
        }
        Ok(out)
    }
}
