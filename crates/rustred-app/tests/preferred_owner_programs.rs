//! Public-API integration checks; no CLI build or private application API needed.
use rustred::sector::Mask;
use rustred::solver::RoutedCandidateReducer;
use rustred_app::{
    CandidateOwnerBundle, CandidateOwnerLoadLimits, FamilyCandidatesRequest,
    OwnerDomainMatchRequest, OwnerDomainWalkCheckpointOptions, OwnerDomainWalkPublicationPolicy,
    OwnerDomainWalkRequest, OwnerDomainWalkSchedulingPolicy, OwnerDomainWalkVerifyOptions,
    family_candidates, inspect_generated_candidate_bundle, load_generated_candidate_owners,
    load_generated_candidate_owners_with_preferences, owner_domain_walk_verify_closure,
    owner_domain_walk_with_progress,
};
use serde_json::{Value, json};
use std::{
    path::PathBuf,
    sync::{
        Arc,
        atomic::{AtomicBool, AtomicU64, Ordering},
    },
};

const FAMILY: &str = r#"
# preferred-byte-a
schema="rustred.project.toml.v1"
[family]
name="preferred_owner_public_api"
loop_momenta=["q"]
external_momenta=[]
dimension="d"
[[family.denominators]]
id="P"
expression="q^2-1"
[target]
powers=[1]
"#;

struct Fixture {
    directory: PathBuf,
    bytes: Vec<u8>,
    selection: Value,
}
impl Fixture {
    fn new() -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let directory = std::env::temp_dir().join(format!(
            "rustred-preferred-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir(&directory).unwrap();
        let mut request = FamilyCandidatesRequest::new(FAMILY);
        request.numerical_depth = 0;
        request.max_numerator_rank = Some(2);
        let generated = family_candidates(request).unwrap();
        let bytes = generated.bundle().to_vec();
        let inspection = inspect_generated_candidate_bundle(&bytes, Default::default()).unwrap();
        std::fs::write(directory.join("baseline.rrbin"), &bytes).unwrap();
        std::fs::write(directory.join("preferred.rrbin"), &bytes).unwrap();
        let selection = json!({"family_fingerprint":inspection.family_fingerprint,
            "owners":[{"path":"baseline.rrbin","bytes":bytes.len(),"mask":"1"}],
            "initial_frontier_routes":[]});
        Self {
            directory,
            bytes,
            selection,
        }
    }
    fn preferred(&self) -> Value {
        let mut selection = self.selection.clone();
        selection["preferred_owner_programs"] = json!([{"path":"preferred.rrbin",
            "bytes":self.bytes.len(),"owner_mask":"1","residual_policy":"defer-to-baseline"}]);
        selection
    }
    fn request(&self, selection: Value) -> OwnerDomainWalkRequest {
        let mut matching = OwnerDomainMatchRequest::new(
            selection.to_string(),
            json!({
            "schema":"rustred.owner-domain-queries.json.v2","queries":[{
                "id":"whole-ray","owner":"1","lower":[0],"upper":[null],"max_numerator_rank":2
            }]})
            .to_string(),
        );
        matching.owner_base = self.directory.clone();
        OwnerDomainWalkRequest::new(matching)
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.directory);
    }
}

#[test]
fn public_loader_a_a_and_empty_preferences_preserve_trace_and_terminals() {
    let fixture = Fixture::new();
    let mask = Mask::try_new([true]).unwrap();
    let input = [CandidateOwnerBundle {
        bytes: &fixture.bytes,
        owner_sector: &mask,
    }];
    let (_, baseline) =
        load_generated_candidate_owners::<1>(&input, Default::default(), Default::default())
            .unwrap();
    let count = baseline.terminal_count();
    let targets = [[1], [2], [4]].map(|p| rustred::family::IntegralKey::try_new(p).unwrap());
    let trace = RoutedCandidateReducer::try_new(Arc::new(baseline), [], Default::default())
        .unwrap()
        .trace_targets(targets.clone())
        .unwrap();
    for preferences in [&[][..], &input[..]] {
        let (_, programs) = load_generated_candidate_owners_with_preferences::<1>(
            &input,
            preferences,
            Default::default(),
            Default::default(),
        )
        .unwrap();
        assert_eq!(programs.terminal_count(), count);
        let result = RoutedCandidateReducer::try_new(Arc::new(programs), [], Default::default())
            .unwrap()
            .trace_targets(targets.clone())
            .unwrap();
        assert_eq!(trace, result);
    }
}

#[test]
fn public_loader_admits_combined_bytes_and_rejects_duplicate_preferences() {
    let fixture = Fixture::new();
    let mask = Mask::try_new([true]).unwrap();
    let input = [CandidateOwnerBundle {
        bytes: &fixture.bytes,
        owner_sector: &mask,
    }];
    let limits = CandidateOwnerLoadLimits {
        max_total_input_bytes: fixture.bytes.len(),
        ..Default::default()
    };
    assert!(
        load_generated_candidate_owners_with_preferences::<1>(
            &input,
            &[],
            limits,
            Default::default()
        )
        .is_ok()
    );
    assert!(
        load_generated_candidate_owners_with_preferences::<1>(
            &input,
            &input,
            limits,
            Default::default()
        )
        .unwrap_err()
        .message()
        .contains("aggregate input-byte")
    );
    assert!(
        load_generated_candidate_owners_with_preferences::<1>(
            &input,
            &[input[0], input[0]],
            Default::default(),
            Default::default()
        )
        .unwrap_err()
        .message()
        .contains("duplicated")
    );
}

