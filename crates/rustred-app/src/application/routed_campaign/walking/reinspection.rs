//! W0.3 native re-inspection harness (test-only).
//!
//! A *fixture* (JSON, written by `checkpoint/reinspection_tests.rs` from a
//! restored CP5 checkpoint) holds the walk argv, the owner payload digests
//! the checkpoint bound, the initial admission prefix and a list of domains
//! to inspect. The initial prefix is the only queue state a native inspection
//! reads: `InitialOrthants` and `InitialOverlapIndex` are built from it with
//! the calls `execution::run_configured` makes. `reinspect_fixture` rebuilds
//! the reducer from the argv's inputs (refusing any owner digest change) and
//! runs `inspection::inspect`, the walk's own entry point, on K threads with
//! a sink that accepts every event (it never stops a stream):
//!
//! - `count`: per-effect counters only (the measurement sink);
//! - `digest`: [`StreamDigest`] of the whole ordered event stream, of its
//!   multiset and of its successor multiset (the differential sink);
//! - `dump`: every event in the stream format of `STREAMS.md`.
//!
//! The differential proof is `tap_control_walk` + `reinspect_fixture` with
//! `RUSTRED_HARNESS_TAP`: the tap records the digest of every stream the
//! *real* walk's inspections offered to the walk's own emit callback, keyed by
//! the domain's bincode digest; the harness must reproduce, for every
//! natively inspected ID of the finished run, the digest of its completed
//! attempt (ordered stream, multiset, successor multiset, counters and native
//! stats).
//!
//! ```text
//! RUSTRED_HARNESS_FIXTURE=<fixture.json> RUSTRED_HARNESS_OUT=<new dir> \
//! [RUSTRED_HARNESS_THREADS=K] [RUSTRED_HARNESS_SINK=count|digest|dump] \
//! [RUSTRED_HARNESS_ORDER=fixture|shuffle:<seed>|cost:<natives.jsonl>] \
//! [RUSTRED_HARNESS_SUBSET=<r>/<m>] [RUSTRED_HARNESS_LIMIT=<n>] \
//! [RUSTRED_HARNESS_PASSES=<p>] [RUSTRED_HARNESS_PIN=1] [RUSTRED_HARNESS_REPLICAS=<G>] \
//! [RUSTRED_HARNESS_REPLICA_HOME=<cpu>] \
//! [RUSTRED_HARNESS_CANCEL_FRACTION=<f> (needs ORDER=cost:...)] \
//! [RUSTRED_HARNESS_TAP=<tap.jsonl> (needs SINK=digest)] \
//! [RUSTRED_HARNESS_{MANIFEST,OWNER_BASE,QUERIES}=<identical copies>] \
//! RAYON_NUM_THREADS=1 cargo test --release -p rustred-app --lib \
//!     reinspect_fixture -- --ignored --nocapture
//! ```
//!
//! Scope: native inspection CPU only. No admission, no queue, no publication;
//! the sink's own cost is the counters (or the digest/dump in those modes).
use super::super::{RoutedCampaignRequest, input, prepare};
use super::{
    OwnerDomainWalkRequest,
    execution::native_stats,
    initial_orthants::InitialOrthants,
    initial_overlap::InitialOverlapIndex,
    inspection::{self, Effect, Event, Finished},
    limits_json, mask, power_bounds_json,
    queue::{Domain, Phase},
};
use rustred::solver::RoutedCandidateReducer;
use serde_json::{Value, json};
use std::collections::HashMap;
use std::ffi::OsString;
use std::io::{BufRead, BufWriter, Write};
use std::ops::ControlFlow;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};
use std::time::{Instant, SystemTime, UNIX_EPOCH};

pub(super) const FIXTURE_SCHEMA: &str = "rustred.reinspection-fixture.v1";
const STREAM_MAGIC: &[u8; 8] = b"RRSTRM01";
const BLOCK_MAGIC: u32 = u32::from_le_bytes(*b"NATV");

/// Effect tags of the canonical event encoding (`STREAMS.md`).
pub(super) const TAGS: [&str; 6] = [
    "count",
    "known_reuse",
    "pre_admitted_orthant_reuse",
    "admit",
    "frontier",
    "optional",
];

fn bincode_domain<const N: usize>(domain: &Domain<N>) -> Vec<u8> {
    // The CP5 domain-segment record encoding (sections.rs uses the same
    // standard configuration; its byte limit does not change the bytes).
    bincode::serde::encode_to_vec(domain, bincode::config::standard())
        .expect("domain bincode encoding")
}

/// Identity of a domain across processes: blake3 of its CP5 record bytes.
pub(super) fn domain_key<const N: usize>(domain: &Domain<N>) -> String {
    blake3::hash(&bincode_domain(domain)).to_hex().to_string()
}

fn phase_name(phase: Phase) -> &'static str {
    match phase {
        Phase::Apply => "Apply",
        Phase::Route => "Route",
    }
}

/// Canonical bytes of one event; returns its tag. Layout in `STREAMS.md`.
pub(super) fn encode_event<const N: usize>(event: &Event<N>, out: &mut Vec<u8>) -> u8 {
    let (tag, successor, conditional) = match &event.effect {
        Effect::Count => (0u8, false, false),
        Effect::KnownReuse {
            successor,
            conditional,
        } => (1, *successor, *conditional),
        Effect::PreAdmittedOrthantReuse {
            successor,
            conditional,
            ..
        } => (2, *successor, *conditional),
        Effect::Admit {
            successor,
            conditional,
            ..
        } => (3, *successor, *conditional),
        Effect::Frontier {
            successor,
            conditional,
            ..
        } => (4, *successor, *conditional),
        Effect::Optional(_) => (5, false, false),
    };
    out.push(tag);
    out.extend_from_slice(
        &u32::try_from(event.count)
            .expect("event count fits u32")
            .to_le_bytes(),
    );
    out.push(u8::from(successor) | u8::from(conditional) << 1);
    let mut with_length = |bytes: &[u8]| {
        out.extend_from_slice(
            &u32::try_from(bytes.len())
                .expect("payload fits u32")
                .to_le_bytes(),
        );
        out.extend_from_slice(bytes);
    };
    match &event.effect {
        Effect::PreAdmittedOrthantReuse { target, .. } => {
            with_length(&u64::try_from(*target).expect("u64 id").to_le_bytes())
        }
        Effect::Admit { domain, .. } => with_length(&bincode_domain(domain)),
        Effect::Frontier { value, .. } => {
            with_length(&serde_json::to_vec(value).expect("frontier JSON"))
        }
        Effect::Optional(d) => with_length(
            &serde_json::to_vec(&json!({"disposition":inspection::debug(&d.disposition),
                "rank":d.rank,"power_bounds":power_bounds_json(d.powers),"lower":d.lower,
                "upper":d.upper,"shift":d.shift,"ordinal":d.ordinal,"resource":d.resource,
                "requested":d.requested,"limit":d.limit}))
            .expect("optional JSON"),
        ),
        Effect::Count | Effect::KnownReuse { .. } => {}
    }
    tag
}

