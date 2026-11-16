//! `rr_arrivals`: CERTIFIED ARRIVAL SETS per DAG edge parent -> child (lane arrivals; interfaces/arrivals.md).
//!
//! ```text
//! rr_arrivals prove-handdowns --tables T [..] --out P [--threads N] [--summary S] [--all-lines] [--max-planes K]
//! rr_arrivals verify-handdown-proofs --tables T [..] --proofs P [..] [--threads N] [--summary S]
//! rr_arrivals compute --spec PARENT_SPEC --rows ROWS --edge E [--edge E2 ..] --out-dir D --parent NAME
//!                     [--child-table LABEL=PATH ..] [--edge-producer TEXT] [--exact all|none|keys:FILE]
//!                     [--no-structural] [--threads N] [--index-dir DIR] [--decisions-sample N] [--cache-entries N]
//!                     [--maps M1,M2,..] [--dump-handdown-keys FILE]
//! rr_arrivals verify --spec PARENT_SPEC --rows ROWS --edge E [..] --arrivals A [..] [--sample N] [--threads N]
//!                    [--index-dir DIR]
//! rr_arrivals child-check (--services MANIFEST | --spec CHILD_SPEC) --statement F --arrivals NAME=A [..] [--closed-set NAME=FILE ..] --out REPORT --work-dir W
//!                    [--threads N] [--index-dir DIR]
//! rr_arrivals second-route --spec PARENT_SPEC --decisions D --facts F [..] --out REPORT [--index-dir DIR]
//! rr_arrivals compare-engine --edge E --facts ENGINE.keys.jsonl [..] [--out REPORT]
//! rr_arrivals closure-rows --spec NODE_SPEC --arrivals A [..] --out ROWS [--summary S] [--threads N] [--index-dir DIR]
//! rr_arrivals version
//! ```
//! Needs the Symbolica licence (`source WS/ops/env.sh`) for every command but `child-check` and `version`. Exit 0 = ok /
//! verified, 3 = a mismatch (verifiers), 2 = error. Nothing here names a node: parents, children, frames, edges are data.

#[path = "../arrivals.rs"]
mod arrivals;
#[path = "../coef_facts.rs"]
#[allow(dead_code)]
mod coef_facts;
#[path = "../zero_proofs.rs"]
#[allow(dead_code)]
mod zero_proofs;

use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::path::{Path, PathBuf};

use rustred_campaign_algebra::SymbolicaAlgebra;
use rustred_campaign_core::closure::CoefFact;
use rustred_campaign_core::import::arrivals::edge::EdgeFile;
use rustred_campaign_core::import::arrivals::format::{self, ArrivalSet};
use rustred_campaign_core::import::arrivals::rows::ParentRows;
use rustred_campaign_core::import::arrivals::{
    CHILD_CHECK_SCHEMA, canonical_json, child, parse_key,
};
use rustred_campaign_core::import::{Evidence, ImportedSet, LoadOptions, TableSetSpec, load_with};
use serde_json::{Value, json};

fn die(msg: &str) -> ! {
    println!("{}", json!({"verdict": "ERROR", "error": msg}));
    std::process::exit(2);
}

#[derive(Default)]
struct Args {
    tables: Vec<PathBuf>,
    proofs: Vec<PathBuf>,
    out: Option<PathBuf>,
    out_dir: Option<PathBuf>,
    summary: Option<PathBuf>,
    spec: Option<PathBuf>,
    rows: Option<PathBuf>,
    edges: Vec<PathBuf>,
    arrivals: Vec<String>,
    child_tables: Vec<String>,
    parent: Option<String>,
    statement: Option<PathBuf>,
    work_dir: Option<PathBuf>,
    index_dir: Option<PathBuf>,
    decisions: Option<PathBuf>,
    dump_handdown_keys: Option<PathBuf>,
    facts: Vec<PathBuf>,
    edge_producer: String,
    exact: String,
    map: String,
    maps: Vec<String>,
    threads: usize,
    sample: usize,
    decisions_sample: usize,
    cache_entries: usize,
    max_planes: usize,
    all_lines: bool,
    /// compute: track the CHAIN DEPTH (box keys 0, arriving keys the min depth of their `--arrivals` lines).
    depth: bool,
    /// compute: report the closure path of these keys (`--trace-key '[..]'`); `--closure-only` stops after the closure.
    trace_keys: Vec<Vec<i32>>,
    closure_only: bool,
    /// compute: write the exact closure keys (depth, kind) to this file (with `.uncovered.jsonl` beside it).
    closure_out: Option<PathBuf>,
    /// compute (streamed): `--shard I/K`
    shard: Option<(usize, usize)>,
    /// arrivals-bin: byte-level extraction of key and depth (canonical lines), checked by order, count and file sha256
    fast: bool,
    closure_chunk: usize,
    local_flush: usize,
    no_structural: bool,
    structural_min_keys: usize,
    decide_all_handdowns: bool,
    closed_sets: Vec<String>,
    services: Option<PathBuf>,
    in_memory: bool,
    terms: Option<PathBuf>,
    terms_out: Option<PathBuf>,
    masks: Vec<String>,
    record_budget_mb: usize,
    map_budget: usize,
    spill_dir: Option<PathBuf>,
}

fn parse(args: &[String]) -> Args {
    let mut a = Args {
        threads: 4,
        exact: "all".into(),
        map: "chosen".into(),
        cache_entries: 20_000_000,
        max_planes: 64,
        record_budget_mb: 2048,
        map_budget: 60_000_000,
        ..Default::default()
    };
    let mut i = 0;
    let val = |i: &mut usize| -> String {
        *i += 1;
        args.get(*i)
            .cloned()
            .unwrap_or_else(|| die(&format!("{} needs a value", args[*i - 1])))
    };
    let num = |s: String| -> usize {
        s.parse()
            .unwrap_or_else(|_| die(&format!("not a number: {s}")))
    };
    while i < args.len() {
        match args[i].as_str() {
            "--tables" => a.tables.push(val(&mut i).into()),
            "--proofs" => a.proofs.push(val(&mut i).into()),
            "--out" => a.out = Some(val(&mut i).into()),
            "--out-dir" => a.out_dir = Some(val(&mut i).into()),
            "--summary" => a.summary = Some(val(&mut i).into()),
            "--spec" => a.spec = Some(val(&mut i).into()),
            "--rows" => a.rows = Some(val(&mut i).into()),
            "--edge" => a.edges.push(val(&mut i).into()),
            "--arrivals" => a.arrivals.push(val(&mut i)),
            "--child-table" => a.child_tables.push(val(&mut i)),
            "--parent" => a.parent = Some(val(&mut i)),
            "--statement" => a.statement = Some(val(&mut i).into()),
            "--work-dir" => a.work_dir = Some(val(&mut i).into()),
            "--index-dir" => a.index_dir = Some(val(&mut i).into()),
            "--decisions" => a.decisions = Some(val(&mut i).into()),
            "--dump-handdown-keys" => a.dump_handdown_keys = Some(val(&mut i).into()),
            "--facts" => a.facts.push(val(&mut i).into()),
            "--edge-producer" => a.edge_producer = val(&mut i),
            "--exact" => a.exact = val(&mut i),
            "--map" => a.map = val(&mut i),
            "--maps" => a.maps = val(&mut i).split(',').map(str::to_string).collect(),
            "--threads" => a.threads = num(val(&mut i)).max(1),
            "--sample" => a.sample = num(val(&mut i)),
            "--decisions-sample" => a.decisions_sample = num(val(&mut i)),
            "--cache-entries" => a.cache_entries = num(val(&mut i)),
            "--max-planes" => a.max_planes = num(val(&mut i)),
            "--all-lines" => a.all_lines = true,
            "--depth" => a.depth = true,
            "--trace-key" => a.trace_keys.push(
                serde_json::from_str(&val(&mut i))
                    .unwrap_or_else(|e| die(&format!("--trace-key: {e}"))),
            ),
            "--closure-only" => a.closure_only = true,
            "--fast" => a.fast = true,
            "--closure-chunk" => a.closure_chunk = num(val(&mut i)),
            "--local-flush" => a.local_flush = num(val(&mut i)),
            "--closure-out" => a.closure_out = Some(PathBuf::from(val(&mut i))),
            "--shard" => {
                let v = val(&mut i);
                let (x, y) = v.split_once('/').unwrap_or_else(|| die("--shard I/K"));
                let (x, y): (usize, usize) = (num(x.to_string()), num(y.to_string()));
                if y == 0 || x >= y {
                    die("--shard I/K with 0 <= I < K");
                }
                a.shard = Some((x, y));
            }
            "--no-structural" => a.no_structural = true,
            "--structural-min-keys" => {
                a.structural_min_keys = val(&mut i)
                    .parse()
                    .unwrap_or_else(|e| die(&format!("--structural-min-keys: {e}")))
            }
            "--decide-all-handdowns" => a.decide_all_handdowns = true,
            "--closed-set" => a.closed_sets.push(val(&mut i)),
            "--services" => a.services = Some(val(&mut i).into()),
            "--in-memory" => a.in_memory = true,
            "--terms" => a.terms = Some(val(&mut i).into()),
            "--terms-out" => a.terms_out = Some(val(&mut i).into()),
            "--masks" => a.masks = val(&mut i).split(',').map(str::to_string).collect(),
            "--record-budget-mb" => a.record_budget_mb = num(val(&mut i)),
            "--map-budget" => a.map_budget = num(val(&mut i)),
            "--spill-dir" => a.spill_dir = Some(val(&mut i).into()),
            x => die(&format!("unknown argument {x}")),
        }
        i += 1;
    }
    a
}

fn algebra() -> &'static SymbolicaAlgebra {
    let a = SymbolicaAlgebra::new().unwrap_or_else(|e| die(&format!("UNSUPPORTED: {e}")));
    arrivals::check_primes().unwrap_or_else(|e| die(&e));
    Box::leak(Box::new(a))
}

fn file_sha256(p: &Path) -> Result<(String, u64), String> {
    use std::io::Read;
    let mut f = std::fs::File::open(p).map_err(|e| format!("{}: {e}", p.display()))?;
    let mut h = rustred_campaign_core::closure::sha256::Sha256::new();
    let mut buf = vec![0u8; 1 << 22];
    let mut n = 0u64;
    loop {
        let k = f
            .read(&mut buf)
            .map_err(|e| format!("{}: {e}", p.display()))?;
        if k == 0 {
            break;
        }
        h.update(&buf[..k]);
        n += k as u64;
    }
    Ok((h.finish_hex(), n))
}

fn write_json(p: &Path, v: &Value) {
    let tmp = p.with_extension("part");
    std::fs::write(
        &tmp,
        format!("{}\n", serde_json::to_string_pretty(v).unwrap_or_default()),
    )
    .unwrap_or_else(|e| die(&format!("{}: {e}", tmp.display())));
    std::fs::rename(&tmp, p).unwrap_or_else(|e| die(&format!("{}: {e}", p.display())));
}

/// Byte offsets of every line of a table file that may be a rule record (same scan as rr_coef_facts).
fn rule_offsets(p: &Path) -> Result<Vec<u64>, String> {
    use std::io::Read;
    let mut f = std::fs::File::open(p).map_err(|e| format!("{}: {e}", p.display()))?;
    let mut buf = vec![0u8; 1 << 22];
    let (mut off, mut start, mut head): (u64, u64, Vec<u8>) = (0, 0, vec![]);
    let mut out = vec![];
    loop {
        let n = f
            .read(&mut buf)
            .map_err(|e| format!("{}: {e}", p.display()))?;
        if n == 0 {
            break;
        }
        for &c in &buf[..n] {
            if c == b'\n' {
                if head.windows(6).any(|w| w == b"\"rule\"") {
                    out.push(start);
                }
                head.clear();
                start = off + 1;
            } else if head.len() < 256 {
                head.push(c);
            }
            off += 1;
        }
    }
    if !head.is_empty() && head.windows(6).any(|w| w == b"\"rule\"") {
        out.push(start);
    }
    Ok(out)
}

fn tables_header(tables: &[PathBuf]) -> Value {
    let mut list = vec![];
    let mut names = vec![];
    for tp in tables {
        let (sha, bytes) = file_sha256(tp).unwrap_or_else(|e| die(&e));
        names.push((tp.display().to_string(), sha.clone()));
        list.push(json!({"path": tp, "sha256": sha, "bytes": bytes}));
    }
    names.sort();
    let mut h = rustred_campaign_core::closure::sha256::Sha256::new();
    for (p, s) in &names {
        h.update(format!("{p}\t{s}\n").as_bytes());
    }
    json!({"schema": zero_proofs::HANDDOWN_HEADER_SCHEMA, "tables": list, "tables_digest": h.finish_hex(),
        "tool": zero_proofs::tool(), "method": zero_proofs::HANDDOWN_METHOD, "method_doc": zero_proofs::HANDDOWN_METHOD_DOC})
}

fn prover_cfg(a: &Args) -> zero_proofs::ProverCfg {
    zero_proofs::ProverCfg {
        max_planes: a.max_planes,
        ..Default::default()
    }
}

