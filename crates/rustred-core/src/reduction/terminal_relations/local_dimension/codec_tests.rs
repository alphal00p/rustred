use super::*;
use crate::algebra::CoefficientContext;
use crate::family::AffineDenominator;

fn tadpole_family(mass: i64) -> Arc<IntegralFamily> {
    let context = CoefficientContext::new(["d"]);
    Arc::new(
        IntegralFamily::new(
            "native_local_dimension_tadpole",
            vec!["q".into()],
            vec![],
            context.clone(),
            context.parameter("d").unwrap(),
            vec![AffineDenominator::new(
                context.integer(-mass),
                vec![context.one()],
            )],
            vec![],
            vec![context.zero()],
        )
        .unwrap(),
    )
}

fn fixture() -> (Arc<IntegralFamily>, LocalDimensionRecurrencePlan, Vec<u8>) {
    let family = tadpole_family(3);
    let keys = BTreeSet::from([
        IntegralKey::try_new([1]).unwrap(),
        IntegralKey::try_new([2]).unwrap(),
    ]);
    let plan = LocalDimensionRecurrencePlan::prepare(
        family.clone(),
        &keys,
        Default::default(),
        &Default::default(),
    )
    .unwrap();
    assert!(!plan.witnesses.is_empty());
    let bytes = plan
        .encode_native(BinaryIoLimits::unbounded_resources())
        .unwrap();
    (family, plan, bytes)
}

fn rewrite(bytes: &[u8], edit: impl FnOnce(&mut Record)) -> Vec<u8> {
    let io = BinaryIoLimits::unbounded_resources();
    let envelope = inspect_program(bytes, io).unwrap();
    let payload = envelope.section(SectionTag::PROGRAM).unwrap();
    let (mut record, consumed): (Record, usize) =
        bincode::decode_from_slice(payload, bincode::config::standard()).unwrap();
    assert_eq!(consumed, payload.len());
    edit(&mut record);
    let program = bincode::encode_to_vec(record, bincode::config::standard()).unwrap();
    encode_program(
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
        io,
    )
    .unwrap()
}

fn reject(family: &Arc<IntegralFamily>, bytes: &[u8]) {
    assert!(
        LocalDimensionRecurrencePlan::from_native_bytes(
            bytes,
            family.clone(),
            Default::default(),
            BinaryIoLimits::unbounded_resources(),
        )
        .is_err()
    );
}

#[test]
fn native_local_dimension_round_trip_replays_sources_and_complete_exact_equations() {
    let (family, original, bytes) = fixture();
    let loaded = LocalDimensionRecurrencePlan::from_native_bytes(
        &bytes,
        family.clone(),
        Default::default(),
        BinaryIoLimits::unbounded_resources(),
    )
    .unwrap();
    assert_eq!(loaded.family_fingerprint(), original.family_fingerprint());
    assert_eq!(loaded.input_keys(), original.input_keys());
    assert_eq!(loaded.inventory(), original.inventory());
    assert_eq!(loaded.witnesses, original.witnesses);
    assert_eq!(loaded.equations, original.equations);
    assert_eq!(loaded.skips, original.skips);
    assert_eq!(loaded.statistics(), original.statistics());
    let envelope = inspect_program(&bytes, BinaryIoLimits::unbounded_resources()).unwrap();
    assert_eq!(
        envelope
            .sections()
            .iter()
            .map(|section| section.tag)
            .collect::<Vec<_>>(),
        [
            SectionTag::SYMBOLICA_STATE,
            SectionTag::COEFFICIENTS,
            SectionTag::PROGRAM
        ]
    );
    assert_eq!(
        decode_claims(
            &loaded
                .encode_native(BinaryIoLimits::unbounded_resources())
                .unwrap(),
            &family,
            BinaryIoLimits::unbounded_resources()
        )
        .unwrap(),
        decode_claims(&bytes, &family, BinaryIoLimits::unbounded_resources()).unwrap(),
    );
}

#[test]
fn native_local_dimension_rejects_schema_recipe_family_and_claimed_counts() {
    let (family, _, bytes) = fixture();
    let mutations: [fn(&mut Record); 5] = [
        |record| record.schema += 1,
        |record| record.recipe.push_str("-untrusted"),
        |record| record.family[0] ^= 1,
        |record| record.statistics[5] += 1,
        |record| record.statistics[6] += 1,
    ];
    for mutation in mutations {
        reject(&family, &rewrite(&bytes, mutation));
    }
    reject(&tadpole_family(4), &bytes);
}

#[test]
fn native_local_dimension_rejects_changed_originals_or_noncanonical_inventory() {
    let (family, _, bytes) = fixture();
    let mutations: [fn(&mut Record); 6] = [
        |record| record.keys.push(record.keys[0].clone()),
        |record| record.keys.swap(0, 1),
        |record| record.keys[0].push(0),
        |record| {
            record.inventory.pop();
        },
        |record| record.inventory.push(vec![999]),
        |record| record.witnesses[0].source = u64::MAX,
    ];
    for mutation in mutations {
        reject(&family, &rewrite(&bytes, mutation));
    }
}

