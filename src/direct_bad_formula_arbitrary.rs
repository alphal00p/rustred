//! Owner-neutral routing for arbitrary-width direct bad-domain formulas.
//!
//! A compiled formula is a deterministic OR of AND clauses. Atoms live in one
//! flat, fixed-size arena and every clause is a checked range into that arena.
//! The compiler is deliberately algebra-free: an owner supplies copyable atom
//! locators and, while routing, supplies their three-valued truth. Empty
//! formulas are false; empty conjunctions are true.
//!
//! The two retained arrays use exact-capacity staging allocations before they
//! become boxed slices. Consequently the reported retained storage is
//! independent of `Vec` growth policy. The census covers inline atom locators,
//! not any referent behind a locator; such referents remain owner-owned.

use std::fmt;
use std::mem::size_of;
use std::ops::Range;

pub(crate) const ARBITRARY_DIRECT_BAD_FORMULA_V1_SCHEMA: &str =
    "rustred-arbitrary-direct-bad-formula-v1";

/// Complete incremental resource envelope for one compiled formula.
///
/// The source slices are caller-owned and excluded. The compilation peak
/// counts only newly owned logical storage. Since exact-capacity staging is
/// transferred directly into boxed slices, that peak equals retained storage.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct ArbitraryDirectBadFormulaLimits {
    pub(crate) max_atoms: usize,
    pub(crate) max_clauses: usize,
    pub(crate) max_atom_storage_bytes: usize,
    pub(crate) max_clause_storage_bytes: usize,
    pub(crate) max_retained_owned_logical_bytes: usize,
    pub(crate) max_compilation_owned_logical_peak_upper_bound: usize,
    pub(crate) max_route_clause_visits: usize,
    pub(crate) max_route_atom_queries: usize,
}

impl Default for ArbitraryDirectBadFormulaLimits {
    fn default() -> Self {
        Self {
            max_atoms: usize::MAX,
            max_clauses: usize::MAX,
            max_atom_storage_bytes: usize::MAX,
            max_clause_storage_bytes: usize::MAX,
            max_retained_owned_logical_bytes: usize::MAX,
            max_compilation_owned_logical_peak_upper_bound: usize::MAX,
            max_route_clause_visits: usize::MAX,
            max_route_atom_queries: usize::MAX,
        }
    }
}

/// Allocation-independent census of one compiled formula.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct ArbitraryDirectBadFormulaStats {
    atoms: usize,
    clauses: usize,
    atom_storage_bytes: usize,
    clause_storage_bytes: usize,
    retained_owned_logical_bytes: usize,
    compilation_owned_logical_peak_upper_bound: usize,
    route_clause_visit_bound: usize,
    route_atom_query_bound: usize,
}

impl ArbitraryDirectBadFormulaStats {
    pub(crate) const fn atoms(self) -> usize {
        self.atoms
    }

    pub(crate) const fn clauses(self) -> usize {
        self.clauses
    }

    pub(crate) const fn atom_storage_bytes(self) -> usize {
        self.atom_storage_bytes
    }

    pub(crate) const fn clause_storage_bytes(self) -> usize {
        self.clause_storage_bytes
    }

    pub(crate) const fn retained_owned_logical_bytes(self) -> usize {
        self.retained_owned_logical_bytes
    }

    pub(crate) const fn compilation_owned_logical_peak_upper_bound(self) -> usize {
        self.compilation_owned_logical_peak_upper_bound
    }

    pub(crate) const fn route_clause_visit_bound(self) -> usize {
        self.route_clause_visit_bound
    }

