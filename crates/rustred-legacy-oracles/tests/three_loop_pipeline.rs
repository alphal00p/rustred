// Restricted Symbolica must stay on one OS worker, so this complete finite-box
// certification lives in a single integration test.

use rustred::{
    CoefficientContext, ExactRational, IndexedVector, LoopVector, LorentzIndex, Metric,
    MetricPairing, TensorMonomial, VacuumTensorProjector,
};
use rustred_legacy_oracles::{
    Denominator, IbpGenerator, Integral, ReductionError, SeedGenerationError, TensorFamilyReducer,
    VacuumFamily,
};
use rustred_legacy_oracles::{
    THREE_LOOP_TETRAHEDRON_ROUTINGS, ThreeLoopBoundaryError, ThreeLoopPipelineError,
    ThreeLoopReductionConfig, ThreeLoopReductionPipeline,
};

fn vector(loop_id: u16, index_id: u32) -> IndexedVector {
    IndexedVector::new(LoopVector::new(loop_id), LorentzIndex::new(index_id))
}

#[test]
fn certified_three_loop_dot_and_numerator_box() {
    let pipeline = ThreeLoopReductionPipeline::build(ThreeLoopReductionConfig::default()).unwrap();
    // All 36 symmetry-unique targets remain certified, while only the 12
    // genuine-sector seeds require sparse rows (12 * 9 IBPs).  Tree/paw rows
    // cannot create genuine-sector pivots and are handled analytically.
    assert_eq!(pipeline.stats().input_equations, 108);
    assert!(pipeline.stats().rules > 0);
    assert_eq!(
        pipeline.masters(),
        &[
            Integral::from([1, 1, 1, 0, 0, 0]),
            Integral::from([1, 1, 1, 1, 0, 0]),
            Integral::from([1, 1, 0, 1, 0, 1]),
            Integral::from([1, 1, 1, 1, 1, 0]),
            Integral::from([1, 1, 1, 1, 1, 1]),
        ]
    );
    // Freeze the complete component one-dot oracle used by four-loop product
    // halo closure. F5 has two line orbits; B4 and M6 are line-transitive.
    let coefficient = pipeline.family().coefficients();
    for (probe, expected) in [
        (
            Integral::from([2, 1, 0, 1, 0, 1]),
            vec![(pipeline.masters()[2].clone(), "(8-3*d)/(8*m2)")],
        ),
        (
            Integral::from([2, 1, 1, 1, 1, 0]),
            vec![
                (pipeline.masters()[2].clone(), "(8-3*d)/(6*m2^2)"),
                (pipeline.masters()[1].clone(), "2*(d-2)/(3*m2^2)"),
                (pipeline.masters()[3].clone(), "(6-d)/(6*m2)"),
            ],
        ),
        (
            Integral::from([1, 2, 1, 1, 1, 0]),
            vec![
                (pipeline.masters()[2].clone(), "(3*d-8)/(24*m2^2)"),
                (pipeline.masters()[1].clone(), "(2-d)/(6*m2^2)"),
                (pipeline.masters()[3].clone(), "(3-d)/(3*m2)"),
            ],
        ),
        (
            Integral::from([2, 1, 1, 1, 1, 1]),
            vec![(pipeline.masters()[4].clone(), "(4-d)/(4*m2)")],
        ),
    ] {
        let reduction = pipeline.reduce_integral(&probe).unwrap();
        assert_eq!(reduction.len(), expected.len());
        for (master, value) in expected {
            assert_eq!(
                reduction.coefficient(&master),
                Some(&coefficient.parse(value).unwrap())
            );
        }
    }

    // Exhaust compact component-line conventions used by the four-loop halo.
    // In particular, compact B4 positions 0,1,2,3 lift to tetrahedron-family
    // positions 0,1,3,5; F5 position 0 is central and 1..4 are outer.
    let b4_positions = [0_usize, 1, 3, 5];
    let mut b4_dot_sum = rustred_legacy_oracles::LinearCombination::new();
    for position in b4_positions {
        let mut powers = [1, 1, 0, 1, 0, 1];
        powers[position] = 2;
        let reduction = pipeline.reduce_integral(&Integral::from(powers)).unwrap();
        assert_eq!(
            reduction.coefficient(&pipeline.masters()[2]),
            Some(&coefficient.parse("(8-3*d)/(8*m2)").unwrap())
        );
        b4_dot_sum.add_scaled(&reduction, &coefficient.one());
    }
    assert_eq!(
        b4_dot_sum,
        rustred_legacy_oracles::LinearCombination::from_term(
            pipeline.masters()[2].clone(),
            coefficient.parse("(8-3*d)/(2*m2)").unwrap(),
        )
    );

    let mut f5_dot_sum = rustred_legacy_oracles::LinearCombination::new();
    for position in 0..5 {
        let mut powers = [1, 1, 1, 1, 1, 0];
        powers[position] = 2;
        let reduction = pipeline.reduce_integral(&Integral::from(powers)).unwrap();
        let expected_f5 = if position == 0 {
            coefficient.parse("(6-d)/(6*m2)").unwrap()
        } else {
            coefficient.parse("(3-d)/(3*m2)").unwrap()
        };
        assert_eq!(
            reduction.coefficient(&pipeline.masters()[3]),
            Some(&expected_f5)
        );
        f5_dot_sum.add_scaled(&reduction, &coefficient.one());
    }
    assert_eq!(
        f5_dot_sum,
        rustred_legacy_oracles::LinearCombination::from_term(
            pipeline.masters()[3].clone(),
            coefficient.parse("(10-3*d)/(2*m2)").unwrap(),
        )
    );

    let mut m6_dot_sum = rustred_legacy_oracles::LinearCombination::new();
    for position in 0..6 {
        let mut powers = [1; 6];
        powers[position] = 2;
        let reduction = pipeline.reduce_integral(&Integral::from(powers)).unwrap();
        assert_eq!(
            reduction.coefficient(&pipeline.masters()[4]),
            Some(&coefficient.parse("(4-d)/(4*m2)").unwrap())
        );
        m6_dot_sum.add_scaled(&reduction, &coefficient.one());
    }
    assert_eq!(
        m6_dot_sum,
        rustred_legacy_oracles::LinearCombination::from_term(
            pipeline.masters()[4].clone(),
            coefficient.parse("3*(4-d)/(2*m2)").unwrap(),
        )
    );

    // Exhaust every labelled exponent vector in the advertised box, including
    // all S4 images, disconnected zero sectors, dots, and numerator positions.
    for encoded in 0_u32..4_u32.pow(6) {
        let mut value = encoded;
        let powers: Vec<i32> = (0..6)
            .map(|_| {
                let power = [-1_i32, 0, 1, 2][(value % 4) as usize];
                value /= 4;
                power
            })
            .collect::<Vec<_>>();
        let dots: u32 = powers
            .iter()
            .map(|&power| u32::try_from((power - 1).max(0)).unwrap())
            .sum();
        let numerators: u32 = powers
            .iter()
            .map(|&power| if power < 0 { power.unsigned_abs() } else { 0 })
            .sum();
        if dots > 1 || numerators > 1 {
            continue;
        }
        let target = Integral::new(powers);
        let reduction = pipeline.reduce_integral(&target).unwrap();
        assert!(
            reduction
                .terms()
                .keys()
                .all(|integral| pipeline.masters().contains(integral)),
            "unregistered terminal while reducing {target}: {reduction:?}"
        );
    }

    // The public certificate is honest about its finite domain.
    let dotted = Integral::from([3, 1, 1, 1, 1, 1]);
    assert!(matches!(
        pipeline.reduce_integral(&dotted),
        Err(ThreeLoopPipelineError::OutOfCoverage {
            integral,
            dots: 2,
            numerator_degree: 0,
            max_dots: 1,
            max_numerator_degree: 1,
        }) if integral == dotted
    ));
    let numerator = Integral::from([-2, 1, 1, 1, 1, 1]);
    assert!(matches!(
        pipeline.reduce_integral(&numerator),
        Err(ThreeLoopPipelineError::OutOfCoverage {
            integral,
            numerator_degree: 2,
            max_numerator_degree: 1,
            ..
        }) if integral == numerator
    ));

    let seed_limited = ThreeLoopReductionConfig {
        max_seed_candidates: 1,
        ..ThreeLoopReductionConfig::default()
    };
    assert!(matches!(
        ThreeLoopReductionPipeline::build(seed_limited),
        Err(ThreeLoopPipelineError::SeedGeneration(
            SeedGenerationError::CandidateLimitExceeded { limit: 1, .. }
        ))
    ));

    let insufficient_halo = ThreeLoopReductionConfig {
        max_two_loop_dots: 1,
        ..ThreeLoopReductionConfig::default()
    };
    assert!(matches!(
        ThreeLoopReductionPipeline::build(insufficient_halo),
        Err(ThreeLoopPipelineError::ResourceLimit {
            resource: "induced two-loop dot coverage",
            requested: 2,
            limit: 1,
        })
    ));

    // Halo arithmetic is checked in a wider integer before any nested
    // two-loop construction.  The induced dot box must itself remain a valid
    // two-loop seed box, and the numerator +1 must not saturate silently.
    let unrepresentable_dot_halo = ThreeLoopReductionConfig {
        max_dots: i32::MAX as u32 - 2,
        max_two_loop_dots: i32::MAX as u32 - 1,
        ..ThreeLoopReductionConfig::default()
    };
    assert!(matches!(
        ThreeLoopReductionPipeline::build(unrepresentable_dot_halo),
        Err(ThreeLoopPipelineError::ResourceLimit {
            resource: "induced two-loop dot coverage",
            requested: 2_147_483_646,
            limit: 2_147_483_645,
        })
    ));
    let unrepresentable_numerator_halo = ThreeLoopReductionConfig {
        max_numerator_degree: i32::MAX as u32,
        ..ThreeLoopReductionConfig::default()
    };
    assert!(matches!(
        ThreeLoopReductionPipeline::build(unrepresentable_numerator_halo),
        Err(ThreeLoopPipelineError::ResourceLimit {
            resource: "induced two-loop numerator coverage",
            requested: 2_147_483_648,
            limit: 2_147_483_647,
        })
    ));

    // A topology-specific pipeline authenticates its routing/sign contract
    // before target enumeration.  The deliberately tiny seed limit would
    // otherwise mask this reversed-line mismatch.
    let coefficients = CoefficientContext::new(["d", "m2"]);
    let mass = coefficients.parameter("m2").unwrap();
    let denominators = THREE_LOOP_TETRAHEDRON_ROUTINGS
        .iter()
        .enumerate()
        .map(|(position, routing)| {
            let momentum = routing
                .iter()
                .map(|&component| ExactRational::from(i64::from(component)))
                .collect();
            if position == 0 {
                Denominator::reversed_propagator(momentum, mass.clone())
            } else {
                Denominator::propagator(momentum, mass.clone())
            }
        })
        .collect();
    let wrong_topology = VacuumFamily::new(
        "wrong_three_loop_sign",
        3,
        coefficients,
        "d",
        denominators,
        vec![],
    )
    .unwrap();
    let topology_vs_seed_limit = ThreeLoopReductionConfig {
        max_seed_candidates: 1,
        ..ThreeLoopReductionConfig::default()
    };
    assert!(matches!(
        ThreeLoopReductionPipeline::build_for_family(wrong_topology, topology_vs_seed_limit),
        Err(ThreeLoopPipelineError::Boundary(
            ThreeLoopBoundaryError::WrongPropagatorSign { position: 0 }
        ))
    ));

    // A raw top-sector row is accepted after canonicalization.  Conversely,
    // replacing a nonzero generated equation by algebraic zero must not pass
    // merely because its reduction remainder vanishes: public row metadata is
    // authenticated against the exact total-derivative generator first.
    let generator = IbpGenerator::new(pipeline.family());
    let seed = Integral::from([1, 1, 1, 1, 1, 1]);
    let raw = generator.generate_raw(&seed);
    pipeline.validate_identities(&raw).unwrap();
    let mut forged = generator
        .generate(&seed)
        .into_iter()
        .find(|identity| !identity.equation.is_zero())
        .unwrap();
    forged.equation = rustred_legacy_oracles::LinearCombination::new();
    assert!(matches!(
        pipeline.validate_identities(&[forged]),
        Err(ThreeLoopPipelineError::Reduction(
            ReductionError::IdentityEquationMismatch { .. }
        ))
    ));

    // Native tensor projection and denominator lowering are checked here. The
    // generic generated-provider composition and total replay path is covered
    // in `certified_three_loop_vakint_oracle.rs`; topology-specific authored
    // pipeline composition is intentionally absent from the core tensor-family
    // surface.
    let family = pipeline.family();
    let coefficients = family.coefficients();
    let mut projector =
        VacuumTensorProjector::with_dimension(coefficients, family.dimension().clone());
    let lowering = TensorFamilyReducer::new(family);
    let metric = |left, right| {
        MetricPairing::new([Metric::new(
            LorentzIndex::new(left),
            LorentzIndex::new(right),
        )])
    };

    for (base, loop_id, indices, pinched) in [
        (
            Integral::from([1, 1, 1, 0, 0, 0]),
            0,
            (100, 101),
            Integral::from([0, 1, 1, 0, 0, 0]),
        ),
        (
            Integral::from([1, 1, 1, 1, 0, 0]),
            1,
            (110, 111),
            Integral::from([1, 0, 1, 1, 0, 0]),
        ),
    ] {
        let projected = projector
            .reduce(&TensorMonomial::new([
                vector(loop_id, indices.0),
                vector(loop_id, indices.1),
            ]))
            .unwrap();
        let lowered = lowering.lower(&base, &projected).unwrap();
        let scalar = lowered.coefficient(&metric(indices.0, indices.1)).unwrap();
        assert_eq!(
            scalar.coefficient(&pinched),
            Some(&coefficients.parse("1/d").unwrap())
        );
        assert_eq!(
            scalar.coefficient(&base),
            Some(&coefficients.parse("-m2/d").unwrap())
        );
        assert_eq!(scalar.len(), 2);
    }

    // A mixed rank-two moment creates the exact signed-power tree numerator
    // before scalar reduction.
    let projected = projector
        .reduce(&TensorMonomial::new([vector(0, 120), vector(1, 121)]))
        .unwrap();
    let base = Integral::from([1, 1, 1, 0, 0, 0]);
    let lowered = lowering.lower(&base, &projected).unwrap();
    let scalar = lowered.coefficient(&metric(120, 121)).unwrap();
    for (integral, expected) in [
        ([0, 1, 1, 0, 0, 0], "1/(2*d)"),
        ([1, 0, 1, 0, 0, 0], "1/(2*d)"),
        ([1, 1, 1, 0, -1, 0], "-1/(2*d)"),
        ([1, 1, 1, 0, 0, 0], "-m2/(2*d)"),
    ] {
        assert_eq!(
            scalar.coefficient(&Integral::from(integral)),
            Some(&coefficients.parse(expected).unwrap())
        );
    }
    assert_eq!(scalar.len(), 4);
}

