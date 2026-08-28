use std::collections::{BTreeMap, BTreeSet, btree_map::Entry};

use rustred::{Coefficient, CoefficientContext, ExactRational};
use rustred_legacy_oracles::four_loop_polynomial_halo::{
    FOUR_LOOP_POLYNOMIAL_HALO_COLLECTED_MONOMIALS, FOUR_LOOP_POLYNOMIAL_HALO_CONVOLUTION_PRODUCTS,
    FOUR_LOOP_POLYNOMIAL_HALO_FACTOR_TERMS,
    FOUR_LOOP_POLYNOMIAL_HALO_MANIFEST_ROW_CONVOLUTION_PRODUCT_BOUND,
    FOUR_LOOP_POLYNOMIAL_HALO_MANIFEST_ROW_OUTPUT_BRANCH_BOUND,
    FOUR_LOOP_POLYNOMIAL_HALO_MANIFEST_ROW_RAW_COLLECTED_TERM_BOUND,
    FOUR_LOOP_POLYNOMIAL_HALO_NUMERATOR_FACTORS, FOUR_LOOP_POLYNOMIAL_HALO_OUTPUT_BRANCHES,
    FourLoopPolynomialBranchKind, FourLoopPolynomialHaloConfig, FourLoopPolynomialHaloError,
    FourLoopPolynomialHaloMapper,
};
use rustred_legacy_oracles::{Denominator, IbpGenerator, Integral, VacuumFamily};
use rustred_legacy_oracles::{
    FourLoopGenuineClassifier, FourLoopGenuineConfig, FourLoopGenuineCornerType,
    FourLoopGenuineWitness, FourLoopNextManifest, FourLoopNextManifestConfig, FourLoopTopology,
    four_loop_corner_seed,
};

const BASIS: usize = 10;

fn corner_in(topology: FourLoopTopology, mask: u16) -> Integral {
    let mut powers = [0_i32; BASIS];
    for (position, power) in powers[..topology.routings().len()].iter_mut().enumerate() {
        *power = i32::from(mask & (1_u16 << position) != 0);
    }
    Integral::from(powers)
}

fn renamed_reference_family(topology: FourLoopTopology) -> VacuumFamily {
    let coefficients = CoefficientContext::new(["d", "m2"]);
    let mass = coefficients.parameter("m2").unwrap();
    let propagators = topology
        .routings()
        .iter()
        .map(|routing| {
            Denominator::propagator(
                routing
                    .iter()
                    .map(|&value| ExactRational::from(i64::from(value)))
                    .collect(),
                mass.clone(),
            )
        })
        .collect();
    VacuumFamily::new_with_standard_auxiliaries(
        format!("{}_renamed_manifest_adversary", topology.name()),
        4,
        coefficients,
        "d",
        propagators,
        Vec::new(),
    )
    .unwrap()
}

fn add_coefficient(
    polynomial: &mut BTreeMap<[u8; BASIS], Coefficient>,
    monomial: [u8; BASIS],
    coefficient: Coefficient,
) {
    if coefficient.is_zero() {
        return;
    }
    match polynomial.entry(monomial) {
        Entry::Vacant(entry) => {
            entry.insert(coefficient);
        }
        Entry::Occupied(mut entry) => {
            let sum = entry.get() + &coefficient;
            if sum.is_zero() {
                entry.remove();
            } else {
                *entry.get_mut() = sum;
            }
        }
    }
}

