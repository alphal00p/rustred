use rustred::{CoefficientContext, IbpGenerator, Integral, LinearCombination};
use rustred_legacy_oracles::{
    FIVE_LOOP_BANANA_AUXILIARY_LINE_PAIRS, FIVE_LOOP_BANANA_S6_ADJACENT_TRANSPOSITIONS,
    FIVE_LOOP_BANANA_S6_ORDER, FiveLoopBananaBoundaryConfig, FiveLoopBananaBoundaryError,
    FiveLoopBananaBoundaryReducer, FiveLoopBananaPermutationError,
    FiveLoopBananaPhysicalPermutation, FiveLoopBananaScalarClass, equal_mass_five_loop_banana,
    five_loop_banana_oriented_line_routing, five_loop_banana_physical_orbit_witness,
};

fn determinant(matrix: [[i8; 5]; 5]) -> i32 {
    fn visit(
        row: usize,
        matrix: &[[i8; 5]; 5],
        used: &mut [bool; 5],
        columns: &mut [usize; 5],
        total: &mut i32,
    ) {
        if row == 5 {
            let inversions = (0..5)
                .flat_map(|left| (left + 1..5).map(move |right| (left, right)))
                .filter(|&(left, right)| columns[left] > columns[right])
                .count();
            let product = (0..5)
                .map(|index| i32::from(matrix[index][columns[index]]))
                .product::<i32>();
            *total += if inversions % 2 == 0 {
                product
            } else {
                -product
            };
            return;
        }
        for column in 0..5 {
            if !used[column] {
                used[column] = true;
                columns[row] = column;
                visit(row + 1, matrix, used, columns, total);
                used[column] = false;
            }
        }
    }

    let mut total = 0;
    visit(0, &matrix, &mut [false; 5], &mut [0; 5], &mut total);
    total
}

fn sp_index(left: usize, right: usize) -> usize {
    let (left, right) = if left <= right {
        (left, right)
    } else {
        (right, left)
    };
    (0..left).map(|row| 5 - row).sum::<usize>() + right - left
}

fn next_permutation(values: &mut [usize; 6]) -> bool {
    let Some(pivot) = (0..values.len() - 1)
        .rev()
        .find(|&position| values[position] < values[position + 1])
    else {
        return false;
    };
    let swap = (pivot + 1..values.len())
        .rev()
        .find(|&position| values[pivot] < values[position])
        .unwrap();
    values.swap(pivot, swap);
    values[pivot + 1..].reverse();
    true
}

fn check_full_s6_proof_surface() {
    assert_eq!(FIVE_LOOP_BANANA_S6_ADJACENT_TRANSPOSITIONS.len(), 5);
    for (generator, permutation) in FIVE_LOOP_BANANA_S6_ADJACENT_TRANSPOSITIONS
        .iter()
        .enumerate()
    {
        assert_eq!(permutation.adjacent_generator_word(), vec![generator]);
        assert_eq!(permutation.inverse(), *permutation);
        assert_eq!(permutation.determinant_sign(), -1);
    }

    let mut sources = [0, 1, 2, 3, 4, 5];
    let mut count = 0;
    loop {
        let permutation = FiveLoopBananaPhysicalPermutation::try_new(sources).unwrap();
        let powers = [10, 11, 12, 13, 14, 15];
        assert_eq!(
            permutation.apply_physical_powers(powers),
            sources.map(|source| powers[source])
        );
        assert_eq!(
            determinant(permutation.unimodular_loop_map()),
            i32::from(permutation.determinant_sign())
        );
        assert_eq!(
            permutation.followed_by(permutation.inverse()),
            FiveLoopBananaPhysicalPermutation::identity()
        );

        let replayed = permutation.adjacent_generator_word().into_iter().fold(
            FiveLoopBananaPhysicalPermutation::identity(),
            |current, generator| {
                current.followed_by(FIVE_LOOP_BANANA_S6_ADJACENT_TRANSPOSITIONS[generator])
            },
        );
        assert_eq!(replayed, permutation);

        // The sixth transformed oriented line is minus the sum of the five
        // exposed rows and equals the remaining permuted oriented line.
        let map = permutation.unimodular_loop_map();
        let induced_sixth: [i8; 5] =
            std::array::from_fn(|column| -map.iter().map(|row| row[column]).sum::<i8>());
        assert_eq!(
            induced_sixth,
            five_loop_banana_oriented_line_routing(sources[5]).unwrap()
        );

        count += 1;
        if !next_permutation(&mut sources) {
            break;
        }
    }
    assert_eq!(count, FIVE_LOOP_BANANA_S6_ORDER);

    let witness = five_loop_banana_physical_orbit_witness([1, 4, 0, 2, 2, 3]);
    assert_eq!(witness.canonical(), &[4, 3, 2, 2, 1, 0]);
    assert_eq!(
        witness
            .permutation()
            .apply_physical_powers(*witness.original()),
        *witness.canonical()
    );
    assert!(matches!(
        FiveLoopBananaPhysicalPermutation::try_new([0, 1, 2, 3, 4, 6]),
        Err(FiveLoopBananaPermutationError::SourceOutOfRange {
            position: 5,
            source: 6,
        })
    ));
    assert!(matches!(
        FiveLoopBananaPhysicalPermutation::try_new([0, 1, 2, 3, 4, 4]),
        Err(FiveLoopBananaPermutationError::DuplicateSource { source: 4 })
    ));
}

