//! Public partial-overlay transport and routed cold loading. This target links
//! the ordinary library; it does not rebuild the large library unit-test target.

use std::cell::RefCell;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};

use rustred::family::IntegralKey;
use rustred::persistence::{BinaryProgramKind, encode_program, inspect_program};
use rustred::sector::Mask;
use rustred::solver::{
    BoundOwnerOverlay, CandidateOwnerPrograms, CoordinateCase, FiniteCasePolicy, OwnerDomainScope,
    OwnerFeedbackPolicy, RoutedCandidateReducer,
};
use rustred_app::{
    CandidateOwnerBundle, CandidateOwnerLoadLimits, FamilyCandidatesRequest,
    OwnerDomainMatchRequest, OwnerDomainWalkCheckpointOptions, OwnerDomainWalkPublicationPolicy,
    OwnerDomainWalkRequest, OwnerDomainWalkSchedulingPolicy, RoutedCampaignRequest,
    encode_generated_domain_overlay, family_candidates, inspect_generated_candidate_bundle,
    load_generated_candidate_bundle, load_generated_candidate_owners,
    load_generated_domain_overlay, owner_domain_walk_with_progress, routed_campaign_with_progress,
    validate_domain_overlay_ingress,
};
use serde_json::json;

const FAMILY: &str = r#"
schema = "rustred.project.toml.v1"
[family]
name = "public_partial_rules"
loop_momenta = ["q"]
external_momenta = []
dimension = "d"
[[family.denominators]]
id = "P"
expression = "q^2-1"
[target]
powers = [1]
"#;

struct Fixture {
    owner: Vec<u8>,
    programs: Arc<CandidateOwnerPrograms<1>>,
}

impl Fixture {
    fn new() -> Self {
        let owner = family_candidates(FamilyCandidatesRequest::new(FAMILY))
            .unwrap()
            .bundle()
            .to_vec();
        let programs = Self::cold_programs(&owner);
        Self { owner, programs }
    }

    fn cold_programs(owner: &[u8]) -> Arc<CandidateOwnerPrograms<1>> {
        let mask = Mask::try_new([true]).unwrap();
        let (_, programs) = load_generated_candidate_owners::<1>(
            &[CandidateOwnerBundle {
                bytes: owner,
                owner_sector: &mask,
            }],
            Default::default(),
            Default::default(),
        )
        .unwrap();
        Arc::new(programs)
    }

    fn digest(&self) -> [u8; 32] {
        *blake3::hash(&self.owner).as_bytes()
    }

    fn overlay(&self, target: i16, depth: u32) -> BoundOwnerOverlay<1> {
        self.overlay_with_policy(
            target,
            OwnerFeedbackPolicy {
                numerical_depth: depth,
                ..Default::default()
            },
        )
    }

    fn overlay_with_policy(
        &self,
        target: i16,
        policy: OwnerFeedbackPolicy,
    ) -> BoundOwnerOverlay<1> {
        self.programs
            .bind_owner_search([true], policy)
            .unwrap()
            .solve_domains_with_observer(
                vec![CoordinateCase::new([Some(target)]).unwrap().into()],
                OwnerDomainScope {
                    max_numerator_rank: Some(0),
                    finite_case_policy: FiniteCasePolicy::SearchFinite,
                },
                Default::default(),
                |_| {},
            )
            .unwrap()
    }

    fn saved(&self) -> Vec<u8> {
        let overlay = self.overlay(2, 1);
        assert_eq!(overlay.terminal_count(), 0);
        encode_generated_domain_overlay(&self.programs, &overlay, self.digest(), Default::default())
            .unwrap()
    }
}

#[test]
fn public_domain_overlay_cold_replay_install_and_terminal_invariance() {
    let fixture = Fixture::new();
    let bytes = fixture.saved();
    let view = inspect_program(&bytes, Default::default()).unwrap();
    assert_eq!(view.kind(), BinaryProgramKind::DomainRules);
    let cold = Fixture::cold_programs(&fixture.owner);
    let (overlay, replay) =
        load_generated_domain_overlay(&cold, &bytes, [true], fixture.digest(), Default::default())
            .unwrap();
    assert!(!replay.rules.is_empty());
    assert_eq!(replay.rules.len(), overlay.rule_count());
    assert_eq!(overlay.terminal_count(), 0);
    assert_eq!(overlay.scope().max_numerator_rank, Some(0));
    assert_eq!(cold.context().scope().max_numerator_rank, None);
    assert!(cold.owns_domain_overlay(&overlay));
    assert!(!fixture.programs.owns_domain_overlay(&overlay));
    let count = cold.terminal_count();
    let next = cold
        .append_residual_free_domain_overlays(vec![overlay], Default::default())
        .unwrap();
    assert_eq!(next.terminal_count(), count);
    assert_eq!(next.overlays(&[true]).count(), 1);
    assert_eq!(cold.overlays(&[true]).count(), 0);
    let trace = RoutedCandidateReducer::try_new(next, [], Default::default())
        .unwrap()
        .trace_targets([IntegralKey::try_new([2]).unwrap()])
        .unwrap();
    assert!(trace.frontier().is_empty());
    assert!(
        trace
            .declared_terminals()
            .contains(&IntegralKey::try_new([1]).unwrap())
    );
}

