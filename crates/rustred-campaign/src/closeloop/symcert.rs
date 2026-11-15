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

/// The import's evidence hook from verdicts: (producer record SHA-256, key) -> certified / refused.
pub fn certificates_for_import(
    v: &[RelationVerdict],
) -> rustred_campaign_core::symcert::SymmetryCertificates {
    let mut m = rustred_campaign_core::symcert::SymmetryCertificates::default();
    for x in v {
        let Some(sha) = &x.producer_record_sha256 else {
            continue;
        };
        let Ok(key) = x
            .key
            .iter()
            .map(|&k| i32::try_from(k))
            .collect::<Result<Vec<i32>, _>>()
        else {
            continue;
        };
        match x.verdict.as_str() {
            "certified" => m.certify(sha, key, x.certificate_sha256.clone().unwrap_or_default()),
            "kernel_refused" => m.refuse(sha, key, x.detail.clone()),
            _ => {}
        }
    }
    m
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
