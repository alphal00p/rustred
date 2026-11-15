//! `rr_coef_facts`: EXACT coefficient facts of rule records from their own coefficient text, on Symbolica (lane
//! closeloop3; interfaces/closeloop.md 16). A LEGACY child of the campaign (contract kind `coefficient-facts`): no job
//! mode; the supervisor-side wrapper checks every answer against the request, seals and writes OUTCOME.
//!
//! ```text
//! rr_coef_facts facts --requests R --out OUT [--threads N] [--detail]
//! rr_coef_facts crosscheck --requests R --answers A [--answers A2 ..] --out REPORT [--threads N] [--max-pairs N]
//! rr_coef_facts version
//! rr_coef_facts prove --tables T [--tables T2 ..] --out OUT [--threads N] [--max-planes K] [--summary S] [--all-lines]
//! rr_coef_facts verify-zero-proofs --tables T [..] --proofs P [..] [--threads N] [--summary S]
//! ```
//! lane zeros2 (interfaces/zero-proofs.md, closeloop.md 23): a `facts` request line carrying `"terms": [[j..] per key]`
//! (and `"zeros": "structural" | "per_key"`) is a ZEROS request: only the asked terms are decided at each key, by a
//! structural proof per (term, locus) where one exists (else the exact per-key evaluation); OUT then holds one
//! `rustred.campaign.zero-verdict.v1` line per (record, key) and the `rustred.campaign.zero-proof.v1` lines of every
//! (term, locus) decided. `prove`: every positive-numerator locus of every rule record of the tables, proof lines only.
//! `facts`: R = rr_proof instantiate form (A) lines `{"table", "offset", "rule", "keys"}`; OUT = one
//! `rustred.campaign.fact-verdict.v1` line per (record, key) with `producer: exact_eval` (written as OUT.part, then
//! renamed). `crosscheck`: the same evaluation (with per-term detail) compared with rr_proof instantiate answers A
//! (`rustred.proof-kernel.instantiate.v1` lines) on every (record, key) both answer; REPORT = the counts and the first
//! disagreements; exit 3 if any disagreement. Errors: one `{"verdict":"ERROR","error":..}` line on stdout, exit 2
//! (rr_proof's convention: the wrapper maps it to a refusal, or to an io end when it names a host-resource errno).
//! The Symbolica licence must be in the environment (`source WS/ops/env.sh`); without it the program refuses (exit 2).

#[path = "../coef_facts.rs"]
#[allow(dead_code)]
mod coef_facts;
#[path = "../zero_proofs.rs"]
#[allow(dead_code)]
mod zero_proofs;

use rustred_campaign_algebra::SymbolicaAlgebra;
use serde_json::{Value, json};
use std::collections::{HashMap, HashSet};
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};

fn die(msg: &str) -> ! {
    println!("{}", json!({"verdict": "ERROR", "error": msg}));
    std::process::exit(2)
}

struct Args {
    requests: Option<PathBuf>,
    answers: Vec<PathBuf>,
    out: Option<PathBuf>,
    threads: usize,
    detail: bool,
    max_pairs: Option<usize>,
    tables: Vec<PathBuf>,
    max_planes: Option<usize>,
    summary: Option<PathBuf>,
    all_lines: bool,
    proofs: Vec<PathBuf>,
}

