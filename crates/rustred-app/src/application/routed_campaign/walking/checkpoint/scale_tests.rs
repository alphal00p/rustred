//! Restore-at-scale test (design §7.1): resume a copied production CP5
//! directory under this build exactly as `owner-domain-match --resume` would,
//! and stop where the walk would start. The request comes from the campaign's
//! frozen native argv through the command's own parser and request builder;
//! the store is opened by `Store::open` (request binding, semantics version,
//! every section digest), restored by `Store::resume`, and the owner payload
//! digests are bound by the same preparation path as a resume. Nothing is
//! written into the checkpoint directory; its listing is compared before and
//! after. A JSON receipt with phase timings, memory, counts and the
//! executable-change signal goes to `RUSTRED_CHECKPOINT_RESTORE_RECEIPT`.
//!
//! ```text
//! RUSTRED_CHECKPOINT_RESTORE_DIRECTORY=<copy of checkpoints/main> \
//! RUSTRED_CHECKPOINT_RESTORE_REQUEST=<campaign run>/request.json \
//! RUSTRED_CHECKPOINT_RESTORE_RECEIPT=<new file under TMP/> \
//! [RUSTRED_CHECKPOINT_RESTORE_SAVER_EXECUTABLE=<binary that wrote it>] \
//! [RUSTRED_CHECKPOINT_RESTORE_MANIFEST=<byte-identical selection.json>] \
//! [RUSTRED_CHECKPOINT_RESTORE_OWNER_BASE=<directory with identical owners>] \
//! [RUSTRED_CHECKPOINT_RESTORE_QUERIES=<byte-identical queries.json>] \
//! RAYON_NUM_THREADS=1 cargo test --release -p rustred-app --lib \
//!     restore_copied_production_checkpoint -- --ignored --nocapture
//! ```
//!
//! The three optional input overrides replace the `--manifest`,
//! `--owner-base` and `--queries` values of the frozen argv, so a live
//! campaign's input directory need not be read. They weaken nothing: the
//! request binding covers the selection and query bytes and the owner binding
//! covers every owner payload digest, so a copy that differs is refused.
//!
//! `root_blockers_tests.rs` runs a read-only analysis on the same restored
//! state through [`RestoredAnalysis`].
//!
//! Scope: no walk, no native owner import (the preparation is cancelled right
//! after the owner digests are bound), no save. Restored memory therefore
//! excludes owner programs, inspector buffers and transient walk state.
use super::super::{
    super::{RoutedCampaignRequest, input, matching, prepare},
    OwnerDomainWalkRequest, WALK_SEMANTICS_VERSION, admit_request,
};
use super::{Store, restore};
use serde_json::{Value, json};
use std::cell::RefCell;
use std::ffi::OsString;
use std::os::unix::fs::MetadataExt;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Instant, SystemTime, UNIX_EPOCH};

const DIRECTORY: &str = "RUSTRED_CHECKPOINT_RESTORE_DIRECTORY";
const REQUEST: &str = "RUSTRED_CHECKPOINT_RESTORE_REQUEST";
const RECEIPT: &str = "RUSTRED_CHECKPOINT_RESTORE_RECEIPT";
const SAVER: &str = "RUSTRED_CHECKPOINT_RESTORE_SAVER_EXECUTABLE";
/// Optional replacements of the argv's input paths (see module docs).
const INPUT_OVERRIDES: [(&str, &str); 3] = [
    ("--manifest", "RUSTRED_CHECKPOINT_RESTORE_MANIFEST"),
    ("--owner-base", "RUSTRED_CHECKPOINT_RESTORE_OWNER_BASE"),
    ("--queries", "RUSTRED_CHECKPOINT_RESTORE_QUERIES"),
];

/// Read-only work on the restored state after every validation passed and
/// before it is dropped. The restore-at-scale test itself runs none.
pub(super) trait RestoredAnalysis {
    fn analyze<const N: usize>(
        &mut self,
        request: &OwnerDomainWalkRequest,
        selection: &input::Selection,
        restored: &restore::Restored<N>,
        receipt: &mut Value,
    ) -> Result<(), String>;
}

