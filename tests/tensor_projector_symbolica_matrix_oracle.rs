//! Independent Symbolica-matrix oracle for the generic vacuum projector.
//!
//! Production constructs the projector for an arbitrary even tensor rank.  This
//! test deliberately does not call its contraction-cycle or matrix-inversion
//! helpers.  It rebuilds the Gram entries with an independent graph traversal,
//! delegates powers, determinants, solves, and products to public Symbolica
//! APIs, and compares the complete retained inverse.  The rank-six closed
//! formula is frozen from Vakint's `tensorreduce.frm`; it is oracle data only
//! and is never used by RustRed production code.

use std::collections::{BTreeMap, BTreeSet};

use rustred::{
    AffineDenominator, Coefficient, CoefficientContext, GenericVacuumTensorProjector,
    IndexedVector, IntegralFamily, LoopVector, LorentzIndex, Metric, MetricPairing,
    ScalarProductCoordinate, SlotPairing, TensorMonomial, VacuumTensorProjector,
};
use symbolica::{
    domains::rational_polynomial::RationalPolynomialField,
    prelude::{IntegerRing, Matrix, Ring, Z},
};

type OracleField = RationalPolynomialField<IntegerRing, u16>;
type OracleMatrix = symbolica::prelude::Matrix<OracleField>;

fn family() -> IntegralFamily {
    let context = CoefficientContext::new(["d", "m2"]);
    IntegralFamily::new(
        "tensor-projector-symbolica-matrix-oracle",
        vec!["k".to_owned()],
        Vec::new(),
        context.clone(),
        context.parameter("d").unwrap(),
        vec![AffineDenominator::new(
            context.parse("-m2").unwrap(),
            vec![context.one()],
        )],
        Vec::new(),
        vec![context.zero()],
    )
    .unwrap()
}

fn source(rank: usize) -> TensorMonomial {
    TensorMonomial::try_new((0..rank).map(|slot| {
        IndexedVector::new(
            LoopVector::new(0),
            LorentzIndex::new(u32::try_from(100 + slot).unwrap()),
        )
    }))
    .unwrap()
}

fn complete_coordinate_family(loops: usize) -> IntegralFamily {
    let context = CoefficientContext::new(["d"]);
    let coordinate_count = loops * (loops + 1) / 2;
    let denominators = (0..coordinate_count)
        .map(|selected| {
            let mut coefficients = vec![context.zero(); coordinate_count];
            coefficients[selected] = context.one();
            AffineDenominator::new(context.zero(), coefficients)
        })
        .collect();
    IntegralFamily::new(
        format!("tensor-projector-{loops}-loop-coordinate-compatibility"),
        (0..loops).map(|loop_id| format!("k{loop_id}")).collect(),
        Vec::new(),
        context.clone(),
        context.parameter("d").unwrap(),
        denominators,
        Vec::new(),
        vec![context.zero(); coordinate_count],
    )
    .unwrap()
}

fn distinct_source(rank: usize) -> TensorMonomial {
    TensorMonomial::try_new((0..rank).map(|slot| {
        IndexedVector::new(
            LoopVector::new(u16::try_from(slot).unwrap()),
            LorentzIndex::new(u32::try_from(1_000 + slot).unwrap()),
        )
    }))
    .unwrap()
}

fn oracle_matrix(data: Vec<Coefficient>, rows: usize, columns: usize, label: &str) -> OracleMatrix {
    let rows = u32::try_from(rows).expect("small oracle row count fits in u32");
    let columns = u32::try_from(columns).expect("small oracle column count fits in u32");
    Matrix::from_linear(data, rows, columns, RationalPolynomialField::new(Z))
        .unwrap_or_else(|error| panic!("could not construct {label}: {error}"))
}

fn assert_coefficient_eq(
    context: &CoefficientContext,
    actual: &Coefficient,
    expected: &Coefficient,
    label: &str,
) {
    assert!(context.contains(actual), "{label} left the declared field");
    assert!(
        (actual - expected).is_zero(),
        "{label}: expected {}, found {}",
        expected.to_expression(),
        actual.to_expression(),
    );
}