fn parse(args: &[String]) -> Args {
    let mut a = Args {
        requests: None,
        answers: vec![],
        out: None,
        threads: 1,
        detail: false,
        max_pairs: None,
        tables: vec![],
        max_planes: None,
        summary: None,
        all_lines: false,
        proofs: vec![],
    };
    let mut i = 0;
    let val = |i: &mut usize| -> String {
        *i += 1;
        args.get(*i)
            .cloned()
            .unwrap_or_else(|| die(&format!("{} needs a value", args[*i - 1])))
    };
    while i < args.len() {
        match args[i].as_str() {
            "--requests" => a.requests = Some(PathBuf::from(val(&mut i))),
            "--answers" => a.answers.push(PathBuf::from(val(&mut i))),
            "--out" => a.out = Some(PathBuf::from(val(&mut i))),
            "--threads" => {
                a.threads = val(&mut i)
                    .parse()
                    .unwrap_or_else(|_| die("--threads: not a number"));
                if a.threads == 0 || a.threads > 16 {
                    die("--threads: 1..16");
                }
            }
            "--max-pairs" => {
                a.max_pairs = Some(
                    val(&mut i)
                        .parse()
                        .unwrap_or_else(|_| die("--max-pairs: not a number")),
                )
            }
            "--detail" => a.detail = true,
            "--tables" => a.tables.push(PathBuf::from(val(&mut i))),
            "--all-lines" => a.all_lines = true,
            "--proofs" => a.proofs.push(PathBuf::from(val(&mut i))),
            "--summary" => a.summary = Some(PathBuf::from(val(&mut i))),
            "--max-planes" => {
                a.max_planes = Some(
                    val(&mut i)
                        .parse()
                        .unwrap_or_else(|_| die("--max-planes: not a number")),
                )
            }
            x => die(&format!("unknown argument {x:?}")),
        }
        i += 1;
    }
    a
}

fn write_atomic(path: &Path, lines: &[Value]) {
    let part = path.with_extension("part");
    let mut s = String::new();
    for l in lines {
        s.push_str(&l.to_string());
        s.push('\n');
    }
    if let Err(e) = std::fs::write(&part, s) {
        die(&format!("{}: {e}", part.display()));
    }
    if let Err(e) = std::fs::rename(&part, path) {
        die(&format!("{}: {e}", path.display()));
    }
}

fn backend(threads: usize) -> coef_facts::SymbolicaRecordFacts {
    match SymbolicaAlgebra::new() {
        Ok(a) => coef_facts::SymbolicaRecordFacts::new(a).with_threads(threads),
        Err(e) => die(&format!("UNSUPPORTED: {e}")),
    }
}

fn requests(a: &Args) -> Vec<coef_facts::Request> {
    let p = a
        .requests
        .as_ref()
        .unwrap_or_else(|| die("--requests missing"));
    let t = std::fs::read_to_string(p).unwrap_or_else(|e| die(&format!("{}: {e}", p.display())));
    let r = coef_facts::parse_requests(&t).unwrap_or_else(|e| die(&e));
    if r.is_empty() {
        die("no request");
    }
    r
}

/// lane zeros2: the zeros part of every request line (None = an ordinary facts request): the asked terms per key and
/// whether the per-key method is forced.
fn zeros_requests(a: &Args) -> Vec<Option<(Vec<Vec<usize>>, bool)>> {
    let p = a.requests.as_ref().expect("checked");
    let t = std::fs::read_to_string(p).unwrap_or_else(|e| die(&format!("{}: {e}", p.display())));
    t.lines()
        .filter(|l| !l.trim().is_empty())
        .enumerate()
        .map(|(i, l)| {
            let v: Value =
                serde_json::from_str(l).unwrap_or_else(|e| die(&format!("request {}: {e}", i + 1)));
            if v.get("terms").is_none() {
                return None;
            }
            let terms: Vec<Vec<usize>> = serde_json::from_value(v["terms"].clone())
                .unwrap_or_else(|e| die(&format!("request {}: terms: {e}", i + 1)));
            let per_key = match v["zeros"].as_str() {
                None | Some("structural") => false,
                Some("per_key") => true,
                Some(o) => die(&format!(
                    "request {}: zeros {o:?} (structural | per_key)",
                    i + 1
                )),
            };
            let nk = v["keys"].as_array().map_or(0, Vec::len);
            if terms.len() != nk {
                die(&format!(
                    "request {}: {} term lists for {nk} keys",
                    i + 1,
                    terms.len()
                ));
            }
            Some((terms, per_key))
        })
        .collect()
}

