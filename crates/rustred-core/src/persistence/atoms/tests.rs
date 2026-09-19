use std::sync::Arc;

use symbolica::prelude::{PolyVariable, Q};

use super::*;

fn coefficient(source: &str) -> Coefficient {
    symbolica::parse!(source).to_rational_polynomial::<_, _, u16>(&Q, &Z, None)
}

fn encoded_one() -> EncodedCoefficientTable {
    let mut builder = CoefficientTableBuilder::new(BinaryIoLimits::default());
    builder
        .intern(&coefficient("1/(atom_table_x + 1)"))
        .unwrap();
    builder.finish().unwrap()
}

#[test]
fn dictionary_preserves_zero_large_integers_and_distinct_variable_maps() {
    let limits = BinaryIoLimits::default();
    let base = coefficient("(atom_table_d + 1)/(atom_table_d - 3)");
    let indexed = coefficient("(atom_table_d + atom_table_n)/(atom_table_n - 1)");
    let zero: Coefficient = indexed.numerator.zero().into();
    let large = coefficient("12345678901234567890123456789012345678901234567890123456789/7");
    let mut builder = CoefficientTableBuilder::new(limits);
    let base_id = builder.intern(&base).unwrap();
    let indexed_id = builder.intern(&indexed).unwrap();
    let zero_id = builder.intern(&zero).unwrap();
    let large_id = builder.intern(&large).unwrap();
    assert_eq!(builder.intern(&indexed).unwrap(), indexed_id);
    assert_eq!(builder.intern(&zero).unwrap(), zero_id);
    assert_eq!(builder.len(), 4);
    assert_eq!(
        [
            base_id.index(),
            indexed_id.index(),
            zero_id.index(),
            large_id.index()
        ],
        [0, 1, 2, 3]
    );
    let encoded = builder.finish().unwrap();
    let decoded =
        DecodedCoefficientTable::import_generated(&encoded.state, &encoded.atoms, limits).unwrap();
    assert_eq!(decoded.len(), 4);
    for (id, expected) in [
        (base_id, base),
        (indexed_id, indexed),
        (zero_id, zero),
        (large_id, large),
    ] {
        assert_eq!(decoded.coefficient(id).unwrap(), &expected);
        assert_eq!(
            decoded.coefficient(id).unwrap().get_variables(),
            expected.get_variables()
        );
    }
    assert!(
        decoded
            .coefficient(CoefficientId::try_from_index(4).unwrap())
            .is_err()
    );
}

#[test]
fn first_occurrence_ids_and_encoded_tables_are_repeatable_in_one_state() {
    let values = [
        coefficient("atom_table_repeat_x + 1"),
        coefficient("atom_table_repeat_x^2 - 1"),
    ];
    let build = || {
        let mut builder = CoefficientTableBuilder::new(BinaryIoLimits::default());
        for expected in 0..2 {
            assert_eq!(builder.intern(&values[expected]).unwrap().index(), expected);
        }
        assert_eq!(builder.intern(&values[0]).unwrap().index(), 0);
        builder.finish().unwrap()
    };
    let left = build();
    let right = build();
    // State export can include ambient registries modified by parallel tests.
    // The atom records and IDs themselves do not depend on hash iteration.
    assert_eq!(left.atoms, right.atoms);
}

#[test]
fn hash_bucket_collisions_still_require_complete_atom_equality() {
    let first = coefficient("atom_table_collision_x + 1");
    let second = coefficient("atom_table_collision_x + 2");
    let mut builder = CoefficientTableBuilder::new(BinaryIoLimits::default());
    let first_id = builder.intern(&first).unwrap();
    let mut atom = Atom::new();
    atom.to_num(NativeCoefficient::RationalPolynomial(second.clone()));
    let mut hash = DefaultHasher::new();
    atom.hash(&mut hash);
    builder.buckets.insert(hash.finish(), vec![first_id]);
    assert_eq!(builder.intern(&second).unwrap().index(), 1);
    assert_eq!(builder.intern(&second).unwrap().index(), 1);
}

#[test]
fn normalization_is_an_explicit_once_per_unique_import_choice() {
    let variable = symbolica::symbol!("atom_table_normalize_x");
    let variables = Arc::new(vec![PolyVariable::Symbol(variable)]);
    let numerator = symbolica::parse!("atom_table_normalize_x^2 - 1")
        .to_polynomial::<_, u16>(&Z, Some(variables.clone()));
    let denominator = symbolica::parse!("atom_table_normalize_x - 1")
        .to_polynomial::<_, u16>(&Z, Some(variables));
    let unreduced = Coefficient {
        numerator,
        denominator,
    };
    let mut builder = CoefficientTableBuilder::new(BinaryIoLimits::default());
    let id = builder.intern(&unreduced).unwrap();
    assert_eq!(builder.intern(&unreduced).unwrap(), id);
    let encoded = builder.finish().unwrap();
    // Deliberately violate the fast path's normalized-producer precondition to
    // demonstrate that it performs structural restoration, not hidden GCD.
    let fast = DecodedCoefficientTable::import_generated(
        &encoded.state,
        &encoded.atoms,
        BinaryIoLimits::default(),
    )
    .unwrap();
    assert_eq!(fast.coefficient(id).unwrap(), &unreduced);
    let normalized = DecodedCoefficientTable::import_generated_normalized(
        &encoded.state,
        &encoded.atoms,
        BinaryIoLimits::default(),
    )
    .unwrap();
    assert_eq!(normalized.len(), 1);
    assert!(normalized.coefficient(id).unwrap().denominator.is_one());
    assert_eq!(
        normalized.coefficient(id).unwrap().to_expression(),
        symbolica::parse!("atom_table_normalize_x + 1")
    );
}

