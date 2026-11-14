//! The selected system over Q(params): exact rows re-instantiated from
//! provenance, the shared input of the finite-field and exact lanes.
//!
//! The modular pass keeps only the provenance `(seed, source)` of the
//! selected rows (the L-dependency closure of the closure pivots; Kira 3
//! selection, arXiv:2505.20197 §3.2) and their pivot columns. This module
//! instantiates exactly those rows over the family parameters, with the
//! same zero-sector pruning, routing and automorphism rows as the Zp row
//! factory. Every row keeps its modular pivot: for generic parameter values
//! the pivot set does not depend on the point, so the lanes eliminate along
//! the known pivots without a pivot search (Kira 2 finite-field black box,
//! arXiv:2008.06494 §2.2).
//!
//! Rows are kept in the modular pivot-column order (hardest first). Every
//! L dependency of a selected row has a harder pivot, so reducing the rows
//! in this order reproduces the modular pivot of each row at every lucky
//! point.

use std::cmp::Ordering;
use std::collections::HashMap;
use std::sync::Arc;

use symbolica::domains::RingOps;
use symbolica::domains::rational_polynomial::RationalPolynomialField;
use symbolica::prelude::{Integer, IntegerRing, Z};

use crate::algebra::{Coefficient, CoefficientContext, CoefficientPolynomial};

use super::super::discovery::ColumnComparator;
use super::super::{Integral, SolverError, SourceSystem};
use super::order::BoxOrder;
use super::router::{SectorRouter, integral_bits};
use super::rows::{SectorState, ZpRowFactory};

pub(crate) type ExactField = RationalPolynomialField<IntegerRing, u16>;

/// The final modular selection of a session, kept for materialisation.
#[derive(Clone, Debug)]
// Read by the materialising lanes (a later step of the series).
#[allow(dead_code)]
pub(crate) struct SelectionState<const N: usize> {
    /// `(seed powers, source)` of the selected rows, in pivot-column order.
    pub rows: Vec<([i16; N], u16)>,
    /// Modular pivot of each selected row.
    pub pivots: Vec<Integral<N>>,
    /// Masters in column order (hardest first).
    pub masters: Vec<Integral<N>>,
    /// Demanded representative-sector integrals.
    pub demand: Vec<Integral<N>>,
    pub selected_hash: String,
    pub master_hash: String,
}

/// Selected rows over the family parameters, in pivot-column order.
pub(crate) struct ExactSystem<const N: usize> {
    /// Every integral of the rows, demand and masters, hardest first.
    pub columns: Vec<Integral<N>>,
    /// Rows as `(column, coefficient)`, ascending column, nonzero.
    pub rows: Vec<Vec<(u32, Coefficient)>>,
    /// Expected pivot column of each row.
    pub pivots: Vec<u32>,
    /// Master columns, ascending.
    pub masters: Vec<u32>,
    pub order: Arc<BoxOrder<N>>,
}

impl<const N: usize> ExactSystem<N> {
    pub(crate) fn column_index(&self, integral: &Integral<N>) -> Option<usize> {
        self.columns
            .binary_search_by(|column| self.order.compare(column, integral))
            .ok()
    }
}

/// Exact image of a routed integral: `(representative integral, factor)`.
type Route<const N: usize> = Arc<Vec<([i16; N], Coefficient)>>;

/// Exact route of one requested target onto system columns.
pub(crate) type TargetRoute = Vec<(u32, Coefficient)>;

/// Builds exact rows with the Zp factory's routing semantics.
pub(crate) struct ExactRowBuilder<'a, const N: usize> {
    sources: &'a SourceSystem<N>,
    factory: &'a ZpRowFactory<N>,
    router: &'a dyn SectorRouter<N>,
    order: &'a BoxOrder<N>,
    context: CoefficientContext,
    field: ExactField,
    zero: CoefficientPolynomial,
    nparams: usize,
    /// Coefficient variable -> axis of an index variable.
    axis_of: Vec<Option<usize>>,
    routes: HashMap<[i16; N], Route<N>>,
}

