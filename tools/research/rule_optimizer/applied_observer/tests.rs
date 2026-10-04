use super::{input::*, record::*};
use rustred::{
    persistence::{CoefficientId, DecodedCoefficientTable},
    solver::{
        OwnerAppliedEvent, OwnerAppliedLimits, OwnerAppliedNonzero, OwnerAppliedStats,
        OwnerAppliedSuccessor,
    },
};
use rustred_app::{
    FamilyCandidatesRequest, FiniteCasePolicy, RoutedCampaignRequest, RoutedFeedbackOptions,
    RoutedFeedbackSession, family_candidates, inspect_generated_candidate_bundle,
};
use serde_json::{Value, json};
use std::{
    fs,
    path::PathBuf,
    sync::atomic::{AtomicBool, AtomicU64, Ordering},
};

const FAMILY: &str = r#"
schema="rustred.project.toml.v1"
[family]
name="applied_observer_public_fixture"
loop_momenta=["q"]
external_momenta=[]
dimension="d"
[[family.denominators]]
id="P"
expression="q^2-1"
[target]
powers=[1]
"#;
struct Directory(PathBuf);
impl Directory {
    fn new() -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let path = std::env::temp_dir().join(format!(
            "rustred-applied-observer-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&path).unwrap();
        Self(path)
    }
}
impl Drop for Directory {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}
fn limits() -> RecorderLimits {
    RecorderLimits {
        max_input_bytes: 1 << 20,
        max_queries: 32,
        max_events: 1000,
        max_json_bytes: 16 << 20,
        max_record_bytes: 1 << 20,
        max_coefficients: 1000,
        max_atom_bytes: 1 << 20,
        max_total_atom_bytes: 4 << 20,
        max_state_bytes: 4 << 20,
        max_error_bytes: 256,
    }
}
fn query() -> Query {
    Query {
        id: "physical-two".into(),
        owner: "1".into(),
        lower: vec![1],
        upper: vec![Some(1)],
        max_numerator_rank: Some(0),
        power_bounds: Powers {
            max_positive_power: Some(2),
            min_power_difference: Some(2),
            max_power_difference: Some(2),
        },
    }
}
fn session(directory: &Directory, cancel: &AtomicBool) -> Option<RoutedFeedbackSession<1>> {
    let mut request = FamilyCandidatesRequest::new(FAMILY);
    request.numerical_depth = 0;
    request.max_numerator_rank = Some(2);
    let generated = family_candidates(request).unwrap();
    let bytes = generated.bundle();
    let inspection = inspect_generated_candidate_bundle(bytes, Default::default()).unwrap();
    fs::write(directory.0.join("owner.rrbin"), bytes).unwrap();
    let selection = json!({"family_fingerprint":inspection.family_fingerprint,
        "owners":[{"path":"owner.rrbin","bytes":bytes.len(),"mask":"1"}],"initial_frontier_routes":[]});
    let mut request = RoutedCampaignRequest::new(selection.to_string(), "2".to_owned());
    request.owner_base = directory.0.clone();
    RoutedFeedbackSession::prepare(
        request,
        RoutedFeedbackOptions::new(Default::default(), FiniteCasePolicy::SearchFinite),
        cancel,
        |_| {},
    )
    .unwrap()
}
fn read_json(directory: &Directory, name: &str) -> Value {
    serde_json::from_slice(&fs::read(directory.0.join(name)).unwrap()).unwrap()
}
fn records(directory: &Directory) -> Vec<Value> {
    fs::read_to_string(directory.0.join("events.jsonl"))
        .unwrap()
        .lines()
        .map(|s| serde_json::from_str(s).unwrap())
        .collect()
}

#[test]
fn native_singleton_preserves_translated_caps_and_coefficient_table_roundtrip() {
    let directory = Directory::new();
    let cancel = AtomicBool::new(false);
    let session = session(&directory, &cancel).unwrap();
    let mut recorder = Recorder::new(&directory.0, limits()).unwrap();
    let query = query();
    recorder.begin_query(0, &query).unwrap();
    let mut coefficients = Vec::new();
    let result = session
        .programs()
        .visit_power_bounded_owner_applied_successors(
            [true],
            &query.lower,
            &query.upper,
            query.max_numerator_rank,
            query.power_bounds.native(),
            OwnerAppliedLimits::default(),
            &cancel,
            |event| {
                if let OwnerAppliedEvent::Successor(child) = &event {
                    coefficients.push(child.coefficient.raw().clone());
                }
                recorder.event(event)
            },
        );
    assert!(recorder.end_query(result, 0.0).unwrap());
    assert!(recorder.finish(json!({}), true).unwrap());
    let events = records(&directory);
    let child = events.iter().find(|v| v["event"] == "successor").unwrap();
    assert_eq!(child["source_lower"], json!([1]));
    assert_eq!(child["source_upper"], json!([1]));
    assert_eq!(child["target_lower"], json!([0]));
    assert_eq!(child["target_upper"], json!([0]));
    assert_eq!(
        child["target_power_bounds"],
        json!({"max_positive_power":1,"min_power_difference":1,"max_power_difference":1})
    );
    assert_eq!(child["target_rank_limit"], 0);
    let decoded = DecodedCoefficientTable::import_generated(
        &fs::read(directory.0.join("coefficients.state")).unwrap(),
        &fs::read(directory.0.join("coefficients.atoms")).unwrap(),
        Default::default(),
    )
    .unwrap();
    let children: Vec<_> = events
        .iter()
        .filter(|v| v["event"] == "successor")
        .collect();
    assert_eq!(children.len(), coefficients.len());
    for (child, coefficient) in children.iter().zip(&coefficients) {
        let id =
            CoefficientId::try_from_index(child["coefficient"].as_u64().unwrap() as usize).unwrap();
        assert_eq!(decoded.coefficient(id).unwrap(), coefficient);
    }
}

#[test]
fn consumer_cap_after_successor_never_claims_complete() {
    let directory = Directory::new();
    let cancel = AtomicBool::new(false);
    let session = session(&directory, &cancel).unwrap();
    let mut caps = limits();
    caps.max_events = 2;
    let mut recorder = Recorder::new(&directory.0, caps).unwrap();
    let query = query();
    recorder.begin_query(0, &query).unwrap();
    let result = session
        .programs()
        .visit_power_bounded_owner_applied_successors(
            [true],
            &query.lower,
            &query.upper,
            query.max_numerator_rank,
            query.power_bounds.native(),
            Default::default(),
            &cancel,
            |event| recorder.event(event),
        );
    assert!(result.is_err());
    assert!(!recorder.end_query(result, 0.0).unwrap());
    assert!(!recorder.finish(json!({}), false).unwrap());
    let receipt = read_json(&directory, "query-000000.json");
    assert_eq!(receipt["delivered"]["successors"], 1);
    assert_eq!(receipt["delivered"]["finished"], 0);
    assert_eq!(receipt["complete_outcome"], false);
}

#[test]
fn conditional_event_retains_original_source_cell_and_native_coefficient() {
    let directory = Directory::new();
    let cancel = AtomicBool::new(false);
    let session = session(&directory, &cancel).unwrap();
    let mut recorder = Recorder::new(&directory.0, limits()).unwrap();
    let query = query();
    recorder.begin_query(0, &query).unwrap();
    // A recorder test, not a physics assertion: alter only the borrowed event's
    // nonzero classification and corresponding aggregate count.
    let mut conditional = 0;
    let result = session
        .programs()
        .visit_power_bounded_owner_applied_successors(
            [true],
            &query.lower,
            &query.upper,
            query.max_numerator_rank,
            query.power_bounds.native(),
            Default::default(),
            &cancel,
            |event| match event {
                OwnerAppliedEvent::Successor(child) => {
                    conditional += 1;
                    recorder.event(OwnerAppliedEvent::Successor(OwnerAppliedSuccessor {
                        coefficient_nonzero: OwnerAppliedNonzero::Conditional,
                        ..child
                    }))
                }
                other => recorder.event(other),
            },
        )
        .map(|mut stats| {
            stats.conditional_successors = conditional;
            stats
        });
    assert!(recorder.end_query(result, 0.0).unwrap());
    assert!(recorder.finish(json!({}), true).unwrap());
    let events = records(&directory);
    let child = events.iter().find(|v| v["event"] == "successor").unwrap();
    assert_eq!(
        child["coefficient_nonzero"],
        "conditional-coefficient-not-zero"
    );
    assert_eq!(child["source_lower"], json!([1]));
    assert!(child["coefficient"].is_u64());
}

#[test]
fn terminal_and_zero_classifications_do_not_require_rule_finished() {
    let counts = Counts {
        events: 2,
        classified: 2,
        terminals: 1,
        zeros: 1,
        ..Default::default()
    };
    let stats = OwnerAppliedStats {
        events: 2,
        ..Default::default()
    };
    assert!(complete_counts(&counts, &PieceState::default(), &stats));
    let gap = Counts {
        other_classifications: 1,
        ..counts
    };
    assert!(!complete_counts(&gap, &PieceState::default(), &stats));
}
#[test]
fn delivered_count_mismatch_and_unfinished_selection_refuse() {
    let mut state = PieceState {
        selected: true,
        successors: 2,
        problems: 1,
    };
    assert!(state.finish(1, 1).is_err());
    assert!(state.selected);
    assert!(state.finish(2, 1).is_ok());
    assert!(state.finish(2, 1).is_err());
    let counts = Counts {
        selected: 1,
        ..Default::default()
    };
    assert!(!complete_counts(
        &counts,
        &PieceState::default(),
        &Default::default()
    ));
}
#[test]
fn problems_and_undelivered_optional_refusal_totals_disqualify_native_ok() {
    let counts = Counts::default();
    for stats in [
        OwnerAppliedStats {
            optional_original_refusals: 1,
            ..Default::default()
        },
        OwnerAppliedStats {
            optional_coalesced_refusals: 1,
            ..Default::default()
        },
        OwnerAppliedStats {
            optional_coefficient_refusals: 1,
            ..Default::default()
        },
    ] {
        assert!(!complete_counts(&counts, &PieceState::default(), &stats));
    }
    let counts = Counts {
        events: 1,
        problems: 1,
        ..Default::default()
    };
    assert!(!complete_counts(
        &counts,
        &PieceState::default(),
        &OwnerAppliedStats {
            events: 1,
            problems: 1,
            ..Default::default()
        }
    ));
}
#[test]
fn native_coefficient_table_limit_refuses_partial_observation() {
    let directory = Directory::new();
    let cancel = AtomicBool::new(false);
    let session = session(&directory, &cancel).unwrap();
    let mut caps = limits();
    caps.max_coefficients = 0;
    let mut recorder = Recorder::new(&directory.0, caps).unwrap();
    let query = query();
    recorder.begin_query(0, &query).unwrap();
    let result = session
        .programs()
        .visit_power_bounded_owner_applied_successors(
            [true],
            &query.lower,
            &query.upper,
            query.max_numerator_rank,
            query.power_bounds.native(),
            Default::default(),
            &cancel,
            |event| recorder.event(event),
        );
    assert!(result.is_err());
    assert!(!recorder.end_query(result, 0.0).unwrap());
    assert!(!recorder.finish(json!({}), false).unwrap());
}
#[test]
fn cancelled_preparation_has_no_usable_session() {
    let directory = Directory::new();
    assert!(session(&directory, &AtomicBool::new(true)).is_none());
}
#[test]
fn original_query_roles_and_caps_are_required_and_preserved() {
    let value = json!({"schema":"rustred.owner-domain-queries.json.v2","queries":[query()],
        "query_roles":{"required":["physical-two"],"auxiliary":[]}});
    let queries: Queries = serde_json::from_value(value.clone()).unwrap();
    assert_eq!(queries.validate(1).unwrap(), 1);
    let mut invalid = value.clone();
    invalid["query_roles"]["auxiliary"] = json!(["physical-two"]);
    assert!(
        serde_json::from_value::<Queries>(invalid)
            .unwrap()
            .validate(1)
            .is_err()
    );
    let mut invalid = value.clone();
    invalid["queries"][0]["upper"] = json!([0]);
    assert!(
        serde_json::from_value::<Queries>(invalid)
            .unwrap()
            .validate(1)
            .is_err()
    );
    let mut invalid = value;
    invalid["queries"][0]
        .as_object_mut()
        .unwrap()
        .remove("power_bounds");
    assert!(serde_json::from_value::<Queries>(invalid).is_err());
}
#[test]
fn prospective_record_cap_precedes_geometry_and_bounded_diagnostics() {
    let directory = Directory::new();
    let mut caps = limits();
    caps.max_record_bytes = 64;
    let mut recorder = Recorder::new(&directory.0, caps).unwrap();
    assert!(recorder.begin_query(0, &query()).is_err());
    assert_eq!(
        fs::metadata(directory.0.join("events.jsonl"))
            .unwrap()
            .len(),
        0
    );
    let text = bounded_debug(&"é".repeat(1000), 17);
    assert_eq!(text["truncated"], true);
    assert!(text["debug"].as_str().unwrap().len() <= 17);
}

#[test]
fn cumulative_json_cap_charges_binding_and_multiple_query_ledgers() {
    let directory = Directory::new();
    let mut caps = limits();
    caps.max_record_bytes = 16_384;
    caps.max_json_bytes = 65_536;
    caps.max_error_bytes = 32;
    write_new_json(
        &directory.0.join("binding.json"),
        &json!({"binding":"x".repeat(1000)}),
        caps.max_record_bytes,
    )
    .unwrap();
    let ceiling = caps.max_json_bytes;
    let mut recorder = Recorder::new(&directory.0, caps).unwrap();
    let mut count = 0;
    loop {
        let mut input = query();
        input.id = "x".repeat(1000);
        if recorder.begin_query(count, &input).is_err() {
            break;
        }
        assert!(recorder.end_query(Ok(Default::default()), 0.0).unwrap());
        count += 1;
        assert!(count < 64);
    }
    assert!(count > 1);
    assert!(!recorder.finish(json!({}), false).unwrap());
    let actual: usize = fs::read_dir(&directory.0)
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .filter(|path| {
            matches!(
                path.extension().and_then(|s| s.to_str()),
                Some("json" | "jsonl")
            )
        })
        .map(|path| fs::metadata(path).unwrap().len() as usize)
        .sum();
    assert!(actual <= ceiling);
    let result = read_json(&directory, "result.json");
    assert_eq!(
        result["json_bytes_before_result"].as_u64().unwrap() as usize
            + fs::metadata(directory.0.join("result.json")).unwrap().len() as usize,
        actual
    );
}
