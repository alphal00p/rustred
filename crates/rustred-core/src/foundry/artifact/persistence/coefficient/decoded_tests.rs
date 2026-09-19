//! Native table-reference tests, independent of source discovery.
use super::*;

fn context(arity: usize) -> IndexedCoefficientContext {
    let base = CoefficientContext::new(["s", "d"]);
    IndexedCoefficientContext::try_new(&base, "source-codec-native-values", arity).unwrap()
}

fn fraction(context: &IndexedCoefficientContext) -> IndexedCoefficient {
    let numerator = context
        .add(
            &context.index(1).unwrap(),
            &context
                .lift(&context.base().parameter("d").unwrap())
                .unwrap(),
        )
        .unwrap();
    let denominator = context
        .sub(&context.index(0).unwrap(), &context.integer(2))
        .unwrap();
    context.div(&numerator, &denominator).unwrap()
}

#[test]
fn indexed_native_transport_preserves_nonprefix_parameter_order() {
    let context = context(2);
    let value = fraction(&context);
    let mut writer = Writer::new(Default::default());
    encode_indexed_coefficient(&mut writer, &value).unwrap();
    encode_indexed_coefficient(&mut writer, &value).unwrap();
    let (bytes, table) = writer.finish_for_test().unwrap();
    assert_eq!(bytes.len(), 10);
    assert_eq!(table.len(), 1);
    let mut reader = Reader::with_table(&bytes, Default::default(), table).unwrap();
    assert_eq!(
        decode_indexed_coefficient(&mut reader, &context, "first value").unwrap(),
        value
    );
    assert_eq!(
        decode_indexed_coefficient(&mut reader, &context, "second value").unwrap(),
        value
    );
    reader.finish().unwrap();
}

#[test]
fn polynomial_rational_and_integer_reference_kinds_are_distinct() {
    let context = context(2);
    let value = fraction(&context);
    let polynomial = context
        .denominator_condition_with_limits(&value, Default::default())
        .unwrap();
    let mut writer = Writer::new(Default::default());
    encode_indexed_polynomial(&mut writer, &polynomial).unwrap();
    let (bytes, table) = writer.finish_for_test().unwrap();
    let mut reader = Reader::with_table(&bytes, Default::default(), table.clone()).unwrap();
    assert_eq!(
        decode_indexed_polynomial(&mut reader, &context, "polynomial").unwrap(),
        polynomial
    );
    reader.finish().unwrap();
    let mut reader = Reader::with_table(&bytes, Default::default(), table.clone()).unwrap();
    assert!(matches!(
        decode_indexed_coefficient(&mut reader, &context, "wrong kind"),
        Err(ArtifactPersistenceError::InvalidCoefficient { .. })
    ));
    let mut reader = Reader::with_table(&bytes, Default::default(), table).unwrap();
    assert!(matches!(
        decode_integer(&mut reader, "wrong integer kind"),
        Err(ArtifactPersistenceError::InvalidCoefficient { .. })
    ));
}

#[test]
fn same_size_wrong_map_and_wrong_arity_are_rejected() {
    let context = context(2);
    let value = fraction(&context);
    let mut writer = Writer::new(Default::default());
    encode_indexed_coefficient(&mut writer, &value).unwrap();
    let (bytes, table) = writer.finish_for_test().unwrap();
    let other_arity = IndexedCoefficientContext::try_new(context.base(), "other", 3).unwrap();
    let other_base = CoefficientContext::new(["q", "d"]);
    let other_map = IndexedCoefficientContext::try_new(&other_base, "other", 2).unwrap();
    for other in [&other_arity, &other_map] {
        let mut reader = Reader::with_table(&bytes, Default::default(), table.clone()).unwrap();
        assert!(matches!(
            decode_indexed_coefficient(&mut reader, other, "wrong map"),
            Err(ArtifactPersistenceError::InvalidCoefficient { .. })
        ));
    }
}

#[test]
fn base_and_indexed_zero_and_one_keep_their_contexts() {
    let context = context(2);
    let base_zero = context.base().zero();
    let base_one = context.base().one();
    let indexed_zero = context.zero();
    let indexed_one = context.one();
    let mut writer = Writer::new(Default::default());
    encode_base_coefficient(&mut writer, &base_zero).unwrap();
    encode_base_coefficient(&mut writer, &base_one).unwrap();
    encode_indexed_coefficient(&mut writer, &indexed_zero).unwrap();
    encode_indexed_coefficient(&mut writer, &indexed_one).unwrap();
    let (bytes, table) = writer.finish_for_test().unwrap();
    assert_eq!(table.len(), 4);
    let mut reader = Reader::with_table(&bytes, Default::default(), table.clone()).unwrap();
    assert_eq!(
        decode_base_coefficient(&mut reader, context.base(), "base zero").unwrap(),
        base_zero
    );
    assert_eq!(
        decode_base_coefficient(&mut reader, context.base(), "base one").unwrap(),
        base_one
    );
    assert_eq!(
        decode_indexed_coefficient(&mut reader, &context, "indexed zero").unwrap(),
        indexed_zero
    );
    assert_eq!(
        decode_indexed_coefficient(&mut reader, &context, "indexed one").unwrap(),
        indexed_one
    );
    reader.finish().unwrap();
    let mut reader = Reader::with_table(&bytes[10..15], Default::default(), table).unwrap();
    assert!(decode_base_coefficient(&mut reader, context.base(), "indexed as base").is_err());
}

