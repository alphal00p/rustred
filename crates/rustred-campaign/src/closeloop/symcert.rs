//! Lane symcert (interfaces/proof-symrows-job.md 5): KERNEL CERTIFICATES of multi-term symmetry relations for the
//! three producers (the closing loop's symmetry relation at the key, the repair flow's composites, lane tworow's
//! two-row forms). [`certify_symmetry_relations`] puts every relation to the kernel (rr_proof `--symrows`) as ONE
//! `proof-symrows` contract job through the process layer (`adapters::job::run_attempt` + the real launcher: own
//! session, PDEATHSIG, memory cap, wall cap; run inside a private pid namespace, L10), `parallel` at once, and returns
//! one verdict per relation:
//! - `certified`: the kernel proved the relation vector at the key in the span of its rows (row 40, class certified):
//!   the relation reaches the evidence level of a per-key proof of a table rule;
//! - `kernel_refused`: the kernel's exact / modular non-membership answer, or its refusal of a symmetry row
//!   (SYMROW_REFUSED other than a capacity limit): a FINDING when our algebra accepted the relation; written with BOTH
//!   sides to `<work_root>/findings.jsonl`; the relation is NOT accepted at the per-key level (never resolved by
//!   choosing a side);
//! - `over_kernel_capacity` (SYMROW_REFUSED: expansion / coefficient bounds), `not_expressible` (typed, no job),
//!   `undecided`, `not_applicable`, `failed` (row, why): the relation keeps today's level.
//!
//! Journal `<work_root>/verdicts.jsonl`: one line per decided relation; a restart with the same relation (id, relation
//! digest) and kernel gets the journaled verdict without a new job. Generic (L8): kernel caps, family, CPUs, caps and
//! programs are configuration.

use crate::adapters::job::{JobAdapter, Program, RunOptions, run_attempt};
use crate::adapters::symrows::{SymrowsAdapter, SymrowsJobRequest};
use crate::launch::RealLauncher;
use crate::resources::{Sampler, SamplerConfig};
use rustred_campaign_contract::fake::FakeScript;
use rustred_campaign_core::symcert::{KernelCaps, SymRelation};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::collections::{BTreeMap, HashMap, VecDeque};
use std::io::{BufRead, Write};
use std::path::PathBuf;
use std::sync::{Arc, Mutex, mpsc};
use std::time::{Duration, Instant};

/// Configuration of a certification batch (data).
#[derive(Clone, Debug)]
pub struct CertifyConfig {
    pub work_root: PathBuf,
    /// The kernel (rr_proof of the frozen engine; `rr_fake_child` with `fake` set in tests).
    pub program: Program,
    pub licence_env: Option<String>,
    /// The presented family text of the keys' frame (header `family_source` of every relation table).
    pub family_source: String,
    pub caps: KernelCaps,
    /// Jobs at once.
    pub parallel: usize,
    /// CPUs to pin the workers to (empty = unpinned).
    pub cpus: Vec<usize>,
    pub memory_cap_bytes: Option<u64>,
    pub wall_cap: Option<Duration>,
    /// Extra attempts after a host-resource end (row 15b).
    pub resource_retries: u32,
    pub resource_backoff: Duration,
    pub max_primes: Option<u32>,
    /// Test stand: the fake child's script (per relation id when present in `fake_by_id`).
    pub fake: Option<FakeScript>,
    pub fake_by_id: BTreeMap<String, FakeScript>,
}

/// The verdict words (closed list).
pub const VERDICTS: [&str; 7] = [
    "certified",
    "kernel_refused",
    "over_kernel_capacity",
    "not_expressible",
    "undecided",
    "not_applicable",
    "failed",
];

/// One relation's verdict.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct RelationVerdict {
    pub id: String,
    pub producer: String,
    pub producer_record_sha256: Option<String>,
    pub key: Vec<i64>,
    pub relation_digest: Option<String>,
    pub kernel_sha256: String,
    /// One of [`VERDICTS`].
    pub verdict: String,
    /// The kernel status / refusal / reason.
    pub detail: String,
    /// The sealed certificate line (row 40) and its SHA-256.
    pub certificate: Option<String>,
    pub certificate_sha256: Option<String>,
    pub job_id: Option<String>,
    pub run_dir: Option<String>,
    /// Decision row of the last attempt (`40`, `41`, `15b`, ...).
    pub row: Option<String>,
    pub attempts: u32,
    pub seconds: f64,
    /// A disagreement with our own algebra (kernel refused what the producer accepted).
    pub finding: bool,
}