fn prover_cfg(a: &Args) -> zero_proofs::ProverCfg {
    let mut c = zero_proofs::ProverCfg::default();
    if let Some(m) = a.max_planes {
        c.max_planes = m;
    }
    if a.all_lines {
        c.report = zero_proofs::Report::All;
    }
    c
}

fn algebra() -> SymbolicaAlgebra {
    SymbolicaAlgebra::new().unwrap_or_else(|e| die(&format!("UNSUPPORTED: {e}")))
}

/// lane zeros2: one zeros request (one record at its keys).
fn zeros_answer(
    alg: &SymbolicaAlgebra,
    r: &coef_facts::Request,
    terms: &[Vec<usize>],
    per_key: bool,
    a: &Args,
) -> Result<Vec<Value>, String> {
    let (frame, _) = coef_facts::table_frame(&r.table)?;
    let (line, _sha) = coef_facts::line_at(&r.table, r.offset)?;
    let rt = rustred_campaign_core::accept::rule::RuleText::parse(&line, &frame)
        .map_err(|e| format!("{}@{}: {e:?}", r.table.display(), r.offset))?;
    if rt.index() != Some(r.rule) {
        return Err(format!(
            "the record at offset {} of {} is rule {:?}, not {}",
            r.offset,
            r.table.display(),
            rt.index(),
            r.rule
        ));
    }
    let mut cfg = prover_cfg(a);
    cfg.report = zero_proofs::Report::Proved;
    let mut p = zero_proofs::RecordProver::new(alg, cfg, &rt, frame.mask())?;
    let (mut v, pr) = zero_proofs::answer_record(&mut p, &r.keys, terms, per_key);
    v.extend(pr);
    Ok(v)
}

