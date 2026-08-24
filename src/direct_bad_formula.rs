//! Owner-independent routing for finite disjunctive bad-domain formulas.
//!
//! Formula owners retain their own atoms, clause provenance, polynomial
//! tables, implication logic, and resource accounting.  This module performs
//! only the allocation-free three-valued Boolean routing shared by global
//! sector coverage and target-relative affine `WhenBad` coverage.

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub(crate) enum DirectBadFormulaClause<A> {
    Atom(A),
    Conjunction(A, A),
}

impl<A> DirectBadFormulaClause<A> {
    pub(crate) const fn atom_count(&self) -> usize {
        match self {
            Self::Atom(_) => 1,
            Self::Conjunction(_, _) => 2,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum DirectBadFormulaTruth {
    False,
    True,
    Unknown,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum DirectBadFormulaRoute<A> {
    Bad { clause_ordinal: usize },
    Good,
    Split { clause_ordinal: usize, atom: A },
}

/// Route one direct formula without allocating or interpreting its atoms.
///
/// Owners must charge their formula-visit and atom-query budgets before this
/// call (a conservative complete-formula charge is sufficient).  Clause
/// ordinals are the exact ordinals of the supplied iterator, so an owner that
/// needs bad-domain provenance must preserve a one-to-one iteration order.
pub(crate) fn route_direct_bad_formula<A: Copy, E>(
    clauses: impl IntoIterator<Item = DirectBadFormulaClause<A>>,
    mut atom_truth: impl FnMut(A) -> Result<DirectBadFormulaTruth, E>,
) -> Result<DirectBadFormulaRoute<A>, E> {
    let mut first_unknown = None;
    for (clause_ordinal, clause) in clauses.into_iter().enumerate() {
        let (truth, unresolved) = match clause {
            DirectBadFormulaClause::Atom(atom) => {
                let truth = atom_truth(atom)?;
                (
                    truth,
                    (truth == DirectBadFormulaTruth::Unknown).then_some(atom),
                )
            }
            DirectBadFormulaClause::Conjunction(left, right) => {
                let left_truth = atom_truth(left)?;
                if left_truth == DirectBadFormulaTruth::False {
                    (DirectBadFormulaTruth::False, None)
                } else {
                    let right_truth = atom_truth(right)?;
                    match (left_truth, right_truth) {
                        (DirectBadFormulaTruth::True, DirectBadFormulaTruth::True) => {
                            (DirectBadFormulaTruth::True, None)
                        }
                        (_, DirectBadFormulaTruth::False) => (DirectBadFormulaTruth::False, None),
                        (DirectBadFormulaTruth::Unknown, _) => {
                            (DirectBadFormulaTruth::Unknown, Some(left))
                        }
                        (DirectBadFormulaTruth::True, DirectBadFormulaTruth::Unknown) => {
                            (DirectBadFormulaTruth::Unknown, Some(right))
                        }
                        _ => unreachable!("covered direct-formula truth combinations above"),
                    }
                }
            }
        };
        if truth == DirectBadFormulaTruth::True {
            // A later true disjunct dominates an earlier unresolved one. This
            // is the key distinction between the formula and one arbitrary
            // local decision-tree transcript.
            return Ok(DirectBadFormulaRoute::Bad { clause_ordinal });
        }
        if first_unknown.is_none() {
            first_unknown = unresolved.map(|atom| (clause_ordinal, atom));
        }
    }
    Ok(match first_unknown {
        Some((clause_ordinal, atom)) => DirectBadFormulaRoute::Split {
            clause_ordinal,
            atom,
        },
        None => DirectBadFormulaRoute::Good,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::convert::Infallible;

    fn truth_table_route(
        left: DirectBadFormulaTruth,
        right: DirectBadFormulaTruth,
    ) -> DirectBadFormulaRoute<u8> {
        route_direct_bad_formula(
            [DirectBadFormulaClause::Conjunction(0, 1)],
            |atom| -> Result<_, Infallible> { Ok(if atom == 0 { left } else { right }) },
        )
        .unwrap()
    }

    #[test]
    fn empty_formula_is_good() {
        let route = route_direct_bad_formula(
            std::iter::empty::<DirectBadFormulaClause<u8>>(),
            |_| -> Result<_, Infallible> { unreachable!("an empty formula has no atom queries") },
        )
        .unwrap();
        assert_eq!(route, DirectBadFormulaRoute::Good);
    }

    #[test]
    fn conjunction_has_the_complete_three_valued_truth_table() {
        use DirectBadFormulaRoute::{Bad, Good, Split};
        use DirectBadFormulaTruth::{False, True, Unknown};

        for (left, right, expected) in [
            (False, False, Good),
            (False, True, Good),
            (False, Unknown, Good),
            (True, False, Good),
            (True, True, Bad { clause_ordinal: 0 }),
            (
                True,
                Unknown,
                Split {
                    clause_ordinal: 0,
                    atom: 1,
                },
            ),
            (Unknown, False, Good),
            (
                Unknown,
                True,
                Split {
                    clause_ordinal: 0,
                    atom: 0,
                },
            ),
            (
                Unknown,
                Unknown,
                Split {
                    clause_ordinal: 0,
                    atom: 0,
                },
            ),
        ] {
            assert_eq!(truth_table_route(left, right), expected);
        }
    }

    #[test]
    fn false_left_conjunct_does_not_query_the_right_atom() {
        let mut queries = Vec::new();
        let route = route_direct_bad_formula(
            [DirectBadFormulaClause::Conjunction(0, 1)],
            |atom| -> Result<_, Infallible> {
                queries.push(atom);
                if atom == 0 {
                    Ok(DirectBadFormulaTruth::False)
                } else {
                    panic!("the right conjunct must be short-circuited")
                }
            },
        )
        .unwrap();
        assert_eq!(queries, [0]);
        assert_eq!(route, DirectBadFormulaRoute::Good);
    }

    #[test]
    fn later_true_clause_dominates_an_earlier_unknown_clause() {
        let route = route_direct_bad_formula(
            [
                DirectBadFormulaClause::Atom(3),
                DirectBadFormulaClause::Atom(7),
            ],
            |atom| -> Result<_, Infallible> {
                Ok(if atom == 3 {
                    DirectBadFormulaTruth::Unknown
                } else {
                    DirectBadFormulaTruth::True
                })
            },
        )
        .unwrap();
        assert_eq!(route, DirectBadFormulaRoute::Bad { clause_ordinal: 1 });
    }

    #[test]
    fn later_true_boundary_gate_clause_dominates_an_unknown_product_atom() {
        #[derive(Clone, Copy, Debug, PartialEq, Eq)]
        enum ProductionAtom {
            Product,
            Boundary,
            Gate,
        }

        let route = route_direct_bad_formula(
            [
                DirectBadFormulaClause::Atom(ProductionAtom::Product),
                DirectBadFormulaClause::Conjunction(ProductionAtom::Boundary, ProductionAtom::Gate),
            ],
            |atom| -> Result<_, Infallible> {
                Ok(match atom {
                    ProductionAtom::Product => DirectBadFormulaTruth::Unknown,
                    ProductionAtom::Boundary | ProductionAtom::Gate => DirectBadFormulaTruth::True,
                })
            },
        )
        .unwrap();
        assert_eq!(route, DirectBadFormulaRoute::Bad { clause_ordinal: 1 });
    }

    #[test]
    fn first_unresolved_atom_retains_clause_and_atom_provenance() {
        let route = route_direct_bad_formula(
            [
                DirectBadFormulaClause::Atom(11),
                DirectBadFormulaClause::Conjunction(13, 17),
            ],
            |_| -> Result<_, Infallible> { Ok(DirectBadFormulaTruth::Unknown) },
        )
        .unwrap();
        assert_eq!(
            route,
            DirectBadFormulaRoute::Split {
                clause_ordinal: 0,
                atom: 11,
            }
        );
    }
}