#[test]
fn public_domain_overlay_rejects_wrong_binding_type_truncation_and_combined_limit() {
    let fixture = Fixture::new();
    let bytes = fixture.saved();
    for (owner, digest) in [([false], fixture.digest()), ([true], [0; 32])] {
        assert!(
            load_generated_domain_overlay(
                &fixture.programs,
                &bytes,
                owner,
                digest,
                Default::default()
            )
            .is_err()
        );
    }
    assert!(
        load_generated_domain_overlay(
            &fixture.programs,
            &bytes[..bytes.len() - 1],
            [true],
            fixture.digest(),
            Default::default()
        )
        .is_err()
    );
    let view = inspect_program(&bytes, Default::default()).unwrap();
    let wrong_kind = encode_program(
        BinaryProgramKind::Candidates,
        view.sections(),
        Default::default(),
    )
    .unwrap();
    assert!(
        load_generated_domain_overlay(
            &fixture.programs,
            &wrong_kind,
            [true],
            fixture.digest(),
            Default::default()
        )
        .is_err()
    );
    assert!(
        load_generated_candidate_bundle::<1>(&bytes, Default::default(), Default::default())
            .is_err()
    );
    let mask = Mask::try_new([true]).unwrap();
    let owners = [CandidateOwnerBundle {
        bytes: &fixture.owner,
        owner_sector: &mask,
    }];
    validate_domain_overlay_ingress(&owners, &[&bytes], Default::default()).unwrap();
    let limits = CandidateOwnerLoadLimits {
        max_total_input_bytes: fixture.owner.len() + bytes.len() - 1,
        ..Default::default()
    };
    assert_eq!(
        validate_domain_overlay_ingress(&owners, &[&bytes], limits)
            .unwrap_err()
            .kind(),
        rustred_app::AppErrorKind::Limit
    );
}

#[test]
fn public_domain_overlay_never_promotes_a_finite_residual_to_a_master() {
    let fixture = Fixture::new();
    let unresolved = fixture.overlay(1, 0);
    assert!(unresolved.terminal_count() > 0);
    assert!(
        encode_generated_domain_overlay(
            &fixture.programs,
            &unresolved,
            fixture.digest(),
            Default::default()
        )
        .is_err()
    );
    assert!(
        fixture
            .programs
            .append_residual_free_domain_overlays(vec![unresolved], Default::default())
            .is_err()
    );
    assert_eq!(fixture.programs.overlays(&[true]).count(), 0);
    let other = Fixture::cold_programs(&fixture.owner);
    assert!(
        encode_generated_domain_overlay(
            &other,
            &fixture.overlay(2, 1),
            fixture.digest(),
            Default::default()
        )
        .is_err()
    );
}

struct Directory(PathBuf);

