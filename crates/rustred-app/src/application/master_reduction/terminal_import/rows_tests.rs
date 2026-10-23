use super::*;
use rustred::algebra::CoefficientContext;
use rustred::family::AffineDenominator;

fn family(name: &str) -> IntegralFamily {
    let context = CoefficientContext::try_new(["d"]).unwrap();
    IntegralFamily::new(
        name,
        vec!["k".into()],
        vec![],
        context.clone(),
        context.parameter("d").unwrap(),
        vec![AffineDenominator::new(
            context.integer(-1),
            vec![context.integer(1)],
        )],
        vec![],
        vec![context.zero()],
    )
    .unwrap()
}

fn equations(family: &IntegralFamily) -> Vec<TerminalEquation> {
    let context = family.coefficient_context();
    vec![TerminalEquation {
        terms: BTreeMap::from([
            (
                IntegralKey::try_new([-1]).unwrap(),
                context.parameter("d").unwrap(),
            ),
            (IntegralKey::try_new([2]).unwrap(), context.integer(-1)),
        ]),
        nonzero_conditions: vec![context.parameter("d").unwrap()],
    }]
}

fn rewrite(bytes: &[u8], change: impl FnOnce(&mut Record)) -> Vec<u8> {
    let limits = super::super::super::profile::native_io_limits(None);
    let envelope = inspect_program(bytes, limits).unwrap();
    let (mut record, _): (Record, usize) = bincode::decode_from_slice(
        envelope.section(SectionTag::PROGRAM).unwrap(),
        bincode::config::standard(),
    )
    .unwrap();
    change(&mut record);
    let program = bincode::encode_to_vec(record, bincode::config::standard()).unwrap();
    encode_program(
        BinaryProgramKind::TerminalRelations,
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
        limits,
    )
    .unwrap()
}

#[test]
fn exact_equation_rows_roundtrip_and_bind_to_family_and_authority() {
    let family = family("native-terminal-equations");
    let rows = equations(&family);
    let encoded = encode(&family, &rows, "proof-binding").unwrap();
    assert_eq!(decode(&encoded, &family, "proof-binding").unwrap(), rows);
    assert_eq!(encode(&family, &rows, "proof-binding").unwrap(), encoded);
    assert!(decode(&encoded, &family, "other-binding").is_err());
    assert!(encode(&family, &rows, "").is_err());
    assert_eq!(
        decode(&encode(&family, &[], "empty").unwrap(), &family, "empty").unwrap(),
        vec![]
    );
    let changed = rewrite(&encoded, |record| record.family_fingerprint.push('x'));
    assert!(decode(&changed, &family, "proof-binding").is_err());
    let mut guarded_zero = equations(&family).remove(0);
    guarded_zero.terms.clear();
    guarded_zero
        .nonzero_conditions
        .push(guarded_zero.nonzero_conditions[0].clone());
    let rows = vec![guarded_zero];
    assert_eq!(
        decode(
            &encode(&family, &rows, "guards").unwrap(),
            &family,
            "guards"
        )
        .unwrap(),
        rows
    );
}

#[test]
fn noncanonical_keys_atoms_guards_and_schema_fail_closed() {
    let family = family("native-terminal-equations-controls");
    let encoded = encode(&family, &equations(&family), "proof-binding").unwrap();
    for changed in [
        rewrite(&encoded, |record| record.schema = 2),
        rewrite(&encoded, |record| record.arity = 2),
        rewrite(&encoded, |record| record.rows[0].terms.reverse()),
        rewrite(&encoded, |record| record.rows[0].terms[0].0.clear()),
        rewrite(&encoded, |record| {
            record.rows[0].terms[1].0 = record.rows[0].terms[0].0.clone()
        }),
        rewrite(&encoded, |record| record.rows[0].terms[0].1 = 3),
        rewrite(&encoded, |record| record.rows[0].terms[0].1 = 1),
        rewrite(&encoded, |record| record.rows.clear()),
    ] {
        assert!(decode(&changed, &family, "proof-binding").is_err());
    }
    let mut rows = equations(&family);
    rows[0].terms.insert(
        IntegralKey::try_new([2]).unwrap(),
        family.coefficient_context().zero(),
    );
    assert!(encode(&family, &rows, "proof-binding").is_err());
    rows = equations(&family);
    rows[0].nonzero_conditions = vec![family.coefficient_context().zero()];
    assert!(encode(&family, &rows, "proof-binding").is_err());
}