/// Independent two-factor convolution using only the public affine-image
/// accessors, with `[u8;10]` rather than the production monomial type.
fn independently_convolve(
    mapper: &FourLoopPolynomialHaloMapper,
    map: &rustred_legacy_oracles::four_loop_polynomial_halo::FourLoopPolynomialMapWitness,
) -> BTreeMap<[u8; BASIS], Coefficient> {
    let context = mapper.affine_mapper().reference_family().coefficients();
    let factors = map
        .factor_images()
        .iter()
        .map(|image| {
            let mut factor = BTreeMap::new();
            add_coefficient(&mut factor, [0; BASIS], image.constant().clone());
            for (position, coefficient) in image.denominator_coefficients().iter().enumerate() {
                if coefficient.is_zero() {
                    continue;
                }
                let mut monomial = [0_u8; BASIS];
                monomial[position] = 1;
                add_coefficient(&mut factor, monomial, context.rational(coefficient));
            }
            factor
        })
        .collect::<Vec<_>>();

    match factors.as_slice() {
        [] => BTreeMap::from([([0; BASIS], context.one())]),
        [factor] => factor.clone(),
        [left, right] => {
            let mut output = BTreeMap::new();
            for (left_monomial, left_coefficient) in left {
                for (right_monomial, right_coefficient) in right {
                    let monomial = std::array::from_fn(|position| {
                        left_monomial[position] + right_monomial[position]
                    });
                    add_coefficient(&mut output, monomial, left_coefficient * right_coefficient);
                }
            }
            output
        }
        _ => panic!("the certified mapper emitted more than two factors"),
    }
}

fn independently_check_branches(
    witness: &FourLoopGenuineWitness,
    map: &rustred_legacy_oracles::four_loop_polynomial_halo::FourLoopPolynomialMapWitness,
) {
    let mut base = [0_i32; BASIS];
    for line in witness.signed_line_matches() {
        base[line.reference_physical_position()] =
            map.source_term().powers()[line.source_physical_position()];
    }
    assert_eq!(map.branches().len(), map.collected_monomials().len());
    for branch in map.branches() {
        let expected: [i32; BASIS] = std::array::from_fn(|position| {
            base[position] - i32::from(branch.monomial().denominator_powers()[position])
        });
        assert_eq!(branch.integral().powers(), expected);
        assert_eq!(
            map.collected_monomials().get(&branch.monomial()),
            Some(branch.coefficient())
        );
        match branch.kind() {
            FourLoopPolynomialBranchKind::SameGenuineMask { mask } => {
                assert_eq!(mask, witness.reference_sector_mask());
            }
            FourLoopPolynomialBranchKind::StrictlyLowerPhysicalMask {
                parent_mask,
                branch_mask,
            } => {
                assert_eq!(parent_mask, witness.reference_sector_mask());
                assert_ne!(branch_mask, parent_mask);
                assert_eq!(branch_mask & !parent_mask, 0);
            }
        }
    }
}

