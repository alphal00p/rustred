use std::collections::{BTreeMap, BTreeSet};

use rustred::Integral;
use rustred_legacy_oracles::{
    FIVE_LOOP_BANANA_D3_ALGEBRAIC_CANDIDATE_BOUND, FIVE_LOOP_BANANA_D3_ALGEBRAIC_ROWS,
    FIVE_LOOP_BANANA_D3_COLLECTED_NONZERO_BOUND, FIVE_LOOP_BANANA_D3_ELIMINATION_UPDATE_BOUND,
    FIVE_LOOP_BANANA_D3_GLOBAL_COLUMNS, FIVE_LOOP_BANANA_D3_MOMENT_POWER_CLASSES,
    FIVE_LOOP_BANANA_D3_NATIVE_EXPANSION_BOUND, FIVE_LOOP_BANANA_D3_NATIVE_RAW_ORIGINS,
    FIVE_LOOP_BANANA_D3_NONZERO_RAW_ROWS, FIVE_LOOP_BANANA_D3_PROPER_BOUNDARY_ROWS,
    FIVE_LOOP_BANANA_D3_RANK, FIVE_LOOP_BANANA_D3_RAW_GRAPH_TERM_BOUND,
    FIVE_LOOP_BANANA_D3_SEED_ORBITS, FIVE_LOOP_BANANA_D3_SOURCE_WEIGHT_BOUND,
    FIVE_LOOP_BANANA_D3_SYMMETRY_IMAGE_BOUND, FiveLoopBananaD3Column,
    FiveLoopBananaD3ConditionSource, FiveLoopBananaD3Config, FiveLoopBananaD3Error,
    FiveLoopBananaD3RowId, FiveLoopBananaD3SeedOrbit, FiveLoopBananaD3Shell,
};

fn scalar(physical: [i32; 6]) -> Integral {
    let mut powers = vec![0; 15];
    powers[..6].copy_from_slice(&physical);
    Integral::new(powers)
}

fn all_permutations(mut values: [i32; 6]) -> Vec<[i32; 6]> {
    values.sort();
    let mut output = Vec::new();
    loop {
        output.push(values);
        let Some(left) = (0..5).rfind(|position| values[*position] < values[*position + 1]) else {
            return output;
        };
        let right = (left + 1..6)
            .rfind(|position| values[left] < values[*position])
            .unwrap();
        values.swap(left, right);
        values[left + 1..].reverse();
    }
}

