#![cfg(feature = "legacy-authored-oracles")]

use std::collections::BTreeSet;

use rustred::{
    THREE_LOOP_B4_D2_BOUNDARY_CALL_BOUND, THREE_LOOP_B4_D2_COLLECTED_NONZERO_BOUND,
    THREE_LOOP_B4_D2_ELIMINATION_UPDATE_BOUND, THREE_LOOP_B4_D2_GLOBAL_COLUMN_BOUND,
    THREE_LOOP_B4_D2_RAW_ROWS, THREE_LOOP_B4_D2_RAW_TERM_INCIDENCE_BOUND,
    THREE_LOOP_B4_D2_SEED_ORBITS, THREE_LOOP_B4_D2_SOURCE_WEIGHT_BOUND,
    THREE_LOOP_B4_D2_SYMMETRY_IMAGE_BOUND, ThreeLoopB4BoundaryColumn, ThreeLoopB4D2Column,
    ThreeLoopB4D2Config, ThreeLoopB4D2Error, ThreeLoopB4D2RawRowId, ThreeLoopB4D2SeedOrbit,
    ThreeLoopB4D2Shell, ThreeLoopB4D2Stats,
};

#[test]
fn native_b4_d2_shell_closes_all_three_scalar_orbits_and_replays() {
    // Every structural bound fails before family, boundary-table, or
    // coefficient construction when set below the exact reservation.
    let cap_failures: [(&str, fn(&mut ThreeLoopB4D2Config)); 9] = [
        ("scalar seed orbits", |config: &mut ThreeLoopB4D2Config| {
            config.max_seed_orbits = THREE_LOOP_B4_D2_SEED_ORBITS - 1
        }),
        ("raw rows", |config: &mut ThreeLoopB4D2Config| {
            config.max_raw_rows = THREE_LOOP_B4_D2_RAW_ROWS - 1
        }),
        ("raw term incidences", |config: &mut ThreeLoopB4D2Config| {
            config.max_raw_term_incidences = THREE_LOOP_B4_D2_RAW_TERM_INCIDENCE_BOUND - 1
        }),
        (
            "boundary normalization calls",
            |config: &mut ThreeLoopB4D2Config| {
                config.max_boundary_calls = THREE_LOOP_B4_D2_BOUNDARY_CALL_BOUND - 1
            },
        ),
        ("B4 symmetry images", |config: &mut ThreeLoopB4D2Config| {
            config.max_symmetry_images = THREE_LOOP_B4_D2_SYMMETRY_IMAGE_BOUND - 1
        }),
        ("global columns", |config: &mut ThreeLoopB4D2Config| {
            config.max_global_columns = THREE_LOOP_B4_D2_GLOBAL_COLUMN_BOUND - 1
        }),
        (
            "collected normalized nonzeros",
            |config: &mut ThreeLoopB4D2Config| {
                config.max_collected_nonzeros = THREE_LOOP_B4_D2_COLLECTED_NONZERO_BOUND - 1
            },
        ),
        (
            "elimination coefficient updates",
            |config: &mut ThreeLoopB4D2Config| {
                config.max_elimination_updates = THREE_LOOP_B4_D2_ELIMINATION_UPDATE_BOUND - 1
            },
        ),
        (
            "source-row provenance weights",
            |config: &mut ThreeLoopB4D2Config| {
                config.max_source_row_weights = THREE_LOOP_B4_D2_SOURCE_WEIGHT_BOUND - 1
            },
        ),
    ];
    for (resource, configure) in cap_failures {
        let mut config = ThreeLoopB4D2Config::default();
        configure(&mut config);
        assert!(matches!(
            ThreeLoopB4D2Shell::build(config),
            Err(ThreeLoopB4D2Error::ResourceLimit {
                resource: actual,
                ..
            }) if actual == resource
        ));
    }

    let mut low_degree = ThreeLoopB4D2Config::default();
    low_degree.max_coefficient_degree = 0;
    assert!(matches!(
        ThreeLoopB4D2Shell::build(low_degree),
        Err(ThreeLoopB4D2Error::ResourceLimit {
            resource: "configured coefficient exponent degree",
            ..
        })
    ));

    let shell = ThreeLoopB4D2Shell::build(ThreeLoopB4D2Config::default()).unwrap();

    assert_eq!(shell.seeds().len(), THREE_LOOP_B4_D2_SEED_ORBITS);
    assert_eq!(
        shell
            .seeds()
            .iter()
            .map(|seed| seed.orbit())
            .collect::<Vec<_>>(),
        ThreeLoopB4D2SeedOrbit::ALL.to_vec()
    );
    assert_eq!(shell.normalized_rows().len(), THREE_LOOP_B4_D2_RAW_ROWS);
    assert_eq!(
        shell.stats(),
        ThreeLoopB4D2Stats {
            raw_rows: 45,
            raw_term_incidences: 311,
            boundary_calls: 62,
            symmetry_images: 5_976,
            collected_nonzeros: 236,
            elimination_updates: 1_014,
            source_row_weights: 55,
        }
    );

    let expected_ids = ThreeLoopB4D2SeedOrbit::ALL
        .into_iter()
        .flat_map(|orbit| {
            (0_u8..3).flat_map(move |differentiated| {
                (0_u8..3).map(move |contracted| {
                    ThreeLoopB4D2RawRowId::new(orbit, differentiated, contracted)
                })
            })
        })
        .collect::<BTreeSet<_>>();
    assert_eq!(
        shell
            .normalized_rows()
            .iter()
            .map(|row| row.raw_id())
            .collect::<BTreeSet<_>>(),
        expected_ids
    );

    // The bookkeeping universe is explicitly disjoint.  The complete
    // one-step numerator halo and exact lower boundary must both actually be
    // present, not silently omitted during scalar projection.
    let columns = shell
        .normalized_rows()
        .iter()
        .flat_map(|row| row.entries().keys().cloned())
        .collect::<BTreeSet<_>>();
    let boundary_columns = columns
        .iter()
        .filter_map(|column| match column {
            ThreeLoopB4D2Column::Boundary(boundary) => Some(*boundary),
            _ => None,
        })
        .collect::<BTreeSet<_>>();
    assert_eq!(
        boundary_columns,
        BTreeSet::from([ThreeLoopB4BoundaryColumn::TadpoleCubed])
    );
    // A B4 raw row can pinch one of its four active cycle lines, leaving a
    // three-line tree.  It cannot activate an inactive line, so the paw
    // terminal T1*S2 is deliberately absent from this exact boundary census.
    assert!(!columns.contains(&ThreeLoopB4D2Column::Boundary(
        ThreeLoopB4BoundaryColumn::TadpoleSunset,
    )));
    assert!(
        columns
            .iter()
            .any(|column| matches!(column, ThreeLoopB4D2Column::Numerator { .. }))
    );
    assert!(columns.iter().all(|column| match column {
        ThreeLoopB4D2Column::Boundary(_) => true,
        ThreeLoopB4D2Column::Scalar { powers } => powers.iter().all(|power| *power > 0),
        ThreeLoopB4D2Column::Numerator { powers } => {
            [0, 1, 3, 5].iter().all(|position| powers[*position] > 0)
                && [2, 4].iter().all(|position| powers[*position] <= 0)
                && column.numerator_degree() == 1
        }
    }));

    let [target_a, target_adjacent, target_opposite] = shell.target_columns();
    assert_eq!(
        [target_a, target_adjacent, target_opposite]
            .into_iter()
            .collect::<BTreeSet<_>>()
            .len(),
        3
    );
    let pivot_columns = shell
        .pivots()
        .iter()
        .map(|rule| rule.pivot().clone())
        .collect::<BTreeSet<_>>();
    for target in [target_a, target_adjacent, target_opposite] {
        assert!(pivot_columns.contains(target));
        let reduction = shell.reduce_column(target).unwrap();
        assert!(!reduction.contains_key(target));
        assert!(
            reduction
                .keys()
                .all(|column| { shell.free_columns().contains(column) && column < target })
        );
    }
    assert_eq!(shell.rank(), 18);
    assert_eq!(
        shell.free_columns(),
        &[
            ThreeLoopB4D2Column::Boundary(ThreeLoopB4BoundaryColumn::TadpoleCubed),
            ThreeLoopB4D2Column::Scalar {
                powers: [1, 1, 1, 1],
            },
        ]
    );

    // Exhaust every proved tetrahedron symmetry image of all three D=2
    // orbits. Full power vectors move together, which is also the convention
    // used for the numerator halo inside the shell.
    for orbit in [
        ThreeLoopB4D2SeedOrbit::TripleDot,
        ThreeLoopB4D2SeedOrbit::AdjacentDoubleDot,
        ThreeLoopB4D2SeedOrbit::OppositeDoubleDot,
    ] {
        let representative = shell
            .seeds()
            .iter()
            .find(|seed| seed.orbit() == orbit)
            .unwrap()
            .integral();
        let expected = shell.reduce_target(representative).unwrap();
        let images = shell
            .family()
            .symmetries()
            .iter()
            .map(|permutation| {
                rustred::Integral::new(
                    permutation
                        .iter()
                        .map(|source| representative.powers()[*source])
                        .collect::<Vec<_>>(),
                )
            })
            .collect::<BTreeSet<_>>();
        assert!(!images.is_empty());
        for image in images {
            assert_eq!(shell.reduce_target(&image).unwrap(), expected);
        }
    }

    // Pivot divisions are generic-Q(d) statements.  Both recorded division
    // events have exactly the one exceptional locus d=4 (possibly with a
    // different nonzero rational normalization), while mass normalization
    // guarantees that neither condition contains m2.
    assert_eq!(shell.nonzero_conditions().len(), 2);
    let coefficients = shell.family().coefficients();
    let d_minus_four = coefficients.parse("d-4").unwrap();
    for condition in shell.nonzero_conditions() {
        assert!(!condition.polynomial().contains("m2"));
        let polynomial = coefficients.parse(condition.polynomial()).unwrap();
        assert!(!polynomial.is_zero());
        let proportionality = &polynomial / &d_minus_four;
        for position in 0..2 {
            assert_eq!(proportionality.numerator.degree(position), 0);
            assert_eq!(proportionality.denominator.degree(position), 0);
        }
    }

    let normalized_ids = shell
        .normalized_rows()
        .iter()
        .map(|row| row.raw_id())
        .collect::<BTreeSet<_>>();
    assert!(shell.pivots().iter().all(|rule| {
        !rule.source_row_weights().is_empty()
            && rule.rhs().keys().all(|column| column < rule.pivot())
            && rule
                .source_row_weights()
                .keys()
                .all(|raw_id| normalized_ids.contains(raw_id))
    }));

    // Construction performs this exact replay already; the public rerun
    // independently regenerates all 45 native rows and checks every stored
    // source-row combination.
    shell.replay().unwrap();
}
