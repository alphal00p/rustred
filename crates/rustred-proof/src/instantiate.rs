//! Verb `instantiate` (campaign engine contract EC-2): exact per-key facts of stored rules, no proof.
//!
//! For every requested (rule, key) the rule record is instantiated at the integer key and the following facts are
//! decided EXACTLY (integer arithmetic, d symbolic: every coefficient is evaluated at the key as a polynomial in d
//! with integer coefficients):
//! * `in_case`: fixed slots and case equations hold at the key; `chart_ok`: the target at the key satisfies the chart
//!   guards (lines >= 1, other slots <= 0);
//! * `branch`: index of the first exception branch whose texts all vanish identically at the key (null if none);
//! * `regular`: `irregular` if some rhs denominator vanishes identically in d at the key; else `hidden` if some
//!   recorded witness locus (`witness_loci`, hidden weight poles) vanishes identically at the key (the stored
//!   derivation is silent there: prove the key with instance-span); else `regular`;
//! * `rhs`: per rhs term its key at the key and `coef` = `zero` / `nonzero` (numerator identically zero in d or not);
//!   `unknown` only when the term's denominator vanishes identically.
//! No denominator is factored: every text is EVALUATED at the key (exact), so huge denominators cost one pass.
//!
//! Input (`--requests FILE`, JSONL; both forms may be mixed):
//!  (A) `{"table": PATH, "offset": BYTES (optional), "rule": INDEX, "keys": [[k0..k19], ...]}`
//!  (B) closure fact requests `{"schema":"rustred.campaign.fact-request.v1","table": PATH,"record":r,"label":INDEX,
//!      "offset": BYTES (optional),"key":[..],"kind":"coefficient"|"regularity","term":j,...}`
//!  The rule is the record whose `index` equals `rule` (A) / `label` (B); with `offset` the record is read there and
//!  its index is verified, else the table is scanned ONCE for all requested indices.
//! Output (`--out FILE`, written as FILE.part then renamed): one line per (rule, key) in schema
//! `rustred.proof-kernel.instantiate.v1`, and for (B) requests additionally one `rustred.campaign.fact.v1` answer
//! line per request (method "exact").
#![allow(clippy::all)]
use std::collections::{BTreeMap, BTreeSet};
use std::io::{BufRead, BufReader, Seek, SeekFrom, Write};

use crate::core::*;
use crate::span::{eval_text_d, pat_at};

struct Req {
    table: String,
    offset: Option<u64>,
    rule: i64,
    key: Vec<i64>,
    fact: Option<(String, Option<i64>, Vec<u8>)>, // (kind, term, raw request line for the echo fields)
}

fn parse_requests(path: &str) -> Vec<Req> {
    let mut out = vec![];
    read_lines_matching(path, |l| {
        if l.iter().all(|c| c.is_ascii_whitespace()) { return true; }
        let j = jparse(l);
        let s = |k: &str| j.get(l, k).map(|x| String::from_utf8_lossy(x.s(l)).to_string());
        let table = s("table").unwrap_or_else(|| die("request without table"));
        let offset = j.get(l, "offset").map(|x| x.int(l) as u64);
        let ints = |x: &J| -> Vec<i64> { x.arr().iter().map(|v| v.int(l)).collect() };
        if j.get(l, "schema").map(|x| x.s(l)) == Some(b"rustred.campaign.fact-request.v1") {
            let rule = j.get(l, "label").unwrap_or_else(|| die("fact request without label")).int(l);
            let kind = s("kind").unwrap_or_else(|| die("fact request without kind"));
            let term = j.get(l, "term").map(|x| x.int(l));
            out.push(Req { table, offset, rule, key: ints(j.get(l, "key").unwrap()), fact: Some((kind, term, l.to_vec())) });
        } else {
            let rule = j.get(l, "rule").unwrap_or_else(|| die("request without rule")).int(l);
            for k in j.get(l, "keys").unwrap_or_else(|| die("request without keys")).arr() {
                out.push(Req { table: table.clone(), offset, rule, key: ints(k), fact: None });
            }
        }
        true
    });
    out
}

fn read_at(path: &str, off: u64) -> Vec<u8> {
    let mut f = std::fs::File::open(path).unwrap_or_else(|e| die(&format!("open {}: {}", path, e)));
    f.seek(SeekFrom::Start(off)).unwrap();
    let mut r = BufReader::with_capacity(1 << 22, f);
    let mut line = vec![];
    r.read_until(b'\n', &mut line).unwrap();
    while line.last() == Some(&b'\n') { line.pop(); }
    line
}