type ExactTerms<const N: usize> = Vec<(Integral<N>, Coefficient)>;

impl<'a, const N: usize> ExactRowBuilder<'a, N> {
    pub(crate) fn new(
        sources: &'a SourceSystem<N>,
        factory: &'a ZpRowFactory<N>,
        router: &'a dyn SectorRouter<N>,
        order: &'a BoxOrder<N>,
        context: &CoefficientContext,
    ) -> Result<Self, SolverError> {
        let nparams = context.parameter_names().len();
        let variables = sources.coefficient_variables();
        if variables.len() < nparams || variables[..nparams] != context.variables()[..] {
            return Err(SolverError::InvalidInput(
                "source coefficient variables must start with the family parameters".into(),
            ));
        }
        let mut axis_of = vec![None; variables.len()];
        for (axis, &variable) in sources.index_variables().iter().enumerate() {
            if variable < axis_of.len() {
                axis_of[variable] = Some(axis);
            }
        }
        if let Some(variable) = (nparams..variables.len()).find(|&v| axis_of[v].is_none()) {
            return Err(SolverError::InvalidInput(format!(
                "source coefficient variable {variable} is neither a family parameter nor an index"
            )));
        }
        Ok(Self {
            sources,
            factory,
            router,
            order,
            context: context.clone(),
            field: ExactField::new(Z),
            zero: context.zero().numerator,
            nparams,
            axis_of,
            routes: HashMap::new(),
        })
    }

    /// Substitute the seed's index values into a template coefficient,
    /// leaving a polynomial in the family parameters only.
    fn instantiate(&self, polynomial: &CoefficientPolynomial, seed: &[i16; N]) -> Coefficient {
        let mut out = self.zero.zero_with_capacity(polynomial.nterms());
        for term in polynomial {
            let mut coefficient: Integer = term.coefficient.clone();
            for (variable, &exponent) in term.exponents.iter().enumerate().skip(self.nparams) {
                if exponent == 0 {
                    continue;
                }
                // `new` checked that every variable after the parameters is
                // an index variable.
                let axis = self.axis_of[variable].expect("index variable");
                let value = Integer::from(i64::from(seed[axis]));
                coefficient = &coefficient * &value.pow(u64::from(exponent));
            }
            if !coefficient.is_zero() {
                out.append_monomial(coefficient, &term.exponents[..self.nparams]);
            }
        }
        out.into()
    }

    fn route(&mut self, powers: &[i16; N]) -> Result<Route<N>, SolverError> {
        if let Some(route) = self.routes.get(powers) {
            return Ok(route.clone());
        }
        let mut image = Vec::new();
        for (target, coefficient) in self.router.route(powers)? {
            match self.factory.sector_state(integral_bits(&target)) {
                SectorState::Zero => continue,
                SectorState::Representative => {}
                SectorState::Routed => {
                    return Err(SolverError::InvalidInput(format!(
                        "route of {powers:?} ends in a non-representative sector at {target:?}"
                    )));
                }
            }
            if !coefficient.is_zero() {
                image.push((target, coefficient));
            }
        }
        let image = Arc::new(image);
        self.routes.insert(*powers, image.clone());
        Ok(image)
    }

    fn push(
        &mut self,
        terms: &mut ExactTerms<N>,
        powers: [i16; N],
        coefficient: Coefficient,
    ) -> Result<(), SolverError> {
        match self.factory.sector_state(integral_bits(&powers)) {
            SectorState::Zero => {}
            SectorState::Representative => {
                terms.push((Integral::numeric(powers)?, coefficient));
            }
            SectorState::Routed => {
                for (image, factor) in self.route(&powers)?.iter() {
                    terms.push((
                        Integral::numeric(*image)?,
                        self.field.mul(&coefficient, factor),
                    ));
                }
            }
        }
        Ok(())
    }

