//! Independent black-box validation of automatic ISP completion.
//!
//! The oracle deliberately does not reproduce the production row-reduction
//! algorithm.  For each small rectangular matrix it defines generic rank as
//! the largest order of a nonzero minor and delegates every determinant to
//! Symbolica's public exact matrix API.  Test-only subset enumeration is only
//! orchestration; all coefficient and matrix algebra remains Symbolica-owned.

use rustred::{
    AUTOMATIC_ISP_COMPLETION_V2_SCHEMA, AffineDenominator, AutomaticIspCompletion, Coefficient,
    CoefficientContext,
};
use symbolica::{
    domains::rational_polynomial::RationalPolynomialField,
    prelude::{IntegerRing, Matrix, Z},
};

type OracleField = RationalPolynomialField<IntegerRing, u16>;
type OracleMatrix = Matrix<OracleField>;

#[derive(Debug, PartialEq, Eq)]
struct OracleTranscript {
    appended_ordinals: Vec<usize>,
    rank_progression: Vec<usize>,
}

fn combinations(cardinality: usize, size: usize) -> Vec<Vec<usize>> {
    fn extend(
        cardinality: usize,
        remaining: usize,
        first: usize,
        current: &mut Vec<usize>,
        output: &mut Vec<Vec<usize>>,
    ) {
        if remaining == 0 {
            output.push(current.clone());
            return;
        }
        let final_first = cardinality - remaining;
        for selected in first..=final_first {
            current.push(selected);
            extend(cardinality, remaining - 1, selected + 1, current, output);
            current.pop();
        }
    }

    assert!(size <= cardinality);
    let mut output = Vec::new();
    extend(
        cardinality,
        size,
        0,
        &mut Vec::with_capacity(size),
        &mut output,
    );
    output
}

/// Exact generic rank from maximal minors.  This is intentionally a different
/// Symbolica matrix algorithm from production's native row reduction.
fn rank_from_symbolica_minors(rows: &[Vec<Coefficient>]) -> usize {
    assert!(!rows.is_empty());
    let columns = rows[0].len();
    assert!(columns > 0);
    assert!(rows.iter().all(|row| row.len() == columns));

    for minor_size in (1..=rows.len().min(columns)).rev() {
        let minor_size_u32 = u32::try_from(minor_size).unwrap();
        let column_subsets = combinations(columns, minor_size);
        for row_subset in combinations(rows.len(), minor_size) {
            for column_subset in &column_subsets {
                let data = row_subset
                    .iter()
                    .flat_map(|&row| {
                        column_subset
                            .iter()
                            .map(move |&column| rows[row][column].clone())
                    })
                    .collect();
                let minor: OracleMatrix = Matrix::from_linear(
                    data,
                    minor_size_u32,
                    minor_size_u32,
                    RationalPolynomialField::new(Z),
                )
                .unwrap();
                let determinant = minor.det().unwrap();
                if !determinant.is_zero() {
                    return minor_size;
                }
            }
        }
    }
    0
}

/// Independently replay LiteRed's deterministic unit-row scan, using only the
/// maximal-minor oracle for every rank decision.
fn transcript_from_symbolica_minors(
    context: &CoefficientContext,
    input_rows: &[Vec<Coefficient>],
) -> OracleTranscript {
    let columns = input_rows[0].len();
    let mut rows = input_rows.to_vec();
    let mut rank = rank_from_symbolica_minors(&rows);
    assert_eq!(rank, input_rows.len(), "oracle fixture must be independent");
    let mut appended_ordinals = Vec::with_capacity(columns - rank);
    let mut rank_progression = vec![rank];

    for coordinate in 0..columns {
        if rank == columns {
            break;
        }
        let mut candidate = vec![context.zero(); columns];
        candidate[coordinate] = context.one();
        rows.push(candidate);
        let candidate_rank = rank_from_symbolica_minors(&rows);
        assert!(
            candidate_rank == rank || candidate_rank == rank + 1,
            "one appended row changed oracle rank from {rank} to {candidate_rank}"
        );
        if candidate_rank == rank + 1 {
            appended_ordinals.push(coordinate);
            rank = candidate_rank;
            rank_progression.push(rank);
        } else {
            rows.pop();
        }
    }
    assert_eq!(rank, columns);

    OracleTranscript {
        appended_ordinals,
        rank_progression,
    }
}

