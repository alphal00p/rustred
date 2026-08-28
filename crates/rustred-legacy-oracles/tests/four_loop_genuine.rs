use std::collections::{BTreeMap, BTreeSet};

use rustred::{CoefficientContext, Denominator, ExactRational, Integral, VacuumFamily};
use rustred_legacy_oracles::{
    FourLoopGenuineClassifier, FourLoopGenuineConfig, FourLoopGenuineCornerType,
    FourLoopGenuineError, FourLoopTopology,
};

fn corner(topology: FourLoopTopology, mask: usize) -> Integral {
    let mut powers = vec![0; 10];
    for (position, power) in powers[..topology.routings().len()].iter_mut().enumerate() {
        *power = i32::from(mask & (1 << position) != 0);
    }
    Integral::new(powers)
}

fn expected(topology: FourLoopTopology, kind: FourLoopGenuineCornerType) -> usize {
    use FourLoopGenuineCornerType as K;
    match (topology, kind) {
        (FourLoopTopology::H, K::FiveLine) => 6,
        (FourLoopTopology::H, K::SixLineA) => 24,
        (FourLoopTopology::H, K::SixLineB) => 4,
        (FourLoopTopology::H, K::SevenLineA) => 3,
        (FourLoopTopology::H, K::SevenLineB) => 18,
        (FourLoopTopology::H, K::SevenLineC) => 9,
        (FourLoopTopology::H, K::EightLineA) => 6,
        (FourLoopTopology::H, K::EightLineB) => 3,
        (FourLoopTopology::H, K::HNineLine) => 1,
        (FourLoopTopology::X, K::FiveLine) => 9,
        (FourLoopTopology::X, K::SixLineA) => 36,
        (FourLoopTopology::X, K::SixLineB) => 6,
        (FourLoopTopology::X, K::SevenLineB) => 18,
        (FourLoopTopology::X, K::SevenLineC) => 18,
        (FourLoopTopology::X, K::EightLineB) => 9,
        (FourLoopTopology::X, K::XNineLine) => 1,
        (FourLoopTopology::Bmw, K::FiveLine) => 4,
        (FourLoopTopology::Bmw, K::SixLineA) => 12,
        (FourLoopTopology::Bmw, K::SixLineB) => 2,
        (FourLoopTopology::Bmw, K::SevenLineB) => 4,
        (FourLoopTopology::Bmw, K::SevenLineC) => 4,
        (FourLoopTopology::Bmw, K::EightLineB) => 1,
        (FourLoopTopology::Fg, K::FiveLine) => 2,
        (FourLoopTopology::Fg, K::SixLineA) => 6,
        (FourLoopTopology::Fg, K::SixLineB) => 1,
        (FourLoopTopology::Fg, K::SevenLineA) => 1,
        (FourLoopTopology::Fg, K::SevenLineB) => 4,
        (FourLoopTopology::Fg, K::SevenLineC) => 1,
        (FourLoopTopology::Fg, K::EightLineA) => 1,
        _ => 0,
    }
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
        format!("{}_genuine_orientation_flipped", topology.name()),
        4,
        coefficients,
        "d",
        propagators,
        Vec::new(),
    )
    .unwrap()
}