    pub(crate) const fn route_atom_query_bound(self) -> usize {
        self.route_atom_query_bound
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
struct StoredClauseRange {
    start: usize,
    end: usize,
}

trait ClauseBounds {
    fn bounds(&self) -> (usize, usize);
}

impl ClauseBounds for Range<usize> {
    fn bounds(&self) -> (usize, usize) {
        (self.start, self.end)
    }
}

impl ClauseBounds for StoredClauseRange {
    fn bounds(&self) -> (usize, usize) {
        (self.start, self.end)
    }
}

/// A fixed-storage, owner-neutral OR-of-AND formula.
///
/// This type is intentionally non-`Clone`: rebuilding is fallible and belongs
/// behind [`Self::replay`]. Atom values are never rendered by `Debug`.
pub(crate) struct ArbitraryDirectBadFormula<A> {
    schema: &'static str,
    atoms: Box<[A]>,
    clauses: Box<[StoredClauseRange]>,
    limits: ArbitraryDirectBadFormulaLimits,
    stats: ArbitraryDirectBadFormulaStats,
}

impl<A> ArbitraryDirectBadFormula<A> {
    pub(crate) const fn schema(&self) -> &'static str {
        self.schema
    }

    pub(crate) const fn limits(&self) -> ArbitraryDirectBadFormulaLimits {
        self.limits
    }

    pub(crate) const fn stats(&self) -> ArbitraryDirectBadFormulaStats {
        self.stats
    }

    pub(crate) fn atoms(&self) -> &[A] {
        &self.atoms
    }

    pub(crate) fn clause_count(&self) -> usize {
        self.clauses.len()
    }

    pub(crate) fn clause_range(&self, clause_ordinal: usize) -> Option<Range<usize>> {
        self.clauses
            .get(clause_ordinal)
            .map(|range| range.start..range.end)
    }

    /// Reauthenticate the complete immutable payload without allocating.
    pub(crate) fn validate_payload(&self) -> Result<(), ArbitraryDirectBadFormulaError> {
        if self.schema != ARBITRARY_DIRECT_BAD_FORMULA_V1_SCHEMA {
            return Err(ArbitraryDirectBadFormulaError::SchemaMismatch);
        }
        let expected = preflight::<A, _>(&self.atoms, &self.clauses, self.limits)?;
        if expected != self.stats {
            return Err(ArbitraryDirectBadFormulaError::PayloadMismatch);
        }
        Ok(())
    }
}

impl<A: Copy> ArbitraryDirectBadFormula<A> {
    /// Compile one canonical flattened formula.
    ///
    /// Clause ranges must form a contiguous partition of `atoms` in supplied
    /// order. Repeated atom locators are represented by repeated arena entries;
    /// this keeps provenance and query order one-to-one and deterministic.
    pub(crate) fn compile(
        atoms: &[A],
        clauses: &[Range<usize>],
        limits: ArbitraryDirectBadFormulaLimits,
    ) -> Result<Self, ArbitraryDirectBadFormulaError> {
        compile_from_ranges(atoms, clauses, limits)
    }

    /// Route the formula without allocating or interpreting atom locators.
    ///
    /// A false atom short-circuits its conjunction. Unknown atoms do not: a
    /// later false atom can still make that whole clause false. The first
    /// unknown in the first still-unknown clause is retained, but any later
    /// true clause dominates it and immediately routes to `Bad`.
    pub(crate) fn route<E>(
        &self,
        mut atom_truth: impl FnMut(A) -> Result<ArbitraryDirectBadFormulaTruth, E>,
    ) -> Result<ArbitraryDirectBadFormulaRoute<A>, E> {
        let mut first_unresolved = None;
        for (clause_ordinal, clause) in self.clauses.iter().copied().enumerate() {
            let mut clause_is_false = false;
            let mut clause_first_unknown = None;
            for atom_ordinal in clause.start..clause.end {
                let atom = self.atoms[atom_ordinal];
                match atom_truth(atom)? {
                    ArbitraryDirectBadFormulaTruth::False => {
                        clause_is_false = true;
                        break;
                    }
                    ArbitraryDirectBadFormulaTruth::True => {}
                    ArbitraryDirectBadFormulaTruth::Unknown => {
                        if clause_first_unknown.is_none() {
                            clause_first_unknown =
                                Some((atom_ordinal - clause.start, atom_ordinal, atom));
                        }
                    }
                }
            }

            if clause_is_false {
                continue;
            }
            if let Some((clause_atom_ordinal, atom_ordinal, atom)) = clause_first_unknown {
                if first_unresolved.is_none() {
                    first_unresolved =
                        Some((clause_ordinal, clause_atom_ordinal, atom_ordinal, atom));
                }
            } else {
                // This includes the empty conjunction, whose identity is true.
                return Ok(ArbitraryDirectBadFormulaRoute::Bad { clause_ordinal });
            }
        }

        Ok(match first_unresolved {
            Some((clause_ordinal, clause_atom_ordinal, atom_ordinal, atom)) => {
                ArbitraryDirectBadFormulaRoute::Split {
                    clause_ordinal,
                    clause_atom_ordinal,
                    atom_ordinal,
                    atom,
                }
            }
            None => ArbitraryDirectBadFormulaRoute::Good,
        })
    }
}

impl<A: Copy + Eq> ArbitraryDirectBadFormula<A> {
    pub(crate) fn payload_eq(&self, other: &Self) -> bool {
        self.schema == other.schema
            && self.atoms == other.atoms
            && self.clauses == other.clauses
            && self.limits == other.limits
            && self.stats == other.stats
    }