struct NoAnalysis;
impl RestoredAnalysis for NoAnalysis {
    fn analyze<const N: usize>(
        &mut self,
        _: &OwnerDomainWalkRequest,
        _: &input::Selection,
        _: &restore::Restored<N>,
        _: &mut Value,
    ) -> Result<(), String> {
        Ok(())
    }
}

/// `VmRSS`/`VmHWM`/`VmPeak` of this process, in bytes.
pub(super) fn memory() -> Value {
    let status = std::fs::read_to_string("/proc/self/status").unwrap_or_default();
    let field = |name: &str| {
        status
            .lines()
            .find(|line| line.starts_with(name))
            .and_then(|line| line.split_ascii_whitespace().nth(1))
            .and_then(|kib| kib.parse::<u64>().ok())
            .map(|kib| kib * 1024)
    };
    json!({"vm_rss_bytes":field("VmRSS:"),"vm_hwm_bytes":field("VmHWM:"),
        "vm_peak_bytes":field("VmPeak:")})
}

/// Name, length, inode, mtime and ctime (ns) of every directory entry: any
/// write, truncation, rename or metadata change shows up as a difference.
fn listing(directory: &Path) -> Vec<Value> {
    let mut entries: Vec<Value> = std::fs::read_dir(directory)
        .expect("list checkpoint directory")
        .map(|entry| {
            let entry = entry.expect("checkpoint directory entry");
            let m = std::fs::symlink_metadata(entry.path()).expect("checkpoint entry metadata");
            json!({"name":entry.file_name().to_string_lossy(),"bytes":m.len(),"inode":m.ino(),
                "mtime_ns":i128::from(m.mtime()) * 1_000_000_000 + i128::from(m.mtime_nsec()),
                "ctime_ns":i128::from(m.ctime()) * 1_000_000_000 + i128::from(m.ctime_nsec())})
        })
        .collect();
    entries.sort_by(|a, b| a["name"].as_str().cmp(&b["name"].as_str()));
    entries
}

