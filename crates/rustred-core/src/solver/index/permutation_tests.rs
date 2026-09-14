use super::*;

#[test]
fn permutation_requires_a_bijection_and_normalizes_identity() {
    let order = IntegralOrder::new([true, false, true], [false; 3]);
    assert!(matches!(
        order.with_permutation([0, 1, 1]),
        Err(SolverError::InvalidInput(_))
    ));
    assert!(matches!(
        order.with_permutation([0, 1, 3]),
        Err(SolverError::InvalidInput(_))
    ));
    assert!(matches!(
        order.with_permutation([usize::MAX, 1, 2]),
        Err(SolverError::InvalidInput(_))
    ));
    assert_eq!(order.with_permutation([0, 1, 2]).unwrap(), order);
    let permuted = order.with_permutation([2, 0, 1]).unwrap();
    assert_eq!(permuted.permutation(), Some(&[2, 0, 1]));
    assert_eq!(permuted.sector(), order.sector());
    assert_eq!(permuted.deltas(), order.deltas());
    assert_eq!(permuted.with_permutation([0, 1, 2]).unwrap(), order);
    assert_eq!(
        IntegralOrder::<0>::new([], [])
            .with_permutation([])
            .unwrap()
            .permutation(),
        None
    );
}

#[test]
fn permutation_changes_only_final_denominator_and_numerator_ties() {
    for symbolic in [false, true] {
        let integral = |values| {
            if symbolic {
                Integral::<3>::symbolic(values).unwrap()
            } else {
                Integral::<3>::numeric(values).unwrap()
            }
        };
        let denominator = IntegralOrder::new([true, true, false], [false; 3]);
        let left = integral([1, 2, -1]);
        let right = integral([2, 1, -1]);
        assert_eq!(denominator.compare(&left, &right), Ordering::Less);
        assert_eq!(
            denominator
                .with_permutation([1, 0, 2])
                .unwrap()
                .compare(&left, &right),
            Ordering::Greater
        );

        let numerator = IntegralOrder::new([true, false, false], [false; 3]);
        let left = integral([1, -1, -2]);
        let right = integral([1, -2, -1]);
        assert_eq!(numerator.compare(&left, &right), Ordering::Less);
        assert_eq!(
            numerator
                .with_permutation([0, 2, 1])
                .unwrap()
                .compare(&left, &right),
            Ordering::Greater
        );
    }
}

#[test]
fn permutation_preserves_sector_degree_and_numerator_total_priorities() {
    let order = IntegralOrder::new([true, true, false], [false; 3]);
    let permuted = order.with_permutation([2, 1, 0]).unwrap();
    for (hard, easy) in [
        ([1, 1, 0], [8, 0, -8]),  // denominator count
        ([1, 0, 0], [0, 9, -9]),  // original-coordinate sector lexicographic order
        ([2, 1, -2], [1, 1, -2]), // total absolute power
        ([1, 1, -2], [2, 1, -1]), // total numerator power
    ] {
        let (hard, easy) = (
            Integral::numeric(hard).unwrap(),
            Integral::numeric(easy).unwrap(),
        );
        assert_eq!(order.compare(&hard, &easy), Ordering::Less);
        assert_eq!(permuted.compare(&hard, &easy), Ordering::Less);
        assert_eq!(permuted.compare(&easy, &hard), Ordering::Greater);
    }
    for (hard, easy) in [([2, 1, -2], [1, 1, -2]), ([1, 1, -2], [2, 1, -1])] {
        assert_eq!(
            permuted.compare(
                &Integral::symbolic(hard).unwrap(),
                &Integral::symbolic(easy).unwrap()
            ),
            Ordering::Less
        );
    }
}

#[test]
fn permutation_keeps_cut_priority_in_original_coordinate_order() {
    let order = IntegralOrder::new([true, true, false], [true, true, false]);
    let permuted = order.with_permutation([1, 0, 2]).unwrap();
    for symbolic in [false, true] {
        let integral = |values| {
            if symbolic {
                Integral::<3>::symbolic(values).unwrap()
            } else {
                Integral::<3>::numeric(values).unwrap()
            }
        };
        // Swapping the cut priorities would reverse this comparison.
        let (left, right) = (integral([2, 1, 0]), integral([1, 2, 0]));
        assert_eq!(order.compare(&left, &right), Ordering::Less);
        assert_eq!(permuted.compare(&left, &right), Ordering::Less);
    }
    // Numeric cut priority uses absolute values even for nonpositive powers.
    let (left, right) = (
        Integral::numeric([-2, -1, 0]).unwrap(),
        Integral::numeric([-1, -2, 0]).unwrap(),
    );
    assert_eq!(order.compare(&left, &right), Ordering::Less);
    assert_eq!(permuted.compare(&left, &right), Ordering::Less);
}

