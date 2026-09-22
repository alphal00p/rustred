//! Ignored index-only replay of completed domain descriptors, not a solver or
//! a replay of every original admission request. Invoke with --ignored
//! --nocapture --test-threads=1 and RUSTRED_DOMAIN_REPLAY_INPUT=/path/result.json.
//! RUSTRED_DOMAIN_REPLAY_PREFIX defaults to 100000 retained records. Optional
//! RUSTRED_DOMAIN_REPLAY_ORDER is raw-first (default) or semantic-first.
use super::*;
use serde::de::{DeserializeSeed, IgnoredAny, MapAccess, SeqAccess, Visitor};
use serde::{Deserialize, Deserializer};
use serde_json::{Value, json};
use std::fmt;
use std::fs::File;
use std::io::BufReader;
use std::time::Instant;

#[derive(Default, Deserialize)]
#[serde(deny_unknown_fields)]
struct Powers {
    max_positive_power: Option<u64>,
    min_power_difference: Option<i64>,
    max_power_difference: Option<i64>,
}

#[derive(Deserialize)]
struct Record {
    id: Option<usize>,
    phase: String,
    owner: String,
    lower: Vec<u64>,
    upper: Vec<Option<u64>>,
    rank: Option<u32>,
    #[serde(default)]
    power_bounds: Powers,
    local_inspection_finished: Option<bool>,
}

#[derive(Default)]
struct Input {
    records: Vec<Record>,
    total_records: usize,
    skipped_incomplete: usize,
    skipped_after_prefix: usize,
    retained_without_completion_flag: usize,
}

/// Stream the JSON document, discarding unrelated fields and the unretained
/// tail. A small prefix does not materialize the full campaign result Value.
struct InputSeed {
    prefix: usize,
}
impl<'de> DeserializeSeed<'de> for InputSeed {
    type Value = Input;
    fn deserialize<D: Deserializer<'de>>(self, deserializer: D) -> Result<Input, D::Error> {
        deserializer.deserialize_any(self)
    }
}
impl<'de> Visitor<'de> for InputSeed {
    type Value = Input;
    fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "a campaign result with domains[] or an array of domain descriptors"
        )
    }
    fn visit_seq<A: SeqAccess<'de>>(self, seq: A) -> Result<Input, A::Error> {
        read_records(seq, self.prefix)
    }
    fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<Input, A::Error> {
        let mut input = None;
        while let Some(key) = map.next_key::<String>()? {
            if key == "domains" {
                if input.is_some() {
                    return Err(serde::de::Error::custom("duplicate domains field"));
                }
                input = Some(map.next_value_seed(RecordsSeed {
                    prefix: self.prefix,
                })?);
            } else {
                map.next_value::<IgnoredAny>()?;
            }
        }
        input.ok_or_else(|| serde::de::Error::custom("missing domains array"))
    }
}
struct RecordsSeed {
    prefix: usize,
}
impl<'de> DeserializeSeed<'de> for RecordsSeed {
    type Value = Input;
    fn deserialize<D: Deserializer<'de>>(self, deserializer: D) -> Result<Input, D::Error> {
        deserializer.deserialize_seq(InputSeed {
            prefix: self.prefix,
        })
    }
}
fn read_records<'de, A: SeqAccess<'de>>(mut seq: A, prefix: usize) -> Result<Input, A::Error> {
    let mut input = Input::default();
    loop {
        if input.records.len() == prefix {
            if seq.next_element::<IgnoredAny>()?.is_none() {
                break;
            }
            input.skipped_after_prefix += 1;
        } else {
            let Some(record) = seq.next_element::<Record>()? else {
                break;
            };
            if record.local_inspection_finished == Some(false) {
                input.skipped_incomplete += 1;
            } else {
                input.retained_without_completion_flag +=
                    usize::from(record.local_inspection_finished.is_none());
                input.records.push(record);
            }
        }
        input.total_records += 1;
    }
    Ok(input)
}

