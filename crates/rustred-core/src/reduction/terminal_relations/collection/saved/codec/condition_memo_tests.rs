//! Borrowed guard memoization must preserve the original encoder exactly.
use super::*;
use crate::family::AffineDenominator;

fn guarded_session() -> TerminalRelationSession {
    let context = CoefficientContext::new(["d"]);
    let family = Arc::new(
        IntegralFamily::new(
            "condition-memo-native-oracle",
            vec!["q".into()],
            vec![],
            context.clone(),
            context.parameter("d").unwrap(),
            vec![AffineDenominator::new(
                context.integer(-1),
                vec![context.one()],
            )],
            vec![],
            vec![context.zero()],
        )
        .unwrap(),
    );
    let keys = (1..=4)
        .map(|power| IntegralKey::try_new([power]).unwrap())
        .collect();
    let mut session = TerminalRelationSession::new(family, keys, 0, Default::default()).unwrap();
    session.conditions = ["d-7", "d-3", "7-d", "(d-7)*(d-3)", "d-7"]
        .map(|value| context.coefficient_fixture(value))
        .into();
    session
}

fn encode_uncached(plan: &TerminalCollectionPlan, io: BinaryIoLimits) -> Result<Vec<u8>> {
    let (record, table) = record_uncached(plan, io)?;
    encode_record(record, table, io)
}

#[test]
fn repeated_borrow_reuses_ids_and_retains_the_exact_slice() {
    let context = CoefficientContext::new(["d"]);
    let values = ["d-4", "d-3", "d-4", "3-d", "1"].map(|value| context.coefficient_fixture(value));
    let mut cached = Intern::new(Default::default());
    let mut uncached = Intern::new_uncached(Default::default());
    let expected = uncached.conditions(&values).unwrap();
    assert_eq!(expected, [0, 1, 0, 2, 3]);
    for _ in 0..4 {
        assert_eq!(cached.conditions(&values).unwrap(), expected);
        assert_eq!(uncached.conditions(&values).unwrap(), expected);
    }
    assert_eq!(cached.condition_ids.len(), 1);
    assert!(uncached.condition_ids.is_empty());
    assert_eq!(cached.native.len(), uncached.native.len());
    let entry = &cached.condition_ids[&(values.as_ptr(), values.len())];
    assert!(std::ptr::eq(entry.values, values.as_slice()));
    let mut returned = cached.conditions(&values).unwrap();
    returned.clear();
    assert_eq!(cached.conditions(&values).unwrap(), expected);
}

#[test]
fn equal_fresh_allocations_and_different_slice_lengths_have_distinct_memo_entries() {
    let context = CoefficientContext::new(["d"]);
    let first = vec![
        context.coefficient_fixture("d-4"),
        context.coefficient_fixture("d-3"),
        context.coefficient_fixture("d-4"),
    ];
    let second = first.clone();
    assert_ne!(first.as_ptr(), second.as_ptr());
    let mut table = Intern::new(Default::default());
    assert_eq!(table.conditions(&first).unwrap(), [0, 1, 0]);
    assert_eq!(table.conditions(&second).unwrap(), [0, 1, 0]);
    assert_eq!(table.conditions(&first[..1]).unwrap(), [0]);
    assert_eq!(table.conditions(&first[1..]).unwrap(), [1, 0]);
    assert_eq!(table.condition_ids.len(), 4);
    assert_eq!(table.native.len(), 2);
}

#[test]
fn empty_conditions_do_not_consume_memo_or_coefficient_grants() {
    let io = BinaryIoLimits {
        max_collection_entries: 0,
        ..Default::default()
    };
    let context = CoefficientContext::new(["d"]);
    let owner = [context.coefficient_fixture("d-4")];
    let mut table = Intern::new(io);
    for values in [&[][..], &owner[..0], &owner[1..]] {
        assert!(table.conditions(values).unwrap().is_empty());
    }
    assert!(table.condition_ids.is_empty());
    assert!(table.native.is_empty());
}

#[test]
fn cached_and_uncached_native_tables_have_identical_bytes_and_encounter_order() {
    let context = CoefficientContext::new(["d"]);
    let values = ["d-4", "d-3", "d-4", "1/(d-4)"].map(|value| context.coefficient_fixture(value));
    let fresh = values.to_vec();
    let mut cached = Intern::new(Default::default());
    let mut uncached = Intern::new_uncached(Default::default());
    for values in [values.as_slice(), values.as_slice(), fresh.as_slice()] {
        assert_eq!(
            cached.conditions(values).unwrap(),
            uncached.conditions(values).unwrap()
        );
        // A coefficient encountered between repeated guards must retain its ID.
        assert_eq!(
            cached.value(&context.one()).unwrap(),
            uncached.value(&context.one()).unwrap()
        );
    }
    let cached = cached.native.finish().unwrap();
    let uncached = uncached.native.finish().unwrap();
    assert_eq!(cached.state, uncached.state);
    assert_eq!(cached.atoms, uncached.atoms);
}

