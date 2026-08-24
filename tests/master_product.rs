use rustred::{
    CoefficientContext, MasterProduct, MasterProductError, ProductConvolutionError,
    ProductLinearCombination,
};

fn product(factors: &[&str]) -> MasterProduct<String> {
    MasterProduct::try_from_factors(factors.iter().map(|factor| (*factor).to_owned())).unwrap()
}

// Keep all Symbolica-backed coefficient checks on one restricted worker.
#[test]
fn canonical_master_products_and_checked_exact_convolution() {
    let canonical = MasterProduct::try_from_factors([
        "tadpole".to_owned(),
        "sunset".to_owned(),
        "tadpole".to_owned(),
    ])
    .unwrap();
    assert_eq!(
        canonical
            .factors()
            .iter()
            .map(|(factor, multiplicity)| (factor.as_str(), *multiplicity))
            .collect::<Vec<_>>(),
        vec![("sunset", 1), ("tadpole", 2)]
    );
    assert_eq!(canonical.total_factor_count(), 3);
    assert_eq!(canonical.to_string(), "sunset*tadpole^2");
    assert!(MasterProduct::<String>::identity().is_identity());

    let merged = MasterProduct::try_from_multiplicities([
        ("sunset".to_owned(), 1),
        ("ignored".to_owned(), 0),
        ("sunset".to_owned(), 2),
    ])
    .unwrap();
    assert_eq!(merged.multiplicity(&"sunset".to_owned()), 3);
    assert_eq!(merged.distinct_factor_count(), 1);
    assert!(matches!(
        MasterProduct::try_from_multiplicities([
            ("tadpole".to_owned(), u32::MAX),
            ("tadpole".to_owned(), 1),
        ]),
        Err(MasterProductError::MultiplicityOverflow {
            current: u32::MAX,
            added: 1,
        })
    ));

    let context = CoefficientContext::new(["d", "m2"]);
    let tadpole = product(&["T"]);
    let sunset = product(&["S"]);
    let mut left = ProductLinearCombination::new();
    left.add_term(tadpole.clone(), context.integer(2));
    left.add_term(sunset.clone(), context.integer(3));
    let mut right = ProductLinearCombination::new();
    right.add_term(tadpole.clone(), context.integer(5));
    right.add_term(sunset.clone(), context.integer(7));

    let scaled = left.scaled(&context.integer(2));
    assert_eq!(scaled.coefficient(&tadpole), Some(&context.integer(4)));
    assert_eq!(scaled.coefficient(&sunset), Some(&context.integer(6)));
    let mut cancelled_combination = left.clone();
    cancelled_combination.add_scaled(&left, &context.integer(-1));
    assert!(cancelled_combination.is_zero());

    let convolution = left.checked_convolve(&right, 3).unwrap();
    assert_eq!(
        left.checked_convolve_with_limits(&right, 3, 4).unwrap(),
        convolution
    );
    assert!(matches!(
        left.checked_convolve_with_limits(&right, 3, 3),
        Err(ProductConvolutionError::PairOperationLimit {
            limit: 3,
            attempted: 4,
        })
    ));
    assert_eq!(convolution.len(), 3);
    assert_eq!(
        convolution.coefficient(&product(&["T", "T"])),
        Some(&context.integer(10))
    );
    assert_eq!(
        convolution.coefficient(&product(&["S", "T"])),
        Some(&context.integer(29))
    );
    assert_eq!(
        convolution.coefficient(&product(&["S", "S"])),
        Some(&context.integer(21))
    );
    assert!(matches!(
        left.checked_convolve(&right, 2),
        Err(ProductConvolutionError::TermLimit {
            limit: 2,
            attempted: 3,
        })
    ));

    let mut cancellation =
        ProductLinearCombination::from_term(tadpole.clone(), context.parameter("d").unwrap());
    cancellation.add_term(tadpole, -context.parameter("d").unwrap());
    assert!(cancellation.is_zero());
    assert!(
        ProductLinearCombination::<String>::new()
            .checked_convolve(&left, 0)
            .unwrap()
            .is_zero()
    );
    assert!(
        ProductLinearCombination::<String>::new()
            .checked_convolve_with_limits(&left, 0, 0)
            .unwrap()
            .is_zero()
    );

    let one_left = ProductLinearCombination::from_term(product(&["T"]), context.integer(1));
    let one_right = ProductLinearCombination::from_term(product(&["S"]), context.integer(1));
    assert!(matches!(
        one_left.checked_convolve_with_limits(&one_right, 1, 0),
        Err(ProductConvolutionError::PairOperationLimit {
            limit: 0,
            attempted: 1,
        })
    ));
    assert!(matches!(
        one_left.checked_convolve_with_limits(&one_right, 0, 1),
        Err(ProductConvolutionError::TermLimit {
            limit: 0,
            attempted: 1,
        })
    ));

    let maximum = MasterProduct::try_from_multiplicities([("T".to_owned(), u32::MAX)]).unwrap();
    let one_more = product(&["T"]);
    let maximum_combination = ProductLinearCombination::from_term(maximum, context.integer(1));
    let one_more_combination = ProductLinearCombination::from_term(one_more, context.integer(1));
    assert!(matches!(
        maximum_combination.checked_convolve(&one_more_combination, 1),
        Err(ProductConvolutionError::FactorMultiplicity(
            MasterProductError::MultiplicityOverflow {
                current: u32::MAX,
                added: 1,
            }
        ))
    ));
}
