use super::*;
use std::{
    fs,
    sync::atomic::{AtomicU64, Ordering},
};
struct Scratch(PathBuf);
impl Scratch {
    fn new() -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let root = std::env::temp_dir().join(format!(
            "rustred-finite-cli-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&root).unwrap();
        for name in ["inputs", "bin", "untouched"] {
            fs::create_dir(root.join(name)).unwrap();
        }
        fs::write(
            root.join("inputs/owner.rrbin"),
            b"never decoded by these source/path tests",
        )
        .unwrap();
        fs::write(
            root.join("inputs/selection.json"),
            json!({"family_fingerprint":"source-parser-only",
            "owners":[{"path":"owner.rrbin","bytes":38,"mask":"1"}],"initial_frontier_routes":[]})
            .to_string(),
        )
        .unwrap();
        fs::write(root.join("inputs/queries.json"),json!({"schema":"rustred.owner-domain-queries.json.v2",
            "queries":[{"id":"q","owner":"1","lower":[0],"upper":[0],"max_numerator_rank":0,
                "power_bounds":{"max_positive_power":1,"min_power_difference":0,"max_power_difference":2}}],
            "query_roles":{"required":["q"],"auxiliary":[]}}).to_string()).unwrap();
        for file in ["old-output", "old-events", "old-stop"] {
            fs::write(root.join("untouched").join(file), file.as_bytes()).unwrap();
        }
        let command = json!([
            "rustred",
            "owner-domain-match",
            "--manifest",
            "../inputs/selection.json",
            "--queries",
            "../inputs/queries.json",
            "--owner-base",
            "../inputs",
            "--follow-successors",
            "--output",
            "../untouched/old-output",
            "--events",
            "../untouched/old-events",
            "--stop-file",
            "../untouched/old-stop",
            "--resume",
            "../untouched/absent-checkpoint"
        ]);
        fs::write(root.join("bin/source.json"), command.to_string()).unwrap();
        fs::write(root.join("bin/policy.json"), policy().to_string()).unwrap();
        Self(root)
    }
}
impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}
fn zero_object(fields: &str) -> Value {
    fields
        .split_whitespace()
        .map(|name| (name.to_string(), json!(0)))
        .collect::<serde_json::Map<_, _>>()
        .into()
}
fn policy() -> Value {
    let mut storage = zero_object(
        "max_arity max_delta_records max_delta_payload_bytes max_page_records max_page_bytes max_cached_pages max_cached_page_bytes max_directory_bytes max_manifest_bytes max_state_bytes max_scope_rows max_census_records max_runs max_pages max_physical_files max_disk_bytes max_lookup_pages max_lookup_bytes max_transient_bytes",
    );
    storage["raw"] = zero_object(
        "max_page_records max_page_bytes max_runs max_pages max_directory_bytes max_stored_references max_merge_runs max_merge_cached_pages max_merge_payload_bytes max_merge_read_bytes max_decode_transient_bytes",
    );
    json!({"workers":1,"windows":{"max_seed_points":1,"max_seed_bytes":65536,"max_positive_layers":64,"max_partition_checks":1000},
        "census":{"max_unique_selections":100,"max_identity_payload_bytes":65536},
        "export":{"max_records":100,"max_live_payload_bytes":65536},"storage":storage,
        "max_total_rows":16,"max_source_bytes":1048576,"control":{"max_bytes":1048576,"max_files":100}})
}
fn args(s: &Scratch, directory: PathBuf) -> FiniteArgs {
    FiniteArgs {
        action: Action::Run,
        directory,
        command: Some(s.0.join("bin/source.json")),
        policy: Some(s.0.join("bin/policy.json")),
        output: Some(s.0.join("result.json")),
        events: Some(s.0.join("events.jsonl")),
        stop_file: None,
        resume: false,
        repair_preparation: false,
        no_progress: true,
        format: "auto".into(),
    }
}

#[test]
fn finite_source_carrier_is_relative_input_only_and_never_opens_legacy_checkpoint() {
    let mut s = Scratch::new();
    // A non-directory source checkpoint must remain completely inert.
    fs::write(s.0.join("untouched/absent-checkpoint"), b"not a checkpoint").unwrap();
    let before = fs::read(s.0.join("inputs/queries.json")).unwrap();
    let (source, inputs, inert) = read_source(&s.0.join("bin/source.json")).unwrap();
    assert_eq!(source.matching.queries_json.as_bytes(), before);
    assert_eq!(source.matching.owner_base, s.0.join("bin/../inputs"));
    assert_eq!(inputs.len(), 4);
    assert_eq!(inert.len(), 4);
    let new = s.0.with_extension("relocated");
    fs::rename(&s.0, &new).unwrap();
    s.0 = new;
    let (relocated, _, _) = read_source(&s.0.join("bin/source.json")).unwrap();
    assert_eq!(source.checkpoint_binding(), relocated.checkpoint_binding());
    for name in ["old-output", "old-events", "old-stop"] {
        assert_eq!(
            fs::read(s.0.join("untouched").join(name)).unwrap(),
            name.as_bytes()
        );
    }
    assert_eq!(
        fs::read(s.0.join("untouched/absent-checkpoint")).unwrap(),
        b"not a checkpoint"
    );
}

