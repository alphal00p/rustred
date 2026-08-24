//! Complete FORM-free one-loop tensor/reduction validation matrix.
//!
//! Tensor expectations are the covariant projector identities exercised by
//! Vakint's tensor tests and documented beside `tensorreduce.frm`.  The scalar
//! recurrence is independently encoded from `IntegrateUV1L` in
//! `integrateduv.frm`.  RustRed itself receives only a generic
//! `IntegralFamily`, projects the complete source monomial with Symbolica,
//! lowers scalar products, and discovers every reduction from generated IBPs.

use rustred::*;

fn family() -> IntegralFamily {
    let context = CoefficientContext::new(["d", "m2"]);
    IntegralFamily::new(
        "vakint-one-loop-tensor-matrix",
        vec!["k".into()],
        Vec::new(),
        context.clone(),
        context.parameter("d").unwrap(),
        // Vakint convention: k.k = D1 + mUV^2.
        vec![AffineDenominator::new(
            context.parse("-m2").unwrap(),
            vec![context.one()],
        )],
        Vec::new(),
        vec![context.zero()],
    )
    .unwrap()
}

fn key(power: i64) -> ConcreteIntegralKey {
    ConcreteIntegralKey::try_new([power]).unwrap()
}

fn k(index: u32) -> IndexedVector {
    IndexedVector::new(LoopVector::new(0), LorentzIndex::new(index))
}

fn p(vector: u32, index: u32) -> IndexedSpectatorVector {
    IndexedSpectatorVector::new(SpectatorVector::new(vector), LorentzIndex::new(index))
}

fn source(
    loops: impl IntoIterator<Item = IndexedVector>,
    spectators: impl IntoIterator<Item = IndexedSpectatorVector>,
) -> CovariantTensorMonomial {
    CovariantTensorMonomial::try_from_parts_with_limits(
        loops,
        spectators,
        [],
        ScalarProductMonomial::one(),
        SpectatorScalarProductMonomial::one(),
        GenericTensorProjectorLimits::default(),
    )
    .unwrap()
}

fn covariant(
    metrics: impl IntoIterator<Item = Metric>,
    spectators: impl IntoIterator<Item = IndexedSpectatorVector>,
    products: impl IntoIterator<Item = (SpectatorScalarProduct, u32)>,
) -> TensorCovariantStructure {
    TensorCovariantStructure::new(
        MetricPairing::new(metrics),
        spectators.into_iter().collect(),
        SpectatorScalarProductMonomial::try_from_factors_with_limits(
            products,
            GenericTensorProjectorLimits::default(),
        )
        .unwrap(),
    )
}

fn checked_mul(
    context: &CoefficientContext,
    left: &Coefficient,
    right: &Coefficient,
) -> Coefficient {
    context
        .try_mul(left, right, ExactAlgebraLimits::default())
        .unwrap()
}

fn checked_add(
    context: &CoefficientContext,
    left: &Coefficient,
    right: &Coefficient,
) -> Coefficient {
    context
        .try_add(left, right, ExactAlgebraLimits::default())
        .unwrap()
}

fn checked_div(
    context: &CoefficientContext,
    left: &Coefficient,
    right: &Coefficient,
) -> Coefficient {
    context
        .try_div(left, right, ExactAlgebraLimits::default())
        .unwrap()
}

fn coefficient_power(
    context: &CoefficientContext,
    base: &Coefficient,
    exponent: usize,
) -> Coefficient {
    let mut result = context.one();
    for _ in 0..exponent {
        result = checked_mul(context, &result, base);
    }
    result
}

fn binomial(n: usize, k: usize) -> i64 {
    let k = k.min(n - k);
    let mut value = 1_u64;
    for step in 0..k {
        value = value * u64::try_from(n - step).unwrap() / u64::try_from(step + 1).unwrap();
    }
    i64::try_from(value).unwrap()
}

/// Frozen alphaLoop `IntegrateUV1L` oracle, with its final master value left
/// unsubstituted: I(n) = (d+2-2n)/(2(n-1)m2) I(n-1), n > 1.
fn alphaloop_integral_coefficient(context: &CoefficientContext, power: i64) -> Coefficient {
    if power <= 0 {
        return context.zero();
    }
    let d = context.parameter("d").unwrap();
    let m2 = context.parameter("m2").unwrap();
    let mut result = context.one();
    for current in 2..=power {
        let numerator = checked_add(context, &d, &context.integer(2 - 2 * current));
        let denominator = checked_mul(context, &context.integer(2 * (current - 1)), &m2);
        result = checked_mul(
            context,
            &result,
            &checked_div(context, &numerator, &denominator),
        );
    }
    result
}

