//! Native codec transport tests, independent of source discovery.
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
    let bytes = writer.finish();
    let mut reader = Reader::root(&bytes, Default::default()).unwrap();
    let decoded = decode_indexed_coefficient(&mut reader, &context, "test value").unwrap();
    reader.finish().unwrap();
    assert_eq!(decoded, value);
    let mut writer = Writer::new(Default::default());
    encode_indexed_coefficient(&mut writer, &decoded).unwrap();
    assert_eq!(writer.finish(), bytes);
}

#[test]
fn indexed_polynomial_transport_is_separate_from_rational_transport() {
    let context = context(2);
    let value = fraction(&context);
    let polynomial = context
        .denominator_condition_with_limits(&value, Default::default())
        .unwrap();
    let mut writer = Writer::new(Default::default());
    encode_indexed_polynomial(&mut writer, &polynomial).unwrap();
    let bytes = writer.finish();
    let mut reader = Reader::root(&bytes, Default::default()).unwrap();
    assert_eq!(
        decode_indexed_polynomial(&mut reader, &context, "test polynomial").unwrap(),
        polynomial,
    );
    reader.finish().unwrap();
    let mut reader = Reader::root(&bytes, Default::default()).unwrap();
    assert!(matches!(
        decode_indexed_coefficient(&mut reader, &context, "wrong kind"),
        Err(ArtifactPersistenceError::InvalidCoefficient { .. }),
    ));
}

#[test]
fn indexed_native_transport_rejects_wrong_map_shape_and_zero_denominator() {
    let context = context(2);
    let value = fraction(&context);
    let mut writer = Writer::new(Default::default());
    encode_indexed_coefficient(&mut writer, &value).unwrap();
    let bytes = writer.finish();
    let other = IndexedCoefficientContext::try_new(context.base(), "other", 3).unwrap();
    let mut reader = Reader::root(&bytes, Default::default()).unwrap();
    assert!(matches!(
        decode_indexed_coefficient(&mut reader, &other, "wrong shape"),
        Err(ArtifactPersistenceError::InvalidCoefficient { .. }),
    ));

    let mut raw = value.raw().clone();
    raw.denominator = raw.denominator.zero();
    let mut writer = Writer::new(Default::default());
    encode_base_coefficient(&mut writer, &raw).unwrap();
    let bytes = writer.finish();
    let mut reader = Reader::root(&bytes, Default::default()).unwrap();
    assert!(matches!(
        decode_indexed_coefficient(&mut reader, &context, "zero denominator"),
        Err(ArtifactPersistenceError::InvalidCoefficient { .. }),
    ));
}

#[test]
fn indexed_transport_rejects_noncanonical_native_fraction() {
    let context = context(2);
    let value = fraction(&context);
    let raw = Coefficient {
        numerator: &value.raw().numerator * &value.raw().numerator.constant(Integer::from(2)),
        denominator: &value.raw().denominator * &value.raw().denominator.constant(Integer::from(2)),
    };
    let mut writer = Writer::new(Default::default());
    encode_base_coefficient(&mut writer, &raw).unwrap();
    let bytes = writer.finish();
    let mut reader = Reader::root(&bytes, Default::default()).unwrap();
    assert!(matches!(
        decode_indexed_coefficient(&mut reader, &context, "scaled fraction"),
        Err(ArtifactPersistenceError::NonCanonicalCoefficient { .. }),
    ));
}

#[test]
fn indexed_transport_obeys_native_and_aggregate_payload_limits() {
    let context = context(2);
    let value = fraction(&context);
    let mut writer = Writer::new(Default::default());
    encode_indexed_coefficient(&mut writer, &value).unwrap();
    let bytes = writer.finish();
    let mut limits = super::super::limits::ArtifactLoadLimits::default();
    limits.family.exact_algebra.max_polynomial_terms = 0;
    let mut reader = Reader::root(&bytes, limits).unwrap();
    assert!(matches!(
        decode_indexed_coefficient(&mut reader, &context, "term budget"),
        Err(ArtifactPersistenceError::ResourceLimit { .. }),
    ));

    let payload_bytes = bytes.len() - std::mem::size_of::<u64>();
    let limits = super::super::limits::ArtifactLoadLimits {
        max_total_coefficient_bytes: payload_bytes,
        ..Default::default()
    };
    let reader = Reader::root(&bytes, limits).unwrap();
    let mut first = reader.child(&bytes);
    decode_indexed_coefficient(&mut first, &context, "first").unwrap();
    first.finish().unwrap();
    let mut second = reader.child(&bytes);
    assert!(matches!(
        decode_indexed_coefficient(&mut second, &context, "second"),
        Err(ArtifactPersistenceError::ResourceLimit {
            resource: "aggregate coefficient bytes",
            ..
        }),
    ));
}
