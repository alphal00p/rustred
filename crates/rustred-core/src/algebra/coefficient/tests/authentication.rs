use std::sync::atomic::{AtomicUsize, Ordering};

use symbolica::{
    atom::{NamespacedSymbol, SymbolAttribute, SymbolBuilder, UserData},
    prelude::Integer,
};

use super::super::{
    CoefficientContext, CoefficientContextError, CoefficientPolynomialPart, ExactAlgebraError,
    ExactAlgebraLimits,
};

static NEXT_SQUATTING_PARAMETER: AtomicUsize = AtomicUsize::new(0);

#[test]
fn coefficient_context_rejects_unsafe_process_global_symbol_squatting() {
    let ordinal = NEXT_SQUATTING_PARAMETER.fetch_add(1, Ordering::Relaxed);
    let name = format!("coefficient_context_squatting_regression_{ordinal}");
    let qualified = format!("rustred::{name}");
    let namespaced = NamespacedSymbol::try_parse(&qualified).unwrap();
    SymbolBuilder::new(namespaced)
        .with_attributes(&[SymbolAttribute::Symmetric])
        .with_tags(["rustred_test::unsafe_coefficient_parameter"])
        .with_user_data(UserData::Integer(17))
        .build()
        .unwrap();

    assert_eq!(
        CoefficientContext::try_new([name.clone()]).unwrap_err(),
        CoefficientContextError::ParameterSymbolCollision { name }
    );
}

#[test]
fn exact_authentication_rejects_malformed_sparse_polynomials_without_panicking() {
    let context = CoefficientContext::new(["x"]);

    let mut malformed_layout = context.one();
    malformed_layout.numerator.exponents.push(0);
    assert!(matches!(
        context.validate_with_limits(&malformed_layout, ExactAlgebraLimits::default()),
        Err(ExactAlgebraError::MalformedExponentLayout {
            part: CoefficientPolynomialPart::Numerator,
            ..
        })
    ));
    assert!(!context.contains(&malformed_layout));

    let mut explicit_zero = context.one();
    explicit_zero.numerator.coefficients[0] = Integer::from(0);
    assert!(matches!(
        context.validate_with_limits(&explicit_zero, ExactAlgebraLimits::default()),
        Err(ExactAlgebraError::ZeroCoefficient {
            part: CoefficientPolynomialPart::Numerator,
            term: 0,
        })
    ));

    let mut wrong_order = context.one();
    wrong_order.numerator.coefficients = vec![Integer::from(1), Integer::from(1)];
    wrong_order.numerator.exponents = vec![1, 0];
    assert!(matches!(
        context.validate_with_limits(&wrong_order, ExactAlgebraLimits::default()),
        Err(ExactAlgebraError::NonCanonicalMonomialOrder {
            part: CoefficientPolynomialPart::Numerator,
            term: 1,
        })
    ));
}

#[test]
fn exact_authentication_rejects_every_backend_representation_of_numeric_zero() {
    let context = CoefficientContext::new(["x"]);
    for (part, zero) in [
        (CoefficientPolynomialPart::Numerator, Integer::Double(0)),
        (
            CoefficientPolynomialPart::Numerator,
            Integer::Large(0.into()),
        ),
        (CoefficientPolynomialPart::Denominator, Integer::Double(0)),
        (
            CoefficientPolynomialPart::Denominator,
            Integer::Large(0.into()),
        ),
    ] {
        let mut malformed = context.one();
        match part {
            CoefficientPolynomialPart::Numerator => {
                malformed.numerator.coefficients[0] = zero;
            }
            CoefficientPolynomialPart::Denominator => {
                malformed.denominator.coefficients[0] = zero;
            }
        }
        assert_eq!(
            context.validate_with_limits(&malformed, ExactAlgebraLimits::default()),
            Err(ExactAlgebraError::ZeroCoefficient { part, term: 0 })
        );
        assert!(!context.contains(&malformed));
    }
}