    /// Rebuild through the same checked, fallible path and compare the full
    /// transcript. The caller-owned source formula is excluded from the new
    /// attempt's incremental resource envelope.
    pub(crate) fn replay(&self) -> Result<(), ArbitraryDirectBadFormulaError> {
        self.validate_payload()?;
        let rebuilt = compile_from_ranges(&self.atoms, &self.clauses, self.limits)?;
        if self.payload_eq(&rebuilt) {
            Ok(())
        } else {
            Err(ArbitraryDirectBadFormulaError::ReplayMismatch)
        }
    }
}

impl<A> fmt::Debug for ArbitraryDirectBadFormula<A> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ArbitraryDirectBadFormula")
            .field("schema", &self.schema)
            .field("limits", &self.limits)
            .field("stats", &self.stats)
            .field("private_atom_arena", &"<redacted>")
            .field("private_clause_ranges", &"<redacted>")
            .finish()
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ArbitraryDirectBadFormulaTruth {
    False,
    True,
    Unknown,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum ArbitraryDirectBadFormulaRoute<A> {
    Bad {
        clause_ordinal: usize,
    },
    Good,
    Split {
        clause_ordinal: usize,
        clause_atom_ordinal: usize,
        atom_ordinal: usize,
        atom: A,
    },
}

impl<A> ArbitraryDirectBadFormulaRoute<A> {
    pub(crate) const fn clause_ordinal(&self) -> Option<usize> {
        match self {
            Self::Bad { clause_ordinal } | Self::Split { clause_ordinal, .. } => {
                Some(*clause_ordinal)
            }
            Self::Good => None,
        }
    }

    pub(crate) const fn split_provenance(&self) -> Option<(usize, usize, usize, &A)> {
        match self {
            Self::Split {
                clause_ordinal,
                clause_atom_ordinal,
                atom_ordinal,
                atom,
            } => Some((*clause_ordinal, *clause_atom_ordinal, *atom_ordinal, atom)),
            Self::Bad { .. } | Self::Good => None,
        }
    }
}

impl<A> fmt::Debug for ArbitraryDirectBadFormulaRoute<A> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Bad { clause_ordinal } => formatter
                .debug_struct("Bad")
                .field("clause_ordinal", clause_ordinal)
                .finish(),
            Self::Good => formatter.write_str("Good"),
            Self::Split {
                clause_ordinal,
                clause_atom_ordinal,
                atom_ordinal,
                ..
            } => formatter
                .debug_struct("Split")
                .field("clause_ordinal", clause_ordinal)
                .field("clause_atom_ordinal", clause_atom_ordinal)
                .field("atom_ordinal", atom_ordinal)
                .field("private_atom", &"<redacted>")
                .finish(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum ArbitraryDirectBadFormulaError {
    MalformedClauseRange {
        clause_ordinal: usize,
        start: usize,
        end: usize,
        expected_start: usize,
        atom_count: usize,
    },
    UncoveredAtomTail {
        first_uncovered: usize,
        atom_count: usize,
    },
    ResourceLimit {
        resource: &'static str,
        requested: usize,
        limit: usize,
    },
    ResourceCountOverflow {
        resource: &'static str,
    },
    AllocationFailure {
        resource: &'static str,
        requested: usize,
    },
    NonExactAllocation {
        resource: &'static str,
        requested: usize,
        actual: usize,
    },
    SchemaMismatch,
    PayloadMismatch,
    ReplayMismatch,
}

impl fmt::Display for ArbitraryDirectBadFormulaError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::MalformedClauseRange { .. } => {
                "arbitrary direct bad-formula clause ranges are malformed"
            }
            Self::UncoveredAtomTail { .. } => {
                "arbitrary direct bad-formula clause ranges do not cover the atom arena"
            }
            Self::ResourceLimit { .. } => "arbitrary direct bad-formula resource limit exceeded",
            Self::ResourceCountOverflow { .. } => {
                "arbitrary direct bad-formula resource count overflow"
            }
            Self::AllocationFailure { .. } => {
                "arbitrary direct bad-formula bounded allocation failed"
            }
            Self::NonExactAllocation { .. } => {
                "arbitrary direct bad-formula allocation retained excess capacity"
            }
            Self::SchemaMismatch => "arbitrary direct bad-formula schema mismatch",
            Self::PayloadMismatch => "arbitrary direct bad-formula payload mismatch",
            Self::ReplayMismatch => "arbitrary direct bad-formula replay mismatch",
        })
    }
}

