//! Kind `tadic-replay` (lane tadicjob; interfaces/tadic-job.md): helper VH 2's frozen t-adic replay driver
//! (`python3 -I tadic_job.py run JOB.json RESULT.json`, schemas `helper-vh2.tadic-job.v1` / `helper-vh2.tadic-result.v1`)
//! as a LEGACY DIAGNOSTIC child. The supervisor writes the driver's job file as the pinned input `tadic_job`; the driver
//! writes `out/tadic_result.json` in every case (exit 0 OK / 1 PARTIAL / 2 invalid or unsupported / 3 engine or
//! resource failure). The supervisor-side wrapper binds the result to the job (sha256 of the job bytes, labels, binary
//! and driver digests), re-derives every answer from the per-prime series (nothing the driver concludes is read), maps
//! the driver's exit and error codes to the contract (rows 50 / 51 / 52, 9, 10, 17) and seals one line per request.
//!
//! A request's outcome is a SERIES (verified terms) or NOT_DECIDED (a typed reason): LENGTH_BUDGET, PARTIAL, UNKNOWN_KEY,
//! UNSUPPORTED, UNDECIDED_PRECISION, ... are "not decided (reason)", never zero and never "no pole".
//!
//! Everything here is pure (bytes / JSON in, verdicts out); SHA-256 digests are computed by the caller (the adapter uses
//! the campaign core's implementation). Generic (L8): keys, masters, primes, seeds, orders and paths are data.

use crate::spec::JobSpec;
use serde_json::{Value, json};
use std::collections::{BTreeMap, BTreeSet};

/// The driver's job schema.
pub const JOB_SCHEMA: &str = "helper-vh2.tadic-job.v1";
/// The driver's result schema.
pub const RESULT_SCHEMA: &str = "helper-vh2.tadic-result.v1";
/// Job / result schemas v2 (frozen 083d2f6c, TADIC_DESIGN 13.11): `requests[].weight_vectors` (m combinations of the
/// request's inputs from ONE recording; result entries `<label>#<j>`, j = 0..m-1), `primes.record_per_seed`,
/// `limits.release_every`; one recording for all seeds (certificate block with `variants` per seed and vector).
pub const JOB_SCHEMA_V2: &str = "helper-vh2.tadic-job.v2";
pub const RESULT_SCHEMA_V2: &str = "helper-vh2.tadic-result.v2";
/// The pinned job file in `in/` (role `tadic_job`).
pub const JOB_INPUT: &str = "tadic_job.json";
/// The driver's result file in `out/`.
pub const CHILD_OUT: &str = "tadic_result.json";
/// The word of an OUTCOME error when a pinned input is not the spec's (row 52).
pub const INPUT_MISMATCH: &str = "TADIC_INPUT_MISMATCH";
/// `type` of a sealed request line.
pub const REQUEST_LINE: &str = "tadic-request";
/// Input roles of the kind.
pub const ROLE_JOB: &str = "tadic_job";
pub const ROLE_DRIVER: &str = "driver";

/// Driver error codes that are a TYPED engine answer for the whole job (every request NOT_DECIDED(code)); the job is
/// complete (row 50), never a crash.
pub const ENGINE_TYPED: [&str; 11] = [
    "LENGTH_BUDGET",
    "PRECISION_FAILURE",
    "PRIMES_EXHAUSTED",
    "TABLE_UNCOVERED",
    "TABLE_CYCLE",
    "TABLE_INVALID",
    "TABLE_EXPRESSION",
    "RULE_POLE",
    "ERASURE_FAILED",
    "EXACT_ALGEBRA",
    "UNSUPPORTED_RECORDS",
];
/// Driver error codes that refuse the job's inputs (contract exit 41, row 52).
pub const INPUT_REFUSALS: [&str; 6] = [
    "JOB_INVALID",
    "UNSUPPORTED",
    "INPUT_MISSING",
    "INPUT_HASH_MISMATCH",
    "BINARY_MISSING",
    "BINARY_HASH_MISMATCH",
];