#[test]
fn native_local_dimension_rejects_changed_or_omitted_weighted_ordinary_sources() {
    let (family, _, bytes) = fixture();
    let mutations: [fn(&mut Record); 6] = [
        |record| record.witnesses[0].sources[0].ordinal += 1,
        |record| record.witnesses[0].sources[0].seed[0] += 1,
        |record| record.witnesses[0].sources.clear(),
        |record| {
            let duplicate = record.witnesses[0].sources[0].clone();
            record.witnesses[0].sources.push(duplicate);
        },
        |record| record.witnesses[0].sources[0].weight = u32::MAX,
        |record| {
            let current = record.witnesses[0].sources[0].weight;
            let altered = record.witnesses[0]
                .equation
                .terms
                .iter()
                .map(|(_, coefficient)| *coefficient)
                .find(|coefficient| *coefficient != current)
                .expect("tadpole recurrence has a nonconstant coefficient");
            record.witnesses[0].sources[0].weight = altered;
        },
    ];
    for mutation in mutations {
        reject(&family, &rewrite(&bytes, mutation));
    }
}

#[test]
fn native_local_dimension_rejects_direction_spectator_or_cohort_mutations() {
    let (family, _, bytes) = fixture();
    let mutations: [fn(&mut Record); 5] = [
        |record| record.witnesses[0].direction.clear(),
        |record| record.witnesses[0].direction[0] = u32::MAX,
        |record| record.witnesses[0].spectator_rank += 1,
        |record| {
            record.witnesses.pop();
        },
        |record| record.witnesses.push(record.witnesses[0].clone()),
    ];
    for mutation in mutations {
        reject(&family, &rewrite(&bytes, mutation));
    }
}

#[test]
fn native_local_dimension_rejects_row_terms_guards_and_unknown_geometry_claims() {
    let (family, _, bytes) = fixture();
    let mutations: [fn(&mut Record); 6] = [
        |record| {
            record.witnesses[0].equation.terms.pop();
        },
        |record| record.witnesses[0].equation.terms[0].0 = u64::MAX,
        |record| record.witnesses[0].equation.terms[0].1 = u32::MAX,
        |record| {
            let one = record.witnesses[0].direction[0];
            record.witnesses[0].equation.conditions.push(one);
        },
        |record| {
            record.skips.push(SkippedRecord {
                source: 0,
                direction: record.witnesses[0].direction.clone(),
                reason: "invented omission".into(),
            })
        },
        |record| record.skipped_reasons.push(("invented omission".into(), 1)),
    ];
    for mutation in mutations {
        reject(&family, &rewrite(&bytes, mutation));
    }
}

#[test]
fn native_local_dimension_rejects_trailing_or_corrupted_native_payloads() {
    let (family, _, bytes) = fixture();
    let mut corrupted = bytes.clone();
    *corrupted.last_mut().unwrap() ^= 1;
    reject(&family, &corrupted);
    let mut trailing = bytes;
    trailing.push(0);
    reject(&family, &trailing);
}

#[test]
fn native_local_dimension_honors_caller_io_and_replay_cancellation() {
    let (family, plan, bytes) = fixture();
    let io = BinaryIoLimits {
        max_collection_entries: 0,
        ..BinaryIoLimits::unbounded_resources()
    };
    assert!(plan.encode_native(io).is_err());
    assert!(
        LocalDimensionRecurrencePlan::from_native_bytes(
            &bytes,
            family.clone(),
            Default::default(),
            io
        )
        .is_err()
    );
    let cancelled = || true;
    assert_eq!(
        LocalDimensionRecurrencePlan::from_native_bytes_with_control(
            &bytes,
            family,
            Default::default(),
            BinaryIoLimits::unbounded_resources(),
            &LocalDimensionRecurrenceControl {
                cancelled: Some(&cancelled),
                progress: None
            },
        )
        .err(),
        Some(Error::Cancelled),
    );
}

#[test]
fn native_local_dimension_preserves_nomination_level_parameter_geometry_refusals() {
    let context = CoefficientContext::new(["d", "x"]);
    let family = Arc::new(
        IntegralFamily::new(
            "parameter_valued_local_geometry",
            vec!["q".into()],
            vec![],
            context.clone(),
            context.parameter("d").unwrap(),
            vec![AffineDenominator::new(
                context.integer(-3),
                vec![context.parameter("x").unwrap()],
            )],
            vec![],
            vec![context.zero()],
        )
        .unwrap(),
    );
    let keys = BTreeSet::from([IntegralKey::try_new([1]).unwrap()]);
    let plan = LocalDimensionRecurrencePlan::prepare(
        family.clone(),
        &keys,
        Default::default(),
        &Default::default(),
    )
    .unwrap();
    assert!(plan.witnesses.is_empty());
    assert!(plan.equations.is_empty());
    assert_eq!(plan.skips.len(), 1);
    assert!(plan.skips[0].direction.is_empty());
    assert!(!plan.skips[0].reason.is_empty());
    let bytes = plan
        .encode_native(BinaryIoLimits::unbounded_resources())
        .unwrap();
    let loaded = LocalDimensionRecurrencePlan::from_native_bytes(
        &bytes,
        family.clone(),
        Default::default(),
        BinaryIoLimits::unbounded_resources(),
    )
    .unwrap();
    assert_eq!(loaded.skips, plan.skips);
    assert_eq!(loaded.statistics(), plan.statistics());
    reject(
        &family,
        &rewrite(&bytes, |record| {
            record.skips[0].reason.push_str(" altered");
        }),
    );
    reject(
        &family,
        &rewrite(&bytes, |record| {
            record.skips.clear();
            record.skipped_reasons.clear();
            record.statistics[4] = 0;
        }),
    );
}