/// Independently count connected components in the two-matching multigraph.
///
/// Production uses a disjoint-set implementation.  This oracle instead builds
/// adjacency lists and performs a depth-first graph traversal.  A component is
/// one closed Lorentz-index cycle.
fn independent_contraction_cycles(left: &SlotPairing, right: &SlotPairing) -> usize {
    assert_eq!(left.rank(), right.rank());
    let rank = left.rank();
    if rank == 0 {
        return 0;
    }

    let mut adjacency = vec![Vec::with_capacity(2); rank];
    for &(first, second) in left.pairs().iter().chain(right.pairs()) {
        adjacency[first].push(second);
        adjacency[second].push(first);
    }
    assert!(
        adjacency.iter().all(|neighbours| neighbours.len() == 2),
        "the union of two perfect matchings must be two-regular",
    );

    let mut visited = vec![false; rank];
    let mut components = 0;
    for start in 0..rank {
        if visited[start] {
            continue;
        }
        components += 1;
        let mut pending = vec![start];
        visited[start] = true;
        while let Some(slot) = pending.pop() {
            for &neighbour in &adjacency[slot] {
                if !visited[neighbour] {
                    visited[neighbour] = true;
                    pending.push(neighbour);
                }
            }
        }
    }
    components
}

fn independent_gram(family: &IntegralFamily, pairings: &[SlotPairing]) -> OracleMatrix {
    let field = RationalPolynomialField::new(Z);
    let mut entries = Vec::with_capacity(pairings.len() * pairings.len());
    for left in pairings {
        for right in pairings {
            let cycles = independent_contraction_cycles(left, right);
            entries.push(field.pow(family.dimension(), u64::try_from(cycles).unwrap()));
        }
    }
    oracle_matrix(
        entries,
        pairings.len(),
        pairings.len(),
        "oracle Gram matrix",
    )
}

/// Reconstruct every inverse column through public `Matrix::solve(G,e_j)`.
/// This intentionally follows a different Symbolica call schedule from the
/// production inverse and never reads a production entry while solving.
fn solve_inverse(context: &CoefficientContext, gram: &OracleMatrix) -> Vec<Vec<Coefficient>> {
    let size = gram.nrows();
    assert_eq!(gram.ncols(), size);
    let mut inverse = vec![vec![context.zero(); size]; size];
    for column in 0..size {
        let right_hand_side = oracle_matrix(
            (0..size)
                .map(|row| {
                    if row == column {
                        context.one()
                    } else {
                        context.zero()
                    }
                })
                .collect(),
            size,
            1,
            "oracle identity column",
        );
        let solution = gram.solve(&right_hand_side).unwrap_or_else(|error| {
            panic!("Symbolica could not solve Gram inverse column {column}: {error}")
        });
        assert_eq!((solution.nrows(), solution.ncols()), (size, 1));
        for (row, coefficient) in solution.into_vec().into_iter().enumerate() {
            assert!(context.contains(&coefficient));
            inverse[row][column] = coefficient;
        }
    }
    inverse
}

fn rows_to_matrix(rows: &[Vec<Coefficient>], label: &str) -> OracleMatrix {
    assert!(!rows.is_empty());
    let columns = rows[0].len();
    assert!(rows.iter().all(|row| row.len() == columns));
    oracle_matrix(
        rows.iter().flatten().cloned().collect(),
        rows.len(),
        columns,
        label,
    )
}

fn assert_native_identity(context: &CoefficientContext, matrix: &OracleMatrix, label: &str) {
    assert_eq!(matrix.nrows(), matrix.ncols(), "{label} is not square");
    for (row, entries) in matrix.row_iter().enumerate() {
        for (column, coefficient) in entries.iter().enumerate() {
            let expected = if row == column {
                context.one()
            } else {
                context.zero()
            };
            assert_coefficient_eq(
                context,
                coefficient,
                &expected,
                &format!("{label} entry ({row},{column})"),
            );
        }
    }
}

fn shared_pairs(left: &SlotPairing, right: &SlotPairing) -> usize {
    left.pairs()
        .iter()
        .filter(|pair| right.pairs().contains(pair))
        .count()
}

type CommonScalarProductFactors = Vec<((usize, usize), u32)>;
type CommonTensorKey = (MetricPairing, CommonScalarProductFactors);