/// The legacy argv: `-I <driver> run <RUN>/in/tadic_job.json <RUN>/out/tadic_result.json` (the program file is the
/// Python interpreter; the driver is the pinned input `driver`).
pub fn argv(spec: &JobSpec) -> Result<Vec<String>, String> {
    let input = |role: &str| {
        spec.inputs
            .iter()
            .find(|i| i.role == role)
            .map(|i| spec.input_path(i).display().to_string())
            .ok_or(format!("no {role} input"))
    };
    Ok(vec![
        "-I".into(),
        input(ROLE_DRIVER)?,
        "run".into(),
        input(ROLE_JOB)?,
        format!("{}/out/{CHILD_OUT}", spec.run_dir),
    ])
}

/// The answer of one output term, re-derived from its per-prime series (REVIEW_LEMMAS C5 / C6).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Answer {
    /// A nonzero residue at order -j < 0 (a PROVED lower bound j of the pole order).
    Pole(u64),
    /// The first nonzero residue is at an order >= 0 (mod the primes used).
    NoPoleModP,
    /// No nonzero residue below the precision (mod the primes used).
    ZeroModPToPrecision,
}

impl Answer {
    pub fn word(&self) -> &'static str {
        match self {
            Answer::Pole(_) => "POLE",
            Answer::NoPoleModP => "NO_POLE_MOD_P",
            Answer::ZeroModPToPrecision => "ZERO_MOD_P_TO_PRECISION",
        }
    }
}

/// One prime's series of one term: residues of orders `valuation .. precision-1` (empty when `valuation` is None).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PerPrime {
    pub prime: u64,
    pub seed: u64,
    pub valuation: Option<i64>,
    pub precision: i64,
    pub coefficients: Vec<u64>,
}

impl PerPrime {
    /// The residue of order `k` (0 below the valuation; None at or above the precision).
    pub fn coefficient(&self, k: i64) -> Option<u64> {
        if k >= self.precision {
            return None;
        }
        match self.valuation {
            None => Some(0),
            Some(v) if k < v => Some(0),
            Some(v) => self.coefficients.get((k - v) as usize).copied(),
        }
    }
}

/// One output term (master / residual / unit / exit) with its series per prime and the re-derived answer.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Term {
    pub basis: String,
    pub tag: String,
    pub kind: String,
    pub per_prime: Vec<PerPrime>,
    pub valuation: Option<i64>,
    pub precision: i64,
    pub answer: Answer,
}

/// A request's typed outcome.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RequestOutcome {
    /// Verified terms (possibly none: the request reduced to zero).
    Series {
        terms: Vec<Term>,
        max_pole_order_lower_bound: u64,
        precision: i64,
    },
    /// Not decided, with the typed reason: never zero, never "no pole".
    NotDecided { code: String, message: String },
}

impl RequestOutcome {
    pub fn is_decided(&self) -> bool {
        matches!(self, RequestOutcome::Series { .. })
    }
    /// "series" or "not decided (CODE)".
    pub fn word(&self) -> String {
        match self {
            RequestOutcome::Series { .. } => "series".into(),
            RequestOutcome::NotDecided { code, .. } => format!("not decided ({code})"),
        }
    }
}

/// One request of a checked result.
#[derive(Clone, Debug, PartialEq)]
pub struct RequestResult {
    pub label: String,
    pub outcome: RequestOutcome,
    /// The driver's certificate blocks (one per seed), as written.
    pub certificates: Vec<Value>,
}

/// A result bound to its job and checked structurally (C1).
#[derive(Clone, Debug, PartialEq)]
pub struct CheckedResult {
    pub job_id: String,
    pub job_sha256: String,
    pub status: String,
    pub error_code: Option<String>,
    pub level: String,
    pub d0: i64,
    pub order: i64,
    pub seeds: Vec<u64>,
    pub primes: Vec<u64>,
    pub binary_sha256: Option<String>,
    pub driver_sha256: Option<String>,
    pub requests: Vec<RequestResult>,
}

/// What the result must be bound to (the adapter's own values, never read from the result).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Pins {
    /// SHA-256 of the pinned job file's bytes.
    pub job_sha256: String,
    /// SHA-256 of the frozen binary the job pins (`binary.sha256`).
    pub binary_sha256: String,
    /// SHA-256 of the frozen driver.
    pub driver_sha256: String,
}

fn s<'a>(v: &'a Value, k: &str) -> Result<&'a str, String> {
    v[k].as_str().ok_or(format!("{k}: not a string"))
}