impl std::error::Error for ArbitraryDirectBadFormulaError {}

fn compile_from_ranges<A: Copy, R: ClauseBounds>(
    atoms: &[A],
    clauses: &[R],
    limits: ArbitraryDirectBadFormulaLimits,
) -> Result<ArbitraryDirectBadFormula<A>, ArbitraryDirectBadFormulaError> {
    let stats = preflight::<A, _>(atoms, clauses, limits)?;

    let atom_arena = exact_boxed_copy("arbitrary direct bad-formula atom arena", atoms)?;
    let mut stored_clauses = try_exact_vec::<StoredClauseRange>(
        "arbitrary direct bad-formula clause ranges",
        clauses.len(),
    )?;
    for clause in clauses {
        let (start, end) = clause.bounds();
        stored_clauses.push(StoredClauseRange { start, end });
    }
    if stored_clauses.len() != clauses.len() {
        return Err(ArbitraryDirectBadFormulaError::PayloadMismatch);
    }
    let clause_ranges = stored_clauses.into_boxed_slice();

    let compiled = ArbitraryDirectBadFormula {
        schema: ARBITRARY_DIRECT_BAD_FORMULA_V1_SCHEMA,
        atoms: atom_arena,
        clauses: clause_ranges,
        limits,
        stats,
    };
    compiled.validate_payload()?;
    Ok(compiled)
}

