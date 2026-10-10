//! Real CP6 predecessor -> required-scope extension, compared with a cold census.
use super::super::e2e_tests::{Run, scratch, walk, walk_argv, write_owners};
use super::super::{OwnerDomainWalkVerifyMutation, checkpoint};
use super::*;
use std::sync::atomic::Ordering;

pub(super) fn fixture() -> Run {
    fixture_with(
        json!({"schema":"rustred.owner-domain-queries.json.v2","queries":[
        query("initial", [0,0,0], 0, 3, 3)],
        "query_roles":{"required":["initial"],"auxiliary":[]}}),
        true,
        false,
    )
}

fn fixture_with(queries: Value, g2: bool, ready: bool) -> Run {
    let dir = scratch("inventory-cache-cp6-extension");
    let selection = write_owners(&dir.0, 2, &[], &json!([]));
    std::fs::write(dir.0.join("selection.json"), selection).unwrap();
    std::fs::write(dir.0.join("queries.json"), queries.to_string()).unwrap();
    let mut argv = walk_argv(
        &dir.0,
        1,
        if g2 {
            &["--g2-residual-anchors", "union"]
        } else {
            &[]
        },
    );
    let at = argv
        .iter()
        .position(|s| s == "--publication-policy")
        .unwrap();
    argv[at + 1] = "epoch".into();
    let mut request = crate::cli::walk_request_from_argv(argv).unwrap();
    if ready {
        request.epoch_rolling = true;
        request.epoch_publication_order = crate::OwnerDomainWalkEpochPublicationOrder::OldestReady;
        request.epoch_window = Some(16);
    }
    let result = walk(&dir.0, &request);
    Run {
        dir,
        request,
        result,
    }
}

fn query(id: &str, upper: [u64; 3], rank: u32, a: u64, d: i64) -> Value {
    json!({"id":id,"owner":"111","lower":[0,0,0],"upper":upper,"max_numerator_rank":rank,
        "power_bounds":{"max_positive_power":a,"min_power_difference":d,"max_power_difference":null}})
}

fn census(run: &Run, source: Option<OwnerDomainWalkCensusSource>) -> OwnerDomainWalkInventory {
    let mut options = OwnerDomainWalkInventoryOptions::new(run.dir.0.join("checkpoint"));
    options.verification.threads = 2;
    options.previous_census = source;
    owner_domain_walk_inventory(&run.request, &options, &AtomicBool::new(false), |_| {}).unwrap()
}

fn save(
    run: &Run,
    inventory: &OwnerDomainWalkInventory,
    name: &str,
) -> OwnerDomainWalkCensusSource {
    let path = run.dir.0.join(name);
    let receipt = inventory
        .write_census(&path)
        .unwrap()
        .expect("safe complete CP6 cache");
    OwnerDomainWalkCensusSource {
        path,
        expected_blake3: receipt["blake3"].as_str().unwrap().into(),
        bytes: receipt["bytes"].as_u64().unwrap(),
    }
}

fn same(actual: &OwnerDomainWalkInventory, expected: &OwnerDomainWalkInventory) {
    assert_eq!(actual.summary()["complete"], true, "{}", actual.summary());
    assert_eq!(
        actual.summary()["encountered"],
        expected.summary()["encountered"]
    );
    assert_eq!(actual.terminal_keys(), expected.terminal_keys());
    assert_eq!(
        actual.rules(0, 1000).unwrap().items,
        expected.rules(0, 1000).unwrap().items
    );
    assert_eq!(actual.summary()["authority"], "trusted_saved_scope");
    assert_eq!(actual.summary()["independently_verified"], false);
}