#[test]
fn arbitrary_width_integers_use_native_empty_map_coefficients() {
    let values = [
        Integer::from(0),
        Integer::from(-1),
        Integer::from(i64::MIN),
        Integer::from(i128::MIN),
        Integer::from(1) << 200u32,
    ];
    let mut writer = Writer::new(Default::default());
    for value in &values {
        encode_integer(&mut writer, value).unwrap();
    }
    let (bytes, table) = writer.finish_for_test().unwrap();
    assert_eq!(bytes.len(), 5 * values.len());
    let mut reader = Reader::with_table(&bytes, Default::default(), table).unwrap();
    for expected in values {
        assert_eq!(
            decode_integer(&mut reader, "wide integer").unwrap(),
            expected
        );
    }
    reader.finish().unwrap();
}

#[test]
fn polynomial_and_integer_references_reject_rational_payloads() {
    let context = context(2);
    let raw = fraction(&context);
    let mut writer = Writer::new(Default::default());
    encode_reference(&mut writer, POLYNOMIAL_PAYLOAD, raw.raw()).unwrap();
    let (bytes, table) = writer.finish_for_test().unwrap();
    let mut reader = Reader::with_table(&bytes, Default::default(), table).unwrap();
    assert!(decode_indexed_polynomial(&mut reader, &context, "rational guard").is_err());

    let template = CoefficientPolynomial::new_zero(&Z);
    let rational = Coefficient {
        numerator: template.one(),
        denominator: template.constant(Integer::from(2)),
    };
    let mut writer = Writer::new(Default::default());
    encode_reference(&mut writer, INTEGER_PAYLOAD, &rational).unwrap();
    let (bytes, table) = writer.finish_for_test().unwrap();
    let mut reader = Reader::with_table(&bytes, Default::default(), table).unwrap();
    assert!(decode_integer(&mut reader, "noninteger constant").is_err());
}

#[test]
fn native_table_rejects_zero_denominators_before_emitting_references() {
    let context = context(2);
    let mut raw = fraction(&context).raw().clone();
    raw.denominator = raw.denominator.zero();
    let mut writer = Writer::new(Default::default());
    assert!(encode_base_coefficient(&mut writer, &raw).is_err());
}

#[test]
fn coefficient_references_preserve_structural_term_budget() {
    let context = context(2);
    let value = fraction(&context);
    let terms = value.raw().numerator.nterms() + value.raw().denominator.nterms();
    let mut writer = Writer::new(super::super::limits::ArtifactEncodingLimits {
        max_collection_entries: terms - 1,
        ..Default::default()
    });
    assert_eq!(
        encode_indexed_coefficient(&mut writer, &value).unwrap_err(),
        ArtifactPersistenceError::ResourceLimit {
            resource: "coefficient polynomial terms",
            requested: terms,
            limit: terms - 1,
        }
    );
    // No reference was retained and no coefficient was interned on failure.
    let (bytes, table) = writer.finish_for_test().unwrap();
    assert!(bytes.is_empty());
    assert!(table.is_empty());
}

#[test]
fn generated_reference_loading_does_not_repeat_native_gcd() {
    let context = context(2);
    let value = fraction(&context);
    let raw = Coefficient {
        numerator: &value.raw().numerator * &value.raw().numerator.constant(Integer::from(2)),
        denominator: &value.raw().denominator * &value.raw().denominator.constant(Integer::from(2)),
    };
    // Deliberately violate the generated-producer normalization precondition:
    // native loading restores this representation, not hidden per-use GCD.
    let mut writer = Writer::new(Default::default());
    encode_base_coefficient(&mut writer, &raw).unwrap();
    let (bytes, table) = writer.finish_for_test().unwrap();
    let mut reader = Reader::with_table(&bytes, Default::default(), table).unwrap();
    let decoded =
        decode_indexed_coefficient(&mut reader, &context, "unchanged native payload").unwrap();
    assert_eq!(decoded.raw(), &raw);
}

#[test]
fn reference_reader_obeys_narrower_native_term_limit() {
    let context = context(2);
    let mut writer = Writer::new(Default::default());
    encode_indexed_coefficient(&mut writer, &fraction(&context)).unwrap();
    let (bytes, table) = writer.finish_for_test().unwrap();
    let mut limits = super::super::limits::ArtifactLoadLimits::default();
    limits.family.exact_algebra.max_polynomial_terms = 0;
    let mut reader = Reader::with_table(&bytes, limits, table).unwrap();
    assert!(matches!(
        decode_indexed_coefficient(&mut reader, &context, "term budget"),
        Err(ArtifactPersistenceError::ResourceLimit { .. })
    ));
}