    fn merge(&self, mut terms: ExactTerms<N>) -> ExactTerms<N> {
        let order = self.order;
        terms.sort_by(|left, right| order.compare(&left.0, &right.0));
        let mut merged: ExactTerms<N> = Vec::with_capacity(terms.len());
        for (integral, coefficient) in terms {
            match merged.last_mut() {
                Some(last) if order.compare(&last.0, &integral) == Ordering::Equal => {
                    last.1 = self.field.add(&last.1, &coefficient);
                }
                _ => merged.push((integral, coefficient)),
            }
        }
        merged.retain(|(_, coefficient)| !coefficient.is_zero());
        merged
    }

    /// The exact row of `(seed, source)`: harder first, merged, nonzero.
    pub(crate) fn row(
        &mut self,
        seed: &[i16; N],
        source: u16,
    ) -> Result<ExactTerms<N>, SolverError> {
        let mut terms = Vec::new();
        if self.factory.is_automorphism_source(source) {
            let ordinal = usize::from(source) - self.factory.source_count();
            let relation = self
                .router
                .automorphism_rows(seed)?
                .into_iter()
                .nth(ordinal)
                .ok_or_else(|| {
                    SolverError::InvalidInput(format!(
                        "seed {seed:?} has no automorphism row {ordinal} (source {source})"
                    ))
                })?;
            for (powers, coefficient) in relation {
                if !coefficient.is_zero() {
                    self.push(&mut terms, powers, coefficient)?;
                }
            }
        } else {
            let fixed = *self.sources.fixed();
            let template = &self.sources.rows()[usize::from(source)];
            for term in template {
                let mut powers = [0i16; N];
                for axis in 0..N {
                    let shift = term.integral[axis].value();
                    powers[axis] = if fixed[axis].is_some() {
                        shift
                    } else {
                        seed[axis] + shift
                    };
                }
                if self.factory.sector_state(integral_bits(&powers)) == SectorState::Zero {
                    continue;
                }
                let coefficient = self.instantiate(&term.coefficient, seed);
                if !coefficient.is_zero() {
                    self.push(&mut terms, powers, coefficient)?;
                }
            }
        }
        Ok(self.merge(terms))
    }

    /// Exact route of a requested target onto representative-sector
    /// integrals (empty for a zero sector).
    pub(crate) fn target_route(&mut self, target: &[i16; N]) -> Result<ExactTerms<N>, SolverError> {
        let mut terms = Vec::new();
        self.push(&mut terms, *target, self.context.one())?;
        Ok(self.merge(terms))
    }

    /// Instantiate every selected row and index the columns.
    pub(crate) fn system(
        &mut self,
        selection: &SelectionState<N>,
        order: Arc<BoxOrder<N>>,
    ) -> Result<ExactSystem<N>, SolverError> {
        let mut exact_rows = Vec::with_capacity(selection.rows.len());
        let mut columns: Vec<Integral<N>> = Vec::new();
        for (seed, source) in &selection.rows {
            let row = self.row(seed, *source)?;
            columns.extend(row.iter().map(|(integral, _)| *integral));
            exact_rows.push(row);
        }
        columns.extend_from_slice(&selection.pivots);
        columns.extend_from_slice(&selection.masters);
        columns.extend_from_slice(&selection.demand);
        columns.sort_unstable_by(|left, right| order.compare(left, right));
        columns.dedup();
        // Every integral below was added to `columns` above.
        let index = |integral: &Integral<N>| -> u32 {
            columns
                .binary_search_by(|column| order.compare(column, integral))
                .expect("every row, pivot, master and demand integral is a column")
                as u32
        };
        let rows = exact_rows
            .into_iter()
            .map(|row| {
                row.into_iter()
                    .map(|(integral, coefficient)| (index(&integral), coefficient))
                    .collect()
            })
            .collect();
        let pivots = selection.pivots.iter().map(index).collect();
        let mut masters: Vec<u32> = selection.masters.iter().map(index).collect();
        masters.sort_unstable();
        Ok(ExactSystem {
            rows,
            pivots,
            masters,
            order,
            columns,
        })
    }
}