#[test]
fn cp6_cache_reuses_same_generation_and_only_classifies_extension_suffix() {
    let mut run = fixture();
    let old = census(&run, None);
    let source = save(&run, &old, "old-census.bin");
    let warm = census(&run, Some(source.clone()));
    same(&warm, &old);
    let stats = &warm.summary()["verification"]["inventory_census"];
    assert!(stats["reused_records"].as_u64().unwrap() > 0);
    assert_eq!(stats["completed"], 0);
    assert!(stats["unique_apply_scopes"].is_null());

    run.request.checkpoint.as_mut().unwrap().resume = true;
    let amendment = json!({"schema":super::super::super::rescue::SCOPE_EXTENSION_SCHEMA,
        "sequence":1,"parent":checkpoint::epoch_request_binding(&run.request),
        "queries":[query("extension", [3,3,3], 2, 5, 2)],
        "query_roles":{"required":["extension"],"auxiliary":[]}});
    run.request
        .amendments
        .push(crate::OwnerDomainWalkAmendment {
            path: run.dir.0.join("extension.json"),
            text: amendment.to_string(),
        });
    run.result = walk(&run.dir.0, &run.request);
    let cold = census(&run, None);
    let warm = census(&run, Some(source.clone()));
    same(&warm, &cold);
    let summary = warm.summary();
    let stats = &summary["verification"]["inventory_census"];
    assert_eq!(stats["cache_status"], "reused_authenticated_record_prefix");
    assert!(stats["completed"].as_u64().unwrap() > 0);
    assert!(stats["reused_apply_records"].as_u64().unwrap() > 0);
    for field in [
        "apply_records",
        "route_records_skipped",
        "full_cover_records_skipped",
    ] {
        assert_eq!(
            stats[field],
            cold.summary()["verification"]["inventory_census"][field]
        );
    }
    let chained = save(&run, &warm, "extended-census.bin");
    same(&census(&run, Some(chained)), &cold);

    let mut bad = source;
    bad.expected_blake3 = blake3::hash(b"corrupted receipt").to_hex().to_string();
    let fallback = census(&run, Some(bad));
    same(&fallback, &cold);
    assert_eq!(
        fallback.summary()["verification"]["inventory_census"]["reused_records"],
        0
    );
}

#[test]
fn cache_never_bypasses_cancellation_mutation_or_fresh_request_checks() {
    let run = fixture();
    let old = census(&run, None);
    let source = save(&run, &old, "old-census.bin");
    let mut options = OwnerDomainWalkInventoryOptions::new(run.dir.0.join("checkpoint"));
    options.previous_census = Some(source);
    let cancellation = AtomicBool::new(false);
    let stopped = owner_domain_walk_inventory(&run.request, &options, &cancellation, |event| {
        if event["event"] == "inventory_census_cache" {
            cancellation.store(true, Ordering::Relaxed);
        }
    })
    .unwrap();
    assert_eq!(stopped.summary()["complete"], false);
    assert!(
        stopped
            .write_census(run.dir.0.join("cancelled.bin"))
            .unwrap()
            .is_none()
    );
    assert!(!run.dir.0.join("cancelled.bin").exists());

    options.verification.mutation = Some(OwnerDomainWalkVerifyMutation::G2ShrunkResidual);
    assert!(
        owner_domain_walk_inventory(&run.request, &options, &AtomicBool::new(false), |_| {})
            .is_err()
    );
    options.verification.mutation = None;
    let mut foreign = run.request.clone();
    foreign.matching.queries_json = foreign.matching.queries_json.replace("initial", "foreign");
    let invalid =
        owner_domain_walk_inventory(&foreign, &options, &AtomicBool::new(false), |_| {}).unwrap();
    assert_eq!(invalid.summary()["complete"], false);
    assert!(
        invalid
            .write_census(run.dir.0.join("foreign.bin"))
            .unwrap()
            .is_none()
    );

    options.deep_verification = true;
    let deep = owner_domain_walk_inventory(&run.request, &options, &AtomicBool::new(false), |_| {})
        .unwrap();
    assert_eq!(deep.summary()["verification"]["verdict"], "PASS");
    assert!(
        deep.write_census(run.dir.0.join("deep.bin"))
            .unwrap()
            .is_none()
    );
}

#[test]
fn cached_partial_and_g2_residual_prefixes_preserve_exact_census() {
    let mut lower011 = query("anchor011", [0, 5, 0], 0, 7, 5);
    lower011["owner"] = "011".into();
    let mut lower110 = query("anchor110", [5, 0, 0], 0, 7, 5);
    lower110["owner"] = "110".into();
    let queries = json!({"schema":"rustred.owner-domain-queries.json.v2","queries":[
        lower011, lower110, query("anchor", [3,3,3], 2, 5, 2),
        query("big", [5,5,5], 2, 8, 0), query("big-inner", [4,4,4], 2, 6, 1)]});
    for g2 in [false, true] {
        let run = fixture_with(queries.clone(), g2, g2);
        let options = OwnerDomainWalkVerifyOptions::new(run.dir.0.join("checkpoint"));
        let mut violations = super::super::Violations::new(100);
        let loaded = super::super::load::<3>(&options, false, &mut violations).unwrap();
        assert!(violations.is_empty());
        if g2 {
            assert!(
                loaded.g2.values().any(|row| row.residual.is_some()),
                "fixture needs a true G2 residual"
            );
            assert!(
                loaded.g2.values().any(|row| row.residual.is_none()),
                "fixture needs a G2 full cover"
            );
        } else {
            assert!(
                loaded
                    .nodes
                    .iter()
                    .any(|node| node.kind == super::super::Kind::Partial),
                "fixture needs a real initial partial"
            );
        }
        let cold = census(&run, None);
        let source = save(&run, &cold, "bands-census.bin");
        let warm = census(&run, Some(source));
        same(&warm, &cold);
        assert_eq!(
            warm.summary()["verification"]["inventory_census"]["completed"],
            0
        );
        assert_eq!(
            warm.summary()["verification"]["inventory_census"]["reused_records"],
            loaded.domains.len()
        );
    }
}

