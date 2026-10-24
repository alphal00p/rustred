//! Residual compression with RustRed sector-symmetry census rows.
//!
//! Candidate terminals are not masters: they are only the leaves of the saved
//! formulas. Exact census identities `I - T(I) = 0` (routes to the class
//! representative and representative automorphisms), reduced through the
//! same DAG, give linear relations among the terminals. Gaussian elimination
//! with the hardest column as pivot leaves the masters at the easy end of the
//! chosen column order. For a fixed row span and column order the pivot set
//! does not depend on row order.

use std::cmp::Ordering;
use std::collections::{BTreeMap, HashMap, HashSet, VecDeque};

use crate::algebra::{Coefficient, CoefficientContext, ExactAlgebraLimits};
use crate::family::IntegralKey;
use crate::sector::Mask;
use crate::sector::OrderingPolicy;
use crate::sector::symmetry::SectorSymmetryCensus;
use crate::sector::symmetry::integral_transport::{ExpansionLimits, Prepared};

use super::LiteRedError;
use super::zp::PrimeField;

/// Column order of the compression. The pivot is always the hardest column.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum ColumnOrder {
    /// `(lines, dots, numerator rank, powers lexicographic)`, larger is
    /// harder (the design prototype's `compress_transport.py` key).
    #[default]
    LinesDotsRank,
    /// The candidate program's own persisted integral order.
    Program,
}

/// Which census identities are used as compression rows.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum SymmetryRows {
    /// One route row per non-representative terminal and one row per
    /// representative automorphism generator.
    Generators,
    /// Rows `I - T_h(I)` for every element `h` of the representative group
    /// (composed with the route), obtained by an exact orbit closure.
    #[default]
    GroupOrbit,
}

/// Origin of one exact census row.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RowOrigin {
    Route,
    Automorphism(usize),
    Orbit,
    /// `P - R(P)` for a preferred non-terminal column.
    Preferred,
}