fn successor_flags<const N: usize>(event: &Event<N>) -> (bool, bool) {
    match &event.effect {
        Effect::KnownReuse {
            successor,
            conditional,
        }
        | Effect::PreAdmittedOrthantReuse {
            successor,
            conditional,
            ..
        }
        | Effect::Admit {
            successor,
            conditional,
            ..
        }
        | Effect::Frontier {
            successor,
            conditional,
            ..
        } => (*successor, *conditional),
        Effect::Count | Effect::Optional(_) => (false, false),
    }
}

/// Per-effect counters: `records` counts `Event`s, `weighted` sums their
/// `count` (compacted callback runs), `successors`/`conditional` sum the
/// counts of events carrying those flags.
#[derive(Default, Clone, Copy)]
pub(super) struct Counts {
    records: [u64; 6],
    weighted: [u64; 6],
    successors: u64,
    conditional: u64,
    admit_apply: u64,
    admit_route: u64,
}
impl Counts {
    #[inline]
    fn push<const N: usize>(&mut self, event: &Event<N>) {
        let tag = match &event.effect {
            Effect::Count => 0,
            Effect::KnownReuse { .. } => 1,
            Effect::PreAdmittedOrthantReuse { .. } => 2,
            Effect::Admit { domain, .. } => {
                match domain.phase {
                    Phase::Apply => self.admit_apply += 1,
                    Phase::Route => self.admit_route += 1,
                }
                3
            }
            Effect::Frontier { .. } => 4,
            Effect::Optional(_) => 5,
        };
        let count = event.count as u64;
        self.records[tag] += 1;
        self.weighted[tag] += count;
        let (successor, conditional) = successor_flags(event);
        self.successors += u64::from(successor) * count;
        self.conditional += u64::from(conditional) * count;
    }
    fn json(&self) -> Value {
        let named = |values: &[u64; 6]| {
            TAGS.iter()
                .zip(values)
                .map(|(k, v)| (k.to_string(), json!(v)))
                .collect::<serde_json::Map<_, _>>()
        };
        json!({"records":named(&self.records),"weighted":named(&self.weighted),
            "successors":self.successors,"conditional":self.conditional,
            "admit_apply":self.admit_apply,"admit_route":self.admit_route,
            "events":self.records.iter().sum::<u64>()})
    }
}

/// Digest of a stream: blake3 over the length-prefixed canonical events in
/// order, and order-free multiset digests (wrapping sums of the first 16
/// bytes of each event's blake3) over all events and over successor events.
pub(super) struct StreamDigest {
    ordered: blake3::Hasher,
    multiset: u128,
    successor_multiset: u128,
    counts: Counts,
    buffer: Vec<u8>,
}
impl Default for StreamDigest {
    fn default() -> Self {
        Self {
            ordered: blake3::Hasher::new(),
            multiset: 0,
            successor_multiset: 0,
            counts: Counts::default(),
            buffer: Vec::new(),
        }
    }
}
impl StreamDigest {
    pub fn push<const N: usize>(&mut self, event: &Event<N>) {
        self.buffer.clear();
        encode_event(event, &mut self.buffer);
        self.ordered
            .update(&(self.buffer.len() as u32).to_le_bytes());
        self.ordered.update(&self.buffer);
        let hash = blake3::hash(&self.buffer);
        let value = u128::from_le_bytes(hash.as_bytes()[..16].try_into().expect("16 bytes"));
        self.multiset = self.multiset.wrapping_add(value);
        if successor_flags(event).0 {
            self.successor_multiset = self.successor_multiset.wrapping_add(value);
        }
        self.counts.push(event);
    }
    pub fn json(&self) -> Value {
        json!({"ordered":self.ordered.finalize().to_hex().to_string(),
            "multiset":format!("{:032x}", self.multiset),
            "successor_multiset":format!("{:032x}", self.successor_multiset),
            "counts":self.counts.json()})
    }
}

/// The real walk's streams, recorded around `inspection::inspect` while
/// enabled (the ignored `tap_control_walk` test only).
pub(super) mod tap {
    use super::*;
    static ENABLED: AtomicBool = AtomicBool::new(false);
    static ATTEMPTS: Mutex<Vec<Value>> = Mutex::new(Vec::new());

    pub fn enabled() -> bool {
        ENABLED.load(Ordering::Acquire)
    }
    pub fn set(on: bool) {
        ENABLED.store(on, Ordering::Release);
    }
    pub fn take() -> Vec<Value> {
        std::mem::take(&mut *ATTEMPTS.lock().expect("tap lock"))
    }
    /// Digest every event `inner` offers to the walk's `emit`, in order,
    /// whether or not `emit` accepts it (a Break is recorded).
    pub fn record<const N: usize>(
        domain: &Domain<N>,
        emit: &mut (impl FnMut(Event<N>) -> ControlFlow<()> + ?Sized),
        inner: impl FnOnce(&mut dyn FnMut(Event<N>) -> ControlFlow<()>) -> Finished,
    ) -> Finished {
        let mut digest = StreamDigest::default();
        let mut consumer_break = false;
        let finished = inner(&mut |event: Event<N>| {
            digest.push(&event);
            let flow = emit(event);
            consumer_break |= flow.is_break();
            flow
        });
        let attempt = json!({"key":domain_key(domain),"phase":phase_name(domain.phase),
            "owner":mask(&domain.owner),"error_kind":finished.error_kind,
            "consumer_break":consumer_break,"digest":digest.json(),
            "stats":native_stats(finished.stats)});
        ATTEMPTS.lock().expect("tap lock").push(attempt);
        finished
    }
}

// ---- fixture ---------------------------------------------------------------

/// The request fields a native inspection reads, plus input digests.
pub(super) fn inspection_request(request: &OwnerDomainWalkRequest) -> Value {
    json!({"limits":limits_json(request),
        "reuse_initial_d_bands":request.reuse_initial_d_bands,
        "route_domain_overcover":request.route_domain_overcover,
        "max_route_masks":request.max_route_masks,
        "route_joint_source_support_pruning":request.route_joint_source_support_pruning,
        "reduction_limits":inspection::debug(&request.matching.reduction_limits),
        "selection_blake3":blake3::hash(request.matching.selection_json.as_bytes()).to_hex().to_string()})
}

/// Checkpoint options are transport for a resume, not inspection inputs.
fn strip_checkpoint(argv: &[String], overrides: &[(&str, PathBuf)]) -> Vec<OsString> {
    let mut out = Vec::with_capacity(argv.len());
    let mut arguments = argv.iter();
    while let Some(argument) = arguments.next() {
        match argument.as_str() {
            "--resume" | "--checkpoint" | "--checkpoint-interval-seconds" => {
                arguments.next();
            }
            other => {
                if let Some((_, path)) = overrides.iter().find(|(o, _)| *o == other) {
                    arguments.next();
                    out.push(OsString::from(other));
                    out.push(path.clone().into_os_string());
                } else {
                    out.push(OsString::from(other));
                }
            }
        }
    }
    out
}

fn input_overrides() -> Vec<(&'static str, PathBuf)> {
    [
        ("--manifest", "RUSTRED_HARNESS_MANIFEST"),
        ("--owner-base", "RUSTRED_HARNESS_OWNER_BASE"),
        ("--queries", "RUSTRED_HARNESS_QUERIES"),
    ]
    .into_iter()
    .filter_map(|(option, variable)| std::env::var_os(variable).map(|p| (option, p.into())))
    .collect()
}

