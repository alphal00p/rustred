use super::*;
use crate::cli::progress::preparation::model::tests::{final_snapshot, fixture};
use serde_json::{Value, json};
use std::sync::atomic::{AtomicU64, Ordering};

struct Scratch(PathBuf);
impl Scratch {
    fn new() -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../TMP")
            .join(format!(
                "final-viewer-{}-{}-{}",
                std::process::id(),
                std::time::SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap()
                    .as_nanos(),
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

fn write(path: &Path, value: &Value) {
    std::fs::write(path, serde_json::to_vec(value).unwrap()).unwrap();
}

fn setup(scratch: &Scratch) -> (PathBuf, Value, PathBuf) {
    let directory = scratch.0.join("attempt");
    std::fs::create_dir(&directory).unwrap();
    let snapshot = directory.join("snapshot.json");
    let native = scratch.0.join("parent-native.json");
    write(&native, &serde_json::to_value(final_snapshot()).unwrap());
    let mut value = fixture();
    value["run_id"] = scratch
        .0
        .canonicalize()
        .unwrap()
        .to_string_lossy()
        .to_string()
        .into();
    value["attempt_id"] = "attempt".into();
    value["timestamp_unix_seconds"] = std::time::SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs_f64()
        .into();
    value["jobs"][0]["state"] = "completed".into();
    value["jobs"][0]["pid"] = 9.into();
    value["jobs"][0]["pid_start"] = 17.into();
    value["jobs"][0]["metadata"]["selected_sectors"] = json!(
        (1..=18)
            .map(|sector| format!("{sector:018b}"))
            .collect::<Vec<_>>()
    );
    value["jobs"][0]["native_progress"] =
        json!({"status":"stale","age_seconds":20000.,"snapshot":null});
    let batch = scratch.0.join("batch.json");
    write(
        &batch,
        &json!({"schema":"rustred.family-candidates-batch.v1",
        "jobs":[{"id":"parent-3","arguments":["--progress-json",native]}]}),
    );
    write(
        &directory.join("request.json"),
        &json!({"started_unix":1.,"jobs":[{
        "id":"shared-worker-pool", "command":["rustred","family-candidates-batch","--config",batch]}]}),
    );
    (snapshot, value, native)
}

fn parse(value: &Value) -> Status {
    Status::parse(&serde_json::to_vec(value).unwrap()).unwrap()
}

#[test]
fn old_running_producer_recovers_final_counts_from_its_recorded_paths_only() {
    let scratch = Scratch::new();
    let (path, value, _) = setup(&scratch);
    let mut status = parse(&value);
    recover(&path, &mut status);
    status.expire_native(status.timestamp_unix_seconds + 20000.);
    let native = status.jobs[0].native().unwrap();
    assert_eq!(native.counts.generated + native.counts.reused, 18);
    assert_eq!(native.counts.rules_generated, 1781);
    assert!(!status.family_closure_claim);
    let shown = super::super::render::plain(&status, status.timestamp_unix_seconds, true);
    assert!(shown.contains("completed · final counters"));
    assert!(shown.contains("sectors 18/18"));
    assert!(!shown.contains("native stale"));
    assert_eq!(
        value["jobs"][0]["native_progress"]["status"], "stale",
        "input was not rewritten"
    );
}

#[test]
fn explicit_progress_paths_need_no_request_and_preserve_real_observation_age() {
    let scratch = Scratch::new();
    let (path, mut value, native) = setup(&scratch);
    std::fs::remove_file(path.parent().unwrap().join("request.json")).unwrap();
    value["jobs"][0]["started_unix"] = 1.into();
    value["jobs"][0]["native_progress_path"] =
        native.into_os_string().to_string_lossy().to_string().into();
    let mut status = parse(&value);
    recover(&path, &mut status);
    assert!(status.jobs[0].native().is_some());
    assert!(
        status.jobs[0]
            .native_progress
            .as_ref()
            .unwrap()
            .native_observed_at
            .is_some()
    );
}

#[test]
fn recovery_refuses_active_failed_wrong_process_and_old_attempt_data() {
    let scratch = Scratch::new();
    let (path, original, _) = setup(&scratch);
    for (state, pid, ticks) in [
        ("running", 9, 17),
        ("failed", 9, 17),
        ("completed", 10, 17),
        ("completed", 9, 18),
    ] {
        let mut value = original.clone();
        value["jobs"][0]["state"] = state.into();
        value["jobs"][0]["pid"] = pid.into();
        value["jobs"][0]["pid_start"] = ticks.into();
        let mut status = parse(&value);
        recover(&path, &mut status);
        assert!(status.jobs[0].native().is_none());
    }
    let mut request: Value = serde_json::from_slice(
        &std::fs::read(path.parent().unwrap().join("request.json")).unwrap(),
    )
    .unwrap();
    request["started_unix"] = (original["timestamp_unix_seconds"].as_f64().unwrap() + 3600.).into();
    write(&path.parent().unwrap().join("request.json"), &request);
    let mut status = parse(&original);
    status.timestamp_unix_seconds += 7200.;
    recover(&path, &mut status);
    assert!(status.jobs[0].native().is_none());
}

#[test]
fn recovery_refuses_nonfinal_malformed_and_closure_claiming_native_files() {
    let scratch = Scratch::new();
    let (path, value, native) = setup(&scratch);
    for replacement in [
        json!({"state":"running"}),
        json!({"state":"failed"}),
        json!({"schema":"other"}),
        json!({"family_closure_claim":true}),
    ] {
        let mut record = serde_json::to_value(final_snapshot()).unwrap();
        for (key, item) in replacement.as_object().unwrap() {
            record[key] = item.clone();
        }
        write(&native, &record);
        let mut status = parse(&value);
        recover(&path, &mut status);
        assert!(status.jobs[0].native().is_none());
    }
    std::fs::write(&native, "malformed").unwrap();
    let mut status = parse(&value);
    recover(&path, &mut status);
    assert!(status.jobs[0].native().is_none());
}

#[test]
fn recovery_rejects_foreign_attempt_layout_duplicate_paths_and_native_symlinks() {
    let scratch = Scratch::new();
    let (path, mut value, native) = setup(&scratch);
    value["attempt_id"] = "foreign".into();
    let mut status = parse(&value);
    recover(&path, &mut status);
    assert!(status.jobs[0].native().is_none());
    value["attempt_id"] = "attempt".into();
    let mut batch: Value =
        serde_json::from_slice(&std::fs::read(scratch.0.join("batch.json")).unwrap()).unwrap();
    let duplicate = batch["jobs"][0].clone();
    batch["jobs"].as_array_mut().unwrap().push(duplicate);
    write(&scratch.0.join("batch.json"), &batch);
    let mut status = parse(&value);
    recover(&path, &mut status);
    assert!(status.jobs[0].native().is_none());
    #[cfg(unix)]
    {
        value["jobs"][0]["started_unix"] = 1.into();
        let link = scratch.0.join("native-link.json");
        std::os::unix::fs::symlink(&native, &link).unwrap();
        value["jobs"][0]["native_progress_path"] = link.to_string_lossy().to_string().into();
        let mut status = parse(&value);
        recover(&path, &mut status);
        assert!(status.jobs[0].native().is_none());
    }
}