fn verdict_of(
    rel: &SymRelation,
    digest: Option<String>,
    kernel: &str,
    verdict: &str,
    detail: impl Into<String>,
) -> RelationVerdict {
    RelationVerdict {
        id: rel.id.clone(),
        producer: rel.producer.clone(),
        producer_record_sha256: rel.producer_record_sha256.clone(),
        key: rel.key.clone(),
        relation_digest: digest,
        kernel_sha256: kernel.to_string(),
        verdict: verdict.into(),
        detail: detail.into(),
        certificate: None,
        certificate_sha256: None,
        job_id: None,
        run_dir: None,
        row: None,
        attempts: 0,
        seconds: 0.0,
        finding: verdict == "kernel_refused",
    }
}

/// Whether a kernel SYMROW_REFUSED message is a CAPACITY limit (expansion / coefficient bounds), not a refutation.
pub fn is_capacity_refusal(msg: &str) -> bool {
    [
        "expansion above",
        "coefficient overflow",
        "coefficient beyond",
    ]
    .iter()
    .any(|p| msg.contains(p))
}

fn utc_now() -> String {
    let s = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    rustred_campaign_contract::time::utc_string(
        std::time::UNIX_EPOCH + std::time::Duration::from_secs(s),
    )
}

fn journal(path: &std::path::Path) -> HashMap<(String, String, String), RelationVerdict> {
    let mut m = HashMap::new();
    let Ok(f) = std::fs::File::open(path) else {
        return m;
    };
    for l in std::io::BufReader::new(f).lines().map_while(Result::ok) {
        if let Ok(v) = serde_json::from_str::<RelationVerdict>(&l)
            && let Some(d) = v.relation_digest.clone()
        {
            m.insert((v.id.clone(), d, v.kernel_sha256.clone()), v);
        }
    }
    m
}

fn append(path: &std::path::Path, v: &Value) -> Result<(), String> {
    let mut f = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
        .map_err(|e| format!("{}: {e}", path.display()))?;
    writeln!(f, "{v}").map_err(|e| e.to_string())
}

struct Done {
    index: usize,
    verdict: RelationVerdict,
}

/// One relation through the kernel (attempts with 15b retries) -> its verdict.
fn run_one(
    cfg: &CertifyConfig,
    req: SymrowsJobRequest,
    cpu: Option<usize>,
    sampler: Arc<Sampler>,
) -> RelationVerdict {
    let rel = req.relation.clone();
    let kernel = req.request.kernel_sha256.clone();
    let digest = Some(req.request.relation_digest.clone());
    let fake = cfg
        .fake_by_id
        .get(&rel.id)
        .cloned()
        .or_else(|| cfg.fake.clone());
    let ad = SymrowsAdapter {
        program: cfg.program.clone(),
        licence_env: cfg.licence_env.clone(),
        fake,
    };
    let reqs = vec![req];
    let plan = match ad.plan(&reqs) {
        Ok(mut p) if p.len() == 1 => p.remove(0),
        Ok(p) => {
            return verdict_of(
                &rel,
                digest,
                &kernel,
                "failed",
                format!("{} plans", p.len()),
            );
        }
        Err(e) => return verdict_of(&rel, digest, &kernel, "failed", e.to_string()),
    };
    let opts = RunOptions {
        cpus: cpu.into_iter().collect(),
        memory_cap_bytes: cfg.memory_cap_bytes,
        licence_env: cfg.licence_env.clone(),
        wall_cap: cfg.wall_cap,
        grace: Some(Duration::from_secs(5)),
    };
    let t0 = Instant::now();
    // a new attempt directory per run (an attempt directory is never reused): the first free attempt number
    let mut attempt = 1;
    while rustred_campaign_contract::layout::attempt_dir(&cfg.work_root, &plan.job_id, attempt)
        .exists()
    {
        attempt += 1;
    }
    let first = attempt;
    loop {
        let mut l = RealLauncher::new(sampler.clone());
        let res = run_attempt(&mut l, &ad, &reqs, &plan, &cfg.work_root, attempt, &opts);
        let (out, rep) = match res {
            Ok(x) => x,
            Err(e) => {
                let mut v = verdict_of(&rel, digest, &kernel, "failed", e.to_string());
                v.attempts = attempt - first + 1;
                v.seconds = t0.elapsed().as_secs_f64();
                return v;
            }
        };
        let c = &out.classification;
        let row = format!("{}{}", c.row, c.sub);
        if (c.row, c.sub.as_str()) == (15, "b") && attempt - first < cfg.resource_retries {
            std::thread::sleep(cfg.resource_backoff);
            attempt += 1;
            continue;
        }
        let mut v = match (&out.certificate, c.row) {
            (Some(cert), 40) => {
                let word = match cert.class.as_str() {
                    "certified" => "certified",
                    "kernel_refused" => "kernel_refused",
                    "not_applicable" => "not_applicable",
                    _ => "undecided",
                };
                let mut v = verdict_of(&rel, digest.clone(), &kernel, word, cert.status.clone());
                let line = String::from_utf8_lossy(&cert.canonical).into_owned();
                v.certificate_sha256 = Some(crate::adapters::job::sha256_hex(line.as_bytes()));
                v.certificate = Some(line);
                v
            }
            (_, 41) => {
                let msg = out.refusal.clone().unwrap_or_default();
                if is_capacity_refusal(&msg) {
                    verdict_of(&rel, digest.clone(), &kernel, "over_kernel_capacity", msg)
                } else {
                    verdict_of(&rel, digest.clone(), &kernel, "kernel_refused", msg)
                }
            }
            _ => verdict_of(
                &rel,
                digest.clone(),
                &kernel,
                "failed",
                format!(
                    "row {row}: {}{}",
                    out.unread.clone().unwrap_or_default(),
                    out.refusal
                        .as_ref()
                        .map(|r| format!(" [{r}]"))
                        .unwrap_or_default()
                ),
            ),
        };
        v.job_id = Some(out.job_id.clone());
        v.run_dir = Some(rep.prepared.spec.run_dir.clone());
        v.row = Some(row);
        v.attempts = attempt - first + 1;
        v.seconds = t0.elapsed().as_secs_f64();
        return v;
    }
}