fn check_exhaustive_manifest() {
    let manifest = FourLoopNextManifest::build(FourLoopNextManifestConfig::default()).unwrap();
    let h = FourLoopGenuineClassifier::build(FourLoopTopology::H, FourLoopGenuineConfig::default())
        .unwrap();
    let x = FourLoopGenuineClassifier::build(FourLoopTopology::X, FourLoopGenuineConfig::default())
        .unwrap();

    let mut atlas = BTreeMap::new();
    for corner_type in FourLoopGenuineCornerType::ALL {
        let classifier = if corner_type.reference_topology() == FourLoopTopology::H {
            &h
        } else {
            &x
        };
        let class = classifier
            .classify_integral(&four_loop_corner_seed(corner_type))
            .unwrap();
        assert_eq!(class.corner_type(), corner_type);
        let genuine_witness = class.witness().clone();
        let mapper = FourLoopPolynomialHaloMapper::from_witness(
            classifier,
            &genuine_witness,
            FourLoopPolynomialHaloConfig::default(),
        )
        .unwrap();
        assert_eq!(
            mapper.source_family_fingerprint(),
            classifier.family().fingerprint()
        );
        assert_eq!(
            mapper.reference_family_fingerprint(),
            mapper.affine_mapper().reference_family().fingerprint()
        );
        atlas.insert(corner_type, (mapper, genuine_witness));
    }

    let mut origin_count = 0_usize;
    let mut mapped_term_count = 0_usize;
    let mut saw_degree_two = false;
    let mut saw_same_mask = false;
    let mut saw_lower_mask = false;
    let mut saw_weighted_recollection = false;
    let mut nested_source_keys = BTreeSet::new();
    for &raw_id in manifest.raw_row_ids() {
        let classifier = if raw_id.seed().topology() == FourLoopTopology::H {
            &h
        } else {
            &x
        };
        let (mapper, witness) = &atlas[&raw_id.seed().corner_type()];
        let row = mapper.map_manifest_raw_identity(raw_id).unwrap();
        assert_eq!(row.raw_id(), raw_id);
        assert_eq!(row.stats().raw_collected_terms(), row.terms().len());
        assert!(
            row.stats().raw_collected_terms()
                <= FOUR_LOOP_POLYNOMIAL_HALO_MANIFEST_ROW_RAW_COLLECTED_TERM_BOUND
        );
        assert!(
            row.stats().aggregate_convolution_products()
                <= FOUR_LOOP_POLYNOMIAL_HALO_MANIFEST_ROW_CONVOLUTION_PRODUCT_BOUND
        );
        assert!(
            row.stats().aggregate_output_branches()
                <= FOUR_LOOP_POLYNOMIAL_HALO_MANIFEST_ROW_OUTPUT_BRANCH_BOUND
        );

        // Regenerate independently and require exact collected keys,
        // coefficients, and deterministic order—not only matching row width.
        let native = IbpGenerator::new(classifier.family())
            .try_generate_raw_identity(
                &raw_id.seed().integral(),
                usize::from(raw_id.differentiated_loop()),
                usize::from(raw_id.contraction_loop()),
            )
            .unwrap();
        assert_eq!(native.equation.len(), row.terms().len());
        let mut reserved_row_convolution = 0_usize;
        let mut reserved_row_output = 0_usize;

        for ((native_integral, native_coefficient), mapped) in
            native.equation.terms().iter().zip(row.terms())
        {
            let map = mapped.polynomial_map();
            assert!(!mapped.raw_coefficient().is_zero());
            assert_eq!(mapped.raw_coefficient(), native_coefficient);
            assert_eq!(map.source_term(), native_integral);
            assert_eq!(map.manifest_raw_id(), Some(raw_id));
            assert!(nested_source_keys.insert((raw_id, native_integral.clone())));
            let weighted = mapped.weighted_branches().collect::<Vec<_>>();
            assert_eq!(weighted.len(), map.branches().len());
            for ((owned_branch, weighted_coefficient), expected_branch) in
                weighted.iter().zip(map.branches())
            {
                assert!(std::ptr::eq(*owned_branch, expected_branch));
                assert_eq!(
                    weighted_coefficient,
                    &(mapped.raw_coefficient() * expected_branch.coefficient())
                );
            }
            assert_eq!(map.stats().numerator_factors(), map.factor_images().len());
            let (convolution, output) = match map.stats().numerator_factors() {
                0 => (0, 1),
                1 => (
                    FOUR_LOOP_POLYNOMIAL_HALO_FACTOR_TERMS,
                    FOUR_LOOP_POLYNOMIAL_HALO_FACTOR_TERMS,
                ),
                2 => (
                    FOUR_LOOP_POLYNOMIAL_HALO_CONVOLUTION_PRODUCTS,
                    FOUR_LOOP_POLYNOMIAL_HALO_OUTPUT_BRANCHES,
                ),
                _ => panic!("manifest term exceeded degree two"),
            };
            reserved_row_convolution += convolution;
            reserved_row_output += output;
            assert!(map.stats().numerator_factors() <= FOUR_LOOP_POLYNOMIAL_HALO_NUMERATOR_FACTORS);
            assert!(
                map.stats().affine_factor_terms()
                    <= FOUR_LOOP_POLYNOMIAL_HALO_NUMERATOR_FACTORS
                        * FOUR_LOOP_POLYNOMIAL_HALO_FACTOR_TERMS
            );
            assert!(
                map.stats().convolution_products()
                    <= FOUR_LOOP_POLYNOMIAL_HALO_CONVOLUTION_PRODUCTS
            );
            assert!(
                map.stats().collected_monomials() <= FOUR_LOOP_POLYNOMIAL_HALO_COLLECTED_MONOMIALS
            );
            assert!(map.stats().output_branches() <= FOUR_LOOP_POLYNOMIAL_HALO_OUTPUT_BRANCHES);
            assert_eq!(
                independently_convolve(mapper, map),
                map.collected_monomials()
                    .iter()
                    .map(|(monomial, coefficient)| {
                        (*monomial.denominator_powers(), coefficient.clone())
                    })
                    .collect::<BTreeMap<_, _>>()
            );
            independently_check_branches(witness, map);
            saw_degree_two |= map.stats().numerator_factors() == 2;
            saw_same_mask |= map.branches().iter().any(|branch| {
                matches!(
                    branch.kind(),
                    FourLoopPolynomialBranchKind::SameGenuineMask { .. }
                )
            });
            saw_lower_mask |= map.branches().iter().any(|branch| {
                matches!(
                    branch.kind(),
                    FourLoopPolynomialBranchKind::StrictlyLowerPhysicalMask { .. }
                )
            });
        }
        assert_eq!(
            row.stats().aggregate_convolution_products(),
            reserved_row_convolution
        );
        assert_eq!(row.stats().aggregate_output_branches(), reserved_row_output);
        let uncollected_branch_count = row
            .terms()
            .iter()
            .map(|term| term.polynomial_map().branches().len())
            .sum::<usize>();
        let mut independently_collected = rustred_legacy_oracles::LinearCombination::new();
        for term in row.terms() {
            for branch in term.polynomial_map().branches() {
                independently_collected.add_term(
                    branch.integral().clone(),
                    term.raw_coefficient() * branch.coefficient(),
                );
            }
        }
        let collected = row.collected_linear_combination();
        assert_eq!(collected, independently_collected);
        assert!(collected.len() <= uncollected_branch_count);
        saw_weighted_recollection |= uncollected_branch_count != 0;
        mapper
            .replay_manifest_raw_identity(classifier, witness, &row)
            .unwrap();
        origin_count += 1;
        mapped_term_count += row.terms().len();
    }
    assert_eq!(origin_count, 1_968);
    assert_eq!(mapped_term_count, manifest.native_collected_terms());
    assert_eq!(nested_source_keys.len(), mapped_term_count);
    assert!(saw_degree_two);
    assert!(saw_same_mask);
    assert!(saw_lower_mask);
    assert!(saw_weighted_recollection);
}