struct Job<const N: usize> {
    index: usize,
    id: u64,
    stratum: String,
    key: String,
    domain: Domain<N>,
}

fn env(name: &str) -> Option<String> {
    std::env::var(name).ok().filter(|v| !v.is_empty())
}

fn now_unix() -> f64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock")
        .as_secs_f64()
}

fn thread_cpu_ns() -> u64 {
    let mut ts = libc::timespec {
        tv_sec: 0,
        tv_nsec: 0,
    };
    // SAFETY: valid out pointer; the clock id is a constant.
    let rc = unsafe { libc::clock_gettime(libc::CLOCK_THREAD_CPUTIME_ID, &mut ts) };
    assert_eq!(rc, 0, "clock_gettime(CLOCK_THREAD_CPUTIME_ID)");
    ts.tv_sec as u64 * 1_000_000_000 + ts.tv_nsec as u64
}

/// (voluntary switches, involuntary switches, minor faults, major faults)
fn thread_rusage() -> [u64; 4] {
    // SAFETY: zeroed rusage is a valid out value.
    let mut ru: libc::rusage = unsafe { std::mem::zeroed() };
    // SAFETY: valid out pointer.
    let rc = unsafe { libc::getrusage(libc::RUSAGE_THREAD, &mut ru) };
    assert_eq!(rc, 0, "getrusage(RUSAGE_THREAD)");
    [
        ru.ru_nvcsw as u64,
        ru.ru_nivcsw as u64,
        ru.ru_minflt as u64,
        ru.ru_majflt as u64,
    ]
}

fn process_rusage() -> Value {
    // SAFETY: as above.
    let mut ru: libc::rusage = unsafe { std::mem::zeroed() };
    // SAFETY: valid out pointer.
    let rc = unsafe { libc::getrusage(libc::RUSAGE_SELF, &mut ru) };
    assert_eq!(rc, 0);
    let tv = |t: libc::timeval| t.tv_sec as f64 + t.tv_usec as f64 * 1e-6;
    json!({"user_seconds":tv(ru.ru_utime),"system_seconds":tv(ru.ru_stime),
        "voluntary_switches":ru.ru_nvcsw,"involuntary_switches":ru.ru_nivcsw,
        "minor_faults":ru.ru_minflt,"major_faults":ru.ru_majflt,"max_rss_kib":ru.ru_maxrss})
}

/// `/proc/thread-self/schedstat`: [on-CPU ns, run-queue wait ns, slices].
fn thread_schedstat() -> [u64; 3] {
    let text = std::fs::read_to_string("/proc/thread-self/schedstat").unwrap_or_default();
    let mut values = text
        .split_ascii_whitespace()
        .map(|v| v.parse::<u64>().unwrap_or(0));
    [
        values.next().unwrap_or(0),
        values.next().unwrap_or(0),
        values.next().unwrap_or(0),
    ]
}

fn affinity() -> Vec<usize> {
    // SAFETY: zeroed cpu_set_t is valid; the size is the struct's own.
    let mut set: libc::cpu_set_t = unsafe { std::mem::zeroed() };
    let rc = unsafe { libc::sched_getaffinity(0, size_of::<libc::cpu_set_t>(), &mut set) };
    assert_eq!(rc, 0, "sched_getaffinity");
    (0..libc::CPU_SETSIZE as usize)
        .filter(|&cpu| unsafe { libc::CPU_ISSET(cpu, &set) })
        .collect()
}

fn pin_to(cpu: usize) {
    // SAFETY: as in `affinity`; pid 0 is the calling thread.
    let mut set: libc::cpu_set_t = unsafe { std::mem::zeroed() };
    unsafe { libc::CPU_SET(cpu, &mut set) };
    let rc = unsafe { libc::sched_setaffinity(0, size_of::<libc::cpu_set_t>(), &set) };
    assert_eq!(rc, 0, "sched_setaffinity");
}

fn current_cpu() -> i64 {
    // SAFETY: no arguments.
    i64::from(unsafe { libc::sched_getcpu() })
}

fn status_kib(field: &str) -> Option<u64> {
    std::fs::read_to_string("/proc/self/status")
        .ok()?
        .lines()
        .find(|l| l.starts_with(field))?
        .split_ascii_whitespace()
        .nth(1)?
        .parse()
        .ok()
}