/// Every rule record of `tp` in parallel: `work(offset)` -> lines; returned in record order.
fn per_record<R: Send>(
    tp: &Path,
    threads: usize,
    work: impl Fn(u64) -> R + Sync,
) -> Result<Vec<R>, String> {
    use std::sync::Mutex;
    use std::sync::atomic::{AtomicUsize, Ordering};
    let offs = rule_offsets(tp)?;
    let next = AtomicUsize::new(0);
    let out: Vec<Mutex<Option<R>>> = offs.iter().map(|_| Mutex::new(None)).collect();
    std::thread::scope(|s| {
        for _ in 0..threads.max(1) {
            s.spawn(|| {
                loop {
                    let i = next.fetch_add(1, Ordering::Relaxed);
                    let Some(&o) = offs.get(i) else { break };
                    let r = work(o);
                    *out[i].lock().expect("lock") = Some(r);
                }
            });
        }
    });
    Ok(out
        .into_iter()
        .filter_map(|m| m.into_inner().expect("lock"))
        .collect())
}

type RecordLines = Result<Option<(Vec<Value>, zero_proofs::HandDownCounts)>, String>;
/// A re-derived record of the hand-down verifier: (record sha256, lines, lines, independent failures).
type RederivedRecord = (
    String,
    usize,
    Vec<Value>,
    Vec<String>,
    Vec<(usize, Value, bool)>,
);
/// A key set (support, exact) per child of the multi-map union.
type KeySetB = BTreeSet<Vec<i32>>;
/// An expansion with i128 coefficients (for the independent identity check).
type ImageI128 = Vec<(Vec<i32>, (i128, i128))>;

fn prove_record(
    alg: &SymbolicaAlgebra,
    cfg: &zero_proofs::ProverCfg,
    tp: &Path,
    off: u64,
    all: bool,
    only: Option<&TermFilter>,
) -> RecordLines {
    let (frame, _) = coef_facts::table_frame(tp)?;
    let (line, sha) = coef_facts::line_at(tp, off)?;
    let terms = match only {
        Some(f) => match f.get(&sha) {
            Some(s) => Some(s),
            None => return Ok(None),
        },
        None => None,
    };
    let rt = match rustred_campaign_core::accept::rule::RuleText::parse(&line, &frame) {
        Ok(rt) => rt,
        Err(rustred_campaign_core::accept::rule::RuleTextError::NotARule(_)) => return Ok(None),
        Err(e) => return Err(format!("{}@{off}: {e:?}", tp.display())),
    };
    let mut c = zero_proofs::HandDownCounts::default();
    let mut p = zero_proofs::RecordProver::new(alg, cfg.clone(), &rt, frame.mask())
        .map_err(|e| format!("{}@{off}: {e}", tp.display()))?;
    let lines = zero_proofs::prove_handdown_record_terms(&mut p, &mut c, all, terms);
    Ok(Some((lines, c)))
}

/// record sha256 -> the terms to prove / verify (`--terms FILE`: JSON lines `{record_sha256, term}`).
type TermFilter = HashMap<String, BTreeSet<usize>>;

fn term_filter(a: &Args) -> Option<TermFilter> {
    let p = a.terms.as_ref()?;
    let text = std::fs::read_to_string(p).unwrap_or_else(|e| die(&format!("{}: {e}", p.display())));
    let mut m: TermFilter = HashMap::new();
    for (i, l) in text
        .lines()
        .enumerate()
        .filter(|(_, l)| !l.trim().is_empty())
    {
        let v: Value = serde_json::from_str(l)
            .unwrap_or_else(|e| die(&format!("{} line {}: {e}", p.display(), i + 1)));
        let sha = v["record_sha256"]
            .as_str()
            .unwrap_or_else(|| die(&format!("{} line {}: record_sha256", p.display(), i + 1)));
        let j = v["term"]
            .as_u64()
            .unwrap_or_else(|| die(&format!("{} line {}: term", p.display(), i + 1)));
        m.entry(sha.to_string()).or_default().insert(j as usize);
    }
    Some(m)
}

/// `prove-handdowns`: every hand-down locus of every rule record of the tables.
fn prove(a: &Args) {
    use std::io::Write;
    if a.tables.is_empty() {
        die("--tables missing");
    }
    let out = a.out.clone().unwrap_or_else(|| die("--out missing"));
    let t0 = std::time::Instant::now();
    let alg = algebra();
    let cfg = prover_cfg(a);
    let header = tables_header(&a.tables);
    let part = out.with_extension("part");
    let mut w = std::io::BufWriter::new(
        std::fs::File::create(&part).unwrap_or_else(|e| die(&format!("{}: {e}", part.display()))),
    );
    writeln!(w, "{}", canonical_json(&header)).unwrap_or_else(|e| die(&e.to_string()));
    let mut total = zero_proofs::HandDownCounts::default();
    let mut errors = vec![];
    let only = term_filter(a);
    for tp in &a.tables {
        let res = per_record(tp, a.threads, |off| {
            prove_record(alg, &cfg, tp, off, a.all_lines, only.as_ref())
        })
        .unwrap_or_else(|e| die(&e));
        for r in res {
            match r {
                Ok(Some((lines, c))) => {
                    total.add(&c);
                    for l in lines {
                        writeln!(w, "{}", canonical_json(&l))
                            .unwrap_or_else(|e| die(&e.to_string()));
                    }
                }
                Ok(None) => {}
                Err(e) => {
                    total.records_refused += 1;
                    errors.push(e);
                }
            }
        }
    }
    w.flush().unwrap_or_else(|e| die(&e.to_string()));
    drop(w);
    std::fs::rename(&part, &out).unwrap_or_else(|e| die(&format!("{}: {e}", out.display())));
    let (osha, obytes) = file_sha256(&out).unwrap_or_else(|e| die(&e));
    let s = json!({"schema": "rustred.campaign.handdown-proof-summary.v1", "tables_digest": header["tables_digest"],
        "out": out, "out_sha256": osha, "out_bytes": obytes, "threads": a.threads, "seconds": t0.elapsed().as_secs_f64(),
        "counts": total, "errors": errors.len(), "first_errors": errors.iter().take(20).collect::<Vec<_>>(),
        "tool": zero_proofs::tool()});
    if let Some(p) = &a.summary {
        write_json(p, &s);
    }
    println!("{s}");
    if !errors.is_empty() {
        std::process::exit(2);
    }
}

/// The comparable identity of a hand-down line: (verdict word, class word).
fn claim_of(v: &Value) -> (String, String) {
    let verdict = v["verdict"].as_str().unwrap_or("").to_string();
    let class = match verdict.as_str() {
        "zero_on_locus" => v["denominator"].as_str().unwrap_or("").to_string(),
        "nonzero_on_locus" => format!(
            "{}/{}",
            v["numerator"].as_str().unwrap_or(""),
            v["denominator"].as_str().unwrap_or("")
        ),
        _ => String::new(),
    };
    (verdict, class)
}

/// `verify-handdown-proofs`: re-derive every hand-down locus from the table bytes and compare; re-check proved zero
/// numerators and constant-d-coefficient non-zero numerators INDEPENDENTLY with the core's exact polynomial.
fn verify_proofs(a: &Args) {
    use std::io::BufRead;
    if a.tables.is_empty() || a.proofs.is_empty() {
        die("--tables and --proofs are required");
    }
    let t0 = std::time::Instant::now();
    let alg = algebra();
    let cfg = prover_cfg(a);
    let header = tables_header(&a.tables);
    type Key = (String, u64, String);
    let mut claims: HashMap<Key, Value> = HashMap::new();
    let mut mismatches: Vec<String> = vec![];
    for p in &a.proofs {
        let f = std::fs::File::open(p).unwrap_or_else(|e| die(&format!("{}: {e}", p.display())));
        for (i, l) in std::io::BufReader::new(f).lines().enumerate() {
            let l = l.unwrap_or_else(|e| die(&e.to_string()));
            let v: Value = serde_json::from_str(&l)
                .unwrap_or_else(|e| die(&format!("{} line {}: {e}", p.display(), i + 1)));
            if v["schema"] == zero_proofs::HANDDOWN_HEADER_SCHEMA {
                if v["tables_digest"] != header["tables_digest"] {
                    mismatches.push(format!(
                        "{}: tables_digest {} != {}",
                        p.display(),
                        v["tables_digest"],
                        header["tables_digest"]
                    ));
                }
                continue;
            }
            let k = (
                v["record_sha256"].as_str().unwrap_or("").to_string(),
                v["term"].as_u64().unwrap_or(u64::MAX),
                canonical_json(&v["locus"]),
            );
            if claims.insert(k.clone(), v).is_some() {
                mismatches.push(format!("duplicate claim {k:?}"));
            }
        }
    }
    let n_claims = claims.len();
    let mut rederived = 0u64;
    let mut independent = 0u64;
    let mut excl_ok: Vec<String> = vec![];
    let mut excl_unconfirmed = 0u64;
    let only = term_filter(a);
    for tp in &a.tables {
        let res = per_record(
            tp,
            a.threads,
            |off| -> Result<Option<RederivedRecord>, String> {
                let (frame, _) = coef_facts::table_frame(tp)?;
                let (line, sha) = coef_facts::line_at(tp, off)?;
                let terms = match only.as_ref() {
                    Some(f) => match f.get(&sha) {
                        Some(s) => Some(s),
                        None => return Ok(None),
                    },
                    None => None,
                };
                let rt = match rustred_campaign_core::accept::rule::RuleText::parse(&line, &frame) {
                    Ok(rt) => rt,
                    Err(rustred_campaign_core::accept::rule::RuleTextError::NotARule(_)) => {
                        return Ok(None);
                    }
                    Err(e) => return Err(format!("{}@{off}: {e:?}", tp.display())),
                };
                let mut c = zero_proofs::HandDownCounts::default();
                let mut p = zero_proofs::RecordProver::new(alg, cfg.clone(), &rt, frame.mask())?;
                let lines =
                    zero_proofs::prove_handdown_record_terms(&mut p, &mut c, a.all_lines, terms);
                // independent re-checks of the re-derived claims
                let mut bad = vec![];
                let mut excl: Vec<(usize, Value, bool)> = vec![];
                for v in &lines {
                    let eqs = v["equations"].as_array().is_some_and(|e| !e.is_empty());
                    let j = v["term"].as_u64().unwrap_or(0) as usize;
                    let locus =
                        rustred_campaign_core::import::zero_proofs::Locus::from_json(&v["locus"])
                            .map_err(|e| e.to_string())?;
                    let subs = p.substitutions(locus);
                    let (num, _) = p.side_texts(j)?;
                    // (claim checked independently?, failure text)
                    let failed = match v["verdict"].as_str() {
                        Some("zero_on_locus") if !eqs => {
                            (!zero_proofs::core_vanishes(&num, p.arity(), &subs)?)
                                .then_some("does not vanish")
                        }
                        Some("nonzero_on_locus")
                            if !eqs && v["numerator"] == "constant_d_coefficient" =>
                        {
                            (!zero_proofs::core_constant_d_coefficient(&num, p.arity(), &subs)?)
                                .then_some("has no constant d-coefficient")
                        }
                        _ => None,
                    };
                    if let Some(f) = failed {
                        bad.push(format!("independent: term {j} {} {f}", v["locus"]));
                    }
                    // an EXCLUDED locus, re-checked with the core's exact polynomial on the record's own text: the plane's
                    // parameter is fixed to another value, or every polynomial of some exception branch vanishes on it, or a
                    // case equation becomes a non-zero constant there
                    if v["verdict"] == "excluded" {
                        let rec: Value = serde_json::from_slice(&line)
                            .map_err(|e| format!("record json: {e}"))?;
                        let mut ok = false;
                        if let rustred_campaign_core::import::zero_proofs::Locus::Plane {
                            param,
                            value,
                        } = locus
                            && let Some(c) =
                                rec["fixed"].get(param as usize).and_then(Value::as_i64)
                        {
                            ok = c != value;
                        }
                        if !ok {
                            for b in rec["exceptions"].as_array().into_iter().flatten() {
                                let texts: Vec<&str> = b
                                    .as_array()
                                    .into_iter()
                                    .flatten()
                                    .filter_map(Value::as_str)
                                    .collect();
                                if !texts.is_empty()
                                    && texts.iter().all(|t| {
                                        zero_proofs::core_vanishes(t, p.arity(), &subs)
                                            .unwrap_or(false)
                                    })
                                {
                                    ok = true;
                                    break;
                                }
                            }
                        }
                        if !ok {
                            for e in rec["equations"]
                                .as_array()
                                .into_iter()
                                .flatten()
                                .filter_map(Value::as_str)
                            {
                                if let Ok(m) = zero_proofs::core_substitute(e, p.arity(), &subs)
                                    && !m.is_empty()
                                    && m.iter().all(|(k, c)| k.is_empty() || c.is_zero())
                                    && m.iter().any(|(k, c)| k.is_empty() && !c.is_zero())
                                {
                                    ok = true;
                                    break;
                                }
                            }
                        }
                        excl.push((j, v["locus"].clone(), ok));
                    }
                }
                let sha = p.record_id().0.to_string();
                Ok(Some((sha, lines.len(), lines, bad, excl)))
            },
        )
        .unwrap_or_else(|e| die(&e));
        for r in res {
            match r {
                Ok(Some((sha, _, lines, bad, excl))) => {
                    for b in bad {
                        independent += 1;
                        mismatches.push(format!("{sha}: {b}"));
                    }
                    for (j, l, ok) in excl {
                        if ok {
                            excl_ok.push(canonical_json(
                                &json!({"record_sha256": sha, "term": j, "locus": l}),
                            ));
                        } else {
                            excl_unconfirmed += 1;
                        }
                    }
                    for v in lines {
                        rederived += 1;
                        let k = (
                            sha.clone(),
                            v["term"].as_u64().unwrap_or(u64::MAX),
                            canonical_json(&v["locus"]),
                        );
                        match claims.remove(&k) {
                            None => mismatches.push(format!(
                                "{k:?}: re-derived {:?}, no line in the file",
                                claim_of(&v)
                            )),
                            Some(c) if claim_of(&c) != claim_of(&v) => mismatches.push(format!(
                                "{k:?}: file {:?} != re-derived {:?}",
                                claim_of(&c),
                                claim_of(&v)
                            )),
                            Some(_) => {}
                        }
                    }
                }
                Ok(None) => {}
                Err(e) => die(&e),
            }
        }
    }
    for (k, c) in claims {
        mismatches.push(format!(
            "{k:?}: claim {:?} without a re-derived locus",
            claim_of(&c)
        ));
    }
    if let Some(sp) = &a.summary {
        let p = PathBuf::from(format!("{}.excluded_confirmed.jsonl", sp.display()));
        excl_ok.sort();
        std::fs::write(
            &p,
            excl_ok.join("\n") + if excl_ok.is_empty() { "" } else { "\n" },
        )
        .unwrap_or_else(|e| die(&format!("{}: {e}", p.display())));
    }
    let s = json!({"schema": "rustred.campaign.handdown-proof-verify.v1", "verdict": if mismatches.is_empty() {"verified"} else {"mismatch"},
        "excluded_confirmed": excl_ok.len(), "excluded_unconfirmed": excl_unconfirmed,
        "claims": n_claims, "rederived": rederived, "independent_failures": independent, "mismatches": mismatches.len(),
        "first_mismatches": mismatches.iter().take(20).collect::<Vec<_>>(), "seconds": t0.elapsed().as_secs_f64(),
        "tool": zero_proofs::tool()});
    if let Some(p) = &a.summary {
        write_json(p, &s);
    }
    println!("{s}");
    if !mismatches.is_empty() {
        std::process::exit(3);
    }
}

