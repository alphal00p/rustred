use super::*;
use std::sync::atomic::AtomicU64;

struct Fixture(PathBuf);
impl Fixture {
    fn new() -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let path = std::env::temp_dir().join(format!(
            "rustred-shard-supervisor-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&path).unwrap();
        Self(path)
    }
    fn setup(&self, mode: &str, count: usize) -> Snapshot {
        fs::create_dir(self.0.join("bin")).unwrap();
        fs::create_dir(self.0.join("artifact")).unwrap();
        fs::create_dir(self.0.join("jobs")).unwrap();
        fs::write(self.0.join("artifact/selection.json"), b"{}").unwrap();
        let script = FAKE.replace("@MODE@", mode);
        fs::write(self.0.join("bin/rustred"), script).unwrap();
        checkpoint::immutable(&self.0.join("bin/rustred"), true).unwrap();
        let cpus: Vec<_> = resources::allowed_cpus()
            .unwrap()
            .into_iter()
            .take(count)
            .collect();
        assert_eq!(
            cpus.len(),
            count,
            "fake two-process test needs two allowed CPUs"
        );
        let config:Config=serde_json::from_value(json!({"schema":"rustred.independent-root-config.v1","manifest":"selection.json","queries":"queries.json","owner_base":".","jobs":count,"workers_per_job":1,"total_workers":count,"cpus":cpus,"max_memory_bytes":u64::MAX})).unwrap();
        let shards = (0..count)
            .map(|id| super::super::plan::Shard {
                id,
                owners: vec![format!("{id:02b}")],
                query_indices: vec![id],
            })
            .collect::<Vec<_>>();
        for shard in &shards {
            fs::create_dir(self.0.join(format!("jobs/{:04}", shard.id))).unwrap();
            fs::write(
                self.0.join(format!("jobs/{:04}/queries.json", shard.id)),
                b"{\"schema\":\"rustred.owner-domain-queries.json.v2\",\"queries\":[{}]}",
            )
            .unwrap();
        }
        let mut executable = checkpoint::FileIdentity::record(&self.0.join("bin/rustred")).unwrap();
        executable.path = "bin/rustred".into();
        Snapshot {
            schema: "rustred.independent-root-snapshot.v1".into(),
            config,
            shards,
            files: vec![executable],
            source_files: vec![],
            root_count: count,
            query_count: count,
        }
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.0).unwrap();
    }
}

// This child exercises the actual process/argument/receipt transport without
// native algebra, licensing or large programs. The pause loop has a test-only
// bound so a regression cannot leave a diagnostic child running indefinitely.
const FAKE: &str = r#"#!/bin/sh
[ "$1" = owner-domain-match ] || exit 64
[ "$RAYON_NUM_THREADS" = 1 ] || exit 65
shift
while [ "$#" -gt 0 ]; do
  case "$1" in
    --output) output="$2"; shift ;;
    --events) events="$2"; shift ;;
    --stop-file) stop="$2"; shift ;;
    --checkpoint|--resume) check="$2"; shift ;;
  esac
  shift
done
if [ '@MODE@' = pause ]; then
  mkdir -p "$check"
  printf '%s\n' '{"metadata":{"state":"saved"}}' > "$check/latest.json"
  head -c 1100000 /dev/zero | tr '\0' ' ' >> "$check/latest.json"
  count=0
  while [ ! -f "$stop" ] && [ "$count" -lt 200 ]; do sleep 0.02; count=$((count+1)); done
  [ -f "$stop" ] || exit 66
  printf '%s\n' '{"event":"checkpoint_saved","checkpoint":{"state":"saved","generation":1,"saved_unix_time":1}}' '{"event":"finished","operation":"owner_domain_walk","status":"paused"}' > "$events"
  printf '%s\n' '{"status":"paused"}' > "$output"
  exit 4
fi
printf '%s\n' '{"event":"finished","operation":"owner_domain_walk","status":"locally_resolved","all_scheduled_domains_resolved":true,"recursive_worklist_exhausted":true,"completed_nodes":9,"queued_nodes":0,"failed_nodes":0,"frontiers":0}' > "$events"
printf '%s\n' '{"status":"locally_resolved"}' > "$output"
"#;

#[test]
fn two_child_processes_use_distinct_affinity_and_cooperative_pause() {
    let fixture = Fixture::new();
    let snapshot = fixture.setup("pause", 2);
    let mut state = MasterState::new(&snapshot.shards);
    let mut children = Children(
        vec![
            launch(&fixture.0, &snapshot, 0, 0, 1, None).unwrap(),
            launch(&fixture.0, &snapshot, 1, 1, 1, None).unwrap(),
        ],
        u64::MAX,
        0,
    );
    assert_ne!(children.0[0].cpus, children.0[1].cpus);
    for child in &children.0 {
        assert!(resources::same_process(child.child.id(), child.start_ticks));
        let affinity = fs::read_to_string(format!("/proc/{}/status", child.child.id())).unwrap();
        assert!(affinity.lines().any(|line| {
            line.strip_prefix("Cpus_allowed_list:")
                .is_some_and(|v| v.trim() == child.cpus[0].to_string())
        }));
        child.stop().unwrap();
    }
    for mut child in children.0.drain(..) {
        let exit = child.child.wait().unwrap();
        child.poll().unwrap();
        finish(&fixture.0, &snapshot, &mut state, &child, exit.code()).unwrap();
        assert_eq!(state.jobs[child.id].state, "paused");
    }
    validate_resume(&fixture.0, &snapshot, &mut state).unwrap();
    assert!(state.jobs.iter().all(|job| job.state == "queued"));
}