fn dec_u64(v: &Value, what: &str) -> Result<u64, String> {
    match v {
        Value::String(t) => t.parse::<u64>().map_err(|e| format!("{what}: {e}")),
        Value::Number(n) => n.as_u64().ok_or(format!("{what}: not a u64")),
        _ => Err(format!("{what}: not a decimal")),
    }
}

/// The job's request labels in order (the job file the adapter wrote).
pub fn job_labels(job: &Value) -> Result<Vec<String>, String> {
    let mut out = vec![];
    for r in job["requests"].as_array().ok_or("job: requests")? {
        let label = r["label"].as_str().ok_or("job: label")?;
        // v2: m weight vectors -> m result entries `<label>#<j>`
        let m = match &r["weight_vectors"] {
            Value::Null => None,
            w => Some(
                w["count"]
                    .as_u64()
                    .or_else(|| w["vectors"].as_array().map(|a| a.len() as u64))
                    .ok_or("job: weight_vectors without count / vectors")?,
            ),
        };
        match m {
            None => out.push(label.to_string()),
            Some(m) => out.extend((0..m).map(|j| format!("{label}#{j}"))),
        }
    }
    Ok(out)
}

fn parse_term(t: &Value, primes: &BTreeSet<u64>, order: i64) -> Result<Term, String> {
    let basis = s(t, "basis")?.to_string();
    let at = |m: &str| format!("term {basis}: {m}");
    let tag = t["tag"].as_str().unwrap_or("").to_string();
    let kind = t["kind"].as_str().unwrap_or("").to_string();
    let mut per_prime = vec![];
    for pp in t["per_prime"].as_array().ok_or(at("per_prime"))? {
        let prime = dec_u64(&pp["prime"], "prime").map_err(|e| at(&e))?;
        if !primes.contains(&prime) {
            return Err(at(&format!(
                "prime {prime} is not among the result's primes"
            )));
        }
        let seed = pp["seed"].as_u64().ok_or(at("seed"))?;
        let valuation = match &pp["valuation"] {
            Value::Null => None,
            v => Some(v.as_i64().ok_or(at("valuation"))?),
        };
        let precision = pp["precision"].as_i64().ok_or(at("precision"))?;
        let coefficients = pp["coefficients"]
            .as_array()
            .ok_or(at("coefficients"))?
            .iter()
            .map(|c| dec_u64(c, "coefficient"))
            .collect::<Result<Vec<u64>, String>>()
            .map_err(|e| at(&e))?;
        if coefficients.iter().any(|&c| c >= prime) {
            return Err(at(&format!("a residue is not below its prime {prime}")));
        }
        match valuation {
            None if !coefficients.is_empty() => {
                return Err(at("coefficients without a valuation"));
            }
            Some(v) => {
                if v >= precision {
                    return Err(at(&format!("valuation {v} >= precision {precision}")));
                }
                if coefficients.len() as i64 != precision - v {
                    return Err(at(&format!(
                        "{} coefficients for orders {v}..{precision}",
                        coefficients.len()
                    )));
                }
                if coefficients[0] == 0 {
                    return Err(at("the coefficient at the valuation is zero"));
                }
            }
            None => {}
        }
        per_prime.push(PerPrime {
            prime,
            seed,
            valuation,
            precision,
            coefficients,
        });
    }
    if per_prime.is_empty() {
        return Err(at("no per-prime series"));
    }
    let precision = per_prime.iter().map(|p| p.precision).min().unwrap_or(order);
    let valuation = per_prime.iter().filter_map(|p| p.valuation).min();
    let answer = match valuation {
        Some(v) if v < 0 => Answer::Pole((-v) as u64),
        Some(_) => Answer::NoPoleModP,
        None => Answer::ZeroModPToPrecision,
    };
    Ok(Term {
        basis,
        tag,
        kind,
        per_prime,
        valuation,
        precision,
        answer,
    })
}