fn check_witness(
    classifier: &FourLoopGenuineClassifier,
    class: &rustred_legacy_oracles::FourLoopGenuineClass,
) {
    let witness = class.witness();
    let kind = class.corner_type();
    assert_eq!(witness.corner_type(), kind);
    assert_eq!(witness.source_topology(), classifier.topology());
    assert_eq!(witness.reference_topology(), kind.reference_topology());
    assert_eq!(witness.reference_sector_mask(), kind.reference_mask());
    assert_eq!(classifier.replay_witness(witness).unwrap(), kind);
    assert_eq!(
        determinant(witness.loop_map()),
        ExactRational::from(i64::from(witness.determinant_sign()))
    );
    assert_eq!(
        witness
            .source_basis_positions()
            .iter()
            .copied()
            .collect::<BTreeSet<_>>()
            .len(),
        4
    );
    assert_eq!(
        witness
            .reference_basis_positions()
            .iter()
            .copied()
            .collect::<BTreeSet<_>>()
            .len(),
        4
    );

    let source_positions = witness
        .signed_line_matches()
        .iter()
        .map(|line| line.source_physical_position())
        .collect::<BTreeSet<_>>();
    let expected_source = (0..classifier.family().propagator_count())
        .filter(|position| witness.source_sector_mask() & (1 << position) != 0)
        .collect::<BTreeSet<_>>();
    assert_eq!(source_positions, expected_source);
    let reference_positions = witness
        .signed_line_matches()
        .iter()
        .map(|line| line.reference_physical_position())
        .collect::<BTreeSet<_>>();
    let expected_reference = (0..kind.reference_topology().routings().len())
        .filter(|position| kind.reference_mask() & (1 << position) != 0)
        .collect::<BTreeSet<_>>();
    assert_eq!(reference_positions, expected_reference);

    for line in witness.signed_line_matches() {
        assert_eq!(line.orientation_sign().unsigned_abs(), 1);
        let source = classifier.family().denominators()[line.source_physical_position()]
            .momentum()
            .unwrap();
        let mapped: Vec<_> = (0..4)
            .map(|column| {
                source
                    .iter()
                    .enumerate()
                    .map(|(row, value)| value * &witness.loop_map()[row][column])
                    .fold(ExactRational::zero(), |sum, value| &sum + &value)
            })
            .collect();
        let expected: Vec<_> = kind.reference_topology().routings()
            [line.reference_physical_position()]
        .iter()
        .map(|&value| ExactRational::from(i64::from(value) * i64::from(line.orientation_sign())))
        .collect();
        assert_eq!(mapped, expected);
    }
}

fn check_exhaustive_catalog() {
    let mut global = BTreeMap::new();
    for topology in FourLoopTopology::ALL {
        let classifier =
            FourLoopGenuineClassifier::build(topology, FourLoopGenuineConfig::default()).unwrap();
        let mut counts = BTreeMap::new();
        let mut total = 0;
        for mask in 0..(1_usize << topology.routings().len()) {
            if let Some(class) = classifier
                .try_classify_integral(&corner(topology, mask))
                .unwrap()
            {
                total += 1;
                *counts.entry(class.corner_type()).or_insert(0) += 1;
                *global.entry(class.corner_type()).or_insert(0) += 1;
                check_witness(&classifier, &class);
            }
        }
        let expected_total = match topology {
            FourLoopTopology::H => 74,
            FourLoopTopology::X => 97,
            FourLoopTopology::Bmw => 27,
            FourLoopTopology::Fg => 16,
        };
        assert_eq!(total, expected_total);
        for kind in FourLoopGenuineCornerType::ALL {
            assert_eq!(
                counts.get(&kind).copied().unwrap_or(0),
                expected(topology, kind)
            );
        }
    }
    assert_eq!(global.values().sum::<usize>(), 214);
    for kind in FourLoopGenuineCornerType::ALL {
        assert_eq!(
            global.get(&kind).copied().unwrap_or(0),
            kind.labelled_multiplicity()
        );
    }

    let keys = FourLoopGenuineCornerType::ALL.map(FourLoopGenuineCornerType::stable_key);
    assert_eq!(keys.into_iter().collect::<BTreeSet<_>>().len(), 10);
    assert!(
        keys.iter()
            .all(|key| key.starts_with(FourLoopGenuineCornerType::SCHEMA))
    );
}

fn check_orientation_invariance() {
    for topology in FourLoopTopology::ALL {
        let baseline =
            FourLoopGenuineClassifier::build(topology, FourLoopGenuineConfig::default()).unwrap();
        let flipped = FourLoopGenuineClassifier::new(
            topology,
            orientation_flipped_family(topology),
            FourLoopGenuineConfig::default(),
        )
        .unwrap();
        for mask in 0..(1_usize << topology.routings().len()) {
            let baseline = baseline
                .try_classify_integral(&corner(topology, mask))
                .unwrap()
                .map(|class| class.corner_type());
            let flipped_class = flipped
                .try_classify_integral(&corner(topology, mask))
                .unwrap();
            if let Some(class) = &flipped_class {
                check_witness(&flipped, class);
            }
            assert_eq!(
                baseline,
                flipped_class.map(|class| class.corner_type()),
                "orientation-dependent genuine type for {topology:?} mask {mask:#x}"
            );
        }
    }
}