/// facts of one rule record at a list of keys
fn facts_for_rule(line: &[u8], keys: &[Vec<i64>]) -> Vec<String> {
    let j = jparse(line);
    let idx = j.get(line, "index").unwrap().int(line);
    let chart = j.get(line, "chart").unwrap().s(line).to_vec();
    let mut fixed: [Option<i64>; 20] = [None; 20];
    for (i, x) in j.get(line, "fixed").unwrap().arr().iter().enumerate() { if let J::Num(..) = x { fixed[i] = Some(x.int(line)); } }
    let eqs: Vec<Aff> = j.get(line, "equations").map(|x| x.arr().iter().map(|e| parse_affine(e.s(line)).unwrap_or_else(|er| die(&format!("case equation: {}", er)))).collect()).unwrap_or_default();
    let exc: Vec<Vec<&[u8]>> = j.get(line, "exceptions").map(|e| e.arr().iter().map(|b| b.arr().iter().map(|x| x.s(line)).collect()).collect()).unwrap_or_default();
    let wl: Vec<&[u8]> = match j.get(line, "witness_loci") { Some(J::Arr(a)) => a.iter().map(|x| x.s(line)).collect(), _ => vec![] };
    if let Some(J::Num(..)) = j.get(line, "witness_loci_elided") { die("UNSUPPORTED: witness loci elided in the record (fail closed)"); }
    for s in &wl { if find(s, b"...") || find(s, b"elided") { die("UNSUPPORTED: an elided witness locus in the record (fail closed)"); } }
    let target = parse_pat(j.get(line, "target").unwrap().s(line));
    let rhs: Vec<(Pat, &[u8], Option<&[u8]>)> = j.get(line, "rhs").unwrap().arr().iter().map(|x| { let a = x.arr(); let (n, d) = split_coef(a[1].s(line)); (parse_pat(a[0].s(line)), n, d) }).collect();
    let mut maxe = 1u32;
    let texts = rhs.iter().flat_map(|(_, n, d)| [Some(*n), *d]).flatten().chain(exc.iter().flatten().copied()).chain(wl.iter().copied());
    for s in texts { let mut it = Terms::new(s); while let Some(t) = it.next() { for q in 0..t.nf { maxe = maxe.max(t.fe[q]); } } }
    let mut out = vec![];
    for key in keys {
        if key.len() != 20 { die("key arity != 20"); }
        let kv: [i64; 20] = std::array::from_fn(|i| key[i]);
        let in_case = (0..20).all(|i| fixed[i].map_or(true, |f| f == kv[i])) && eqs.iter().all(|e| e.lin.iter().fold(e.c0, |s, &(v, c)| s + c * kv[v as usize]) == 0);
        let tv = pat_at(&target, &kv);
        let chart_ok = (0..20).all(|i| if chart[i] == b'1' { tv[i] >= 1 } else { tv[i] <= 0 });
        let kp: Vec<Vec<Big>> = (0..20).map(|v| { let mut t = vec![Big::one()]; for e in 1..=maxe as usize { let y = t[e - 1].mul_i64(kv[v]); t.push(y); } t }).collect();
        let branch = exc.iter().position(|b| !b.is_empty() && b.iter().all(|s| eval_text_d(s, &kp).is_empty()));
        let mut irregular = vec![];
        let mut terms = vec![];
        for (ti, (p, n, d)) in rhs.iter().enumerate() {
            let den_zero = d.map_or(false, |s| eval_text_d(s, &kp).is_empty());
            if den_zero { irregular.push(ti); }
            let coef = if den_zero { "unknown" } else if eval_text_d(n, &kp).is_empty() { "zero" } else { "nonzero" };
            terms.push(format!("{{\"term\":{},\"key\":{:?},\"coef\":\"{}\"}}", ti, pat_at(p, &kv), coef));
        }
        let hidden: Vec<usize> = wl.iter().enumerate().filter(|(_, s)| eval_text_d(s, &kp).is_empty()).map(|(i, _)| i).collect();
        let regular = if !irregular.is_empty() { "irregular" } else if !hidden.is_empty() { "hidden" } else { "regular" };
        out.push(format!("{{\"schema\":\"rustred.proof-kernel.instantiate.v1\",\"rule\":{},\"key\":{:?},\"in_case\":{},\"chart_ok\":{},\"branch\":{},\"regular\":\"{}\",\"irregular_terms\":{:?},\"hidden_loci\":{:?},\"rhs\":[{}],\"zero_test\":\"exact\",\"error\":null}}",
            idx, key, in_case, chart_ok, branch.map(|b| b.to_string()).unwrap_or("null".into()), regular, irregular, hidden, terms.join(",")));
    }
    out
}