/// C1: bind the driver's result to the job and the pins and check its structure. Every answer is RE-DERIVED from the
/// per-prime series. `Err` = the result is not this job's or not readable (nothing is read: row 51).
pub fn check_result(job: &Value, result: &Value, pins: &Pins) -> Result<CheckedResult, String> {
    if result["schema"] != RESULT_SCHEMA && result["schema"] != RESULT_SCHEMA_V2 {
        return Err(format!(
            "schema {} is not {RESULT_SCHEMA} / {RESULT_SCHEMA_V2}",
            result["schema"]
        ));
    }
    if job["schema"] != JOB_SCHEMA && job["schema"] != JOB_SCHEMA_V2 {
        return Err("job schema".into());
    }
    let job_id = s(job, "job_id")?.to_string();
    if result["job_id"] != job["job_id"] {
        return Err("job_id is not the job's".into());
    }
    let job_sha256 = s(result, "job_sha256")?.to_string();
    if job_sha256 != pins.job_sha256 {
        return Err(format!(
            "job_sha256 {job_sha256} is not the pinned job's {}",
            pins.job_sha256
        ));
    }
    let status = s(result, "status")?.to_string();
    let labels = job_labels(job)?;
    let order = job["expansion"]["order"]
        .as_i64()
        .ok_or("job: expansion.order")?;
    let d0 = job["expansion"]["d0"].as_i64().ok_or("job: expansion.d0")?;
    let seeds: Vec<u64> = job["primes"]["seeds"]
        .as_array()
        .ok_or("job: primes.seeds")?
        .iter()
        .map(|x| x.as_u64().ok_or("job: seed".to_string()))
        .collect::<Result<_, _>>()?;
    let level = s(job, "level")?.to_string();
    let binary_sha256 = result["binary"]["sha256"].as_str().map(str::to_string);
    let driver_sha256 = result["driver"]["sha256"].as_str().map(str::to_string);
    if status == "ERROR" {
        let code = result["error"]["code"]
            .as_str()
            .ok_or("ERROR without error.code")?
            .to_string();
        let message = result["error"]["message"]
            .as_str()
            .unwrap_or("")
            .to_string();
        // a binary the driver checked must be the pinned one (an ERROR before the binary check has none)
        if let Some(b) = &binary_sha256
            && *b != pins.binary_sha256
            && code != "BINARY_HASH_MISMATCH"
        {
            return Err(format!(
                "binary.sha256 {b} is not the pinned {}",
                pins.binary_sha256
            ));
        }
        return Ok(CheckedResult {
            job_id,
            job_sha256,
            status,
            error_code: Some(code.clone()),
            level,
            d0,
            order,
            seeds,
            primes: vec![],
            binary_sha256,
            driver_sha256,
            requests: labels
                .into_iter()
                .map(|label| RequestResult {
                    label,
                    outcome: RequestOutcome::NotDecided {
                        code: code.clone(),
                        message: message.clone(),
                    },
                    certificates: vec![],
                })
                .collect(),
        });
    }
    if status != "OK" && status != "PARTIAL" {
        return Err(format!("status {status}"));
    }
    if result["level"].as_str() != Some(level.as_str()) {
        return Err("level is not the job's".into());
    }
    if binary_sha256.as_deref() != Some(pins.binary_sha256.as_str()) {
        return Err("binary.sha256 is not the pinned binary's".into());
    }
    if driver_sha256.as_deref() != Some(pins.driver_sha256.as_str()) {
        return Err("driver.sha256 is not the pinned driver's".into());
    }
    if result["expansion"]["order"].as_i64() != Some(order)
        || result["expansion"]["d0"].as_i64() != Some(d0)
    {
        return Err("expansion is not the job's".into());
    }
    let primes: Vec<u64> = result["primes"]
        .as_array()
        .ok_or("primes")?
        .iter()
        .map(|p| dec_u64(p, "prime"))
        .collect::<Result<_, _>>()?;
    let pset: BTreeSet<u64> = primes.iter().copied().collect();
    let reqs = result["requests"].as_array().ok_or("requests")?;
    if reqs.len() != labels.len() {
        return Err(format!(
            "{} requests for {} in the job",
            reqs.len(),
            labels.len()
        ));
    }
    let mut requests = vec![];
    let mut any_error = false;
    for (r, label) in reqs.iter().zip(&labels) {
        if r["label"].as_str() != Some(label.as_str()) {
            return Err(format!("request {label}: label is not the job's (order)"));
        }
        let certificates = r["certificate"].as_array().cloned().unwrap_or_default();
        let outcome = match r["status"].as_str() {
            Some("OK") => {
                let mut terms = vec![];
                for t in r["terms"]
                    .as_array()
                    .ok_or(format!("request {label}: terms"))?
                {
                    terms.push(
                        parse_term(t, &pset, order).map_err(|e| format!("request {label}: {e}"))?,
                    );
                }
                let precision = terms.iter().map(|t| t.precision).min().unwrap_or(order);
                let max_pole = terms
                    .iter()
                    .filter_map(|t| match t.answer {
                        Answer::Pole(j) => Some(j),
                        _ => None,
                    })
                    .max()
                    .unwrap_or(0);
                if precision < 0 && max_pole == 0 {
                    RequestOutcome::NotDecided {
                        code: "UNDECIDED_PRECISION".into(),
                        message: format!("precision {precision} < 0"),
                    }
                } else {
                    RequestOutcome::Series {
                        terms,
                        max_pole_order_lower_bound: max_pole,
                        precision,
                    }
                }
            }
            Some("ERROR") => {
                any_error = true;
                let code = r["error"]["code"]
                    .as_str()
                    .ok_or(format!("request {label}: error.code"))?;
                RequestOutcome::NotDecided {
                    code: if code == "LENGTH_BUDGET"
                        || code == "UNKNOWN_KEY"
                        || code == "UNSUPPORTED"
                    {
                        code.to_string()
                    } else {
                        format!("PARTIAL:{code}")
                    },
                    message: r["error"]["message"].as_str().unwrap_or("").to_string(),
                }
            }
            other => return Err(format!("request {label}: status {other:?}")),
        };
        requests.push(RequestResult {
            label: label.clone(),
            outcome,
            certificates,
        });
    }
    if (status == "PARTIAL") != any_error {
        return Err(format!("status {status} inconsistent with the requests"));
    }
    Ok(CheckedResult {
        job_id,
        job_sha256,
        status,
        error_code: None,
        level,
        d0,
        order,
        seeds,
        primes,
        binary_sha256,
        driver_sha256,
        requests,
    })
}