#[test]
fn completion_receipt_skips_finished_jobs_and_detects_tampering() {
    let fixture = Fixture::new();
    let snapshot = fixture.setup("complete", 1);
    let mut state = MasterState::new(&snapshot.shards);
    let mut child = launch(&fixture.0, &snapshot, 0, 0, 1, None).unwrap();
    let launch_doc: Value =
        checkpoint::read_json(&child.directory.join("launch.json"), 1024 * 1024).unwrap();
    let args = std::iter::once(std::ffi::OsString::from("rustred")).chain(
        launch_doc["arguments"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| std::ffi::OsString::from(v.as_str().unwrap())),
    );
    assert!(matches!(
        super::super::super::args::parse_args(args).unwrap(),
        super::super::super::args::Command::OwnerDomainMatch(_)
    ));
    let exit = child.child.wait().unwrap();
    child.poll().unwrap();
    finish(&fixture.0, &snapshot, &mut state, &child, exit.code()).unwrap();
    assert_eq!(state.jobs[0].state, "completed");
    validate_resume(&fixture.0, &snapshot, &mut state).unwrap();
    assert_eq!(state.jobs[0].state, "completed");
    // Receipt installation survived; recover after loss of the final state write.
    state.jobs[0].state = "running".into();
    state.jobs[0].receipt = None;
    validate_resume(&fixture.0, &snapshot, &mut state).unwrap();
    assert_eq!(state.jobs[0].state, "completed");
    fs::write(fixture.0.join("jobs/0000/completion.json"), b"{}").unwrap();
    assert!(validate_resume(&fixture.0, &snapshot, &mut state).is_err());
}

#[test]
fn resume_refuses_live_identity_and_changed_frozen_bytes() {
    let fixture = Fixture::new();
    let snapshot = fixture.setup("pause", 1);
    let mut state = MasterState::new(&snapshot.shards);
    let mut children = Children(
        vec![launch(&fixture.0, &snapshot, 0, 0, 1, None).unwrap()],
        u64::MAX,
        0,
    );
    let job = &children.0[0];
    state.jobs[0].state = "running".into();
    state.jobs[0].pid = Some(job.child.id());
    state.jobs[0].start_ticks = Some(job.start_ticks);
    assert!(validate_resume(&fixture.0, &snapshot, &mut state).is_err());
    children.0[0].stop().unwrap();
    children.0[0].child.wait().unwrap();
    children.0.clear();
    fs::remove_file(fixture.0.join("bin/rustred")).unwrap();
    fs::write(fixture.0.join("bin/rustred"), b"changed").unwrap();
    assert!(snapshot.verify(&fixture.0).is_err());
}

#[test]
fn successful_exit_without_native_completion_is_incomplete() {
    let fixture = Fixture::new();
    let snapshot = fixture.setup("complete", 1);
    let mut state = MasterState::new(&snapshot.shards);
    let mut child = launch(&fixture.0, &snapshot, 0, 0, 1, None).unwrap();
    let exit = child.child.wait().unwrap();
    assert!(exit.success());
    assert!(finish(&fixture.0, &snapshot, &mut state, &child, exit.code()).is_err());
}

#[test]
fn recent_throughput_uses_only_current_attempt_deltas_and_resets_on_regression() {
    let mut previous = None;
    assert_eq!(recent_rate(&mut previous, 10000, 1.0), None);
    assert_eq!(recent_rate(&mut previous, 10008, 3.0), Some(4.0));
    assert_eq!(recent_rate(&mut previous, 10008, 5.0), Some(0.0));
    assert_eq!(recent_rate(&mut previous, 2, 6.0), None);
    assert_eq!(recent_rate(&mut previous, 4, 7.0), Some(2.0));
    assert_eq!(recent_rate(&mut previous, 5, 7.0), None);
}

#[test]
fn orphan_child_retains_campaign_lock_until_exit() {
    let fixture = Fixture::new();
    let snapshot = fixture.setup("pause", 1);
    let lock = checkpoint::acquire_lock(&fixture.0).unwrap();
    let mut children = Children(
        vec![launch(&fixture.0, &snapshot, 0, 0, 1, Some(&lock)).unwrap()],
        u64::MAX,
        0,
    );
    drop(lock);
    assert!(checkpoint::acquire_lock(&fixture.0).is_err());
    children.0[0].stop().unwrap();
    children.0[0].child.wait().unwrap();
    children.0.clear();
    checkpoint::acquire_lock(&fixture.0).unwrap();
}
