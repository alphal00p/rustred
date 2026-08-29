use super::super::fingerprint::census_family_fingerprint;
use super::*;

fn huge_gmp_fingerprint_family(
    limits: IntegralFamilyLimits,
) -> Result<IntegralFamily, IntegralFamilyError> {
    let context = CoefficientContext::new(["x"]);
    let decimal = format!("1{}", "0".repeat(1_500));
    let magnitude = decimal.parse::<Integer>().unwrap();
    let mut dimension = context.parameter("x").unwrap();
    dimension.numerator.coefficients[0] = -magnitude;
    IntegralFamily::new_with_limits(
        "huge-gmp-fingerprint",
        vec!["k".into()],
        Vec::new(),
        context.clone(),
        dimension,
        identity_denominators(&context, 1),
        Vec::new(),
        vec![context.zero()],
        limits,
    )
}

#[test]
fn typed_fingerprint_preflights_exact_and_one_below_huge_gmp_payloads() {
    let family = huge_gmp_fingerprint_family(IntegralFamilyLimits::default()).unwrap();
    let stats = census_family_fingerprint(&family).unwrap();
    assert_eq!(stats.encoded_bytes(), family.fingerprint().len());
    assert!(stats.integer_bits() > 4_000);
    assert!(family.fingerprint().contains("I-"));
    let mut exact = IntegralFamilyLimits::default();
    exact.max_fingerprint_bytes = stats.encoded_bytes();
    exact.max_fingerprint_encoding_work = stats.encoding_work();
    exact.max_fingerprint_polynomial_terms = stats.polynomial_terms();
    exact.max_fingerprint_exponent_entries = stats.exponent_entries();
    exact.max_fingerprint_integer_bits = stats.integer_bits();
    let rebuilt = huge_gmp_fingerprint_family(exact).unwrap();
    assert_eq!(rebuilt.fingerprint(), family.fingerprint());
    assert_eq!(census_family_fingerprint(&rebuilt).unwrap(), stats);

    macro_rules! one_below {
        ($field:ident, $getter:ident, $resource:literal) => {{
            let requested = stats.$getter();
            assert!(requested > 0, $resource);
            let mut limits = IntegralFamilyLimits::default();
            limits.$field = requested - 1;
            assert!(matches!(
                huge_gmp_fingerprint_family(limits),
                Err(IntegralFamilyError::ResourceLimit {
                    resource: actual,
                    requested: actual_requested,
                    limit,
                }) if actual == $resource
                    && actual_requested == requested
                    && limit == requested - 1
            ));
        }};
    }
    one_below!(
        max_fingerprint_bytes,
        encoded_bytes,
        "family fingerprint bytes"
    );
    one_below!(
        max_fingerprint_encoding_work,
        encoding_work,
        "family fingerprint encoding work"
    );
    one_below!(
        max_fingerprint_polynomial_terms,
        polynomial_terms,
        "family fingerprint polynomial terms"
    );
    one_below!(
        max_fingerprint_exponent_entries,
        exponent_entries,
        "family fingerprint exponent entries"
    );
    one_below!(
        max_fingerprint_integer_bits,
        integer_bits,
        "family fingerprint integer bits"
    );
}
