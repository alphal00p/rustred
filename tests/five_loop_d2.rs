#![cfg(feature = "legacy-authored-oracles")]

use rustred::{FiveLoopBananaD2Config, FiveLoopBananaD2Error, FiveLoopBananaD2Reducer, Integral};

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

fn scalar(physical: [i32; 6]) -> Integral {
    let mut powers = vec![0; 15];
    powers[..6].copy_from_slice(&physical);
    Integral::new(powers)
}

#[test]
fn certified_five_loop_banana_d2_scalar_box() {
    let reducer = FiveLoopBananaD2Reducer::build(FiveLoopBananaD2Config::default()).unwrap();
    let context = reducer.family().coefficients();
    let corner = reducer.boundary().top_master();
    let candidate = reducer.d2_candidate_terminal();

    // Exactly two labelled top-sector D=2 orbits: six triple-dot placements
    // and fifteen double-double placements.  Every image has the same stable
    // output, while only the latter remains the explicitly named candidate.
    let expected_a2_candidate = context.parse("-5/2").unwrap();
    let expected_a2_corner = context.parse("(25*d^2-130*d+168)/(48*m2^2)").unwrap();
    let mut labelled = 0;
    for physical in all_permutations([3, 1, 1, 1, 1, 1]) {
        labelled += 1;
        let reduction = reducer.reduce_integral(&scalar(physical)).unwrap();
        assert_eq!(
            reduction.coefficient(candidate),
            Some(&expected_a2_candidate)
        );
        assert_eq!(reduction.coefficient(corner), Some(&expected_a2_corner));
        assert_eq!(reduction.len(), 2);
    }
    for physical in all_permutations([2, 2, 1, 1, 1, 1]) {
        labelled += 1;
        let reduction = reducer.reduce_integral(&scalar(physical)).unwrap();
        assert_eq!(reduction.len(), 1);
        assert_eq!(reduction.coefficient(candidate), Some(&context.one()));
    }
    assert_eq!(labelled, 21);

    // Algebraic oracle for the separately derived orbit projection of the
    // one-dot seed layer.  The three orbit classes {A2,B2,R} obey E00 and E0j;
    // Ejj is exactly E00+5*E0j.  The displayed two-row matrix has a nonzero
    // 2x2 minor and hence rank two, leaving one free column in this projection.
    // The native replay below verifies the resulting parameterization against
    // every generated row; it does not itself perform orbit projection or rank
    // extraction.  Choosing B2 as the stable candidate and substituting the
    // known one-dot A gives the implementation's A2 and R formulae.
    let m2 = context.parameter("m2").unwrap();
    let e00 = [
        context.scale_integer(&m2, 4),
        context.zero(),
        context.integer(2),
    ];
    let e0j = [
        context.scale_rational(&m2, rustred::ExactRational::new(-4, 5)),
        context.scale_rational(&m2, rustred::ExactRational::new(1, 2)),
        context.rational(rustred::ExactRational::new(-1, 2)),
    ];
    let ejj = [
        &e00[0] + &context.scale_integer(&e0j[0], 5),
        &e00[1] + &context.scale_integer(&e0j[1], 5),
        &e00[2] + &context.scale_integer(&e0j[2], 5),
    ];
    assert_eq!(ejj[0], context.zero());
    assert_eq!(
        ejj[1],
        context.scale_rational(&m2, rustred::ExactRational::new(5, 2))
    );
    assert_eq!(ejj[2], context.rational(rustred::ExactRational::new(-1, 2)));
    let rank_minor = &e00[0] * &e0j[2] - &e00[2] * &e0j[0];
    assert_eq!(
        rank_minor,
        context.scale_rational(&m2, rustred::ExactRational::new(-2, 5))
    );
    assert!(!rank_minor.is_zero());

    // The public A2 formula is homogeneous term by term: A2/B2 is
    // dimensionless, while A2/M has numerator degree two in d and denominator
    // degree two in m2 (the mass-squared parameter).
    assert_eq!(context.parameter_names(), &["d", "m2"]);
    assert_eq!(expected_a2_candidate.numerator.degree(0), 0);
    assert_eq!(expected_a2_candidate.denominator.degree(1), 0);
    assert_eq!(expected_a2_corner.numerator.degree(0), 2);
    assert_eq!(expected_a2_corner.numerator.degree(1), 0);
    assert_eq!(expected_a2_corner.denominator.degree(0), 0);
    assert_eq!(expected_a2_corner.denominator.degree(1), 2);

    // Every one of the 25 native raw rows is generated independently and
    // reduced through the oriented-line moment halo to literal zero.
    reducer.validate_raw_ibp_provenance().unwrap();

    // D<=1 and proper sectors are composed through the existing exact
    // boundary service rather than duplicated in this module.
    assert_eq!(
        reducer
            .reduce_integral(&scalar([2, 1, 1, 1, 1, 1]))
            .unwrap()
            .coefficient(corner),
        Some(&context.parse("(12-5*d)/(12*m2)").unwrap())
    );
    let product = reducer
        .reduce_integral(&scalar([2, 1, 1, 1, 1, 0]))
        .unwrap();
    assert_eq!(product.len(), 1);
    assert_eq!(
        product.coefficient(reducer.boundary().product_master()),
        Some(&context.parse("(2-d)/(2*m2)").unwrap())
    );

    // Public scope, work limits, and shift-domain failures are typed before
    // caller-controlled coefficient construction.
    assert!(matches!(
        reducer.reduce_integral(&scalar([4, 1, 1, 1, 1, 1])),
        Err(FiveLoopBananaD2Error::OutOfCoverage {
            dot_degree: 3,
            maximum: 2,
        })
    ));
    let mut numerator = [1; 15];
    numerator[6..].fill(0);
    numerator[6] = -1;
    assert!(matches!(
        reducer.reduce_integral(&Integral::from(numerator)),
        Err(FiveLoopBananaD2Error::NumeratorOrPositiveAuxiliary {
            position: 6,
            power: -1,
        })
    ));
    let capped = FiveLoopBananaD2Reducer::build(FiveLoopBananaD2Config {
        max_explicit_formula_terms: 3,
        ..FiveLoopBananaD2Config::default()
    })
    .unwrap();
    assert!(matches!(
        capped.reduce_integral(&scalar([3, 1, 1, 1, 1, 1])),
        Err(FiveLoopBananaD2Error::ResourceLimit {
            resource: "explicit formula terms",
            requested: 4,
            limit: 3,
        })
    ));
    // The formula quota is local to paths that construct the A2/halo formula.
    // Returning B2 or delegating a factorized D=2 subsector must not consume it.
    assert!(capped.reduce_integral(&scalar([2, 2, 1, 1, 1, 1])).is_ok());
    assert!(capped.reduce_integral(&scalar([3, 1, 1, 1, 1, 0])).is_ok());
    let provenance_capped = FiveLoopBananaD2Reducer::build(FiveLoopBananaD2Config {
        max_provenance_operations: 4_095,
        ..FiveLoopBananaD2Config::default()
    })
    .unwrap();
    assert!(matches!(
        provenance_capped.validate_raw_ibp_provenance(),
        Err(FiveLoopBananaD2Error::ResourceLimit {
            resource: "raw-IBP provenance operations",
            requested: 4_096,
            limit: 4_095,
        })
    ));
    let symmetry_capped = FiveLoopBananaD2Reducer::build(FiveLoopBananaD2Config {
        max_symmetry_steps: 0,
        ..FiveLoopBananaD2Config::default()
    })
    .unwrap();
    assert!(matches!(
        symmetry_capped.reduce_integral(&scalar([1, 1, 1, 1, 1, 3])),
        Err(FiveLoopBananaD2Error::ResourceLimit {
            resource: "adjacent symmetry steps",
            requested: 5,
            limit: 0,
        })
    ));
}
