use std::collections::BTreeSet;

use rustred::{ExactRational, MasterProduct};
use rustred_legacy_oracles::{
    FOUR_LOOP_AFFINE_MAP_OPERATION_BOUND, FourLoopGenuineClassifier, FourLoopGenuineConfig,
    FourLoopGenuineCornerType, FourLoopGenuineWitness, FourLoopHaloColumnKey, FourLoopHaloConfig,
    FourLoopHaloError, FourLoopHaloMapper, FourLoopTopology, MassiveVacuumMaster,
};
use rustred_legacy_oracles::{IbpGenerator, Integral};

fn corner_in(topology: FourLoopTopology, mask: u16) -> Integral {
    let mut powers = [0_i32; 10];
    for (position, power) in powers[..topology.routings().len()].iter_mut().enumerate() {
        *power = i32::from(mask & (1_u16 << position) != 0);
    }
    Integral::from(powers)
}

fn corner(kind: FourLoopGenuineCornerType) -> Integral {
    corner_in(kind.reference_topology(), kind.reference_mask())
}

fn evaluate_quadratic(
    quadratic_form: &[ExactRational],
    momenta: &[ExactRational; 4],
) -> ExactRational {
    let mut value = ExactRational::zero();
    let mut position = 0;
    for left in 0..4 {
        for right in left..4 {
            let term = &quadratic_form[position] * &momenta[left] * &momenta[right];
            value = &value + &term;
            position += 1;
        }
    }
    value
}

/// Check the map without reusing the production upper-triangular transform.
/// Evaluations at `e_i` and `e_i+e_j` determine every diagonal and cross
/// coefficient of a four-variable quadratic form independently.
fn check_affine_map_by_evaluation(
    classifier: &FourLoopGenuineClassifier,
    witness: &FourLoopGenuineWitness,
    mapper: &FourLoopHaloMapper,
) {
    let mut probes = Vec::with_capacity(10);
    for left in 0..4 {
        let mut probe: [ExactRational; 4] = std::array::from_fn(|_| ExactRational::zero());
        probe[left] = ExactRational::one();
        probes.push(probe.clone());
        for right in left + 1..4 {
            let mut pair = probe.clone();
            pair[right] = ExactRational::one();
            probes.push(pair);
        }
    }
    assert_eq!(probes.len(), 10);

    for (source, image) in classifier
        .family()
        .denominators()
        .iter()
        .zip(mapper.images())
    {
        for reference_momenta in &probes {
            let source_momenta: [ExactRational; 4] = std::array::from_fn(|row| {
                (0..4)
                    .map(|column| &witness.loop_map()[row][column] * &reference_momenta[column])
                    .fold(ExactRational::zero(), |sum, value| &sum + &value)
            });
            let source_value = evaluate_quadratic(source.quadratic_form(), &source_momenta);
            let reference_value = image
                .denominator_coefficients()
                .iter()
                .zip(mapper.reference_family().denominators())
                .map(|(coefficient, denominator)| {
                    coefficient
                        * evaluate_quadratic(denominator.quadratic_form(), reference_momenta)
                })
                .fold(ExactRational::zero(), |sum, value| &sum + &value);
            assert_eq!(source_value, reference_value);
        }

        let mut reconstructed_shift = image.constant().clone();
        for (coefficient, reference) in image
            .denominator_coefficients()
            .iter()
            .zip(mapper.reference_family().denominators())
        {
            reconstructed_shift = &reconstructed_shift
                + &mapper
                    .reference_family()
                    .coefficients()
                    .scale_rational(reference.shift(), coefficient);
        }
        assert_eq!(&reconstructed_shift, source.shift());
    }
}