/// Test-only historical raw maximal-candidate index, not the capped full-scan
/// lane. Keep the former exact and orthant shortcuts, stable forward order and
/// raw reverse retirement. Retirement does not remove any admitted descriptor.
struct RawIndex<const N: usize> {
    domains: Vec<Arc<Domain<N>>>,
    exact: HashMap<Arc<Domain<N>>, usize>,
    buckets: HashMap<(Phase, [bool; N]), OwnerBucket>,
    comparisons: usize,
    maintenance: usize,
    retired: usize,
    exact_hits: usize,
    orthant_hits: usize,
}
impl<const N: usize> RawIndex<N> {
    fn new() -> Self {
        Self {
            domains: Vec::new(),
            exact: HashMap::new(),
            buckets: HashMap::new(),
            comparisons: 0,
            maintenance: 0,
            retired: 0,
            exact_hits: 0,
            orthant_hits: 0,
        }
    }
    fn admit(&mut self, domain: Domain<N>) -> (usize, bool) {
        if let Some(&id) = self.exact.get(&domain) {
            self.exact_hits += 1;
            return (id, false);
        }
        let key = (domain.phase, domain.owner);
        if let Some(bucket) = self.buckets.get(&key) {
            if let Some(id) = bucket.orthant
                && rank_contains(self.domains[id].rank, domain.rank)
            {
                self.orthant_hits += 1;
                return (id, false);
            }
            for &id in &bucket.ids {
                self.comparisons += 1;
                if self.domains[id].contains(&domain) {
                    return (id, false);
                }
            }
        }
        let id = self.domains.len();
        let domain = Arc::new(domain);
        let bucket = self.buckets.entry(key).or_default();
        let before = bucket.ids.len();
        self.comparisons += before;
        self.maintenance += before;
        bucket
            .ids
            .retain(|&old| !domain.contains(&self.domains[old]));
        self.retired += before - bucket.ids.len();
        bucket.ids.push(id);
        if domain.is_full_orthant()
            && bucket
                .orthant
                .is_none_or(|old| rank_contains(domain.rank, self.domains[old].rank))
        {
            bucket.orthant = Some(id);
        }
        self.exact.insert(Arc::clone(&domain), id);
        self.domains.push(domain);
        (id, true)
    }
}

/// Linux scheduler's per-thread accumulated runtime. Null on unsupported
/// systems; never substitute wall time or claim process-wide CPU accounting.
fn thread_cpu_ns() -> Option<u64> {
    std::fs::read_to_string("/proc/thread-self/schedstat")
        .ok()?
        .split_whitespace()
        .next()?
        .parse()
        .ok()
}
struct Timer {
    cpu: Option<u64>,
    wall: Instant,
}
impl Timer {
    fn start() -> Self {
        Self {
            cpu: thread_cpu_ns(),
            wall: Instant::now(),
        }
    }
    fn finish(self) -> Value {
        let wall_seconds = self.wall.elapsed().as_secs_f64();
        let cpu_seconds = self
            .cpu
            .zip(thread_cpu_ns())
            .and_then(|(before, after)| after.checked_sub(before))
            .map(|ns| ns as f64 / 1e9);
        json!({"wall_seconds":wall_seconds, "thread_cpu_seconds":cpu_seconds})
    }
}

fn raw_replay<const N: usize>(domains: &[Domain<N>]) -> (RawIndex<N>, Vec<(usize, bool)>, Value) {
    let mut index = RawIndex::new();
    let mut outcomes = Vec::with_capacity(domains.len());
    let timer = Timer::start();
    for domain in domains {
        outcomes.push(index.admit(domain.clone()));
    }
    let timing = timer.finish();
    (index, outcomes, timing)
}
fn semantic_replay<const N: usize>(domains: &[Domain<N>]) -> (Queue<N>, Vec<(usize, bool)>, Value) {
    let mut queue = Queue::new(domains.len(), None);
    let mut outcomes = Vec::with_capacity(domains.len());
    let timer = Timer::start();
    for domain in domains {
        outcomes.push(
            queue
                .admit(domain.clone())
                .expect("validated replay admission"),
        );
    }
    let timing = timer.finish();
    (queue, outcomes, timing)
}