fn legacy_output_snapshot(
    reduction: &rustred::TensorReduction,
) -> BTreeMap<CommonTensorKey, Coefficient> {
    reduction
        .terms()
        .iter()
        .map(|term| {
            let factors = term
                .scalar_products()
                .factors()
                .iter()
                .map(|(product, exponent)| {
                    (
                        (
                            usize::from(product.left().id()),
                            usize::from(product.right().id()),
                        ),
                        *exponent,
                    )
                })
                .collect();
            (
                (term.metrics().clone(), factors),
                term.coefficient().clone(),
            )
        })
        .collect()
}

fn authenticated_output_snapshot(
    projection: &rustred::AuthenticatedVacuumTensorProjection,
) -> BTreeMap<CommonTensorKey, Coefficient> {
    projection
        .numerator()
        .terms()
        .iter()
        .map(|term| {
            let factors = term
                .scalar_products()
                .factors()
                .iter()
                .map(|(coordinate, exponent)| match coordinate {
                    ScalarProductCoordinate::LoopLoop { left, right } => {
                        ((*left, *right), *exponent)
                    }
                    ScalarProductCoordinate::LoopExternal { .. } => {
                        panic!("a vacuum projector emitted a loop-external scalar product")
                    }
                })
                .collect();
            (
                (term.metrics().clone(), factors),
                term.coefficient().clone(),
            )
        })
        .collect()
}

fn inverse_entry_output_key(
    vectors: &[IndexedVector],
    output_pairing: &SlotPairing,
    source_pairing: &SlotPairing,
) -> CommonTensorKey {
    let metrics = MetricPairing::new(
        output_pairing
            .pairs()
            .iter()
            .map(|&(left, right)| Metric::new(vectors[left].index(), vectors[right].index())),
    );
    let mut factors = BTreeMap::<(usize, usize), u32>::new();
    for &(left, right) in source_pairing.pairs() {
        let left = usize::from(vectors[left].vector().id());
        let right = usize::from(vectors[right].vector().id());
        let pair = if left <= right {
            (left, right)
        } else {
            (right, left)
        };
        *factors.entry(pair).or_default() += 1;
    }
    (metrics, factors.into_iter().collect())
}

fn assert_legacy_authenticated_compatibility() {
    // Six distinct loop labels make every source scalar-product pairing
    // unique.  Thus every rank-six output coefficient exposes one inverse-Gram
    // entry rather than only the identical-vector row sum.
    let family = complete_coordinate_family(6);
    let context = family.coefficient_context();
    let mut legacy = VacuumTensorProjector::with_dimension(context, family.dimension().clone());
    let authenticated = GenericVacuumTensorProjector::new();

    for (rank, pairing_count) in [(0, 0), (2, 1), (4, 3), (6, 15)] {
        let input = distinct_source(rank);
        let legacy_output = legacy.reduce(&input).unwrap();
        let authenticated_output = authenticated.project(&family, &input).unwrap();
        let legacy_snapshot = legacy_output_snapshot(&legacy_output);
        let authenticated_snapshot = authenticated_output_snapshot(&authenticated_output);
        let expected_terms = if rank == 0 {
            1
        } else {
            pairing_count * pairing_count
        };
        assert_eq!(legacy_snapshot.len(), expected_terms);
        assert_eq!(authenticated_snapshot.len(), expected_terms);

        for (key, legacy_coefficient) in &legacy_snapshot {
            let authenticated_coefficient = authenticated_snapshot
                .get(key)
                .unwrap_or_else(|| panic!("rank-{rank} legacy structure is missing natively"));
            assert_coefficient_eq(
                context,
                legacy_coefficient,
                authenticated_coefficient,
                &format!("rank-{rank} legacy/authenticated output coefficient"),
            );
        }

        if rank == 0 {
            continue;
        }
        let witness = authenticated_output.witness();
        let vectors = witness.contraction().vectors();
        for (output_position, output_pairing) in witness.pairings().iter().enumerate() {
            for (source_position, source_pairing) in witness.pairings().iter().enumerate() {
                let key = inverse_entry_output_key(vectors, output_pairing, source_pairing);
                let expected_inverse = &witness.inverse_gram()[output_position][source_position];
                let authenticated_coefficient = authenticated_snapshot.get(&key).unwrap();
                let legacy_coefficient = legacy_snapshot.get(&key).unwrap();
                assert_coefficient_eq(
                    context,
                    authenticated_coefficient,
                    expected_inverse,
                    &format!(
                        "rank-{rank} authenticated inverse/output entry ({output_position},{source_position})"
                    ),
                );
                assert_coefficient_eq(
                    context,
                    legacy_coefficient,
                    expected_inverse,
                    &format!(
                        "rank-{rank} legacy inverse/output entry ({output_position},{source_position})"
                    ),
                );
            }
        }
    }
}

