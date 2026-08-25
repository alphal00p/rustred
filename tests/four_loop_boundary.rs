#![cfg(feature = "legacy-authored-oracles")]

use std::collections::{BTreeMap, BTreeSet};

use rustred::{
    CoefficientContext, Denominator, ExactRational, FourLoopBoundaryConfig, FourLoopBoundaryError,
    FourLoopBoundaryReducer, FourLoopScalarClass, FourLoopTopology, Integral, MassiveVacuumMaster,
    MasterProduct, VacuumFamily, equal_mass_four_loop_vacuum,
};

#[derive(Clone, Copy)]
struct ExpectedCensus {
    zero: usize,
    factorized: usize,
    genuine: usize,
    t4: usize,
    t2s: usize,
    s2: usize,
    tb4: usize,
    tf5: usize,
    tm6: usize,
}

fn expected(topology: FourLoopTopology) -> ExpectedCensus {
    match topology {
        FourLoopTopology::H => ExpectedCensus {
            zero: 198,
            factorized: 240,
            genuine: 74,
            t4: 75,
            t2s: 75,
            s2: 6,
            tb4: 30,
            tf5: 48,
            tm6: 6,
        },
        FourLoopTopology::X => ExpectedCensus {
            zero: 184,
            factorized: 231,
            genuine: 97,
            t4: 81,
            t2s: 72,
            s2: 6,
            tb4: 36,
            tf5: 36,
            tm6: 0,
        },
        FourLoopTopology::Bmw => ExpectedCensus {
            zero: 122,
            factorized: 107,
            genuine: 27,
            t4: 45,
            t2s: 32,
            s2: 2,
            tb4: 16,
            tf5: 12,
            tm6: 0,
        },
        FourLoopTopology::Fg => ExpectedCensus {
            zero: 132,
            factorized: 108,
            genuine: 16,
            t4: 40,
            t2s: 34,
            s2: 2,
            tb4: 12,
            tf5: 18,
            tm6: 2,
        },
    }
}

fn product(factors: &[MassiveVacuumMaster]) -> MasterProduct<MassiveVacuumMaster> {
    MasterProduct::try_from_factors(factors.iter().copied()).unwrap()
}

fn corner(topology: FourLoopTopology, mask: usize) -> Integral {
    let mut powers = vec![0; 10];
    for (position, power) in powers[..topology.routings().len()].iter_mut().enumerate() {
        *power = i32::from(mask & (1 << position) != 0);
    }
    Integral::new(powers)
}

fn determinant(matrix: &[[ExactRational; 4]; 4]) -> ExactRational {
    fn visit(
        row: usize,
        matrix: &[[ExactRational; 4]; 4],
        used: &mut [bool; 4],
        columns: &mut [usize; 4],
        total: &mut ExactRational,
    ) {
        if row == 4 {
            let inversions = (0..4)
                .flat_map(|left| (left + 1..4).map(move |right| (left, right)))
                .filter(|&(left, right)| columns[left] > columns[right])
                .count();
            let value = (0..4)
                .map(|index| &matrix[index][columns[index]])
                .fold(ExactRational::one(), |left, right| &left * right);
            let signed = if inversions % 2 == 0 { value } else { -value };
            *total = &*total + &signed;
            return;
        }
        for column in 0..4 {
            if !used[column] {
                used[column] = true;
                columns[row] = column;
                visit(row + 1, matrix, used, columns, total);
                used[column] = false;
            }
        }
    }
    let mut total = ExactRational::zero();
    visit(0, matrix, &mut [false; 4], &mut [0; 4], &mut total);
    total
}