/// `(k^2)^rank I(a) = sum_s binomial(rank,s) m2^(rank-s) I(a-s)` followed by
/// the independent alphaLoop scalar recurrence above.
fn alphaloop_moment_coefficient(
    context: &CoefficientContext,
    power: i64,
    moment_rank: usize,
) -> Coefficient {
    let m2 = context.parameter("m2").unwrap();
    let mut result = context.zero();
    for shifted in 0..=moment_rank {
        let mass_power = coefficient_power(context, &m2, moment_rank - shifted);
        let integral =
            alphaloop_integral_coefficient(context, power - i64::try_from(shifted).unwrap());
        let weighted = checked_mul(
            context,
            &context.integer(binomial(moment_rank, shifted)),
            &checked_mul(context, &mass_power, &integral),
        );
        result = checked_add(context, &result, &weighted);
    }
    result
}

fn generated_active_coverage(
    family: &IntegralFamily,
) -> (
    ParametricCoefficientContext,
    ParametricSectorCoverageCertificate,
) {
    let generated = ParametricIbpGenerator::try_new(family)
        .unwrap()
        .generate()
        .unwrap();
    let context = generated.context().clone();
    let sector = SectorMask::try_new([true]).unwrap();
    let discovery = GeneratedSectorDiscoveryCompiler::compile(
        family,
        &context,
        sector.clone(),
        IntegralOrderingPolicy::RustRedUnshiftedV1,
        GeneratedSectorDiscoveryLimits::default(),
    )
    .unwrap();
    discovery.replay(family, &context).unwrap();
    (context, discovery.coverage().clone())
}

struct TensorCase {
    name: &'static str,
    source: CovariantTensorMonomial,
    moment_rank: usize,
    projector_denominator: Coefficient,
    covariants: Vec<TensorCovariantStructure>,
}

