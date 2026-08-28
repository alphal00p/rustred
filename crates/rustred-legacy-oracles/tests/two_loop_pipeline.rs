use rustred::{IbpGenerator, Integral, SparseReducer};
use rustred_legacy_oracles::families::equal_mass_two_loop_vacuum;
use rustred_legacy_oracles::two_loop_pipeline::{
    TwoLoopPipelineError, TwoLoopReductionConfig, TwoLoopReductionPipeline,
};

fn coefficient_or_zero(
    pipeline: &TwoLoopReductionPipeline,
    reduction: &rustred::LinearCombination,
    master: &Integral,
) -> rustred::Coefficient {
    reduction
        .coefficient(master)
        .cloned()
        .unwrap_or_else(|| pipeline.family().coefficients().zero())
}

fn assert_two_master_reduction(
    pipeline: &TwoLoopReductionPipeline,
    powers: [i32; 3],
    sunset: &str,
    product: &str,
) {
    let reduction = pipeline.reduce_integral(&Integral::from(powers)).unwrap();
    let coefficients = pipeline.family().coefficients();
    assert_eq!(
        coefficient_or_zero(pipeline, &reduction, pipeline.sunset_master()),
        coefficients.parse(sunset).unwrap(),
        "wrong sunset coefficient for {powers:?}"
    );
    assert_eq!(
        coefficient_or_zero(pipeline, &reduction, pipeline.product_master()),
        coefficients.parse(product).unwrap(),
        "wrong product coefficient for {powers:?}"
    );
    assert!(reduction.terms().keys().all(|integral| {
        integral == pipeline.sunset_master() || integral == pipeline.product_master()
    }));
}

fn check_top_and_boundary_goldens(pipeline: &TwoLoopReductionPipeline) {
    assert_two_master_reduction(pipeline, [1, 1, 1], "1", "0");
    assert_two_master_reduction(pipeline, [2, 1, 1], "(3-d)/(3*m2)", "0");
    assert_two_master_reduction(
        pipeline,
        [2, 2, 1],
        "(d-2)*(d-3)/(9*m2^2)",
        "(d-2)^2/(12*m2^3)",
    );
    assert_two_master_reduction(
        pipeline,
        [3, 1, 1],
        "(d-8)*(d-3)/(18*m2^2)",
        "-(d-2)^2/(12*m2^3)",
    );

    // The sparse table canonicalizes this boundary to I(2,1,0); the
    // integrated surface always maps it back to the fixed P=I(0,1,1).
    assert_two_master_reduction(pipeline, [2, 1, 0], "0", "(2-d)/(2*m2)");
    assert_two_master_reduction(pipeline, [-2, 1, 1], "0", "m2^2*(d+4)/d");
    assert!(
        pipeline
            .reduce_integral(&Integral::from([-20, 0, 7]))
            .unwrap()
            .is_zero()
    );
}

fn check_complete_index_cube(pipeline: &TwoLoopReductionPipeline) {
    const PERMUTATIONS: [[usize; 3]; 6] = [
        [0, 1, 2],
        [0, 2, 1],
        [1, 0, 2],
        [1, 2, 0],
        [2, 0, 1],
        [2, 1, 0],
    ];

    let mut checked = 0;
    for first in -2..=4 {
        for second in -2..=4 {
            for third in -2..=4 {
                let powers = [first, second, third];
                let reference = pipeline.reduce_integral(&Integral::from(powers)).unwrap();
                assert!(reference.terms().keys().all(|integral| {
                    integral == pipeline.sunset_master() || integral == pipeline.product_master()
                }));

                for permutation in PERMUTATIONS {
                    let permuted = Integral::from([
                        powers[permutation[0]],
                        powers[permutation[1]],
                        powers[permutation[2]],
                    ]);
                    assert_eq!(
                        pipeline.reduce_integral(&permuted).unwrap(),
                        reference,
                        "permutation invariance failed for {powers:?} -> {permuted}"
                    );
                }
                checked += 1;
            }
        }
    }
    assert_eq!(checked, 7_usize.pow(3));
}

fn check_top_sector_ibp_box(pipeline: &TwoLoopReductionPipeline) {
    // A raw IBP raises the total dot degree by at most one.  Seeds with each
    // power in 1..=3 therefore produce terms of dot degree at most seven,
    // strictly inside the configured degree-nine coverage simplex.
    let mut seeds = Vec::with_capacity(27);
    for first in 1..=3 {
        for second in 1..=3 {
            for third in 1..=3 {
                seeds.push(Integral::from([first, second, third]));
            }
        }
    }
    assert_eq!(seeds.len(), 27);
    let generator = IbpGenerator::new(pipeline.family());
    let identities: Vec<_> = seeds
        .iter()
        .flat_map(|seed| generator.generate_raw(seed))
        .collect();
    pipeline.validate_identities(&identities).unwrap();
}