#[test]
fn public_campaign_rejects_ambiguous_policy_collision_and_missing_owner_before_io() {
    let fixture = Fixture::new();
    let mut variants = vec![];
    let mut input = fixture.preferred();
    input["preferred_owner_programs"][0]
        .as_object_mut()
        .unwrap()
        .remove("residual_policy");
    variants.push(input);
    let mut input = fixture.preferred();
    input["preferred_owner_programs"][0]["residual_policy"] = "promote".into();
    variants.push(input);
    let mut input = fixture.preferred();
    input["preferred_owner_programs"][0]["owner_mask"] = "0".into();
    variants.push(input);
    let mut input = fixture.preferred();
    let second = input["preferred_owner_programs"][0].clone();
    input["preferred_owner_programs"]
        .as_array_mut()
        .unwrap()
        .push(second);
    variants.push(input);
    let mut input = fixture.preferred();
    input["domain_rule_overlays"] =
        json!([{"path":"does-not-exist.rrbin","bytes":1,"owner_mask":"1"}]);
    variants.push(input);
    for selection in variants {
        let error = owner_domain_walk_with_progress(
            fixture.request(selection),
            &AtomicBool::new(false),
            |_| {},
        )
        .unwrap_err();
        assert!(!error.message().contains("No such file"), "{error}");
    }
}

#[test]
fn preferred_cp5_and_cp6_cold_load_bind_both_payloads_and_selection_policy() {
    let fixture = Fixture::new();
    for policy in [
        OwnerDomainWalkPublicationPolicy::Ordered,
        OwnerDomainWalkPublicationPolicy::Epoch,
    ] {
        let directory = fixture.directory.join(format!("checkpoint-{policy:?}"));
        let mut request = fixture.request(fixture.preferred());
        request.publication_policy = policy;
        if policy == OwnerDomainWalkPublicationPolicy::Epoch {
            request.scheduling_policy = OwnerDomainWalkSchedulingPolicy::TransferUnreserved {
                lookahead: std::num::NonZeroUsize::new(8).unwrap(),
            };
        }
        request.checkpoint = Some(OwnerDomainWalkCheckpointOptions::new(&directory));
        let result =
            owner_domain_walk_with_progress(request.clone(), &AtomicBool::new(false), |_| {})
                .unwrap();
        assert_eq!(result.document["recursive_worklist_exhausted"], true);
        let mut options = OwnerDomainWalkVerifyOptions::new(&directory);
        options.require_closure = true;
        let cold =
            owner_domain_walk_verify_closure(&request, &options, &AtomicBool::new(false), |_| {})
                .unwrap();
        assert_eq!(cold["verdict"], "PASS", "{cold}");
        assert_eq!(cold["checkpoint"]["owner_digests_match"], true, "{cold}");
        let mut changed = request.clone();
        changed.matching.selection_json = fixture.selection.to_string();
        let mismatch =
            owner_domain_walk_verify_closure(&changed, &options, &AtomicBool::new(false), |_| {})
                .unwrap();
        assert_eq!(mismatch["verdict"], "FAIL", "{mismatch}");
        assert_eq!(mismatch["checkpoint"]["request_binding_matches"], false);
        // Change only an input-source COMMENT, leaving a native-valid program
        // of identical size and mathematics. This tests the immutable payload
        // binding itself, not merely a malformed decoder rejection.
        let marker = b"# preferred-byte-a";
        let positions = fixture
            .bytes
            .windows(marker.len())
            .enumerate()
            .filter_map(|(i, bytes)| (bytes == marker).then_some(i))
            .collect::<Vec<_>>();
        assert_eq!(positions.len(), 1);
        let mut changed_payload = fixture.bytes.clone();
        changed_payload[positions[0] + marker.len() - 1] = b'b';
        let owner = Mask::try_new([true]).unwrap();
        let base = [CandidateOwnerBundle {
            bytes: &fixture.bytes,
            owner_sector: &owner,
        }];
        let preferred = [CandidateOwnerBundle {
            bytes: &changed_payload,
            owner_sector: &owner,
        }];
        assert!(
            load_generated_candidate_owners_with_preferences::<1>(
                &base,
                &preferred,
                Default::default(),
                Default::default()
            )
            .is_ok()
        );
        std::fs::write(fixture.directory.join("preferred.rrbin"), &changed_payload).unwrap();
        let mismatch =
            owner_domain_walk_verify_closure(&request, &options, &AtomicBool::new(false), |_| {})
                .unwrap();
        assert_eq!(mismatch["verdict"], "FAIL", "{mismatch}");
        assert_eq!(mismatch["checkpoint"]["owner_digests_match"], false);
        let mut bad_resume = request.clone();
        bad_resume.checkpoint.as_mut().unwrap().resume = true;
        let error = owner_domain_walk_with_progress(bad_resume, &AtomicBool::new(false), |_| {})
            .unwrap_err();
        let expected = match policy {
            OwnerDomainWalkPublicationPolicy::Ordered => "immutable owner payload bytes differ",
            OwnerDomainWalkPublicationPolicy::Epoch => {
                "epoch checkpoint/request binding or requested domain limit differs"
            }
            _ => unreachable!("fixture has only CP5 and CP6 policies"),
        };
        eprintln!("{policy:?} changed preferred bytes refused: {error}");
        assert!(error.message().contains(expected), "{error}");
        std::fs::write(fixture.directory.join("preferred.rrbin"), &fixture.bytes).unwrap();
        request.checkpoint.as_mut().unwrap().resume = true;
        assert!(owner_domain_walk_with_progress(request, &AtomicBool::new(false), |_| {}).is_ok());
    }
}