fn load_spec(
    path: &Path,
    index_dir: &Option<PathBuf>,
    edit: impl FnOnce(&mut Value),
) -> (ImportedSet, Value) {
    let text =
        std::fs::read_to_string(path).unwrap_or_else(|e| die(&format!("{}: {e}", path.display())));
    let mut v: Value =
        serde_json::from_str(&text).unwrap_or_else(|e| die(&format!("{}: {e}", path.display())));
    edit(&mut v);
    let spec =
        TableSetSpec::from_json(&v.to_string()).unwrap_or_else(|e| die(&format!("spec: {e}")));
    let set = load_with(
        &spec,
        Path::new("/"),
        &LoadOptions {
            index_dir: index_dir.clone(),
        },
    )
    .unwrap_or_else(|e| die(&format!("load: {e}")));
    (set, v)
}

fn spec_tables(set: &ImportedSet) -> Vec<(String, String)> {
    let mut out: BTreeMap<String, String> = BTreeMap::new();
    for t in &set.rules {
        for r in t {
            out.entry(r.path.clone()).or_default();
        }
    }
    out.into_keys()
        .map(|p| {
            let (s, _) = file_sha256(Path::new(&p)).unwrap_or_else(|e| die(&e));
            (p, s)
        })
        .collect()
}

/// The `family_source` and line mask of a table header.
fn header_frame(path: &Path) -> Result<(String, Vec<bool>), String> {
    let (frame, _) = coef_facts::table_frame(path)?;
    let h = rustred_campaign_core::import::lines::read_header("arrivals", path)
        .map_err(|e| e.to_string())?;
    let fs = h
        .raw
        .get("family_source")
        .and_then(Value::as_str)
        .ok_or_else(|| format!("{}: no family_source", path.display()))?;
    Ok((fs.to_string(), frame.mask().to_vec()))
}

fn walk_cfg(a: &Args) -> arrivals::WalkCfg {
    let exact = match a.exact.as_str() {
        "all" => arrivals::ExactLevel::All,
        "none" => arrivals::ExactLevel::None,
        x if x.starts_with("keys:") => {
            let p = &x[5..];
            let text = std::fs::read_to_string(p).unwrap_or_else(|e| die(&format!("{p}: {e}")));
            let mut s = BTreeSet::new();
            for l in text.lines().filter(|l| !l.trim().is_empty()) {
                let v: Value =
                    serde_json::from_str(l).unwrap_or_else(|_| Value::String(l.trim().to_string()));
                s.insert(parse_key(&v).unwrap_or_else(|e| die(&e)));
            }
            arrivals::ExactLevel::Keys(s)
        }
        x => die(&format!("--exact {x}: all | none | keys:FILE")),
    };
    arrivals::WalkCfg {
        threads: a.threads,
        prover: prover_cfg(a),
        exact,
        structural: !a.no_structural,
        structural_min_keys: a.structural_min_keys,
        closure_chunk: a.closure_chunk,
        cache_entries: a.cache_entries,
        routed_masks: None,
    }
}

/// The masks the given edges route (hand-down terms on other masks are only counted), unless --decide-all-handdowns.
fn routed(a: &Args, edges: &[EdgeFile]) -> Option<BTreeSet<u64>> {
    (!a.decide_all_handdowns).then(|| {
        edges
            .iter()
            .flat_map(|e| e.sectors.iter().map(|s| s.dropped_mask))
            .collect()
    })
}

struct Prepared {
    set: ImportedSet,
    rows: ParentRows,
    edges: Vec<EdgeFile>,
    frame_checks: Value,
}

fn prepare(a: &Args) -> Prepared {
    let rows_p = a.rows.clone().unwrap_or_else(|| die("--rows missing"));
    let (set, edges, frame_checks) = prepare_base(a);
    let rows = ParentRows::read(&set, &rows_p).unwrap_or_else(|e| die(&e));
    Prepared {
        set,
        rows,
        edges,
        frame_checks,
    }
}

/// The parent's imported set, its edge files and the frame checks (both sides of every edge).
fn prepare_base(a: &Args) -> (ImportedSet, Vec<EdgeFile>, Value) {
    let spec = a.spec.clone().unwrap_or_else(|| die("--spec missing"));
    if a.edges.is_empty() && !a.closure_only {
        die("--edge missing (only --closure-only runs without an edge)");
    }
    let (set, _) = load_spec(&spec, &a.index_dir, |_| {});
    let edges: Vec<EdgeFile> = a
        .edges
        .iter()
        .map(|p| EdgeFile::read(p, &a.map).unwrap_or_else(|e| die(&e)))
        .collect();
    // frame binding: the parent side against every table of the set; the child side against the given child table
    let child_tables: HashMap<String, PathBuf> = a
        .child_tables
        .iter()
        .map(|s| {
            let (l, p) = s
                .split_once('=')
                .unwrap_or_else(|| die("--child-table LABEL=PATH"));
            (l.to_string(), PathBuf::from(p))
        })
        .collect();
    let mut checks = vec![];
    let tables: BTreeSet<String> = set.rules.iter().flatten().map(|r| r.path.clone()).collect();
    for e in &edges {
        for t in &tables {
            let (fs, mask) = header_frame(Path::new(t)).unwrap_or_else(|x| die(&x));
            let n = e
                .parent
                .check_frame(&fs, &mask)
                .unwrap_or_else(|x| die(&format!("FRAME parent {} vs {t}: {x}", e.path)));
            checks.push(
                json!({"edge": e.path, "side": "parent", "table": t, "slots": n, "ok": true}),
            );
        }
        let ct = child_tables
            .get(&e.child.label)
            .cloned()
            .unwrap_or_else(|| PathBuf::from(&e.child.table));
        match header_frame(&ct) {
            Ok((fs, mask)) => {
                let n = e.child.check_frame(&fs, &mask).unwrap_or_else(|x| {
                    die(&format!("FRAME child {} vs {}: {x}", e.path, ct.display()))
                });
                checks.push(
                    json!({"edge": e.path, "side": "child", "table": ct, "slots": n, "ok": true,
                    "given": child_tables.contains_key(&e.child.label)}),
                );
            }
            Err(x) => die(&format!("FRAME child {}: {x}", e.path)),
        }
    }
    (set, edges, json!(checks))
}

/// `compute`: walk the parent, decide, map, write one arrival file per edge + a summary.
fn compute(a: &Args) {
    if !a.in_memory && a.maps.len() <= 1 {
        return compute_stream(a);
    }
    let t0 = std::time::Instant::now();
    let parent = a.parent.clone().unwrap_or_else(|| die("--parent missing"));
    let out_dir = a
        .out_dir
        .clone()
        .unwrap_or_else(|| die("--out-dir missing"));
    std::fs::create_dir_all(&out_dir)
        .unwrap_or_else(|e| die(&format!("{}: {e}", out_dir.display())));
    let alg = algebra();
    let pr = prepare(a);
    let mut cfg = walk_cfg(a);
    cfg.routed_masks = routed(a, &pr.edges);
    let t_load = t0.elapsed().as_secs_f64();
    let recs: BTreeSet<(usize, usize)> =
        pr.rows.served.iter().map(|r| (r.table, r.record)).collect();
    let cache = arrivals::RecCache::new(alg, &pr.set, cfg.prover.clone(), recs);
    let w = arrivals::walk(&pr.set, &pr.rows, &cache, &cfg).unwrap_or_else(|e| die(&e));
    if let Some(p) = &a.dump_handdown_keys {
        // distinct routed hand-down term keys with a non-zero (or 0/0) coefficient at some closure key, parent frame
        let owners = rustred_campaign_core::import::arrivals::edge::mask_owners(&pr.edges)
            .unwrap_or_else(|e| die(&e));
        let mut tks: BTreeSet<Vec<i32>> = BTreeSet::new();
        for h in w.hand.iter().flatten() {
            if owners.contains_key(&h.mask) {
                tks.insert(h.term_key());
            }
        }
        let text: String = tks
            .iter()
            .map(|k| {
                format!(
                    "I({})\n",
                    k.iter()
                        .map(|x| x.to_string())
                        .collect::<Vec<_>>()
                        .join(",")
                )
            })
            .collect();
        std::fs::write(p, text).unwrap_or_else(|e| die(&format!("{}: {e}", p.display())));
    }
    let tables = spec_tables(&pr.set);
    let mut files = vec![];
    let maps: Vec<String> = if a.maps.is_empty() {
        vec![a.map.clone()]
    } else {
        a.maps.clone()
    };
    let multi = maps.len() > 1;
    let mut union: BTreeMap<String, (KeySetB, KeySetB)> = BTreeMap::new();
    let mut other = BTreeMap::new();
    for map in &maps {
        let edges: Vec<EdgeFile> = if *map == a.map {
            pr.edges.clone()
        } else {
            a.edges
                .iter()
                .map(|p| EdgeFile::read(p, map).unwrap_or_else(|e| die(&e)))
                .collect()
        };
        // each edge's file is written as soon as its lines are formed (one edge's lines in memory at a time)
        let mut emit = |ei: usize, r: arrivals::EdgeResult| -> Result<(), String> {
            let e = &edges[ei];
            if multi {
                let u = union.entry(e.child.label.clone()).or_default();
                u.0.extend(r.lines.iter().map(|l| l.key.clone()));
                u.1.extend(
                    r.lines
                        .iter()
                        .filter(|l| l.exact_senders.is_some_and(|n| n > 0))
                        .map(|l| l.key.clone()),
                );
            }
            let mut h = arrivals::header(
                &parent,
                &tables,
                &arrivals::RowsId::of(&pr.rows),
                &w.stats,
                &w.closure_digest,
                e,
                &a.edge_producer,
                &pr.frame_checks,
                &cfg,
            );
            // the file is deterministic: no wall-clock field (timings stay in the compute summary)
            h["walk"] = no_timing(serde_json::to_value(&w.stats).unwrap_or(Value::Null));
            let summ = json!({"edge_stats": no_timing(serde_json::to_value(&r.stats).unwrap_or(Value::Null)),
            "walk": no_timing(serde_json::to_value(&w.stats).unwrap_or(Value::Null))});
            let name = if multi {
                format!("{}-{}.{}.arrivals.jsonl", parent, e.child.label, map)
            } else {
                format!("{}-{}.arrivals.jsonl", parent, e.child.label)
            };
            let p = out_dir.join(&name);
            let sha = format::write(&p, &h, &r.lines, &summ)?;
            let exact = r
                .lines
                .iter()
                .filter(|l| l.exact_senders.is_some_and(|n| n > 0))
                .count();
            files.push(json!({"child": e.child.label, "map": map, "file": p, "sha256": sha, "keys": r.lines.len(), "exact_keys": exact,
            "exact_level_keys": r.lines.iter().filter(|l| l.exact_senders.is_some()).count(), "stats": r.stats}));
            Ok(())
        };
        other = arrivals::aggregate_each(&pr.set, &pr.rows, &w, &edges, &cache, &cfg, &mut emit)
            .unwrap_or_else(|e| die(&e));
        for (m, c) in &w.unrouted {
            *other.entry(*m).or_default() += c;
        }
    }
    // per-record decision sample for the second route (rr_proof instantiate requests + our words)
    if a.decisions_sample > 0 {
        let step = w
            .cl_served
            .len()
            .checked_div(a.decisions_sample)
            .unwrap_or(1)
            .max(1);
        let mut req: BTreeMap<(usize, usize), Vec<Vec<i32>>> = BTreeMap::new();
        let mut dec = vec![];
        for ci in (0..w.cl_served.len())
            .step_by(step)
            .take(a.decisions_sample)
        {
            let row = &pr.rows.served[w.cl_served[ci]];
            if w.hand[ci].is_empty() {
                continue;
            }
            req.entry((row.table, row.record))
                .or_default()
                .push(row.applied.clone());
            let info = &pr.set.rules[row.table][row.record];
            dec.push(json!({"record_sha256": info.record_sha256, "applied": row.applied,
                "nonzero": w.hand[ci].iter().map(|h| json!([h.j, h.term_key(), match h.word {CoefFact::NonZero => "nonzero", CoefFact::Zero => "zero", CoefFact::Unknown => "unknown"}, &*h.by])).collect::<Vec<_>>()}));
        }
        let rq: Vec<String> = req
            .into_iter()
            .map(|((t, r), keys)| {
                let info = &pr.set.rules[t][r];
                json!({"table": info.path, "offset": info.offset, "rule": info.index.unwrap_or(info.label), "keys": keys}).to_string()
            })
            .collect();
        std::fs::write(
            out_dir.join(format!("{parent}.second-route.requests.jsonl")),
            rq.join("\n") + "\n",
        )
        .unwrap_or_else(|e| die(&e.to_string()));
        let dl: Vec<String> = dec.iter().map(canonical_json).collect();
        std::fs::write(
            out_dir.join(format!("{parent}.decisions.jsonl")),
            dl.join("\n") + "\n",
        )
        .unwrap_or_else(|e| die(&e.to_string()));
        // distinct routed hand-down term keys (every k-th), for the engine's own transport (compare-engine)
        let owners = rustred_campaign_core::import::arrivals::edge::mask_owners(&pr.edges)
            .unwrap_or_else(|e| die(&e));
        let mut tks: BTreeSet<Vec<i32>> = BTreeSet::new();
        for h in w.hand.iter().flatten() {
            if owners.contains_key(&h.mask) {
                tks.insert(h.term_key());
            }
        }
        let step = tks
            .len()
            .checked_div(a.decisions_sample)
            .unwrap_or(1)
            .max(1);
        let sample: Vec<&Vec<i32>> = tks.iter().step_by(step).take(a.decisions_sample).collect();
        std::fs::write(
            out_dir.join(format!("{parent}.handdown-keys.sample.json")),
            json!(sample).to_string(),
        )
        .unwrap_or_else(|e| die(&e.to_string()));
    }
    let s = json!({"schema": "rustred.campaign.arrival-compute-summary.v1", "parent": parent, "files": files,
        "maps": maps, "union_over_maps": union.iter().map(|(c, (sk, ek))| json!({"child": c, "support_keys": sk.len(), "exact_keys": ek.len()})).collect::<Vec<_>>(),
        "other_children_masks": other.iter().map(|(m, c)| json!({"dropped_mask": format!("{m:#x}"),
            "dropped": (0..64).filter(|i| m >> i & 1 == 1).collect::<Vec<u32>>(), "terms": c})).collect::<Vec<_>>(),
        "walk": w.stats, "frame_checks": pr.frame_checks, "seconds_load": t_load, "seconds": t0.elapsed().as_secs_f64(),
        "threads": a.threads, "tool": arrivals::TOOL_VERSION});
    write_json(&out_dir.join(format!("{parent}.summary.json")), &s);
    println!("{}", canonical_json(&s));
}