/// lane zeros2: the header line of a proof file: the tables (path, sha256, bytes), their digest, the producer.
fn proof_header(tables: &[PathBuf]) -> Value {
    let mut list = vec![];
    for tp in tables {
        let (sha, bytes) = file_sha256(tp).unwrap_or_else(|e| die(&e));
        list.push(json!({"path": tp, "sha256": sha, "bytes": bytes}));
    }
    let mut h = rustred_campaign_core::closure::sha256::Sha256::new();
    let mut names: Vec<(String, String)> = list
        .iter()
        .map(|v| {
            (
                v["path"].as_str().unwrap_or("").to_string(),
                v["sha256"].as_str().unwrap_or("").to_string(),
            )
        })
        .collect();
    names.sort();
    for (p, s) in &names {
        h.update(format!("{p}\t{s}\n").as_bytes());
    }
    json!({"schema": zero_proofs::HEADER_SCHEMA, "tables": list, "tables_digest": h.finish_hex(),
        "tool": zero_proofs::tool(), "method": zero_proofs::METHOD, "method_doc": zero_proofs::METHOD_DOC})
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

/// Run `work` on every rule-record offset of `tp` on `threads` threads, in chunks of `chunk` records; `emit` gets each
/// chunk's results in record order (bounded memory over huge tables).
fn for_records<R: Send>(
    tp: &Path,
    threads: usize,
    chunk: usize,
    work: impl Fn(u64) -> R + Sync,
    mut emit: impl FnMut(Vec<R>),
) -> Result<(), String> {
    use std::sync::Mutex;
    use std::sync::atomic::{AtomicUsize, Ordering};
    let offsets = rule_offsets(tp)?;
    for part in offsets.chunks(chunk.max(1)) {
        let next = AtomicUsize::new(0);
        let results: Vec<Mutex<Option<R>>> = part.iter().map(|_| Mutex::new(None)).collect();
        std::thread::scope(|s| {
            for _ in 0..threads.max(1) {
                s.spawn(|| {
                    loop {
                        let i = next.fetch_add(1, Ordering::Relaxed);
                        let Some(&off) = part.get(i) else { break };
                        let r = work(off);
                        *results[i].lock().expect("lock") = Some(r);
                    }
                });
            }
        });
        emit(
            results
                .into_iter()
                .filter_map(|m| m.into_inner().expect("lock"))
                .collect(),
        );
    }
    Ok(())
}

/// lane zeros2: `prove`: every positive-numerator locus of every rule record of the tables (records in parallel on
/// `--threads` threads, chunks of 2048 records streamed to OUT in file / record order). OUT starts with the header line
/// (tables, sha256, digest, producer).
fn prove(a: &Args) {
    use std::io::Write;
    if a.tables.is_empty() {
        die("--tables missing");
    }
    let out = a.out.clone().unwrap_or_else(|| die("--out missing"));
    let t0 = std::time::Instant::now();
    let alg = algebra();
    let cfg = prover_cfg(a);
    let header = proof_header(&a.tables);
    let part = out.with_extension("part");
    let mut w = std::io::BufWriter::new(
        std::fs::File::create(&part).unwrap_or_else(|e| die(&format!("{}: {e}", part.display()))),
    );
    writeln!(w, "{header}").unwrap_or_else(|e| die(&e.to_string()));
    let mut total = zero_proofs::PosnumCounts::default();
    let mut errors: Vec<String> = vec![];
    for tp in &a.tables {
        let (frame, _) = coef_facts::table_frame(tp).unwrap_or_else(|e| die(&e));
        let work = |off: u64| -> Result<Option<(Vec<Value>, zero_proofs::PosnumCounts)>, String> {
            let (line, _) = coef_facts::line_at(tp, off)?;
            let rt = match rustred_campaign_core::accept::rule::RuleText::parse(&line, &frame) {
                Ok(rt) => rt,
                Err(rustred_campaign_core::accept::rule::RuleTextError::NotARule(_)) => {
                    return Ok(None);
                }
                Err(e) => return Err(format!("{}@{off}: {e:?}", tp.display())),
            };
            let mut c = zero_proofs::PosnumCounts::default();
            let mut p = zero_proofs::RecordProver::new(&alg, cfg.clone(), &rt, frame.mask())
                .map_err(|e| format!("{}@{off}: {e}", tp.display()))?;
            let lines = zero_proofs::prove_posnum_record(&mut p, &mut c);
            Ok(Some((lines, c)))
        };
        for_records(tp, a.threads, 2048, work, |res| {
            for r in res {
                match r {
                    Ok(Some((lines, c))) => {
                        total.add(&c);
                        for l in lines {
                            writeln!(w, "{l}").unwrap_or_else(|e| die(&e.to_string()));
                        }
                    }
                    Ok(None) => {}
                    Err(e) => {
                        total.records_refused += 1;
                        errors.push(e);
                    }
                }
            }
        })
        .unwrap_or_else(|e| die(&e));
    }
    w.flush().unwrap_or_else(|e| die(&e.to_string()));
    drop(w);
    std::fs::rename(&part, &out).unwrap_or_else(|e| die(&format!("{}: {e}", out.display())));
    let (osha, obytes) = file_sha256(&out).unwrap_or_else(|e| die(&e));
    let s = json!({"schema": "rustred.campaign.zero-proof-summary.v1", "tables_digest": header["tables_digest"],
        "out": out, "out_sha256": osha, "out_bytes": obytes,
        "threads": a.threads, "seconds": t0.elapsed().as_secs_f64(), "counts": total,
        "errors": errors.len(), "first_errors": errors.iter().take(20).collect::<Vec<_>>(), "tool": zero_proofs::tool()});
    if let Some(p) = &a.summary {
        write_atomic(p, std::slice::from_ref(&s));
    }
    println!("{s}");
}

/// lane zeros2: `verify-zero-proofs`: re-check a proof file from the TABLE BYTES without trusting the prover's run:
/// the header's table digests; every record re-hashed and found by its sha256; every positive-numerator locus of every
/// record RE-DERIVED (Symbolica) and compared with the file (a proved line must re-derive proved with the same
/// denominator class; a not-proved / excluded line the same; a locus without a line must re-derive excluded or be
/// covered by a domain proof); every proved numerator claim RE-CHECKED by the core's own exact polynomial (independent
/// of Symbolica; claims that used case equations are re-derived only), every `nowhere_zero` denominator by the core's
/// constant-d-coefficient test where that criterion applies. Exit 0 = every claim verified; 3 = a mismatch; 2 = error.
fn verify(a: &Args) {
    use std::collections::{BTreeMap, HashMap};
    if a.tables.is_empty() || a.proofs.is_empty() {
        die("--tables and --proofs are required");
    }
    let t0 = std::time::Instant::now();
    let alg = algebra();
    let mut cfg = prover_cfg(a);
    cfg.report = zero_proofs::Report::All;
    // the file's claims by (record sha256, term, locus)
    type Claim = (String, Option<String>);
    let mut claims: HashMap<
        String,
        BTreeMap<(usize, rustred_campaign_core::import::zero_proofs::Locus), Claim>,
    > = HashMap::new();
    let mut headers: Vec<Value> = vec![];
    for p in &a.proofs {
        let f = std::fs::File::open(p).unwrap_or_else(|e| die(&format!("{}: {e}", p.display())));
        for (i, l) in BufReader::new(f).lines().enumerate() {
            let l = l.unwrap_or_else(|e| die(&format!("{}: {e}", p.display())));
            if l.trim().is_empty() {
                continue;
            }
            let v: Value = serde_json::from_str(&l)
                .unwrap_or_else(|e| die(&format!("{} line {}: {e}", p.display(), i + 1)));
            if v["schema"] == zero_proofs::HEADER_SCHEMA {
                headers.push(v);
                continue;
            }
            let pl = rustred_campaign_core::import::zero_proofs::parse_line(&v)
                .unwrap_or_else(|e| die(&format!("{} line {}: {e}", p.display(), i + 1)));
            claims.entry(pl.record_sha256.clone()).or_default().insert(
                (pl.term as usize, pl.locus),
                (
                    pl.verdict.clone(),
                    pl.denominator.map(|d| d.word().to_string()),
                ),
            );
        }
    }
    // header binding: every table named there must have the sha256 it names
    let mut header_mismatch: Vec<String> = vec![];
    for h in &headers {
        for t in h["tables"].as_array().into_iter().flatten() {
            let path = PathBuf::from(t["path"].as_str().unwrap_or(""));
            match file_sha256(&path) {
                Ok((s, _)) if Some(s.as_str()) == t["sha256"].as_str() => {}
                Ok((s, _)) => header_mismatch.push(format!(
                    "{}: sha256 {s}, header {}",
                    path.display(),
                    t["sha256"]
                )),
                Err(e) => header_mismatch.push(e),
            }
        }
    }
    #[derive(Default, serde::Serialize)]
    struct Counts {
        records: u64,
        records_with_claims: u64,
        posnum_terms: u64,
        loci: u64,
        proved: u64,
        excluded: u64,
        not_proved: u64,
        claims_checked: u64,
        independent_numerator: u64,
        independent_denominator: u64,
        denominator_symbolica_only: u64,
        equations_symbolica_only: u64,
        mismatches: u64,
        claims_without_record: u64,
    }
    let mut c = Counts::default();
    let mut mism: Vec<Value> = vec![];
    let mut not_proved_list: Vec<Value> = vec![];
    let mut seen: std::collections::HashSet<String> = std::collections::HashSet::new();
    for tp in &a.tables {
        let (frame, _) = coef_facts::table_frame(tp).unwrap_or_else(|e| die(&e));
        let work = |off: u64| -> Result<Option<(String, Counts, Vec<Value>, Vec<Value>)>, String> {
            let (line, sha) = coef_facts::line_at(tp, off)?;
            let rt = match rustred_campaign_core::accept::rule::RuleText::parse(&line, &frame) {
                Ok(rt) => rt,
                Err(rustred_campaign_core::accept::rule::RuleTextError::NotARule(_)) => {
                    return Ok(None);
                }
                Err(e) => return Err(format!("{}@{off}: {e:?}", tp.display())),
            };
            let mut k = Counts {
                records: 1,
                ..Counts::default()
            };
            let (mut mm, mut np) = (vec![], vec![]);
            let empty = BTreeMap::new();
            let cl = claims.get(&sha).unwrap_or(&empty);
            if !cl.is_empty() {
                k.records_with_claims = 1;
            }
            let mut p = zero_proofs::RecordProver::new(&alg, cfg.clone(), &rt, frame.mask())
                .map_err(|e| format!("{}@{off}: {e}", tp.display()))?;
            let mut loci_all: Vec<(usize, rustred_campaign_core::import::zero_proofs::Locus)> =
                vec![];
            for j in 0..p.n_terms() {
                let (loci, _) = p.posnum_loci(j);
                if loci.is_empty() {
                    continue;
                }
                k.posnum_terms += 1;
                let domain_proved = matches!(
                    p.prove(j, rustred_campaign_core::import::zero_proofs::Locus::Domain),
                    zero_proofs::Outcome::Proved { .. }
                );
                for l in loci {
                    loci_all.push((j, l));
                }
                if domain_proved {
                    loci_all.push((j, rustred_campaign_core::import::zero_proofs::Locus::Domain));
                }
            }
            // claims of loci outside the enumeration are re-derived too
            for (j, l) in cl.keys() {
                if !loci_all.contains(&(*j, *l)) {
                    loci_all.push((*j, *l));
                }
            }
            loci_all.sort();
            loci_all.dedup();
            for (j, l) in loci_all {
                let o = p.prove(j, l);
                let (word, den) = match &o {
                    zero_proofs::Outcome::Proved { den, .. } => {
                        ("zero_on_locus", Some(den.word().to_string()))
                    }
                    zero_proofs::Outcome::NotProved(_) => ("not_proved", None),
                    zero_proofs::Outcome::Excluded(_) => ("excluded", None),
                };
                k.loci += 1;
                match word {
                    "zero_on_locus" => k.proved += 1,
                    "excluded" => k.excluded += 1,
                    _ => {
                        k.not_proved += 1;
                        if np.len() < 1000 {
                            np.push(json!({"record_sha256": sha, "rule": rt.index(), "term": j, "locus": l.json(),
                                "coef": rt.rhs.get(j).map(|x| x.1.chars().take(200).collect::<String>())}));
                        }
                    }
                }
                let claim = cl.get(&(j, l));
                let ok = match claim {
                    Some((w, d)) => {
                        k.claims_checked += 1;
                        w == word && (word != "zero_on_locus" || d.as_deref() == den.as_deref())
                    }
                    // a missing line is allowed for excluded loci, for planes a domain proof covers, and for a failed
                    // domain attempt whose planes carry the claims
                    None => {
                        word == "excluded"
                            || cl
                                .get(&(
                                    j,
                                    rustred_campaign_core::import::zero_proofs::Locus::Domain,
                                ))
                                .is_some_and(|x| x.0 == "zero_on_locus")
                            || (l == rustred_campaign_core::import::zero_proofs::Locus::Domain
                                && word == "not_proved")
                    }
                };
                // independent numerator / denominator re-check of every proved claim
                let mut indep_ok = true;
                if let zero_proofs::Outcome::Proved {
                    den: dst,
                    equations,
                } = &o
                {
                    if equations.is_empty() {
                        let subs = p.substitutions(l);
                        let (nt, dt) = p.side_texts(j)?;
                        match zero_proofs::core_vanishes(&nt, p.arity(), &subs) {
                            Ok(true) => k.independent_numerator += 1,
                            Ok(false) => indep_ok = false,
                            Err(_) => {} // the core parser refuses the text form: Symbolica only
                        }
                        if *dst
                            == rustred_campaign_core::import::zero_proofs::DenStatus::NowhereZero
                        {
                            match dt {
                                None => k.independent_denominator += 1,
                                Some(dt) => match zero_proofs::core_constant_d_coefficient(
                                    &dt,
                                    p.arity(),
                                    &subs,
                                ) {
                                    Ok(true) => k.independent_denominator += 1,
                                    _ => k.denominator_symbolica_only += 1,
                                },
                            }
                        }
                    } else {
                        k.equations_symbolica_only += 1;
                    }
                }
                if !ok || !indep_ok {
                    k.mismatches += 1;
                    if mm.len() < 100 {
                        mm.push(json!({"record_sha256": sha, "term": j, "locus": l.json(), "rederived": word,
                            "denominator": den, "claim": claim.map(|x| json!([x.0, x.1])),
                            "independent_numerator_ok": indep_ok}));
                    }
                }
            }
            Ok(Some((sha, k, mm, np)))
        };
        for_records(tp, a.threads, 2048, work, |res| {
            for r in res {
                match r {
                    Ok(Some((sha, k, mm, np))) => {
                        seen.insert(sha);
                        c.records += k.records;
                        c.records_with_claims += k.records_with_claims;
                        c.posnum_terms += k.posnum_terms;
                        c.loci += k.loci;
                        c.proved += k.proved;
                        c.excluded += k.excluded;
                        c.not_proved += k.not_proved;
                        c.claims_checked += k.claims_checked;
                        c.independent_numerator += k.independent_numerator;
                        c.independent_denominator += k.independent_denominator;
                        c.denominator_symbolica_only += k.denominator_symbolica_only;
                        c.equations_symbolica_only += k.equations_symbolica_only;
                        c.mismatches += k.mismatches;
                        mism.extend(mm);
                        not_proved_list.extend(np);
                    }
                    Ok(None) => {}
                    Err(e) => die(&e),
                }
            }
        })
        .unwrap_or_else(|e| die(&e));
    }
    c.claims_without_record = claims.keys().filter(|s| !seen.contains(*s)).count() as u64;
    let verified = c.mismatches == 0 && header_mismatch.is_empty() && c.claims_without_record == 0;
    let s = json!({"schema": "rustred.campaign.zero-proof-verify.v1", "verified": verified, "tables": a.tables,
        "proofs": a.proofs, "headers": headers.len(), "header_mismatch": header_mismatch, "counts": c,
        "mismatches": mism.into_iter().take(100).collect::<Vec<_>>(), "not_proved": not_proved_list,
        "seconds": t0.elapsed().as_secs_f64(), "tool": zero_proofs::tool()});
    if let Some(p) = &a.summary {
        write_atomic(p, std::slice::from_ref(&s));
    }
    println!(
        "{}",
        json!({"verified": verified, "counts": s["counts"], "seconds": s["seconds"]})
    );
    if !verified {
        std::process::exit(3);
    }
}

/// Byte offsets of every line of a table file that may be a rule record (`"rule"` occurs in its first bytes).
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

fn main() {
    let argv: Vec<String> = std::env::args().collect();
    let Some(cmd) = argv.get(1) else {
        die("usage: rr_coef_facts facts|crosscheck|prove|verify-zero-proofs|version ...")
    };
    let a = parse(&argv[2..]);
    match cmd.as_str() {
        "version" => {
            println!(
                "{}",
                json!({"program": "rr_coef_facts", "version": env!("CARGO_PKG_VERSION"),
                       "producer": coef_facts::PRODUCER, "backend": coef_facts::BACKEND, "method": coef_facts::METHOD,
                       "zero_proofs": {"method": zero_proofs::METHOD, "doc": zero_proofs::METHOD_DOC, "tool": zero_proofs::tool()}})
            );
        }
        "facts" => {
            let reqs = requests(&a);
            let out = a.out.clone().unwrap_or_else(|| die("--out missing"));
            let zreq = zeros_requests(&a);
            let mut f = backend(a.threads);
            let alg = zreq.iter().any(Option::is_some).then(algebra);
            let mut lines = vec![];
            for (r, z) in reqs.iter().zip(&zreq) {
                match z {
                    None => lines.extend(
                        coef_facts::answer(std::slice::from_ref(r), &mut f, a.detail)
                            .unwrap_or_else(|e| die(&e)),
                    ),
                    Some((terms, per_key)) => lines.extend(
                        zeros_answer(alg.as_ref().expect("zeros"), r, terms, *per_key, &a)
                            .unwrap_or_else(|e| die(&e)),
                    ),
                }
            }
            write_atomic(&out, &lines);
        }
        "prove" => prove(&a),
        "verify-zero-proofs" => verify(&a),
        "crosscheck" => {
            let mut reqs = requests(&a);
            if a.answers.is_empty() {
                die("--answers missing");
            }
            let out = a.out.clone().unwrap_or_else(|| die("--out missing"));
            if let Some(m) = a.max_pairs {
                let mut left = m;
                for r in reqs.iter_mut() {
                    r.keys.truncate(left);
                    left -= r.keys.len();
                }
                reqs.retain(|r| !r.keys.is_empty());
            }
            // only the answers of requested keys are kept (answer files of a whole node do not fit in memory)
            let wanted: HashSet<Vec<i32>> =
                reqs.iter().flat_map(|r| r.keys.iter().cloned()).collect();
            let mut theirs: HashMap<(String, Vec<i32>), coef_facts::TheirAnswer> = HashMap::new();
            for p in &a.answers {
                let f = std::fs::File::open(p)
                    .unwrap_or_else(|e| die(&format!("{}: {e}", p.display())));
                for l in BufReader::new(f).lines() {
                    let l = l.unwrap_or_else(|e| die(&format!("{}: {e}", p.display())));
                    if l.trim().is_empty() {
                        continue;
                    }
                    let v: Value = serde_json::from_str(&l)
                        .unwrap_or_else(|e| die(&format!("{}: {e}", p.display())));
                    if v["schema"] != "rustred.proof-kernel.instantiate.v1" {
                        continue;
                    }
                    let (Some(sha), Ok(key)) = (
                        v["record_sha256"].as_str(),
                        serde_json::from_value::<Vec<i32>>(v["key"].clone()),
                    ) else {
                        continue;
                    };
                    if wanted.contains(&key) {
                        theirs.insert((sha.to_string(), key), coef_facts::their_answer(&v));
                    }
                }
            }
            let t0 = std::time::Instant::now();
            let mut f = backend(a.threads);
            let c = coef_facts::cross_check(&reqs, &mut f, &theirs).unwrap_or_else(|e| die(&e));
            let rep = json!({"schema": "rustred.campaign.coefficient-facts-crosscheck.v1",
                "producer": coef_facts::PRODUCER, "backend": coef_facts::BACKEND,
                "requests": a.requests, "answers": a.answers, "seconds": t0.elapsed().as_secs_f64(),
                "result": c});
            write_atomic(&out, &[rep]);
            println!(
                "{}",
                json!({"pairs": c.pairs, "triples": c.triples, "agree": c.agree, "disagree": c.disagree,
                       "pairs_without_answer": c.pairs_without_answer, "pairs_unusable": c.pairs_unusable})
            );
            if c.disagree > 0 {
                std::process::exit(3);
            }
        }
        x => die(&format!("unknown command {x:?}")),
    }
}