fn check_all_corner_masks(reducer: &FiveLoopBananaBoundaryReducer) {
    for mask in 0_u8..64 {
        let mut powers = vec![0; 15];
        for (line, power) in powers[..6].iter_mut().enumerate() {
            *power = if mask & (1_u8 << line) != 0 { 1 } else { 0 };
        }
        let integral = Integral::new(powers);
        let active = mask.count_ones() as usize;
        let class = reducer.classify_integral(&integral).unwrap();
        let reduction = reducer.reduce_integral(&integral).unwrap();
        match active {
            0..=4 => {
                assert_eq!(
                    class,
                    FiveLoopBananaScalarClass::ScalelessPinch {
                        sector_mask: mask,
                        active_lines: active,
                    }
                );
                assert!(reduction.is_zero());
            }
            5 => {
                assert!(matches!(
                    class,
                    FiveLoopBananaScalarClass::UnimodularProduct {
                        tadpole_steps: 0,
                        ..
                    }
                ));
                assert_eq!(
                    reduction.coefficient(reducer.product_master()),
                    Some(&reducer.family().coefficients().one())
                );
                assert_eq!(reduction.len(), 1);
            }
            6 => {
                assert_eq!(class, FiveLoopBananaScalarClass::TopCorner);
                assert_eq!(
                    reduction.coefficient(reducer.top_master()),
                    Some(&reducer.family().coefficients().one())
                );
                assert_eq!(reduction.len(), 1);
            }
            _ => unreachable!(),
        }
    }
}

fn check_products_and_top_orbit(reducer: &FiveLoopBananaBoundaryReducer) {
    let expected_product = reducer
        .family()
        .coefficients()
        .parse("(2-d)^2*(4-d)/(16*m2^3)")
        .unwrap();
    for missing in 0..6 {
        let mut powers = vec![0; 15];
        powers[..6].fill(1);
        powers[missing] = 0;
        let active: Vec<_> = (0..6).filter(|&line| line != missing).collect();
        powers[active[0]] = 2;
        powers[active[1]] = 3;
        let integral = Integral::new(powers);
        let reduction = reducer.reduce_integral(&integral).unwrap();
        assert_eq!(
            reduction.coefficient(reducer.product_master()),
            Some(&expected_product),
            "missing physical line {missing}"
        );
        assert_eq!(reduction.len(), 1);
    }

    let expected_dot = reducer
        .family()
        .coefficients()
        .parse("(12-5*d)/(12*m2)")
        .unwrap();
    for dotted in 0..6 {
        let mut powers = vec![0; 15];
        powers[..6].fill(1);
        powers[dotted] = 2;
        let integral = Integral::new(powers);
        assert!(matches!(
            reducer.classify_integral(&integral).unwrap(),
            FiveLoopBananaScalarClass::TopOneDot {
                dotted_line,
                ..
            } if dotted_line == dotted
        ));
        let reduction = reducer.reduce_integral(&integral).unwrap();
        assert_eq!(
            reduction.coefficient(reducer.top_master()),
            Some(&expected_dot)
        );
        assert_eq!(reduction.len(), 1);
    }
}