#[test]
fn permutation_handles_mixed_fixed_and_symbolic_patterns() {
    let make = |fixed, first, second| {
        Integral::new([
            Power::new(false, fixed).unwrap(),
            Power::new(true, first).unwrap(),
            Power::new(true, second).unwrap(),
        ])
    };
    let order = IntegralOrder::new([true, true, true], [true, false, false]);
    let (left, right) = (make(1, 0, 1), make(1, 1, 0));
    assert_eq!(order.compare(&left, &right), Ordering::Less);
    let permuted = order.with_permutation([2, 0, 1]).unwrap();
    assert_eq!(permuted.compare(&left, &right), Ordering::Greater);
    assert_eq!(permuted.compare(&left, &left), Ordering::Equal);
}

#[test]
fn dynamic_orders_match_all_six_compiled_cpp_permutations() {
    // Independently generated using the actual vendored intLessDynamic<3>
    // (GCC, C++20, -O2). Half the samples explicitly tie the aggregate
    // criteria so that this also exercises both permuted coordinate passes;
    // the rest cover arbitrary encoded powers and mixed coordinate patterns.
    let fixtures = [
        ([0, 1, 2], 6_292_230_989_086_773_244u64, 0),
        ([0, 2, 1], 6_490_232_701_391_373_722, 105),
        ([1, 0, 2], 592_787_499_437_584_584, 6_482),
        ([1, 2, 0], 9_168_153_296_072_243_788, 6_600),
        ([2, 0, 1], 6_028_647_411_123_298_218, 6_717),
        ([2, 1, 0], 16_150_967_943_978_861_976, 6_838),
    ];
    let byte = |order| match order {
        Ordering::Less => 0u64,
        Ordering::Equal => 1,
        Ordering::Greater => 2,
    };
    for (permutation, expected_hash, expected_changed) in fixtures {
        let mut random = 0x5245_5350_4952_4544u64;
        let mut next = || {
            random = random
                .wrapping_mul(6_364_136_223_846_793_005)
                .wrapping_add(1_442_695_040_888_963_407);
            random
        };
        let mut hash = 14_695_981_039_346_656_037u64;
        let mut changed = 0;
        for sample in 0..20_000 {
            let flags = next();
            let mut sector = [false; 3];
            let mut deltas = [false; 3];
            let mut left = [Power::default(); 3];
            let mut right = [Power::default(); 3];
            for index in 0..3 {
                let symbolic = (flags >> index) & 1 != 0;
                sector[index] = (flags >> (index + 3)) & 1 != 0;
                deltas[index] = (flags >> (index + 6)) & 1 != 0 && (!symbolic || sector[index]);
                left[index] = Power::new(symbolic, ((next() >> 32) & 127) as i16 - 64).unwrap();
                right[index] = Power::new(symbolic, ((next() >> 32) & 127) as i16 - 64).unwrap();
            }
            if sample % 4 < 2 {
                let symbolic = sample % 4 == 0;
                sector.fill(symbolic);
                deltas.fill(false);
                for power in &mut left {
                    let value = power.value();
                    *power =
                        Power::new(symbolic, if symbolic { value } else { -value.abs() }).unwrap();
                }
                for index in 0..3 {
                    right[index] = left[(index + 1) % 3];
                }
            }
            let (left, right) = (Integral::new(left), Integral::new(right));
            let original = IntegralOrder::new(sector, deltas);
            let permuted = original.with_permutation(permutation).unwrap();
            let actual = byte(permuted.compare(&left, &right));
            changed += usize::from(actual != byte(original.compare(&left, &right)));
            hash = (hash ^ actual).wrapping_mul(1_099_511_628_211);
        }
        assert_eq!(hash, expected_hash, "permutation {permutation:?}");
        assert_eq!(changed, expected_changed, "permutation {permutation:?}");
    }
}
