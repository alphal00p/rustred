use super::*;

fn family(loops: usize, externals: usize) -> IntegralFamily {
    let context = CoefficientContext::try_new(["d", "s", "m2", "delta"]).unwrap();
    let n = loops * (loops + 1) / 2 + loops * externals;
    let denominators = (0..n)
        .map(|row| {
            AffineDenominator::new(
                -context.parameter("m2").unwrap(),
                (0..n)
                    .map(|column| context.integer(i64::from(row == column)))
                    .collect(),
            )
        })
        .collect();
    let gram = (0..externals)
        .map(|row| {
            (0..externals)
                .map(|column| {
                    if row == column {
                        context.parameter("s").unwrap()
                    } else {
                        context.zero()
                    }
                })
                .collect()
        })
        .collect();
    IntegralFamily::new(
        "generic-native-family",
        (0..loops).map(|i| format!("q{i}")).collect(),
        (0..externals).map(|i| format!("p{i}")).collect(),
        context.clone(),
        context.parameter("d").unwrap(),
        denominators,
        gram,
        vec![context.parameter("delta").unwrap(); n],
    )
    .unwrap()
}

fn encode(family: &IntegralFamily) -> (NativeFamilyRecord, DecodedCoefficientTable) {
    let limits = BinaryIoLimits::default();
    let mut builder = CoefficientTableBuilder::new(limits);
    let record = NativeFamilyRecord::from_family(family, &mut builder).unwrap();
    let table = builder.finish().unwrap();
    let table =
        DecodedCoefficientTable::import_generated(&table.state, &table.atoms, limits).unwrap();
    let bytes = bincode::encode_to_vec(&record, bincode::config::standard()).unwrap();
    let (record, consumed) =
        bincode::decode_from_slice(&bytes, bincode::config::standard()).unwrap();
    assert_eq!(consumed, bytes.len());
    (record, table)
}

#[test]
fn native_family_roundtrips_general_affine_geometry_without_input_text() {
    for (loops, externals) in [(1, 0), (2, 2), (4, 0), (6, 0)] {
        let expected = family(loops, externals);
        let (record, table) = encode(&expected);
        let actual = record
            .to_family(
                &table,
                IntegralFamilyLimits::default(),
                BinaryIoLimits::default(),
            )
            .unwrap();
        assert_eq!(record.arity(), expected.denominator_count());
        assert_eq!(actual.fingerprint(), expected.fingerprint());
        assert_eq!(actual.dimension(), expected.dimension());
        assert_eq!(actual.denominators(), expected.denominators());
        assert_eq!(actual.external_gram(), expected.external_gram());
        assert_eq!(actual.power_shifts(), expected.power_shifts());
        assert_eq!(actual.inverse_basis(), expected.inverse_basis());
        assert_eq!(actual.domain(), expected.domain());
        assert!(table.len() < expected.denominator_count().pow(2) + 6);
    }
}

#[test]
fn native_family_rejects_bad_ids_contexts_shapes_and_limits() {
    let (record, table) = encode(&family(2, 1));
    let decode = |record: &NativeFamilyRecord| {
        record.to_family(
            &table,
            IntegralFamilyLimits::default(),
            BinaryIoLimits::default(),
        )
    };
    let mut invalid = record.clone();
    invalid.dimension = u32::MAX;
    assert!(decode(&invalid).is_err());
    let mut invalid = record.clone();
    invalid.parameters.swap(0, 1);
    assert!(decode(&invalid).is_err());
    let mut invalid = record.clone();
    invalid.denominators[0].coefficients.pop();
    assert!(decode(&invalid).is_err());
    let mut invalid = record.clone();
    invalid.external_gram[0].clear();
    assert!(decode(&invalid).is_err());
    let mut invalid = record.clone();
    invalid.power_shifts.pop();
    assert!(decode(&invalid).is_err());
    let mut invalid = record.clone();
    invalid.loop_momenta[1] = invalid.loop_momenta[0].clone();
    assert!(decode(&invalid).is_err());
    let mut invalid = record.clone();
    invalid.denominators[1] = invalid.denominators[0].clone();
    assert!(
        decode(&invalid).is_err(),
        "singular bases still fail admission"
    );
    assert!(
        record
            .validate_shape(
                IntegralFamilyLimits {
                    max_matrix_entries: 25,
                    ..Default::default()
                },
                BinaryIoLimits::default()
            )
            .is_err(),
        "preflight includes augmented inverse storage"
    );
    assert!(
        record
            .to_family(
                &table,
                IntegralFamilyLimits {
                    max_scalar_products: 4,
                    ..Default::default()
                },
                BinaryIoLimits::default()
            )
            .is_err()
    );
    assert!(
        record
            .to_family(
                &table,
                IntegralFamilyLimits::default(),
                BinaryIoLimits {
                    max_collection_entries: 1,
                    ..Default::default()
                }
            )
            .is_err()
    );
}

#[test]
fn native_family_preserves_dense_rational_basis_gram_and_distinct_shifts() {
    let context = CoefficientContext::try_new(["d", "s", "m2", "delta"]).unwrap();
    let s = context.parameter("s").unwrap();
    let delta = context.parameter("delta").unwrap();
    let n = 7;
    // I + uv^T with positive rational u,v is invertible; unequal entries
    // distinguish a matrix from its transpose.
    let denominators = (0..n)
        .map(|row| {
            AffineDenominator::new(
                &context.parameter("m2").unwrap() + &context.integer(row as i64),
                (0..n)
                    .map(|column| {
                        let rank_one = &context.integer((row + 1) as i64)
                            * &context.integer((column + 2) as i64);
                        &(&rank_one / &s) + &context.integer(i64::from(row == column))
                    })
                    .collect(),
            )
        })
        .collect();
    let expected = IntegralFamily::new(
        "affine-rational-native",
        vec!["k1".into(), "k2".into()],
        vec!["p1".into(), "p2".into()],
        context.clone(),
        context.parameter("d").unwrap(),
        denominators,
        vec![
            vec![s.clone(), &s / &context.integer(3)],
            vec![&s / &context.integer(3), &s * &context.integer(2)],
        ],
        (0..n)
            .map(|i| &delta + &(&context.integer(i as i64) / &context.integer(3)))
            .collect(),
    )
    .unwrap();
    let (record, table) = encode(&expected);
    let actual = record
        .to_family(
            &table,
            IntegralFamilyLimits::default(),
            BinaryIoLimits::default(),
        )
        .unwrap();
    assert_eq!(actual.fingerprint(), expected.fingerprint());
    assert_eq!(actual.denominators(), expected.denominators());
    assert_eq!(actual.external_gram(), expected.external_gram());
    assert_eq!(actual.power_shifts(), expected.power_shifts());
    assert_eq!(actual.domain(), expected.domain());
    let mut asymmetric = record;
    asymmetric.external_gram[0][1] = asymmetric.dimension;
    assert!(
        asymmetric
            .to_family(
                &table,
                IntegralFamilyLimits::default(),
                BinaryIoLimits::default()
            )
            .is_err()
    );
}