#[test]
fn publication_resume_reuses_durable_census_before_native_preparation() {
    let run = fixture();
    let checkpoint = run.dir.0.join("checkpoint");
    let checkpoint_before = std::fs::read(checkpoint.join("latest.json")).unwrap();
    let directory = run.dir.0.join("paused-publication");
    let mut options = crate::MasterReductionOptions::new(&checkpoint, &directory);
    options.operation = crate::MasterReductionOperation::Publish;
    options.deep_verification = Some(false);
    options.threads = 2;
    let cancellation = AtomicBool::new(false);
    let intercepted = AtomicBool::new(false);

    let paused =
        crate::master_reduce_saved_campaign(&run.request, &options, &cancellation, |event| {
            if event["stage"] == "normalize" && !intercepted.swap(true, Ordering::Relaxed) {
                // Observe the actual durable checkpoint, not the in-memory
                // event. This fails if its write moves after preparation.
                let saved: Value =
                    serde_json::from_slice(&std::fs::read(directory.join("latest.json")).unwrap())
                        .unwrap();
                assert_eq!(saved["status"], "running");
                assert_eq!(saved["inventory"]["complete"], true);
                assert_eq!(saved["independently_verified"], false);
                assert!(saved["native_state"].is_null());
                assert!(!directory.join("artifact.json").exists());
                let receipt = &saved["inventory_census_cache"];
                let bytes =
                    std::fs::read(directory.join(receipt["file"].as_str().unwrap())).unwrap();
                assert_eq!(receipt["bytes"], bytes.len());
                assert_eq!(receipt["blake3"], blake3::hash(&bytes).to_hex().to_string());

                // A deterministic local packaging failure exercises the
                // real pause/error handler before any native state is made.
                cancellation.store(true, Ordering::Relaxed);
                std::fs::write(directory.join("inputs"), b"test packaging obstruction").unwrap();
            }
        })
        .unwrap();
    assert!(intercepted.load(Ordering::Relaxed));
    assert_eq!(paused["status"], "paused");
    assert!(paused["last_error"].is_string());
    assert!(paused["native_state"].is_null());
    assert!(paused["inventory_census_cache"].is_object());
    assert!(!directory.join("artifact.json").exists());
    assert_eq!(crate::master_reduction_inspect(&directory).unwrap(), paused);

    std::fs::remove_file(directory.join("inputs")).unwrap();
    cancellation.store(false, Ordering::Relaxed);
    options.resume = true;
    let resumed =
        crate::master_reduce_saved_campaign(&run.request, &options, &cancellation, |_| {}).unwrap();
    let census = &resumed["inventory"]["verification"]["inventory_census"];
    assert_eq!(census["cache_status"], "reused_authenticated_record_prefix");
    assert!(census["reused_records"].as_u64().unwrap() > 0);
    assert_eq!(census["completed"], 0);
    assert!(resumed["native_state"].is_object());
    assert_eq!(resumed["publication_assurance"], "trusted_saved_scope");
    assert_eq!(resumed["independently_verified"], false);

    let cold_directory = run.dir.0.join("cold-publication");
    options.directory = cold_directory.clone();
    options.resume = false;
    let cold = crate::master_reduce_saved_campaign(
        &run.request,
        &options,
        &AtomicBool::new(false),
        |_| {},
    )
    .unwrap();
    assert_eq!(resumed["status"], cold["status"]);
    assert!(matches!(
        resumed["status"].as_str(),
        Some("published_unrefined" | "completed_nonminimal")
    ));
    assert_eq!(
        resumed["inventory"]["encountered"],
        cold["inventory"]["encountered"]
    );
    for field in ["native_state", "collection_state"] {
        assert_eq!(resumed[field]["blake3"], cold[field]["blake3"]);
        assert_eq!(
            std::fs::read(directory.join(resumed[field]["file"].as_str().unwrap())).unwrap(),
            std::fs::read(cold_directory.join(cold[field]["file"].as_str().unwrap())).unwrap()
        );
    }
    assert_eq!(
        std::fs::read(checkpoint.join("latest.json")).unwrap(),
        checkpoint_before
    );
}
