use super::*;
use crate::{
    CandidateOwnerBundle, FamilyCandidatesRequest, family_candidates,
    load_generated_candidate_owners,
};
use rustred::algebra::CoefficientContext;
use rustred::family::AffineDenominator;
use serde_json::json;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

pub(super) struct Scratch(pub(super) PathBuf);
impl Scratch {
    pub(super) fn new() -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let path = std::env::temp_dir().join(format!(
            "rustred-application-test-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir(&path).unwrap();
        Self(path)
    }
}
impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn qualified(family: &IntegralFamily, power: i64) -> SavedApplicationIntegral {
    SavedApplicationIntegral {
        family_fingerprint: family.fingerprint().to_owned(),
        integral: IntegralKey::try_new([power]).unwrap(),
    }
}

fn maps_fixture() -> TerminalMaps {
    let context = CoefficientContext::try_new(["d"]).unwrap();
    let family = Arc::new(
        IntegralFamily::new(
            "native-application-codec",
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
    let guard = context
        .try_sub(
            &context.parameter("d").unwrap(),
            &context.integer(4),
            Default::default(),
        )
        .unwrap();
    let coefficient = context
        .try_div(&context.parameter("d").unwrap(), &guard, Default::default())
        .unwrap();
    TerminalMaps {
        families: BTreeMap::from([(family.fingerprint().to_owned(), family.clone())]),
        rows: BTreeMap::from([
            (
                qualified(&family, 1),
                TerminalRow {
                    terms: BTreeMap::from([(qualified(&family, 1), context.one())]),
                    conditions: Arc::new(vec![guard.clone()]),
                },
            ),
            (
                qualified(&family, 2),
                TerminalRow {
                    terms: BTreeMap::from([(qualified(&family, 1), coefficient)]),
                    conditions: Arc::new(vec![guard]),
                },
            ),
        ]),
        remaining: BTreeSet::from([qualified(&family, 1)]),
    }
}

#[test]
fn partial_saved_application_loads_policy_without_changing_recorded_coverage() {
    let scratch = Scratch::new();
    let original = prepared_fixture(&scratch.0.join("source"));
    let runtime = scratch.0.join("runtime");
    original.export_runtime(&runtime).unwrap();
    let mut strict = load_saved_application::<1>(&runtime, Default::default()).unwrap();
    let mut partial = load_saved_application::<1>(
        &runtime,
        SavedApplicationOptions {
            unresolved_policy: UnresolvedPolicy::Preserve,
            ..Default::default()
        },
    )
    .unwrap();
    let key = IntegralKey::try_new([3]).unwrap();
    let one = partial.family().coefficient_context().one();
    assert!(!partial.routed_entry_is_admitted(&key).unwrap());
    assert!(
        strict
            .reduce_routed_linear_combination([(key.clone(), one.clone())])
            .is_err()
    );
    let actual = partial
        .reduce_routed_linear_combination([(key.clone(), one.clone())])
        .unwrap();
    assert!(actual.unresolved().is_empty());
    assert_eq!(actual.terms().len(), 1);
    assert!(!partial.routed_entry_is_admitted(&key).unwrap());
    let warm = partial.reduce_linear_combination([(key, one)]).unwrap();
    assert_eq!(actual.terms(), warm.terms());
    assert_eq!(actual.nonzero_conditions(), warm.nonzero_conditions());
}

#[test]
fn partial_saved_substitution_preserves_unpublished_leaves_and_cancellation() {
    use rustred::sector::OrderingPolicy;
    use rustred::solver::{
        CandidateOwnerContext, CandidateOwnerInput, CandidateOwnerPrograms, CandidateOwnerScope,
        Integral, IntegralOrder, RoutedCandidateReducer, SectorSolution,
    };
    let scratch = Scratch::new();
    let mut application = prepared_fixture(&scratch.0.join("source"));
    let family = application
        .output_family_shared(application.family().fingerprint())
        .unwrap();
    // A deliberately incomplete owner: only I1 is declared, and no rule
    // applies to I2. This fixture tests composition, not IBP provenance.
    let context = Arc::new(
        CandidateOwnerContext::try_new(
            family.clone(),
            CandidateOwnerScope {
                max_numerator_rank: None,
                finite_case_policy: Default::default(),
            },
            vec![],
            Default::default(),
        )
        .unwrap(),
    );
    let owner = CandidateOwnerInput {
        sector: [true],
        saved_root: [true],
        ordering: OrderingPolicy::SpiredUncutV1,
        solution: SectorSolution {
            order: IntegralOrder::new([true], [false]),
            max_numerator_rank: None,
            finite_case_policy: Default::default(),
            rules: vec![],
            finite_residuals: vec![Integral::numeric([1]).unwrap()],
            stats: Default::default(),
        },
    };
    let programs = Arc::new(CandidateOwnerPrograms::try_new(context, [owner]).unwrap());
    application.application = RoutedCandidateReducer::try_new(programs, [], Default::default())
        .unwrap()
        .application();
    application
        .application
        .set_unresolved_policy(UnresolvedPolicy::Preserve)
        .unwrap();
    application.options.unresolved_policy = UnresolvedPolicy::Preserve;
    let key = IntegralKey::try_new([2]).unwrap();
    let c = family.coefficient_context();
    let result = application
        .reduce_linear_combination([(key.clone(), c.integer(7))])
        .unwrap();
    assert_eq!(
        result.terms(),
        &BTreeMap::from([(qualified(&family, 2), c.integer(7))])
    );
    assert_eq!(
        result.unresolved(),
        &BTreeSet::from([qualified(&family, 2)])
    );
    assert_eq!(
        application.remaining_terminals(),
        &BTreeSet::from([qualified(&family, 1)])
    );
    let zero = application
        .reduce_linear_combination([(key.clone(), c.one()), (key, c.integer(-1))])
        .unwrap();
    assert!(zero.terms().is_empty());
    assert!(zero.unresolved().is_empty());
}

#[test]
fn native_compact_maps_preserve_every_coefficient_context_and_guard() {
    let maps = maps_fixture();
    let options = SavedApplicationOptions::default();
    let bytes = codec::encode(&maps, options).unwrap();
    codec::verify_roundtrip(&maps, &bytes, options).unwrap();
    let restored = codec::decode(&bytes, options).unwrap();
    assert_eq!(maps.rows, restored.rows);
    assert_eq!(maps.remaining, restored.remaining);
    let mut restored_rows = restored.rows.values();
    let first_guards = &restored_rows.next().unwrap().conditions;
    assert!(restored_rows.all(|row| Arc::ptr_eq(first_guards, &row.conditions)));
    let family = maps.families.values().next().unwrap();
    let restored_family = restored.families.values().next().unwrap();
    assert!(
        family
            .coefficient_context()
            .has_same_variable_map(restored_family.coefficient_context())
    );
    let mut trailing = bytes.clone();
    trailing.push(0);
    assert!(codec::decode(&trailing, options).is_err());
    let mut short_limit = options;
    short_limit.binary_limits.max_program_bytes = bytes.len() - 1;
    assert!(codec::decode(&bytes, short_limit).is_err());
}

#[test]
fn application_progress_preserves_output_and_orders_completion_after_substitution() {
    let scratch = Scratch::new();
    let mut application = prepared_fixture(&scratch.0.join("source"));
    let input = [(
        IntegralKey::try_new([4]).unwrap(),
        application.family().coefficient_context().one(),
    )];
    let expected = application
        .reduce_linear_combination(input.clone())
        .unwrap();
    application.clear_cache().unwrap();
    let mut events = Vec::new();
    let actual = application
        .reduce_linear_combination_with_progress(input, Duration::ZERO, |event| events.push(event))
        .unwrap();
    assert_eq!(actual.terms(), expected.terms());
    assert_eq!(actual.nonzero_conditions(), expected.nonzero_conditions());
    assert_eq!(actual.statistics(), expected.statistics());
    assert_eq!(
        actual.terminal_substitution_additions(),
        expected.terminal_substitution_additions()
    );
    let core_done = events
        .iter()
        .position(|event| {
            event["phase"] == "routed_reduction" && event["core"]["status"] == "succeeded"
        })
        .unwrap();
    let substitution = events
        .iter()
        .position(|event| event["phase"] == "terminal_substitution")
        .unwrap();
    assert!(core_done < substitution);
    assert!(
        events[..events.len() - 1]
            .iter()
            .all(|event| event["request_finished"] == false)
    );
    let last = events.last().unwrap();
    assert_eq!(last["request_finished"], true);
    assert_eq!(last["status"], "succeeded");
    assert_eq!(last["completed_terminal_rows"], last["terminal_rows"]);
    assert_eq!(
        last["terminal_substitution_additions"],
        actual.terminal_substitution_additions()
    );
    assert!(last["core"]["cached"]["coefficient_bytes"].is_u64());
    assert_eq!(last["schema"], "rustred.saved-application-progress.v1");
}

#[test]
fn application_progress_reports_admission_and_terminal_substitution_failures() {
    let scratch = Scratch::new();
    let mut application = prepared_fixture(&scratch.0.join("source"));
    let one = application.family().coefficient_context().one();
    let mut events = Vec::new();
    assert!(
        application
            .reduce_linear_combination_with_progress(
                [(IntegralKey::try_new([1, 2]).unwrap(), one.clone())],
                Duration::ZERO,
                |event| events.push(event),
            )
            .is_err()
    );
    assert_eq!(events.len(), 2);
    assert_eq!(events[1]["phase"], "entry_admission");
    assert_eq!(events[1]["status"], "failed");
    assert!(events[1]["core"].is_null());
    events.clear();
    application.terminals.rows.clear();
    assert!(
        application
            .reduce_linear_combination_with_progress(
                [(IntegralKey::try_new([4]).unwrap(), one)],
                Duration::ZERO,
                |event| events.push(event),
            )
            .is_err()
    );
    let last = events.last().unwrap();
    assert_eq!(last["phase"], "terminal_substitution");
    assert_eq!(last["status"], "failed");
    assert_eq!(last["core"]["status"], "succeeded");
    assert_eq!(last["request_finished"], true);
}

#[test]
#[ignore = "bounded release timing diagnostic; reports observations, no timing assertion"]
fn application_progress_cost_probe() {
    let scratch = Scratch::new();
    let mut application = prepared_fixture(&scratch.0.join("source"));
    let input = [(
        IntegralKey::try_new([4]).unwrap(),
        application.family().coefficient_context().one(),
    )];
    let expected = application
        .reduce_linear_combination(input.clone())
        .unwrap();
    // Alternate order to reduce warming/drift bias. This small symbolic fixture
    // is deliberately a high callback-frequency test, not a five-loop forecast.
    for pair in 0..6 {
        for observed in if pair % 2 == 0 {
            [false, true]
        } else {
            [true, false]
        } {
            let started = Instant::now();
            let mut callbacks = 0usize;
            for _ in 0..1000 {
                application.clear_cache().unwrap();
                let actual = if observed {
                    application
                        .reduce_linear_combination_with_progress(
                            input.clone(),
                            Duration::from_secs(1),
                            |event| {
                                std::hint::black_box(event);
                                callbacks += 1;
                            },
                        )
                        .unwrap()
                } else {
                    application
                        .reduce_linear_combination(input.clone())
                        .unwrap()
                };
                assert_eq!(actual.terms(), expected.terms());
                assert_eq!(actual.nonzero_conditions(), expected.nonzero_conditions());
            }
            println!(
                "{}",
                json!({"event":"application_progress_cost", "pair":pair,
                "observed":observed,"requests":1000,"callbacks":callbacks,
                "seconds":started.elapsed().as_secs_f64()})
            );
        }
    }
}

#[test]
fn compact_boundary_rejects_unknown_terminals_and_impossible_guards() {
    let mut maps = maps_fixture();
    let options = SavedApplicationOptions::default();
    maps.remaining.clear();
    assert!(maps.validate(options).is_err());
    let mut maps = maps_fixture();
    let context = maps
        .families
        .values()
        .next()
        .unwrap()
        .coefficient_context()
        .clone();
    Arc::make_mut(&mut maps.rows.values_mut().next().unwrap().conditions).push(context.zero());
    assert!(codec::encode(&maps, options).is_err());
}

#[test]
fn thousands_of_overlapping_guards_keep_first_seen_order_on_warm_application() {
    let scratch = Scratch::new();
    let mut application = prepared_fixture(&scratch.0.join("source"));
    let c = application.family().coefficient_context().clone();
    let input = [(IntegralKey::try_new([2]).unwrap(), c.one())];
    let baseline = application
        .reduce_linear_combination(input.clone())
        .unwrap();
    let d = c.parameter("d").unwrap();
    let guards = (100..6244)
        .map(|n| c.try_sub(&d, &c.integer(n), Default::default()).unwrap())
        .collect::<Vec<_>>();
    let conditions = Arc::make_mut(
        &mut application
            .terminals
            .rows
            .values_mut()
            .next()
            .unwrap()
            .conditions,
    );
    conditions.extend_from_slice(&guards[2048..4096]);
    conditions.extend_from_slice(&guards[..3072]);
    conditions.extend_from_slice(&guards[1024..]);
    let mut expected = baseline.nonzero_conditions().to_vec();
    expected.extend_from_slice(&guards[2048..4096]);
    expected.extend_from_slice(&guards[..2048]);
    expected.extend_from_slice(&guards[4096..]);
    for _ in 0..2 {
        let result = application
            .reduce_linear_combination(input.clone())
            .unwrap();
        assert_eq!(result.nonzero_conditions(), expected);
        assert_eq!(result.terms(), baseline.terms());
        assert_eq!(result.statistics().work.rule_applications(), 0);
        assert!(result.statistics().work.cache_hits() > 0);
    }
}

fn prepared_fixture(source: &Path) -> PreparedSavedApplication<1> {
    std::fs::create_dir_all(source.join("objects")).unwrap();
    let source_text = r#"
schema="rustred.project.toml.v1"
[family]
name="saved_application_portable_test"
loop_momenta=["q"]
external_momenta=[]
dimension="d"
[[family.denominators]]
id="P"
expression="q^2-1"
[target]
powers=[1]
"#;
    let mut request = FamilyCandidatesRequest::new(source_text);
    request.numerical_depth = 0;
    request.max_numerator_rank = Some(2);
    let bundle = family_candidates(request).unwrap();
    let bytes = bundle.bundle();
    let (family, _) = load_generated_candidate_owners::<1>(
        &[CandidateOwnerBundle {
            bytes,
            owner_sector: &rustred::sector::Mask::try_new([true]).unwrap(),
        }],
        Default::default(),
        Default::default(),
    )
    .unwrap();
    std::fs::write(source.join("objects/owner.rrbin"), bytes).unwrap();
    let selection = json!({"family_fingerprint":family.fingerprint(),
        "owners":[{"path":"objects/owner.rrbin","bytes":bytes.len(),"mask":"1"}],
        "initial_frontier_routes":[]})
    .to_string();
    // A hole between admitted powers 2 and 4 catches rectangular-envelope bugs.
    let queries = json!({"schema":"rustred.owner-domain-queries.json.v2", "queries":[
        {"id":"two", "owner":"1", "lower":[1], "upper":[1], "max_numerator_rank":0},
        {"id":"four", "owner":"1", "lower":[3], "upper":[3], "max_numerator_rank":0}
    ]})
    .to_string();
    let context = family.coefficient_context();
    let guard = context
        .try_sub(
            &context.parameter("d").unwrap(),
            &context.integer(4),
            Default::default(),
        )
        .unwrap();
    let terminals = TerminalMaps {
        families: BTreeMap::from([(family.fingerprint().to_owned(), family.clone())]),
        rows: BTreeMap::from([(
            qualified(&family, 1),
            TerminalRow {
                terms: BTreeMap::from([(qualified(&family, 1), context.one())]),
                conditions: Arc::new(vec![guard]),
            },
        )]),
        remaining: BTreeSet::from([qualified(&family, 1)]),
    };
    let payloads =
        serde_json::from_value(json!([{"path":"objects/owner.rrbin","bytes":bytes.len(),
        "blake3":blake3::hash(bytes).to_hex().to_string()}]))
        .unwrap();
    let metadata = json!({"family_fingerprint":family.fingerprint(),"raw_terminals":1,"remaining_terminals":1,
        "source_keys_checked":1,"publication_assurance":"trusted_saved_scope","minimality_claim":false});
    PreparedSavedApplication::from_parts(
        package::ApplicationInputs {
            selection,
            queries,
            owner_base: source.to_path_buf(),
            payloads,
        },
        terminals,
        metadata,
        Default::default(),
    )
    .unwrap()
}

#[test]
fn runtime_is_portable_reuses_cache_and_keeps_exact_scope_holes() {
    let scratch = Scratch::new();
    let source = scratch.0.join("source");
    let mut original = prepared_fixture(&source);
    let context = original.family().coefficient_context().clone();
    let input = vec![
        (IntegralKey::try_new([4]).unwrap(), context.integer(3)),
        (IntegralKey::try_new([2]).unwrap(), context.integer(-2)),
    ];
    let expected = original.reduce_linear_combination(input.clone()).unwrap();
    assert!(!expected.terms().is_empty());
    assert!(!expected.nonzero_conditions().is_empty());
    let destination = scratch.0.join("runtime");
    let report = original.export_runtime(&destination).unwrap();
    assert_eq!(report["schema"], SAVED_APPLICATION_SCHEMA);
    assert!(!destination.join("state-0000000000000001.rrbin").exists());
    assert!(!destination.join("latest.json").exists());
    std::fs::rename(&source, scratch.0.join("source-away")).unwrap();
    let mut restored = load_saved_application::<1>(&destination, Default::default()).unwrap();
    let actual = restored.reduce_linear_combination(input.clone()).unwrap();
    assert_eq!(actual.terms(), expected.terms());
    assert_eq!(actual.nonzero_conditions(), expected.nonzero_conditions());
    let warm = restored.reduce_linear_combination(input.clone()).unwrap();
    assert!(warm.statistics().work.cache_hits() > 0);
    assert_eq!(warm.statistics().work.rule_applications(), 0);
    // Power 3 was necessarily cached as a descendant of power 4; neither a
    // cached answer nor a zero caller coefficient can widen the entry union.
    assert!(
        restored
            .reduce_linear_combination([(IntegralKey::try_new([3]).unwrap(), context.one())])
            .is_err()
    );
    assert!(
        restored
            .reduce_linear_combination([(IntegralKey::try_new([3]).unwrap(), context.zero())])
            .is_err()
    );
    restored.clear_cache().unwrap();
    let cold_again = restored.reduce_linear_combination(input).unwrap();
    assert!(cold_again.statistics().work.rule_applications() > 0);
    assert_eq!(cold_again.terms(), expected.terms());
    let domains = saved_application_domains::<1>(&destination).unwrap();
    assert_eq!(
        domains.iter().map(|d| d.id.as_str()).collect::<Vec<_>>(),
        ["two", "four"]
    );
    // Integrity is checked before any changed owner coefficient is imported.
    let chunk = report["payloads"][0]["chunks"][0]["path"].as_str().unwrap();
    std::fs::write(destination.join(chunk), b"changed").unwrap();
    assert!(load_saved_application::<1>(&destination, Default::default()).is_err());
}

#[test]
fn runtime_options_select_native_cache_representation_and_report_preparation() {
    use rustred::solver::CandidateCacheRepresentation;
    let scratch = Scratch::new();
    let original = prepared_fixture(&scratch.0.join("source"));
    let runtime = scratch.0.join("runtime");
    original.export_runtime(&runtime).unwrap();
    let mut expected = None;
    for representation in [
        CandidateCacheRepresentation::Sparse,
        CandidateCacheRepresentation::Factorized,
    ] {
        let options = SavedApplicationOptions {
            cache_representation: representation,
            ..Default::default()
        };
        let mut application = load_saved_application::<1>(&runtime, options).unwrap();
        assert_eq!(
            application.application.cache_representation(),
            representation
        );
        assert_eq!(application.cache_statistics().cached_integrals(), 0);
        assert_eq!(
            application.owner_preparation_statistics().prepared_owners,
            0
        );
        let timing = application.preparation_statistics();
        assert_eq!(
            timing.total,
            timing.metadata
                + timing.terminal_maps
                + timing.owner_payloads
                + timing.owner_preparation
                + timing.route_preparation
                + timing.other
        );
        let input = [(
            IntegralKey::try_new([4]).unwrap(),
            application.family().coefficient_context().one(),
        )];
        application.route_entry_to_owner(&input[0].0).unwrap();
        assert_eq!(
            application
                .owner_preparation_statistics()
                .preparation_attempts,
            0
        );
        let first = application
            .reduce_linear_combination(input.clone())
            .unwrap();
        assert_eq!(
            application.owner_preparation_statistics().prepared_owners,
            1
        );
        assert_eq!(
            application
                .owner_preparation_statistics()
                .preparation_attempts,
            1
        );
        if let Some((terms, conditions)) = &expected {
            assert_eq!(first.terms(), terms);
            assert_eq!(first.nonzero_conditions(), conditions);
        } else {
            expected = Some((first.terms().clone(), first.nonzero_conditions().to_vec()));
        }
        assert!(first.statistics().work.rule_applications() > 0);
        let warm = application
            .reduce_linear_combination(input.clone())
            .unwrap();
        assert_eq!(warm.terms(), first.terms());
        assert_eq!(warm.nonzero_conditions(), first.nonzero_conditions());
        assert_eq!(warm.statistics().work.rule_applications(), 0);
        assert!(warm.statistics().work.cache_hits() > 0);
        assert!(application.cache_statistics().cached_coefficient_bytes() > 0);
        application.clear_cache().unwrap();
        assert_eq!(
            application.owner_preparation_statistics().prepared_owners,
            1
        );
        assert_eq!(application.cache_statistics().cached_integrals(), 0);
        assert_eq!(application.cache_statistics().cached_coefficient_terms(), 0);
        assert_eq!(application.cache_statistics().cached_coefficient_bytes(), 0);
        assert_eq!(
            application.application.cache_representation(),
            representation
        );
        let after_clear = application.reduce_linear_combination(input).unwrap();
        assert_eq!(after_clear.terms(), first.terms());
        assert!(after_clear.statistics().work.rule_applications() > 0);
        assert_eq!(
            application
                .owner_preparation_statistics()
                .preparation_attempts,
            1
        );
    }
}

#[test]
fn compressed_terminal_export_and_legacy_raw_load_have_identical_maps_and_metadata() {
    let scratch = Scratch::new();
    let original = prepared_fixture(&scratch.0.join("source"));
    let runtime = scratch.0.join("runtime");
    let mut report = original.export_runtime(&runtime).unwrap();
    let descriptor: package::Payload =
        serde_json::from_value(report["terminal_maps"].clone()).unwrap();
    assert_eq!(descriptor.encoding.as_deref(), Some("gzip-chunks-v1"));
    assert!(!runtime.join(&descriptor.path).exists());
    assert!(report["selection"].get("encoding").is_none());
    assert!(report["queries"].get("encoding").is_none());
    let decoded = transport::decode(&runtime, &descriptor).unwrap();
    let native = codec::encode(&original.terminals, original.options).unwrap();
    assert_eq!(decoded, native);
    assert_eq!(descriptor.blake3, blake3::hash(&native).to_hex().as_str());
    let compressed = load_saved_application::<1>(&runtime, Default::default()).unwrap();

    // Simulate a runtime from before terminal-map compression; no native codec
    // version, guard, family definition or mathematical metadata is rewritten.
    std::fs::write(runtime.join(&descriptor.path), &native).unwrap();
    report["terminal_maps"]
        .as_object_mut()
        .unwrap()
        .remove("encoding");
    report["terminal_maps"]
        .as_object_mut()
        .unwrap()
        .remove("chunks");
    std::fs::write(
        runtime.join("application.json"),
        serde_json::to_vec(&report).unwrap(),
    )
    .unwrap();
    let raw = load_saved_application::<1>(&runtime, Default::default()).unwrap();
    for restored in [&compressed, &raw] {
        assert_eq!(restored.terminals.rows, original.terminals.rows);
        assert_eq!(restored.terminals.remaining, original.terminals.remaining);
        assert_eq!(restored.metadata, original.metadata);
        assert_eq!(restored.inputs.selection, original.inputs.selection);
        assert_eq!(restored.inputs.queries, original.inputs.queries);
    }
}

#[test]
fn saved_runtime_eviction_is_default_and_preserves_exact_refined_outputs() {
    use rustred::solver::{CandidateCacheRepresentation, CandidateRoutedCachePolicy};
    let scratch = Scratch::new();
    let mut original = prepared_fixture(&scratch.0.join("source"));
    let runtime = scratch.0.join("runtime");
    original.export_runtime(&runtime).unwrap();
    let input = [(
        IntegralKey::try_new([4]).unwrap(),
        original.family().coefficient_context().one(),
    )];
    let expected = original.reduce_linear_combination(input.clone()).unwrap();
    for representation in [
        CandidateCacheRepresentation::Sparse,
        CandidateCacheRepresentation::Factorized,
    ] {
        let mut options = SavedApplicationOptions {
            cache_representation: representation,
            ..Default::default()
        };
        options.reduction_limits.max_cached_integrals = 2;
        assert_eq!(
            options.cache_policy,
            CandidateRoutedCachePolicy::EvictLeastRecentlyUsed
        );
        let mut application = load_saved_application::<1>(&runtime, options).unwrap();
        let result = application
            .reduce_linear_combination(input.clone())
            .unwrap();
        assert_eq!(result.terms(), expected.terms());
        assert_eq!(result.nonzero_conditions(), expected.nonzero_conditions());
        assert!(result.statistics().cache.evicted_entries > 0);
        assert!(application.cache_statistics().cached_integrals() <= 2);
        assert_eq!(
            application.cache_work_statistics(),
            result.statistics().cache
        );
        let warm = application
            .reduce_linear_combination(input.clone())
            .unwrap();
        assert_eq!(warm.terms(), result.terms());
        assert_eq!(warm.nonzero_conditions(), result.nonzero_conditions());
        assert_eq!(warm.statistics().work.rule_applications(), 0);
        let work = application.cache_work_statistics();
        application.clear_cache().unwrap();
        assert_eq!(application.cache_work_statistics(), work);
        let again = application
            .reduce_linear_combination(input.clone())
            .unwrap();
        assert_eq!(again.terms(), result.terms());
        assert_eq!(again.nonzero_conditions(), result.nonzero_conditions());
        assert!(application.cache_work_statistics().evicted_entries > work.evicted_entries);
        options.cache_policy = CandidateRoutedCachePolicy::FailOnLimit;
        let mut fail_on_limit = load_saved_application::<1>(&runtime, options).unwrap();
        let error = fail_on_limit
            .reduce_linear_combination(input.clone())
            .unwrap_err();
        assert_eq!(error.kind(), crate::AppErrorKind::Execution);
        assert!(
            error.message().contains(
                &rustred::reduction::ReductionError::CacheLimit {
                    requested: 3,
                    limit: 2,
                }
                .to_string()
            )
        );
    }
}

#[test]
fn deferred_runtime_owns_authenticated_bytes_before_first_formula_use() {
    let scratch = Scratch::new();
    let mut original = prepared_fixture(&scratch.0.join("source"));
    let key = IntegralKey::try_new([2]).unwrap();
    let one = original.family().coefficient_context().one();
    let expected = original
        .reduce_linear_combination([(key.clone(), one.clone())])
        .unwrap();
    let runtime = scratch.0.join("runtime");
    let report = original.export_runtime(&runtime).unwrap();
    let mut lazy = load_saved_application::<1>(&runtime, Default::default()).unwrap();
    assert_eq!(lazy.owner_preparation_statistics().prepared_owners, 0);
    let chunk = report["payloads"][0]["chunks"][0]["path"].as_str().unwrap();
    std::fs::write(runtime.join(chunk), b"changed after boundary validation").unwrap();
    let actual = lazy.reduce_linear_combination([(key, one)]).unwrap();
    assert_eq!(actual.terms(), expected.terms());
    assert_eq!(actual.nonzero_conditions(), expected.nonzero_conditions());
    assert_eq!(lazy.owner_preparation_statistics().prepared_owners, 1);
    assert!(load_saved_application::<1>(&runtime, Default::default()).is_err());
    let invalid_workers = SavedApplicationOptions {
        preparation_workers: 0,
        ..Default::default()
    };
    assert!(
        load_saved_application::<1>(&runtime, invalid_workers)
            .err()
            .unwrap()
            .to_string()
            .contains("preparation_workers")
    );
}

#[test]
fn entry_routing_uses_only_admitted_images_and_preserves_exact_batch_coefficients() {
    let scratch = Scratch::new();
    std::fs::create_dir(scratch.0.join("objects")).unwrap();
    let source = r#"
schema="rustred.project.toml.v1"
[family]
name="saved_application_entry_bridge"
loop_momenta=["q1","q2"]
external_momenta=[]
dimension="d"
[[family.denominators]]
id="P1"
expression="q1^2-1"
[[family.denominators]]
id="P2"
expression="q2^2-1"
[[family.denominators]]
id="P3"
expression="(q1-q2)^2-1"
[target]
powers=[0,1,1]
"#;
    let mut request = FamilyCandidatesRequest::new(source);
    request.nonpositive_indices = vec![0];
    request.numerical_depth = 0;
    request.max_numerator_rank = Some(2);
    let bundle = family_candidates(request).unwrap();
    let bytes = bundle.bundle();
    let (family, programs) = load_generated_candidate_owners::<3>(
        &[CandidateOwnerBundle {
            bytes,
            owner_sector: &rustred::sector::Mask::try_new([false, true, true]).unwrap(),
        }],
        Default::default(),
        Default::default(),
    )
    .unwrap();
    let key = |powers| IntegralKey::try_new(powers).unwrap();
    let canonical = key([0, 2, 2]);
    let original = key([2, 2, 0]);
    let c = family.coefficient_context().clone();
    let direct = rustred::solver::RoutedCandidateReducer::try_new(
        Arc::new(programs),
        [],
        Default::default(),
    )
    .unwrap();
    let trace = direct.trace_targets([canonical.clone()]).unwrap();
    assert!(trace.frontier().is_empty());
    let mut rows = BTreeMap::new();
    let mut remaining = BTreeSet::new();
    for terminal in trace.declared_terminals() {
        let terminal = SavedApplicationIntegral {
            family_fingerprint: family.fingerprint().to_owned(),
            integral: terminal.clone(),
        };
        rows.insert(
            terminal.clone(),
            TerminalRow {
                terms: BTreeMap::from([(terminal.clone(), c.one())]),
                conditions: Arc::new(vec![]),
            },
        );
        remaining.insert(terminal);
    }
    assert!(!remaining.is_empty());
    std::fs::write(scratch.0.join("objects/owner.rrbin"), bytes).unwrap();
    let selection = json!({"family_fingerprint":family.fingerprint(),
        "owners":[{"path":"objects/owner.rrbin","bytes":bytes.len(),"mask":"011"}],
        "initial_frontier_routes":[{"source_mask":"110","owner_mask":"011","requires_transport":true,
            "source_to_representative":[["1","0"],["0","1"]],
            "owner_to_representative":[["1","-1"],["1","0"]]},
            {"source_mask":"101","owner_mask":"011","requires_transport":true,
            "source_to_representative":[["1","0"],["1","-1"]],
            "owner_to_representative":[["1","-1"],["1","0"]]}]}).to_string();
    let queries = json!({"schema":"rustred.owner-domain-queries.json.v2", "queries":[
        {"id":"single-owner-image", "owner":"011", "lower":[0,1,1], "upper":[0,1,1], "max_numerator_rank":0}
    ]}).to_string();
    let payloads =
        serde_json::from_value(json!([{"path":"objects/owner.rrbin","bytes":bytes.len(),
        "blake3":blake3::hash(bytes).to_hex().to_string()}]))
        .unwrap();
    let metadata = json!({"family_fingerprint":family.fingerprint(),"raw_terminals":rows.len(),
        "remaining_terminals":remaining.len(),"source_keys_checked":rows.len()});
    let mut options = SavedApplicationOptions::default();
    options.trace_limits.max_transport_calls = 1;
    let mut application = PreparedSavedApplication::<3>::from_parts(
        package::ApplicationInputs {
            selection,
            queries,
            owner_base: scratch.0.clone(),
            payloads,
        },
        TerminalMaps {
            families: BTreeMap::from([(family.fingerprint().to_owned(), family)]),
            rows,
            remaining,
        },
        metadata,
        options,
    )
    .unwrap();
    assert!(application.validate_entry(&original).is_err());
    assert!(application.routed_entry_is_admitted(&original).unwrap());
    let routed = application.route_entry_to_owner(&original).unwrap();
    assert_eq!(
        routed.terms(),
        &BTreeMap::from([(canonical.clone(), c.one())])
    );
    assert_eq!(routed.statistics().work.rule_applications(), 0);
    assert_eq!(application.cache_statistics().cached_integrals(), 0);
    let expected = application
        .reduce_linear_combination([(canonical.clone(), c.integer(3))])
        .unwrap();
    let actual = application
        .reduce_routed_linear_combination([(original.clone(), c.integer(3))])
        .unwrap();
    assert_eq!(actual.terms(), expected.terms());
    assert_eq!(actual.nonzero_conditions(), expected.nonzero_conditions());
    assert_eq!(actual.statistics().transport_calls, 1);
    assert!(actual.statistics().work.cache_hits() > 0);
    // Admission of the original strict API stays unchanged even after a
    // successful equivalent-entry request and warmed descendant caches.
    assert!(
        application
            .reduce_linear_combination([(original.clone(), c.one())])
            .is_err()
    );
    assert!(application.route_entry_to_owner(&key([0, 1, 1])).is_err());
    assert!(
        !application
            .routed_entry_is_admitted(&key([0, 1, 1]))
            .unwrap()
    );
    assert!(
        !application
            .routed_entry_is_admitted(&key([1, 1, 1]))
            .unwrap()
    );
    // A rejected geometrical witness must not abort a search that can reach an
    // admitted alternative. This does not turn an absent route into authority.
    let mut accepted = None;
    for candidate in [key([1, 1, 1]), original.clone()] {
        if application.routed_entry_is_admitted(&candidate).unwrap() {
            accepted = Some(candidate);
            break;
        }
    }
    assert_eq!(accepted, Some(original.clone()));
    assert!(
        application
            .routed_entry_is_admitted(&IntegralKey::try_new([1]).unwrap())
            .is_err()
    );
    assert!(application.route_entry_to_owner(&key([1, 1, 1])).is_err());
    assert!(
        application
            .reduce_routed_linear_combination([(key([1, 1, 1]), c.one())])
            .is_err()
    );
    assert!(
        application
            .reduce_routed_linear_combination([(key([2, 2, -1]), c.zero())])
            .is_err()
    );
    // Two individually valid transports cannot each reset the one-call budget,
    // even when their final root coefficients would cancel exactly.
    assert!(
        application
            .reduce_routed_linear_combination([
                (original.clone(), c.one()),
                (original, c.integer(-1)),
            ])
            .is_err()
    );
    // Product preparation is optional presentation over the unchanged flat
    // terminal basis; it neither evaluates lower masters nor clears work.
    let flat = application.remaining_terminals().clone();
    let cache = application.cache_statistics();
    let statistics = application
        .prepare_terminal_products(Default::default())
        .unwrap();
    assert!(statistics.work.factored_terminals > 0);
    assert_eq!(application.remaining_terminals(), &flat);
    assert_eq!(application.cache_statistics(), cache);
    assert_eq!(
        application
            .prepare_terminal_products(Default::default())
            .unwrap(),
        statistics
    );
    assert_eq!(application.terminal_product_statistics(), Some(&statistics));
    for terminal in actual.terms().keys() {
        let product = application.terminal_product(terminal).unwrap();
        assert_eq!(product.coefficient(), &c.one());
        assert_eq!(product.factors().len(), 2);
        assert!(product.factors().iter().all(|factor| {
            application
                .output_family(factor.family_fingerprint())
                .unwrap()
                .loop_count()
                == 1
        }));
    }
    assert!(
        application
            .prepare_terminal_products(
                rustred::reduction::terminal_normalization::VacuumProductLimits {
                    max_terminals: 0,
                    ..Default::default()
                }
            )
            .is_err()
    );
    assert_eq!(application.terminal_product_statistics(), Some(&statistics));
    let after_products = application
        .reduce_linear_combination([(canonical, c.integer(3))])
        .unwrap();
    assert_eq!(after_products.terms(), actual.terms());
    let runtime = scratch.0.join("runtime");
    application.export_runtime(&runtime).unwrap();
    let mut serial = load_saved_application::<3>(&runtime, options).unwrap();
    let mut parallel = load_saved_application::<3>(
        &runtime,
        SavedApplicationOptions {
            preparation_workers: 2,
            ..options
        },
    )
    .unwrap();
    assert_eq!(serial.owner_preparation_statistics().prepared_owners, 0);
    assert_eq!(parallel.owner_preparation_statistics().prepared_owners, 0);
    assert!(parallel.routed_entry_is_admitted(&key([2, 2, 0])).unwrap());
    assert!(!parallel.routed_entry_is_admitted(&key([0, 1, 1])).unwrap());
    assert_eq!(
        parallel.owner_preparation_statistics().preparation_attempts,
        0
    );
    for target in [key([2, 2, 0]), key([2, 0, 2])] {
        let serial_route = serial.route_entry_to_owner(&target).unwrap();
        let parallel_route = parallel.route_entry_to_owner(&target).unwrap();
        assert_eq!(serial_route.terms(), parallel_route.terms());
        assert_eq!(
            serial_route.nonzero_conditions(),
            parallel_route.nonzero_conditions()
        );
        let expected = serial
            .reduce_routed_linear_combination([(target.clone(), c.one())])
            .unwrap();
        let actual = parallel
            .reduce_routed_linear_combination([(target, c.one())])
            .unwrap();
        assert_eq!(actual.terms(), expected.terms());
        assert_eq!(actual.nonzero_conditions(), expected.nonzero_conditions());
    }
    assert_eq!(
        parallel.owner_preparation_statistics().preparation_attempts,
        1
    );
}