fn assert_matches_minor_oracle(
    completion: &AutomaticIspCompletion,
    input_rows: &[Vec<Coefficient>],
) {
    let oracle =
        transcript_from_symbolica_minors(completion.family().coefficient_context(), input_rows);
    assert_eq!(completion.schema(), AUTOMATIC_ISP_COMPLETION_V2_SCHEMA);
    assert_eq!(
        completion.appended_coordinate_ordinals(),
        oracle.appended_ordinals
    );
    assert_eq!(completion.rank_progression(), oracle.rank_progression);
    assert_eq!(
        completion.stats().appended_isps(),
        oracle.appended_ordinals.len()
    );

    let context = completion.family().coefficient_context();
    let columns = input_rows[0].len();
    for (appended, &coordinate) in oracle.appended_ordinals.iter().enumerate() {
        let position = completion.input_denominator_count() + appended;
        let denominator = &completion.family().denominators()[position];
        assert!(denominator.constant().is_zero());
        assert_eq!(denominator.coefficients().len(), columns);
        for (candidate, coefficient) in denominator.coefficients().iter().enumerate() {
            let expected = if candidate == coordinate {
                context.one()
            } else {
                context.zero()
            };
            assert!((coefficient - &expected).is_zero());
        }
        assert!(completion.family().power_shifts()[position].is_zero());
    }
    completion.replay().unwrap();
}

fn affine_rows(
    constants: impl IntoIterator<Item = Coefficient>,
    rows: &[Vec<Coefficient>],
) -> Vec<AffineDenominator> {
    constants
        .into_iter()
        .zip(rows)
        .map(|(constant, row)| AffineDenominator::new(constant, row.clone()))
        .collect()
}

fn parsed_row(context: &CoefficientContext, entries: &[&str]) -> Vec<Coefficient> {
    entries
        .iter()
        .map(|entry| context.parse(entry).unwrap())
        .collect()
}

#[test]
fn one_loop_two_external_gmp_rational_row_is_metadata_invariant() {
    let context = CoefficientContext::new(["d", "x", "m", "nu", "s00", "s01", "s11", "g"]);
    // Coordinate order is k^2, k.p0, k.p1.  The sole supplied row spans e1.
    // The scan must accept e0, reject e1, and then accept e2.  Its large
    // coefficient is deliberately beyond fixed-width integer arithmetic.
    let rows = vec![parsed_row(
        &context,
        &["0", "340282366920938463463374607431768211507/(x+1)", "0"],
    )];
    let generic_gram = vec![
        vec![
            context.parameter("s00").unwrap(),
            context.parameter("s01").unwrap(),
        ],
        vec![
            context.parameter("s01").unwrap(),
            context.parameter("s11").unwrap(),
        ],
    ];
    let completion = AutomaticIspCompletion::try_new(
        "minor-oracle-1l-e2-generic-metadata",
        vec!["k".into()],
        vec!["p0".into(), "p1".into()],
        context.clone(),
        context.parameter("d").unwrap(),
        affine_rows([context.parse("-m").unwrap()], &rows),
        generic_gram,
        vec![context.parameter("nu").unwrap()],
    )
    .unwrap();
    assert_matches_minor_oracle(&completion, &rows);
    assert_eq!(completion.appended_coordinate_ordinals(), &[0, 2]);
    assert_eq!(completion.rank_progression(), &[1, 2, 3]);

    // Rank depends only on the linear scalar-product rows.  Change the affine
    // constant, power shift, and even use a singular external Gram matrix.
    let g = context.parameter("g").unwrap();
    let changed_metadata = AutomaticIspCompletion::try_new(
        "minor-oracle-1l-e2-changed-metadata",
        vec!["k".into()],
        vec!["p0".into(), "p1".into()],
        context.clone(),
        context.parameter("d").unwrap(),
        affine_rows([context.parse("(x-m)/(x+1)").unwrap()], &rows),
        vec![vec![g.clone(), g.clone()], vec![g.clone(), g]],
        vec![context.parse("x/(x+1)").unwrap()],
    )
    .unwrap();
    assert_matches_minor_oracle(&changed_metadata, &rows);
    assert_eq!(
        changed_metadata.appended_coordinate_ordinals(),
        completion.appended_coordinate_ordinals()
    );
    assert_eq!(
        changed_metadata.rank_progression(),
        completion.rank_progression()
    );
}