fn run<const N: usize>(input: &Input, semantic_first: bool) -> Value {
    let decode_timer = Timer::start();
    let domains: Vec<_> = input
        .records
        .iter()
        .map(|record| {
            let phase = match record.phase.as_str() {
                "Apply" | "apply" => Phase::Apply,
                "Route" | "route" => Phase::Route,
                value => panic!("invalid replay phase {value}"),
            };
            assert_eq!(record.owner.len(), N, "mixed or invalid owner arity");
            let owner: [bool; N] = std::array::from_fn(|axis| match record.owner.as_bytes()[axis] {
                b'0' => false,
                b'1' => true,
                _ => panic!("owner mask must be binary"),
            });
            assert_eq!(record.lower.len(), N);
            assert_eq!(record.upper.len(), N);
            Domain {
                phase,
                owner,
                lower: record.lower.clone(),
                upper: record.upper.clone(),
                rank: record.rank,
                powers: DomainPowerBounds {
                    max_positive_power: record.power_bounds.max_positive_power,
                    min_power_difference: record.power_bounds.min_power_difference,
                    max_power_difference: record.power_bounds.max_power_difference,
                },
            }
        })
        .collect();
    let decode_timing = decode_timer.finish();
    let (raw, raw_outcomes, raw_timing, queue, outcomes, semantic_timing) = if semantic_first {
        let (queue, outcomes, semantic_timing) = semantic_replay(&domains);
        let (raw, raw_outcomes, raw_timing) = raw_replay(&domains);
        (
            raw,
            raw_outcomes,
            raw_timing,
            queue,
            outcomes,
            semantic_timing,
        )
    } else {
        let (raw, raw_outcomes, raw_timing) = raw_replay(&domains);
        let (queue, outcomes, semantic_timing) = semantic_replay(&domains);
        (
            raw,
            raw_outcomes,
            raw_timing,
            queue,
            outcomes,
            semantic_timing,
        )
    };
    // Verification is deliberately OUTSIDE both measured admission loops.
    let verification_timer = Timer::start();
    for ((original, &(id, admitted)), &(raw_id, raw_admitted)) in
        domains.iter().zip(&outcomes).zip(&raw_outcomes)
    {
        let summary = DomainPowerSummary::try_new(
            original.owner,
            &original.lower,
            &original.upper,
            original.rank,
            original.powers,
        )
        .unwrap();
        assert_eq!(queue.domains[id].owner, original.owner);
        assert_eq!(queue.domains[id].phase, original.phase);
        assert!(
            queue.summaries[id].contains(&summary),
            "returned semantic ID does not contain request"
        );
        assert!(raw.domains[raw_id].contains(original));
        if admitted {
            assert_eq!(queue.domains[id].as_ref(), original);
        }
        if raw_admitted {
            assert_eq!(raw.domains[raw_id].as_ref(), original);
        }
    }
    assert_eq!(
        queue.next, 0,
        "index retirement must not process pending work"
    );
    assert_eq!(queue.domains.len(), queue.exact.len());
    assert_eq!(queue.domains.len(), queue.summaries.len());
    assert_eq!(raw.domains.len(), raw.exact.len());
    for (id, domain) in queue.domains.iter().enumerate() {
        assert_eq!(queue.exact.get(domain), Some(&id));
    }
    for (id, domain) in raw.domains.iter().enumerate() {
        assert_eq!(raw.exact.get(domain), Some(&id));
    }
    let verification_timing = verification_timer.finish();
    json!({"arity":N, "decode_to_domains":decode_timing, "verification":verification_timing,
        "raw_maximal_index":{"timing":raw_timing,"admissions":raw.domains.len(),
            "candidates":raw.domains.len()-raw.retired,"comparisons":raw.comparisons,
            "maintenance_comparisons":raw.maintenance,"retirements":raw.retired,
            "exact_hits":raw.exact_hits,"orthant_hits":raw.orthant_hits},
        "semantic_maximal_index":{"timing":semantic_timing,"admissions":queue.domains.len(),
            "candidates":queue.containment_candidate_count(),"comparisons":queue.containment_checks,
            "maintenance_comparisons":queue.containment_maintenance_checks,
            "retirements":queue.containment_retired_candidates,"summary_builds":queue.containment_summary_builds,
            "new_semantic_hits":queue.containment_semantic_hits,"new_semantic_retirements":queue.containment_semantic_retirements,
            "exact_hits":queue.exact_hits,"orthant_hits":queue.orthant_hits},
        "semantic_containment_verified":true,"raw_originals_retained":true,"pending_next":queue.next})
}