#[test]
fn finite_source_transport_uses_policy_without_rebinding_original_query_allowances() {
    let s = Scratch::new();
    for path in [
        s.0.join("bin/source.json"),
        s.0.join("inputs/selection.json"),
    ] {
        let mut bytes = fs::read(&path).unwrap();
        bytes.resize(crate::MAX_INPUT_BYTES + 1, b' ');
        fs::write(path, bytes).unwrap();
    }
    let path = s.0.join("bin/source.json");
    let mut document = policy();
    let small = FiniteDemandPolicy::from_json(&serde_json::to_vec(&document).unwrap()).unwrap();
    assert!(read_source_with_policy(&path, &small).is_err());
    document["max_source_bytes"] = json!(crate::MAX_INPUT_BYTES + 1);
    let large = FiniteDemandPolicy::from_json(&serde_json::to_vec(&document).unwrap()).unwrap();
    let (source, _, _) = read_source_with_policy(&path, &large).unwrap();
    document["max_source_bytes"] = Value::Null;
    let uncapped = FiniteDemandPolicy::from_json(&serde_json::to_vec(&document).unwrap()).unwrap();
    let (same, _, _) = read_source_with_policy(&path, &uncapped).unwrap();
    assert_eq!(source.checkpoint_binding(), same.checkpoint_binding());
    assert_eq!(source.matching.max_query_bytes, 1 << 20);
    assert_eq!(source.matching.max_queries, 256);
}

#[test]
fn finite_namespace_cannot_overlap_inputs_or_inert_carrier_paths_before_mutation() {
    let s = Scratch::new();
    for directory in [
        s.0.join("inputs/new-store"),
        s.0.join("untouched/absent-checkpoint"),
    ] {
        let error = run(args(&s, directory.clone())).unwrap_err();
        assert!(error.to_string().contains("overlap"), "{error}");
        assert!(!directory.exists());
        assert!(!s.0.join("result.json").exists());
        assert!(!s.0.join("events.jsonl").exists());
    }
}

#[cfg(unix)]
#[test]
fn finite_namespace_resolves_existing_symlink_ancestors_for_write_protection() {
    let s = Scratch::new();
    std::os::unix::fs::symlink(s.0.join("inputs"), s.0.join("alias")).unwrap();
    let error = run(args(&s, s.0.join("alias/new-store"))).unwrap_err();
    assert!(error.to_string().contains("overlap"), "{error}");
    assert!(!s.0.join("inputs/new-store").exists());
    assert!(!s.0.join("events.jsonl").exists());
}