/// The start depth of every rows key (`compute --depth`): 0 for a key of the node's own start box, else the minimum
/// `depth` of its lines in the `--arrivals` files (the same files `closure-rows` was given; every arrival line must carry
/// a depth); UNREACHED for keys that are not start keys. A start key of the rows that is neither in the box nor in an
/// arrival file is refused by the walk (no start depth).
fn start_depths(
    set: &ImportedSet,
    rows: &arrivals::stream::CompactRows,
    files: &[String],
) -> Vec<u8> {
    use arrivals::stream::UNREACHED;
    let mut sd = vec![UNREACHED; rows.n_keys()];
    for (ki, d) in sd.iter_mut().enumerate() {
        if rows.is_start(ki) && set.start.contains(&rows.key_i32(ki)) {
            *d = 0;
        }
    }
    for p in files {
        read_arrival_keys(p, &mut |key, depth| {
            let ki = rows
                .find(key)
                .filter(|&ki| rows.is_start(ki))
                .ok_or_else(|| {
                    format!("{p}: arrival key {key:?} is not a start key of the rows")
                })?;
            let d = depth.ok_or_else(|| {
                format!(
                    "{p}: arrival line {key:?} without a depth (--depth needs depth-tracked inputs)"
                )
            })?;
            let d = u8::try_from(d).unwrap_or(UNREACHED - 1).min(UNREACHED - 1);
            sd[ki] = sd[ki].min(d);
            Ok(())
        })
        .unwrap_or_else(|e| die(&e));
    }
    sd
}

/// A consumer of (arrival key, chain depth).
type KeySink<'a> = dyn FnMut(&[i32], Option<u32>) -> Result<(), String> + 'a;

/// The keys (and chain depths) of an arrival input: an arrival-set file (format::read_streamed, verified) or a key file written by
/// `arrivals-bin` / the delivery (magic RRKEYS01: u32 LE header length, header JSON with `arity`, then arity x i8 key, u8 depth,
/// u8 parents per record).
fn read_arrival_keys(p: &str, f: &mut KeySink<'_>) -> Result<format::Streamed, String> {
    use std::io::Read;
    let mut fh = std::fs::File::open(p).map_err(|e| format!("{p}: {e}"))?;
    let mut magic = [0u8; 8];
    let is_bin = fh.read_exact(&mut magic).is_ok() && &magic == b"RRKEYS01";
    if !is_bin {
        return format::read_streamed(Path::new(p), &mut |l| f(&l.key, l.depth));
    }
    let mut rd = std::io::BufReader::with_capacity(1 << 22, fh);
    let mut l4 = [0u8; 4];
    rd.read_exact(&mut l4).map_err(|e| format!("{p}: {e}"))?;
    let mut hb = vec![0u8; u32::from_le_bytes(l4) as usize];
    rd.read_exact(&mut hb).map_err(|e| format!("{p}: {e}"))?;
    let header: Value = serde_json::from_slice(&hb).map_err(|e| format!("{p} header: {e}"))?;
    let n = header["arity"]
        .as_u64()
        .ok_or_else(|| format!("{p}: header without arity"))? as usize;
    let mut rec = vec![0u8; n + 2];
    let mut key = vec![0i32; n];
    let mut count = 0u64;
    loop {
        match rd.read_exact(&mut rec) {
            Ok(()) => {}
            Err(e) if e.kind() == std::io::ErrorKind::UnexpectedEof => break,
            Err(e) => return Err(format!("{p}: {e}")),
        }
        for (k, b) in key.iter_mut().zip(&rec[..n]) {
            *k = i32::from(*b as i8);
        }
        f(&key, Some(u32::from(rec[n])))?;
        count += 1;
    }
    let (sha, _) = file_sha256(Path::new(p))?;
    Ok(format::Streamed {
        header,
        summary: Value::Null,
        sha256: sha,
        keys: count,
    })
}

/// `verify`: re-derive the arrival sets from the tables (hand-down terms decided PER KEY only: no structural proof is
/// used), on all parent keys (`--sample 0`) or every k-th; compare with the files (key set, sender counts, first
/// senders, exact counts when the file has the exact level), re-check every expansion used by a first sender with the
/// independent mod-p identity. Exit 0 verified / 3 mismatch.
/// `compute` in the streamed form (default; `--in-memory` or `--maps` with several maps take the in-memory walk): the
/// same files, memory bounded by `--record-budget-mb` (parsed records kept) and `--map-budget` (accumulator entries
/// before a spill to `--spill-dir`, default OUT/spill).
fn compute_stream(a: &Args) {
    let t0 = std::time::Instant::now();
    let parent = a.parent.clone().unwrap_or_else(|| die("--parent missing"));
    let out_dir = a
        .out_dir
        .clone()
        .unwrap_or_else(|| die("--out-dir missing"));
    std::fs::create_dir_all(&out_dir)
        .unwrap_or_else(|e| die(&format!("{}: {e}", out_dir.display())));
    if let Some(m) = a.maps.first()
        && *m != a.map
    {
        die("--maps with one map: give it as --map");
    }
    let alg = algebra();
    let rows_p = a.rows.clone().unwrap_or_else(|| die("--rows missing"));
    let (set, edges, frame_checks) = prepare_base(a);
    let rows =
        arrivals::stream::CompactRows::read(&set, &rows_p, a.threads).unwrap_or_else(|e| die(&e));
    let mut cfg = walk_cfg(a);
    cfg.routed_masks = routed(a, &edges);
    let t_load = t0.elapsed().as_secs_f64();
    let cache = arrivals::RecCache::new(alg, &set, cfg.prover.clone(), rows.records())
        .with_budget(Some(a.record_budget_mb << 20));
    let scfg = arrivals::stream::StreamCfg {
        map_budget: a.map_budget.max(1),
        spill_dir: a.spill_dir.clone().unwrap_or_else(|| out_dir.join("spill")),
        decisions_sample: a.decisions_sample,
        term_keys: a.dump_handdown_keys.is_some() || a.decisions_sample > 0,
        start_depth: a.depth.then(|| start_depths(&set, &rows, &a.arrivals)),
        trace: a.trace_keys.clone(),
        closure_only: a.closure_only,
        closure_out: a.closure_out.clone(),
        shard: a.shard,
        local_flush: if a.local_flush == 0 {
            1 << 21
        } else {
            a.local_flush
        },
    };
    let tables = spec_tables(&set);
    let rows_id = rows.id();
    let mut files = vec![];
    let mut emit = |r: &arrivals::stream::StreamResult,
                    ei: usize,
                    stats: arrivals::EdgeStats,
                    lines: &mut dyn Iterator<Item = Result<format::ArrivalLine, String>>|
     -> Result<(), String> {
        let e = &edges[ei];
        let mut h = arrivals::header(
            &parent,
            &tables,
            &rows_id,
            &r.stats,
            &r.closure_digest,
            e,
            &a.edge_producer,
            &frame_checks,
            &cfg,
        );
        // the file is deterministic: no wall-clock field (timings stay in the compute summary)
        h["walk"] = no_timing(serde_json::to_value(&r.stats).unwrap_or(Value::Null));
        if let Some((x, y)) = a.shard {
            h["shard"] = json!(format!("{x}/{y}"));
        }
        let summ = json!({"edge_stats": no_timing(serde_json::to_value(&stats).unwrap_or(Value::Null)),
            "walk": no_timing(serde_json::to_value(&r.stats).unwrap_or(Value::Null))});
        let p = out_dir.join(format!("{}-{}.arrivals.jsonl", parent, e.child.label));
        let wr = format::write_stream(&p, &h, lines, &summ)?;
        files.push(json!({"child": e.child.label, "map": a.map, "file": p, "sha256": wr.sha256, "keys": wr.keys,
            "exact_keys": wr.exact_keys, "exact_level_keys": wr.exact_level_keys, "stats": stats}));
        Ok(())
    };
    let r = arrivals::stream::walk_stream(&set, &rows, &cache, &cfg, &scfg, &edges, &mut emit)
        .unwrap_or_else(|e| die(&e));
    let mut other = r.other.clone();
    for (m, c) in &r.unrouted {
        *other.entry(*m).or_default() += c;
    }
    if let Some(p) = &a.dump_handdown_keys {
        let text: String = r
            .term_keys
            .iter()
            .map(|k| {
                format!(
                    "I({})\n",
                    k.iter()
                        .map(|x| x.to_string())
                        .collect::<Vec<_>>()
                        .join(",")
                )
            })
            .collect();
        std::fs::write(p, text).unwrap_or_else(|e| die(&format!("{}: {e}", p.display())));
    }
    if a.decisions_sample > 0 {
        let mut req: BTreeMap<(usize, usize), Vec<Vec<i32>>> = BTreeMap::new();
        let mut dec = vec![];
        for (si, hand) in &r.sample {
            if hand.is_empty() {
                continue;
            }
            let (t, rr) = rows.rec(*si);
            let applied = rows.applied(*si);
            req.entry((t, rr)).or_default().push(applied.clone());
            let info = &set.rules[t][rr];
            dec.push(json!({"record_sha256": info.record_sha256, "applied": applied,
                "nonzero": hand.iter().map(|h| json!([h.j, h.term_key(), match h.word {CoefFact::NonZero => "nonzero", CoefFact::Zero => "zero", CoefFact::Unknown => "unknown"}, &*h.by])).collect::<Vec<_>>()}));
        }
        let rq: Vec<String> = req
            .into_iter()
            .map(|((t, rr), keys)| {
                let info = &set.rules[t][rr];
                json!({"table": info.path, "offset": info.offset, "rule": info.index.unwrap_or(info.label), "keys": keys}).to_string()
            })
            .collect();
        std::fs::write(
            out_dir.join(format!("{parent}.second-route.requests.jsonl")),
            rq.join("\n") + "\n",
        )
        .unwrap_or_else(|e| die(&e.to_string()));
        let dl: Vec<String> = dec.iter().map(canonical_json).collect();
        std::fs::write(
            out_dir.join(format!("{parent}.decisions.jsonl")),
            dl.join("\n") + "\n",
        )
        .unwrap_or_else(|e| die(&e.to_string()));
        let step = r
            .term_keys
            .len()
            .checked_div(a.decisions_sample)
            .unwrap_or(1)
            .max(1);
        let sample: Vec<&Vec<i32>> = r
            .term_keys
            .iter()
            .step_by(step)
            .take(a.decisions_sample)
            .collect();
        std::fs::write(
            out_dir.join(format!("{parent}.handdown-keys.sample.json")),
            json!(sample).to_string(),
        )
        .unwrap_or_else(|e| die(&e.to_string()));
    }
    let s = json!({"schema": "rustred.campaign.arrival-compute-summary.v1", "parent": parent, "files": files,
        "maps": [a.map.clone()], "union_over_maps": [], "form": "streamed", "spills": r.spills,
        "rows": {"keys": rows.n_keys(), "served": rows.n_served(), "terminal": rows.terminal, "open": rows.open},
        "budgets": {"record_mb": a.record_budget_mb, "map_entries": a.map_budget},
        "other_children_masks": other.iter().map(|(m, c)| json!({"dropped_mask": format!("{m:#x}"),
            "dropped": (0..64).filter(|i| m >> i & 1 == 1).collect::<Vec<u32>>(), "terms": c})).collect::<Vec<_>>(),
        "walk": r.stats, "frame_checks": frame_checks, "seconds_load": t_load, "seconds": t0.elapsed().as_secs_f64(),
        "threads": a.threads, "tool": arrivals::TOOL_VERSION, "depth_tracked": a.depth, "closure_only": a.closure_only,
        "trace": r.trace, "closure_out": r.closure_out, "shard": a.shard.map(|(x, y)| format!("{x}/{y}"))});
    write_json(&out_dir.join(format!("{parent}.summary.json")), &s);
    println!("{}", canonical_json(&s));
}