fn expected_top_numerator(
    reducer: &FiveLoopBananaBoundaryReducer,
    dotted: Option<usize>,
    numerator_position: usize,
) -> LinearCombination {
    let coefficients = reducer.family().coefficients();
    let mut expected = LinearCombination::new();
    match dotted {
        None => {
            expected.add_term(
                reducer.top_master().clone(),
                coefficients.parse("m2/5").unwrap(),
            );
            expected.add_term(
                reducer.product_master().clone(),
                coefficients.parse("-1/5").unwrap(),
            );
        }
        Some(dotted)
            if FIVE_LOOP_BANANA_AUXILIARY_LINE_PAIRS[numerator_position - 6].contains(&dotted) =>
        {
            expected.add_term(
                reducer.top_master().clone(),
                coefficients.parse("-d/12").unwrap(),
            );
        }
        Some(_) => {
            expected.add_term(
                reducer.top_master().clone(),
                coefficients.parse("(3-d)/12").unwrap(),
            );
            expected.add_term(
                reducer.product_master().clone(),
                coefficients.parse("(d-2)/(8*m2)").unwrap(),
            );
        }
    }
    expected
}

fn check_complete_dn11_target_box(reducer: &FiveLoopBananaBoundaryReducer) {
    let mut labelled_targets = 0_usize;
    let mut scaleless = 0_usize;
    let mut nonzero = 0_usize;
    for mask in 0_u8..64 {
        let active: Vec<_> = (0..6).filter(|line| mask & (1_u8 << line) != 0).collect();
        let inactive: Vec<_> = (0..6).filter(|line| mask & (1_u8 << line) == 0).collect();
        let dot_choices: Vec<Option<usize>> = std::iter::once(None)
            .chain(active.iter().copied().map(Some))
            .collect();
        let numerator_choices: Vec<Option<usize>> = std::iter::once(None)
            .chain(inactive.iter().copied().map(Some))
            .chain((6..15).map(Some))
            .collect();
        for dotted in &dot_choices {
            for numerator in &numerator_choices {
                labelled_targets += 1;
                let mut powers = vec![0; 15];
                for &line in &active {
                    powers[line] = 1;
                }
                if let Some(line) = dotted {
                    powers[*line] += 1;
                }
                if let Some(position) = numerator {
                    powers[*position] = -1;
                }
                let integral = Integral::new(powers);
                let reduction = reducer.reduce_integral(&integral).unwrap();
                assert!(reduction.terms().keys().all(|terminal| {
                    terminal == reducer.top_master() || terminal == reducer.product_master()
                }));
                if active.len() <= 4 {
                    assert!(reduction.is_zero());
                    scaleless += 1;
                    continue;
                }
                if !reduction.is_zero() {
                    nonzero += 1;
                }
                if active.len() == 6 {
                    match (*dotted, *numerator) {
                        (None, None) => assert_eq!(
                            reduction.coefficient(reducer.top_master()),
                            Some(&reducer.family().coefficients().one())
                        ),
                        (Some(_), None) => assert_eq!(
                            reduction.coefficient(reducer.top_master()),
                            Some(
                                &reducer
                                    .family()
                                    .coefficients()
                                    .parse("(12-5*d)/(12*m2)")
                                    .unwrap()
                            )
                        ),
                        (None, Some(position)) => {
                            assert_eq!(reduction, expected_top_numerator(reducer, None, position));
                        }
                        (Some(line), Some(position)) => {
                            assert_eq!(
                                reduction,
                                expected_top_numerator(reducer, Some(line), position)
                            );
                        }
                    }
                } else if active.len() == 5 && numerator.is_none() && dotted.is_none() {
                    assert_eq!(
                        reduction.coefficient(reducer.product_master()),
                        Some(&reducer.family().coefficients().one())
                    );
                }
            }
        }
    }
    assert_eq!(labelled_targets, 3_232);
    assert_eq!(scaleless, 2_766);
    assert_eq!(nonzero, 250);
}