/// Certify every relation (see the module doc). Verdicts in input order; findings and the journal under
/// `cfg.work_root`. `Err` only for a configuration / IO failure of the batch itself.
pub fn certify_symmetry_relations(
    relations: &[SymRelation],
    cfg: &CertifyConfig,
) -> Result<Vec<RelationVerdict>, String> {
    std::fs::create_dir_all(&cfg.work_root).map_err(|e| e.to_string())?;
    let jpath = cfg.work_root.join("verdicts.jsonl");
    let fpath = cfg.work_root.join("findings.jsonl");
    let known = journal(&jpath);
    let kernel = cfg
        .program
        .sha256
        .clone()
        .ok_or("the kernel program has no sha256")?;
    let mut out: Vec<Option<RelationVerdict>> = vec![None; relations.len()];
    let mut todo: VecDeque<(usize, SymrowsJobRequest)> = VecDeque::new();
    for (i, rel) in relations.iter().enumerate() {
        if let Err(nx) = rel.expressible(&cfg.caps) {
            out[i] = Some(verdict_of(
                rel,
                None,
                &kernel,
                "not_expressible",
                format!("{}: {}", nx.class, nx.why),
            ));
            continue;
        }
        let req = match SymrowsJobRequest::new(
            rel.clone(),
            &cfg.family_source,
            &kernel,
            cfg.max_primes,
        ) {
            Ok(r) => r,
            Err(e) => {
                out[i] = Some(verdict_of(rel, None, &kernel, "not_expressible", e));
                continue;
            }
        };
        let jk = (
            rel.id.clone(),
            req.request.relation_digest.clone(),
            kernel.clone(),
        );
        if let Some(v) = known.get(&jk) {
            out[i] = Some(v.clone());
            continue;
        }
        todo.push_back((i, req));
    }
    if !todo.is_empty() {
        let sampler = Arc::new(
            Sampler::spawn(SamplerConfig {
                interval: Duration::from_millis(500),
                collect_foreign_pinning: false,
            })
            .map_err(|e| format!("sampler: {e}"))?,
        );
        let n = todo.len();
        let queue = Arc::new(Mutex::new(todo));
        let (tx, rx) = mpsc::channel::<Done>();
        let workers = cfg.parallel.max(1).min(n);
        let mut handles = vec![];
        for w in 0..workers {
            let (q, tx, sampler, cfg) = (queue.clone(), tx.clone(), sampler.clone(), cfg.clone());
            let cpu = cfg.cpus.get(w).copied();
            handles.push(
                std::thread::Builder::new()
                    .name(format!("symcert-{w}"))
                    .spawn(move || {
                        loop {
                            let next = q.lock().ok().and_then(|mut g| g.pop_front());
                            let Some((index, req)) = next else { break };
                            let verdict = run_one(&cfg, req, cpu, sampler.clone());
                            if tx.send(Done { index, verdict }).is_err() {
                                break;
                            }
                        }
                    })
                    .map_err(|e| format!("worker thread: {e}"))?,
            );
        }
        drop(tx);
        for d in rx.iter() {
            let v = &d.verdict;
            if v.verdict != "failed" {
                append(&jpath, &serde_json::to_value(v).map_err(|e| e.to_string())?)?;
            }
            if v.finding {
                let rel = &relations[d.index];
                append(
                    &fpath,
                    &json!({"utc": utc_now(), "finding": "kernel refused a relation our algebra accepted",
                        "id": rel.id, "producer": rel.producer, "key": rel.key,
                        "relation_digest": v.relation_digest, "producer_record_sha256": rel.producer_record_sha256,
                        "kernel": {"sha256": v.kernel_sha256, "row": v.row, "status": v.detail,
                                   "certificate": v.certificate, "run_dir": v.run_dir},
                        "ours": rel.own, "group_digest": rel.group_digest,
                        "rows": rel.rows.iter().map(|r| json!({"element": r.element, "seed": r.seed})).collect::<Vec<_>>(),
                        "catalogue": rel.catalogue.iter().map(|c| json!({"source": c.source, "label": c.label,
                            "record_sha256": c.record_sha256, "term": c.term})).collect::<Vec<_>>()}),
                )?;
            }
            out[d.index] = Some(d.verdict);
        }
        for h in handles {
            let _ = h.join();
        }
    }
    out.into_iter()
        .enumerate()
        .map(|(i, v)| v.ok_or_else(|| format!("relation {i}: no verdict (worker lost)")))
        .collect()
}