#[test]
fn two_loop_vacuum_row_swap_and_rejected_units_match_minor_oracle() {
    let context = CoefficientContext::new(["d", "m0", "m1"]);
    // Coordinate order is k0^2, k0.k1, k1^2.  The zero-leading first row
    // forces a pivot search.  e0 and e1 are already in the row span and must
    // be rejected before e2 completes it.
    let rows = vec![
        parsed_row(&context, &["0", "1", "0"]),
        parsed_row(&context, &["1", "0", "0"]),
    ];
    let completion = AutomaticIspCompletion::try_new(
        "minor-oracle-2l-vacuum-row-swap",
        vec!["k0".into(), "k1".into()],
        Vec::new(),
        context.clone(),
        context.parameter("d").unwrap(),
        affine_rows(
            [context.parse("-m0").unwrap(), context.parse("-m1").unwrap()],
            &rows,
        ),
        Vec::new(),
        vec![context.zero(); 2],
    )
    .unwrap();
    assert_matches_minor_oracle(&completion, &rows);
    assert_eq!(completion.appended_coordinate_ordinals(), &[2]);
    assert_eq!(completion.rank_progression(), &[2, 3]);
    assert_eq!(completion.stats().rank_tests(), 4);

    // Permuting the independent input rows changes neither their row span nor
    // the deterministic coordinate scan.
    let permuted_rows = vec![rows[1].clone(), rows[0].clone()];
    let permuted = AutomaticIspCompletion::try_new(
        "minor-oracle-2l-vacuum-permuted",
        vec!["k0".into(), "k1".into()],
        Vec::new(),
        context.clone(),
        context.parameter("d").unwrap(),
        affine_rows(
            [context.parse("-m1").unwrap(), context.parse("-m0").unwrap()],
            &permuted_rows,
        ),
        Vec::new(),
        vec![context.zero(); 2],
    )
    .unwrap();
    assert_matches_minor_oracle(&permuted, &permuted_rows);
    assert_eq!(
        permuted.appended_coordinate_ordinals(),
        completion.appended_coordinate_ordinals()
    );
    assert_eq!(permuted.rank_progression(), completion.rank_progression());
}

#[test]
fn dense_two_loop_two_external_symbolic_rectangle_matches_minor_oracle() {
    let context = CoefficientContext::new(["d", "x", "m0", "m1", "m2", "s00", "s01", "s11"]);
    // Coordinate order is
    // k0^2,k0.k1,k1^2,k0.p0,k0.p1,k1.p0,k1.p1.  The leading 3x3 minor is
    // x^3+x^2+1, so these three dense rational-function rows are generically
    // independent without any numeric specialization.
    let rows = vec![
        parsed_row(&context, &["x", "1", "0", "1/(x+1)", "2", "0", "1"]),
        parsed_row(&context, &["0", "x+1", "1", "3", "0", "1/(x+2)", "2"]),
        parsed_row(&context, &["1", "0", "x", "1", "1", "2", "0"]),
    ];
    let completion = AutomaticIspCompletion::try_new(
        "minor-oracle-2l-e2-dense-symbolic",
        vec!["k0".into(), "k1".into()],
        vec!["p0".into(), "p1".into()],
        context.clone(),
        context.parameter("d").unwrap(),
        affine_rows(
            [
                context.parse("-m0").unwrap(),
                context.parse("-m1").unwrap(),
                context.parse("-m2").unwrap(),
            ],
            &rows,
        ),
        vec![
            vec![
                context.parameter("s00").unwrap(),
                context.parameter("s01").unwrap(),
            ],
            vec![
                context.parameter("s01").unwrap(),
                context.parameter("s11").unwrap(),
            ],
        ],
        vec![context.zero(); 3],
    )
    .unwrap();
    assert_matches_minor_oracle(&completion, &rows);
    assert_eq!(completion.stats().appended_isps(), 4);
    assert_eq!(completion.rank_progression(), &[3, 4, 5, 6, 7]);
}
