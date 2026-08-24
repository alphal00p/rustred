//! Natural generated sunset validation for the generic Boolean product-locus
//! layer. Loop count and concrete powers occur only in this oracle fixture.

use std::sync::Arc;

use rustred::{
    AffineDenominator, CoefficientContext, GeneratedSectorDiscoveryCompiler,
    GeneratedSectorDiscoveryLimits, GeneratedSectorLiveLeafQueueCompiler,
    GeneratedSectorLiveLeafQueueLimits, IntegralFamily, IntegralOrderingPolicy,
    ParametricCoefficientContext, ParametricIbpGenerator, ResidualProductLocusBooleanCoverCompiler,
    ResidualProductLocusBooleanCoverLimits, ResidualProductLocusBooleanNodeOutcome, SectorMask,
};

fn coordinate_clause(
    context: &ParametricCoefficientContext,
    cover: &rustred::ResidualProductLocusBooleanCoverCertificate,
    clause: &[usize],
) -> Vec<(usize, i64)> {
    let mut result = Vec::new();
    let exact = cover
        .source_queue()
        .discovery()
        .limits()
        .coverage
        .sector_cases
        .exact_algebra;
    // This independent exact oracle contains every coordinate constant
    // expected from the generated sunset fixture. It proves K-association to
    // n_i-c rather than inferring a coordinate root from sampled evaluations.
    let probes = [i64::MIN + 1, -2, -1, 0, 1, 2, i64::MAX - 1, i64::MAX];
    for &ordinal in clause {
        let polynomial = cover
            .source_queue()
            .discovery()
            .coverage()
            .structural_locus(ordinal)
            .unwrap();
        let mut recognized = None;
        for position in 0..context.index_count() {
            for &value in &probes {
                let coordinate = context
                    .sub(&context.index(position).unwrap(), &context.integer(value))
                    .unwrap();
                let coordinate = context.numerator_condition(&coordinate).unwrap();
                if context
                    .polynomial_loci_are_associates_with_limits(polynomial, &coordinate, exact)
                    .unwrap()
                {
                    assert!(recognized.replace((position, value)).is_none());
                }
            }
        }
        result.push(recognized.expect("sunset factor must be a coordinate locus"));
    }
    result.sort_unstable();
    result
}

fn sunset(name: &str) -> IntegralFamily {
    let coefficients = CoefficientContext::new(["d", "m2"]);
    let zero = coefficients.zero();
    let one = coefficients.one();
    let minus_m2 = coefficients.parse("-m2").unwrap();
    IntegralFamily::new(
        name,
        vec!["k1".into(), "k2".into()],
        Vec::new(),
        coefficients.clone(),
        coefficients.parameter("d").unwrap(),
        vec![
            AffineDenominator::new(
                minus_m2.clone(),
                vec![one.clone(), zero.clone(), zero.clone()],
            ),
            AffineDenominator::new(
                minus_m2.clone(),
                vec![zero.clone(), zero.clone(), one.clone()],
            ),
            AffineDenominator::new(minus_m2, vec![one.clone(), coefficients.integer(2), one]),
        ],
        Vec::new(),
        vec![zero.clone(), zero.clone(), zero],
    )
    .unwrap()
}

fn generated_cover(
    bits: &str,
) -> (
    IntegralFamily,
    ParametricCoefficientContext,
    rustred::ResidualProductLocusBooleanCoverCertificate,
) {
    let family = sunset(&format!("boolean-product-locus-sunset-{bits}"));
    let context = ParametricIbpGenerator::try_new(&family)
        .unwrap()
        .context()
        .clone();
    let sector = SectorMask::try_from_bit_string(bits).unwrap();
    let mut discovery_limits = GeneratedSectorDiscoveryLimits::default();
    discovery_limits.adaptive.max_search_depth = 0;
    let discovery = GeneratedSectorDiscoveryCompiler::compile(
        &family,
        &context,
        sector,
        IntegralOrderingPolicy::RustRedUnshiftedV1,
        discovery_limits,
    )
    .unwrap();
    let mut queue_limits = GeneratedSectorLiveLeafQueueLimits::default();
    queue_limits.translation_radius = 0;
    queue_limits.max_translation_points = 1;
    let queue = Arc::new(
        GeneratedSectorLiveLeafQueueCompiler::compile(&family, &context, &discovery, queue_limits)
            .unwrap(),
    );
    assert_eq!(queue.work_items().len(), 1, "sector {bits}");
    let cover = ResidualProductLocusBooleanCoverCompiler::compile(
        &family,
        &context,
        queue,
        0,
        ResidualProductLocusBooleanCoverLimits::default(),
    )
    .unwrap();
    (family, context, cover)
}

fn test_values(active: bool) -> [i64; 3] {
    if active {
        [1, 2, i64::MAX]
    } else {
        [0, -1, -2]
    }
}