/// Counts by verdict word, plus the time per relation of the jobs run (min / median / max seconds).
pub fn summary(v: &[RelationVerdict]) -> Value {
    let mut by: BTreeMap<&str, usize> = BTreeMap::new();
    for x in v {
        *by.entry(x.verdict.as_str()).or_insert(0) += 1;
    }
    let mut secs: Vec<f64> = v
        .iter()
        .filter(|x| x.job_id.is_some())
        .map(|x| x.seconds)
        .collect();
    secs.sort_by(|a, b| a.total_cmp(b));
    let pick = |q: f64| -> Option<f64> {
        (!secs.is_empty()).then(|| secs[((secs.len() - 1) as f64 * q).round() as usize])
    };
    json!({"relations": v.len(), "by_verdict": by, "findings": v.iter().filter(|x| x.finding).count(),
        "seconds_per_job": {"min": pick(0.0), "median": pick(0.5), "max": pick(1.0),
                            "total": secs.iter().sum::<f64>(), "jobs": secs.len()}})
}

/// The verdicts of a batch journal (`<work_root>/verdicts.jsonl`), e.g. for the import hook of a later pass.
pub fn read_verdicts(path: &std::path::Path) -> Result<Vec<RelationVerdict>, String> {
    let t = std::fs::read_to_string(path).map_err(|e| format!("{}: {e}", path.display()))?;
    t.lines()
        .filter(|l| !l.trim().is_empty())
        .map(|l| serde_json::from_str(l).map_err(|e| format!("{}: {e}", path.display())))
        .collect()
}

/// What an import-side lift is bound to (lane symcert; audit r2-sym F4): the pinned kernels, the import's family and
/// the directories certificates may come from. Data of the caller (the campaign's configuration), never constants.
#[derive(Clone, Debug)]
pub struct ImportBinding {
    /// SHA-256 of every kernel binary whose certificates count (the pinned frozen kernels).
    pub kernels: Vec<String>,
    /// BLAKE3 of the presented family text of the import's frame (the frame header's `family_source`).
    pub family_blake3: String,
    /// Certificate run directories must lie under one of these (canonical) roots: a certificate of another campaign's
    /// work tree is never read.
    pub roots: Vec<PathBuf>,
}