#[test]
fn malformed_framing_is_rejected_before_native_import() {
    let encoded = encoded_one();
    let limits = BinaryIoLimits::default();
    let bad_state = b"not even a native Symbolica state";
    let mut wrong_inner_length = encoded.atoms.clone();
    wrong_inner_length[17..25].copy_from_slice(&u64::MAX.to_le_bytes());
    assert!(matches!(
        DecodedCoefficientTable::import_generated(bad_state, &wrong_inner_length, limits),
        Err(BinaryIoError::Invalid(
            "native atom length differs from frame"
        ))
    ));
    let mut trailing = encoded.atoms.clone();
    trailing.push(0);
    assert!(matches!(
        DecodedCoefficientTable::import_generated(bad_state, &trailing, limits),
        Err(BinaryIoError::Invalid("trailing coefficient table bytes"))
    ));
    let mut truncated = encoded.atoms.clone();
    truncated.pop();
    assert!(matches!(
        DecodedCoefficientTable::import_generated(bad_state, &truncated, limits),
        Err(BinaryIoError::Invalid("truncated coefficient atom"))
    ));
}

#[test]
fn record_count_and_byte_limits_are_enforced() {
    let encoded = encoded_one();
    let mut limits = BinaryIoLimits::default();
    limits.max_collection_entries = 0;
    assert!(matches!(
        DecodedCoefficientTable::import_generated(&encoded.state, &encoded.atoms, limits),
        Err(BinaryIoError::Limit {
            resource: "coefficient table entries",
            ..
        })
    ));
    let mut builder = CoefficientTableBuilder::new(limits);
    assert!(matches!(
        builder.intern(&coefficient("1")),
        Err(BinaryIoError::Limit {
            resource: "coefficient table entries",
            ..
        })
    ));
    limits = BinaryIoLimits::default();
    limits.max_atom_bytes = 1;
    assert!(matches!(
        DecodedCoefficientTable::import_generated(&encoded.state, &encoded.atoms, limits),
        Err(BinaryIoError::Limit {
            resource: "coefficient atom bytes",
            ..
        })
    ));
    let mut builder = CoefficientTableBuilder::new(limits);
    assert!(matches!(
        builder.intern(&coefficient("1")),
        Err(BinaryIoError::Limit {
            resource: "coefficient atom bytes",
            ..
        })
    ));
}

#[test]
fn valid_native_non_polynomial_atom_is_rejected() {
    let mut builder = CoefficientTableBuilder::new(BinaryIoLimits::default());
    builder.intern(&coefficient("1")).unwrap();
    let encoded = builder.finish().unwrap();
    let native = bincode::encode_to_vec(Atom::num(2), bincode::config::standard()).unwrap();
    let mut atoms = Vec::new();
    write_length(&mut atoms, 1).unwrap();
    write_length(&mut atoms, native.len()).unwrap();
    atoms.extend(native);
    assert!(matches!(
        DecodedCoefficientTable::import_generated(
            &encoded.state,
            &atoms,
            BinaryIoLimits::default()
        ),
        Err(BinaryIoError::Invalid(
            "coefficient atom is not a rational polynomial"
        ))
    ));
}

#[test]
fn malformed_sparse_map_is_rejected_before_native_encoding() {
    let left = coefficient("atom_table_wrong_left");
    let right = coefficient("atom_table_wrong_right");
    let value = Coefficient {
        numerator: left.numerator,
        denominator: right.denominator,
    };
    let mut builder = CoefficientTableBuilder::new(BinaryIoLimits::default());
    assert!(builder.intern(&value).is_err());
    assert!(builder.is_empty());
}

#[test]
fn empty_dictionary_roundtrips() {
    let builder = CoefficientTableBuilder::new(BinaryIoLimits::default());
    assert!(builder.is_empty());
    let encoded = builder.finish().unwrap();
    let decoded = DecodedCoefficientTable::import_generated(
        &encoded.state,
        &encoded.atoms,
        BinaryIoLimits::default(),
    )
    .unwrap();
    assert!(decoded.is_empty());
}