fn assert_frozen_vakint_inverse(
    context: &CoefficientContext,
    rank: usize,
    pairings: &[SlotPairing],
    inverse: &[Vec<Coefficient>],
) {
    match rank {
        2 => {
            let expected = context.parse("1/d").unwrap();
            assert_eq!(pairings.len(), 1);
            assert_coefficient_eq(context, &inverse[0][0], &expected, "rank-two inverse");
        }
        4 => {
            let diagonal = context.parse("(d+1)/(d*(d-1)*(d+2))").unwrap();
            let off_diagonal = context.parse("-1/(d*(d-1)*(d+2))").unwrap();
            for (row, left) in pairings.iter().enumerate() {
                let mut diagonal_count = 0;
                let mut off_diagonal_count = 0;
                for (column, right) in pairings.iter().enumerate() {
                    let common = shared_pairs(left, right);
                    let expected = match common {
                        2 => {
                            diagonal_count += 1;
                            &diagonal
                        }
                        0 => {
                            off_diagonal_count += 1;
                            &off_diagonal
                        }
                        _ => panic!("unexpected rank-four pairing intersection {common}"),
                    };
                    assert_coefficient_eq(
                        context,
                        &inverse[row][column],
                        expected,
                        &format!("rank-four Vakint inverse entry ({row},{column})"),
                    );
                }
                assert_eq!((diagonal_count, off_diagonal_count), (1, 2));
            }
        }
        6 => {
            // `tensorreduce.frm` groups the 15 source pairings into the
            // identical/share-one/share-none classes of sizes 1, 6, and 8.
            let common_denominator = "d*(d-1)*(d-2)*(d+2)*(d+4)";
            let identical = context
                .parse(&format!("(d^2+3*d-2)/({common_denominator})"))
                .unwrap();
            let share_one = context
                .parse(&format!("-(d+2)/({common_denominator})"))
                .unwrap();
            let share_none = context.parse(&format!("2/({common_denominator})")).unwrap();
            for (row, left) in pairings.iter().enumerate() {
                let mut class_counts = [0_usize; 4];
                for (column, right) in pairings.iter().enumerate() {
                    let common = shared_pairs(left, right);
                    class_counts[common] += 1;
                    let expected = match common {
                        3 => &identical,
                        1 => &share_one,
                        0 => &share_none,
                        _ => panic!("unexpected rank-six pairing intersection {common}"),
                    };
                    assert_coefficient_eq(
                        context,
                        &inverse[row][column],
                        expected,
                        &format!("rank-six Vakint inverse entry ({row},{column})"),
                    );
                }
                assert_eq!(class_counts, [8, 6, 0, 1]);
            }
        }
        _ => panic!("no frozen Vakint inverse fixture for rank {rank}"),
    }
}

fn determinant_formula(context: &CoefficientContext, rank: usize) -> Coefficient {
    context
        .parse(match rank {
            2 => "d",
            4 => "d^3*(d-1)^2*(d+2)",
            6 => "d^15*(d-1)^14*(d-2)^5*(d+2)^10*(d+4)",
            _ => panic!("no determinant fixture for rank {rank}"),
        })
        .unwrap()
}