fn check_product_numerator_witnesses(reducer: &FiveLoopBananaBoundaryReducer) {
    for missing in 0..6 {
        for numerator in (0..6).filter(|&line| line == missing).chain(6..15) {
            let mut powers = vec![0; 15];
            powers[..6].fill(1);
            powers[missing] = 0;
            powers[numerator] = -1;
            let integral = Integral::new(powers);
            let witness = reducer
                .product_numerator_witness_for(&integral)
                .unwrap()
                .unwrap();
            assert_eq!(witness.missing_line(), missing);
            assert_eq!(witness.numerator_position(), numerator);
            assert_eq!(determinant(*witness.loop_map()).abs(), 1);
            assert_eq!(witness.active_lines().len(), 5);
            assert_eq!(witness.transformed_quadratic_form().len(), 15);
            let old = reducer.family().denominators()[numerator].quadratic_form();
            let transformed = witness.transformed_quadratic_form();
            for old_left in 0..5 {
                for old_right in old_left..5 {
                    let mut reconstructed = rustred::ExactRational::zero();
                    for new_left in 0..5 {
                        for new_right in new_left..5 {
                            let coefficient = &transformed[sp_index(new_left, new_right)];
                            let pullback = if new_left == new_right {
                                let product = witness.loop_map()[new_left][old_left] as i64
                                    * witness.loop_map()[new_right][old_right] as i64;
                                if old_left == old_right {
                                    product
                                } else {
                                    2 * product
                                }
                            } else {
                                let product = witness.loop_map()[new_left][old_left] as i64
                                    * witness.loop_map()[new_right][old_right] as i64;
                                if old_left == old_right {
                                    product
                                } else {
                                    product
                                        + witness.loop_map()[new_left][old_right] as i64
                                            * witness.loop_map()[new_right][old_left] as i64
                                }
                            };
                            let contribution =
                                coefficient * &rustred::ExactRational::from(pullback);
                            reconstructed = &reconstructed + &contribution;
                        }
                    }
                    assert_eq!(
                        reconstructed,
                        old[sp_index(old_left, old_right)],
                        "missing={missing}, numerator={numerator}, old=({old_left},{old_right}), map={:?}, transformed={transformed:?}",
                        witness.loop_map(),
                    );
                }
            }
        }
    }

    // A mixed moment in independent tadpoles vanishes by parity.
    let zero = reducer
        .reduce_integral(&Integral::from([
            1, 1, 1, 1, 1, 0, -1, 0, 0, 0, 0, 0, 0, 0, 0,
        ]))
        .unwrap();
    assert!(zero.is_zero());
}