/// Why a verdict does not lift a level (typed; the relation keeps today's level).
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct NoLift {
    pub id: String,
    pub key: Vec<i64>,
    /// `not_decided` | `no_run_dir` | `outside_certificate_roots` | `job_unreadable` | `not_a_certificate_job` |
    /// `certificate_not_sealed` | `certificate_unbound` | `kernel_not_pinned` | `family_differs` |
    /// `no_import_record` | `import_record_unreadable` | `relation_digest_differs` | `key_out_of_range`
    pub reason: &'static str,
    pub detail: String,
}

/// The import's supplied records at a key: (record SHA-256, the record line) for every supplied relation set that
/// serves the key (callers: [`import_records`] over an `ImportedSet`, tests over data).
pub type ImportRecords<'a> = dyn Fn(&[i32]) -> Vec<(String, Vec<u8>)> + 'a;

/// What a run directory proves when classified again from its own files.
enum RunAnswer {
    /// Row 40: the sealed certificate, re-checked against the run's pinned request / table / symrows input.
    Sealed(Box<rustred_campaign_contract::classify::symcert::Certificate>),
    /// Row 41 (not a kernel capacity limit): the kernel refused a symmetry row of THIS pinned request.
    Refused41(
        Box<rustred_campaign_contract::classify::symcert::SymrowsRequest>,
        String,
    ),
}

/// The answer of one run directory (the attempt classified again from its files).
fn run_answer(run_dir: &std::path::Path) -> Result<RunAnswer, (&'static str, String)> {
    use rustred_campaign_contract::classify::{self, KillReason, WaitStatus};
    use rustred_campaign_contract::layout::RunLayout;
    use rustred_campaign_contract::spec::{JobKind, JobSpec};
    let layout = RunLayout::new(run_dir);
    let bytes = std::fs::read(layout.job_spec())
        .map_err(|e| ("job_unreadable", format!("{}: {e}", run_dir.display())))?;
    let spec: JobSpec =
        serde_json::from_slice(&bytes).map_err(|e| ("job_unreadable", format!("job.json: {e}")))?;
    if spec.kind != JobKind::ProofSymrows
        || std::fs::canonicalize(&spec.run_dir).ok().as_deref() != Some(run_dir)
    {
        return Err((
            "not_a_certificate_job",
            format!("kind {:?}, run_dir {}", spec.kind, spec.run_dir),
        ));
    }
    let digest = rustred_campaign_contract::spec::spec_blake3(&bytes);
    let ev = classify::assess(
        &layout,
        &spec,
        &digest,
        WaitStatus::Unknown,
        KillReason::None,
        false,
    );
    let c = classify::classify(&ev);
    if c.row == 41 {
        let msg = crate::adapters::symrows::refusal_message(&layout).unwrap_or_default();
        if is_capacity_refusal(&msg) {
            return Err(("certificate_not_sealed", format!("row 41 capacity: {msg}")));
        }
        let r = rustred_campaign_contract::classify::symcert::request(&spec)
            .map_err(|e| ("certificate_unbound", e))?;
        return Ok(RunAnswer::Refused41(Box::new(r), msg));
    }
    if c.row != 40 {
        return Err((
            "certificate_not_sealed",
            format!("row {}{}: {}", c.row, c.sub, c.why),
        ));
    }
    let v = ev
        .verified
        .as_ref()
        .ok_or(("certificate_not_sealed", "no verified outputs".to_string()))?;
    let mut certs =
        rustred_campaign_contract::classify::symcert::certificates_of_run(&layout.root, &spec, v)
            .map_err(|e| ("certificate_unbound", e))?;
    match (certs.pop(), certs.is_empty()) {
        (Some(c), true) => Ok(RunAnswer::Sealed(Box::new(c))),
        _ => Err(("certificate_unbound", "not exactly one certificate".into())),
    }
}

