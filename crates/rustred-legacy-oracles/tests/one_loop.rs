use rustred::{CoefficientContext, CoefficientContextError};
use rustred_legacy_oracles::FamilyError;
use rustred_legacy_oracles::{
    OneLoopTadpoleConfig, OneLoopTadpoleError, OneLoopTadpoleReducer,
    equal_mass_two_loop_vacuum_in_context,
};

fn assert_resource(config: OneLoopTadpoleConfig, power: i32, expected_resource: &'static str) {
    let reducer = OneLoopTadpoleReducer::build(config).unwrap();
    assert!(matches!(
        reducer.preflight(power),
        Err(OneLoopTadpoleError::ResourceLimit { resource, .. })
            if resource == expected_resource
    ));
}

#[test]
fn exact_tadpole_recurrence_replays_in_the_callers_context() {
    let context = CoefficientContext::new(["d", "m2"]);
    let reducer =
        OneLoopTadpoleReducer::new(context.clone(), "d", "m2", OneLoopTadpoleConfig::default())
            .unwrap();

    for power in [-3, 0] {
        let reduction = reducer.reduce_power(power).unwrap();
        assert!(reduction.coefficient().is_zero());
        assert_eq!(reduction.stats().recurrence_steps(), 0);
        reducer.replay(&reduction).unwrap();
    }
    let unit = reducer.reduce_power(1).unwrap();
    assert_eq!(unit.coefficient(), &context.one());
    reducer.replay(&unit).unwrap();

    let squared = reducer.reduce_power(2).unwrap();
    assert_eq!(
        squared.coefficient(),
        &context.parse("(2-d)/(2*m2)").unwrap()
    );
    assert_eq!(squared.stats().recurrence_steps(), 1);
    assert_eq!(squared.stats().coefficient_operations(), 4);
    assert_eq!(squared.stats().dense_term_operation_bound(), 8);
    assert_eq!(squared.stats().coefficient_degree_bound(), 1);
    reducer.replay(&squared).unwrap();

    let cubed = reducer.reduce_power(3).unwrap();
    assert_eq!(
        cubed.coefficient(),
        &context.parse("(2-d)*(4-d)/(8*m2^2)").unwrap()
    );
    assert_eq!(cubed.stats().recurrence_steps(), 2);
    assert_eq!(cubed.stats().coefficient_operations(), 8);
    assert_eq!(cubed.stats().dense_term_operation_bound(), 24);
    assert_eq!(cubed.stats().coefficient_degree_bound(), 2);
    reducer.replay(&cubed).unwrap();

    assert!(
        reducer
            .replay(&cubed.with_coefficient_for_replay(cubed.coefficient() + &context.one()))
            .is_err()
    );
    assert!(reducer.replay(&cubed.with_power_for_replay(2)).is_err());
    assert!(
        reducer
            .replay(&cubed.with_coefficient_operations_for_replay(7))
            .is_err()
    );

    let family = equal_mass_two_loop_vacuum_in_context(context.clone()).unwrap();
    assert!(family.coefficients().has_same_variable_map(&context));
    assert_eq!(
        family.denominators()[0].shift(),
        context.parameter("m2").as_ref().unwrap()
    );

    check_tadpole_preflight_resources();
}

fn check_tadpole_preflight_resources() {
    let defaults = OneLoopTadpoleConfig::default();

    assert_resource(
        OneLoopTadpoleConfig {
            max_recurrence_steps: 1,
            ..defaults
        },
        3,
        "tadpole recurrence steps",
    );
    assert_resource(
        OneLoopTadpoleConfig {
            max_coefficient_operations: 7,
            ..defaults
        },
        3,
        "tadpole coefficient operations",
    );
    assert_resource(
        OneLoopTadpoleConfig {
            max_dense_term_operations: 23,
            ..defaults
        },
        3,
        "tadpole dense term operations",
    );
    assert_resource(
        OneLoopTadpoleConfig {
            max_coefficient_degree: 1,
            ..defaults
        },
        3,
        "tadpole coefficient degree",
    );

    let boundary = OneLoopTadpoleReducer::build(defaults).unwrap();
    let stats = boundary.preflight(257).unwrap();
    assert_eq!(stats.recurrence_steps(), 256);
    assert_eq!(stats.coefficient_operations(), 1_024);
    assert_eq!(stats.dense_term_operation_bound(), 263_168);
    assert_eq!(stats.coefficient_degree_bound(), 256);
    assert!(matches!(
        boundary.preflight(258),
        Err(OneLoopTadpoleError::ResourceLimit {
            resource: "tadpole recurrence steps",
            requested: 257,
            limit: 256,
        })
    ));
    assert!(matches!(
        boundary.preflight(i32::MAX),
        Err(OneLoopTadpoleError::ResourceLimit { .. })
    ));
    let dense_exact = OneLoopTadpoleReducer::build(OneLoopTadpoleConfig {
        max_dense_term_operations: 48,
        ..defaults
    })
    .unwrap();
    assert_eq!(
        dense_exact
            .preflight(4)
            .unwrap()
            .dense_term_operation_bound(),
        48
    );
    assert_resource(
        OneLoopTadpoleConfig {
            max_dense_term_operations: 47,
            ..defaults
        },
        4,
        "tadpole dense term operations",
    );
    let degree_guard = OneLoopTadpoleReducer::build(OneLoopTadpoleConfig {
        max_recurrence_steps: 65_536,
        max_coefficient_operations: 262_144,
        max_dense_term_operations: 20_000_000_000,
        ..defaults
    })
    .unwrap();
    assert!(matches!(
        degree_guard.preflight(65_537),
        Err(OneLoopTadpoleError::ResourceLimit {
            resource: "tadpole coefficient degree",
            requested: 65_536,
            limit: 65_535,
        })
    ));

    assert!(matches!(
        OneLoopTadpoleReducer::new(
            CoefficientContext::new(["d"]),
            "d",
            "m2",
            defaults,
        ),
        Err(OneLoopTadpoleError::MissingParameter { name }) if name == "m2"
    ));
    assert!(matches!(
        OneLoopTadpoleReducer::new(
            CoefficientContext::new(["d", "m2"]),
            "d",
            "d",
            defaults,
        ),
        Err(OneLoopTadpoleError::ParameterAlias { name }) if name == "d"
    ));
    assert!(matches!(
        CoefficientContext::try_new(["d", "d", "m2"]),
        Err(CoefficientContextError::DuplicateParameter(name)) if name == "d"
    ));
    assert!(matches!(
        CoefficientContext::try_new(["d", "bad name"]),
        Err(CoefficientContextError::InvalidParameter { name, .. }) if name == "bad name"
    ));

    assert!(matches!(
        equal_mass_two_loop_vacuum_in_context(CoefficientContext::new(["d", "mu2"])),
        Err(FamilyError::UnknownCoefficientParameter(name)) if name == "m2"
    ));
}