fn verify(a: &Args) {
    let t0 = std::time::Instant::now();
    if a.arrivals.is_empty() {
        die("--arrivals missing");
    }
    let alg = algebra();
    let pr = prepare(a);
    let files: Vec<ArrivalSet> = a
        .arrivals
        .iter()
        .map(|p| format::read(Path::new(p)).unwrap_or_else(|e| die(&e)))
        .collect();
    let mut mism: Vec<String> = vec![];
    // bindings
    for (f, p) in files.iter().zip(&a.arrivals) {
        if f.header["parent"]["rows"]["sha256"] != pr.rows.sha256.as_str() {
            mism.push(format!(
                "{p}: rows sha256 {} != {}",
                f.header["parent"]["rows"]["sha256"], pr.rows.sha256
            ));
        }
        if !pr
            .edges
            .iter()
            .any(|e| f.header["edge"]["sha256"] == e.sha256.as_str())
        {
            mism.push(format!(
                "{p}: edge sha256 {} is none of the given edges",
                f.header["edge"]["sha256"]
            ));
        }
    }
    let mut cfg = walk_cfg(a);
    cfg.routed_masks = routed(a, &pr.edges);
    cfg.structural = false;
    let exact_in_files = files
        .iter()
        .any(|f| f.lines.iter().any(|l| l.exact_senders.is_some()));
    if !exact_in_files {
        cfg.exact = arrivals::ExactLevel::None;
    }
    let mut rows = pr.rows.clone();
    let full = a.sample == 0;
    let recs: BTreeSet<(usize, usize)> = rows.served.iter().map(|r| (r.table, r.record)).collect();
    let cache = arrivals::RecCache::new(alg, &pr.set, cfg.prover.clone(), recs);
    let w = arrivals::walk(&pr.set, &rows, &cache, &cfg).unwrap_or_else(|e| die(&e));
    for f in &files {
        if f.header["parent"]["closure_digest"] != w.closure_digest.as_str() {
            mism.push(format!(
                "closure digest {} != re-derived {}",
                f.header["parent"]["closure_digest"], w.closure_digest
            ));
        }
    }
    // a sample of parent keys: restrict the walk's served set (every k-th closure key keeps its hand-down terms)
    let mut wv = w;
    if !full {
        let step = a.sample.max(1);
        for (ci, h) in wv.hand.iter_mut().enumerate() {
            if ci % step != 0 {
                h.clear();
            }
        }
    }
    let (res, _) = arrivals::aggregate(&pr.set, &rows, &wv, &pr.edges, &cache, &cfg)
        .unwrap_or_else(|e| die(&e));
    let tr = arrivals::Transporter::new(alg, pr.set.arity()).unwrap_or_else(|e| die(&e));
    let mut checked = 0u64;
    let mut identity_checks = 0u64;
    for (e, r) in pr.edges.iter().zip(&res) {
        let Some(f) = files
            .iter()
            .find(|f| f.header["edge"]["sha256"] == e.sha256.as_str())
        else {
            continue;
        };
        let fm: HashMap<&Vec<i32>, &format::ArrivalLine> =
            f.lines.iter().map(|l| (&l.key, l)).collect();
        for l in &r.lines {
            checked += 1;
            match fm.get(&l.key) {
                None => mism.push(format!(
                    "{}: re-derived key {:?} missing from the file",
                    e.child.label, l.key
                )),
                Some(fl) => {
                    // the sender's identity (how it was decided differs by design: the verifier decides per key)
                    let same_first = (
                        &fl.first.parent_key,
                        &fl.first.applied,
                        &fl.first.record_sha256,
                        fl.first.term,
                        &fl.first.term_key,
                    ) == (
                        &l.first.parent_key,
                        &l.first.applied,
                        &l.first.record_sha256,
                        l.first.term,
                        &l.first.term_key,
                    );
                    if full && (fl.senders != l.senders || !same_first) {
                        mism.push(format!(
                            "{}: key {:?}: file senders {} first {:?} != re-derived {} {:?}",
                            e.child.label,
                            l.key,
                            fl.senders,
                            fl.first.parent_key,
                            l.senders,
                            l.first.parent_key
                        ));
                    }
                    if full && exact_in_files && fl.exact_senders != l.exact_senders {
                        mism.push(format!(
                            "{}: key {:?}: exact senders {:?} != re-derived {:?}",
                            e.child.label, l.key, fl.exact_senders, l.exact_senders
                        ));
                    }
                }
            }
        }
        if full && f.lines.len() != r.lines.len() {
            mism.push(format!(
                "{}: file has {} keys, re-derived {}",
                e.child.label,
                f.lines.len(),
                r.lines.len()
            ));
        }
        // every file line's first sender: the expansion re-checked independently (mod-p identity)
        for fl in f
            .lines
            .iter()
            .step_by(if full { 1 } else { a.sample.max(1) })
        {
            let mask = fl
                .first
                .term_key
                .iter()
                .zip(pr.set.frame.mask())
                .enumerate()
                .filter(|(_, (v, l))| **l && **v <= 0)
                .fold(0u64, |m, (i, _)| m | 1 << i);
            let Some(sec) = e.sector(mask) else {
                mism.push(format!(
                    "{}: key {:?}: first sender's term mask {mask:#x} is not routed by the edge",
                    e.child.label, fl.key
                ));
                continue;
            };
            match tr.apply(sec, &fl.first.term_key) {
                Err(x) => mism.push(format!(
                    "{}: key {:?}: expansion: {x}",
                    e.child.label, fl.key
                )),
                Ok(img) => {
                    if !img.iter().any(|(k, _)| k == &fl.key) {
                        mism.push(format!(
                            "{}: key {:?}: not in the expansion of its first sender's term",
                            e.child.label, fl.key
                        ));
                    }
                    let im: Option<ImageI128> = img
                        .iter()
                        .map(|(k, c)| arrivals::rat_i128(c).map(|x| (k.clone(), x)))
                        .collect();
                    if let Some(im) = im {
                        identity_checks += 1;
                        match arrivals::transport_identity_mod_p(
                            sec,
                            &fl.first.term_key,
                            &im,
                            identity_checks,
                        ) {
                            Ok(true) => {}
                            Ok(false) => mism.push(format!(
                                "{}: key {:?}: independent identity check of the expansion FAILED",
                                e.child.label, fl.key
                            )),
                            Err(x) => mism.push(format!("{}: identity: {x}", e.child.label)),
                        }
                    }
                }
            }
        }
    }
    rows.served.clear();
    let s = json!({"schema": "rustred.campaign.arrival-verify.v1", "verdict": if mism.is_empty() {"verified"} else {"mismatch"},
        "full": full, "sample_step": a.sample, "rederived_keys_checked": checked, "identity_checks": identity_checks,
        "exact_compared": exact_in_files, "mismatches": mism.len(), "first_mismatches": mism.iter().take(20).collect::<Vec<_>>(),
        "seconds": t0.elapsed().as_secs_f64(), "tool": arrivals::TOOL_VERSION});
    if let Some(p) = &a.summary {
        write_json(p, &s);
    }
    println!("{}", canonical_json(&s));
    if !mism.is_empty() {
        std::process::exit(3);
    }
}

/// `child-check`: classify every parent's arrivals against the child's certified set and tables (core `arrivals::child`).
/// Schema of a child-side SERVICE SET manifest (interfaces/arrivals.md 5): the child's import spec (its tables and
/// relations in FIRST-MATCH order) and its closed key sets, every file pinned by sha256 (verified here, fail closed).
const SERVICES_SCHEMA: &str = "rustred.campaign.arrival-services.v1";

/// Read and verify a service manifest: (name, spec path, closed sets as NAME=PATH, manifest sha256).
fn read_services(p: &Path) -> (String, PathBuf, Vec<String>, String) {
    let (msha, _) = file_sha256(p).unwrap_or_else(|e| die(&e));
    let v: Value = serde_json::from_str(
        &std::fs::read_to_string(p).unwrap_or_else(|e| die(&format!("{}: {e}", p.display()))),
    )
    .unwrap_or_else(|e| die(&format!("{}: {e}", p.display())));
    if v["schema"] != SERVICES_SCHEMA {
        die(&format!(
            "{}: schema {} (expected {SERVICES_SCHEMA})",
            p.display(),
            v["schema"]
        ));
    }
    let pinned = |x: &Value, what: &str| -> PathBuf {
        let path = PathBuf::from(
            x["path"]
                .as_str()
                .unwrap_or_else(|| die(&format!("{what}: no path"))),
        );
        let want = x["sha256"]
            .as_str()
            .unwrap_or_else(|| die(&format!("{what}: not pinned (no sha256)")));
        let (got, _) = file_sha256(&path).unwrap_or_else(|e| die(&e));
        if got != want {
            die(&format!(
                "{what}: {} has sha256 {got}, the manifest pins {want}",
                path.display()
            ));
        }
        path
    };
    let spec = pinned(&v["spec"], "services spec");
    let closed = v["closed_sets"]
        .as_array()
        .into_iter()
        .flatten()
        .map(|c| {
            let n = c["name"]
                .as_str()
                .unwrap_or_else(|| die("closed set without a name"));
            format!("{n}={}", pinned(c, &format!("closed set {n}")).display())
        })
        .collect();
    (
        v["name"].as_str().unwrap_or("").to_string(),
        spec,
        closed,
        msha,
    )
}