fn check_witness(
    reducer: &FourLoopBoundaryReducer,
    expected_product: &MasterProduct<MassiveVacuumMaster>,
    witness: &rustred::FourLoopFactorizationWitness,
) {
    assert_eq!(witness.topology(), reducer.topology());
    assert_eq!(witness.product().unwrap(), *expected_product);
    assert_eq!(reducer.replay_witness(witness).unwrap(), *expected_product);
    assert_eq!(
        determinant(witness.global_loop_map()),
        ExactRational::from(i64::from(witness.determinant_sign()))
    );
    assert_eq!(
        witness
            .global_basis_positions()
            .iter()
            .copied()
            .collect::<BTreeSet<_>>()
            .len(),
        4
    );

    let active = witness
        .line_coordinates()
        .iter()
        .map(|line| line.physical_position())
        .collect::<BTreeSet<_>>();
    let expected_active = (0..reducer.family().propagator_count())
        .filter(|&position| witness.sector_mask() & (1_u16 << position) != 0)
        .collect::<BTreeSet<_>>();
    assert_eq!(active, expected_active);

    for line in witness.line_coordinates() {
        let reconstructed: [ExactRational; 4] = std::array::from_fn(|column| {
            (0..4)
                .map(|slot| &line.coordinates()[slot] * &witness.global_loop_map()[slot][column])
                .fold(ExactRational::zero(), |left, right| &left + &right)
        });
        let expected = reducer.family().denominators()[line.physical_position()]
            .momentum()
            .unwrap();
        assert_eq!(reconstructed.as_slice(), expected);
    }

    let component_slots = witness
        .components()
        .iter()
        .flat_map(|component| component.global_basis_slots().iter().copied())
        .collect::<BTreeSet<_>>();
    assert_eq!(component_slots, BTreeSet::from([0, 1, 2, 3]));
    let component_lines = witness
        .components()
        .iter()
        .flat_map(|component| component.physical_positions().iter().copied())
        .collect::<BTreeSet<_>>();
    assert_eq!(component_lines, expected_active);
    assert!(witness.components().len() >= 2);
    assert!(witness.components().iter().all(|component| {
        component.master().loops() == component.global_basis_slots().len()
            && component.master().physical_lines() == component.physical_positions().len()
            && component.component_basis_positions().len() == component.master().loops()
            && component.canonical_signature().len() == component.master().physical_lines()
            && component.component_loop_map().len() == component.master().loops()
            && component.determinant_sign().unsigned_abs() == 1
            && component.signed_line_matches().len() == component.master().physical_lines()
    }));

    for component in witness.components() {
        let reference = match component.master() {
            MassiveVacuumMaster::T1 => vec![vec![1]],
            MassiveVacuumMaster::S2 => vec![vec![1, 0], vec![0, 1], vec![1, 1]],
            MassiveVacuumMaster::B4 => {
                vec![vec![1, 0, 0], vec![0, 1, 0], vec![-1, 0, 1], vec![0, 1, -1]]
            }
            MassiveVacuumMaster::F5 => vec![
                vec![1, 0, 0],
                vec![0, 1, 0],
                vec![0, 0, 1],
                vec![-1, 0, 1],
                vec![1, -1, 0],
            ],
            MassiveVacuumMaster::M6 => vec![
                vec![1, 0, 0],
                vec![0, 1, 0],
                vec![0, 0, 1],
                vec![-1, 0, 1],
                vec![1, -1, 0],
                vec![0, 1, -1],
            ],
        };
        let reference = reference
            .into_iter()
            .map(|row| {
                row.into_iter()
                    .map(|value| ExactRational::from(i64::from(value)))
                    .collect::<Vec<_>>()
            })
            .collect::<Vec<_>>();
        let map = component.component_loop_map();
        let determinant = match map.len() {
            1 => map[0][0].clone(),
            2 => &map[0][0] * &map[1][1] - &map[0][1] * &map[1][0],
            3 => {
                &map[0][0] * (&map[1][1] * &map[2][2] - &map[1][2] * &map[2][1])
                    - &map[0][1] * (&map[1][0] * &map[2][2] - &map[1][2] * &map[2][0])
                    + &map[0][2] * (&map[1][0] * &map[2][1] - &map[1][1] * &map[2][0])
            }
            _ => unreachable!(),
        };
        assert_eq!(
            determinant,
            ExactRational::from(i64::from(component.determinant_sign()))
        );
        let reference_positions = component
            .signed_line_matches()
            .iter()
            .map(|line| line.reference_position())
            .collect::<BTreeSet<_>>();
        assert_eq!(
            reference_positions,
            (0..reference.len()).collect::<BTreeSet<_>>()
        );
        for line_match in component.signed_line_matches() {
            assert!(line_match.orientation_sign().unsigned_abs() == 1);
            let line = witness
                .line_coordinates()
                .iter()
                .find(|line| line.physical_position() == line_match.physical_position())
                .unwrap();
            let local = component
                .global_basis_slots()
                .iter()
                .map(|&slot| line.coordinates()[slot].clone())
                .collect::<Vec<_>>();
            let mapped = (0..map.len())
                .map(|column| {
                    local
                        .iter()
                        .zip(map)
                        .map(|(left, right)| left * &right[column])
                        .fold(ExactRational::zero(), |left, right| &left + &right)
                })
                .collect::<Vec<_>>();
            let expected = reference[line_match.reference_position()]
                .iter()
                .map(|value| value * &ExactRational::from(i64::from(line_match.orientation_sign())))
                .collect::<Vec<_>>();
            assert_eq!(mapped, expected);
        }
    }
}