fn check_typed_rejections_and_limits(reducer: &FiveLoopBananaBoundaryReducer) {
    let mut powers = vec![0; 15];
    powers[..6].fill(1);
    powers[6] = -1;
    assert_eq!(
        reducer
            .reduce_integral(&Integral::new(powers.clone()))
            .unwrap(),
        expected_top_numerator(reducer, None, 6)
    );
    powers[6] = 1;
    assert!(matches!(
        reducer.reduce_integral(&Integral::new(powers.clone())),
        Err(FiveLoopBananaBoundaryError::PositiveAuxiliary {
            position: 6,
            power: 1,
        })
    ));
    powers[6] = 0;
    powers[0] = -1;
    let reduction = reducer
        .reduce_integral(&Integral::new(powers.clone()))
        .unwrap();
    assert_eq!(
        reduction.coefficient(reducer.product_master()),
        Some(&reducer.family().coefficients().parse("-4*m2").unwrap())
    );

    powers[..6].fill(1);
    powers[0] = 2;
    powers[1] = 2;
    let deep_top = Integral::new(powers.clone());
    assert!(matches!(
        reducer.reduce_integral(&deep_top),
        Err(FiveLoopBananaBoundaryError::UnsupportedTopDots {
            integral,
            dot_degree: 2,
        }) if integral == deep_top
    ));

    powers[..6].fill(1);
    powers[6] = -2;
    let deep_numerator = Integral::new(powers);
    assert!(matches!(
        reducer.reduce_integral(&deep_numerator),
        Err(FiveLoopBananaBoundaryError::UnsupportedNumeratorDegree {
            integral,
            numerator_degree: 2,
        }) if integral == deep_numerator
    ));
    assert!(matches!(
        reducer.reduce_integral(&Integral::from([1, 1, 1, 1, 1, 1])),
        Err(FiveLoopBananaBoundaryError::WrongIntegralArity {
            expected: 15,
            actual: 6,
        })
    ));

    let tadpole_limited = FiveLoopBananaBoundaryReducer::new(
        equal_mass_five_loop_banana().unwrap(),
        FiveLoopBananaBoundaryConfig {
            max_tadpole_steps: 2,
            ..FiveLoopBananaBoundaryConfig::default()
        },
    )
    .unwrap();
    assert!(matches!(
        tadpole_limited.reduce_integral(&Integral::from([
            4, 1, 1, 1, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
        ])),
        Err(FiveLoopBananaBoundaryError::ResourceLimit {
            resource: "tadpole recurrence steps",
            requested: 3,
            limit: 2,
        })
    ));

    let coefficient_degree_limited = FiveLoopBananaBoundaryReducer::new(
        equal_mass_five_loop_banana().unwrap(),
        FiveLoopBananaBoundaryConfig {
            max_tadpole_steps: usize::MAX,
            ..FiveLoopBananaBoundaryConfig::default()
        },
    )
    .unwrap();
    assert!(matches!(
        coefficient_degree_limited.reduce_integral(&Integral::from([
            65_537, 1, 1, 1, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
        ])),
        Err(FiveLoopBananaBoundaryError::ResourceLimit {
            resource: "Symbolica coefficient exponent degree",
            requested: 65_536,
            limit: 65_535,
        })
    ));

    let symmetry_limited = FiveLoopBananaBoundaryReducer::new(
        equal_mass_five_loop_banana().unwrap(),
        FiveLoopBananaBoundaryConfig {
            max_symmetry_steps: 0,
            ..FiveLoopBananaBoundaryConfig::default()
        },
    )
    .unwrap();
    assert!(matches!(
        symmetry_limited.reduce_integral(&Integral::from([
            0, 1, 1, 1, 1, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0,
        ])),
        Err(FiveLoopBananaBoundaryError::ResourceLimit {
            resource: "adjacent symmetry steps",
            requested: 5,
            limit: 0,
        })
    ));

    let combination_limited = FiveLoopBananaBoundaryReducer::new(
        equal_mass_five_loop_banana().unwrap(),
        FiveLoopBananaBoundaryConfig {
            max_combination_terms: 1,
            ..FiveLoopBananaBoundaryConfig::default()
        },
    )
    .unwrap();
    let mut combination = LinearCombination::new();
    combination.add_term(
        combination_limited.product_master().clone(),
        combination_limited.family().coefficients().one(),
    );
    combination.add_term(
        combination_limited.top_master().clone(),
        combination_limited.family().coefficients().one(),
    );
    assert!(matches!(
        combination_limited.reduce_combination(&combination),
        Err(FiveLoopBananaBoundaryError::ResourceLimit {
            resource: "input combination terms",
            requested: 2,
            limit: 1,
        })
    ));

    let aggregate_tadpole_limited = FiveLoopBananaBoundaryReducer::new(
        equal_mass_five_loop_banana().unwrap(),
        FiveLoopBananaBoundaryConfig {
            max_combination_tadpole_steps: 1,
            ..FiveLoopBananaBoundaryConfig::default()
        },
    )
    .unwrap();
    let mut combination = LinearCombination::new();
    combination.add_term(
        Integral::from([2, 1, 1, 1, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0]),
        aggregate_tadpole_limited.family().coefficients().one(),
    );
    combination.add_term(
        Integral::from([1, 2, 1, 1, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0]),
        aggregate_tadpole_limited.family().coefficients().one(),
    );
    assert!(matches!(
        aggregate_tadpole_limited.reduce_combination(&combination),
        Err(FiveLoopBananaBoundaryError::ResourceLimit {
            resource: "combination tadpole recurrence steps",
            requested: 2,
            limit: 1,
        })
    ));

    let aggregate_symmetry_limited = FiveLoopBananaBoundaryReducer::new(
        equal_mass_five_loop_banana().unwrap(),
        FiveLoopBananaBoundaryConfig {
            max_combination_symmetry_steps: 4,
            ..FiveLoopBananaBoundaryConfig::default()
        },
    )
    .unwrap();
    let mut combination = LinearCombination::new();
    combination.add_term(
        Integral::from([0, 1, 1, 1, 1, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0]),
        aggregate_symmetry_limited.family().coefficients().one(),
    );
    assert!(matches!(
        aggregate_symmetry_limited.reduce_combination(&combination),
        Err(FiveLoopBananaBoundaryError::ResourceLimit {
            resource: "combination adjacent symmetry steps",
            requested: 5,
            limit: 4,
        })
    ));

    let algebra_limited = FiveLoopBananaBoundaryReducer::new(
        equal_mass_five_loop_banana().unwrap(),
        FiveLoopBananaBoundaryConfig {
            max_algebra_operations: 4_095,
            ..FiveLoopBananaBoundaryConfig::default()
        },
    )
    .unwrap();
    assert!(matches!(
        algebra_limited.reduce_integral(&Integral::from([
            0, 1, 1, 1, 1, 1, -1, 0, 0, 0, 0, 0, 0, 0, 0,
        ])),
        Err(FiveLoopBananaBoundaryError::ResourceLimit {
            resource: "exact algebra operations",
            requested: 4_096,
            limit: 4_095,
        })
    ));

    let scalar_algebra_limited = FiveLoopBananaBoundaryReducer::new(
        equal_mass_five_loop_banana().unwrap(),
        FiveLoopBananaBoundaryConfig {
            max_algebra_operations: 63,
            ..FiveLoopBananaBoundaryConfig::default()
        },
    )
    .unwrap();
    assert!(matches!(
        scalar_algebra_limited.reduce_integral(&Integral::from([
            2, 1, 1, 1, 1, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0,
        ])),
        Err(FiveLoopBananaBoundaryError::ResourceLimit {
            resource: "exact algebra operations",
            requested: 64,
            limit: 63,
        })
    ));

    let aggregate_algebra_limited = FiveLoopBananaBoundaryReducer::new(
        equal_mass_five_loop_banana().unwrap(),
        FiveLoopBananaBoundaryConfig {
            max_combination_algebra_operations: 7_000,
            ..FiveLoopBananaBoundaryConfig::default()
        },
    )
    .unwrap();
    let mut combination = LinearCombination::new();
    combination.add_term(
        Integral::from([0, 1, 1, 1, 1, 1, -1, 0, 0, 0, 0, 0, 0, 0, 0]),
        aggregate_algebra_limited.family().coefficients().one(),
    );
    combination.add_term(
        Integral::from([1, 0, 1, 1, 1, 1, 0, -1, 0, 0, 0, 0, 0, 0, 0]),
        aggregate_algebra_limited.family().coefficients().one(),
    );
    assert!(matches!(
        aggregate_algebra_limited.reduce_combination(&combination),
        Err(FiveLoopBananaBoundaryError::ResourceLimit {
            resource: "combination exact algebra operations",
            requested: 8_192,
            limit: 7_000,
        })
    ));

    // Combination coefficients are caller-controlled.  Their product with a
    // valid one-dot reduction must be preflighted before Symbolica attempts to
    // construct exponent 65,536 in its u16 polynomial domain.
    let mut one_dot = [1_i32; 15];
    one_dot[6..].fill(0);
    one_dot[0] = 2;
    let mut exponent_product = LinearCombination::new();
    exponent_product.add_term(
        Integral::from(one_dot),
        reducer.family().coefficients().parse("d^65535").unwrap(),
    );
    assert!(matches!(
        reducer.reduce_combination(&exponent_product),
        Err(FiveLoopBananaBoundaryError::ResourceLimit {
            resource: "Symbolica coefficient exponent degree",
            requested: 65_536,
            limit: 65_535,
        })
    ));

    // Each scaling is separately representable, but merging two one-dot
    // images would cross-multiply relatively prime degree-65,534 and degree-2
    // denominators.  Check the sum before invoking LinearCombination::add_term.
    let mut other_dot = one_dot;
    other_dot[0] = 1;
    other_dot[1] = 2;
    let mut exponent_merge = LinearCombination::new();
    exponent_merge.add_term(
        Integral::from(one_dot),
        reducer.family().coefficients().parse("1/d^65534").unwrap(),
    );
    exponent_merge.add_term(
        Integral::from(other_dot),
        reducer.family().coefficients().parse("1/(d+1)^2").unwrap(),
    );
    assert!(matches!(
        reducer.reduce_combination(&exponent_merge),
        Err(FiveLoopBananaBoundaryError::ResourceLimit {
            resource: "Symbolica coefficient exponent degree",
            requested: 65_536,
            limit: 65_535,
        })
    ));

    // A foreign variable map is not silently unified during checked
    // combination arithmetic.
    let foreign = CoefficientContext::new(["foreign"]);
    let mut mismatched_variables = LinearCombination::new();
    mismatched_variables.add_term(reducer.top_master().clone(), foreign.one());
    assert!(matches!(
        reducer.reduce_combination(&mismatched_variables),
        Err(FiveLoopBananaBoundaryError::ResourceLimit {
            resource: "Symbolica coefficient exponent degree",
            requested: u128::MAX,
            limit: 65_535,
        })
    ));
}

