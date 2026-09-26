//! Ignored insertion-only replay of real retained queue geometry. No native
//! program is loaded, no descendant is inspected, and no closure is claimed.
//! Preparation validates two genuine checkpoints once; timed runs use their
//! frozen queue plus complete appended-domain sequence, not original callbacks.
use super::*;
use crate::application::routed_campaign::walking::{checkpoint, queue::Domain};
use serde::Serialize;
use std::fs::{File, OpenOptions};
use std::io::{BufReader, BufWriter, Write};
use std::num::NonZeroUsize;
use std::path::{Path, PathBuf};

const N: usize = 15;

fn directory() -> PathBuf {
    std::env::var_os("RUSTRED_ADMISSION_GRAIN_DIRECTORY")
        .map(PathBuf::from)
        .expect("explicit ignored replay directory")
}

fn load<T: serde::de::DeserializeOwned>(path: &Path) -> T {
    serde_json::from_reader(BufReader::new(File::open(path).unwrap())).unwrap()
}

fn save(path: &Path, value: &impl Serialize) {
    let file = OpenOptions::new()
        .create_new(true)
        .write(true)
        .open(path)
        .unwrap();
    let mut output = BufWriter::new(file);
    serde_json::to_writer(&mut output, value).unwrap();
    output.write_all(b"\n").unwrap();
    output.flush().unwrap();
    output.get_ref().sync_all().unwrap();
}

fn restore(state: &Path, manifest: &Path) -> State<N> {
    let expected: Value = load(manifest);
    assert_eq!(expected["schema"], 2);
    assert_eq!(expected["kind"], "state");
    let bytes = std::fs::read(state).unwrap();
    assert_eq!(expected["bytes"].as_u64().unwrap(), bytes.len() as u64);
    assert_eq!(
        expected["digest"],
        blake3::hash(&bytes).to_hex().to_string()
    );
    checkpoint::codec::read(bytes.as_slice()).unwrap().state
}

/// Only the outer owner-bucket HashMap order is unordered. Inner group/block,
/// immutable ID, domain and responsibility ordering remains comparison data.
fn canonical_queue<const D: usize>(queue: &Queue<D>) -> Value {
    let mut image = serde_json::to_value(queue).unwrap();
    image[2]
        .as_array_mut()
        .unwrap()
        .sort_by_cached_key(|bucket| serde_json::to_string(&(&bucket[0], &bucket[1])).unwrap());
    image
}

struct Digest(blake3::Hasher);
impl Write for Digest {
    fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
        self.0.update(bytes);
        Ok(bytes.len())
    }
    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}
fn digest(value: &impl Serialize) -> String {
    let mut output = Digest(blake3::Hasher::new());
    serde_json::to_writer(&mut output, value).unwrap();
    output.0.finalize().to_hex().to_string()
}

fn counts<const D: usize>(queue: &Queue<D>) -> Value {
    let ledger = queue.delegation.as_ref().unwrap();
    json!({"domains":queue.domains.len(), "candidates":queue.containment_candidate_count(),
        "containment_checks":queue.containment_checks,
        "maintenance_checks":queue.containment_maintenance_checks,
        "retired":queue.containment_retired_candidates,
        "summary_builds":queue.containment_summary_builds,
        "semantic_hits":queue.containment_semantic_hits,
        "semantic_retirements":queue.containment_semantic_retirements,
        "deduplicated":queue.deduplicated,"exact_hits":queue.exact_hits,
        "orthant_hits":queue.orthant_hits,"watermark":queue.next,
        "ledger_published":ledger.published_count(),
        "ledger_transfers":ledger.transfer_count(),
        "ledger_outstanding_native":ledger.outstanding_native(),
        "ledger_reservation_scan":ledger.reservation_scan()})
}