fn check_exhaustive_census(topology: FourLoopTopology) {
    let reducer =
        FourLoopBoundaryReducer::build(topology, FourLoopBoundaryConfig::default()).unwrap();
    let mut zero = 0;
    let mut genuine = 0;
    let mut products = BTreeMap::<MasterProduct<MassiveVacuumMaster>, usize>::new();
    let masks = 1_usize << topology.routings().len();
    for mask in 0..masks {
        let integral = corner(topology, mask);
        match reducer.classify_integral(&integral).unwrap() {
            FourLoopScalarClass::Scaleless {
                sector_mask,
                active_lines,
                routing_rank,
            } => {
                zero += 1;
                assert_eq!(usize::from(sector_mask), mask);
                assert_eq!(active_lines, mask.count_ones() as usize);
                assert!(routing_rank < 4);
                assert!(reducer.reduce_integral(&integral).unwrap().is_zero());
            }
            FourLoopScalarClass::Factorized { product, witness } => {
                *products.entry(product.clone()).or_default() += 1;
                check_witness(&reducer, &product, &witness);
                let reduction = reducer.reduce_integral(&integral).unwrap();
                assert_eq!(reduction.len(), 1);
                assert_eq!(
                    reduction.coefficient(&product),
                    Some(&reducer.family().coefficients().one())
                );
            }
            FourLoopScalarClass::GenuineFourLoop {
                sector_mask,
                active_lines,
                determinant_sign,
                ..
            } => {
                genuine += 1;
                assert_eq!(usize::from(sector_mask), mask);
                assert_eq!(active_lines, mask.count_ones() as usize);
                assert!(determinant_sign == 1 || determinant_sign == -1);
                assert!(reducer.try_reduce_integral(&integral).unwrap().is_none());
                assert!(matches!(
                    reducer.reduce_integral(&integral),
                    Err(FourLoopBoundaryError::GenuineFourLoopCorner { .. })
                ));
            }
        }
    }

    let expected = expected(topology);
    assert_eq!(zero, expected.zero);
    assert_eq!(genuine, expected.genuine);
    assert_eq!(products.values().sum::<usize>(), expected.factorized);
    assert_eq!(zero + genuine + expected.factorized, masks);
    for (key, count) in [
        (product(&[MassiveVacuumMaster::T1; 4]), expected.t4),
        (
            product(&[
                MassiveVacuumMaster::T1,
                MassiveVacuumMaster::T1,
                MassiveVacuumMaster::S2,
            ]),
            expected.t2s,
        ),
        (product(&[MassiveVacuumMaster::S2; 2]), expected.s2),
        (
            product(&[MassiveVacuumMaster::T1, MassiveVacuumMaster::B4]),
            expected.tb4,
        ),
        (
            product(&[MassiveVacuumMaster::T1, MassiveVacuumMaster::F5]),
            expected.tf5,
        ),
        (
            product(&[MassiveVacuumMaster::T1, MassiveVacuumMaster::M6]),
            expected.tm6,
        ),
    ] {
        assert_eq!(products.get(&key).copied().unwrap_or(0), count);
    }
    assert_eq!(products.len(), 6 - usize::from(expected.tm6 == 0));
}

fn orientation_flipped_family(topology: FourLoopTopology) -> VacuumFamily {
    let coefficients = CoefficientContext::new(["d", "m2"]);
    let mass = coefficients.parameter("m2").unwrap();
    let propagators = topology
        .routings()
        .iter()
        .enumerate()
        .map(|(position, routing)| {
            let sign = if position % 2 == 0 { -1 } else { 1 };
            Denominator::propagator(
                routing
                    .iter()
                    .map(|&value| ExactRational::from(i64::from(sign * value)))
                    .collect(),
                mass.clone(),
            )
        })
        .collect();
    VacuumFamily::new_with_standard_auxiliaries(
        format!("{}_orientation_flipped", topology.name()),
        4,
        coefficients,
        "d",
        propagators,
        Vec::new(),
    )
    .unwrap()
}

fn class_summary(class: FourLoopScalarClass) -> (u8, Option<MasterProduct<MassiveVacuumMaster>>) {
    match class {
        FourLoopScalarClass::Scaleless { .. } => (0, None),
        FourLoopScalarClass::Factorized { product, .. } => (1, Some(product)),
        FourLoopScalarClass::GenuineFourLoop { .. } => (2, None),
    }
}