#[test]
fn natural_sunset_product_leaves_replay_and_form_the_exact_disjoint_source_leaf() {
    for bits in ["011", "101", "110", "111"] {
        let (family, context, cover) = generated_cover(bits);
        cover.replay(&family, &context).unwrap();
        assert!(
            cover.stats().product_equalities_expanded() >= 1,
            "sector {bits}"
        );
        assert!(
            cover.stats().factor_references_expanded() >= 2,
            "sector {bits}"
        );
        assert!(
            cover.terminals().any(|node| matches!(
                node.outcome(),
                ResidualProductLocusBooleanNodeOutcome::ReadyForAffineRecognition
            )),
            "sector {bits}"
        );

        let mut actual_clauses: Vec<_> = cover
            .root_clauses()
            .iter()
            .map(|clause| coordinate_clause(&context, &cover, clause))
            .collect();
        actual_clauses.sort();
        let m = i64::MAX;
        let mut expected_clauses: Vec<Vec<(usize, i64)>> = match bits {
            "011" => vec![vec![(0, 0), (2, m), (1, 1)], vec![(0, 0), (1, m), (2, 1)]],
            "101" => vec![
                vec![(1, 0), (2, 0), (2, m), (0, 1)],
                vec![(1, 0), (0, m), (2, 1)],
            ],
            "110" => vec![
                vec![(2, 0), (2, -1), (0, m), (1, 1)],
                vec![(2, 0), (2, -1), (0, 1)],
                vec![(2, 0), (0, 1), (1, m)],
            ],
            "111" => vec![
                vec![(0, 1), (2, m)],
                vec![(1, 1), (2, m)],
                vec![(0, 0), (0, m), (2, 1)],
            ],
            _ => unreachable!(),
        };
        for clause in &mut expected_clauses {
            clause.sort_unstable();
        }
        expected_clauses.sort();
        assert_eq!(actual_clauses, expected_clauses, "root CNF sector {bits}");

        let values: Vec<_> = bits.bytes().map(|bit| test_values(bit == b'1')).collect();
        for &n0 in &values[0] {
            for &n1 in &values[1] {
                for &n2 in &values[2] {
                    let indices = [n0, n1, n2];
                    let classification = cover
                        .source_queue()
                        .discovery()
                        .coverage()
                        .classification_for_indices(&context, &indices)
                        .unwrap();
                    let in_source =
                        classification.is_some_and(|leaf| leaf.case() == cover.source_case());
                    let matched = cover
                        .ready_terminal_for_indices(&context, &indices)
                        .unwrap()
                        .is_some();
                    assert_eq!(matched, in_source, "sector {bits}, point {indices:?}");

                    let coverage = cover.source_queue().discovery().coverage();
                    let arithmetic = cover
                        .source_queue()
                        .discovery()
                        .limits()
                        .coverage
                        .generated_when_bad
                        .when_bad
                        .arithmetic;
                    let terminal_matches = cover
                        .terminals()
                        .filter(|node| {
                            if !matches!(
                                node.outcome(),
                                ResidualProductLocusBooleanNodeOutcome::ReadyForAffineRecognition
                            ) {
                                return false;
                            }
                            node.equal_zero_atoms().iter().all(|&ordinal| {
                                context
                                    .specialize_polynomial(
                                        coverage.structural_locus(ordinal).unwrap(),
                                        &indices,
                                        arithmetic,
                                    )
                                    .unwrap()
                                    .is_zero()
                            }) && node.nonzero_atoms().iter().all(|&ordinal| {
                                !context
                                    .specialize_polynomial(
                                        coverage.structural_locus(ordinal).unwrap(),
                                        &indices,
                                        arithmetic,
                                    )
                                    .unwrap()
                                    .is_zero()
                            })
                        })
                        .count();
                    assert_eq!(
                        terminal_matches,
                        usize::from(in_source),
                        "terminal disjointness/union, sector {bits}, point {indices:?}"
                    );
                }
            }
        }
    }
}

#[test]
fn natural_sunset_cover_respects_a_pre_native_source_lookup_budget() {
    let family = sunset("boolean-product-locus-sunset-budget");
    let context = ParametricIbpGenerator::try_new(&family)
        .unwrap()
        .context()
        .clone();
    let mut discovery_limits = GeneratedSectorDiscoveryLimits::default();
    discovery_limits.adaptive.max_search_depth = 0;
    let discovery = GeneratedSectorDiscoveryCompiler::compile(
        &family,
        &context,
        SectorMask::try_from_bit_string("111").unwrap(),
        IntegralOrderingPolicy::RustRedUnshiftedV1,
        discovery_limits,
    )
    .unwrap();
    let mut queue_limits = GeneratedSectorLiveLeafQueueLimits::default();
    queue_limits.translation_radius = 0;
    queue_limits.max_translation_points = 1;
    let queue = Arc::new(
        GeneratedSectorLiveLeafQueueCompiler::compile(&family, &context, &discovery, queue_limits)
            .unwrap(),
    );
    let mut limits = ResidualProductLocusBooleanCoverLimits::default();
    limits.max_structural_locus_lookup_comparisons = 0;
    assert!(matches!(
        ResidualProductLocusBooleanCoverCompiler::compile(&family, &context, queue, 0, limits),
        Err(
            rustred::ResidualProductLocusBooleanCoverError::ResourceLimit {
                resource: "structural-locus lookup comparisons",
                ..
            }
        )
    ));
}