fn degree_two_examples(
    topology: FourLoopTopology,
) -> (
    FourLoopGenuineClassifier,
    FourLoopGenuineWitness,
    FourLoopPolynomialHaloMapper,
    Integral,
    Integral,
    Integral,
) {
    let classifier =
        FourLoopGenuineClassifier::build(topology, FourLoopGenuineConfig::default()).unwrap();
    let corner = corner_in(topology, 0xff);
    let class = classifier.classify_integral(&corner).unwrap();
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
    let witness = class.witness().clone();
    let mapper = FourLoopPolynomialHaloMapper::from_witness(
        &classifier,
        &witness,
        FourLoopPolynomialHaloConfig::default(),
    )
    .unwrap();
    let inactive = (0..BASIS)
        .filter(|&position| witness.source_sector_mask() & (1_u16 << position) == 0)
        .collect::<Vec<_>>();
    assert!(inactive.len() >= 2);

    let mut seed: [i32; BASIS] = corner.powers().try_into().unwrap();
    seed[0] = 2;
    seed[inactive[0]] = -1;
    let mut repeated = seed;
    repeated[1] += 1;
    repeated[inactive[0]] -= 1;
    let mut distinct = seed;
    distinct[1] += 1;
    distinct[inactive[1]] -= 1;
    (
        classifier,
        witness,
        mapper,
        Integral::from(seed),
        Integral::from(repeated),
        Integral::from(distinct),
    )
}

fn check_nonidentity_convolution_and_replay() {
    for topology in [FourLoopTopology::Bmw, FourLoopTopology::Fg] {
        let (classifier, witness, mapper, seed, repeated, distinct) = degree_two_examples(topology);
        for (term, repeated_factor) in [(&repeated, true), (&distinct, false)] {
            let map = mapper.map_authenticated_raw_term(&seed, term).unwrap();
            assert_eq!(map.stats().numerator_factors(), 2);
            assert_eq!(map.factor_images().len(), 2);
            assert_eq!(
                map.source_numerator_factors()[0] == map.source_numerator_factors()[1],
                repeated_factor
            );
            assert_eq!(
                independently_convolve(&mapper, &map),
                map.collected_monomials()
                    .iter()
                    .map(|(monomial, coefficient)| {
                        (*monomial.denominator_powers(), coefficient.clone())
                    })
                    .collect::<BTreeMap<_, _>>()
            );
            independently_check_branches(&witness, &map);
            mapper
                .replay_polynomial_map(&classifier, &witness, &map)
                .unwrap();
        }
    }
}