/// One exact homogeneous relation `sum c_k I_k = 0`.
#[derive(Clone, Debug)]
pub struct CensusRow {
    pub source: IntegralKey,
    pub origin: RowOrigin,
    pub terms: Vec<(IntegralKey, Coefficient)>,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct CensusRowStatistics {
    pub sources: usize,
    pub rows: usize,
    pub route_rows: usize,
    pub automorphism_rows: usize,
    pub orbit_rows: usize,
    pub transports: usize,
    pub max_orbit: usize,
    pub row_terms: usize,
}

type Expression = BTreeMap<IntegralKey, Coefficient>;

fn exact(error: crate::algebra::ExactAlgebraError) -> LiteRedError {
    LiteRedError::Algebra(error.to_string())
}

fn census_error(error: impl std::fmt::Display) -> LiteRedError {
    LiteRedError::Census(error.to_string())
}

fn is_zero_key(zero: &dyn Fn(&IntegralKey) -> bool, key: &IntegralKey) -> bool {
    zero(key)
}

fn apply(
    prepared: &Prepared,
    expression: &Expression,
    context: &CoefficientContext,
    limits: ExpansionLimits,
    zero: &dyn Fn(&IntegralKey) -> bool,
    statistics: &mut CensusRowStatistics,
) -> Result<Expression, LiteRedError> {
    let mut output = Expression::new();
    for (key, coefficient) in expression {
        let transported = prepared.transport(key, limits).map_err(census_error)?;
        statistics.transports += 1;
        for endpoint in transported.terms() {
            if is_zero_key(zero, endpoint.key()) {
                continue;
            }
            let value = context
                .try_mul(
                    coefficient,
                    endpoint.coefficient(),
                    ExactAlgebraLimits::default(),
                )
                .map_err(exact)?;
            accumulate(&mut output, endpoint.key(), value, context)?;
        }
    }
    Ok(output)
}

fn accumulate(
    expression: &mut Expression,
    key: &IntegralKey,
    value: Coefficient,
    context: &CoefficientContext,
) -> Result<(), LiteRedError> {
    match expression.get_mut(key) {
        Some(old) => {
            let sum = context
                .try_add(old, &value, ExactAlgebraLimits::default())
                .map_err(exact)?;
            if sum.is_zero() {
                expression.remove(key);
            } else {
                *old = sum;
            }
        }
        None => {
            if !value.is_zero() {
                expression.insert(key.clone(), value);
            }
        }
    }
    Ok(())
}

fn row_from(
    source: &IntegralKey,
    image: &Expression,
    origin: RowOrigin,
    context: &CoefficientContext,
) -> Result<Option<CensusRow>, LiteRedError> {
    let mut terms = Expression::new();
    terms.insert(source.clone(), context.one());
    for (key, coefficient) in image {
        let negative = context
            .try_neg(coefficient, ExactAlgebraLimits::default())
            .map_err(exact)?;
        accumulate(&mut terms, key, negative, context)?;
    }
    if terms.is_empty() {
        return Ok(None);
    }
    Ok(Some(CensusRow {
        source: source.clone(),
        origin,
        terms: terms.into_iter().collect(),
    }))
}

/// Exact census rows for every source integral (typically all terminals).
/// Endpoints in proved-zero sectors (`zero`) are dropped.
pub fn census_rows(
    census: &SectorSymmetryCensus,
    sources: &[IntegralKey],
    mode: SymmetryRows,
    zero: &dyn Fn(&IntegralKey) -> bool,
    max_orbit: usize,
) -> Result<(Vec<CensusRow>, CensusRowStatistics), LiteRedError> {
    let context = census.family().coefficient_context().clone();
    let limits = census.options().expansion;
    let mut statistics = CensusRowStatistics {
        sources: sources.len(),
        ..Default::default()
    };
    let mut rows = Vec::new();
    for source in sources {
        let sector = Mask::try_from_indices(source.powers()).map_err(census_error)?;
        let member = census.member(&sector).ok_or_else(|| {
            LiteRedError::Census(format!("source {source:?} lies outside the census"))
        })?;
        let class = census.class_of(&sector).expect("member has a class");
        match mode {
            SymmetryRows::Generators => {
                if member.is_representative() {
                    for relation in census
                        .automorphism_relations(source)
                        .map_err(census_error)?
                    {
                        rows.push(CensusRow {
                            source: source.clone(),
                            origin: RowOrigin::Automorphism(relation.automorphism().unwrap_or(0)),
                            terms: relation
                                .terms()
                                .iter()
                                .filter(|(key, _)| !zero(key))
                                .cloned()
                                .collect(),
                        });
                        statistics.automorphism_rows += 1;
                    }
                } else if let Some(relation) =
                    census.route_relation(source).map_err(census_error)?
                {
                    rows.push(CensusRow {
                        source: source.clone(),
                        origin: RowOrigin::Route,
                        terms: relation
                            .terms()
                            .iter()
                            .filter(|(key, _)| !zero(key))
                            .cloned()
                            .collect(),
                    });
                    statistics.route_rows += 1;
                }
            }
            SymmetryRows::GroupOrbit => {
                let start: Expression = if member.is_representative() {
                    Expression::from([(source.clone(), context.one())])
                } else {
                    let transported = census
                        .transport_to_representative(source)
                        .map_err(census_error)?;
                    statistics.transports += 1;
                    let mut expression = Expression::new();
                    for endpoint in transported.terms() {
                        if !zero(endpoint.key()) {
                            accumulate(
                                &mut expression,
                                endpoint.key(),
                                endpoint.coefficient().clone(),
                                &context,
                            )?;
                        }
                    }
                    expression
                };
                // `seen` makes the membership test O(1) per image; `orbit`
                // keeps the discovery order, so the rows are unchanged.
                let mut seen: HashSet<Expression> = HashSet::from([start.clone()]);
                let mut orbit: Vec<Expression> = vec![start];
                let mut queue = VecDeque::from([0usize]);
                while let Some(position) = queue.pop_front() {
                    for automorphism in class.automorphisms() {
                        let image = apply(
                            automorphism,
                            &orbit[position],
                            &context,
                            limits,
                            zero,
                            &mut statistics,
                        )?;
                        if !seen.contains(&image) {
                            if orbit.len() >= max_orbit {
                                return Err(LiteRedError::Limit {
                                    resource: "census orbit size",
                                    limit: max_orbit,
                                });
                            }
                            seen.insert(image.clone());
                            orbit.push(image);
                            queue.push_back(orbit.len() - 1);
                        }
                    }
                }
                statistics.max_orbit = statistics.max_orbit.max(orbit.len());
                for image in &orbit {
                    if let Some(row) = row_from(source, image, RowOrigin::Orbit, &context)? {
                        rows.push(row);
                        statistics.orbit_rows += 1;
                    }
                }
            }
        }
    }
    statistics.rows = rows.len();
    statistics.row_terms = rows.iter().map(|row| row.terms.len()).sum();
    Ok((rows, statistics))
}

/// Hardness comparison of two equal-arity keys under a column order.
pub fn compare_hardness(
    order: ColumnOrder,
    ordering: &OrderingPolicy,
    left: &[i64],
    right: &[i64],
) -> Result<Ordering, LiteRedError> {
    match order {
        ColumnOrder::LinesDotsRank => {
            let key = |powers: &[i64]| {
                (
                    powers.iter().filter(|&&n| n > 0).count(),
                    powers
                        .iter()
                        .filter(|&&n| n > 1)
                        .map(|&n| n - 1)
                        .sum::<i64>(),
                    powers.iter().filter(|&&n| n < 0).map(|&n| -n).sum::<i64>(),
                )
            };
            Ok(key(left).cmp(&key(right)).then_with(|| left.cmp(right)))
        }
        ColumnOrder::Program => ordering
            .compare(left, right)
            .map_err(|e| LiteRedError::InvalidInput(e.to_string())),
    }
}

/// Column positions: position 0 is the hardest column. Preferred columns are
/// forced to the easy end, in their own hardness order.
#[derive(Clone, Debug)]
pub struct ColumnSpace {
    pub columns: Vec<IntegralKey>,
    pub position: HashMap<IntegralKey, usize>,
    pub preferred: usize,
}

impl ColumnSpace {
    pub fn new(
        mut ordinary: Vec<IntegralKey>,
        mut preferred: Vec<IntegralKey>,
        order: ColumnOrder,
        ordering: &OrderingPolicy,
    ) -> Result<Self, LiteRedError> {
        ordinary.retain(|key| !preferred.contains(key));
        let mut error = None;
        let mut sort = |keys: &mut Vec<IntegralKey>| {
            keys.sort_by(
                |a, b| match compare_hardness(order, ordering, b.powers(), a.powers()) {
                    Ok(ordering) => ordering,
                    Err(e) => {
                        error.get_or_insert(e);
                        Ordering::Equal
                    }
                },
            );
            keys.dedup();
        };
        sort(&mut ordinary);
        sort(&mut preferred);
        if let Some(error) = error {
            return Err(error);
        }
        let preferred_count = preferred.len();
        let columns: Vec<IntegralKey> = ordinary.into_iter().chain(preferred).collect();
        let position = columns
            .iter()
            .enumerate()
            .map(|(position, key)| (key.clone(), position))
            .collect();
        Ok(Self {
            columns,
            position,
            preferred: preferred_count,
        })
    }