fn check_raw_trace_ibp(reducer: &FiveLoopBananaBoundaryReducer) {
    let context = reducer.family().coefficients();
    let identities = IbpGenerator::new(reducer.family()).generate_raw(reducer.top_master());
    let mut trace = LinearCombination::new();
    for identity in identities
        .iter()
        .filter(|identity| identity.differentiated_loop == identity.contraction_loop)
    {
        trace.add_scaled(&identity.equation, &context.one());
    }

    // Euler homogeneity of the six physical quadratics makes all auxiliary
    // numerator terms cancel in the generated trace before reduction.
    assert_eq!(trace.len(), 7);
    assert_eq!(
        trace.coefficient(reducer.top_master()),
        Some(&context.parse("5*d-12").unwrap())
    );
    for dotted in 0..6 {
        let mut powers = reducer.top_master().powers().to_vec();
        powers[dotted] = 2;
        assert_eq!(
            trace.coefficient(&Integral::new(powers)),
            Some(&context.parse("2*m2").unwrap())
        );
    }
    assert!(reducer.reduce_combination(&trace).unwrap().is_zero());
}

fn check_all_raw_top_corner_ibps(reducer: &FiveLoopBananaBoundaryReducer) {
    let identities = IbpGenerator::new(reducer.family()).generate_raw(reducer.top_master());
    assert_eq!(identities.len(), 25);
    for identity in identities {
        let reduced = reducer.reduce_combination(&identity.equation).unwrap();
        assert!(
            reduced.is_zero(),
            "raw top-corner IBP ({},{}) did not close: {:?}",
            identity.differentiated_loop,
            identity.contraction_loop,
            reduced
        );
    }
}

// Restricted Symbolica binds an instance to the first OS thread that enters
// it.  Keep all Symbolica-backed checks on one test worker.
#[test]
fn exact_five_loop_banana_boundary_slice() {
    check_full_s6_proof_surface();
    let reducer = FiveLoopBananaBoundaryReducer::new(
        equal_mass_five_loop_banana().unwrap(),
        FiveLoopBananaBoundaryConfig::default(),
    )
    .unwrap();
    assert_eq!(
        reducer.product_master(),
        &Integral::from([1, 1, 1, 1, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0])
    );
    assert_eq!(
        reducer.top_master(),
        &Integral::from([1, 1, 1, 1, 1, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0])
    );

    check_all_corner_masks(&reducer);
    check_products_and_top_orbit(&reducer);
    check_complete_dn11_target_box(&reducer);
    check_product_numerator_witnesses(&reducer);
    check_typed_rejections_and_limits(&reducer);
    check_raw_trace_ibp(&reducer);
    check_all_raw_top_corner_ibps(&reducer);
}