pub fn run_instantiate(requests: &str, out_path: &str) -> String {
    let t0 = std::time::Instant::now();
    let reqs = parse_requests(requests);
    // group: table -> rule index -> (offset, keys)
    let mut groups: BTreeMap<String, BTreeMap<i64, (Option<u64>, BTreeSet<Vec<i64>>)>> = BTreeMap::new();
    for r in &reqs {
        let g = groups.entry(r.table.clone()).or_default().entry(r.rule).or_insert((r.offset, BTreeSet::new()));
        if g.0.is_none() { g.0 = r.offset; }
        g.1.insert(r.key.clone());
    }
    let mut lines_out: Vec<String> = vec![];
    let mut answers: BTreeMap<(String, i64, Vec<i64>), String> = BTreeMap::new(); // per (table, rule, key): the fact line
    let mut nrec = 0;
    for (table, rules) in &groups {
        let mut records: BTreeMap<i64, Vec<u8>> = BTreeMap::new();
        for (idx, (off, _)) in rules { if let Some(o) = off { records.insert(*idx, read_at(table, *o)); } }
        let missing: BTreeSet<i64> = rules.keys().filter(|i| !records.contains_key(i)).copied().collect();
        if !missing.is_empty() {
            read_lines_matching(table, |l| {
                if l.starts_with(b"{\"type\":\"rule\"") {
                    let head = &l[..l.len().min(400)];
                    if let Some(p) = head.windows(8).position(|w| w == b"\"index\":") {
                        let digits: Vec<u8> = head[p + 8..].iter().take_while(|c| c.is_ascii_digit()).copied().collect();
                        let i: i64 = std::str::from_utf8(&digits).unwrap().parse().unwrap_or(-1);
                        if missing.contains(&i) { records.insert(i, l.to_vec()); }
                    }
                }
                records.len() < rules.len()
            });
        }
        for (idx, (_, keys)) in rules {
            let line = records.get(idx).unwrap_or_else(|| die(&format!("rule {} not found in {}", idx, table)));
            if !line.starts_with(b"{\"type\":\"rule\"") { die("offset does not point at a rule record"); }
            let jj = jparse(&line[..line.len()]);
            if jj.get(line, "index").map(|x| x.int(line)) != Some(*idx) { die("record at offset has a different index"); }
            nrec += 1;
            let keys: Vec<Vec<i64>> = keys.iter().cloned().collect();
            let f = facts_for_rule(line, &keys);
            for (k, l) in keys.iter().zip(f) { answers.insert((table.clone(), *idx, k.clone()), l.clone()); lines_out.push(l); }
        }
    }
    // closure fact answers for (B) requests
    for r in &reqs {
        if let Some((kind, term, raw)) = &r.fact {
            let fl = &answers[&(r.table.clone(), r.rule, r.key.clone())];
            let b = fl.as_bytes();
            let fj = jparse(b);
            let rj = jparse(raw);
            let echo = |k: &str| rj.get(raw, k).map(|x| match x { J::Str(..) => jstr(&String::from_utf8_lossy(x.s(raw))), _ => x.int(raw).to_string() }).unwrap_or("null".into());
            let (value, evidence) = if kind == "regularity" {
                let reg = String::from_utf8_lossy(fj.get(b, "regular").unwrap().s(b)).to_string();
                (match reg.as_str() { "regular" => "regular", "irregular" => "pole", _ => "unknown" }.to_string(), format!("{{\"regular\":{}}}", jstr(&reg)))
            } else {
                let t = term.unwrap_or_else(|| die("coefficient request without term")) as usize;
                let rhs = fj.get(b, "rhs").unwrap().arr();
                if t >= rhs.len() { die("term index out of range"); }
                (String::from_utf8_lossy(rhs[t].get(b, "coef").unwrap().s(b)).to_string(), "{}".to_string())
            };
            lines_out.push(format!("{{\"schema\":\"rustred.campaign.fact.v1\",\"table\":{},\"record\":{},\"label\":{},\"key\":{:?},\"kind\":{},{}\"value\":{},\"method\":\"exact\",\"evidence\":{}}}",
                echo("table"), echo("record"), echo("label"), r.key, jstr(kind), term.map(|t| format!("\"term\":{},", t)).unwrap_or_default(), jstr(&value), evidence));
        }
    }
    let part = format!("{}.part", out_path);
    {
        let mut f = std::fs::File::create(&part).unwrap_or_else(|e| die(&format!("create {}: {}", part, e)));
        for l in &lines_out { writeln!(f, "{}", l).unwrap(); }
        f.sync_all().ok();
    }
    std::fs::rename(&part, out_path).unwrap_or_else(|e| die(&format!("rename: {}", e)));
    format!("{{\"schema\":\"rustred.proof-kernel.instantiate-summary.v1\",\"verdict\":\"DONE\",\"requests\":{},\"rules\":{},\"lines\":{},\"out\":{},\"seconds\":{:.3},\"peak_rss_mb\":{:.1}}}",
        reqs.len(), nrec, lines_out.len(), jstr(out_path), t0.elapsed().as_secs_f64(), peak_mb())
}