#[test]
fn full_vakint_one_loop_tensor_matrix_reduces_for_powers_one_through_four() {
    let family = family();
    let context = family.coefficient_context();
    let d = context.parameter("d").unwrap();
    let d_plus_two = checked_add(context, &d, &context.integer(2));
    let rank_four_denominator = checked_mul(context, &d, &d_plus_two);

    let empty = covariant([], [], []);
    let free_rank_two = covariant([Metric::new(1.into(), 2.into())], [], []);
    let external_rank_two = covariant(
        [],
        [],
        [(SpectatorScalarProduct::new(10.into(), 11.into()), 1)],
    );
    let mixed_rank_two = covariant([], [p(10, 1)], []);

    let free_rank_four = vec![
        covariant(
            [
                Metric::new(1.into(), 2.into()),
                Metric::new(3.into(), 4.into()),
            ],
            [],
            [],
        ),
        covariant(
            [
                Metric::new(1.into(), 3.into()),
                Metric::new(2.into(), 4.into()),
            ],
            [],
            [],
        ),
        covariant(
            [
                Metric::new(1.into(), 4.into()),
                Metric::new(2.into(), 3.into()),
            ],
            [],
            [],
        ),
    ];
    let external_rank_four = vec![
        covariant(
            [],
            [],
            [
                (SpectatorScalarProduct::new(10.into(), 11.into()), 1),
                (SpectatorScalarProduct::new(12.into(), 13.into()), 1),
            ],
        ),
        covariant(
            [],
            [],
            [
                (SpectatorScalarProduct::new(10.into(), 12.into()), 1),
                (SpectatorScalarProduct::new(11.into(), 13.into()), 1),
            ],
        ),
        covariant(
            [],
            [],
            [
                (SpectatorScalarProduct::new(10.into(), 13.into()), 1),
                (SpectatorScalarProduct::new(11.into(), 12.into()), 1),
            ],
        ),
    ];
    let mixed_rank_four = vec![
        covariant(
            [Metric::new(1.into(), 2.into())],
            [],
            [(SpectatorScalarProduct::new(10.into(), 11.into()), 1)],
        ),
        covariant([], [p(10, 1), p(11, 2)], []),
        covariant([], [p(10, 2), p(11, 1)], []),
    ];

    let cases = vec![
        TensorCase {
            name: "scalar",
            source: source([], []),
            moment_rank: 0,
            projector_denominator: context.one(),
            covariants: vec![empty],
        },
        TensorCase {
            name: "odd-free-rank-one",
            source: source([k(1)], []),
            moment_rank: 0,
            projector_denominator: context.one(),
            covariants: vec![],
        },
        TensorCase {
            name: "odd-k-dot-p",
            source: source([k(1)], [p(10, 1)]),
            moment_rank: 0,
            projector_denominator: context.one(),
            covariants: vec![],
        },
        TensorCase {
            name: "free-rank-two",
            source: source([k(1), k(2)], []),
            moment_rank: 1,
            projector_denominator: d.clone(),
            covariants: vec![free_rank_two],
        },
        TensorCase {
            name: "external-rank-two",
            source: source([k(1), k(2)], [p(10, 1), p(11, 2)]),
            moment_rank: 1,
            projector_denominator: d.clone(),
            covariants: vec![external_rank_two],
        },
        TensorCase {
            name: "mixed-rank-two",
            source: source([k(1), k(2)], [p(10, 2)]),
            moment_rank: 1,
            projector_denominator: d.clone(),
            covariants: vec![mixed_rank_two],
        },
        TensorCase {
            name: "odd-mixed-rank-three",
            source: source([k(1), k(2), k(3)], [p(10, 2), p(11, 3)]),
            moment_rank: 0,
            projector_denominator: context.one(),
            covariants: vec![],
        },
        TensorCase {
            name: "free-rank-four",
            source: source([k(1), k(2), k(3), k(4)], []),
            moment_rank: 2,
            projector_denominator: rank_four_denominator.clone(),
            covariants: free_rank_four,
        },
        TensorCase {
            name: "external-rank-four",
            source: source(
                [k(1), k(2), k(3), k(4)],
                [p(10, 1), p(11, 2), p(12, 3), p(13, 4)],
            ),
            moment_rank: 2,
            projector_denominator: rank_four_denominator.clone(),
            covariants: external_rank_four,
        },
        TensorCase {
            name: "mixed-rank-four",
            source: source([k(1), k(2), k(3), k(4)], [p(10, 3), p(11, 4)]),
            moment_rank: 2,
            projector_denominator: rank_four_denominator,
            covariants: mixed_rank_four,
        },
    ];

    let (parametric_context, coverage) = generated_active_coverage(&family);
    let sector_provider = ParametricSectorRuleProvider::try_new(
        &family,
        &parametric_context,
        [coverage],
        ParametricSectorRuleProviderLimits::default(),
    )
    .unwrap();
    let master_provider = MasterPolicyProvider::with_selected(sector_provider, [key(1)]).unwrap();
    let provider = CertifiedZeroSectorRuleProvider::try_unrestricted(
        &family,
        PowerShiftPolicy::FormalGeneric,
        master_provider,
        CertifiedRewriteLimits::default(),
    )
    .unwrap();
    let mut engine = ParametricReductionEngine::new(
        family.fingerprint(),
        family.coefficient_context(),
        IntegralOrderingPolicy::RustRedUnshiftedV1,
        provider,
        ReductionEngineLimits::default(),
    );

    for power in 1..=4_i64 {
        for case in &cases {
            let projection = GenericVacuumTensorPolynomialProjector::new()
                .project(
                    &family,
                    [WeightedCovariantTensorMonomial::new(
                        context.one(),
                        case.source.clone(),
                    )],
                )
                .unwrap();
            let lowering = projection.lower(&family, &key(power)).unwrap();
            let result = TensorParametricReductionComposer::new(&family)
                .reduce_authenticated_covariant_polynomial(lowering, &mut engine)
                .unwrap();
            result.require_complete().unwrap();
            result.verify(&family).unwrap();

            if case.covariants.is_empty() {
                assert!(
                    result.scalar_reduction().is_zero(),
                    "{} at propagator power {power} should vanish",
                    case.name,
                );
                continue;
            }

            let expected = checked_div(
                context,
                &alphaloop_moment_coefficient(context, power, case.moment_rank),
                &case.projector_denominator,
            );
            assert_eq!(
                result.scalar_reduction().len(),
                case.covariants.len(),
                "{} at propagator power {power} emitted the wrong covariant count",
                case.name,
            );
            for expected_covariant in &case.covariants {
                assert_eq!(
                    result
                        .scalar_reduction()
                        .term(expected_covariant, &key(1))
                        .unwrap()
                        .coefficient(),
                    &expected,
                    "{} at propagator power {power} disagrees with the frozen Vakint/alphaLoop oracle",
                    case.name,
                );
            }
        }
    }
}