#[test]
fn failed_guard_fill_preserves_caller_limits_and_never_caches_a_partial_result() {
    let context = CoefficientContext::new(["d"]);
    let values = ["d-4", "d-3", "d-2"].map(|value| context.coefficient_fixture(value));
    let io = BinaryIoLimits {
        max_collection_entries: 2,
        ..Default::default()
    };
    let mut cached = Intern::new(io);
    let mut uncached = Intern::new_uncached(io);
    for _ in 0..2 {
        let expected = uncached.conditions(&values).unwrap_err().to_string();
        let actual = cached.conditions(&values).unwrap_err().to_string();
        assert_eq!(actual, expected);
        assert!(actual.contains("coefficient table entries"));
        assert!(cached.condition_ids.is_empty());
        assert_eq!(cached.native.len(), 2);
    }
    assert_eq!(cached.conditions(&values[..2]).unwrap(), [0, 1]);
    assert_eq!(cached.conditions(&values[..2]).unwrap(), [0, 1]);
    assert_eq!(cached.condition_ids.len(), 1);
}

#[test]
fn inherited_shared_guards_preserve_full_records_and_native_wire_bytes() {
    let session = guarded_session();
    let plan = TerminalCollectionPlan::from_sessions(&[&session], Default::default()).unwrap();
    let shared: Vec<_> = session
        .raw_terminals()
        .iter()
        .map(|key| {
            plan.apply(session.family_owner(), key)
                .unwrap()
                .shared_nonzero_conditions()
        })
        .collect();
    assert_eq!(shared.len(), 4);
    assert_eq!(shared[0].len(), 4);
    assert!(shared.iter().all(|values| Arc::ptr_eq(values, &shared[0])));
    for io in [Default::default(), BinaryIoLimits::unbounded_resources()] {
        let (cached_record, cached_table) = record(&plan, io).unwrap();
        let (uncached_record, uncached_table) = record_uncached(&plan, io).unwrap();
        assert_eq!(cached_record, uncached_record);
        assert!(cached_table.condition_ids.len() < cached_record.maps.len());
        assert!(uncached_table.condition_ids.is_empty());
        let expected = encode_record(uncached_record, uncached_table, io).unwrap();
        assert_eq!(
            encode_record(cached_record, cached_table, io).unwrap(),
            expected
        );
        assert_eq!(encode(&plan, io).unwrap(), expected);
    }
}

#[test]
fn memoized_collection_matches_uncached_limit_failures_and_cold_roundtrip() {
    let session = guarded_session();
    let plan = TerminalCollectionPlan::from_sessions(&[&session], Default::default()).unwrap();
    let io = BinaryIoLimits::unbounded_resources();
    let bytes = encode_uncached(&plan, io).unwrap();
    let envelope = inspect_program(&bytes, io).unwrap();
    let structural_bytes = envelope.section(SectionTag::PROGRAM).unwrap().len();
    for limited in [
        BinaryIoLimits {
            max_collection_entries: 0,
            ..io
        },
        BinaryIoLimits {
            max_collection_entries: 4,
            ..io
        },
        BinaryIoLimits {
            max_atom_bytes: 0,
            ..io
        },
        BinaryIoLimits {
            max_total_atom_bytes: 0,
            ..io
        },
        BinaryIoLimits {
            max_program_bytes: structural_bytes - 1,
            ..io
        },
    ] {
        assert_eq!(
            encode(&plan, limited).unwrap_err().to_string(),
            encode_uncached(&plan, limited).unwrap_err().to_string(),
            "caller limits: {limited:?}"
        );
    }
    let loaded = decode(&bytes, &[&session], Default::default(), io).unwrap();
    assert_eq!(loaded.statistics(), plan.statistics());
    assert_eq!(loaded.remaining_terminals(), plan.remaining_terminals());
    for key in session.raw_terminals() {
        let expected = plan.apply(session.family_owner(), key).unwrap();
        let actual = loaded.apply(session.family_owner(), key).unwrap();
        assert_eq!(actual.terms(), expected.terms());
        assert_eq!(actual.nonzero_conditions(), expected.nonzero_conditions());
    }
    assert_eq!(encode(&loaded, io).unwrap(), bytes);
    assert_eq!(encode_uncached(&loaded, io).unwrap(), bytes);
}