impl Directory {
    fn new() -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../TMP/domain-overlay-api");
        std::fs::create_dir_all(&root).unwrap();
        let path = root.join(format!(
            "{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir(&path).unwrap();
        Self(path)
    }
}

impl Drop for Directory {
    fn drop(&mut self) {
        // Only this test's freshly created, uniquely named workspace directory.
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

#[test]
fn public_routed_selection_cold_loads_and_replays_partial_rules() {
    let fixture = Fixture::new();
    let patch = fixture.saved();
    let directory = Directory::new();
    std::fs::write(directory.0.join("owner.rrbin"), &fixture.owner).unwrap();
    std::fs::write(directory.0.join("partial.rrbin"), &patch).unwrap();
    let fingerprint = inspect_generated_candidate_bundle(&fixture.owner, Default::default())
        .unwrap()
        .family_fingerprint;
    let selection = json!({
        "family_fingerprint": fingerprint,
        "owners": [{"path":"owner.rrbin", "bytes":fixture.owner.len(), "mask":"1"}],
        "initial_frontier_routes": [],
        "domain_rule_overlays": [{"path":"partial.rrbin", "bytes":patch.len(), "owner_mask":"1"}]
    });
    let mut request = RoutedCampaignRequest::new(selection.to_string(), "2\n".into());
    request.owner_base = directory.0.clone();
    let events = RefCell::new(Vec::new());
    let report = routed_campaign_with_progress(request, &AtomicBool::new(false), |event| {
        events.borrow_mut().push(event)
    })
    .unwrap();
    assert!(report.completed_finite_trace, "{}", report.document);
    let events = events.into_inner();
    let installed = events
        .iter()
        .find(|event| event["phase"] == "domain_rule_overlay_installed")
        .expect("cold overlay install event");
    assert_eq!(installed["rules"], installed["replayed_rules"]);
    assert!(installed["rules"].as_u64().unwrap() > 0);
    assert_eq!(installed["new_terminals"], 0);
}

#[test]
fn public_epoch_checkpoint_binds_same_length_valid_partial_payloads() {
    let fixture = Fixture::new();
    let patch = fixture.saved();
    let mut changed_policy = OwnerFeedbackPolicy {
        numerical_depth: 1,
        ..Default::default()
    };
    changed_policy.symbolic.sample_seed = 1;
    let alternative = fixture.overlay_with_policy(2, changed_policy);
    let changed_patch = encode_generated_domain_overlay(
        &fixture.programs,
        &alternative,
        fixture.digest(),
        Default::default(),
    )
    .unwrap();
    // Both payloads are genuine, independently replayable rule artifacts.
    // A same-width prospective policy change must not evade exact-byte binding.
    assert_eq!(patch.len(), changed_patch.len());
    assert_ne!(patch, changed_patch);
    for bytes in [&patch, &changed_patch] {
        let cold = Fixture::cold_programs(&fixture.owner);
        let (overlay, replay) = load_generated_domain_overlay(
            &cold,
            bytes,
            [true],
            fixture.digest(),
            Default::default(),
        )
        .unwrap();
        assert_eq!(overlay.terminal_count(), 0);
        assert!(!replay.rules.is_empty());
    }

    let directory = Directory::new();
    std::fs::write(directory.0.join("owner.rrbin"), &fixture.owner).unwrap();
    let patch_path = directory.0.join("partial.rrbin");
    std::fs::write(&patch_path, &patch).unwrap();
    let fingerprint = inspect_generated_candidate_bundle(&fixture.owner, Default::default())
        .unwrap()
        .family_fingerprint;
    let selection = json!({
        "family_fingerprint": fingerprint,
        "owners": [{"path":"owner.rrbin", "bytes":fixture.owner.len(), "mask":"1"}],
        "initial_frontier_routes": [],
        "domain_rule_overlays": [{"path":"partial.rrbin", "bytes":patch.len(), "owner_mask":"1"}]
    });
    let queries = json!({"schema":"rustred.owner-domain-queries.json.v2", "queries":[
        {"id":"point", "owner":"1", "lower":[1], "upper":[1], "max_numerator_rank":0}
    ]});
    let mut matching = OwnerDomainMatchRequest::new(selection.to_string(), queries.to_string());
    matching.owner_base = directory.0.clone();
    let mut request = OwnerDomainWalkRequest::new(matching);
    request.publication_policy = OwnerDomainWalkPublicationPolicy::Epoch;
    request.scheduling_policy = OwnerDomainWalkSchedulingPolicy::TransferUnreserved {
        lookahead: std::num::NonZeroUsize::new(8).unwrap(),
    };
    request.workers = 1;
    let checkpoint = directory.0.join("checkpoint");
    request.checkpoint = Some(OwnerDomainWalkCheckpointOptions::new(&checkpoint));
    let initial =
        owner_domain_walk_with_progress(request.clone(), &AtomicBool::new(false), |_| {}).unwrap();
    assert_eq!(initial.document["recursive_worklist_exhausted"], true);
    assert!(checkpoint.join("latest.json").is_file());

    request.checkpoint.as_mut().unwrap().resume = true;
    let resumed =
        owner_domain_walk_with_progress(request.clone(), &AtomicBool::new(false), |_| {}).unwrap();
    assert_eq!(resumed.document["recursive_worklist_exhausted"], true);
    let latest_before = std::fs::read(checkpoint.join("latest.json")).unwrap();

    // Selection text, declared length, base owner and valid equations stay the
    // same. Only exact sidecar bytes change; CP6 must reject before walking.
    std::fs::write(&patch_path, &changed_patch).unwrap();
    let error =
        owner_domain_walk_with_progress(request, &AtomicBool::new(false), |_| {}).unwrap_err();
    assert!(
        error
            .to_string()
            .contains("epoch checkpoint/request binding or requested domain limit differs"),
        "{error}"
    );
    assert_eq!(
        std::fs::read(checkpoint.join("latest.json")).unwrap(),
        latest_before
    );
}