fn child_check(a: &Args) {
    let t0 = std::time::Instant::now();
    // the service set: a pinned manifest (preferred) or an explicit spec; never a default path
    let services = a.services.as_ref().map(|p| read_services(p));
    let spec = match &services {
        Some((_, s, _, _)) => s.clone(),
        None => a
            .spec
            .clone()
            .unwrap_or_else(|| die("--services MANIFEST (or --spec) missing")),
    };
    let mut closed_args = a.closed_sets.clone();
    if let Some((_, _, c, _)) = &services {
        closed_args.extend(c.iter().cloned());
    }
    let st = a
        .statement
        .clone()
        .unwrap_or_else(|| die("--statement missing"));
    let out = a.out.clone().unwrap_or_else(|| die("--out missing"));
    let work = a
        .work_dir
        .clone()
        .unwrap_or_else(|| die("--work-dir missing"));
    if a.arrivals.is_empty() {
        die("--arrivals NAME=FILE missing");
    }
    let (set, sv) = load_spec(&spec, &a.index_dir, |v| {
        // keys only: the child's tables as they dispatch; evidence is not needed (and not loaded)
        if let Some(o) = v.as_object_mut() {
            o.remove("evidence");
        }
        v["policies"] = json!({"witness_check": "off_diagnostic", "tier": "K"});
    });
    let ev = Evidence::from_spec(&set, Path::new("/"))
        .unwrap_or_else(|e| die(&format!("evidence: {e}")));
    let (cert, cert_sha) = child::read_certified(&st).unwrap_or_else(|e| die(&e));
    // named closed sets of the child (served by definition), each bound by its sha256
    let closed: Vec<child::ClosedSet> = closed_args
        .iter()
        .map(|x| {
            let (n, p) = x
                .split_once('=')
                .unwrap_or_else(|| die("--closed-set NAME=FILE"));
            child::ClosedSet::read(n, Path::new(p), &set).unwrap_or_else(|e| die(&e))
        })
        .collect();
    let mut parents = vec![];
    let lines = set.frame.mask().to_vec();
    for s in &a.arrivals {
        let (name, p) = s
            .split_once('=')
            .unwrap_or_else(|| die("--arrivals NAME=FILE"));
        let arr = format::read(Path::new(p)).unwrap_or_else(|e| die(&e));
        let rows = work.join(format!("rows.{name}.jsonl"));
        let pc = child::check_parent(&set, &ev, &cert, &closed, name, &arr, &rows, a.threads)
            .unwrap_or_else(|e| die(&e));
        // every (c) key and every open key reached, in full (the report lists the first 50)
        let kl = |k: &Vec<i32>| {
            format!(
                "[{}]\n",
                k.iter()
                    .map(|x| x.to_string())
                    .collect::<Vec<_>>()
                    .join(",")
            )
        };
        let cp = out.with_file_name(format!("{name}.c_keys.jsonl"));
        let op = out.with_file_name(format!("{name}.open_keys.jsonl"));
        std::fs::write(&cp, pc.c.iter().map(kl).collect::<String>())
            .unwrap_or_else(|e| die(&e.to_string()));
        std::fs::write(
            &op,
            pc.open_reached
                .iter()
                .map(|(k, _)| kl(k))
                .collect::<String>(),
        )
        .unwrap_or_else(|e| die(&e.to_string()));
        let mut j = pc.json(&lines);
        j["c_keys_file"] = json!(cp);
        j["open_keys_file"] = json!(op);
        j["arrivals_file"] = json!({"path": p, "sha256": arr.sha256});
        parents.push(j);
        let _ = std::fs::remove_file(&rows);
    }
    let r = json!({"schema": CHILD_CHECK_SCHEMA, "child_spec": {"path": spec, "node": sv["node"]},
        "certified_set": {"path": st, "sha256": cert_sha, "keys": cert.len()}, "parents": parents,
        "closed_sets": closed.iter().map(|c| json!({"name": c.name, "path": c.path, "sha256": c.sha256,
            "keys": c.keys.len()})).collect::<Vec<_>>(),
        "services": services.as_ref().map(|(n, s, _, m)| json!({"manifest": a.services, "manifest_sha256": m, "name": n, "spec": s})),
        "seconds": t0.elapsed().as_secs_f64(), "tool": arrivals::TOOL_VERSION});
    write_json(&out, &r);
    println!("{}", canonical_json(&r));
}

/// `second-route`: compare our hand-down words (compute's decisions sample) with rr_proof instantiate's facts for the
/// same (record, key) pairs (loaded by the core's evidence reader). Exit 0 agree / 3 disagreement.
fn second_route(a: &Args) {
    use std::io::BufRead;
    let spec = a.spec.clone().unwrap_or_else(|| die("--spec missing"));
    let dec = a
        .decisions
        .clone()
        .unwrap_or_else(|| die("--decisions missing"));
    let out = a.out.clone().unwrap_or_else(|| die("--out missing"));
    let facts: Vec<Value> = a
        .facts
        .iter()
        .map(|f| {
            let (s, _) = file_sha256(f).unwrap_or_else(|e| die(&e));
            json!({"path": f, "sha256": s})
        })
        .collect();
    let (set, _) = load_spec(&spec, &a.index_dir, |v| {
        let fam = v["sources"][0]["path"]
            .as_str()
            .and_then(|p| header_family(Path::new(p)));
        let ev: Vec<Value> = facts
            .iter()
            .map(|f| {
                let mut f = f.clone();
                if let Some(fm) = &fam {
                    f["family"] = json!(fm);
                }
                f
            })
            .collect();
        v["evidence"] = json!({"facts": ev});
        v["policies"] = json!({"tier": "C"});
    });
    let ev = Evidence::from_spec(&set, Path::new("/"))
        .unwrap_or_else(|e| die(&format!("evidence: {e}")));
    use rustred_campaign_core::closure::FactSource;
    let f = std::fs::File::open(&dec).unwrap_or_else(|e| die(&e.to_string()));
    let (mut pairs, mut terms, mut agree, mut unanswered) = (0u64, 0u64, 0u64, 0u64);
    let mut dis = vec![];
    for l in std::io::BufReader::new(f).lines() {
        let l = l.unwrap_or_else(|e| die(&e.to_string()));
        if l.trim().is_empty() {
            continue;
        }
        let v: Value = serde_json::from_str(&l).unwrap_or_else(|e| die(&e.to_string()));
        let sha = v["record_sha256"].as_str().unwrap_or("");
        let (t, r) = set
            .find_sha(sha)
            .unwrap_or_else(|| die(&format!("record {sha} not in the set")));
        let applied: Vec<i32> =
            serde_json::from_value(v["applied"].clone()).unwrap_or_else(|e| die(&e.to_string()));
        let info = &set.rules[t][r];
        let rid = rustred_campaign_core::closure::RuleId {
            table: &info.table_name,
            record: r,
            label: info.label,
            digest: "",
        };
        pairs += 1;
        // every hand-down term of the record at the key (ours: listed = non-zero / unknown, unlisted = zero)
        let listed: HashMap<u64, String> = v["nonzero"]
            .as_array()
            .into_iter()
            .flatten()
            .map(|x| {
                (
                    x[0].as_u64().unwrap_or(0),
                    x[2].as_str().unwrap_or("").to_string(),
                )
            })
            .collect();
        let (mut buf, mut cm) = (vec![], vec![]);
        let n = set.arity();
        let Ok(Some(nt)) = set.bodies.instantiate(
            &set.disp,
            set.disp.rule_ref(t, r),
            &applied,
            &mut buf,
            &mut cm,
        ) else {
            die("instantiate failed");
        };
        for j in 0..nt {
            let tk = &buf[j * n..(j + 1) * n];
            let lines = set.frame.mask();
            let pos = tk.iter().zip(lines).any(|(x, l)| !*l && *x > 0);
            let drop = tk.iter().zip(lines).any(|(x, l)| *l && *x <= 0);
            if pos || !drop {
                continue;
            }
            terms += 1;
            let ours = listed
                .get(&(j as u64))
                .cloned()
                .unwrap_or_else(|| "zero".into());
            let theirs = match ev.coefficient(&rid, &applied, j, tk) {
                CoefFact::Zero => "zero",
                CoefFact::NonZero => "nonzero",
                CoefFact::Unknown => "unknown",
            };
            if theirs == "unknown" && !ev.has_fact(&set, t, r, &applied) {
                unanswered += 1;
                continue;
            }
            if ours == theirs {
                agree += 1;
            } else if dis.len() < 20 {
                dis.push(json!({"record_sha256": sha, "applied": applied, "term": j, "ours": ours, "rr_proof": theirs}));
            }
        }
    }
    let disagree = terms - agree - unanswered;
    let r = json!({"schema": "rustred.campaign.arrival-second-route.v1", "pairs": pairs, "handdown_terms": terms,
        "agree": agree, "disagree": disagree, "unanswered": unanswered, "first_disagreements": dis, "facts": facts});
    write_json(&out, &r);
    println!("{}", canonical_json(&r));
    if disagree > 0 {
        std::process::exit(3);
    }
}

/// `compare-engine`: the engine's own transport (the frozen route probe's `<out>.keys.jsonl`: per parent table key, the
/// chosen map's child table terms with exact coefficients) vs our Symbolica expansion of the same keys under the edge
/// file's map. Exit 0 identical / 3 a difference.
fn compare_engine(a: &Args) {
    use std::io::BufRead;
    let alg = algebra();
    let e = EdgeFile::read(
        a.edges.first().unwrap_or_else(|| die("--edge missing")),
        &a.map,
    )
    .unwrap_or_else(|x| die(&x));
    let tr = arrivals::Transporter::new(alg, e.parent.momenta.len()).unwrap_or_else(|x| die(&x));
    let (mut keys, mut terms, mut diffs) = (0u64, 0u64, vec![]);
    for f in &a.facts {
        let r = std::fs::File::open(f).unwrap_or_else(|x| die(&format!("{}: {x}", f.display())));
        for l in std::io::BufReader::new(r).lines() {
            let l = l.unwrap_or_else(|x| die(&x.to_string()));
            if l.trim().is_empty() {
                continue;
            }
            let v: Value = serde_json::from_str(&l).unwrap_or_else(|x| die(&x.to_string()));
            let pk: Vec<i32> = serde_json::from_value(v["parent_table_key"].clone())
                .unwrap_or_else(|x| die(&x.to_string()));
            let routes = v["routes"].as_object().cloned().unwrap_or_default();
            for (_, r) in routes {
                let eng: BTreeMap<Vec<i32>, String> = r[&a.map]
                    .as_array()
                    .unwrap_or_else(|| die("engine line without the map"))
                    .iter()
                    .map(|t| {
                        let k: Vec<i32> = serde_json::from_value(t[0].clone())
                            .unwrap_or_else(|x| die(&x.to_string()));
                        let c = rustred_campaign_core::import::arrivals::edge::Rat::parse(&t[1])
                            .map(|c| c.text())
                            .unwrap_or_else(|_| format!("unparsed:{}", t[1]));
                        (k, c)
                    })
                    .collect();
                keys += 1;
                let mask =
                    pk.iter()
                        .zip(&e.parent.table_lines.iter().fold(
                            vec![false; pk.len()],
                            |mut m, &i| {
                                m[i] = true;
                                m
                            },
                        ))
                        .enumerate()
                        .filter(|(_, (x, l))| **l && **x <= 0)
                        .fold(0u64, |m, (i, _)| m | 1 << i);
                let Some(sec) = e.sector(mask) else {
                    diffs.push(format!("{pk:?}: mask {mask:#x} not routed by the edge"));
                    continue;
                };
                let ours: BTreeMap<Vec<i32>, String> = tr
                    .apply(sec, &pk)
                    .unwrap_or_else(|x| die(&x))
                    .into_iter()
                    .map(|(k, c)| {
                        let (n, d) = arrivals::rat_i128(&c).unwrap_or((0, 0));
                        (
                            k,
                            if d == 1 {
                                n.to_string()
                            } else {
                                format!("{n}/{d}")
                            },
                        )
                    })
                    .collect();
                terms += eng.len() as u64;
                if ours != eng && diffs.len() < 20 {
                    let only_e: Vec<_> = eng
                        .iter()
                        .filter(|(k, c)| ours.get(*k) != Some(c))
                        .take(3)
                        .collect();
                    let only_o: Vec<_> = ours
                        .iter()
                        .filter(|(k, c)| eng.get(*k) != Some(c))
                        .take(3)
                        .collect();
                    diffs.push(format!(
                        "{pk:?}: engine-only {only_e:?} ours-only {only_o:?}"
                    ));
                } else if ours != eng {
                    diffs.push(String::new());
                }
            }
        }
    }
    let r = json!({"schema": "rustred.campaign.arrival-engine-compare.v1", "edge": e.identity(), "keys": keys, "engine_terms": terms,
        "differing_keys": diffs.len(), "first": diffs.iter().filter(|x| !x.is_empty()).take(20).collect::<Vec<_>>(),
        "verdict": if diffs.is_empty() {"identical"} else {"differ"}});
    if let Some(o) = &a.out {
        write_json(o, &r);
    }
    println!("{}", canonical_json(&r));
    if !diffs.is_empty() {
        std::process::exit(3);
    }
}

/// A counters object without its wall-clock fields (`seconds*`).
fn no_timing(mut v: Value) -> Value {
    if let Some(o) = v.as_object_mut() {
        o.retain(|k, _| !k.starts_with("seconds"));
    }
    v
}

/// `closure-rows`: the CHAIN step (coordinator 2026-11-15: arrivals through intermediate nodes count, taken to closure):
/// the node's keys-only (Tier K) closure from its spec's start set (its box) UNION every incoming arrival set (support
/// level, keys in this node's table frame), written as a check rows file with `in_start` = box or arriving key; `compute`
/// then walks it (exact closure from those starts) and hands down on every outgoing edge. Keys of an arrival file outside
/// the node's codec range are refused.
fn closure_rows(a: &Args) {
    let t0 = std::time::Instant::now();
    let spec = a.spec.clone().unwrap_or_else(|| die("--spec missing"));
    let out = a.out.clone().unwrap_or_else(|| die("--out missing"));
    let (set, _) = load_spec(&spec, &a.index_dir, |v| {
        if let Some(o) = v.as_object_mut() {
            o.remove("evidence");
        }
        v["policies"] = json!({"witness_check": "off_diagnostic", "tier": "K"});
    });
    let ev = Evidence::from_spec(&set, Path::new("/"))
        .unwrap_or_else(|e| die(&format!("evidence: {e}")));
    // arrival files streamed (keys only: an arrival file can hold tens of millions of lines), packed per file
    let mut files = vec![];
    let mut start = set.start.clone();
    for p in &a.arrivals {
        let mut flat: Vec<i32> = vec![];
        let r = read_arrival_keys(p, &mut |k, _| {
            flat.extend_from_slice(k);
            Ok(())
        })
        .unwrap_or_else(|e| die(&e));
        let n = set.arity();
        let add =
            rustred_campaign_core::closure::KeySet::from_keys(set.codec, flat.chunks(n.max(1)))
                .unwrap_or_else(|e| {
                    die(&format!(
                        "{p}: arrival keys do not fit the node's codec: {e}"
                    ))
                });
        drop(flat);
        start = start.union(&add);
        files.push(json!({"path": p, "sha256": r.sha256, "keys": r.keys}));
    }
    let n_box = set.start.len();
    let r = rustred_campaign_core::import::check::check_streamed_from(
        &set,
        &ev,
        &rustred_campaign_core::import::CheckOptions {
            threads: a.threads,
            tier: Some(rustred_campaign_core::closure::Tier::K),
            max_rounds: None,
        },
        &out,
        &start,
    )
    .unwrap_or_else(|e| die(&format!("closure: {e}")));
    let s = json!({"schema": "rustred.campaign.arrival-closure-rows.v1", "spec": spec, "rows": out, "box_keys": n_box,
        "arrival_files": files, "arrival_keys_new": start.len() - n_box, "start_keys": start.len(),
        "closure_keys": r.summary.keys_total, "open": r.summary.open_total, "open_by_status": r.summary.open_by_status,
        "seconds": t0.elapsed().as_secs_f64(), "tool": arrivals::TOOL_VERSION});
    if let Some(p) = &a.summary {
        write_json(p, &s);
    }
    println!("{}", canonical_json(&s));
}

