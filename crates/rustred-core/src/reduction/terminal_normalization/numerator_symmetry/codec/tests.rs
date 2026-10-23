use super::*;
use crate::algebra::CoefficientContext;
use crate::family::AffineDenominator;

fn family(mass: i64) -> Arc<IntegralFamily> {
    let context = CoefficientContext::new(["d"]);
    Arc::new(
        IntegralFamily::new(
            "native_numerator_witness",
            vec!["k".into(), "l".into()],
            vec![],
            context.clone(),
            context.parameter("d").unwrap(),
            [[1, 0, 0], [0, 0, 1], [1, 2, 1]]
                .into_iter()
                .map(|row| {
                    AffineDenominator::new(
                        context.integer(-mass),
                        row.map(|value| context.integer(value)).into(),
                    )
                })
                .collect(),
            vec![],
            vec![context.zero(); 3],
        )
        .unwrap(),
    )
}
fn fixture() -> (Arc<IntegralFamily>, Vec<u8>) {
    let family = family(1);
    let keys = BTreeSet::from([IntegralKey::try_new([1, 1, -1]).unwrap()]);
    let plan =
        TerminalNumeratorSymmetryEquations::prepare(family.clone(), &keys, Default::default())
            .unwrap();
    assert!(!plan.equations(keys.first().unwrap()).is_empty());
    (
        family,
        plan.encode_native(BinaryIoLimits::unbounded_resources())
            .unwrap(),
    )
}
fn unpack(bytes: &[u8]) -> (Record, crate::persistence::EncodedCoefficientTable) {
    let envelope = inspect_program(bytes, BinaryIoLimits::unbounded_resources()).unwrap();
    let (record, consumed): (Record, usize) = bincode::decode_from_slice(
        envelope.section(SectionTag::PROGRAM).unwrap(),
        bincode::config::standard(),
    )
    .unwrap();
    assert_eq!(
        consumed,
        envelope.section(SectionTag::PROGRAM).unwrap().len()
    );
    (
        record,
        crate::persistence::EncodedCoefficientTable {
            state: envelope
                .section(SectionTag::SYMBOLICA_STATE)
                .unwrap()
                .to_vec(),
            atoms: envelope.section(SectionTag::COEFFICIENTS).unwrap().to_vec(),
        },
    )
}
fn zero_id(table: &crate::persistence::EncodedCoefficientTable) -> u32 {
    let decoded = DecodedCoefficientTable::import_generated(
        &table.state,
        &table.atoms,
        BinaryIoLimits::unbounded_resources(),
    )
    .unwrap();
    (0..decoded.len())
        .find(|&id| {
            decoded
                .coefficient(CoefficientId::try_from_index(id).unwrap())
                .unwrap()
                .is_zero()
        })
        .unwrap() as u32
}
fn reject(bytes: &[u8], family: Arc<IntegralFamily>) {
    assert!(
        TerminalNumeratorSymmetryEquations::from_native_bytes(
            bytes,
            family,
            Default::default(),
            BinaryIoLimits::unbounded_resources(),
        )
        .is_err()
    );
}

#[test]
fn native_round_trip_replays_exact_rows_and_keeps_diagnostics() {
    let (family, bytes) = fixture();
    let decoded = TerminalNumeratorSymmetryEquations::from_native_bytes(
        &bytes,
        family.clone(),
        Default::default(),
        BinaryIoLimits::unbounded_resources(),
    )
    .unwrap();
    let expected = TerminalNumeratorSymmetryEquations::prepare(
        family.clone(),
        decoded.input_keys(),
        Default::default(),
    )
    .unwrap();
    assert_eq!(decoded.statistics(), expected.statistics());
    for key in decoded.input_keys() {
        assert_eq!(decoded.equations(key), expected.equations(key));
        for proof in decoded.witnesses(key) {
            proof.replay(family.clone(), Default::default()).unwrap();
        }
    }
    assert_eq!(
        decoded
            .encode_native(BinaryIoLimits::unbounded_resources())
            .unwrap(),
        bytes
    );
}