#[test]
fn finite_cli_is_explicit_and_public_policy_defaults_only_aggregate_grants() {
    let parse = |tail: &[&str]| {
        args::parse_args(
            ["rustred", "finite-demand"]
                .into_iter()
                .chain(tail.iter().copied())
                .map(OsString::from),
        )
    };
    assert!(
        parse(&[
            "run",
            "--directory",
            "new",
            "--source-command",
            "source.json",
            "--policy",
            "policy.json",
            "--output",
            "result.json"
        ])
        .is_ok()
    );
    assert!(
        parse(&[
            "run",
            "--directory",
            "new",
            "--source-command",
            "source.json",
            "--policy",
            "policy.json"
        ])
        .is_err()
    );
    assert!(parse(&["inspect", "--directory", "existing", "--resume"]).is_err());
    assert!(
        parse(&[
            "inspect",
            "--directory",
            "existing",
            "--policy",
            "policy.json"
        ])
        .is_ok()
    );
    assert!(parse(&["run", "--directory", "new", "--directory", "other"]).is_err());
    let public = FiniteDemandPolicy::from_json(&serde_json::to_vec(&policy()).unwrap()).unwrap();
    let expanded: Value = serde_json::from_slice(&public.to_json().unwrap()).unwrap();
    for name in [
        "max_nodes",
        "max_rule_applications",
        "max_transport_calls",
        "max_transport_operations",
        "max_transport_endpoints",
        "max_coalescing_additions",
    ] {
        assert_eq!(expanded["per_batch_work"][name], json!(usize::MAX));
    }
    for name in [
        "max_rule_applications",
        "max_pending_frames",
        "max_coalescing_additions",
    ] {
        assert_eq!(
            expanded["prepared_reduction_aggregates"][name],
            json!(usize::MAX)
        );
    }
    assert_eq!(expanded["storage"]["max_disk_bytes"], 0); // No silent R1/R6 storage grant.
    assert_eq!(expanded["max_resident_work_bytes"], 0);
    assert_eq!(public.native().max_resident_work_bytes, 0);
    let mut resident = policy();
    resident["max_resident_work_bytes"] = json!(1_000_000);
    let resident = FiniteDemandPolicy::from_json(&serde_json::to_vec(&resident).unwrap()).unwrap();
    assert_eq!(resident.native().max_resident_work_bytes, 1_000_000);
    let mut invalid_resident = policy();
    invalid_resident["max_resident_work_bytes"] = json!(-1);
    assert!(
        FiniteDemandPolicy::from_json(&serde_json::to_vec(&invalid_resident).unwrap()).is_err()
    );
    let mut unknown = policy();
    unknown["clipped_rank"] = json!(6);
    assert!(FiniteDemandPolicy::from_json(&serde_json::to_vec(&unknown).unwrap()).is_err());
    assert!(
        args::parse_args(
            [
                "rustred",
                "walk-publish",
                "--command",
                "source.json",
                "--finite-directory",
                "finite",
                "--finite-policy",
                "policy.json",
                "--directory",
                "phase"
            ]
            .into_iter()
            .map(OsString::from)
        )
        .is_ok()
    );
    assert!(
        args::parse_args(
            [
                "rustred",
                "walk-publish",
                "--command",
                "source.json",
                "--finite-directory",
                "finite",
                "--finite-policy",
                "policy.json",
                "--checkpoint",
                "old-cp6",
                "--directory",
                "phase"
            ]
            .into_iter()
            .map(OsString::from)
        )
        .is_err()
    );
}

#[test]
fn finite_policy_null_aggregate_limits_round_trip_without_unbounding_buffers() {
    let mut value = policy();
    let fields = [
        ("", vec!["max_total_rows", "max_source_bytes"]),
        (
            "census",
            vec!["max_unique_selections", "max_identity_payload_bytes"],
        ),
        ("export", vec!["max_records", "max_live_payload_bytes"]),
        ("control", vec!["max_bytes", "max_files"]),
        (
            "storage",
            vec![
                "max_arity",
                "max_delta_records",
                "max_delta_payload_bytes",
                "max_directory_bytes",
                "max_scope_rows",
                "max_census_records",
                "max_runs",
                "max_pages",
                "max_physical_files",
                "max_disk_bytes",
                "max_lookup_pages",
                "max_lookup_bytes",
            ],
        ),
        (
            "storage/raw",
            vec![
                "max_runs",
                "max_pages",
                "max_directory_bytes",
                "max_stored_references",
                "max_merge_runs",
                "max_merge_payload_bytes",
                "max_merge_read_bytes",
            ],
        ),
    ];
    for (parent, fields) in &fields {
        for field in fields {
            let path = if parent.is_empty() {
                format!("/{field}")
            } else {
                format!("/{parent}/{field}")
            };
            *value.pointer_mut(&path).unwrap() = Value::Null;
        }
    }
    let parsed = FiniteDemandPolicy::from_json(&serde_json::to_vec(&value).unwrap()).unwrap();
    let native = parsed.native();
    assert_eq!(native.max_total_rows, usize::MAX);
    assert_eq!(native.max_source_bytes, usize::MAX);
    assert_eq!(native.census.max_unique_selections, usize::MAX);
    assert_eq!(native.export.max_records, usize::MAX);
    assert_eq!(native.storage.max_disk_bytes, u64::MAX);
    assert_eq!(native.storage.raw.max_stored_references, u64::MAX);
    let encoded: Value = serde_json::from_slice(&parsed.to_json().unwrap()).unwrap();
    for (parent, fields) in fields {
        for field in fields {
            let path = if parent.is_empty() {
                format!("/{field}")
            } else {
                format!("/{parent}/{field}")
            };
            assert!(encoded.pointer(&path).unwrap().is_null(), "{path}");
        }
    }
    for path in [
        "/windows/max_seed_points",
        "/storage/max_page_records",
        "/storage/max_cached_pages",
        "/storage/raw/max_page_records",
        "/storage/raw/max_merge_cached_pages",
    ] {
        let mut invalid = value.clone();
        *invalid.pointer_mut(path).unwrap() = Value::Null;
        assert!(
            FiniteDemandPolicy::from_json(&serde_json::to_vec(&invalid).unwrap()).is_err(),
            "{path}"
        );
    }
}