fn check_rejections_limits_and_tampering() {
    let (classifier, witness, mapper, seed, repeated, _) =
        degree_two_examples(FourLoopTopology::Bmw);

    // Positive denominator powers are never numerator multiplicities.  This
    // guards the signed-to-usize conversion before any allocation is driven
    // by the factor count.
    let scalar_corner = corner_in(FourLoopTopology::Bmw, 0xff);
    let scalar_map = mapper
        .map_authenticated_raw_term(&scalar_corner, &scalar_corner)
        .unwrap();
    assert!(scalar_map.source_numerator_factors().is_empty());
    assert_eq!(scalar_map.stats().numerator_factors(), 0);

    let wrong_seed = Integral::from([1_i32; 9]);
    assert!(matches!(
        mapper.map_authenticated_raw_term(&wrong_seed, &wrong_seed),
        Err(FourLoopPolynomialHaloError::WrongIntegralArity {
            role: "seed",
            expected: 10,
            actual: 9,
        })
    ));
    let wrong_term = Integral::from([1_i32; 9]);
    assert!(matches!(
        mapper.map_authenticated_raw_term(&seed, &wrong_term),
        Err(FourLoopPolynomialHaloError::WrongIntegralArity {
            role: "term",
            expected: 10,
            actual: 9,
        })
    ));

    let mut outside_seed: [i32; BASIS] = seed.powers().try_into().unwrap();
    outside_seed[1] = 2;
    let outside_seed = Integral::from(outside_seed);
    assert!(matches!(
        mapper.map_authenticated_raw_term(&outside_seed, &outside_seed),
        Err(FourLoopPolynomialHaloError::OutsideNextShellSeed { .. })
    ));

    let mut nonadjacent: [i32; BASIS] = seed.powers().try_into().unwrap();
    nonadjacent[1] += 1;
    nonadjacent[2] += 1;
    assert!(matches!(
        mapper.map_authenticated_raw_term(&seed, &Integral::from(nonadjacent)),
        Err(FourLoopPolynomialHaloError::NonAdjacentRawTerm { .. })
    ));

    let manifest = FourLoopNextManifest::build(FourLoopNextManifestConfig::default()).unwrap();
    let foreign_id = manifest
        .raw_row_ids()
        .iter()
        .copied()
        .find(|raw_id| raw_id.seed().topology() != mapper.affine_mapper().source_topology())
        .unwrap_or(manifest.raw_row_ids()[0]);
    assert!(matches!(
        mapper.map_manifest_raw_identity(foreign_id),
        Err(FourLoopPolynomialHaloError::ManifestMapperMismatch { .. })
    ));

    let h = FourLoopGenuineClassifier::build(FourLoopTopology::H, FourLoopGenuineConfig::default())
        .unwrap();
    let h_id = manifest
        .raw_row_ids()
        .iter()
        .copied()
        .find(|raw_id| {
            if raw_id.seed().topology() != FourLoopTopology::H {
                return false;
            }
            IbpGenerator::new(h.family())
                .try_generate_raw_identity(
                    &raw_id.seed().integral(),
                    usize::from(raw_id.differentiated_loop()),
                    usize::from(raw_id.contraction_loop()),
                )
                .is_ok_and(|identity| !identity.equation.is_zero())
        })
        .unwrap();
    let h_class = h
        .classify_integral(&four_loop_corner_seed(h_id.seed().corner_type()))
        .unwrap();
    let h_mapper = FourLoopPolynomialHaloMapper::from_witness(
        &h,
        h_class.witness(),
        FourLoopPolynomialHaloConfig::default(),
    )
    .unwrap();
    assert!(matches!(
        h_mapper.map_manifest_raw_term(h_id, &Integral::from([0_i32; BASIS])),
        Err(FourLoopPolynomialHaloError::TermAbsentFromManifestRawRow { .. })
    ));

    // A routing-compatible H family remains valid for the general authenticated
    // surface, but its changed semantic fingerprint cannot own frozen manifest
    // origins.
    let renamed = renamed_reference_family(FourLoopTopology::H);
    let renamed_classifier = FourLoopGenuineClassifier::new(
        FourLoopTopology::H,
        renamed,
        FourLoopGenuineConfig::default(),
    )
    .unwrap();
    let h9_corner = four_loop_corner_seed(FourLoopGenuineCornerType::HNineLine);
    let renamed_class = renamed_classifier.classify_integral(&h9_corner).unwrap();
    let renamed_mapper = FourLoopPolynomialHaloMapper::from_witness(
        &renamed_classifier,
        renamed_class.witness(),
        FourLoopPolynomialHaloConfig::default(),
    )
    .unwrap();
    renamed_mapper
        .map_authenticated_raw_term(&h9_corner, &h9_corner)
        .unwrap();
    let h9_id = manifest
        .raw_row_ids()
        .iter()
        .copied()
        .find(|raw_id| raw_id.seed().corner_type() == FourLoopGenuineCornerType::HNineLine)
        .unwrap();
    for error in [
        renamed_mapper.map_manifest_raw_identity(h9_id).unwrap_err(),
        renamed_mapper
            .map_manifest_raw_term(h9_id, &h9_corner)
            .unwrap_err(),
    ] {
        assert!(matches!(
            error,
            FourLoopPolynomialHaloError::ManifestFamilyFingerprintMismatch { .. }
        ));
    }

    // The manifest's exponent vector is meaningful only in the frozen
    // reference sector.  A different labelled H sector may share its corner
    // type, but must not be permitted to regenerate that origin.
    let (alternate_corner, alternate_class) = (0_u16..(1_u16 << 9))
        .find_map(|mask| {
            let integral = corner_in(FourLoopTopology::H, mask);
            let class = h.try_classify_integral(&integral).ok().flatten()?;
            (class.witness().source_sector_mask() != class.corner_type().reference_mask())
                .then_some((integral, class))
        })
        .expect("the H atlas contains a non-reference labelled genuine sector");
    let alternate_id = manifest
        .raw_row_ids()
        .iter()
        .copied()
        .find(|raw_id| raw_id.seed().corner_type() == alternate_class.corner_type())
        .unwrap();
    let alternate_mapper = FourLoopPolynomialHaloMapper::from_witness(
        &h,
        alternate_class.witness(),
        FourLoopPolynomialHaloConfig::default(),
    )
    .unwrap();
    alternate_mapper
        .map_authenticated_raw_term(&alternate_corner, &alternate_corner)
        .unwrap();
    assert!(matches!(
        alternate_mapper.map_manifest_raw_identity(alternate_id),
        Err(FourLoopPolynomialHaloError::ManifestSourceSectorMismatch {
            expected_mask,
            actual_mask,
            ..
        }) if expected_mask == alternate_class.corner_type().reference_mask()
            && actual_mask == alternate_class.witness().source_sector_mask()
    ));

    // Row limits are separate from the per-transported-term limits.  Exercise
    // each conservative pre-generation cap one below the phase-sensitive
    // request of a mixed (therefore potentially N2) manifest origin.
    let mixed_id = manifest
        .raw_row_ids()
        .iter()
        .copied()
        .find(|raw_id| {
            let seed = raw_id.seed().integral();
            seed.dot_degree() == 1
                && seed.numerator_degree() == 1
                && raw_id.differentiated_loop() == raw_id.contraction_loop()
        })
        .unwrap();
    let mixed_class = h
        .classify_integral(&four_loop_corner_seed(mixed_id.seed().corner_type()))
        .unwrap();
    let mixed_seed = mixed_id.seed().integral();
    let row_raw_request = mixed_seed
        .powers()
        .iter()
        .filter(|&&power| power != 0)
        .count()
        * FOUR_LOOP_POLYNOMIAL_HALO_FACTOR_TERMS
        + 1;
    let row_convolution_request = row_raw_request * FOUR_LOOP_POLYNOMIAL_HALO_CONVOLUTION_PRODUCTS;
    let row_output_request = row_raw_request * FOUR_LOOP_POLYNOMIAL_HALO_OUTPUT_BRANCHES;
    assert!(row_raw_request <= FOUR_LOOP_POLYNOMIAL_HALO_MANIFEST_ROW_RAW_COLLECTED_TERM_BOUND);
    assert!(
        row_convolution_request <= FOUR_LOOP_POLYNOMIAL_HALO_MANIFEST_ROW_CONVOLUTION_PRODUCT_BOUND
    );
    assert!(row_output_request <= FOUR_LOOP_POLYNOMIAL_HALO_MANIFEST_ROW_OUTPUT_BRANCH_BOUND);
    for (config, resource, requested, limit) in [
        (
            FourLoopPolynomialHaloConfig {
                max_manifest_row_raw_collected_terms: row_raw_request - 1,
                ..FourLoopPolynomialHaloConfig::default()
            },
            "manifest row raw collected terms",
            row_raw_request,
            row_raw_request - 1,
        ),
        (
            FourLoopPolynomialHaloConfig {
                max_manifest_row_convolution_products: row_convolution_request - 1,
                ..FourLoopPolynomialHaloConfig::default()
            },
            "manifest row aggregate convolution products",
            row_convolution_request,
            row_convolution_request - 1,
        ),
        (
            FourLoopPolynomialHaloConfig {
                max_manifest_row_output_branches: row_output_request - 1,
                ..FourLoopPolynomialHaloConfig::default()
            },
            "manifest row aggregate output branches",
            row_output_request,
            row_output_request - 1,
        ),
    ] {
        let limited =
            FourLoopPolynomialHaloMapper::from_witness(&h, mixed_class.witness(), config).unwrap();
        assert!(matches!(
            limited.map_manifest_raw_identity(mixed_id),
            Err(FourLoopPolynomialHaloError::ResourceLimit {
                resource: actual_resource,
                requested: actual_requested,
                limit: actual_limit,
            }) if actual_resource == resource
                && actual_requested == requested as u128
                && actual_limit == limit as u128
        ));
    }

    // Row replay owns the nested raw coefficient, row ID, and complete term
    // sequence; altering or omitting any of them must fail exact rebuild.
    let row = h_mapper.map_manifest_raw_identity(h_id).unwrap();
    assert!(!row.terms().is_empty());
    let coefficient_one = h_mapper
        .affine_mapper()
        .reference_family()
        .coefficients()
        .one();
    let bad_term = row.terms()[0]
        .with_raw_coefficient_for_replay(row.terms()[0].raw_coefficient() + &coefficient_one);
    let bad_coefficient_row = row.with_term_for_replay(0, bad_term);
    assert!(matches!(
        h_mapper.replay_manifest_raw_identity(&h, h_class.witness(), &bad_coefficient_row),
        Err(FourLoopPolynomialHaloError::PolynomialReplayMismatch)
    ));
    let alternate_row_id = manifest
        .raw_row_ids()
        .iter()
        .copied()
        .find(|candidate| {
            candidate.seed().corner_type() == h_id.seed().corner_type() && *candidate != h_id
        })
        .unwrap();
    assert!(matches!(
        h_mapper.replay_manifest_raw_identity(
            &h,
            h_class.witness(),
            &row.with_raw_id_for_replay(alternate_row_id),
        ),
        Err(FourLoopPolynomialHaloError::PolynomialReplayMismatch)
    ));
    assert!(matches!(
        h_mapper.replay_manifest_raw_identity(
            &h,
            h_class.witness(),
            &row.without_term_for_replay(0),
        ),
        Err(FourLoopPolynomialHaloError::PolynomialReplayMismatch)
    ));

    let make_limited = |config: FourLoopPolynomialHaloConfig| {
        FourLoopPolynomialHaloMapper::from_witness(&classifier, &witness, config).unwrap()
    };
    for (config, resource, requested, limit) in [
        (
            FourLoopPolynomialHaloConfig {
                max_numerator_factors: 1,
                ..FourLoopPolynomialHaloConfig::default()
            },
            "numerator factors",
            2,
            1,
        ),
        (
            FourLoopPolynomialHaloConfig {
                max_factor_terms: FOUR_LOOP_POLYNOMIAL_HALO_FACTOR_TERMS - 1,
                ..FourLoopPolynomialHaloConfig::default()
            },
            "terms per affine factor",
            FOUR_LOOP_POLYNOMIAL_HALO_FACTOR_TERMS,
            FOUR_LOOP_POLYNOMIAL_HALO_FACTOR_TERMS - 1,
        ),
        (
            FourLoopPolynomialHaloConfig {
                max_convolution_products: FOUR_LOOP_POLYNOMIAL_HALO_CONVOLUTION_PRODUCTS - 1,
                ..FourLoopPolynomialHaloConfig::default()
            },
            "affine convolution products",
            FOUR_LOOP_POLYNOMIAL_HALO_CONVOLUTION_PRODUCTS,
            FOUR_LOOP_POLYNOMIAL_HALO_CONVOLUTION_PRODUCTS - 1,
        ),
        (
            FourLoopPolynomialHaloConfig {
                max_collected_monomials: FOUR_LOOP_POLYNOMIAL_HALO_COLLECTED_MONOMIALS - 1,
                ..FourLoopPolynomialHaloConfig::default()
            },
            "collected polynomial monomials",
            FOUR_LOOP_POLYNOMIAL_HALO_COLLECTED_MONOMIALS,
            FOUR_LOOP_POLYNOMIAL_HALO_COLLECTED_MONOMIALS - 1,
        ),
        (
            FourLoopPolynomialHaloConfig {
                max_output_branches: FOUR_LOOP_POLYNOMIAL_HALO_OUTPUT_BRANCHES - 1,
                ..FourLoopPolynomialHaloConfig::default()
            },
            "polynomial output branches",
            FOUR_LOOP_POLYNOMIAL_HALO_OUTPUT_BRANCHES,
            FOUR_LOOP_POLYNOMIAL_HALO_OUTPUT_BRANCHES - 1,
        ),
    ] {
        let limited = make_limited(config);
        assert!(matches!(
            limited.map_authenticated_raw_term(&seed, &repeated),
            Err(FourLoopPolynomialHaloError::ResourceLimit {
                resource: actual_resource,
                requested: actual_requested,
                limit: actual_limit,
            }) if actual_resource == resource
                && actual_requested == requested as u128
                && actual_limit == limit as u128
        ));
    }

    let map = mapper.map_authenticated_raw_term(&seed, &repeated).unwrap();
    let bad_fingerprint =
        map.with_source_family_fingerprint_for_replay("tampered-family".to_owned());
    assert!(matches!(
        mapper.replay_polynomial_map(&classifier, &witness, &bad_fingerprint),
        Err(FourLoopPolynomialHaloError::PolynomialReplayMismatch)
    ));

    let one = mapper
        .affine_mapper()
        .reference_family()
        .coefficients()
        .one();
    let image = &map.factor_images()[0];
    let bad_image = image.with_constant_for_replay(image.constant() + &one);
    let bad_factors = map.with_factor_image_for_replay(0, bad_image);
    assert!(matches!(
        mapper.replay_polynomial_map(&classifier, &witness, &bad_factors),
        Err(FourLoopPolynomialHaloError::PolynomialReplayMismatch)
    ));

    let (&monomial, coefficient) = map.collected_monomials().first_key_value().unwrap();
    let bad_polynomial = map.with_monomial_coefficient_for_replay(monomial, coefficient + &one);
    assert!(matches!(
        mapper.replay_polynomial_map(&classifier, &witness, &bad_polynomial),
        Err(FourLoopPolynomialHaloError::PolynomialReplayMismatch)
    ));

    let mut bad_powers: [i32; BASIS] = map.branches()[0].integral().powers().try_into().unwrap();
    bad_powers[0] += 1;
    let bad_branch = map.branches()[0].with_integral_for_replay(Integral::from(bad_powers));
    let bad_branches = map.with_branch_for_replay(0, bad_branch);
    assert!(matches!(
        mapper.replay_polynomial_map(&classifier, &witness, &bad_branches),
        Err(FourLoopPolynomialHaloError::PolynomialReplayMismatch)
    ));
}

// Restricted Symbolica remains serialized, so the exhaustive exact manifest,
// nonidentity maps, independent convolution, failures, limits, and tamper
// replay share one integration test.
#[test]
fn exact_degree_two_four_loop_affine_polynomial_halo() {
    check_exhaustive_manifest();
    check_nonidentity_convolution_and_replay();
    check_rejections_limits_and_tampering();
}