#[test]
fn refreshed_envelope_does_not_authorize_changed_coefficients() {
    let (family, bytes) = fixture();
    let (mut record, table) = unpack(&bytes);
    let decoded = DecodedCoefficientTable::import_generated(
        &table.state,
        &table.atoms,
        BinaryIoLimits::unbounded_resources(),
    )
    .unwrap();
    let old = decoded
        .coefficient(
            CoefficientId::try_from_index(record.witnesses[0].terms[0].1 as usize).unwrap(),
        )
        .unwrap();
    let different = (0..decoded.len())
        .find(|&id| {
            let value = decoded
                .coefficient(CoefficientId::try_from_index(id).unwrap())
                .unwrap();
            !value.is_zero() && value != old
        })
        .unwrap();
    record.witnesses[0].terms[0].1 = different as u32;
    reject(
        &encode_record(&record, table, BinaryIoLimits::unbounded_resources()).unwrap(),
        family,
    );
}

#[test]
fn refreshed_envelope_does_not_authorize_changed_momentum_maps() {
    let (family, bytes) = fixture();
    let (mut record, table) = unpack(&bytes);
    let zero = zero_id(&table);
    record.witnesses[0].maps[0].entries.fill(zero);
    reject(
        &encode_record(&record, table, BinaryIoLimits::unbounded_resources()).unwrap(),
        family,
    );
}

#[test]
fn refreshed_envelope_does_not_authorize_changed_source_powers_or_guards() {
    let (family, bytes) = fixture();
    let (mut record, table) = unpack(&bytes);
    record.keys[0][0] += 1;
    reject(
        &encode_record(&record, table, BinaryIoLimits::unbounded_resources()).unwrap(),
        family.clone(),
    );
    let (mut record, table) = unpack(&bytes);
    record.witnesses[0].conditions.push(zero_id(&table));
    reject(
        &encode_record(&record, table, BinaryIoLimits::unbounded_resources()).unwrap(),
        family,
    );
}

#[test]
fn wrong_mass_family_never_reuses_a_valid_certificate() {
    let (_, bytes) = fixture();
    let changed = family(2);
    reject(&bytes, changed.clone());
    // Rewriting the identity hash is not a substitute for deriving the mass
    // constant in the complete transported numerator again.
    let (mut record, table) = unpack(&bytes);
    record.family = *blake3::hash(changed.fingerprint().as_bytes()).as_bytes();
    reject(
        &encode_record(&record, table, BinaryIoLimits::unbounded_resources()).unwrap(),
        changed,
    );
}

#[test]
fn distinct_schema_recipe_and_row_counts_are_required() {
    let (family, bytes) = fixture();
    for mutation in 0..3 {
        let (mut record, table) = unpack(&bytes);
        match mutation {
            0 => record.schema += 1,
            1 => record.recipe.push_str("-wrong"),
            _ => record.statistics.counts[6] += 1,
        }
        reject(
            &encode_record(&record, table, BinaryIoLimits::unbounded_resources()).unwrap(),
            family.clone(),
        );
    }
}

#[test]
fn corrupted_digest_and_trailing_payload_are_rejected() {
    let (family, mut bytes) = fixture();
    let mut trailing = bytes.clone();
    trailing.push(0);
    reject(&trailing, family.clone());
    let middle = bytes.len() / 2;
    bytes[middle] ^= 1;
    reject(&bytes, family.clone());
    let (_, bytes) = fixture();
    let envelope = inspect_program(&bytes, BinaryIoLimits::unbounded_resources()).unwrap();
    let mut program = envelope.section(SectionTag::PROGRAM).unwrap().to_vec();
    program.push(0);
    let refreshed = encode_program(
        BinaryProgramKind::TerminalNormalization,
        &[
            BinarySection {
                tag: SectionTag::SYMBOLICA_STATE,
                bytes: envelope.section(SectionTag::SYMBOLICA_STATE).unwrap(),
            },
            BinarySection {
                tag: SectionTag::COEFFICIENTS,
                bytes: envelope.section(SectionTag::COEFFICIENTS).unwrap(),
            },
            BinarySection {
                tag: SectionTag::PROGRAM,
                bytes: &program,
            },
        ],
        BinaryIoLimits::unbounded_resources(),
    )
    .unwrap();
    reject(&refreshed, family);
}