/// Prior per-native costs (`natives.jsonl` of an earlier run): max over
/// passes of cpu and wall ns, keyed by fixture index.
fn prior_costs(path: &Path) -> HashMap<usize, (u64, u64)> {
    let file = std::fs::File::open(path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
    let mut costs: HashMap<usize, (u64, u64)> = HashMap::new();
    for line in std::io::BufReader::new(file).lines() {
        let line = line.expect("costs line");
        let value: Value = serde_json::from_str(&line).expect("costs JSON");
        let index = value["i"].as_u64().expect("i") as usize;
        let cpu = value["cpu_ns"].as_u64().unwrap_or(0);
        let wall = value["wall_ns"].as_u64().unwrap_or(0);
        let entry = costs.entry(index).or_insert((0, 0));
        entry.0 = entry.0.max(cpu);
        entry.1 = entry.1.max(wall);
    }
    costs
}

fn splitmix(mut x: u64) -> u64 {
    x = x.wrapping_add(0x9E37_79B9_7F4A_7C15);
    x = (x ^ (x >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    x = (x ^ (x >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    x ^ (x >> 31)
}

/// Cancellation slots: the timer sets a worker's flag once its deadline has
/// passed and records when; the worker measures until `inspect` returns.
struct CancelSlot {
    flag: AtomicBool,
    deadline_ns: AtomicU64,
    set_ns: AtomicU64,
}

enum Sink {
    Count(Counts),
    Digest(StreamDigest),
    Dump(Counts, Vec<u8>),
}

struct StreamWriter {
    file: BufWriter<std::fs::File>,
    index: BufWriter<std::fs::File>,
    offset: u64,
    name: String,
}

fn run_fixture<const N: usize>(
    fixture: &Value,
    out: &Path,
    receipt: &mut Value,
) -> Result<(), String> {
    let started = Instant::now();
    let argv: Vec<String> = fixture["argv"]
        .as_array()
        .ok_or("fixture has no argv")?
        .iter()
        .map(|a| a.as_str().expect("argv string").to_owned())
        .collect();
    let overrides = input_overrides();
    let argv = strip_checkpoint(&argv, &overrides);
    let request = crate::cli::walk_request_from_argv(argv.clone())?;
    let observed = inspection_request(&request);
    if observed != fixture["inspection_request"] {
        return Err(format!(
            "inspection request differs from the fixture's: {observed} vs {}",
            fixture["inspection_request"]
        ));
    }
    receipt["argv"] = json!(argv.iter().map(|a| a.to_string_lossy()).collect::<Vec<_>>());
    let (selection, arity, load_limits) =
        input::Selection::parse(&request.matching.selection_json).map_err(|e| e.to_string())?;
    if arity != N {
        return Err("selection arity differs from fixture".into());
    }

    // Native owner import exactly as the walk prepares it; the payload
    // digests must equal the ones the checkpoint bound.
    let phase = Instant::now();
    let digests: Vec<String> = fixture["owner_digests"]
        .as_array()
        .ok_or("fixture has no owner digests")?
        .iter()
        .map(|d| d.as_str().expect("digest").to_owned())
        .collect();
    let mut load = RoutedCampaignRequest::new(String::new(), String::new());
    // This research harness can prepare replicas concurrently on individually
    // pinned threads below. Keep their preparation inline: a pool using the
    // saved campaign's worker count would multiply the replica budget and
    // violate the single-CPU affinity. Production cold verification instead
    // passes its own explicit verifier thread budget to the shared preparer.
    load.workers = 1;
    load.owner_base = request.matching.owner_base.clone();
    load.reduction_limits = request.matching.reduction_limits;
    let prepare_one = |never: &AtomicBool| -> Result<RoutedCandidateReducer<N>, String> {
        let mut bind = |owners: Vec<String>| {
            if owners == digests {
                Ok(())
            } else {
                Err("owner payload digests differ from the fixture's checkpoint binding".to_owned())
            }
        };
        prepare::prepare_with_fingerprints::<N>(
            &load,
            &selection,
            load_limits,
            never,
            &|_: Value| {},
            Some(&mut bind),
        )
        .map_err(|e| e.to_string())?
        .ok_or_else(|| "preparation cancelled".to_owned())
    };
    // Initial prefix -> the two read-only indexes, as run_configured builds them.
    let build_indexes =
        |never: &AtomicBool| -> Result<(InitialOrthants<N>, InitialOverlapIndex<N>), String> {
            let initial_domains: Vec<Arc<Domain<N>>> = fixture["initial_domains"]
                .as_array()
                .ok_or("fixture has no initial domains")?
                .iter()
                .map(|d| serde_json::from_value::<Domain<N>>(d.clone()).map(Arc::new))
                .collect::<Result<_, _>>()
                .map_err(|e| e.to_string())?;
            let initial_count = fixture["initial_domain_count"].as_u64().ok_or("count")? as usize;
            if initial_domains.len() < initial_count {
                return Err("fixture initial prefix is short".into());
            }
            let initial = InitialOrthants::from_initial(&initial_domains[..initial_count], never);
            let overlap = if request.reuse_initial_d_bands {
                let prefix = fixture["overlap_prefix"]
                    .as_u64()
                    .ok_or("D-band reuse needs the protected initial prefix")?
                    as usize;
                if prefix > initial_domains.len() {
                    return Err("overlap prefix exceeds the fixture's initial domains".into());
                }
                InitialOverlapIndex::from_initial(&initial_domains[..prefix], never)
            } else {
                InitialOverlapIndex::empty()
            };
            Ok((initial, overlap))
        };
    // W0.3 contention probe (RUSTRED_HARNESS_REPLICAS=G, default 1): G fully
    // independent copies of everything an inspection reads (reducer from its
    // own owner import, and the initial indexes), each prepared on a thread
    // pinned to the first CPU of its worker group (NUMA first touch); worker k
    // uses copy k*G/K. No reference-counted or otherwise written state of the
    // owner programs is then shared between groups (Symbolica polynomials
    // clone and drop the Arc of their shared PolynomialContext on every
    // zero()/clone()/unify_variables(), see the W0.3 note).
    // RUSTRED_HARNESS_REPLICA_HOME=<cpu> prepares every copy on that CPU
    // instead (placement control: copies still private to their group, but
    // first-touched on one node).
    let replica_home: Option<usize> = env("RUSTRED_HARNESS_REPLICA_HOME")
        .map(|c| c.parse().map_err(|_| "REPLICA_HOME"))
        .transpose()?;
    let replica_count: usize = env("RUSTRED_HARNESS_REPLICAS")
        .map_or(Ok(1), |p| p.parse())
        .map_err(|_| "REPLICAS")?;
    let threads_requested: usize = env("RUSTRED_HARNESS_THREADS")
        .map_or(Ok(1), |p| p.parse())
        .map_err(|_| "THREADS")?;
    if replica_count == 0 || replica_count > threads_requested.max(1) {
        return Err("REPLICAS must be in 1..=THREADS".into());
    }
    let replica_cpus = affinity();
    let replica_pin = env("RUSTRED_HARNESS_PIN").is_some_and(|v| v == "1")
        && replica_cpus.len() >= threads_requested;
    let never = AtomicBool::new(false);
    #[allow(clippy::type_complexity)]
    let replicas: Vec<(
        RoutedCandidateReducer<N>,
        InitialOrthants<N>,
        InitialOverlapIndex<N>,
    )> = if replica_count == 1 && replica_home.is_none() {
        let reducer = prepare_one(&never)?;
        let (initial, overlap) = build_indexes(&never)?;
        vec![(reducer, initial, overlap)]
    } else {
        std::thread::scope(|scope| {
            let handles: Vec<_> = (0..replica_count)
                .map(|g| {
                    let cpu = replica_home.or_else(|| {
                        replica_pin.then(|| replica_cpus[g * threads_requested / replica_count])
                    });
                    let (prepare_one, build_indexes) = (&prepare_one, &build_indexes);
                    scope.spawn(move || {
                        if let Some(cpu) = cpu {
                            pin_to(cpu);
                        }
                        let never = AtomicBool::new(false);
                        let reducer = prepare_one(&never)?;
                        let (initial, overlap) = build_indexes(&never)?;
                        Ok::<_, String>((reducer, initial, overlap))
                    })
                })
                .collect();
            handles
                .into_iter()
                .map(|h| h.join().expect("replica preparation panicked"))
                .collect::<Result<Vec<_>, String>>()
        })?
    };
    receipt["timings"]["prepare_seconds"] = json!(phase.elapsed().as_secs_f64());
    receipt["memory"]["after_prepare_rss_kib"] = json!(status_kib("VmRSS:"));
    receipt["replicas"] = json!({"count":replica_count,
        "pinned_prepare":replica_home.is_some() || (replica_pin && replica_count > 1),
        "home_cpu":replica_home,
        "worker_to_replica":"k * replicas / threads"});
    let phase = Instant::now();
    receipt["initial_overlap_index"] = super::index_report::render(
        Some(replicas[0].2.build_report()),
        super::index_report::Scope::GlobalInitial,
    );

    // Jobs, subset, order, limit, passes.
    let mut jobs: Vec<Job<N>> = Vec::new();
    for entry in fixture["sample"]
        .as_array()
        .ok_or("fixture has no sample")?
    {
        let domain: Domain<N> =
            serde_json::from_value(entry["domain"].clone()).map_err(|e| e.to_string())?;
        let key = domain_key(&domain);
        if entry["key"].as_str() != Some(key.as_str()) {
            return Err("sample domain key mismatch".into());
        }
        jobs.push(Job {
            index: entry["i"].as_u64().ok_or("i")? as usize,
            id: entry["id"].as_u64().ok_or("id")?,
            stratum: entry["stratum"].as_str().unwrap_or("").to_owned(),
            key,
            domain,
        });
    }
    receipt["timings"]["fixture_decode_seconds"] = json!(phase.elapsed().as_secs_f64());
    if let Some(subset) = env("RUSTRED_HARNESS_SUBSET") {
        let (r, m) = subset.split_once('/').ok_or("SUBSET is r/m")?;
        let (r, m): (usize, usize) = (r.parse().map_err(|_| "r")?, m.parse().map_err(|_| "m")?);
        jobs.retain(|job| job.index % m == r);
    }
    let order = env("RUSTRED_HARNESS_ORDER").unwrap_or_else(|| "fixture".into());
    let mut costs = HashMap::new();
    if let Some(seed) = order.strip_prefix("shuffle:") {
        let seed: u64 = seed.parse().map_err(|_| "shuffle seed")?;
        jobs.sort_by_key(|job| splitmix(seed ^ job.index as u64));
    } else if let Some(path) = order.strip_prefix("cost:") {
        costs = prior_costs(Path::new(path));
        // Longest processing time first; unknown costs last in fixture order.
        jobs.sort_by_key(|job| std::cmp::Reverse(costs.get(&job.index).map_or(0, |c| c.0)));
    } else if order != "fixture" {
        return Err(format!("unknown ORDER {order}"));
    }
    if let Some(limit) = env("RUSTRED_HARNESS_LIMIT") {
        jobs.truncate(limit.parse().map_err(|_| "LIMIT")?);
    }
    let passes: usize = env("RUSTRED_HARNESS_PASSES")
        .map_or(Ok(1), |p| p.parse())
        .map_err(|_| "PASSES")?;
    let threads: usize = env("RUSTRED_HARNESS_THREADS")
        .map_or(Ok(1), |p| p.parse())
        .map_err(|_| "THREADS")?;
    let pin = env("RUSTRED_HARNESS_PIN").is_some_and(|v| v == "1");
    let sink_mode = env("RUSTRED_HARNESS_SINK").unwrap_or_else(|| "count".into());
    if !["count", "digest", "dump"].contains(&sink_mode.as_str()) {
        return Err(format!("unknown SINK {sink_mode}"));
    }
    let cancel_fraction: Option<f64> = env("RUSTRED_HARNESS_CANCEL_FRACTION")
        .map(|f| f.parse().map_err(|_| "CANCEL_FRACTION"))
        .transpose()?;
    if cancel_fraction.is_some() && costs.is_empty() {
        return Err("CANCEL_FRACTION needs ORDER=cost:<natives.jsonl>".into());
    }
    let cpus = affinity();
    if pin && cpus.len() < threads {
        return Err(format!(
            "PIN needs {threads} CPUs, affinity has {}",
            cpus.len()
        ));
    }
    receipt["run"] = json!({"threads":threads,"passes":passes,"pin":pin,"sink":sink_mode,
        "order":order,"subset":env("RUSTRED_HARNESS_SUBSET"),"limit":env("RUSTRED_HARNESS_LIMIT"),
        "jobs":jobs.len(),"cancel_fraction":cancel_fraction,"affinity":cpus,
        "rayon_num_threads":std::env::var("RAYON_NUM_THREADS").ok(),
        "ld_preload":std::env::var("LD_PRELOAD").ok(),
        "glibc_tunables":std::env::var("GLIBC_TUNABLES").ok()});
    if sink_mode == "dump" {
        std::fs::create_dir_all(out.join("streams")).map_err(|e| e.to_string())?;
    }

    // Parallel section.
    let total = jobs.len() * passes;
    let next = AtomicUsize::new(0);
    let active = AtomicUsize::new(0);
    let done = AtomicBool::new(false);
    let slots: Vec<CancelSlot> = (0..threads)
        .map(|_| CancelSlot {
            flag: AtomicBool::new(false),
            deadline_ns: AtomicU64::new(0),
            set_ns: AtomicU64::new(0),
        })
        .collect();
    let section_rusage_before = process_rusage();
    let section = Instant::now();
    let section_unix = now_unix();
    let ns = |at: Instant| at.duration_since(section).as_nanos() as u64;
    let per_thread: Vec<Value> = std::thread::scope(|scope| {
        if cancel_fraction.is_some() {
            scope.spawn(|| {
                while !done.load(Ordering::Acquire) {
                    let now = ns(Instant::now());
                    for slot in &slots {
                        let deadline = slot.deadline_ns.load(Ordering::Acquire);
                        if deadline != 0
                            && now >= deadline
                            && slot
                                .deadline_ns
                                .compare_exchange(deadline, 0, Ordering::AcqRel, Ordering::Acquire)
                                .is_ok()
                        {
                            slot.set_ns.store(ns(Instant::now()), Ordering::Release);
                            slot.flag.store(true, Ordering::Release);
                        }
                    }
                    std::thread::sleep(std::time::Duration::from_micros(20));
                }
            });
        }
        let handles: Vec<_> = (0..threads)
            .map(|k| {
                let (jobs, next, active, slots, costs) = (&jobs, &next, &active, &slots, &costs);
                let replica = &replicas[k * replicas.len() / threads];
                let (reducer, request, initial, overlap) =
                    (&replica.0, &request, &replica.1, &replica.2);
                let sink_mode = sink_mode.as_str();
                let cpu = pin.then(|| cpus[k]);
                scope.spawn(move || -> Result<Value, String> {
                    if let Some(cpu) = cpu {
                        pin_to(cpu);
                    }
                    let sched_before = thread_schedstat();
                    let rusage_before = thread_rusage();
                    let cpu_before = thread_cpu_ns();
                    let lines_path = out.join(format!("natives-t{k:03}.jsonl"));
                    let mut lines = BufWriter::new(
                        std::fs::File::create(&lines_path).map_err(|e| e.to_string())?,
                    );
                    let mut streams = if sink_mode == "dump" {
                        let name = format!("streams/part-{k:03}.bin");
                        let mut file = BufWriter::new(
                            std::fs::File::create(out.join(&name)).map_err(|e| e.to_string())?,
                        );
                        file.write_all(STREAM_MAGIC).map_err(|e| e.to_string())?;
                        file.write_all(&(N as u32).to_le_bytes())
                            .map_err(|e| e.to_string())?;
                        file.write_all(&0u32.to_le_bytes())
                            .map_err(|e| e.to_string())?;
                        let index = BufWriter::new(
                            std::fs::File::create(
                                out.join(format!("streams/part-{k:03}.index.jsonl")),
                            )
                            .map_err(|e| e.to_string())?,
                        );
                        Some(StreamWriter {
                            file,
                            index,
                            offset: 16,
                            name,
                        })
                    } else {
                        None
                    };
                    let (mut natives, mut cpu_sum, mut wall_sum) = (0u64, 0u64, 0u64);
                    loop {
                        let n = next.fetch_add(1, Ordering::AcqRel);
                        if n >= total {
                            break;
                        }
                        let job = &jobs[n % jobs.len()];
                        let pass = n / jobs.len();
                        let slot = &slots[k];
                        slot.flag.store(false, Ordering::Release);
                        slot.set_ns.store(0, Ordering::Release);
                        let mut sink: Sink = match sink_mode {
                            "digest" => Sink::Digest(StreamDigest::default()),
                            "dump" => Sink::Dump(Counts::default(), Vec::new()),
                            _ => Sink::Count(Counts::default()),
                        };
                        let active_start = active.fetch_add(1, Ordering::AcqRel) + 1;
                        let start_cpu = current_cpu();
                        let r0 = thread_rusage();
                        let c0 = thread_cpu_ns();
                        let t0 = Instant::now();
                        if let Some(fraction) = cancel_fraction
                            && let Some(&(_, wall)) = costs.get(&job.index)
                        {
                            let delay = ((wall as f64) * fraction).max(1.0) as u64;
                            slot.deadline_ns.store(ns(t0) + delay, Ordering::Release);
                        }
                        let finished = {
                            let mut emit = |event: Event<N>| {
                                match &mut sink {
                                    Sink::Count(counts) => counts.push(&event),
                                    Sink::Digest(digest) => digest.push(&event),
                                    Sink::Dump(counts, bytes) => {
                                        counts.push(&event);
                                        let at = bytes.len();
                                        bytes.extend_from_slice(&0u32.to_le_bytes());
                                        encode_event(&event, bytes);
                                        let len = (bytes.len() - at - 4) as u32;
                                        bytes[at..at + 4].copy_from_slice(&len.to_le_bytes());
                                    }
                                }
                                ControlFlow::Continue(())
                            };
                            inspection::inspect(
                                reducer,
                                &job.domain,
                                request,
                                &slot.flag,
                                initial,
                                overlap,
                                &mut emit,
                            )
                        };
                        let t1 = Instant::now();
                        let c1 = thread_cpu_ns();
                        let r1 = thread_rusage();
                        let active_end = active.fetch_sub(1, Ordering::AcqRel);
                        let cleared = slot.deadline_ns.swap(0, Ordering::AcqRel);
                        let set_ns = slot.set_ns.load(Ordering::Acquire);
                        let cancel = cancel_fraction.map(|_| {
                            if set_ns == 0 {
                                json!({"flag_set":false,"deadline_pending":cleared != 0})
                            } else {
                                json!({"flag_set":true,"latency_ns":ns(t1).saturating_sub(set_ns),
                                    "flag_at_ns":set_ns.saturating_sub(ns(t0))})
                            }
                        });
                        let (cpu_ns, wall_ns) = (c1 - c0, (t1 - t0).as_nanos() as u64);
                        natives += 1;
                        cpu_sum += cpu_ns;
                        wall_sum += wall_ns;
                        let (counts, digest) = match &sink {
                            Sink::Count(c) | Sink::Dump(c, _) => (c.json(), Value::Null),
                            Sink::Digest(d) => (d.json()["counts"].clone(), d.json()),
                        };
                        let mut line = json!({"i":job.index,"id":job.id,"pass":pass,"thread":k,
                            "stratum":job.stratum,"phase":phase_name(job.domain.phase),
                            "owner":mask(&job.domain.owner),"rank":job.domain.rank,
                            "key":job.key,"cpu_ns":cpu_ns,"wall_ns":wall_ns,
                            "start_ns":ns(t0),"end_ns":ns(t1),
                            "active_start":active_start,"active_end":active_end,
                            "cpu_start":start_cpu,"cpu_end":current_cpu(),
                            "voluntary_switches":r1[0] - r0[0],"involuntary_switches":r1[1] - r0[1],
                            "minor_faults":r1[2] - r0[2],"major_faults":r1[3] - r0[3],
                            "native_seconds":finished.seconds,"error_kind":finished.error_kind,
                            "error":finished.error,"counts":counts,
                            "stats":native_stats(finished.stats)});
                        if !digest.is_null() {
                            line["digest"] = digest;
                        }
                        if let Some(cancel) = cancel {
                            line["cancel"] = cancel;
                        }
                        if let (Sink::Dump(counts, bytes), Some(w)) = (&sink, streams.as_mut()) {
                            let mut head = Vec::with_capacity(32);
                            head.extend_from_slice(&BLOCK_MAGIC.to_le_bytes());
                            head.extend_from_slice(&job.id.to_le_bytes());
                            head.extend_from_slice(&(job.index as u32).to_le_bytes());
                            head.push(match job.domain.phase {
                                Phase::Apply => 0,
                                Phase::Route => 1,
                            });
                            let events: u64 = counts.records.iter().sum();
                            head.extend_from_slice(&(events as u32).to_le_bytes());
                            head.extend_from_slice(&(bytes.len() as u64).to_le_bytes());
                            w.file.write_all(&head).map_err(|e| e.to_string())?;
                            w.file.write_all(bytes).map_err(|e| e.to_string())?;
                            let entry = json!({"i":job.index,"id":job.id,"file":w.name,
                                "offset":w.offset,"block_bytes":head.len() + bytes.len(),
                                "events":events,"phase":phase_name(job.domain.phase),
                                "owner":mask(&job.domain.owner),"stratum":job.stratum,
                                "error_kind":finished.error_kind,"pass":pass});
                            w.offset += (head.len() + bytes.len()) as u64;
                            serde_json::to_writer(&mut w.index, &entry)
                                .map_err(|e| e.to_string())?;
                            w.index.write_all(b"\n").map_err(|e| e.to_string())?;
                        }
                        serde_json::to_writer(&mut lines, &line).map_err(|e| e.to_string())?;
                        lines.write_all(b"\n").map_err(|e| e.to_string())?;
                    }
                    let finished_at = ns(Instant::now());
                    let cpu_after = thread_cpu_ns();
                    let rusage_after = thread_rusage();
                    let sched_after = thread_schedstat();
                    lines.flush().map_err(|e| e.to_string())?;
                    if let Some(mut w) = streams {
                        w.file.flush().map_err(|e| e.to_string())?;
                        w.index.flush().map_err(|e| e.to_string())?;
                    }
                    Ok(json!({"thread":k,"pinned_cpu":cpu,"natives":natives,
                        "native_cpu_ns":cpu_sum,"native_wall_ns":wall_sum,
                        "thread_cpu_ns":cpu_after - cpu_before,"idle_from_ns":finished_at,
                        "schedstat_on_cpu_ns":sched_after[0] - sched_before[0],
                        "schedstat_run_delay_ns":sched_after[1] - sched_before[1],
                        "schedstat_slices":sched_after[2] - sched_before[2],
                        "voluntary_switches":rusage_after[0] - rusage_before[0],
                        "involuntary_switches":rusage_after[1] - rusage_before[1],
                        "minor_faults":rusage_after[2] - rusage_before[2]}))
                })
            })
            .collect();
        let results = handles
            .into_iter()
            .map(|h| h.join().expect("harness worker panicked"))
            .collect::<Vec<_>>();
        done.store(true, Ordering::Release);
        results
            .into_iter()
            .map(|r| r.unwrap_or_else(|e| json!({"error":e})))
            .collect()
    });
    let section_seconds = section.elapsed().as_secs_f64();
    receipt["section"] = json!({"wall_seconds":section_seconds,"started_unix":section_unix,
        "natives":total,"rusage_before":section_rusage_before,"rusage_after":process_rusage(),
        "threads":per_thread});
    receipt["memory"]["vm_hwm_kib"] = json!(status_kib("VmHWM:"));
    receipt["memory"]["vm_rss_kib"] = json!(status_kib("VmRSS:"));

    // Merge per-thread lines into natives.jsonl.
    let mut merged = BufWriter::new(
        std::fs::File::create(out.join("natives.jsonl")).map_err(|e| e.to_string())?,
    );
    for k in 0..threads {
        let path = out.join(format!("natives-t{k:03}.jsonl"));
        let mut file = std::fs::File::open(&path).map_err(|e| e.to_string())?;
        std::io::copy(&mut file, &mut merged).map_err(|e| e.to_string())?;
        std::fs::remove_file(&path).map_err(|e| e.to_string())?;
    }
    merged.flush().map_err(|e| e.to_string())?;
    receipt["timings"]["total_seconds"] = json!(started.elapsed().as_secs_f64());
    Ok(())
}

/// Compare the harness digests (`natives.jsonl`, SINK=digest) with the tap.
fn compare_with_tap(natives: &Path, tap: &Path) -> Result<Value, String> {
    let read = |path: &Path| -> Result<Vec<Value>, String> {
        let file = std::fs::File::open(path).map_err(|e| format!("{}: {e}", path.display()))?;
        std::io::BufReader::new(file)
            .lines()
            .map(|l| {
                l.map_err(|e| e.to_string())
                    .and_then(|l| serde_json::from_str(&l).map_err(|e| e.to_string()))
            })
            .collect()
    };
    let attempts = read(tap)?;
    let mut completed: HashMap<String, Vec<&Value>> = HashMap::new();
    let mut incomplete = HashMap::<String, usize>::new();
    for attempt in &attempts {
        let key = attempt["key"].as_str().ok_or("tap key")?.to_owned();
        if attempt["error_kind"] == "none" && attempt["consumer_break"] == false {
            completed.entry(key).or_default().push(attempt);
        } else {
            *incomplete
                .entry(format!(
                    "{}{}",
                    attempt["error_kind"].as_str().unwrap_or("?"),
                    if attempt["consumer_break"] == true {
                        "+break"
                    } else {
                        ""
                    }
                ))
                .or_default() += 1;
        }
    }
    let lines = read(natives)?;
    let (mut identical, mut mismatched, mut missing, mut repeated) = (0u64, 0u64, 0u64, 0u64);
    let mut by_phase: HashMap<String, [u64; 3]> = HashMap::new();
    let mut examples = Vec::new();
    let mut seen = std::collections::HashSet::new();
    let fields = ["ordered", "multiset", "successor_multiset", "counts"];
    for line in &lines {
        let key = line["key"].as_str().ok_or("native key")?;
        seen.insert(key.to_owned());
        let phase = line["phase"].as_str().unwrap_or("?").to_owned();
        let entry = by_phase.entry(phase).or_default();
        let Some(tapped) = completed.get(key) else {
            missing += 1;
            entry[2] += 1;
            if examples.len() < 16 {
                examples.push(json!({"kind":"not_in_tap","id":line["id"],"key":key}));
            }
            continue;
        };
        repeated += u64::from(tapped.len() > 1);
        let equal = line["error_kind"] == "none"
            && tapped.iter().all(|t| {
                fields.iter().all(|f| t["digest"][*f] == line["digest"][*f])
                    && t["stats"] == line["stats"]
            });
        if equal {
            identical += 1;
            entry[0] += 1;
        } else {
            mismatched += 1;
            entry[1] += 1;
            if examples.len() < 16 {
                examples.push(json!({"kind":"mismatch","id":line["id"],"key":key,
                    "harness":{"digest":line["digest"],"stats":line["stats"],"error_kind":line["error_kind"]},
                    "tap":tapped.iter().map(|t| json!({"digest":t["digest"],"stats":t["stats"]})).collect::<Vec<_>>()}));
            }
        }
    }
    let tap_only = completed.keys().filter(|k| !seen.contains(*k)).count();
    Ok(
        json!({"harness_natives":lines.len(),"tap_attempts":attempts.len(),
        "tap_completed_keys":completed.len(),"tap_incomplete_attempts":incomplete,
        "identical":identical,"mismatched":mismatched,"not_in_tap":missing,
        "tap_keys_not_reinspected":tap_only,"keys_with_repeated_completed_attempts":repeated,
        "by_phase":by_phase.into_iter().map(|(k, v)| (k, json!({"identical":v[0],"mismatched":v[1],"not_in_tap":v[2]}))).collect::<serde_json::Map<_,_>>(),
        "compared_fields":["digest.ordered","digest.multiset","digest.successor_multiset","digest.counts","stats"],
        "examples":examples,
        "passed":mismatched == 0 && missing == 0 && tap_only == 0 && lines.len() > 0}),
    )
}

#[test]
#[ignore = "needs a reinspection fixture and SYMBOLICA_LICENSE (see module docs)"]
fn reinspect_fixture() {
    let fixture_path = PathBuf::from(
        std::env::var_os("RUSTRED_HARNESS_FIXTURE").expect("RUSTRED_HARNESS_FIXTURE"),
    );
    let out = PathBuf::from(std::env::var_os("RUSTRED_HARNESS_OUT").expect("RUSTRED_HARNESS_OUT"));
    std::fs::create_dir(&out).unwrap_or_else(|e| panic!("{}: {e} (must be new)", out.display()));
    let started_unix = now_unix();
    let fixture: Value = serde_json::from_slice(&std::fs::read(&fixture_path).expect("fixture"))
        .expect("fixture JSON");
    assert_eq!(fixture["schema"], FIXTURE_SCHEMA);
    let arity = fixture["arity"].as_u64().expect("arity") as usize;
    let mut receipt = json!({"schema":"rustred.reinspection-run.v1","fixture":fixture_path,
        "fixture_provenance":fixture["provenance"],"started_unix":started_unix,
        "executable":std::env::current_exe().ok(),"host_cpus":std::thread::available_parallelism().map(|n| n.get()).ok(),
        "timings":{},"memory":{},"scope":"native inspection only (inspection::inspect with a sink that accepts every event); no admission, queue or publication"});
    macro_rules! dispatch { ($($n:literal),*) => { match arity {
        $($n => run_fixture::<$n>(&fixture, &out, &mut receipt),)*
        _ => Err(format!("unsupported arity {arity}")),
    }} }
    let outcome = dispatch!(1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16);
    receipt["error"] = json!(outcome.as_ref().err());
    if outcome.is_ok()
        && let Some(tap) = std::env::var_os("RUSTRED_HARNESS_TAP")
    {
        receipt["differential"] = compare_with_tap(&out.join("natives.jsonl"), Path::new(&tap))
            .unwrap_or_else(|e| json!({"error":e,"passed":false}));
    }
    receipt["passed"] = json!(
        outcome.is_ok()
            && receipt
                .get("differential")
                .is_none_or(|d| d["passed"] == true)
    );
    std::fs::write(
        out.join("receipt.json"),
        serde_json::to_vec_pretty(&receipt).expect("receipt"),
    )
    .expect("write receipt");
    println!(
        "{}",
        serde_json::to_string_pretty(&json!({"out":out,"timings":receipt["timings"],
            "section":{"wall_seconds":receipt["section"]["wall_seconds"],"natives":receipt["section"]["natives"]},
            "differential":receipt.get("differential").map(|d| json!({"passed":d["passed"],
                "identical":d["identical"],"mismatched":d["mismatched"],"not_in_tap":d["not_in_tap"],
                "tap_keys_not_reinspected":d["tap_keys_not_reinspected"]})),
            "error":receipt["error"],"passed":receipt["passed"]}))
        .expect("summary")
    );
    outcome.expect("reinspection");
    assert_eq!(receipt["passed"], true, "differential failed");
}

/// Run a control walk in this process with the tap enabled: the walk is
/// `owner_domain_walk_with_progress` on the control's argv (outputs and the
/// checkpoint moved under RUSTRED_TAP_OUT), and every inspection's stream
/// digest goes to `tap.jsonl`. `request.json` is written for the restore
/// (`reinspection_extract`).
///
/// ```text
/// RUSTRED_TAP_COMMAND=<control command.json> RUSTRED_TAP_OUT=<new dir> \
/// [RUSTRED_TAP_WORKERS=W] [RUSTRED_HARNESS_{MANIFEST,OWNER_BASE,QUERIES}=...] \
/// cargo test --release -p rustred-app --lib tap_control_walk -- --ignored --nocapture
/// ```
#[test]
#[ignore = "needs a control command, its inputs and SYMBOLICA_LICENSE"]
fn tap_control_walk() {
    let command: Vec<String> = serde_json::from_slice(
        &std::fs::read(std::env::var_os("RUSTRED_TAP_COMMAND").expect("RUSTRED_TAP_COMMAND"))
            .expect("command"),
    )
    .expect("command JSON");
    let out = PathBuf::from(std::env::var_os("RUSTRED_TAP_OUT").expect("RUSTRED_TAP_OUT"));
    std::fs::create_dir(&out).unwrap_or_else(|e| panic!("{}: {e} (must be new)", out.display()));
    let overrides = input_overrides();
    let workers = env("RUSTRED_TAP_WORKERS");
    let mut argv: Vec<String> = Vec::new();
    let mut arguments = command.iter();
    argv.push(arguments.next().expect("program").clone());
    while let Some(argument) = arguments.next() {
        let replacement = match argument.as_str() {
            "--output" => Some(out.join("result.json").display().to_string()),
            "--events" => Some(out.join("events.jsonl").display().to_string()),
            "--stop-file" => Some(out.join("stop-request.json").display().to_string()),
            "--checkpoint" | "--resume" => Some(out.join("checkpoint").display().to_string()),
            "--workers" => workers.clone(),
            other => overrides
                .iter()
                .find(|(o, _)| *o == other)
                .map(|(_, p)| p.display().to_string()),
        };
        argv.push(argument.clone());
        if let Some(value) = replacement {
            arguments.next();
            argv.push(value);
        }
    }
    let request = crate::cli::walk_request_from_argv(argv.iter().map(OsString::from).collect())
        .expect("request");
    std::fs::write(
        out.join("request.json"),
        serde_json::to_vec_pretty(&json!({"command":argv})).expect("request JSON"),
    )
    .expect("write request.json");
    let cancel = AtomicBool::new(false);
    let events = AtomicUsize::new(0);
    let started = Instant::now();
    tap::set(true);
    let result = crate::owner_domain_walk_with_progress(request, &cancel, |_| {
        events.fetch_add(1, Ordering::Relaxed);
    });
    tap::set(false);
    let seconds = started.elapsed().as_secs_f64();
    let attempts = tap::take();
    let mut file = BufWriter::new(std::fs::File::create(out.join("tap.jsonl")).expect("tap"));
    for attempt in &attempts {
        serde_json::to_writer(&mut file, attempt).expect("tap line");
        file.write_all(b"\n").expect("tap newline");
    }
    file.flush().expect("tap flush");
    let result = result.expect("walk");
    let d = &result.document;
    let summary = json!({"argv":argv,"seconds":seconds,"observer_events":events.load(Ordering::Relaxed),
        "tap_attempts":attempts.len(),
        "completed_nodes":d["completed_nodes"],"scheduled_nodes":d["scheduled_nodes"],
        "native_processed_nodes":d["native_processed_nodes"],"frontiers":d["frontiers"],
        "all_scheduled_domains_resolved":d["all_scheduled_domains_resolved"],
        "recursive_worklist_exhausted":d["recursive_worklist_exhausted"],
        "routed_domains":d["routed_domains"],"successors":d["successors"],"events":d["events"],
        "error":d["error"],"status":d["status"]});
    std::fs::write(
        out.join("walk-summary.json"),
        serde_json::to_vec_pretty(&summary).expect("summary"),
    )
    .expect("write summary");
    println!(
        "{}",
        serde_json::to_string_pretty(&summary).expect("summary")
    );
}

#[test]
fn event_encoding_is_canonical_and_digest_order_free_for_multisets() {
    let domain = |lower: u64| Domain::<3> {
        phase: Phase::Apply,
        owner: [true, false, true],
        lower: vec![lower, 0, 1],
        upper: vec![Some(4), None, Some(7)],
        rank: Some(5),
        powers: Default::default(),
    };
    let admit = |lower| {
        Event::one(Effect::Admit {
            domain: domain(lower),
            successor: true,
            conditional: false,
        })
    };
    let mut bytes = Vec::new();
    assert_eq!(encode_event(&admit(2), &mut bytes), 3);
    assert_eq!(bytes[0], 3);
    assert_eq!(u32::from_le_bytes(bytes[1..5].try_into().unwrap()), 1);
    assert_eq!(bytes[5], 1);
    let len = u32::from_le_bytes(bytes[6..10].try_into().unwrap()) as usize;
    assert_eq!(&bytes[10..], bincode_domain(&domain(2)).as_slice());
    assert_eq!(len, bytes.len() - 10);
    let decoded: (Domain<3>, usize) =
        bincode::serde::decode_from_slice(&bytes[10..], bincode::config::standard()).unwrap();
    assert_eq!(decoded.0, domain(2));

    let mut a = StreamDigest::default();
    let mut b = StreamDigest::default();
    for e in [admit(1), admit(2), Event::one(Effect::Count)] {
        a.push(&e);
    }
    for e in [Event::one(Effect::Count), admit(2), admit(1)] {
        b.push(&e);
    }
    let (a, b) = (a.json(), b.json());
    assert_ne!(a["ordered"], b["ordered"]);
    assert_eq!(a["multiset"], b["multiset"]);
    assert_eq!(a["successor_multiset"], b["successor_multiset"]);
    assert_eq!(a["counts"], b["counts"]);
    let mut c = StreamDigest::default();
    for e in [admit(1), admit(1), Event::one(Effect::Count)] {
        c.push(&e);
    }
    assert_ne!(c.json()["successor_multiset"], a["successor_multiset"]);
    assert_eq!(a["counts"]["successors"], 2);
    assert_eq!(a["counts"]["records"]["admit"], 2);
}

#[test]
fn checkpoint_options_are_stripped_and_inputs_overridden() {
    let argv: Vec<String> = [
        "rustred",
        "owner-domain-match",
        "--manifest",
        "/a/selection.json",
        "--resume",
        "/ck",
        "--checkpoint-interval-seconds",
        "14400",
        "--workers",
        "100",
        "--owner-base",
        "/a",
    ]
    .map(str::to_owned)
    .to_vec();
    let out = strip_checkpoint(&argv, &[("--owner-base", PathBuf::from("/b"))]);
    let out: Vec<_> = out.iter().map(|a| a.to_str().unwrap()).collect();
    assert_eq!(
        out,
        [
            "rustred",
            "owner-domain-match",
            "--manifest",
            "/a/selection.json",
            "--workers",
            "100",
            "--owner-base",
            "/b"
        ]
    );
}