/// How the wrapper ends an attempt from the driver's exit and the checked result.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum WrapDecision {
    /// Seal the requests and write OUTCOME exit 0 (row 50).
    Seal,
    /// OUTCOME with this contract exit and no sealed stream (21 time, 22 memory: rows 9 / 10).
    Killed { exit: u8, why: String },
    /// Input refusal: OUTCOME exit 41 (row 52).
    Refused { why: String },
    /// Crash: OUTCOME exit 70 (row 17).
    Crashed { why: String },
    /// OUTCOME exit 0 without a sealed stream: nothing is read (row 51).
    Unverified { why: String },
}

/// The wrapper's decision (interfaces/tadic-job.md 2.1).
pub fn decide(driver_exit: i32, checked: &Result<CheckedResult, String>) -> WrapDecision {
    match (driver_exit, checked) {
        (0 | 1, Ok(c)) if c.status != "ERROR" => WrapDecision::Seal,
        (0 | 1, Ok(c)) => WrapDecision::Unverified {
            why: format!("exit {driver_exit} with status ERROR {:?}", c.error_code),
        },
        (0 | 1, Err(why)) => WrapDecision::Unverified { why: why.clone() },
        (2 | 3, Ok(c)) => {
            let code = c.error_code.clone().unwrap_or_default();
            if c.status != "ERROR" {
                WrapDecision::Crashed {
                    why: format!("exit {driver_exit} with status {}", c.status),
                }
            } else if ENGINE_TYPED.contains(&code.as_str()) && driver_exit == 3 {
                WrapDecision::Seal
            } else if code == "TIMEOUT" {
                WrapDecision::Killed {
                    exit: 21,
                    why: "driver TIMEOUT (limits.wall_seconds)".into(),
                }
            } else if code == "OOM_KILLED" {
                WrapDecision::Killed {
                    exit: 22,
                    why: "driver OOM_KILLED (limits.memory_gib)".into(),
                }
            } else if INPUT_REFUSALS.contains(&code.as_str()) {
                WrapDecision::Refused {
                    why: format!("{INPUT_MISMATCH}: driver {code}"),
                }
            } else {
                WrapDecision::Crashed {
                    why: format!("driver exit {driver_exit} {code}"),
                }
            }
        }
        (e, Err(why)) => WrapDecision::Crashed {
            why: format!("driver exit {e}, no bound result: {why}"),
        },
        (e, Ok(_)) => WrapDecision::Crashed {
            why: format!("driver exit {e}"),
        },
    }
}