fn check_domain_and_resources() {
    assert!(matches!(
        FourLoopGenuineClassifier::build(
            FourLoopTopology::H,
            FourLoopGenuineConfig {
                max_catalog_signature_candidates: 204_287,
                ..FourLoopGenuineConfig::default()
            }
        ),
        Err(FourLoopGenuineError::ResourceLimit {
            resource: "catalog signed ordered-basis presentations",
            requested: 204_288,
            limit: 204_287,
        })
    ));

    let bounded = FourLoopGenuineClassifier::build(
        FourLoopTopology::H,
        FourLoopGenuineConfig {
            max_input_signature_candidates: 48_383,
            ..FourLoopGenuineConfig::default()
        },
    )
    .unwrap();
    assert!(matches!(
        bounded.try_classify_integral(&corner(FourLoopTopology::H, 0x1ff)),
        Err(FourLoopGenuineError::ResourceLimit {
            resource: "input signed ordered-basis presentations",
            requested: 48_384,
            limit: 48_383,
        })
    ));

    assert!(matches!(
        FourLoopGenuineClassifier::build(
            FourLoopTopology::H,
            FourLoopGenuineConfig {
                max_ordered_basis_storage: 3_023,
                ..FourLoopGenuineConfig::default()
            }
        ),
        Err(FourLoopGenuineError::ResourceLimit {
            resource: "ordered-basis candidate storage",
            requested: 3_024,
            limit: 3_023,
        })
    ));

    let h = FourLoopGenuineClassifier::build(FourLoopTopology::H, FourLoopGenuineConfig::default())
        .unwrap();
    assert!(
        h.try_classify_integral(&corner(FourLoopTopology::H, 0))
            .unwrap()
            .is_none()
    );
    assert!(
        h.try_classify_integral(&corner(FourLoopTopology::H, 15))
            .unwrap()
            .is_none()
    );
    assert!(matches!(
        h.classify_integral(&corner(FourLoopTopology::H, 15)),
        Err(FourLoopGenuineError::NotGenuineFourLoopCorner { .. })
    ));

    let witness = h
        .classify_integral(&corner(FourLoopTopology::H, 0x1ff))
        .unwrap()
        .into_witness();

    let mut tampered_basis = *witness.source_basis_positions();
    let replacement = (0..9)
        .find(|position| !tampered_basis.contains(position))
        .expect("a nine-line corner has an active non-basis line");
    tampered_basis[0] = replacement;
    let tampered = witness.with_source_basis_positions_for_replay(tampered_basis);
    assert!(matches!(
        h.replay_witness(&tampered),
        Err(FourLoopGenuineError::WitnessMismatch)
    ));

    let mut tampered_reference = *witness.reference_basis_positions();
    let replacement = (0..9)
        .find(|position| !tampered_reference.contains(position))
        .expect("a nine-line reference has an active non-basis line");
    tampered_reference[0] = replacement;
    let tampered = witness.with_reference_basis_positions_for_replay(tampered_reference);
    assert!(matches!(
        h.replay_witness(&tampered),
        Err(FourLoopGenuineError::WitnessMismatch)
    ));

    // These are paired active, distinct, but singular routing subsets.  They
    // would satisfy the stored identity line map if replay authenticated only
    // membership and pairwise line images instead of the claimed loop bases.
    let h9_identity = h
        .classify_integral(&corner(FourLoopTopology::H, 0x1ff))
        .unwrap()
        .into_witness();
    let paired_singular = h9_identity
        .with_source_basis_positions_for_replay([0, 1, 2, 4])
        .with_reference_basis_positions_for_replay([0, 1, 2, 4]);
    assert!(matches!(
        h.replay_witness(&paired_singular),
        Err(FourLoopGenuineError::WitnessMismatch)
    ));

    let x = FourLoopGenuineClassifier::build(FourLoopTopology::X, FourLoopGenuineConfig::default())
        .unwrap();
    assert!(matches!(
        x.replay_witness(&witness),
        Err(FourLoopGenuineError::WitnessMismatch)
    ));
}

// Restricted Symbolica requires all coefficient-backed checks in one worker.
#[test]
fn exhaustive_four_loop_genuine_corner_catalog() {
    check_exhaustive_catalog();
    check_orientation_invariance();
    check_domain_and_resources();
}