#[test]
fn certified_three_loop_three_dot_scalar_box() {
    let config = ThreeLoopReductionConfig {
        max_dots: 3,
        max_numerator_degree: 0,
        max_seed_candidates: 100_000,
        max_two_loop_dots: 4,
        ..ThreeLoopReductionConfig::default()
    };
    let pipeline = ThreeLoopReductionPipeline::build(config).unwrap();

    // This larger finite certificate contains the first genuinely nontrivial
    // dotted-B4 partitions.  Construction exhaustively validates every
    // symmetry-unique target and replays every native row used by elimination.
    assert_eq!(pipeline.config(), config);
    assert!(pipeline.stats().input_equations > 108);

    // Independently freeze the complete D=2 B4 orbit set. The four-cycle
    // stabilizer distinguishes adjacent from opposite double dots even though
    // the exact reductions of those two orbits coincide at generic d.
    let coefficients = pipeline.family().coefficients();
    let d2_b4 = [
        (
            Integral::from([3, 1, 0, 1, 0, 1]),
            "-3*(d-2)^3/(64*(d-4)*m2^3)",
            "(9*d^3-117*d^2+458*d-560)/(128*(d-4)*m2^2)",
        ),
        (
            Integral::from([2, 2, 0, 1, 0, 1]),
            "(d-2)^3/(32*(d-4)*m2^3)",
            "(9*d^3-81*d^2+242*d-240)/(64*(d-4)*m2^2)",
        ),
        (
            Integral::from([2, 1, 0, 1, 0, 2]),
            "(d-2)^3/(32*(d-4)*m2^3)",
            "(9*d^3-81*d^2+242*d-240)/(64*(d-4)*m2^2)",
        ),
    ];
    for (target, tadpole_cubed, b4) in &d2_b4 {
        let reduction = pipeline.reduce_integral(target).unwrap();
        assert_eq!(reduction.len(), 2);
        assert_eq!(
            reduction.coefficient(&pipeline.masters()[0]),
            Some(&coefficients.parse(tadpole_cubed).unwrap())
        );
        assert_eq!(
            reduction.coefficient(&pipeline.masters()[2]),
            Some(&coefficients.parse(b4).unwrap())
        );
    }

    // The scalar transfer identity gives one relation among the three D=2
    // orbits. The extra native numerator-coupled rows are what separate them.
    let mut transfer = rustred_legacy_oracles::LinearCombination::new();
    transfer.add_scaled(
        &pipeline.reduce_integral(&d2_b4[0].0).unwrap(),
        &coefficients.integer(2),
    );
    transfer.add_scaled(
        &pipeline.reduce_integral(&d2_b4[1].0).unwrap(),
        &coefficients.integer(2),
    );
    transfer.add_scaled(
        &pipeline.reduce_integral(&d2_b4[2].0).unwrap(),
        &coefficients.one(),
    );
    transfer.add_scaled(
        &pipeline
            .reduce_integral(&Integral::from([2, 1, 0, 1, 0, 1]))
            .unwrap(),
        &coefficients.parse("(3*d/2-5)/m2").unwrap(),
    );
    assert!(transfer.is_zero());

    for target in [
        Integral::from([3, 1, 0, 1, 0, 1]),
        Integral::from([2, 2, 0, 1, 0, 1]),
        Integral::from([2, 1, 0, 1, 0, 2]),
        Integral::from([4, 1, 0, 1, 0, 1]),
        Integral::from([3, 2, 0, 1, 0, 1]),
        Integral::from([3, 1, 1, 1, 1, 1]),
        Integral::from([2, 2, 1, 1, 1, 1]),
        Integral::from([4, 1, 1, 1, 1, 1]),
        Integral::from([3, 2, 1, 1, 1, 1]),
        Integral::from([2, 2, 2, 1, 1, 1]),
    ] {
        let reduction = pipeline.reduce_integral(&target).unwrap();
        assert!(
            reduction
                .terms()
                .keys()
                .all(|integral| pipeline.masters().contains(integral)),
            "unexpected terminal while reducing {target}: {reduction:?}"
        );
    }
}