fn preflight<A, R: ClauseBounds>(
    atoms: &[A],
    clauses: &[R],
    limits: ArbitraryDirectBadFormulaLimits,
) -> Result<ArbitraryDirectBadFormulaStats, ArbitraryDirectBadFormulaError> {
    check_limit(
        "arbitrary direct bad-formula atoms",
        atoms.len(),
        limits.max_atoms,
    )?;
    check_limit(
        "arbitrary direct bad-formula clauses",
        clauses.len(),
        limits.max_clauses,
    )?;
    let atom_storage_bytes = checked_mul(
        "arbitrary direct bad-formula atom storage bytes",
        atoms.len(),
        size_of::<A>(),
    )?;
    check_limit(
        "arbitrary direct bad-formula atom storage bytes",
        atom_storage_bytes,
        limits.max_atom_storage_bytes,
    )?;
    let clause_storage_bytes = checked_mul(
        "arbitrary direct bad-formula clause storage bytes",
        clauses.len(),
        size_of::<StoredClauseRange>(),
    )?;
    check_limit(
        "arbitrary direct bad-formula clause storage bytes",
        clause_storage_bytes,
        limits.max_clause_storage_bytes,
    )?;

    validate_clause_ranges(atoms.len(), clauses)?;

    // Every nonempty clause before the first empty clause can be forced to
    // inspect its complete range by making its final atom false. An empty
    // conjunction is unconditionally true and terminates the OR. If there is
    // no empty clause, assigning an unknown atom in every clause reaches the
    // complete formula. These are therefore exact, attainable route bounds.
    let mut route_clause_visit_bound = 0usize;
    let mut route_atom_query_bound = 0usize;
    for clause in clauses {
        let (start, end) = clause.bounds();
        route_clause_visit_bound = checked_add(
            "arbitrary direct bad-formula route clause visits",
            route_clause_visit_bound,
            1,
        )?;
        if start == end {
            break;
        }
        route_atom_query_bound = checked_add(
            "arbitrary direct bad-formula route atom queries",
            route_atom_query_bound,
            end - start,
        )?;
    }
    check_limit(
        "arbitrary direct bad-formula route clause visits",
        route_clause_visit_bound,
        limits.max_route_clause_visits,
    )?;
    check_limit(
        "arbitrary direct bad-formula route atom queries",
        route_atom_query_bound,
        limits.max_route_atom_queries,
    )?;

    let retained_owned_logical_bytes = checked_add(
        "arbitrary direct bad-formula retained owned logical bytes",
        size_of::<ArbitraryDirectBadFormula<A>>(),
        checked_add(
            "arbitrary direct bad-formula retained owned logical bytes",
            atom_storage_bytes,
            clause_storage_bytes,
        )?,
    )?;
    check_limit(
        "arbitrary direct bad-formula retained owned logical bytes",
        retained_owned_logical_bytes,
        limits.max_retained_owned_logical_bytes,
    )?;

    // Exact-capacity staging allocations become the retained boxes without a
    // second backing allocation, so no newly owned heap payload exceeds the
    // final retained amount. Stack-local Vec headers are not owned payload.
    let compilation_owned_logical_peak_upper_bound = retained_owned_logical_bytes;
    check_limit(
        "arbitrary direct bad-formula compilation owned logical peak upper bound",
        compilation_owned_logical_peak_upper_bound,
        limits.max_compilation_owned_logical_peak_upper_bound,
    )?;

    Ok(ArbitraryDirectBadFormulaStats {
        atoms: atoms.len(),
        clauses: clauses.len(),
        atom_storage_bytes,
        clause_storage_bytes,
        retained_owned_logical_bytes,
        compilation_owned_logical_peak_upper_bound,
        route_clause_visit_bound,
        route_atom_query_bound,
    })
}

fn validate_clause_ranges<R: ClauseBounds>(
    atom_count: usize,
    clauses: &[R],
) -> Result<(), ArbitraryDirectBadFormulaError> {
    let mut expected_start = 0usize;
    for (clause_ordinal, clause) in clauses.iter().enumerate() {
        let (start, end) = clause.bounds();
        if start > end || start != expected_start || end > atom_count {
            return Err(ArbitraryDirectBadFormulaError::MalformedClauseRange {
                clause_ordinal,
                start,
                end,
                expected_start,
                atom_count,
            });
        }
        expected_start = end;
    }
    if expected_start != atom_count {
        return Err(ArbitraryDirectBadFormulaError::UncoveredAtomTail {
            first_uncovered: expected_start,
            atom_count,
        });
    }
    Ok(())
}

fn exact_boxed_copy<T: Copy>(
    resource: &'static str,
    source: &[T],
) -> Result<Box<[T]>, ArbitraryDirectBadFormulaError> {
    let mut values = try_exact_vec(resource, source.len())?;
    values.extend_from_slice(source);
    if values.len() != source.len() {
        return Err(ArbitraryDirectBadFormulaError::PayloadMismatch);
    }
    Ok(values.into_boxed_slice())
}

fn try_exact_vec<T>(
    resource: &'static str,
    requested: usize,
) -> Result<Vec<T>, ArbitraryDirectBadFormulaError> {
    let mut values = Vec::new();
    values.try_reserve_exact(requested).map_err(|_| {
        ArbitraryDirectBadFormulaError::AllocationFailure {
            resource,
            requested,
        }
    })?;
    // A zero-sized locator has no backing allocation; Vec deliberately
    // reports usize::MAX capacity for it, while its exact storage census is 0.
    if size_of::<T>() != 0 && values.capacity() != requested {
        return Err(ArbitraryDirectBadFormulaError::NonExactAllocation {
            resource,
            requested,
            actual: values.capacity(),
        });
    }
    Ok(values)
}

fn checked_add(
    resource: &'static str,
    left: usize,
    right: usize,
) -> Result<usize, ArbitraryDirectBadFormulaError> {
    left.checked_add(right)
        .ok_or(ArbitraryDirectBadFormulaError::ResourceCountOverflow { resource })
}