fn assert_identical_vector_projection(
    family: &IntegralFamily,
    rank: usize,
    projection: &rustred::AuthenticatedVacuumTensorProjection,
) {
    let context = family.coefficient_context();
    let (term_count, coefficient) = match rank {
        0 => (1, context.one()),
        2 => (1, context.parse("1/d").unwrap()),
        4 => (3, context.parse("1/(d*(d+2))").unwrap()),
        6 => (15, context.parse("1/(d*(d+2)*(d+4))").unwrap()),
        _ => panic!("no identical-vector fixture for rank {rank}"),
    };
    assert_eq!(projection.numerator().terms().len(), term_count);

    let vectors = projection.witness().contraction().vectors();
    let expected_metrics =
        projection
            .witness()
            .pairings()
            .iter()
            .map(|pairing| {
                MetricPairing::new(pairing.pairs().iter().map(|&(left, right)| {
                    Metric::new(vectors[left].index(), vectors[right].index())
                }))
            })
            .collect::<BTreeSet<_>>();
    let actual_metrics = projection
        .numerator()
        .terms()
        .iter()
        .map(|term| term.metrics().clone())
        .collect::<BTreeSet<_>>();

    if rank == 0 {
        assert!(expected_metrics.is_empty());
        assert_eq!(actual_metrics, BTreeSet::from([MetricPairing::empty()]));
    } else {
        assert_eq!(actual_metrics, expected_metrics);
    }
    for term in projection.numerator().terms() {
        assert_coefficient_eq(
            context,
            term.coefficient(),
            &coefficient,
            &format!("rank-{rank} identical-vector projector coefficient"),
        );
        assert_eq!(term.metrics().metrics().len(), rank / 2);
        assert_eq!(
            term.scalar_products()
                .exponent(ScalarProductCoordinate::LoopLoop { left: 0, right: 0 }),
            u32::try_from(rank / 2).unwrap(),
        );
    }
}

#[test]
fn ranks_zero_two_four_and_six_match_independent_symbolica_and_vakint_oracles() {
    let family = family();
    let context = family.coefficient_context();
    let projector = GenericVacuumTensorProjector::new();

    let scalar = projector.project(&family, &source(0)).unwrap();
    assert_eq!(scalar.witness().rank(), 0);
    assert!(scalar.witness().pairings().is_empty());
    assert!(scalar.witness().inverse_gram().is_empty());
    assert_eq!(scalar.stats().gram_entries, 0);
    assert_eq!(scalar.stats().inverse_entries, 0);
    assert_identical_vector_projection(&family, 0, &scalar);
    scalar.verify(&family).unwrap();

    for (rank, pairing_count) in [(2, 1), (4, 3), (6, 15)] {
        let projection = projector.project(&family, &source(rank)).unwrap();
        assert_eq!(projection.witness().rank(), rank);
        assert_eq!(projection.witness().pairings().len(), pairing_count);
        assert_eq!(projection.witness().inverse_gram().len(), pairing_count);
        assert!(
            projection
                .witness()
                .inverse_gram()
                .iter()
                .all(|row| row.len() == pairing_count),
        );

        let gram = independent_gram(&family, projection.witness().pairings());
        let determinant = gram.det().unwrap_or_else(|error| {
            panic!("Symbolica could not compute the rank-{rank} Gram determinant: {error}")
        });
        assert_coefficient_eq(
            context,
            &determinant,
            &determinant_formula(context, rank),
            &format!("rank-{rank} Gram determinant"),
        );

        let solved = solve_inverse(context, &gram);
        for row in 0..pairing_count {
            for column in 0..pairing_count {
                assert_coefficient_eq(
                    context,
                    &projection.witness().inverse_gram()[row][column],
                    &solved[row][column],
                    &format!("rank-{rank} solved inverse entry ({row},{column})"),
                );
            }
        }

        let retained_inverse = rows_to_matrix(
            projection.witness().inverse_gram(),
            "retained projector inverse",
        );
        assert_native_identity(
            context,
            &(&gram * &retained_inverse),
            &format!("rank-{rank} Gram times retained inverse"),
        );
        assert_native_identity(
            context,
            &(&retained_inverse * &gram),
            &format!("rank-{rank} retained inverse times Gram"),
        );
        assert_frozen_vakint_inverse(
            context,
            rank,
            projection.witness().pairings(),
            projection.witness().inverse_gram(),
        );
        assert_identical_vector_projection(&family, rank, &projection);
        projection.verify(&family).unwrap();
    }

    // Compatibility is deliberately checked last: agreement between two
    // production-facing projectors is supporting evidence, not the oracle for
    // either implementation.
    assert_legacy_authenticated_compatibility();
}