#[test]
fn exact_five_loop_banana_d3_shell_replays_all_native_and_algebraic_rows() {
    // Every structural reservation is checked before family or coefficient
    // construction.  These failures are deliberately cheap.
    let cap_failures: [(&str, fn(&mut FiveLoopBananaD3Config)); 12] = [
        ("scalar seed orbits", |config| {
            config.max_seed_orbits = FIVE_LOOP_BANANA_D3_SEED_ORBITS - 1
        }),
        ("native raw origins", |config| {
            config.max_native_raw_origins = FIVE_LOOP_BANANA_D3_NATIVE_RAW_ORIGINS - 1
        }),
        ("raw graph terms", |config| {
            config.max_raw_graph_terms = FIVE_LOOP_BANANA_D3_RAW_GRAPH_TERM_BOUND - 1
        }),
        ("native expansion incidences", |config| {
            config.max_native_expansion_incidences = FIVE_LOOP_BANANA_D3_NATIVE_EXPANSION_BOUND - 1
        }),
        ("algebraic row candidates", |config| {
            config.max_algebraic_candidates = FIVE_LOOP_BANANA_D3_ALGEBRAIC_CANDIDATE_BOUND - 1
        }),
        ("algebraic rows", |config| {
            config.max_algebraic_rows = FIVE_LOOP_BANANA_D3_ALGEBRAIC_ROWS - 1
        }),
        ("proper-boundary rows", |config| {
            config.max_boundary_rows = FIVE_LOOP_BANANA_D3_PROPER_BOUNDARY_ROWS - 1
        }),
        ("joint S6 symmetry images", |config| {
            config.max_symmetry_images = FIVE_LOOP_BANANA_D3_SYMMETRY_IMAGE_BOUND - 1
        }),
        ("global columns", |config| {
            config.max_global_columns = FIVE_LOOP_BANANA_D3_GLOBAL_COLUMNS - 1
        }),
        ("collected normalized nonzeros", |config| {
            config.max_collected_nonzeros = FIVE_LOOP_BANANA_D3_COLLECTED_NONZERO_BOUND - 1
        }),
        ("elimination coefficient updates", |config| {
            config.max_elimination_updates = FIVE_LOOP_BANANA_D3_ELIMINATION_UPDATE_BOUND - 1
        }),
        ("source-row provenance weights", |config| {
            config.max_source_row_weights = FIVE_LOOP_BANANA_D3_SOURCE_WEIGHT_BOUND - 1
        }),
    ];
    for (resource, configure) in cap_failures {
        let mut config = FiveLoopBananaD3Config::default();
        configure(&mut config);
        assert!(matches!(
            FiveLoopBananaD3Shell::build(config),
            Err(FiveLoopBananaD3Error::ResourceLimit { resource: actual, .. })
                if actual == resource
        ));
    }

    let shell = FiveLoopBananaD3Shell::build(FiveLoopBananaD3Config::default()).unwrap();
    let context = shell.family().coefficients();

    assert_eq!(shell.seeds().len(), FIVE_LOOP_BANANA_D3_SEED_ORBITS);
    assert_eq!(
        shell
            .seeds()
            .iter()
            .map(|seed| seed.orbit())
            .collect::<Vec<_>>(),
        FiveLoopBananaD3SeedOrbit::ALL.to_vec()
    );
    assert_eq!(
        shell
            .seeds()
            .iter()
            .map(|seed| seed.labelled_orbit_size())
            .collect::<Vec<_>>(),
        [1, 6, 6, 15]
    );

    let native_ids = shell
        .normalized_rows()
        .iter()
        .filter_map(|row| match row.row_id() {
            FiveLoopBananaD3RowId::Native(id) => Some(id),
            _ => None,
        })
        .collect::<BTreeSet<_>>();
    let algebra_ids = shell
        .normalized_rows()
        .iter()
        .filter_map(|row| match row.row_id() {
            FiveLoopBananaD3RowId::Algebraic(id) => Some(id),
            _ => None,
        })
        .collect::<BTreeSet<_>>();
    let boundary_ids = shell
        .normalized_rows()
        .iter()
        .filter_map(|row| match row.row_id() {
            FiveLoopBananaD3RowId::Boundary(id) => Some(id),
            _ => None,
        })
        .collect::<BTreeSet<_>>();
    assert_eq!(native_ids.len(), FIVE_LOOP_BANANA_D3_NATIVE_RAW_ORIGINS);
    assert_eq!(algebra_ids.len(), FIVE_LOOP_BANANA_D3_ALGEBRAIC_ROWS);
    assert_eq!(boundary_ids.len(), FIVE_LOOP_BANANA_D3_PROPER_BOUNDARY_ROWS);
    assert_eq!(shell.boundary_closures().len(), boundary_ids.len());
    assert!(shell.boundary_closures().iter().all(|closure| {
        closure.reduction().len() == 1
            && closure
                .reduction()
                .coefficient(shell.boundary().product_master())
                .is_some()
    }));

    let columns = shell
        .normalized_rows()
        .iter()
        .flat_map(|row| row.entries().keys().cloned())
        .collect::<BTreeSet<_>>();
    assert_eq!(columns.len(), FIVE_LOOP_BANANA_D3_GLOBAL_COLUMNS);
    assert!(columns.contains(&FiveLoopBananaD3Column::BoundaryTerminal));
    assert!(
        columns
            .iter()
            .any(|column| { matches!(column, FiveLoopBananaD3Column::ProperBoundary { .. }) })
    );
    assert!(
        columns
            .iter()
            .any(|column| { matches!(column, FiveLoopBananaD3Column::OneMoment { .. }) })
    );
    assert!(columns.iter().all(|column| match column {
        FiveLoopBananaD3Column::BoundaryTerminal => true,
        FiveLoopBananaD3Column::ProperBoundary { powers } => {
            powers.iter().filter(|power| **power > 0).count() == 5
        }
        FiveLoopBananaD3Column::Scalar { powers }
        | FiveLoopBananaD3Column::OneMoment { powers, .. } => {
            powers.iter().all(|power| *power > 0)
        }
    }));

    assert_eq!(shell.rank(), FIVE_LOOP_BANANA_D3_RANK);
    assert_eq!(shell.free_columns().len(), 3);
    assert!(
        shell
            .free_columns()
            .contains(&FiveLoopBananaD3Column::BoundaryTerminal)
    );
    assert!(
        shell
            .free_columns()
            .contains(&FiveLoopBananaD3Column::Scalar {
                powers: [1, 1, 1, 1, 1, 1]
            })
    );
    assert!(
        shell
            .free_columns()
            .contains(&FiveLoopBananaD3Column::Scalar {
                powers: [2, 2, 1, 1, 1, 1]
            })
    );
    assert_eq!(shell.d2_candidate_terminal(), &scalar([2, 2, 1, 1, 1, 1]));

    let expected = [
        (
            [4, 1, 1, 1, 1, 1],
            "5*(11*d-50)/(72*m2)",
            "(-125*d^3+1225*d^2-3830*d+3864)/(864*m2^3)",
        ),
        (
            [3, 2, 1, 1, 1, 1],
            "(19*d-46)/(24*m2)",
            "(-50*d^3+385*d^2-986*d+840)/(288*m2^3)",
        ),
        (
            [2, 2, 2, 1, 1, 1],
            "(47-17*d)/(12*m2)",
            "(50*d^3-385*d^2+986*d-840)/(288*m2^3)",
        ),
    ];
    let master = scalar([1, 1, 1, 1, 1, 1]);
    let b2 = shell.d2_candidate_terminal();
    let mut labelled_targets = 0;
    for (representative, b2_coefficient, master_coefficient) in expected {
        for image in all_permutations(representative) {
            labelled_targets += 1;
            let reduction = shell.reduce_integral(&scalar(image)).unwrap();
            assert_eq!(reduction.len(), 2);
            assert_eq!(
                reduction.coefficient(b2),
                Some(&context.parse(b2_coefficient).unwrap())
            );
            assert_eq!(
                reduction.coefficient(&master),
                Some(&context.parse(master_coefficient).unwrap())
            );
        }
    }
    assert_eq!(labelled_targets, 56);

    // Independent single-scale derivative checks on the exact outputs.
    let a3 = shell.reduce_integral(&scalar([4, 1, 1, 1, 1, 1])).unwrap();
    let b3 = shell.reduce_integral(&scalar([3, 2, 1, 1, 1, 1])).unwrap();
    let c3 = shell.reduce_integral(&scalar([2, 2, 2, 1, 1, 1])).unwrap();
    assert_eq!(
        b3.coefficient(b2).unwrap() + c3.coefficient(b2).unwrap(),
        context.parse("(16-5*d)/(8*m2)").unwrap()
    );
    assert!((b3.coefficient(&master).unwrap() + c3.coefficient(&master).unwrap()).is_zero());
    let derivative_b2 = &(&context.scale_integer(a3.coefficient(b2).unwrap(), 3)
        + &context.scale_integer(b3.coefficient(b2).unwrap(), 15))
        + &context.scale_integer(c3.coefficient(b2).unwrap(), 10);
    assert!(derivative_b2.is_zero());
    let derivative_master = &(&context.scale_integer(a3.coefficient(&master).unwrap(), 3)
        + &context.scale_integer(b3.coefficient(&master).unwrap(), 15))
        + &context.scale_integer(c3.coefficient(&master).unwrap(), 10);
    assert_eq!(
        derivative_master,
        context
            .parse("(25*d^2-130*d+168)*(16-5*d)/(96*m2^3)")
            .unwrap()
    );

    assert_eq!(shell.stats().seed_orbits, FIVE_LOOP_BANANA_D3_SEED_ORBITS);
    assert_eq!(
        shell.stats().native_raw_origins,
        FIVE_LOOP_BANANA_D3_NATIVE_RAW_ORIGINS
    );
    assert_eq!(
        shell.stats().nonzero_native_rows,
        FIVE_LOOP_BANANA_D3_NONZERO_RAW_ROWS
    );
    assert_eq!(
        shell.stats().algebraic_rows,
        FIVE_LOOP_BANANA_D3_ALGEBRAIC_ROWS
    );
    assert_eq!(
        shell.stats().moment_power_classes,
        FIVE_LOOP_BANANA_D3_MOMENT_POWER_CLASSES
    );
    assert_eq!(
        shell.stats().algebraic_candidates,
        FIVE_LOOP_BANANA_D3_ALGEBRAIC_CANDIDATE_BOUND
    );
    assert_eq!(
        shell.stats().boundary_rows,
        FIVE_LOOP_BANANA_D3_PROPER_BOUNDARY_ROWS
    );
    assert_eq!(
        shell.stats().global_columns,
        FIVE_LOOP_BANANA_D3_GLOBAL_COLUMNS
    );

    assert!(matches!(
        shell.nonzero_conditions().first().unwrap().source(),
        FiveLoopBananaD3ConditionSource::GenericMassDomain
    ));
    assert!(
        shell
            .nonzero_conditions()
            .first()
            .unwrap()
            .polynomial()
            .contains("m2")
    );
    let row_ids = shell
        .normalized_rows()
        .iter()
        .map(|row| row.row_id())
        .collect::<BTreeSet<_>>();
    assert!(shell.pivots().iter().all(|rule| {
        !rule.source_row_weights().is_empty()
            && rule.rhs().keys().all(|column| column < rule.pivot())
            && rule
                .source_row_weights()
                .keys()
                .all(|row_id| row_ids.contains(row_id))
    }));

    for accepted in [
        FiveLoopBananaD3Column::BoundaryTerminal,
        FiveLoopBananaD3Column::Scalar {
            powers: [1, 1, 1, 1, 1, 1],
        },
        FiveLoopBananaD3Column::Scalar {
            powers: [2, 2, 1, 1, 1, 1],
        },
    ] {
        assert_eq!(
            shell.reduce_column(&accepted).unwrap(),
            BTreeMap::from([(accepted, context.one())])
        );
    }
    for unsupported in [
        FiveLoopBananaD3Column::Scalar {
            powers: [5, 1, 1, 1, 1, 1],
        },
        FiveLoopBananaD3Column::OneMoment {
            powers: [1, 1, 1, 1, 1, 1],
            edge: [0, 1],
        },
        FiveLoopBananaD3Column::ProperBoundary {
            powers: [5, 1, 1, 1, 1, 0],
        },
    ] {
        assert!(matches!(
            shell.reduce_column(&unsupported),
            Err(FiveLoopBananaD3Error::ColumnOutsideCertifiedShell { column })
                if column == unsupported
        ));
    }

    // Construction already performs a full replay.  This explicit call
    // regenerates the 100 native identities, all algebra rows, and every
    // boundary witness once more before checking source-row combinations.
    shell.replay().unwrap();

    assert!(matches!(
        shell.reduce_integral(&scalar([5, 1, 1, 1, 1, 1])),
        Err(FiveLoopBananaD3Error::OutOfCoverage {
            dot_degree: 4,
            maximum: 3,
            ..
        })
    ));
    let mut numerator_powers = [0; 15];
    numerator_powers[..6].fill(1);
    numerator_powers[6] = -1;
    let numerator = Integral::from(numerator_powers);
    assert!(matches!(
        shell.reduce_integral(&numerator),
        Err(FiveLoopBananaD3Error::NonScalarInput {
            position: 6,
            power: -1,
        })
    ));
}