fn checked_mul(
    resource: &'static str,
    left: usize,
    right: usize,
) -> Result<usize, ArbitraryDirectBadFormulaError> {
    left.checked_mul(right)
        .ok_or(ArbitraryDirectBadFormulaError::ResourceCountOverflow { resource })
}

fn check_limit(
    resource: &'static str,
    requested: usize,
    limit: usize,
) -> Result<(), ArbitraryDirectBadFormulaError> {
    if requested > limit {
        Err(ArbitraryDirectBadFormulaError::ResourceLimit {
            resource,
            requested,
            limit,
        })
    } else {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::convert::Infallible;

    fn compile<A: Copy>(atoms: &[A], clauses: &[Range<usize>]) -> ArbitraryDirectBadFormula<A> {
        ArbitraryDirectBadFormula::compile(
            atoms,
            clauses,
            ArbitraryDirectBadFormulaLimits::default(),
        )
        .unwrap()
    }

    #[test]
    fn width_zero_empty_conjunction_is_true_and_empty_formula_is_false() {
        let empty_formula = compile::<u8>(&[], &[]);
        let mut queries = 0usize;
        assert_eq!(
            empty_formula
                .route(|_| -> Result<_, Infallible> {
                    queries += 1;
                    unreachable!("an empty formula has no atom queries")
                })
                .unwrap(),
            ArbitraryDirectBadFormulaRoute::Good,
        );
        assert_eq!(queries, 0);

        let empty_conjunction = compile::<u8>(&[], &[0..0]);
        assert_eq!(
            empty_conjunction
                .route(|_| -> Result<_, Infallible> {
                    unreachable!("an empty conjunction has no atom queries")
                })
                .unwrap(),
            ArbitraryDirectBadFormulaRoute::Bad { clause_ordinal: 0 },
        );
    }

    #[test]
    fn widths_one_two_and_three_have_complete_ordered_semantics() {
        use ArbitraryDirectBadFormulaRoute::{Bad, Split};
        use ArbitraryDirectBadFormulaTruth::{True, Unknown};

        let width_one = compile(&[10u8], &[0..1]);
        assert_eq!(
            width_one
                .route(|_| -> Result<_, Infallible> { Ok(True) })
                .unwrap(),
            Bad { clause_ordinal: 0 },
        );

        let width_two = compile(&[10u8, 11], &[0..2]);
        assert_eq!(
            width_two
                .route(|atom| -> Result<_, Infallible> {
                    Ok(if atom == 10 { True } else { Unknown })
                })
                .unwrap(),
            Split {
                clause_ordinal: 0,
                clause_atom_ordinal: 1,
                atom_ordinal: 1,
                atom: 11,
            },
        );

        let width_three = compile(&[20u8, 21, 22], &[0..3]);
        assert_eq!(
            width_three
                .route(|_| -> Result<_, Infallible> { Ok(True) })
                .unwrap(),
            Bad { clause_ordinal: 0 },
        );
    }

    #[test]
    fn large_conjunction_routes_without_router_allocation() {
        const WIDTH: usize = 16_384;
        let atoms: Vec<usize> = (0..WIDTH).collect();
        let formula = compile(&atoms, &[0..WIDTH]);
        let mut queries = 0usize;
        let route = formula
            .route(|atom| -> Result<_, Infallible> {
                assert_eq!(atom, queries);
                queries += 1;
                Ok(ArbitraryDirectBadFormulaTruth::True)
            })
            .unwrap();
        assert_eq!(queries, WIDTH);
        assert_eq!(
            route,
            ArbitraryDirectBadFormulaRoute::Bad { clause_ordinal: 0 }
        );
    }

    #[test]
    fn false_atom_short_circuits_the_rest_of_its_clause() {
        let formula = compile(&[0u8, 1, 2], &[0..3]);
        let mut queries = Vec::new();
        let route = formula
            .route(|atom| -> Result<_, Infallible> {
                queries.push(atom);
                match atom {
                    0 => Ok(ArbitraryDirectBadFormulaTruth::True),
                    1 => Ok(ArbitraryDirectBadFormulaTruth::False),
                    _ => panic!("an atom after false must not be queried"),
                }
            })
            .unwrap();
        assert_eq!(queries, [0, 1]);
        assert_eq!(route, ArbitraryDirectBadFormulaRoute::Good);
    }

    #[test]
    fn first_unresolved_provenance_is_stable_within_and_across_clauses() {
        let formula = compile(&[10u8, 11, 12, 20, 21], &[0..3, 3..5]);
        let route = formula
            .route(|atom| -> Result<_, Infallible> {
                Ok(match atom {
                    10 | 12 | 20 => ArbitraryDirectBadFormulaTruth::Unknown,
                    _ => ArbitraryDirectBadFormulaTruth::True,
                })
            })
            .unwrap();
        assert_eq!(
            route,
            ArbitraryDirectBadFormulaRoute::Split {
                clause_ordinal: 0,
                clause_atom_ordinal: 0,
                atom_ordinal: 0,
                atom: 10,
            }
        );
        assert_eq!(route.split_provenance(), Some((0, 0, 0, &10)));
    }

    #[test]
    fn unknown_before_false_does_not_survive_that_false_clause() {
        let formula = compile(&[10u8, 11, 12], &[0..2, 2..3]);
        let route = formula
            .route(|atom| -> Result<_, Infallible> {
                Ok(match atom {
                    10 | 12 => ArbitraryDirectBadFormulaTruth::Unknown,
                    11 => ArbitraryDirectBadFormulaTruth::False,
                    _ => unreachable!(),
                })
            })
            .unwrap();
        assert_eq!(
            route,
            ArbitraryDirectBadFormulaRoute::Split {
                clause_ordinal: 1,
                clause_atom_ordinal: 0,
                atom_ordinal: 2,
                atom: 12,
            }
        );
    }

    #[test]
    fn later_true_clause_dominates_an_earlier_unknown_clause() {
        let formula = compile(&[3u8, 7, 8, 9], &[0..1, 1..4]);
        let route = formula
            .route(|atom| -> Result<_, Infallible> {
                Ok(if atom == 3 {
                    ArbitraryDirectBadFormulaTruth::Unknown
                } else {
                    ArbitraryDirectBadFormulaTruth::True
                })
            })
            .unwrap();
        assert_eq!(
            route,
            ArbitraryDirectBadFormulaRoute::Bad { clause_ordinal: 1 }
        );
        assert_eq!(route.clause_ordinal(), Some(1));
    }

    #[test]
    fn malformed_ranges_fail_closed_before_allocation() {
        let atoms = [0u8, 1, 2];
        for ranges in [vec![0..2, 1..3], vec![0..1, 2..3], vec![0..4], vec![2..1]] {
            assert!(matches!(
                ArbitraryDirectBadFormula::compile(
                    &atoms,
                    &ranges,
                    ArbitraryDirectBadFormulaLimits::default(),
                ),
                Err(ArbitraryDirectBadFormulaError::MalformedClauseRange { .. })
            ));
        }
        assert!(matches!(
            ArbitraryDirectBadFormula::compile(
                &atoms,
                &[0..2],
                ArbitraryDirectBadFormulaLimits::default(),
            ),
            Err(ArbitraryDirectBadFormulaError::UncoveredAtomTail { .. })
        ));
        assert!(matches!(
            ArbitraryDirectBadFormula::compile(
                &atoms,
                &[],
                ArbitraryDirectBadFormulaLimits::default(),
            ),
            Err(ArbitraryDirectBadFormulaError::UncoveredAtomTail { .. })
        ));
    }

    #[test]
    fn every_positive_resource_limit_is_exact_and_one_below() {
        let atoms = [10u64, 11, 12, 13];
        let clauses = [0..1, 1..2, 2..4];
        let baseline = compile(&atoms, &clauses);
        let stats = baseline.stats();
        assert_eq!(stats.atoms(), atoms.len());
        assert_eq!(stats.clauses(), clauses.len());
        assert_eq!(stats.atom_storage_bytes(), atoms.len() * size_of::<u64>());
        assert_eq!(
            stats.clause_storage_bytes(),
            clauses.len() * size_of::<StoredClauseRange>()
        );
        assert_eq!(stats.route_clause_visit_bound(), clauses.len());
        assert_eq!(stats.route_atom_query_bound(), atoms.len());
        assert_eq!(
            stats.compilation_owned_logical_peak_upper_bound(),
            stats.retained_owned_logical_bytes()
        );

        let exact = ArbitraryDirectBadFormulaLimits {
            max_atoms: stats.atoms(),
            max_clauses: stats.clauses(),
            max_atom_storage_bytes: stats.atom_storage_bytes(),
            max_clause_storage_bytes: stats.clause_storage_bytes(),
            max_retained_owned_logical_bytes: stats.retained_owned_logical_bytes(),
            max_compilation_owned_logical_peak_upper_bound: stats
                .compilation_owned_logical_peak_upper_bound(),
            max_route_clause_visits: stats.route_clause_visit_bound(),
            max_route_atom_queries: stats.route_atom_query_bound(),
        };
        ArbitraryDirectBadFormula::compile(&atoms, &clauses, exact).unwrap();

        let mut one_below = Vec::new();
        macro_rules! lower {
            ($field:ident, $value:expr) => {
                if $value > 0 {
                    let mut limits = exact;
                    limits.$field = $value - 1;
                    one_below.push(limits);
                }
            };
        }
        lower!(max_atoms, stats.atoms());
        lower!(max_clauses, stats.clauses());
        lower!(max_atom_storage_bytes, stats.atom_storage_bytes());
        lower!(max_clause_storage_bytes, stats.clause_storage_bytes());
        lower!(
            max_retained_owned_logical_bytes,
            stats.retained_owned_logical_bytes()
        );
        lower!(
            max_compilation_owned_logical_peak_upper_bound,
            stats.compilation_owned_logical_peak_upper_bound()
        );
        lower!(max_route_clause_visits, stats.route_clause_visit_bound());
        lower!(max_route_atom_queries, stats.route_atom_query_bound());
        assert_eq!(one_below.len(), 8);
        for limits in one_below {
            assert!(matches!(
                ArbitraryDirectBadFormula::compile(&atoms, &clauses, limits),
                Err(ArbitraryDirectBadFormulaError::ResourceLimit { .. })
            ));
        }
    }

    #[test]
    fn deterministic_replay_validates_the_complete_payload_and_debug_is_redacted() {
        let atoms = ["secret-alpha", "secret-beta", "secret-gamma"];
        let clauses = [0..1, 1..3];
        let formula = compile(&atoms, &clauses);
        formula.validate_payload().unwrap();
        formula.replay().unwrap();
        let rebuilt = compile(&atoms, &clauses);
        assert!(formula.payload_eq(&rebuilt));
        assert_eq!(formula.schema(), ARBITRARY_DIRECT_BAD_FORMULA_V1_SCHEMA);
        assert_eq!(formula.atoms(), atoms);
        assert_eq!(formula.clause_count(), clauses.len());
        assert_eq!(formula.clause_range(1), Some(1..3));

        let rendered = format!("{formula:?}");
        assert!(rendered.contains("<redacted>"));
        assert!(!rendered.contains("secret-alpha"));

        let route = formula
            .route(|_| -> Result<_, Infallible> { Ok(ArbitraryDirectBadFormulaTruth::Unknown) })
            .unwrap();
        let rendered_route = format!("{route:?}");
        assert!(rendered_route.contains("<redacted>"));
        assert!(!rendered_route.contains("secret-alpha"));
    }

    #[test]
    fn payload_validation_rejects_range_and_stats_corruption() {
        let mut malformed_range = compile(&[0u8, 1], &[0..2]);
        malformed_range.clauses[0].end = 3;
        assert!(matches!(
            malformed_range.validate_payload(),
            Err(ArbitraryDirectBadFormulaError::MalformedClauseRange { .. })
        ));

        let mut malformed_stats = compile(&[0u8], &[0..1]);
        malformed_stats.stats.atoms = 0;
        assert_eq!(
            malformed_stats.validate_payload(),
            Err(ArbitraryDirectBadFormulaError::PayloadMismatch)
        );
    }

    #[test]
    fn zero_sized_owner_neutral_atoms_have_zero_exact_storage() {
        let formula = compile(&[(), (), ()], &[0..3]);
        assert_eq!(formula.stats().atom_storage_bytes(), 0);
        formula.replay().unwrap();
    }
}