    pub fn len(&self) -> usize {
        self.columns.len()
    }
    pub fn is_empty(&self) -> bool {
        self.columns.is_empty()
    }
}

/// Echelon form of the relation rows over a [`ColumnSpace`] at one sample.
#[derive(Clone, Debug)]
pub struct Elimination {
    /// Pivot rows (sparse, entries right of the pivot), by pivot position.
    pivots: Vec<Option<Vec<(usize, u64)>>>,
    pub rows_added: usize,
    pub rank: usize,
}

impl Elimination {
    pub fn new(width: usize) -> Self {
        Self {
            pivots: vec![None; width],
            rows_added: 0,
            rank: 0,
        }
    }

    /// Insert one dense row (column positions); returns whether rank grew.
    pub fn add_row(&mut self, mut row: Vec<u64>, field: &PrimeField) -> bool {
        self.rows_added += 1;
        let width = self.pivots.len();
        for column in 0..width {
            let factor = row[column];
            if factor == 0 {
                continue;
            }
            match &self.pivots[column] {
                Some(pivot) => {
                    row[column] = 0;
                    let negative = field.neg(factor);
                    for &(position, value) in pivot {
                        row[position] = field.mul_add(row[position], negative, value);
                    }
                }
                None => {
                    let inverse = field.inv(factor).expect("nonzero pivot");
                    let normalized: Vec<(usize, u64)> = (column + 1..width)
                        .filter(|&position| row[position] != 0)
                        .map(|position| (position, field.mul(row[position], inverse)))
                        .collect();
                    self.pivots[column] = Some(normalized);
                    self.rank += 1;
                    return true;
                }
            }
        }
        false
    }

    pub fn pivot_columns(&self) -> Vec<usize> {
        (0..self.pivots.len())
            .filter(|&column| self.pivots[column].is_some())
            .collect()
    }

    /// Fully reduce: every column as a combination of the non-pivot
    /// (master) columns. Returns `(masters, map)`, where `map[column]` lists
    /// `(master ordinal, coefficient)`.
    pub fn solve(&self, field: &PrimeField) -> (Vec<usize>, Vec<Vec<(u32, u64)>>) {
        let width = self.pivots.len();
        let masters: Vec<usize> = (0..width)
            .filter(|&column| self.pivots[column].is_none())
            .collect();
        let master_ordinal: HashMap<usize, u32> = masters
            .iter()
            .enumerate()
            .map(|(ordinal, &column)| (column, ordinal as u32))
            .collect();
        let mut map: Vec<Vec<(u32, u64)>> = vec![Vec::new(); width];
        for &column in &masters {
            map[column] = vec![(master_ordinal[&column], 1)];
        }
        // Easiest pivot first: every substituted pivot is already reduced.
        let mut dense = vec![0u64; masters.len()];
        for column in (0..width).rev() {
            let Some(pivot) = &self.pivots[column] else {
                continue;
            };
            // column + sum pivot[j] col_j = 0  =>  column = -sum pivot[j] col_j
            for &(position, value) in pivot {
                let negative = field.neg(value);
                for &(master, coefficient) in &map[position] {
                    let slot = &mut dense[master as usize];
                    *slot = field.mul_add(*slot, negative, coefficient);
                }
            }
            map[column] = dense
                .iter_mut()
                .enumerate()
                .filter_map(|(master, slot)| {
                    let value = std::mem::take(slot);
                    (value != 0).then_some((master as u32, value))
                })
                .collect();
        }
        (masters, map)
    }
}