/// The canonical JSON of a request's terms (input of `terms_sha256`): basis, tag, kind and per-prime series.
pub fn terms_json(terms: &[Term]) -> Value {
    Value::Array(
        terms
            .iter()
            .map(|t| {
                json!({"basis": t.basis, "tag": t.tag, "kind": t.kind,
                "per_prime": t.per_prime.iter().map(|p| json!({"prime": p.prime.to_string(), "seed": p.seed,
                    "valuation": p.valuation, "precision": p.precision,
                    "coefficients": p.coefficients.iter().map(|c| c.to_string()).collect::<Vec<_>>()})).collect::<Vec<_>>()})
            })
            .collect(),
    )
}

/// One sealed request line (interfaces/tadic-job.md 2.2); `terms_sha256` is computed by the caller from [`terms_json`].
pub fn request_line(r: &RequestResult, terms_sha256: Option<&str>) -> Value {
    match &r.outcome {
        RequestOutcome::Series {
            terms,
            max_pole_order_lower_bound,
            precision,
        } => {
            json!({"type": REQUEST_LINE, "label": r.label, "status": "SERIES", "terms": terms.len(),
            "terms_sha256": terms_sha256, "verdict": if *max_pole_order_lower_bound > 0 {"POLE"} else {"NO_POLE_MOD_P"},
            "max_pole_order_lower_bound": max_pole_order_lower_bound, "precision": precision})
        }
        RequestOutcome::NotDecided { code, .. } => {
            json!({"type": REQUEST_LINE, "label": r.label, "status": "NOT_DECIDED", "code": code})
        }
    }
}

/// S of the kind: the sealed lines (requests then end) against the pinned job (labels in order, job digest) and the
/// result file digest (BLAKE3, raw bytes). `Err` = not this job's.
pub fn check_sealed(
    lines: &[Vec<u8>],
    job: &Value,
    job_blake3: &str,
    result_blake3: &str,
) -> Result<Vec<Value>, String> {
    let (end, body) = lines.split_last().ok_or("results without records")?;
    let end: Value = serde_json::from_slice(end).map_err(|e| format!("end record: {e}"))?;
    if end["type"] != "end" || end["records"].as_u64() != Some(body.len() as u64) {
        return Err("end record does not count the requests".into());
    }
    if end["job_blake3"].as_str() != Some(job_blake3) {
        return Err("end record: job_blake3 is not the pinned job's".into());
    }
    if end["result_blake3"].as_str() != Some(result_blake3) {
        return Err("end record: result_blake3 is not the result file's".into());
    }
    let labels = job_labels(job)?;
    if labels.len() != body.len() {
        return Err(format!(
            "{} lines for {} requests",
            body.len(),
            labels.len()
        ));
    }
    let mut out = vec![];
    for (l, label) in body.iter().zip(&labels) {
        let v: Value = serde_json::from_slice(l).map_err(|e| format!("line: {e}"))?;
        if v["type"] != REQUEST_LINE || v["label"].as_str() != Some(label.as_str()) {
            return Err(format!("line of {label}: type / label"));
        }
        if !matches!(v["status"].as_str(), Some("SERIES" | "NOT_DECIDED")) {
            return Err(format!("line of {label}: status"));
        }
        out.push(v);
    }
    Ok(out)
}

/// The certificate checks C2-C6 (interfaces/tadic-job.md 3): no trust in the driver.
pub mod certificate {
    use super::*;

    /// A finding of the checker (each one withholds `certified` from its request or the job).
    #[derive(Clone, Debug, PartialEq, Eq)]
    pub enum Finding {
        /// C2: a request's certificate block for a seed lacks its digests.
        MissingDigest {
            label: String,
            seed: u64,
            what: String,
        },
        /// C3: the primes of one seed disagree on a term's valuation.
        PrimeDisagreement {
            label: String,
            basis: String,
            tag: String,
            seed: u64,
            valuations: Vec<Option<i64>>,
        },
        /// C4: the seeds disagree on a term's valuation (per seed: the agreed valuation of its primes).
        SeedDisagreement {
            label: String,
            basis: String,
            tag: String,
            valuations: Vec<(u64, Option<i64>)>,
        },
        /// C4: a prime is used by two seeds (the producers are not independent).
        SeedPrimesOverlap { prime: u64 },
        /// C4: fewer than two seeds (no second producer).
        SingleSeed,
        /// C5: a term's precision is below the job's order.
        PrecisionBelowOrder {
            label: String,
            basis: String,
            precision: i64,
        },
    }