/// The import's evidence of kernel certificates, BOUND AT IMPORT (audit r2-sym F4): a verdict line is only a POINTER to
/// a run directory; a level is lifted only when that directory lies under `b.roots`, its attempt classifies as row 40
/// from its own files, its sealed certificate re-checks against its pinned request / table / symrows input, the
/// certificate's kernel is one of `b.kernels`, its family is the import's (`b.family_blake3`), and the relation digest
/// recomputed from the IMPORT'S OWN record (the supplied record with the certificate's producer record SHA-256 at the
/// certificate's key, from `records`) equals the certificate's. Class certified -> certify; kernel_refused -> refuse
/// (never per-key while the disagreement stands). Everything else: no lift, with a typed reason.
pub fn bind_certificates(
    verdicts: &[RelationVerdict],
    records: &ImportRecords<'_>,
    b: &ImportBinding,
) -> (
    rustred_campaign_core::symcert::SymmetryCertificates,
    Vec<NoLift>,
) {
    use rustred_campaign_contract::classify::symcert as sc;
    let mut m = rustred_campaign_core::symcert::SymmetryCertificates::default();
    let mut no = vec![];
    let roots: Vec<PathBuf> = b
        .roots
        .iter()
        .filter_map(|r| std::fs::canonicalize(r).ok())
        .collect();
    for v in verdicts {
        let fail = |reason: &'static str, detail: String| NoLift {
            id: v.id.clone(),
            key: v.key.clone(),
            reason,
            detail,
        };
        if !matches!(v.verdict.as_str(), "certified" | "kernel_refused") {
            no.push(fail("not_decided", v.verdict.clone()));
            continue;
        }
        let Some(rd) = &v.run_dir else {
            no.push(fail(
                "no_run_dir",
                "the verdict names no run directory".into(),
            ));
            continue;
        };
        let Ok(run) = std::fs::canonicalize(rd) else {
            no.push(fail("job_unreadable", format!("{rd}: not found")));
            continue;
        };
        if !roots.iter().any(|r| run.starts_with(r)) {
            no.push(fail("outside_certificate_roots", run.display().to_string()));
            continue;
        }
        // (kernel, family, relation digest, key, producer record, class, status, certificate sha)
        let cert = match run_answer(&run) {
            Ok(RunAnswer::Sealed(c)) => Bound {
                kernel: c.kernel_sha256.clone(),
                family: c.family_blake3.clone(),
                digest: c.relation_digest.clone(),
                key: c.key.clone(),
                producer: c.producer_record_sha256.clone(),
                class: c.class.clone(),
                status: c.status.clone(),
                sha: crate::adapters::job::sha256_hex(&c.canonical),
            },
            Ok(RunAnswer::Refused41(r, msg)) => Bound {
                kernel: r.kernel_sha256.clone(),
                family: r.family_blake3.clone(),
                digest: r.relation_digest.clone(),
                key: r.key.clone(),
                producer: r.producer_record_sha256.clone(),
                class: "kernel_refused".into(),
                status: msg,
                sha: String::new(),
            },
            Err((reason, d)) => {
                no.push(fail(reason, d));
                continue;
            }
        };
        if !b.kernels.contains(&cert.kernel) {
            no.push(fail("kernel_not_pinned", cert.kernel.clone()));
            continue;
        }
        if cert.family != b.family_blake3 {
            no.push(fail("family_differs", cert.family.clone()));
            continue;
        }
        let Ok(key) = cert
            .key
            .iter()
            .map(|&k| i32::try_from(k))
            .collect::<Result<Vec<i32>, _>>()
        else {
            no.push(fail("key_out_of_range", format!("{:?}", cert.key)));
            continue;
        };
        let Some(sha) = &cert.producer else {
            no.push(fail(
                "no_import_record",
                "the certificate names no producer record".into(),
            ));
            continue;
        };
        let Some((_, line)) = records(&key).into_iter().find(|(s, _)| s == sha) else {
            no.push(fail(
                "no_import_record",
                format!("no supplied record {sha} at the key in the import"),
            ));
            continue;
        };
        let digest = match sc::relation_of_record(&line)
            .and_then(|(k, rhs)| sc::relation_digest(&k, &rhs).map(|d| (k, d)))
        {
            Ok((k, d)) if k == cert.key => d,
            Ok(_) => {
                no.push(fail(
                    "relation_digest_differs",
                    "the import record is at another key".into(),
                ));
                continue;
            }
            Err(e) => {
                no.push(fail("import_record_unreadable", e));
                continue;
            }
        };
        if digest != cert.digest {
            no.push(fail(
                "relation_digest_differs",
                format!("import {digest}, certificate {}", cert.digest),
            ));
            continue;
        }
        match cert.class.as_str() {
            "certified" => m.certify(sha, key, cert.sha.clone()),
            "kernel_refused" => m.refuse(sha, key, cert.status.clone()),
            other => no.push(fail("not_decided", format!("certificate class {other}"))),
        }
    }
    (m, no)
}