fn check_all_affine_maps_and_raw_rows() {
    let h = FourLoopGenuineClassifier::build(
        rustred_legacy_oracles::FourLoopTopology::H,
        FourLoopGenuineConfig::default(),
    )
    .unwrap();
    let x = FourLoopGenuineClassifier::build(
        rustred_legacy_oracles::FourLoopTopology::X,
        FourLoopGenuineConfig::default(),
    )
    .unwrap();
    let mut raw_rows = 0_usize;
    let mut mapped_integrals = 0_usize;
    for kind in FourLoopGenuineCornerType::ALL {
        let classifier = if kind.reference_topology() == rustred_legacy_oracles::FourLoopTopology::H
        {
            &h
        } else {
            &x
        };
        let source_corner = corner(kind);
        let class = classifier.classify_integral(&source_corner).unwrap();
        assert_eq!(class.corner_type(), kind);
        let mapper = FourLoopHaloMapper::from_witness(
            classifier,
            class.witness(),
            FourLoopHaloConfig::default(),
        )
        .unwrap();

        assert_eq!(mapper.source_topology(), kind.reference_topology());
        assert_eq!(mapper.source_sector_mask(), kind.reference_mask());
        assert_eq!(mapper.corner_type(), kind);
        assert_eq!(mapper.images().len(), 10);
        assert_eq!(mapper.reference_family().denominator_count(), 10);
        mapper
            .replay_affine_images(classifier, class.witness())
            .unwrap();
        check_affine_map_by_evaluation(classifier, class.witness(), &mapper);

        // The scalar corner itself transports to the frozen representative.
        let mapped_corner = mapper.map_raw_halo_integral(&source_corner).unwrap();
        assert_eq!(mapped_corner.len(), 1);
        let one = mapper.reference_family().coefficients().one();
        assert_eq!(mapped_corner.coefficient(&corner(kind)), Some(&one));

        // These are precisely the ten representatives' 10*16 raw corner rows.
        // Every emitted D1/N1 integral must be transportable without treating
        // an ISP as if it were an independently matched physical line.
        let identities = IbpGenerator::new(classifier.family()).generate_raw(&source_corner);
        assert_eq!(identities.len(), 16);
        raw_rows += identities.len();
        for identity in identities {
            assert!(!identity.equation.is_zero());
            for integral in identity.equation.terms().keys() {
                let mapped = mapper.map_raw_halo_integral(integral).unwrap();
                assert!(
                    mapped
                        .terms()
                        .keys()
                        .all(|integral| integral.powers().len() == 10)
                );
                mapped_integrals += 1;
            }
        }
    }
    assert_eq!(raw_rows, 160);
    assert!(mapped_integrals > raw_rows);
}

fn check_nontrivial_interfamily_maps() {
    // The frozen H/X representatives above necessarily classify through a
    // self-map.  The full BMW and FG corners exercise nonidentity maps, the
    // source/reference direction, and transport between different generated
    // ISP completions.
    for (topology, expected_kind) in [
        (FourLoopTopology::Bmw, FourLoopGenuineCornerType::EightLineB),
        (FourLoopTopology::Fg, FourLoopGenuineCornerType::EightLineA),
    ] {
        let classifier =
            FourLoopGenuineClassifier::build(topology, FourLoopGenuineConfig::default()).unwrap();
        let source_corner = corner_in(topology, 0xff);
        let class = classifier.classify_integral(&source_corner).unwrap();
        assert_eq!(class.corner_type(), expected_kind);
        assert!((0..4).any(|row| {
            (0..4).any(|column| {
                class.witness().loop_map()[row][column]
                    != if row == column {
                        ExactRational::one()
                    } else {
                        ExactRational::zero()
                    }
            })
        }));

        let mapper = FourLoopHaloMapper::from_witness(
            &classifier,
            class.witness(),
            FourLoopHaloConfig::default(),
        )
        .unwrap();
        check_affine_map_by_evaluation(&classifier, class.witness(), &mapper);
        for identity in IbpGenerator::new(classifier.family()).generate_raw(&source_corner) {
            for integral in identity.equation.terms().keys() {
                mapper.map_raw_halo_integral(integral).unwrap();
            }
        }
    }
}

