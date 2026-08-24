use rustred::{
    CoefficientContext, ParametricArithmeticLimits, ParametricCoefficientContext,
    ParametricCoefficientError,
};

#[test]
fn affine_translation_preflights_integer_growth_before_symbolica_expansion() {
    let base = CoefficientContext::new(Vec::<String>::new());
    let context =
        ParametricCoefficientContext::try_new(&base, "translation-bit-preflight", 1).unwrap();
    let n = context.index(0).unwrap();
    let mut power = context.one();
    for _ in 0..20 {
        power = context.mul(&power, &n).unwrap();
    }
    let polynomial = context.numerator_condition(&power).unwrap();
    let limits = ParametricArithmeticLimits {
        max_specialization_integer_bits: 64,
        ..ParametricArithmeticLimits::default()
    };

    assert!(matches!(
        context.translate_polynomial(&polynomial, &[i64::MIN], limits),
        Err(ParametricCoefficientError::ResourceLimit {
            resource: "parametric translation integer bits",
            limit: 64,
            ..
        })
    ));

    // Rejection is read-only and does not damage the authenticated source.
    context
        .validate_polynomial_with_limits(&polynomial, limits.exact_algebra)
        .unwrap();
}

#[test]
fn ordinary_affine_translation_remains_exact() {
    let base = CoefficientContext::new(Vec::<String>::new());
    let context = ParametricCoefficientContext::try_new(&base, "translation-exact", 1).unwrap();
    let n = context.index(0).unwrap();
    let square = context.mul(&n, &n).unwrap();
    let polynomial = context.numerator_condition(&square).unwrap();
    let translated = context
        .translate_polynomial(&polynomial, &[3], ParametricArithmeticLimits::default())
        .unwrap();

    let n_plus_three = context.add(&n, &context.integer(3)).unwrap();
    let expected = context.mul(&n_plus_three, &n_plus_three).unwrap();
    assert_eq!(
        translated.to_expression(),
        context
            .numerator_condition(&expected)
            .unwrap()
            .to_expression()
    );
}