/// `arrivals-bin --arrivals F --out B`: one arrival file as a key file (RRKEYS01: u32 LE header length, header JSON, then per line in
/// file order = signed key order: arity x i8 key, u8 chain depth (0 when absent), u8 1 (one sending parent)); prints counts per depth.
fn arrivals_bin(a: &Args) {
    use std::io::Write;
    let src = a
        .arrivals
        .first()
        .cloned()
        .unwrap_or_else(|| die("--arrivals missing"));
    let out = a.out.clone().unwrap_or_else(|| die("--out missing"));
    let tmp = out.with_extension("partial");
    let mut w = std::io::BufWriter::with_capacity(
        1 << 22,
        std::fs::File::create(&tmp).unwrap_or_else(|e| die(&format!("{}: {e}", tmp.display()))),
    );
    let (mut n, mut arity) = (0u64, 0usize);
    let mut by_depth: BTreeMap<u32, u64> = BTreeMap::new();
    let mut body: Vec<u8> = vec![];
    if a.fast {
        return arrivals_bin_fast(&src, &out);
    }
    let r = format::read_streamed(Path::new(&src), &mut |l| {
        if arity == 0 {
            arity = l.key.len();
        }
        if l.key.len() != arity {
            return Err(format!("{src}: key of arity {} after {arity}", l.key.len()));
        }
        for &x in &l.key {
            body.push(
                i8::try_from(x).map_err(|_| format!("{src}: key slot {x} outside i8"))? as u8,
            );
        }
        let d = l.depth.unwrap_or(0);
        body.push(u8::try_from(d).unwrap_or(u8::MAX));
        body.push(1);
        *by_depth.entry(d).or_default() += 1;
        n += 1;
        Ok(())
    })
    .unwrap_or_else(|e| die(&e));
    let lines: Vec<usize> = r.header["child"]["lines"]
        .as_array()
        .map(|v| {
            v.iter()
                .filter_map(|x| x.as_u64().map(|x| x as usize))
                .collect()
        })
        .unwrap_or_default();
    let frame: String = (0..arity)
        .map(|i| if lines.contains(&i) { '1' } else { '0' })
        .collect();
    let hdr = json!({"schema": "rustred.campaign.arrival-keys.v1", "source": src, "source_sha256": r.sha256, "frame": frame,
        "arity": arity, "record": format!("{arity} x i8 key (child table frame), u8 chain depth, u8 sending parents (1)")});
    let hb = hdr.to_string().into_bytes();
    let wr = |w: &mut std::io::BufWriter<std::fs::File>, b: &[u8]| {
        w.write_all(b)
            .unwrap_or_else(|e| die(&format!("{}: {e}", tmp.display())))
    };
    wr(&mut w, b"RRKEYS01");
    wr(&mut w, &(hb.len() as u32).to_le_bytes());
    wr(&mut w, &hb);
    wr(&mut w, &body);
    w.flush().unwrap_or_else(|e| die(&e.to_string()));
    drop(w);
    std::fs::rename(&tmp, &out).unwrap_or_else(|e| die(&format!("{}: {e}", out.display())));
    let (sha, bytes) = file_sha256(&out).unwrap_or_else(|e| die(&e));
    println!(
        "{}",
        canonical_json(
            &json!({"out": out, "sha256": sha, "bytes": bytes, "keys": n, "by_depth": by_depth, "source_sha256": r.sha256})
        )
    );
}

/// `arrivals-bin --fast`: the same key file, with key and depth taken from each canonical arrival line by byte search (the
/// top-level `"key":[` is unique: sender keys are `parent_key` / `term_key`); checked: keys strictly increasing in signed order,
/// the number of lines = the summary's `keys`, the file's sha256 reported. About I/O speed instead of a full parse per line.
fn arrivals_bin_fast(src: &str, out: &Path) {
    use std::io::{BufRead, Write};
    let f = std::fs::File::open(src).unwrap_or_else(|e| die(&format!("{src}: {e}")));
    let mut rd = std::io::BufReader::with_capacity(1 << 24, f);
    let mut h = rustred_campaign_core::closure::sha256::Sha256::new();
    let (mut n, mut arity) = (0u64, 0usize);
    let mut header: Option<Value> = None;
    let mut summary: Option<Value> = None;
    let mut by_depth: BTreeMap<u32, u64> = BTreeMap::new();
    let mut body: Vec<u8> = vec![];
    let mut prev: Vec<i8> = vec![];
    let mut key: Vec<i8> = vec![];
    let mut line: Vec<u8> = vec![];
    let find = |hay: &[u8], pat: &[u8]| hay.windows(pat.len()).position(|w| w == pat);
    loop {
        line.clear();
        let k = rd
            .read_until(b'\n', &mut line)
            .unwrap_or_else(|e| die(&format!("{src}: {e}")));
        if k == 0 {
            break;
        }
        h.update(&line);
        let l = if line.last() == Some(&b'\n') {
            &line[..line.len() - 1]
        } else {
            &line[..]
        };
        if header.is_none() {
            header = Some(
                serde_json::from_slice(l).unwrap_or_else(|e| die(&format!("{src} header: {e}"))),
            );
            continue;
        }
        if find(l, b"\"kind\":\"arrival\"").is_none() {
            summary = Some(
                serde_json::from_slice(l)
                    .unwrap_or_else(|e| die(&format!("{src} line {}: {e}", n + 2))),
            );
            continue;
        }
        let d: u32 = if l.starts_with(b"{\"depth\":") {
            let s = &l[9..];
            let e = s
                .iter()
                .position(|c| !c.is_ascii_digit())
                .unwrap_or(s.len());
            std::str::from_utf8(&s[..e])
                .ok()
                .and_then(|x| x.parse().ok())
                .unwrap_or_else(|| die(&format!("{src} line {}: depth", n + 2)))
        } else {
            0
        };
        let p = find(l, b"\"key\":[")
            .unwrap_or_else(|| die(&format!("{src} line {}: no key", n + 2)))
            + 7;
        let e = p + l[p..]
            .iter()
            .position(|&c| c == b']')
            .unwrap_or_else(|| die(&format!("{src} line {}: key", n + 2)));
        key.clear();
        for x in l[p..e].split(|&c| c == b',') {
            let v: i32 = std::str::from_utf8(x)
                .ok()
                .and_then(|s| s.trim().parse().ok())
                .unwrap_or_else(|| die(&format!("{src} line {}: key slot", n + 2)));
            key.push(
                i8::try_from(v)
                    .unwrap_or_else(|_| die(&format!("{src} line {}: slot {v} outside i8", n + 2))),
            );
        }
        if arity == 0 {
            arity = key.len();
        }
        if key.len() != arity || (n > 0 && key <= prev) {
            die(&format!("{src} line {}: key arity / order", n + 2));
        }
        body.extend(key.iter().map(|&x| x as u8));
        body.push(u8::try_from(d).unwrap_or(u8::MAX));
        body.push(1);
        *by_depth.entry(d).or_default() += 1;
        std::mem::swap(&mut prev, &mut key);
        n += 1;
    }
    let header = header.unwrap_or_else(|| die(&format!("{src}: empty")));
    let want = summary.as_ref().and_then(|s| s["keys"].as_u64());
    if want != Some(n) {
        die(&format!("{src}: {n} arrival lines, summary keys {want:?}"));
    }
    let lines: Vec<usize> = header["child"]["lines"]
        .as_array()
        .map(|v| {
            v.iter()
                .filter_map(|x| x.as_u64().map(|x| x as usize))
                .collect()
        })
        .unwrap_or_default();
    let frame: String = (0..arity)
        .map(|i| if lines.contains(&i) { '1' } else { '0' })
        .collect();
    let src_sha = h.finish_hex();
    let hdr = json!({"schema": "rustred.campaign.arrival-keys.v1", "source": src, "source_sha256": src_sha, "frame": frame,
        "arity": arity, "record": format!("{arity} x i8 key (child table frame), u8 chain depth, u8 sending parents (1)"), "extraction": "fast"});
    let hb = hdr.to_string().into_bytes();
    let tmp = out.with_extension("partial");
    let mut w = std::io::BufWriter::with_capacity(
        1 << 22,
        std::fs::File::create(&tmp).unwrap_or_else(|e| die(&format!("{}: {e}", tmp.display()))),
    );
    for b in [
        &b"RRKEYS01"[..],
        &(hb.len() as u32).to_le_bytes(),
        &hb,
        &body,
    ] {
        w.write_all(b)
            .unwrap_or_else(|e| die(&format!("{}: {e}", tmp.display())));
    }
    w.flush().unwrap_or_else(|e| die(&e.to_string()));
    drop(w);
    std::fs::rename(&tmp, out).unwrap_or_else(|e| die(&format!("{}: {e}", out.display())));
    let (sha, bytes) = file_sha256(out).unwrap_or_else(|e| die(&e));
    println!(
        "{}",
        canonical_json(
            &json!({"out": out, "sha256": sha, "bytes": bytes, "keys": n, "by_depth": by_depth, "source_sha256": src_sha, "extraction": "fast"})
        )
    );
}

/// `cone-excess --tables T [..] --out F`: per node (the union of its tables' rule records) the exact hand-down excess per
/// drop mask, the top terms' excess, the largest feasible mask (core `import::arrivals::cone`). Frames must agree.
fn cone_excess(a: &Args) {
    use rustred_campaign_core::import::arrivals::cone::{NodeExcess, read_rules};
    let t0 = std::time::Instant::now();
    if a.tables.is_empty() {
        die("--tables missing");
    }
    let out = a.out.clone().unwrap_or_else(|| die("--out missing"));
    let mut ne = NodeExcess::default();
    let mut frame: Option<Vec<bool>> = None;
    let mut tables = vec![];
    let mut term_lines: Vec<String> = vec![];
    for p in &a.tables {
        let (fr, rules) = read_rules(p).unwrap_or_else(|e| die(&e));
        match &frame {
            None => frame = Some(fr.clone()),
            Some(f) if *f != fr => die(&format!(
                "{}: frame differs from the first table's",
                p.display()
            )),
            _ => {}
        }
        for r in &rules {
            ne.add_rule(r, &fr);
        }
        if !a.masks.is_empty() {
            let want: BTreeSet<u64> = a
                .masks
                .iter()
                .map(|m| {
                    m.bytes()
                        .enumerate()
                        .filter(|(_, b)| *b == b'1')
                        .map(|(i, _)| 1u64 << i)
                        .sum()
                })
                .collect();
            for r in &rules {
                for (j, m, planes) in
                    rustred_campaign_core::import::arrivals::cone::terms_with_masks(r, &fr, &want)
                {
                    term_lines.push(canonical_json(&json!({"record_sha256": r.sha256, "rule": r.index, "term": j,
                        "mask": rustred_campaign_core::import::arrivals::cone::mask_string(m, fr.len()),
                        "planes": planes.iter().map(|(s, pl)| match pl {
                            Some((q, lo, hi)) => json!({"slot": s, "param": q, "lo": lo, "hi": hi}),
                            None => json!({"slot": s, "constant": true}),
                        }).collect::<Vec<_>>(), "table": p})));
                }
            }
        }
        let (sha, bytes) = file_sha256(p).unwrap_or_else(|e| die(&e));
        tables.push(json!({"path": p, "sha256": sha, "bytes": bytes, "records": rules.len()}));
    }
    let frame = frame.expect("frame");
    if let Some(tp) = &a.terms_out {
        std::fs::write(
            tp,
            term_lines.join("\n") + if term_lines.is_empty() { "" } else { "\n" },
        )
        .unwrap_or_else(|e| die(&format!("{}: {e}", tp.display())));
    }
    let s = json!({"schema": "rustred.campaign.cone-excess.v1", "tables": tables,
        "frame": frame.iter().map(|&l| if l { '1' } else { '0' }).collect::<String>(), "excess": ne,
        "definition": "excess = D + r(sub) of the hand-down term (dropped lines counted as numerators) - D + r of the target, max over the record's rule-point range per drop mask; equations and exception branches ignored (sound)",
        "seconds": t0.elapsed().as_secs_f64(), "tool": arrivals::TOOL_VERSION});
    write_json(&out, &s);
    println!(
        "{}",
        canonical_json(
            &json!({"out": out, "records": s["excess"]["records"], "terms": s["excess"]["terms"],
        "top_max": s["excess"]["top_max"], "handdown_max": s["excess"]["handdown_max"],
        "max_lines_dropped": s["excess"]["max_lines_dropped"], "undecided": s["excess"]["undecided"].as_array().map_or(0, |v| v.len())})
        )
    );
}