#[test]
fn replay_streaming_prefix_preserves_order_and_accounts_for_skipped_records() {
    let record = |id, finished| {
        json!({"id":id,"phase":"Apply","owner":"1",
        "lower":[0],"upper":[null],"rank":null,"local_inspection_finished":finished})
    };
    let document = json!({"unrelated":{"large_field":[1,2,3]},"domains":[
        record(0, json!(false)),record(1, json!(true)),record(2, Value::Null),null]});
    let encoded = document.to_string();
    let mut deserializer = serde_json::Deserializer::from_str(&encoded);
    let input = InputSeed { prefix: 2 }
        .deserialize(&mut deserializer)
        .unwrap();
    deserializer.end().unwrap();
    assert_eq!(
        input.records.iter().map(|r| r.id).collect::<Vec<_>>(),
        [Some(1), Some(2)]
    );
    assert_eq!(input.total_records, 4);
    assert_eq!(input.skipped_incomplete, 1);
    assert_eq!(input.skipped_after_prefix, 1);
    assert_eq!(input.retained_without_completion_flag, 1);
}

#[test]
fn replay_accepts_direct_arrays_and_refuses_ambiguous_document_fields() {
    let mut direct = serde_json::Deserializer::from_str("[]");
    let input = InputSeed { prefix: 1 }.deserialize(&mut direct).unwrap();
    assert!(input.records.is_empty());
    let mut duplicate = serde_json::Deserializer::from_str(r#"{"domains":[],"domains":[]}"#);
    assert!(InputSeed { prefix: 1 }.deserialize(&mut duplicate).is_err());
    let mut missing = serde_json::Deserializer::from_str(r#"{"not_domains":[]}"#);
    assert!(InputSeed { prefix: 1 }.deserialize(&mut missing).is_err());
}

#[test]
#[ignore = "input-driven index benchmark; requires RUSTRED_DOMAIN_REPLAY_INPUT and release build"]
fn replay_committed_domain_admissions() {
    assert!(
        !cfg!(debug_assertions),
        "admission timing requires a release test binary"
    );
    let path =
        std::env::var_os("RUSTRED_DOMAIN_REPLAY_INPUT").expect("set RUSTRED_DOMAIN_REPLAY_INPUT");
    let prefix = std::env::var("RUSTRED_DOMAIN_REPLAY_PREFIX")
        .map(|s| s.parse::<usize>().unwrap())
        .unwrap_or(100_000);
    assert!(prefix > 0, "replay prefix must be positive");
    let order = std::env::var("RUSTRED_DOMAIN_REPLAY_ORDER").unwrap_or_else(|_| "raw-first".into());
    assert!(matches!(order.as_str(), "raw-first" | "semantic-first"));
    let parse_timer = Timer::start();
    let file = File::open(&path).unwrap();
    let input_bytes = file.metadata().unwrap().len();
    let mut deserializer = serde_json::Deserializer::from_reader(BufReader::new(file));
    let input = InputSeed { prefix }.deserialize(&mut deserializer).unwrap();
    deserializer.end().unwrap();
    let parse_timing = parse_timer.finish();
    let first = input
        .records
        .first()
        .expect("no completed domain descriptors in input");
    let arity = first.owner.len();
    macro_rules! dispatch { ($($n:literal),*) => { match arity {
        $($n => run::<$n>(&input, order == "semantic-first"),)*
        _ => panic!("unsupported replay arity {arity}"),
    }} }
    let lanes = dispatch!(1, 2, 3, 6, 10, 15, 21);
    println!(
        "{}",
        json!({"event":"domain_admission_replay", "schema":"rustred-domain-admission-replay-v1",
        "input":path.to_string_lossy(),"input_bytes":input_bytes,"prefix_limit":prefix,
        "records_in_document":input.total_records,"replayed_descriptors":input.records.len(),
        "skipped_incomplete_before_prefix":input.skipped_incomplete,"skipped_tail_records":input.skipped_after_prefix,
        "retained_without_completion_flag":input.retained_without_completion_flag,
        "first_record_id":first.id,"last_record_id":input.records.last().and_then(|r| r.id),
        "parse":parse_timing,"order":order,"lanes":lanes,
        "workload":"completed_descriptor_prefix_not_all_original_admission_requests",
        "timing_boundary":"owned_descriptor_clone_and_index_admission; excludes parsing, type decoding and post-run verification",
        "cpu_source":"optional_linux_thread_self_schedstat_runtime_ns_not_process_cpu",
        "end_to_end_speedup_claim":false,"family_closure_claim":false})
    );
}