fn check_identity_provenance(pipeline: &TwoLoopReductionPipeline) {
    // Raw generator rows are canonically equivalent to the rows stored in the
    // table and remain valid at the integrated pipeline boundary.
    let generator = IbpGenerator::new(pipeline.family());
    let seed = Integral::from([1, 1, 1]);
    let raw = generator.generate_raw(&seed);
    pipeline.validate_identities(&raw).unwrap();

    // An empty equation has a trivially vanishing remainder, but it is not the
    // total derivative claimed by this public metadata.  It must fail the
    // generator-oracle check rather than being accepted as a certificate.
    let mut forged = generator
        .generate(&seed)
        .into_iter()
        .find(|identity| !identity.equation.is_zero())
        .unwrap();
    forged.equation = rustred::LinearCombination::new();
    assert!(matches!(
        pipeline.validate_identities(&[forged]),
        Err(TwoLoopPipelineError::Reduction(
            rustred::ReductionError::IdentityEquationMismatch { .. }
        ))
    ));
}

fn check_typed_coverage_and_resource_failures(pipeline: &TwoLoopReductionPipeline) {
    let outside = Integral::from([5, 4, 4]);
    assert!(matches!(
        pipeline.reduce_integral(&outside),
        Err(TwoLoopPipelineError::OutOfCoverage {
            integral,
            dots: 10,
            numerator_degree: 0,
            max_dots: 9,
            max_numerator_degree: 2,
        }) if integral == outside
    ));

    let outside_numerator = Integral::from([-3, 1, 1]);
    assert!(matches!(
        pipeline.reduce_integral(&outside_numerator),
        Err(TwoLoopPipelineError::OutOfCoverage {
            integral,
            dots: 0,
            numerator_degree: 3,
            max_dots: 9,
            max_numerator_degree: 2,
        }) if integral == outside_numerator
    ));

    let seed_limited = TwoLoopReductionConfig {
        max_seed_candidates: 1,
        ..TwoLoopReductionConfig::default()
    };
    assert!(matches!(
        TwoLoopReductionPipeline::build(seed_limited),
        Err(TwoLoopPipelineError::ResourceLimit {
            resource: "seed candidate upper bound",
            ..
        })
    ));

    let boundary_limited = TwoLoopReductionPipeline::from_table(
        pipeline.table().clone(),
        TwoLoopReductionConfig {
            max_dots: 0,
            max_numerator_degree: 2,
            max_seed_candidates: 100,
            max_boundary_terms: 1,
        },
    )
    .unwrap();
    assert!(matches!(
        boundary_limited.reduce_integral(&Integral::from([-2, 1, 1])),
        Err(TwoLoopPipelineError::ResourceLimit {
            resource: "boundary formula iteration estimate",
            ..
        })
    ));
}

fn check_missing_rules_are_not_masters() {
    let family = equal_mass_two_loop_vacuum().unwrap();
    let empty_table = SparseReducer::new(family).reduce(&[]).unwrap();
    let error = TwoLoopReductionPipeline::from_table(
        empty_table,
        TwoLoopReductionConfig {
            max_dots: 1,
            max_numerator_degree: 0,
            max_seed_candidates: 100,
            max_boundary_terms: 100,
        },
    )
    .unwrap_err();
    assert!(matches!(
        error,
        TwoLoopPipelineError::UnresolvedIntegral {
            requested,
            unresolved,
        } if requested == Integral::from([2, 1, 1])
            && unresolved == Integral::from([2, 1, 1])
    ));
}

// Restricted Symbolica must remain on one test worker, so the complete
// integration, coverage, and validation suite is grouped into one test.
#[test]
fn complete_equal_mass_two_loop_pipeline() {
    let pipeline = TwoLoopReductionPipeline::build(TwoLoopReductionConfig::default()).unwrap();
    assert_eq!(pipeline.sunset_master(), &Integral::from([1, 1, 1]));
    assert_eq!(pipeline.product_master(), &Integral::from([0, 1, 1]));
    assert!(pipeline.stats().rules > 0);

    check_top_and_boundary_goldens(&pipeline);
    check_complete_index_cube(&pipeline);
    check_top_sector_ibp_box(&pipeline);
    check_identity_provenance(&pipeline);
    check_typed_coverage_and_resource_failures(&pipeline);
    check_missing_rules_are_not_masters();
}