    impl Finding {
        pub fn label(&self) -> Option<&str> {
            match self {
                Finding::MissingDigest { label, .. }
                | Finding::PrimeDisagreement { label, .. }
                | Finding::SeedDisagreement { label, .. }
                | Finding::PrecisionBelowOrder { label, .. } => Some(label),
                _ => None,
            }
        }
        pub fn code(&self) -> &'static str {
            match self {
                Finding::MissingDigest { .. } => "MISSING_DIGEST",
                Finding::PrimeDisagreement { .. } => "PRIME_DISAGREEMENT",
                Finding::SeedDisagreement { .. } => "SEED_DISAGREEMENT",
                Finding::SeedPrimesOverlap { .. } => "SEED_PRIMES_OVERLAP",
                Finding::SingleSeed => "SINGLE_SEED",
                Finding::PrecisionBelowOrder { .. } => "PRECISION_BELOW_ORDER",
            }
        }
        pub fn to_json(&self) -> Value {
            let mut v = match self {
                Finding::MissingDigest { label, seed, what } => {
                    json!({"label": label, "seed": seed, "what": what})
                }
                Finding::PrimeDisagreement {
                    label,
                    basis,
                    tag,
                    seed,
                    valuations,
                } => {
                    json!({"label": label, "basis": basis, "tag": tag, "seed": seed, "valuations": valuations})
                }
                Finding::SeedDisagreement {
                    label,
                    basis,
                    tag,
                    valuations,
                } => {
                    json!({"label": label, "basis": basis, "tag": tag, "valuations": valuations})
                }
                Finding::SeedPrimesOverlap { prime } => json!({"prime": prime.to_string()}),
                Finding::SingleSeed => json!({}),
                Finding::PrecisionBelowOrder {
                    label,
                    basis,
                    precision,
                } => {
                    json!({"label": label, "basis": basis, "precision": precision})
                }
            };
            v["code"] = json!(self.code());
            v
        }
    }

    /// The checker's report: `certified` per request (decided, no finding of its own, no job-level finding).
    #[derive(Clone, Debug, PartialEq, Eq)]
    pub struct CertReport {
        pub findings: Vec<Finding>,
        pub certified: BTreeMap<String, bool>,
    }

    impl CertReport {
        pub fn job_certified(&self) -> bool {
            self.certified.values().all(|&c| c)
        }
    }

    fn digests_present(cert: &Value, level: &str) -> Result<(), String> {
        let nonempty = |k: &str| match &cert[k] {
            Value::Array(a) => !a.is_empty(),
            Value::String(t) => !t.is_empty(),
            _ => false,
        };
        let need: &[&str] = if level == "node" {
            &["graph_digest", "plan_digest"]
        } else {
            &["trace_digests", "plan_digests"]
        };
        for k in need {
            if !nonempty(k) {
                return Err((*k).to_string());
            }
        }
        Ok(())
    }

    /// C2-C5 on a checked result.
    pub fn check(r: &CheckedResult) -> CertReport {
        let mut findings = vec![];
        let seeds: BTreeSet<u64> = r.seeds.iter().copied().collect();
        if seeds.len() < 2 {
            findings.push(Finding::SingleSeed);
        }
        // C4 primes of different seeds disjoint (from the per-prime series of every term)
        let mut prime_seed: BTreeMap<u64, u64> = BTreeMap::new();
        let mut overlap = BTreeSet::new();
        for q in &r.requests {
            if let RequestOutcome::Series { terms, .. } = &q.outcome {
                for t in terms {
                    for p in &t.per_prime {
                        match prime_seed.get(&p.prime) {
                            Some(&s0) if s0 != p.seed => {
                                overlap.insert(p.prime);
                            }
                            _ => {
                                prime_seed.insert(p.prime, p.seed);
                            }
                        }
                    }
                }
            }
        }
        findings.extend(
            overlap
                .into_iter()
                .map(|prime| Finding::SeedPrimesOverlap { prime }),
        );
        for q in &r.requests {
            let RequestOutcome::Series { terms, .. } = &q.outcome else {
                continue;
            };
            // C2 (v2: one block for all seeds when it lists a `variants` record for the seed)
            for seed in &seeds {
                let block = q.certificates.iter().find(|c| {
                    c["seed"].as_u64() == Some(*seed)
                        || c["variants"].as_array().is_some_and(|v| {
                            v.iter().any(|x| {
                                x["seed"].as_u64() == Some(*seed)
                                    || x["seed"].as_str() == Some(seed.to_string().as_str())
                            })
                        })
                });
                match block {
                    None => findings.push(Finding::MissingDigest {
                        label: q.label.clone(),
                        seed: *seed,
                        what: "certificate block".into(),
                    }),
                    Some(c) => {
                        if let Err(what) = digests_present(c, &r.level) {
                            findings.push(Finding::MissingDigest {
                                label: q.label.clone(),
                                seed: *seed,
                                what,
                            });
                        }
                    }
                }
            }
            for t in terms {
                // C5
                if t.precision < r.order {
                    findings.push(Finding::PrecisionBelowOrder {
                        label: q.label.clone(),
                        basis: t.basis.clone(),
                        precision: t.precision,
                    });
                }
                // C3 / C4: per seed the valuations of its primes (a seed without a series for the term: zero to
                // its precision, i.e. valuation None)
                let mut per_seed: Vec<(u64, Option<i64>)> = vec![];
                for seed in &seeds {
                    let vals: Vec<Option<i64>> = t
                        .per_prime
                        .iter()
                        .filter(|p| p.seed == *seed)
                        .map(|p| p.valuation)
                        .collect();
                    let set: BTreeSet<Option<i64>> = vals.iter().copied().collect();
                    if set.len() > 1 {
                        findings.push(Finding::PrimeDisagreement {
                            label: q.label.clone(),
                            basis: t.basis.clone(),
                            tag: t.tag.clone(),
                            seed: *seed,
                            valuations: vals.clone(),
                        });
                    }
                    // the smaller valuation is the true one when primes disagree (C6)
                    let v = vals.iter().filter_map(|x| *x).min();
                    per_seed.push((*seed, v));
                }
                let set: BTreeSet<Option<i64>> = per_seed.iter().map(|x| x.1).collect();
                if set.len() > 1 {
                    findings.push(Finding::SeedDisagreement {
                        label: q.label.clone(),
                        basis: t.basis.clone(),
                        tag: t.tag.clone(),
                        valuations: per_seed,
                    });
                }
            }
        }
        let job_level = findings.iter().any(|f| f.label().is_none());
        let certified = r
            .requests
            .iter()
            .map(|q| {
                let own = findings.iter().any(|f| f.label() == Some(q.label.as_str()));
                (
                    q.label.clone(),
                    q.outcome.is_decided() && !own && !job_level,
                )
            })
            .collect();
        CertReport {
            findings,
            certified,
        }
    }

    fn strip_volatile(v: &Value) -> Value {
        match v {
            Value::Object(m) => Value::Object(
                m.iter()
                    .filter(|(k, _)| {
                        !(k.contains("seconds")
                            || *k == "written_utc"
                            || *k == "argv"
                            || *k == "runs")
                    })
                    .map(|(k, x)| (k.clone(), strip_volatile(x)))
                    .collect(),
            ),
            Value::Array(a) => Value::Array(a.iter().map(strip_volatile).collect()),
            x => x.clone(),
        }
    }

    /// C6: a rerun of the same job gives identical primes, residues and certificate digests / histograms (timing,
    /// argv and run paths excluded). Returns the differing parts (empty = identical).
    pub fn compare_rerun(a: &Value, b: &Value) -> Vec<String> {
        let mut diffs = vec![];
        for k in ["job_sha256", "status", "primes", "unusable_primes"] {
            if a[k] != b[k] {
                diffs.push(k.to_string());
            }
        }
        let (ra, rb) = (
            a["requests"].as_array().cloned().unwrap_or_default(),
            b["requests"].as_array().cloned().unwrap_or_default(),
        );
        if ra.len() != rb.len() {
            diffs.push("requests".into());
            return diffs;
        }
        for (x, y) in ra.iter().zip(&rb) {
            let label = x["label"].as_str().unwrap_or("?");
            if x["terms"] != y["terms"] {
                diffs.push(format!("{label}: terms"));
            }
            if strip_volatile(&x["certificate"]) != strip_volatile(&y["certificate"]) {
                diffs.push(format!("{label}: certificate"));
            }
            if x["status"] != y["status"] || x["error"] != y["error"] {
                diffs.push(format!("{label}: status"));
            }
        }
        diffs
    }
}