/// The binding members of a run's answer (sealed certificate or row-41 refusal of its pinned request).
struct Bound {
    kernel: String,
    family: String,
    digest: String,
    key: Vec<i64>,
    producer: Option<String>,
    class: String,
    status: String,
    sha: String,
}

/// A sealed certificate stream for the import's route (b) (lane symfix: `kernel_certificates.files`): the run's
/// `out/results.jsonl`, pinned by SHA-256.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct CertificateFile {
    pub path: String,
    pub sha256: String,
    pub relation_id: String,
}

/// The sealed certificate streams of the CERTIFIED verdicts for an import spec's `kernel_certificates.files` (the
/// import checks each certificate against its own record at load): every run directory re-verified as in
/// [`bind_certificates`] (under the roots, row 40 from its own files, sealed line re-checked against its pinned inputs,
/// kernel pinned, family the import's); anything else is left out with a typed reason. The kernels allow-list of the
/// spec is `b.kernels`.
pub fn kernel_certificate_files(
    verdicts: &[RelationVerdict],
    b: &ImportBinding,
) -> (Vec<CertificateFile>, Vec<NoLift>) {
    let roots: Vec<PathBuf> = b
        .roots
        .iter()
        .filter_map(|r| std::fs::canonicalize(r).ok())
        .collect();
    let mut files = vec![];
    let mut no = vec![];
    for v in verdicts {
        let fail = |reason: &'static str, detail: String| NoLift {
            id: v.id.clone(),
            key: v.key.clone(),
            reason,
            detail,
        };
        if v.verdict != "certified" {
            no.push(fail("not_decided", v.verdict.clone()));
            continue;
        }
        let Some(run) = v
            .run_dir
            .as_ref()
            .and_then(|d| std::fs::canonicalize(d).ok())
        else {
            no.push(fail("no_run_dir", format!("{:?}", v.run_dir)));
            continue;
        };
        if !roots.iter().any(|r| run.starts_with(r)) {
            no.push(fail("outside_certificate_roots", run.display().to_string()));
            continue;
        }
        match run_answer(&run) {
            Ok(RunAnswer::Sealed(c)) => {
                if !b.kernels.contains(&c.kernel_sha256) {
                    no.push(fail("kernel_not_pinned", c.kernel_sha256.clone()));
                } else if c.family_blake3 != b.family_blake3 {
                    no.push(fail("family_differs", c.family_blake3.clone()));
                } else if c.class != "certified" {
                    no.push(fail("not_decided", c.class.clone()));
                } else {
                    let p = run.join("out/results.jsonl");
                    match std::fs::read(&p) {
                        Ok(bytes) => files.push(CertificateFile {
                            path: p.display().to_string(),
                            sha256: crate::adapters::job::sha256_hex(&bytes),
                            relation_id: v.id.clone(),
                        }),
                        Err(e) => no.push(fail("job_unreadable", e.to_string())),
                    }
                }
            }
            Ok(RunAnswer::Refused41(..)) => no.push(fail("not_decided", "row 41".into())),
            Err((reason, d)) => no.push(fail(reason, d)),
        }
    }
    (files, no)
}

/// [`ImportRecords`] over an imported set: every supplied relation set's record at the key (its line read from the
/// source file and bound by its SHA-256, which the loader recorded).
pub fn import_records(
    set: &rustred_campaign_core::import::ImportedSet,
) -> impl Fn(&[i32]) -> Vec<(String, Vec<u8>)> + '_ {
    move |key: &[i32]| {
        let mut out = vec![];
        for m in set.supplied_info.values() {
            let Some(i) = m.get(key) else { continue };
            let Ok(f) = std::fs::File::open(&i.path) else {
                continue;
            };
            for l in std::io::BufReader::new(f)
                .split(b'\n')
                .map_while(Result::ok)
            {
                if crate::adapters::job::sha256_hex(&l) == i.record_sha256 {
                    out.push((i.record_sha256.clone(), l));
                    break;
                }
            }
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn capacity_refusals_are_typed_apart() {
        assert!(is_capacity_refusal(
            "SYMROW_REFUSED: expansion above 5,000,000 terms (symrow e3#0 [..])"
        ));
        assert!(is_capacity_refusal(
            "SYMROW_REFUSED: coefficient beyond i64"
        ));
        assert!(!is_capacity_refusal(
            "SYMROW_REFUSED: loop part not unimodular"
        ));
        assert_eq!(VERDICTS.len(), 7);
    }
}