/// `edge-cone --edge E [..] --out F` (JSON lines): per edge file its sha256 and the child-side image of every routed sector.
fn edge_cone(a: &Args) {
    use rustred_campaign_core::import::arrivals::cone::{edge_image, mask_string};
    let out = a.out.clone().unwrap_or_else(|| die("--out missing"));
    let mut lines = vec![];
    for p in &a.edges {
        let e = EdgeFile::read(p, &a.map).unwrap_or_else(|x| die(&x));
        let pn = e.parent.momenta.len();
        let imgs = edge_image(&e);
        let masks: Vec<String> = e
            .sectors
            .iter()
            .map(|s| mask_string(s.dropped_mask, pn))
            .collect();
        lines.push(canonical_json(&json!({"edge": p, "sha256": e.sha256, "parent": e.parent.label, "child": e.child.label,
            "parent_lines": e.parent.table_lines, "child_lines": e.child.table_lines, "child_arity": e.child.momenta.len(),
            "masks": masks, "sectors": imgs, "bound_holds": imgs.iter().all(|s| s.bound_holds)})));
    }
    std::fs::write(&out, lines.join("\n") + "\n")
        .unwrap_or_else(|e| die(&format!("{}: {e}", out.display())));
    println!(
        "{}",
        canonical_json(&json!({"out": out, "edges": lines.len()}))
    );
}

/// `arrival-dr --arrivals F [..] --out F` (JSON lines): per arrival file (streamed, verified) D / r / D + r statistics of its
/// keys in the child's frame (the header's child lines).
fn arrival_dr(a: &Args) {
    use rustred_campaign_core::import::arrivals::cone::DrStats;
    let out = a.out.clone().unwrap_or_else(|| die("--out missing"));
    let files: Vec<String> = a.arrivals.clone();
    let res = arrivals::par_map(&files, a.threads, |p: &String| -> Result<Value, String> {
        // the header (first line): the child's line slots
        let first = {
            use std::io::BufRead;
            let f = std::fs::File::open(p).map_err(|e| format!("{p}: {e}"))?;
            let mut s = String::new();
            std::io::BufReader::new(f)
                .read_line(&mut s)
                .map_err(|e| format!("{p}: {e}"))?;
            s
        };
        let h: Value = serde_json::from_str(&first).map_err(|e| format!("{p}: header: {e}"))?;
        let child_lines: Vec<usize> = serde_json::from_value(h["child"]["lines"].clone())
            .map_err(|e| format!("{p}: child lines: {e}"))?;
        let mut st = DrStats::default();
        let mut fr: Option<Vec<bool>> = None;
        let r = format::read_streamed(Path::new(p), &mut |l| {
            let f = fr.get_or_insert_with(|| {
                let mut v = vec![false; l.key.len()];
                for &j in &child_lines {
                    if j < v.len() {
                        v[j] = true;
                    }
                }
                v
            });
            st.add(&l.key, f);
            Ok(())
        })?;
        Ok(
            json!({"file": p, "sha256": r.sha256, "parent": r.header["parent"]["node"], "child": r.header["child"]["label"],
            "edge_sha256": r.header["edge"]["sha256"], "child_lines": child_lines, "stats": st}),
        )
    });
    let mut lines = vec![];
    for r in res {
        lines.push(canonical_json(&r.unwrap_or_else(|e| die(&e))));
    }
    std::fs::write(&out, lines.join("\n") + "\n")
        .unwrap_or_else(|e| die(&format!("{}: {e}", out.display())));
    println!(
        "{}",
        canonical_json(&json!({"out": out, "files": lines.len()}))
    );
}

/// `check-zero-claims --spec S --proofs P [..] --out F`: load proof files (zero lines, hand-down class included) for the
/// spec's import set through the CORE loader (`import::zero_proofs::ZeroProofs::load`: zerosfix3 semantics, every proved
/// claim re-checked against its record's own text; a false claim refuses), and write its counters (or the refusal).
fn check_zero_claims(a: &Args) {
    let spec = a.spec.clone().unwrap_or_else(|| die("--spec missing"));
    let out = a.out.clone().unwrap_or_else(|| die("--out missing"));
    if a.proofs.is_empty() {
        die("--proofs missing");
    }
    let (set, _) = load_spec(&spec, &a.index_dir, |_| {});
    let r = rustred_campaign_core::import::zero_proofs::ZeroProofs::load(
        &set,
        &a.proofs,
        &rustred_campaign_core::import::zero_proofs::LoadOptions::default(),
    );
    let s = match r {
        Ok(z) => {
            json!({"schema": "rustred.campaign.zero-claims-check.v1", "spec": spec, "proofs": a.proofs,
            "loaded": true, "stats": z.stats,
            "ok": z.stats.claims_false == 0 && z.stats.claims_unchecked == 0 && z.stats.den_text_mismatch == 0})
        }
        Err(e) => {
            json!({"schema": "rustred.campaign.zero-claims-check.v1", "spec": spec, "proofs": a.proofs,
            "loaded": false, "refusal": String::from(e), "ok": false})
        }
    };
    write_json(&out, &s);
    println!("{}", canonical_json(&s));
}

/// `handdown-witness --spec S --terms F --out W [--sample K]` (K = tries per term, default 400): for every listed term (JSON
/// lines `{record_sha256, term, mask, planes}` as cone-excess --terms-out writes them) search a KEY where the record applies
/// (fixed slots, equations, bounds, exception branches: `CaseDomain::applies`), the term's drop mask is exactly `mask` (no
/// positive numerator) and its coefficient is NON-ZERO exactly (`RecordProver::word_at`); report the key, the term key, the
/// key's D + r and whether the record is the dispatcher's FIRST MATCH there. Canonical targets only (the key = the rule
/// point); deterministic search (seeded).
fn handdown_witness(a: &Args) {
    use std::sync::Mutex;
    let spec = a.spec.clone().unwrap_or_else(|| die("--spec missing"));
    let out = a.out.clone().unwrap_or_else(|| die("--out missing"));
    let tf = a.terms.clone().unwrap_or_else(|| die("--terms missing"));
    let tries = if a.sample == 0 { 400 } else { a.sample };
    let alg = algebra();
    let cfg = prover_cfg(a);
    let (set, _) = load_spec(&spec, &a.index_dir, |_| {});
    let text =
        std::fs::read_to_string(&tf).unwrap_or_else(|e| die(&format!("{}: {e}", tf.display())));
    let mut by_rec: BTreeMap<String, Vec<Value>> = BTreeMap::new();
    for l in text.lines().filter(|l| !l.trim().is_empty()) {
        let v: Value =
            serde_json::from_str(l).unwrap_or_else(|e| die(&format!("{}: {e}", tf.display())));
        by_rec
            .entry(v["record_sha256"].as_str().unwrap_or("").to_string())
            .or_default()
            .push(v);
    }
    let lines = set.frame.mask().to_vec();
    let n = set.arity();
    let frames = Mutex::new(BTreeMap::new());
    let recs: Vec<(String, Vec<Value>)> = by_rec.into_iter().collect();
    let res = arrivals::par_map(&recs, a.threads, |(sha, entries)| -> Vec<Value> {
        let fail = |why: String| {
            entries.iter().map(|e| json!({"record_sha256": sha, "term": e["term"], "mask": e["mask"], "witness": null, "why": why})).collect::<Vec<_>>()
        };
        let Some((t, r)) = set.find_sha(sha) else {
            return fail("record not in the set".into());
        };
        let (rt, frame, _) = match arrivals::read_record(&set, &frames, t, r) {
            Ok(x) => x,
            Err(e) => return fail(e),
        };
        let mut p = match zero_proofs::RecordProver::new(alg, cfg.clone(), &rt, frame.mask()) {
            Ok(p) => p,
            Err(e) => return fail(e),
        };
        let mut outv = vec![];
        for e in entries {
            let j = e["term"].as_u64().unwrap_or(0) as usize;
            let want: u64 = e["mask"]
                .as_str()
                .unwrap_or("")
                .bytes()
                .enumerate()
                .filter(|(_, b)| *b == b'1')
                .map(|(i, _)| 1u64 << i)
                .sum();
            // per parameter: the candidate interval (chart of the target slot, fixed slots, drop planes)
            let mut lo: Vec<i64> = (0..n).map(|q| if lines[q] { 1 } else { -6 }).collect();
            let mut hi: Vec<i64> = (0..n).map(|q| if lines[q] { 6 } else { 0 }).collect();
            for (q, f) in p.domain().fixed.iter().enumerate() {
                if let Some(v) = f {
                    lo[q] = *v as i64;
                    hi[q] = *v as i64;
                }
            }
            for pl in e["planes"].as_array().into_iter().flatten() {
                if let (Some(q), plo, Some(phi)) =
                    (pl["param"].as_u64(), pl["lo"].as_i64(), pl["hi"].as_i64())
                {
                    let q = q as usize;
                    lo[q] = lo[q].max(plo.unwrap_or(phi - 3));
                    hi[q] = hi[q].min(phi);
                }
            }
            let mut found: Option<Value> = None;
            let mut seed: u64 = 0x9E3779B97F4A7C15 ^ (j as u64);
            for k in 0..tries {
                let key: Vec<i32> = (0..n)
                    .map(|q| {
                        if lo[q] > hi[q] {
                            return lo[q] as i32;
                        }
                        let span = (hi[q] - lo[q] + 1) as u64;
                        let v = if k == 0 {
                            if lines[q] { lo[q] } else { hi[q] }
                        } else {
                            seed ^= seed << 13;
                            seed ^= seed >> 7;
                            seed ^= seed << 17;
                            lo[q] + (seed % span) as i64
                        };
                        v as i32
                    })
                    .collect();
                if !matches!(
                    p.domain().applies(&key),
                    rustred_campaign_core::domain::Verdict::Applies
                ) {
                    continue;
                }
                let Some(pt) = p.point(&key) else { continue };
                let Some(tk) = p.term_key(j, &pt) else {
                    continue;
                };
                let mut m = 0u64;
                let mut pos = false;
                for (s, &v) in tk.iter().enumerate() {
                    if lines[s] {
                        if v <= 0 {
                            m |= 1 << s;
                        }
                    } else if v > 0 {
                        pos = true;
                    }
                }
                if pos || m != want {
                    continue;
                }
                if p.word_at(j, &pt) != CoefFact::NonZero {
                    continue;
                }
                let fm = match set.disp.first_match(&key) {
                    Ok(rustred_campaign_core::cover::Dispatch::Covered(rr)) => {
                        if (rr.table, rr.record) == (t, r) {
                            "this record".to_string()
                        } else {
                            format!("another record ({}, {})", rr.table, rr.record)
                        }
                    }
                    Ok(d) => format!("{d:?}").chars().take(400).collect(),
                    Err(x) => format!("error {x:?}").chars().take(400).collect(),
                };
                let dr: i64 = key
                    .iter()
                    .enumerate()
                    .map(|(s, &v)| if lines[s] { v as i64 - 1 } else { -(v as i64) })
                    .sum();
                found = Some(
                    json!({"key": key, "term_key": tk, "d_plus_r": dr, "first_match": fm, "tries": k + 1}),
                );
                break;
            }
            outv.push(json!({"record_sha256": sha, "rule": e["rule"], "term": j, "mask": e["mask"],
                "witness": found, "why": if found.is_none() { json!(format!("no key in {tries} tries")) } else { Value::Null }}));
        }
        outv
    });
    let mut text = String::new();
    for v in res.into_iter().flatten() {
        text.push_str(&canonical_json(&v));
        text.push('\n');
    }
    std::fs::write(&out, text).unwrap_or_else(|e| die(&format!("{}: {e}", out.display())));
    println!("{}", canonical_json(&json!({"out": out})));
}

fn header_family(p: &Path) -> Option<String> {
    let h = rustred_campaign_core::import::lines::read_header("arrivals", p).ok()?;
    h.raw
        .get("family")
        .and_then(Value::as_str)
        .map(str::to_string)
}

fn main() {
    let argv: Vec<String> = std::env::args().collect();
    let Some(cmd) = argv.get(1) else {
        die(
            "usage: rr_arrivals prove-handdowns|verify-handdown-proofs|compute|verify|child-check|second-route|version ...",
        )
    };
    let a = parse(&argv[2..]);
    match cmd.as_str() {
        "version" => println!(
            "{}",
            json!({"program": "rr_arrivals", "version": arrivals::TOOL_VERSION, "crate": env!("CARGO_PKG_VERSION"), "symbolica": zero_proofs::SYMBOLICA_REV})
        ),
        "prove-handdowns" => prove(&a),
        "verify-handdown-proofs" => verify_proofs(&a),
        "compute" => compute(&a),
        "verify" => verify(&a),
        "child-check" => child_check(&a),
        "second-route" => second_route(&a),
        "compare-engine" => compare_engine(&a),
        "closure-rows" => closure_rows(&a),
        "arrivals-bin" => arrivals_bin(&a),
        "cone-excess" => cone_excess(&a),
        "edge-cone" => edge_cone(&a),
        "arrival-dr" => arrival_dr(&a),
        "check-zero-claims" => check_zero_claims(&a),
        "handdown-witness" => handdown_witness(&a),
        x => die(&format!("unknown command {x}")),
    }
}