fn check_replay_tampering_and_limits() {
    let kind = FourLoopGenuineCornerType::HNineLine;
    let classifier = FourLoopGenuineClassifier::build(
        kind.reference_topology(),
        FourLoopGenuineConfig::default(),
    )
    .unwrap();
    let class = classifier.classify_integral(&corner(kind)).unwrap();

    for (config, resource, requested, limit) in [
        (
            FourLoopHaloConfig {
                max_affine_basis_images: 9,
                ..FourLoopHaloConfig::default()
            },
            "affine denominator-basis images",
            10,
            9,
        ),
        (
            FourLoopHaloConfig {
                max_affine_operations: FOUR_LOOP_AFFINE_MAP_OPERATION_BOUND as usize - 1,
                ..FourLoopHaloConfig::default()
            },
            "affine exact operations",
            FOUR_LOOP_AFFINE_MAP_OPERATION_BOUND,
            FOUR_LOOP_AFFINE_MAP_OPERATION_BOUND - 1,
        ),
    ] {
        assert!(matches!(
            FourLoopHaloMapper::from_witness(&classifier, class.witness(), config),
            Err(FourLoopHaloError::ResourceLimit {
                resource: actual_resource,
                requested: actual_requested,
                limit: actual_limit,
            }) if actual_resource == resource
                && actual_requested == requested
                && actual_limit == limit
        ));
    }

    let mapper = FourLoopHaloMapper::from_witness(
        &classifier,
        class.witness(),
        FourLoopHaloConfig::default(),
    )
    .unwrap();
    let bad_image = mapper.images()[0].with_denominator_coefficient_for_replay(
        0,
        &mapper.images()[0].denominator_coefficients()[0] + &rustred::ExactRational::one(),
    );
    let tampered = mapper.with_affine_image_for_replay(0, bad_image);
    assert!(matches!(
        tampered.replay_affine_images(&classifier, class.witness()),
        Err(FourLoopHaloError::AffineReplayMismatch { position: 0 })
            | Err(FourLoopHaloError::ActiveLineImageMismatch {
                source_position: 0,
                ..
            })
    ));

    let zero_output = FourLoopHaloMapper::from_witness(
        &classifier,
        class.witness(),
        FourLoopHaloConfig {
            max_expanded_terms: 0,
            ..FourLoopHaloConfig::default()
        },
    )
    .unwrap();
    assert!(matches!(
        zero_output.map_raw_halo_integral(&corner(kind)),
        Err(FourLoopHaloError::ResourceLimit {
            resource: "expanded halo terms",
            requested: 1,
            limit: 0,
        })
    ));
    assert!(matches!(
        mapper.map_raw_halo_integral(&Integral::from([1_i32; 9])),
        Err(FourLoopHaloError::WrongIntegralArity {
            expected: 10,
            actual: 9,
        })
    ));

    let mut outside: [i32; 10] = corner(kind)
        .powers()
        .try_into()
        .expect("the corner has ten powers");
    outside[0] = 3;
    assert!(matches!(
        mapper.map_raw_halo_integral(&Integral::from(outside)),
        Err(FourLoopHaloError::OutsideRawCornerHalo { .. })
    ));

    // `(D,N)` does not count pinches.  Reject shapes that satisfy the degree
    // bounds but cannot be emitted directly by an IBP at this scalar corner.
    let mut scalar_pinch: [i32; 10] = corner(kind).powers().try_into().unwrap();
    scalar_pinch[0] = 0;
    let mut numerator_without_dot: [i32; 10] = corner(kind).powers().try_into().unwrap();
    numerator_without_dot[9] = -1;
    let mut two_pinches: [i32; 10] = corner(kind).powers().try_into().unwrap();
    two_pinches[0] = 2;
    two_pinches[1] = 0;
    two_pinches[2] = 0;
    let mut numerator_and_pinch: [i32; 10] = corner(kind).powers().try_into().unwrap();
    numerator_and_pinch[0] = 2;
    numerator_and_pinch[1] = 0;
    numerator_and_pinch[9] = -1;
    for powers in [
        scalar_pinch,
        numerator_without_dot,
        two_pinches,
        numerator_and_pinch,
    ] {
        assert!(matches!(
            mapper.map_raw_halo_integral(&Integral::from(powers)),
            Err(FourLoopHaloError::OutsideRawCornerHalo { .. })
        ));
    }
}

fn check_stable_column_namespaces() {
    let mut keys = BTreeSet::new();
    assert!(keys.insert(FourLoopHaloColumnKey::Scaleless.stable_key()));
    assert!(keys.insert(FourLoopHaloColumnKey::Factorized(MasterProduct::identity()).stable_key()));
    for master in [
        MassiveVacuumMaster::T1,
        MassiveVacuumMaster::S2,
        MassiveVacuumMaster::B4,
        MassiveVacuumMaster::F5,
        MassiveVacuumMaster::M6,
    ] {
        assert!(keys.insert(
            FourLoopHaloColumnKey::Factorized(MasterProduct::from_factor(master)).stable_key()
        ));
    }
    for corner_type in FourLoopGenuineCornerType::ALL {
        assert!(
            keys.insert(
                FourLoopHaloColumnKey::GenuineRepresentative {
                    corner_type,
                    integral: corner(corner_type),
                }
                .stable_key()
            )
        );
    }
    assert_eq!(keys.len(), 17);
    assert!(
        keys.iter()
            .all(|key| key.starts_with(FourLoopHaloColumnKey::SCHEMA))
    );

    let product =
        MasterProduct::try_from_factors([MassiveVacuumMaster::T1, MassiveVacuumMaster::M6])
            .unwrap();
    let product_key = FourLoopHaloColumnKey::Factorized(product).stable_key();
    let zero_key = FourLoopHaloColumnKey::Scaleless.stable_key();
    assert_ne!(product_key, zero_key);
    assert!(product_key.contains(MassiveVacuumMaster::T1.stable_key()));
    assert!(product_key.contains(MassiveVacuumMaster::M6.stable_key()));
}

// Restricted Symbolica must remain on one worker, so construction, all 160
// frozen raw rows, nontrivial interfamily maps, replay/domain failures, and
// stable-key checks share one integration test.
#[test]
fn exact_four_loop_genuine_halo_affine_transport() {
    check_all_affine_maps_and_raw_rows();
    check_nontrivial_interfamily_maps();
    check_replay_tampering_and_limits();
    check_stable_column_namespaces();
}