#[test]
#[ignore = "explicit genuine-checkpoint preparation; never part of the default gate"]
fn prepare_retained_admission_grain_fixture() {
    let root = directory();
    let input: Value = load(&root.join("input.json"));
    let base = restore(
        Path::new(input["base_state"].as_str().unwrap()),
        Path::new(input["base_manifest"].as_str().unwrap()),
    );
    let later = restore(
        Path::new(input["later_state"].as_str().unwrap()),
        Path::new(input["later_manifest"].as_str().unwrap()),
    );
    assert_eq!(
        base.queue.domains.len(),
        input["base_domains"].as_u64().unwrap() as usize
    );
    assert_eq!(
        later.queue.domains.len(),
        input["later_domains"].as_u64().unwrap() as usize
    );
    let prefix = base.queue.domains.len();
    assert!(later.queue.domains.len() > prefix);
    assert_eq!(
        base.queue.domains.as_slice(),
        &later.queue.domains[..prefix]
    );
    let ledger = base.queue.delegation.as_ref().unwrap();
    assert!(ledger.is_ready());
    assert_eq!(ledger.lookahead().get(), 256);
    ledger.validate_checkpoint().unwrap();
    assert_eq!(base.queue.containment_limit(), None);
    let proposals: Vec<_> = later.queue.domains[prefix..]
        .iter()
        .map(|d| d.as_ref().clone())
        .collect();
    assert_eq!(proposals.len(), 60_154);
    assert!(proposals.len() % BATCH_RECORDS >= MIN_ADMISSIONS);
    let report = json!({"base_domains":prefix,"later_domains":later.queue.domains.len(),
        "proposals":proposals.len(),"common_prefix_exact":true,
        "baseline_counts":counts(&base.queue),
        "baseline_canonical_blake3":digest(&canonical_queue(&base.queue)),
        "proposal_blake3":digest(&proposals),
        "ready_transfer_unreserved_lookahead":256,
        "scope":"retained-admission geometry only; excludes original rejected/reused proposals and callback flags",
        "historical_gen7_lifecycle_replayed":false,"resumable_campaign_state":false});
    save(&root.join("baseline-queue.json"), &base.queue);
    save(&root.join("proposals.json"), &proposals);
    save(&root.join("fixture.json"), &report);
    eprintln!(
        "retained admission fixture prepared: {prefix} base + {} proposals",
        proposals.len()
    );
}

fn cpu_ticks() -> u64 {
    let stat = std::fs::read_to_string("/proc/self/stat").unwrap();
    let fields: Vec<_> = stat[stat.rfind(')').unwrap() + 1..]
        .split_whitespace()
        .collect();
    fields[11]
        .parse::<u64>()
        .unwrap()
        .checked_add(fields[12].parse().unwrap())
        .unwrap()
}

