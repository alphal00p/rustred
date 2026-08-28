use rustred::{Integral, MasterProduct};
use rustred_legacy_oracles::{
    FOUR_LOOP_BOUNDARY_HALO_BLOCKER_OCCURRENCES, FOUR_LOOP_BOUNDARY_HALO_FORMULA_DISPATCHES,
    FOUR_LOOP_BOUNDARY_HALO_OUTPUT_PRODUCTS, FOUR_LOOP_BOUNDARY_HALO_PRECOLLECTION_TERMS,
    FOUR_LOOP_BOUNDARY_HALO_PRODUCT_MULTIPLICATIONS,
    FOUR_LOOP_BOUNDARY_HALO_SIGNED_LINE_DISPATCHES, FOUR_LOOP_BOUNDARY_HALO_UNIQUE_WITNESS_PLANS,
    FourLoopBoundaryConfig, FourLoopBoundaryHaloConfig, FourLoopBoundaryHaloError,
    FourLoopBoundaryHaloReducer, FourLoopBoundaryHaloStats, FourLoopBoundaryReducer,
    FourLoopScalarClass, FourLoopTopology,
};

fn corner(mask: u16) -> Integral {
    Integral::from(std::array::from_fn::<_, 10, _>(|position| {
        i32::from(mask & (1_u16 << position) != 0)
    }))
}

#[test]
fn direct_four_loop_factorized_d1n0_halo_is_guarded() {
    let boundary =
        FourLoopBoundaryReducer::build(FourLoopTopology::H, FourLoopBoundaryConfig::default())
            .unwrap();
    let FourLoopScalarClass::Factorized { product, witness } =
        boundary.classify_integral(&corner(43)).unwrap()
    else {
        panic!("H mask 43 must be the frozen four-tadpole product")
    };
    let reducer = FourLoopBoundaryHaloReducer::new(boundary, Default::default()).unwrap();
    reducer.preflight_formula_table().unwrap();

    let mut dotted = corner(43).powers().to_vec();
    let dot = dotted.iter().position(|power| *power == 1).unwrap();
    dotted[dot] = 2;
    let dotted = Integral::new(dotted);
    let reduction = reducer
        .reduce_integral(&dotted, &product, &witness)
        .unwrap();
    assert_eq!(
        reduction.dotted_component(),
        rustred_legacy_oracles::MassiveVacuumMaster::T1
    );
    assert_eq!(reduction.compact_reference_position(), 0);
    assert_eq!(reduction.ordinary().len(), 1);
    assert_eq!(reduction.mass_normalized().len(), 1);
    let context = reducer.boundary().family().coefficients();
    assert_eq!(
        reduction.mass_normalized().coefficient(&product),
        Some(&context.parse("(2-d)/2").unwrap())
    );

    assert!(matches!(
        reducer.reduce_integral(&corner(43), &product, &witness),
        Err(FourLoopBoundaryHaloError::OutsideD1N0 {
            dots: 0,
            numerator_degree: 0,
        })
    ));
    assert!(matches!(
        reducer.reduce_integral(&dotted, &MasterProduct::identity(), &witness),
        Err(FourLoopBoundaryHaloError::ProductOutsideClosure { .. })
    ));

    let limits = FourLoopBoundaryHaloConfig::default();
    for (resource, request) in [
        (
            "blocker occurrences",
            FourLoopBoundaryHaloStats::new(
                FOUR_LOOP_BOUNDARY_HALO_BLOCKER_OCCURRENCES + 1,
                0,
                0,
                0,
                0,
                0,
                0,
            ),
        ),
        (
            "unique witness plans",
            FourLoopBoundaryHaloStats::new(
                0,
                FOUR_LOOP_BOUNDARY_HALO_UNIQUE_WITNESS_PLANS + 1,
                0,
                0,
                0,
                0,
                0,
            ),
        ),
        (
            "signed-line dispatches",
            FourLoopBoundaryHaloStats::new(
                0,
                0,
                FOUR_LOOP_BOUNDARY_HALO_SIGNED_LINE_DISPATCHES + 1,
                0,
                0,
                0,
                0,
            ),
        ),
        (
            "formula dispatches",
            FourLoopBoundaryHaloStats::new(
                0,
                0,
                0,
                FOUR_LOOP_BOUNDARY_HALO_FORMULA_DISPATCHES + 1,
                0,
                0,
                0,
            ),
        ),
        (
            "product multiplications",
            FourLoopBoundaryHaloStats::new(
                0,
                0,
                0,
                0,
                FOUR_LOOP_BOUNDARY_HALO_PRODUCT_MULTIPLICATIONS + 1,
                0,
                0,
            ),
        ),
        (
            "precollection terms",
            FourLoopBoundaryHaloStats::new(
                0,
                0,
                0,
                0,
                0,
                FOUR_LOOP_BOUNDARY_HALO_PRECOLLECTION_TERMS + 1,
                0,
            ),
        ),
        (
            "output products",
            FourLoopBoundaryHaloStats::new(
                0,
                0,
                0,
                0,
                0,
                0,
                FOUR_LOOP_BOUNDARY_HALO_OUTPUT_PRODUCTS + 1,
            ),
        ),
    ] {
        assert!(matches!(
            reducer.preflight_stats(request),
            Err(FourLoopBoundaryHaloError::ResourceLimit {
                resource: actual,
                requested: _,
                limit: _,
            }) if actual == resource
        ));
    }
    assert_eq!(reducer.config(), limits);

    let low_degree = FourLoopBoundaryHaloReducer::build(FourLoopBoundaryHaloConfig {
        max_coefficient_degree: 1,
        ..Default::default()
    })
    .unwrap();
    assert!(matches!(
        low_degree.preflight_formula_table(),
        Err(FourLoopBoundaryHaloError::ResourceLimit {
            resource: "configured coefficient exponent degree",
            requested: 2,
            limit: 1,
        })
    ));
    assert!(matches!(
        low_degree.reduce_integral(&dotted, &product, &witness),
        Err(FourLoopBoundaryHaloError::ResourceLimit {
            resource: "configured coefficient exponent degree",
            requested: 2,
            limit: 1,
        })
    ));
}