/// The campaign's argv, turned into its resume: `--checkpoint`/`--resume`
/// point at the copy with `--resume` (as the supervisors do), and the output,
/// event and stop paths move to never-created scratch names outside it (they
/// are transport, not part of the request binding, and nothing here opens
/// them). `inputs` replaces the value of each named input option (bound by
/// content, see module docs). Every other option is kept verbatim.
fn resume_argv(
    command: &[String],
    directory: &Path,
    scratch: &Path,
    inputs: &[(&'static str, PathBuf)],
) -> Result<(Vec<OsString>, Vec<Value>), String> {
    let mut argv = Vec::with_capacity(command.len());
    let mut rewrites = Vec::new();
    let mut checkpoint = false;
    let mut arguments = command.iter();
    argv.push(OsString::from(
        arguments.next().ok_or("empty campaign command")?,
    ));
    while let Some(argument) = arguments.next() {
        let replacement = match argument.as_str() {
            "--checkpoint" | "--resume" => {
                checkpoint = true;
                Some(("--resume", directory.to_path_buf()))
            }
            "--output" => Some(("--output", scratch.join("result.json"))),
            "--events" => Some(("--events", scratch.join("events.jsonl"))),
            "--stop-file" => Some(("--stop-file", scratch.join("stop-request.json"))),
            other => inputs
                .iter()
                .find(|(option, _)| *option == other)
                .map(|(option, path)| (*option, path.clone())),
        };
        match replacement {
            Some((option, path)) => {
                let original = arguments
                    .next()
                    .ok_or_else(|| format!("{argument} needs a value"))?;
                rewrites.push(json!({"option":argument,"original":original,
                    "resume_option":option,"resume_value":path}));
                argv.push(option.into());
                argv.push(path.into_os_string());
            }
            None => argv.push(argument.into()),
        }
    }
    if !checkpoint {
        return Err("campaign command has no --checkpoint or --resume directory".into());
    }
    Ok((argv, rewrites))
}

fn seconds(started: Instant) -> f64 {
    started.elapsed().as_secs_f64()
}

/// Everything a resume does before its first walk step, for one arity, then
/// `analysis` on the validated state.
fn restore_without_walking<const N: usize>(
    request: &OwnerDomainWalkRequest,
    selection: &input::Selection,
    load_limits: crate::CandidateOwnerLoadLimits,
    receipt: &mut Value,
    analysis: &mut impl RestoredAnalysis,
) -> Result<(), String> {
    let events = RefCell::new(Vec::<Value>::new());
    let observer = |event: Value| events.borrow_mut().push(event);

    let started = Instant::now();
    let mut store = Store::open(request)?.ok_or("request has no checkpoint")?;
    receipt["timings"]["store_open_seconds"] = json!(seconds(started));
    receipt["timings"]["verify_seconds"] = json!(store.verify_seconds);
    receipt["memory"]["after_store_open"] = memory();
    let manifest = store.manifest.clone().ok_or("resume opened no manifest")?;
    let open_events = store.take_open_events();
    receipt["executable"] = json!({
        "current_blake3":store.executable,"saved_blake3":manifest.executable,
        "executable_first_blake3":manifest.executable_first,
        "checkpoint_executable_changed":open_events.iter()
            .find(|event| event["event"] == "checkpoint_executable_changed"),
        "current_scope":"digest of the running test executable (Store::open hashes current_exe)"});
    if let Some(path) = std::env::var_os(SAVER) {
        let (bytes, blake3) = restore::file_digest(Path::new(&path))?;
        receipt["executable"]["saver_file"] = json!(PathBuf::from(&path));
        receipt["executable"]["saver_bytes"] = json!(bytes);
        receipt["executable"]["saver_blake3"] = json!(blake3);
        receipt["executable"]["saver_matches_saved"] = json!(blake3 == manifest.executable);
    }
    receipt["open_events"] = json!(open_events);
    // The owner payload digests this checkpoint bound (read by analyses that
    // must re-import the same owners elsewhere, e.g. the W0.3 fixture).
    receipt["owner_digests"] = json!(manifest.owners);

    let started = Instant::now();
    let restored = store
        .resume::<N>(&observer)?
        .ok_or("checkpoint holds no walk state")?;
    receipt["timings"]["resume_seconds"] = json!(seconds(started));
    receipt["memory"]["after_restore"] = memory();
    let restored_event = events
        .borrow()
        .iter()
        .find(|event| event["event"] == "checkpoint_restored")
        .cloned()
        .ok_or("no checkpoint_restored event")?;
    receipt["checkpoint_restored"] = restored_event["restore"].clone();
    receipt["timings"]["phase_seconds"] = restored_event["restore"]["phase_seconds"].clone();

    // Owner binding exactly as a resume performs it: the payload digests go
    // through `bind_owners`; the preparation is then cancelled before the
    // native import (not part of the restored walk state).
    let started = Instant::now();
    let mut load = RoutedCampaignRequest::new(String::new(), String::new());
    load.owner_base = request.matching.owner_base.clone();
    load.reduction_limits = request.matching.reduction_limits;
    let stop_before_native_load = AtomicBool::new(false);
    let mut bound = None;
    let mut bind = |owners: Vec<String>| {
        bound = Some(owners.len());
        let outcome = store.bind_owners(owners);
        stop_before_native_load.store(true, Ordering::Relaxed);
        outcome
    };
    let prepared = prepare::prepare_with_fingerprints::<N>(
        &load,
        selection,
        load_limits,
        &stop_before_native_load,
        &|_: Value| {},
        Some(&mut bind),
    )
    .map_err(|e| e.to_string())?;
    if prepared.is_some() || bound.is_none() {
        return Err("owner binding did not run before the native import".into());
    }
    receipt["owner_binding"] = json!({"owners":bound,"bound":true,
        "seconds":seconds(started),"native_owner_import":"skipped (cancelled after binding)"});
    store.attach_records(&restored.state)?;
    receipt["memory"]["after_owner_binding"] = memory();

    let state = &restored.state;
    let total = state.queue.domains.len();
    let closure = state.closure.borrow();
    let closure_json = closure.json(total, state.initial_domain_count);
    let ledger_entries = state.queue.delegation.as_ref().map(|l| l.len());
    let counts = json!({
        "domains":total,
        "live_candidates":state.queue.containment_candidate_count(),
        "dependency_edges":closure.edge_count(),
        "records":state.records.borrow().total(),
        "ledger_entries":ledger_entries,
        "committed_domains":state.published_count(),
        "pending_domains":total - state.published_count(),
        "contiguous_publication_watermark":state.queue.next,
        "completed_native_inspections":state.completed,
        "native_records":state.native_records,
        "committed_events":state.events,
        "records_accepted_events":state.records_accepted_events,
        "initial_domains":state.initial_domain_count,
        "total_closed":closure_json["total_closed"],
        "inputs":restored.inputs.len(),"input_frontiers":restored.input_frontiers.len()});
    drop(closure);
    let m = &manifest.metadata;
    let s = &manifest.sections;
    let expected = [
        ("domains", json!(s.domains.as_ref().map(|d| d.total))),
        ("dependency_edges", json!(s.edges.as_ref().map(|d| d.total))),
        ("records", json!(s.records.as_ref().map(|d| d.total))),
        ("committed_domains", m["committed_domains"].clone()),
        ("pending_domains", m["pending_domains"].clone()),
        (
            "contiguous_publication_watermark",
            m["contiguous_publication_watermark"].clone(),
        ),
        (
            "completed_native_inspections",
            m["completed_native_inspections"].clone(),
        ),
        ("committed_events", m["committed_events"].clone()),
        ("ledger_entries", json!(s.ledger.as_ref().map(|_| total))),
    ];
    let mut agreement = serde_json::Map::new();
    let mut disagreements = Vec::new();
    for (key, saved) in expected {
        let equal = saved == counts[key];
        if !equal {
            disagreements.push(key);
        }
        agreement.insert(
            key.into(),
            json!({"manifest":saved,"restored":counts[key],"equal":equal}),
        );
    }
    receipt["counts"] = counts;
    receipt["manifest_agreement"] = Value::Object(agreement);
    receipt["queue_storage"] = state.queue.storage_json();
    receipt["descendant_closure"] = closure_json;
    receipt["manifest"] = json!({"schema":manifest.schema,"format":manifest.format,
        "kind":manifest.kind,"generation":manifest.generation,
        "walk_semantics_version":manifest.walk_semantics_version,"arity":manifest.arity,
        "publication_policy":manifest.publication_policy,"request":manifest.request,
        "owners":manifest.owners.len(),"bytes":manifest.total_bytes(),
        "saved_unix_time":m["saved_unix_time"],"save_seconds":m["save_seconds"]});
    if !disagreements.is_empty() {
        return Err(format!(
            "restored counts disagree with the manifest: {disagreements:?}"
        ));
    }
    let started = Instant::now();
    analysis.analyze::<N>(request, selection, &restored, receipt)?;
    receipt["timings"]["analysis_seconds"] = json!(seconds(started));
    receipt["memory"]["after_analysis"] = memory();
    let started = Instant::now();
    drop(restored);
    drop(store);
    receipt["timings"]["drop_seconds"] = json!(seconds(started));
    receipt["memory"]["after_drop"] = memory();
    Ok(())
}

#[test]
#[ignore = "needs a copied production CP5 checkpoint and its campaign request (see module docs)"]
fn restore_copied_production_checkpoint() {
    restore_copied_checkpoint(
        "restore_copied_production_checkpoint",
        "rustred.checkpoint-restore-at-scale.v1",
        "checkpoint-restore-receipt",
        &mut NoAnalysis,
    );
}

/// The restore-at-scale procedure (module docs) followed by `analysis`; the
/// receipt is written under `schema`, and the test fails on any refusal, on
/// an analysis error or when the checkpoint directory changed.
pub(super) fn restore_copied_checkpoint(
    test: &str,
    schema: &str,
    default_receipt_prefix: &str,
    analysis: &mut impl RestoredAnalysis,
) {
    let started = Instant::now();
    let unix = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock")
        .as_secs();
    let directory = PathBuf::from(std::env::var_os(DIRECTORY).expect(DIRECTORY));
    let directory = std::fs::canonicalize(&directory).expect("checkpoint directory");
    let request_file = PathBuf::from(std::env::var_os(REQUEST).expect(REQUEST));
    let receipt_path = std::env::var_os(RECEIPT).map_or_else(
        || {
            Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("../../TMP")
                .join(format!("{default_receipt_prefix}-{unix}.json"))
        },
        PathBuf::from,
    );
    let scratch = receipt_path
        .parent()
        .expect("receipt directory")
        .join("unused-cli-outputs");
    let mut receipt = json!({"schema":schema,
        "test":test,"started_unix_time":unix,
        "directory":directory,"request_file":request_file,
        "walk_semantics_version":WALK_SEMANTICS_VERSION,
        "rayon_threads":rayon::current_num_threads(),
        "rayon_num_threads_env":std::env::var("RAYON_NUM_THREADS").ok(),
        "timings":{},"memory":{"start":memory()},
        "scope":"restore and validation only: request binding, semantics version, every section digest, section decode and validation, ledger/closure cross-check, owner digest binding; no walk step, no native owner import, no save; restored memory excludes owner programs, inspector buffers and transient walk state",
        "family_closure_claim":false});
    let before = listing(&directory);

    let request_json: Value =
        serde_json::from_slice(&std::fs::read(&request_file).expect("campaign request"))
            .expect("campaign request JSON");
    let command: Vec<String> = request_json["command"]
        .as_array()
        .expect("campaign request has a frozen native \"command\" argv")
        .iter()
        .map(|argument| argument.as_str().expect("argv string").to_owned())
        .collect();
    let inputs: Vec<(&'static str, PathBuf)> = INPUT_OVERRIDES
        .iter()
        .filter_map(|&(option, variable)| {
            std::env::var_os(variable).map(|path| (option, PathBuf::from(path)))
        })
        .collect();
    let (argv, rewrites) =
        resume_argv(&command, &directory, &scratch, &inputs).expect("resume argv");
    receipt["argv_original"] = json!(command);
    receipt["argv_resume"] = json!(
        argv.iter()
            .map(|a| a.to_string_lossy().into_owned())
            .collect::<Vec<_>>()
    );
    receipt["argv_rewrites"] = json!(rewrites);
    let request_started = Instant::now();
    let request = crate::cli::walk_request_from_argv(argv).expect("CLI walk request");
    let checkpoint = request.checkpoint.as_ref().expect("checkpointed request");
    assert!(checkpoint.resume && checkpoint.directory == directory);
    admit_request(&request).expect("walk request admission");
    let (selection, arity, load_limits) =
        input::Selection::parse(&request.matching.selection_json).expect("selection manifest");
    let queries = matching::input::parse(
        &request.matching.queries_json,
        arity,
        request.matching.max_queries,
        request.matching.max_query_bytes,
    )
    .expect("campaign queries");
    receipt["timings"]["request_build_seconds"] = json!(seconds(request_started));
    receipt["request"] = json!({"arity":arity,"queries":queries.len(),
        "owners":selection.owners.len(),"workers":request.workers,
        "publication_policy":format!("{:?}",request.publication_policy),
        "scheduling_policy":format!("{:?}",request.scheduling_policy),
        "interval_seconds":checkpoint.interval_seconds,
        "binding":super::binding(&request),
        "admission":"admit_request (walk allowances and policies); host core-budget and inner-pool environment preflights not applied"});
    drop(queries);

    macro_rules! dispatch { ($($n:literal),*) => { match arity {
        $($n => restore_without_walking::<$n>(&request, &selection, load_limits, &mut receipt, analysis),)*
        _ => Err(format!("unsupported arity {arity}")),
    }} }
    let outcome = dispatch!(1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16);
    let after = listing(&directory);
    let unchanged = before == after;
    receipt["directory_listing"] = json!(after);
    receipt["directory_unchanged"] = json!(unchanged);
    if !unchanged {
        receipt["directory_listing_before"] = json!(before);
    }
    receipt["timings"]["total_seconds"] = json!(seconds(started));
    receipt["memory"]["end"] = memory();
    receipt["passed"] = json!(outcome.is_ok() && unchanged);
    receipt["error"] = json!(outcome.as_ref().err());
    let bytes = serde_json::to_vec_pretty(&receipt).expect("receipt JSON");
    std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&receipt_path)
        .and_then(|mut file| std::io::Write::write_all(&mut file, &bytes))
        .expect("write receipt");
    println!("receipt: {}", receipt_path.display());
    println!(
        "{}",
        serde_json::to_string_pretty(&json!({"timings":receipt["timings"],
            "memory":receipt["memory"],"counts":receipt["counts"],
            "executable":receipt["executable"],"passed":receipt["passed"],
            "error":receipt["error"],"analysis_summary":receipt["analysis_summary"]}))
        .expect("summary JSON")
    );
    outcome.expect("restore without walking");
    assert!(unchanged, "checkpoint directory changed during restore");
}

#[test]
fn resume_argv_points_the_checkpoint_at_the_copy_and_keeps_policy() {
    let command: Vec<String> = [
        "/campaign/bin/rustred",
        "owner-domain-match",
        "--output",
        "/campaign/run/result.json",
        "--events",
        "/campaign/run/events.jsonl",
        "--stop-file",
        "/campaign/run/stop-request.json",
        "--workers",
        "100",
        "--checkpoint",
        "/campaign/checkpoints/main",
        "--checkpoint-interval-seconds",
        "14400",
    ]
    .map(str::to_owned)
    .to_vec();
    let (argv, rewrites) =
        resume_argv(&command, Path::new("/copy"), Path::new("/scratch"), &[]).expect("argv");
    let argv: Vec<_> = argv.iter().map(|a| a.to_str().unwrap()).collect();
    assert_eq!(
        argv,
        [
            "/campaign/bin/rustred",
            "owner-domain-match",
            "--output",
            "/scratch/result.json",
            "--events",
            "/scratch/events.jsonl",
            "--stop-file",
            "/scratch/stop-request.json",
            "--workers",
            "100",
            "--resume",
            "/copy",
            "--checkpoint-interval-seconds",
            "14400",
        ]
    );
    assert_eq!(rewrites.len(), 4);
    let without: Vec<String> = command[..10].to_vec();
    assert!(
        resume_argv(&without, Path::new("/copy"), Path::new("/scratch"), &[])
            .unwrap_err()
            .contains("--checkpoint")
    );
}

#[test]
fn resume_argv_replaces_only_the_named_input_paths() {
    let command: Vec<String> = [
        "/campaign/bin/rustred",
        "owner-domain-match",
        "--manifest",
        "/campaign/inputs/selection.json",
        "--owner-base",
        "/campaign/inputs",
        "--queries",
        "/campaign/inputs/queries.json",
        "--checkpoint",
        "/campaign/checkpoints/main",
    ]
    .map(str::to_owned)
    .to_vec();
    let inputs = [
        ("--manifest", PathBuf::from("/copy-inputs/selection.json")),
        ("--owner-base", PathBuf::from("/copy-inputs")),
    ];
    let (argv, rewrites) =
        resume_argv(&command, Path::new("/copy"), Path::new("/scratch"), &inputs).expect("argv");
    let argv: Vec<_> = argv.iter().map(|a| a.to_str().unwrap()).collect();
    assert_eq!(
        argv,
        [
            "/campaign/bin/rustred",
            "owner-domain-match",
            "--manifest",
            "/copy-inputs/selection.json",
            "--owner-base",
            "/copy-inputs",
            "--queries",
            "/campaign/inputs/queries.json",
            "--resume",
            "/copy",
        ]
    );
    assert_eq!(rewrites.len(), 3);
}