#[test]
#[ignore = "explicit 24-helper admission microbenchmark; not a solver or closure gate"]
fn replay_retained_admission_grain() {
    let root = directory();
    let setup = Instant::now();
    let fixture: Value = load(&root.join("fixture.json"));
    let proposals: Vec<Domain<N>> = load(&root.join("proposals.json"));
    assert_eq!(digest(&proposals), fixture["proposal_blake3"]);
    assert_eq!(
        proposals.len(),
        fixture["proposals"].as_u64().unwrap() as usize
    );
    let order = std::env::var("RUSTRED_ADMISSION_GRAIN_ORDER").unwrap();
    assert!(matches!(order.as_str(), "default,min8" | "min8,default"));
    let output = PathBuf::from(std::env::var_os("RUSTRED_ADMISSION_GRAIN_OUTPUT").unwrap());
    let hz = std::process::Command::new("getconf")
        .arg("CLK_TCK")
        .output()
        .unwrap();
    assert!(hz.status.success());
    let hz: u64 = std::str::from_utf8(&hz.stdout)
        .unwrap()
        .trim()
        .parse()
        .unwrap();
    assert!(hz > 0);
    let setup_seconds = setup.elapsed().as_secs_f64();
    let mut results = Vec::new();
    let mut reference = None;
    for mode in order.split(',') {
        let preparation = Instant::now();
        let mut queue: Queue<N> = load(&root.join("baseline-queue.json"));
        assert_eq!(
            digest(&canonical_queue(&queue)),
            fixture["baseline_canonical_blake3"]
        );
        assert_eq!(counts(&queue), fixture["baseline_counts"]);
        queue
            .delegation
            .as_ref()
            .unwrap()
            .validate_checkpoint()
            .unwrap();
        // Test-only queue preference also propagates to future owner buckets;
        // preserve the entire proposal stream without precreating any bucket.
        queue.disable_index_work_counters();
        assert!(queue.index_work_counters_disabled_and_zero());
        // Deliberately discard historical records/replay contexts, NOT queue
        // authority. This scratch State must never become a resume checkpoint.
        let mut state = State::new(queue, 0, None);
        let budget = WorkerBudget {
            requested: 25,
            inspection: 0,
            helpers: 24,
            coordinator: 1,
        };
        state.admission = Metrics::new(budget);
        let engine = match mode {
            "default" => Engine::new(budget),
            "min8" => Engine::new_with_min_task_len(budget, NonZeroUsize::new(8).unwrap()),
            _ => unreachable!(),
        }
        .unwrap();
        assert_eq!(engine.pool.as_ref().unwrap().current_num_threads(), 24);
        let events: Vec<_> = proposals
            .iter()
            .cloned()
            .map(|domain| {
                Event::one(Effect::Admit {
                    domain,
                    successor: false,
                    conditional: false,
                })
            })
            .collect();
        let mut request = OwnerDomainWalkRequest::new(crate::OwnerDomainMatchRequest::new(
            String::new(),
            String::new(),
        ));
        request.disable_work_limits();
        let cancellation = AtomicBool::new(false);
        let producer_stop = AtomicBool::new(false);
        let prepared_seconds = preparation.elapsed().as_secs_f64();
        let mut batches = Vec::with_capacity(proposals.len().div_ceil(BATCH_RECORDS));
        eprintln!(
            "retained admission {mode} starts: {} proposals, 24 helpers",
            proposals.len()
        );
        let before_cpu = cpu_ticks();
        let started = Instant::now();
        engine
            .commit_chunk(
                &mut state,
                &request,
                events,
                &cancellation,
                &producer_stop,
                &mut |state| batches.push(counts(&state.queue)),
            )
            .unwrap();
        let wall_seconds = started.elapsed().as_secs_f64();
        let ticks = cpu_ticks().checked_sub(before_cpu).unwrap();
        drop(engine); // Joining idle pool teardown is outside the timed span.
        assert_eq!(state.events, proposals.len());
        assert_eq!(state.successors, 0); // Synthetic admission flags, not native counters.
        assert_eq!(state.conditional, 0);
        assert_eq!(state.error, None);
        assert!(state.records.is_empty() && state.details.is_empty());
        state
            .queue
            .delegation
            .as_ref()
            .unwrap()
            .validate_checkpoint()
            .unwrap();
        assert!(state.queue.index_work_counters_disabled_and_zero());
        let metrics = state.admission.metrics_json(false);
        assert_eq!(metrics["counter_saturated"], false);
        assert_eq!(
            metrics["parallel_batches"].as_u64().unwrap() as usize,
            proposals.len().div_ceil(BATCH_RECORDS)
        );
        let final_counts = counts(&state.queue);
        let mut semantic_metrics = metrics.clone();
        for key in ["preparation_wall_seconds", "ordered_commit_wall_seconds"] {
            semantic_metrics.as_object_mut().unwrap().remove(key);
        }
        let semantic = json!({"final_queue_canonical_blake3":digest(&canonical_queue(&state.queue)),
            "final_counts":final_counts,"batch_counts":batches,"admission":semantic_metrics,
            "accepted_synthetic_events":state.events});
        if let Some(previous) = &reference {
            assert_eq!(previous, &semantic);
        } else {
            reference = Some(semantic.clone());
        }
        results.push(json!({"mode":mode,"prepared_seconds":prepared_seconds,
            "admission_loop_wall_seconds":wall_seconds,"process_cpu_ticks":ticks,
            "process_cpu_seconds":ticks as f64 / hz as f64,
            "index_work_counters_disabled_and_zero":true,
            "admission":metrics,"semantic":semantic}));
        eprintln!("retained admission {mode} complete: {wall_seconds:.6}s");
    }
    save(
        &output,
        &json!({"complete":true,"order":order,"helpers":24,"coordinator":1,
        "setup_seconds":setup_seconds,"clock_ticks_per_second":hz,"fixture":fixture,
        "results":results,"same_pair_semantic_receipts":true,
        "scope":"insertion-only replay on frozen gen5 lifecycle; not original native callback replay",
        "closure_claim":false,"production_recommendation":false}),
    );
}

#[test]
fn grain_replay_canonicalization_preserves_domain_order_and_geometry() {
    let mut queue = Queue::<1>::new(100, None);
    for value in [0, 2] {
        queue
            .admit(Domain {
                phase: super::super::super::queue::Phase::Apply,
                owner: [true],
                lower: vec![value],
                upper: vec![Some(value)],
                rank: None,
                powers: Default::default(),
            })
            .unwrap();
    }
    let image = canonical_queue(&queue);
    let restored: Queue<1> = serde_json::from_value(image.clone()).unwrap();
    assert_eq!(canonical_queue(&restored), image);
    let mut changed = image.clone();
    changed[1].as_array_mut().unwrap().swap(0, 1);
    assert_ne!(digest(&image), digest(&changed));
}