fn check_orientation_invariance() {
    let stable_keys = [
        MassiveVacuumMaster::T1,
        MassiveVacuumMaster::S2,
        MassiveVacuumMaster::B4,
        MassiveVacuumMaster::F5,
        MassiveVacuumMaster::M6,
    ]
    .map(MassiveVacuumMaster::stable_key);
    assert_eq!(stable_keys.into_iter().collect::<BTreeSet<_>>().len(), 5);
    assert!(
        stable_keys
            .iter()
            .all(|key| key.starts_with(MassiveVacuumMaster::SCHEMA))
    );

    for topology in FourLoopTopology::ALL {
        let baseline =
            FourLoopBoundaryReducer::build(topology, FourLoopBoundaryConfig::default()).unwrap();
        let flipped = FourLoopBoundaryReducer::new(
            topology,
            orientation_flipped_family(topology),
            FourLoopBoundaryConfig::default(),
        )
        .unwrap();
        for mask in 0..(1_usize << topology.routings().len()) {
            let flipped_class = flipped.classify_integral(&corner(topology, mask)).unwrap();
            if let FourLoopScalarClass::Factorized { product, witness } = &flipped_class {
                assert_eq!(flipped.replay_witness(witness).unwrap(), *product);
            }
            assert_eq!(
                class_summary(baseline.classify_integral(&corner(topology, mask)).unwrap()),
                class_summary(flipped_class),
                "orientation-dependent classification for {:?} mask {mask:#x}",
                topology,
            );
        }
    }
}

fn check_domain_and_resources() {
    let reducer =
        FourLoopBoundaryReducer::build(FourLoopTopology::H, FourLoopBoundaryConfig::default())
            .unwrap();
    assert!(matches!(
        reducer.classify_integral(&Integral::from([1, 1, 1, 1])),
        Err(FourLoopBoundaryError::WrongIntegralArity {
            expected: 10,
            actual: 4,
        })
    ));
    let mut powers = vec![0; 10];
    powers[0] = -1;
    assert!(matches!(
        reducer.classify_integral(&Integral::new(powers.clone())),
        Err(FourLoopBoundaryError::PhysicalNumerator {
            position: 0,
            power: -1,
        })
    ));
    powers[0] = 2;
    assert!(matches!(
        reducer.classify_integral(&Integral::new(powers.clone())),
        Err(FourLoopBoundaryError::PhysicalDot {
            position: 0,
            power: 2,
        })
    ));
    powers[0] = 0;
    powers[9] = -1;
    assert!(matches!(
        reducer.classify_integral(&Integral::new(powers)),
        Err(FourLoopBoundaryError::NonzeroAuxiliary {
            position: 9,
            power: -1,
        })
    ));

    let no_global_work = FourLoopBoundaryReducer::build(
        FourLoopTopology::H,
        FourLoopBoundaryConfig {
            max_global_basis_candidates: 0,
            ..FourLoopBoundaryConfig::default()
        },
    )
    .unwrap();
    assert!(matches!(
        no_global_work.classify_integral(&corner(FourLoopTopology::H, (1 << 9) - 1)),
        Err(FourLoopBoundaryError::ResourceLimit {
            resource: "global unimodular basis candidates",
            requested: 126,
            limit: 0,
        })
    ));

    let no_component_work = FourLoopBoundaryReducer::build(
        FourLoopTopology::H,
        FourLoopBoundaryConfig {
            max_component_basis_candidates: 0,
            ..FourLoopBoundaryConfig::default()
        },
    )
    .unwrap();
    assert!(matches!(
        no_component_work.classify_integral(&corner(FourLoopTopology::H, 15)),
        Err(FourLoopBoundaryError::ResourceLimit {
            resource: "component ordered-basis candidates",
            requested: 2,
            limit: 0,
        })
    ));

    assert!(matches!(
        FourLoopBoundaryReducer::new(
            FourLoopTopology::H,
            equal_mass_four_loop_vacuum(FourLoopTopology::X).unwrap(),
            FourLoopBoundaryConfig::default(),
        ),
        Err(FourLoopBoundaryError::WrongMomentumRouting { .. })
    ));

    let h = FourLoopBoundaryReducer::build(FourLoopTopology::H, FourLoopBoundaryConfig::default())
        .unwrap();
    let x = FourLoopBoundaryReducer::build(FourLoopTopology::X, FourLoopBoundaryConfig::default())
        .unwrap();
    let FourLoopScalarClass::Factorized { witness, .. } = h
        .classify_integral(&corner(FourLoopTopology::H, 15))
        .unwrap()
    else {
        panic!("four independent tadpoles must factorize")
    };
    assert!(matches!(
        x.replay_witness(&witness),
        Err(FourLoopBoundaryError::WitnessMismatch)
    ));
}

// Restricted Symbolica requires all coefficient-backed checks in one worker.
#[test]
fn exhaustive_four_loop_scalar_corner_boundary() {
    for topology in FourLoopTopology::ALL {
        check_exhaustive_census(topology);
    }
    check_orientation_invariance();
    check_domain_and_resources();
}
