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
//! ELIDED WITNESS LOCI (`--elided-loci refuse|facts`, default `refuse`). The engine writes a witness locus longer than
//! 4000 characters as the marker `"elided"` (driver.rs write_rule); only `witness_loci` (and `witness_d_poles`) are
//! ever elided, never an rhs coefficient, exception or case text. `refuse` (default, unchanged): the whole job fails
//! closed (`UNSUPPORTED`). `facts`: such a record is instantiated like any other; every fact that does not read the
//! witness loci (`in_case`, `chart_ok`, `branch`, per-term `coef`, `irregular`) is computed by exactly the same code
//! and is exactly as decided as for a complete record. What is LOST is the kernel's own `hidden` test for the elided
//! loci: `regular` is `irregular` (rhs denominator vanishes, decided), else `hidden` (a RECORDED locus vanishes,
//! decided), else `unknown` (never `regular`). Extra fields on such lines only: `witness_loci_elided` (number of
//! elided loci) and `elided_cover`: `guards_v2` when the table header says guard_version >= 2 and no exception branch
//! holds at the key (by the engine's guard construction, ecbe5daf, every index-containing factor of every witness
//! pole is an exception branch, and a polynomial in d vanishes identically iff one of its factors does, so no elided
//! locus vanishes at such a key; this is the ENGINE's invariant, recorded here, not re-derived by the kernel),
//! `excepted` when a branch holds, `none` otherwise. Records without elided loci give byte-identical lines in both
//! modes. A `regularity` fact request (B) on an elided record answers `unknown`.
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

/// How `instantiate` treats records whose witness loci are elided (see the module doc).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum ElidedLoci {
    /// fail closed: the whole job is refused (`UNSUPPORTED`), as before
    #[default]
    Refuse,
    /// instantiate the record; every loci-independent fact exact, `regular` never `regular` when a locus is elided
    Facts,
}

impl ElidedLoci {
    pub fn parse(s: &str) -> Option<Self> {
        match s { "refuse" => Some(Self::Refuse), "facts" => Some(Self::Facts), _ => None }
    }
}

fn is_elided_text(s: &[u8]) -> bool { find(s, b"...") || find(s, b"elided") }

struct Req {
    table: String,
    offset: Option<u64>,
    rule: i64,
    /// optional chart mask: rule indices restart per chart in multi-chart (eps) tables
    chart: Option<String>,
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
        let chart = s("chart");
        let ints = |x: &J| -> Vec<i64> { x.arr().iter().map(|v| v.int(l)).collect() };
        if j.get(l, "schema").map(|x| x.s(l)) == Some(b"rustred.campaign.fact-request.v1") {
            let rule = j.get(l, "label").unwrap_or_else(|| die("fact request without label")).int(l);
            let kind = s("kind").unwrap_or_else(|| die("fact request without kind"));
            let term = j.get(l, "term").map(|x| x.int(l));
            out.push(Req { table, offset, rule, chart, key: ints(j.get(l, "key").unwrap()), fact: Some((kind, term, l.to_vec())) });
        } else {
            let rule = j.get(l, "rule").unwrap_or_else(|| die("request without rule")).int(l);
            for k in j.get(l, "keys").unwrap_or_else(|| die("request without keys")).arr() {
                out.push(Req { table: table.clone(), offset, rule, chart: chart.clone(), key: ints(k), fact: None });
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

/// facts of one rule record at a list of keys (`guard_version`: the table header's, absent = 1)
fn facts_for_rule(line: &[u8], keys: &[Vec<i64>], elided_mode: ElidedLoci, guard_version: i64) -> Vec<String> {
    let j = jparse(line);
    let idx = j.get(line, "index").unwrap().int(line);
    let chart = j.get(line, "chart").unwrap().s(line).to_vec();
    let (na, fixed) = fixed_slots(&j, line);
    let eqs: Vec<Aff> = j.get(line, "equations").map(|x| x.arr().iter().map(|e| parse_affine(e.s(line)).unwrap_or_else(|er| die(&format!("case equation: {}", er)))).collect()).unwrap_or_default();
    let exc: Vec<Vec<&[u8]>> = j.get(line, "exceptions").map(|e| e.arr().iter().map(|b| b.arr().iter().map(|x| x.s(line)).collect()).collect()).unwrap_or_default();
    if exc.iter().any(|b| b.is_empty()) {
        // typed per-record refusal: never skipped silently, never evaluated (see the interface file, section 3)
        return keys.iter().map(|key| format!("{{\"schema\":\"rustred.proof-kernel.instantiate.v1\",\"rule\":{},\"key\":{:?},\"in_case\":null,\"chart_ok\":null,\"branch\":null,\"regular\":\"unknown\",\"rhs\":[],\"zero_test\":\"exact\",\"error\":\"EMPTY_EXCEPTION_BRANCH\"}}", idx, key)).collect();
    }
    let wl_all: Vec<&[u8]> = match j.get(line, "witness_loci") { Some(J::Arr(a)) => a.iter().map(|x| x.s(line)).collect(), _ => vec![] };
    let mut n_elided = 0i64;
    if let Some(x @ J::Num(..)) = j.get(line, "witness_loci_elided") {
        if elided_mode == ElidedLoci::Refuse { die("UNSUPPORTED: witness loci elided in the record (fail closed)"); }
        n_elided += x.int(line).max(1);
    }
    for s in &wl_all {
        if is_elided_text(s) {
            if elided_mode == ElidedLoci::Refuse { die("UNSUPPORTED: an elided witness locus in the record (fail closed)"); }
            n_elided += 1;
        }
    }
    // the recorded (complete) loci; elided markers are never evaluated
    let wl: Vec<&[u8]> = wl_all.iter().copied().filter(|s| !is_elided_text(s)).collect();
    if n_elided > 0 {
        // elision is defined for witness loci only: a marker anywhere the facts are read is a refusal in every mode
        let rhs_arr = j.get(line, "rhs").unwrap().arr();
        let marked = rhs_arr.iter().any(|x| x.arr().iter().any(|t| is_elided_text(t.s(line))))
            || exc.iter().flatten().any(|t| is_elided_text(t))
            || j.get(line, "target").map_or(false, |t| is_elided_text(t.s(line)));
        if marked { die("UNSUPPORTED: an elided rhs / exception / target text (fail closed)"); }
    }
    let bounds = domain_bounds(&j, line, na);
    let target = parse_pat(j.get(line, "target").unwrap().s(line));
    let rhs: Vec<(Pat, &[u8], Option<&[u8]>)> = j.get(line, "rhs").unwrap().arr().iter().map(|x| { let a = x.arr(); let (n, d) = split_coef(a[1].s(line)); (parse_pat(a[0].s(line)), n, d) }).collect();
    let mut maxe = 1u32;
    let texts = rhs.iter().flat_map(|(_, n, d)| [Some(*n), *d]).flatten().chain(exc.iter().flatten().copied()).chain(wl.iter().copied());
    for s in texts { let mut it = Terms::new(s); while let Some(t) = it.next() { for q in 0..t.nf { maxe = maxe.max(t.fe[q]); } } }
    let mut out = vec![];
    for key in keys {
        if key.len() != na { die(&format!("key arity {} != rule arity {}", key.len(), na)); }
        let kv: [i64; 20] = std::array::from_fn(|i| if i < na { key[i] } else { 0 });
        let in_case = (0..na).all(|i| fixed[i].map_or(true, |f| f == kv[i])) && eqs.iter().all(|e| e.lin.iter().fold(e.c0, |s, &(v, c)| s + c * kv[v as usize]) == 0)
            && in_bounds(&bounds, |i| kv[i]);
        let tv = pat_at(&target, &kv);
        let chart_ok = (0..na).all(|i| if chart[i] == b'1' { tv[i] >= 1 } else { tv[i] <= 0 });
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
        // indices into the record's `witness_loci` array; elided markers are never evaluated
        let hidden: Vec<usize> = wl_all.iter().enumerate().filter(|(_, s)| !is_elided_text(s) && eval_text_d(s, &kp).is_empty()).map(|(i, _)| i).collect();
        let regular = if !irregular.is_empty() { "irregular" } else if !hidden.is_empty() { "hidden" } else if n_elided > 0 { "unknown" } else { "regular" };
        let elided_fields = if n_elided > 0 {
            let cover = if branch.is_some() { "excepted" } else if guard_version >= 2 { "guards_v2" } else { "none" };
            format!(",\"witness_loci_elided\":{},\"elided_cover\":\"{}\"", n_elided, cover)
        } else { String::new() };
        out.push(format!("{{\"schema\":\"rustred.proof-kernel.instantiate.v1\",\"rule\":{},\"key\":{:?},\"in_case\":{},\"chart_ok\":{},\"branch\":{},\"regular\":\"{}\",\"irregular_terms\":{:?},\"hidden_loci\":{:?}{},\"rhs\":[{}],\"zero_test\":\"exact\",\"error\":null}}",
            idx, key, in_case, chart_ok, branch.map(|b| b.to_string()).unwrap_or("null".into()), regular, irregular, hidden, elided_fields, terms.join(",")));
    }
    out
}

pub fn run_instantiate(requests: &str, out_path: &str) -> String { run_instantiate_with(requests, out_path, ElidedLoci::Refuse) }

/// the table header's guard_version (absent or no header = 1)
fn header_guard_version(table: &str) -> i64 {
    let mut gv = 1;
    read_lines_matching(table, |l| {
        if record_kind(l, table, 1, TABLE_KINDS) == Some(b"header") {
            let j = jparse(l);
            if let Some(x @ J::Num(..)) = j.get(l, "guard_version") { gv = x.int(l); }
        }
        false
    });
    gv
}

pub fn run_instantiate_with(requests: &str, out_path: &str, elided_mode: ElidedLoci) -> String {
    let t0 = std::time::Instant::now();
    let reqs = parse_requests(requests);
    // group: table -> (rule index, chart or none) -> (offset, keys)
    type RuleId = (i64, Option<String>);
    let mut groups: BTreeMap<String, BTreeMap<RuleId, (Option<u64>, BTreeSet<Vec<i64>>)>> = BTreeMap::new();
    for r in &reqs {
        let g = groups.entry(r.table.clone()).or_default().entry((r.rule, r.chart.clone())).or_insert((r.offset, BTreeSet::new()));
        if g.0.is_none() { g.0 = r.offset; }
        g.1.insert(r.key.clone());
    }
    let mut lines_out: Vec<String> = vec![];
    let mut answers: BTreeMap<(String, RuleId, Vec<i64>), String> = BTreeMap::new(); // per (table, rule, key): the fact line
    let mut nrec = 0;
    let mut nrec_elided = 0;
    for (table, rules) in &groups {
        let gv = if elided_mode == ElidedLoci::Facts { header_guard_version(table) } else { 1 };
        let mut records: BTreeMap<RuleId, Vec<u8>> = BTreeMap::new();
        for (id, (off, _)) in rules { if let Some(o) = off { records.insert(id.clone(), read_at(table, *o)); } }
        let missing: BTreeSet<RuleId> = rules.keys().filter(|i| !records.contains_key(*i)).cloned().collect();
        if !missing.is_empty() {
            // a request without a chart must match ONE record: the whole table is scanned to see every chart
            let unchartered = missing.iter().any(|(_, c)| c.is_none());
            let mut ln = 0usize;
            read_lines_matching(table, |l| {
                ln += 1;
                if record_kind(l, table, ln, TABLE_KINDS) == Some(b"rule") {
                    let i = top_int(l, "index").unwrap_or_else(|| die(&format!("UNREADABLE_RECORD: {} line {}: rule record without an integer index", table, ln)));
                    let ch = top_field(l, "chart").map(|c| String::from_utf8_lossy(c).to_string());
                    if let Some(c) = &ch {
                        let id = (i, Some(c.clone()));
                        if missing.contains(&id) { records.insert(id, l.to_vec()); }
                    }
                    let id0 = (i, None);
                    if missing.contains(&id0) {
                        if let Some(prev) = records.get(&id0) {
                            let pc = top_field(prev, "chart").map(|c| String::from_utf8_lossy(c).to_string());
                            die(&format!("AMBIGUOUS_RULE_INDEX: {} rule {} exists in charts {:?} and {:?}: give \"chart\" (or \"offset\") in the request", table, i, pc, ch));
                        }
                        records.insert(id0, l.to_vec());
                    }
                }
                unchartered || records.len() < rules.len()
            });
        }
        for (id, (_, keys)) in rules {
            let idx = &id.0;
            let line = records.get(id).unwrap_or_else(|| die(&format!("rule {} not found in {}", idx, table)));
            if record_kind(line, table, 0, TABLE_KINDS) != Some(b"rule") { die("offset does not point at a rule record"); }
            let jj = jparse(&line[..line.len()]);
            if jj.get(line, "index").map(|x| x.int(line)) != Some(*idx) { die("record at offset has a different index"); }
            if let Some(c) = &id.1 {
                if top_field(line, "chart") != Some(c.as_bytes()) { die("record at offset has a different chart"); }
            }
            nrec += 1;
            let keys: Vec<Vec<i64>> = keys.iter().cloned().collect();
            let f = facts_for_rule(line, &keys, elided_mode, gv);
            if f.first().map_or(false, |l| l.contains("\"witness_loci_elided\":")) { nrec_elided += 1; }
            let rsha = record_sha256(line);
            for (k, l) in keys.iter().zip(f) {
                // echo the record digest right after the schema field
                let l = l.replacen("\"rustred.proof-kernel.instantiate.v1\",", &format!("\"rustred.proof-kernel.instantiate.v1\",\"record_sha256\":\"{}\",", rsha), 1);
                answers.insert((table.clone(), id.clone(), k.clone()), l.clone()); lines_out.push(l);
            }
        }
    }
    // closure fact answers for (B) requests
    for r in &reqs {
        if let Some((kind, term, raw)) = &r.fact {
            let fl = &answers[&(r.table.clone(), (r.rule, r.chart.clone()), r.key.clone())];
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
                if let Some(J::Str(..)) = fj.get(b, "error") { ("unknown".to_string(), format!("{{\"error\":{}}}", jstr(&String::from_utf8_lossy(fj.get(b, "error").unwrap().s(b))))) }
                else {
                    if t >= rhs.len() { die("term index out of range"); }
                    (String::from_utf8_lossy(rhs[t].get(b, "coef").unwrap().s(b)).to_string(), "{}".to_string())
                }
            };
            let rsha_v = fj.get(b, "record_sha256").map(|x| jstr(&String::from_utf8_lossy(x.s(b)))).unwrap_or("null".into());
            lines_out.push(format!("{{\"schema\":\"rustred.campaign.fact.v1\",\"record_sha256\":{},\"rule\":{},\"table\":{},\"record\":{},\"label\":{},\"key\":{:?},\"kind\":{},{}\"value\":{},\"method\":\"exact\",\"evidence\":{}}}",
                rsha_v, r.rule, echo("table"), echo("record"), echo("label"), r.key, jstr(kind), term.map(|t| format!("\"term\":{},", t)).unwrap_or_default(), jstr(&value), evidence));
        }
    }
    let part = format!("{}.part", out_path);
    {
        let mut f = std::fs::File::create(&part).unwrap_or_else(|e| die(&format!("create {}: {}", part, e)));
        for l in &lines_out { writeln!(f, "{}", l).unwrap(); }
        f.sync_all().ok();
    }
    std::fs::rename(&part, out_path).unwrap_or_else(|e| die(&format!("rename: {}", e)));
    let elided_summary = if elided_mode == ElidedLoci::Facts { format!(",\"elided_loci\":\"facts\",\"rules_with_elided_loci\":{}", nrec_elided) } else { String::new() };
    format!("{{\"schema\":\"rustred.proof-kernel.instantiate-summary.v1\",\"verdict\":\"DONE\",\"requests\":{},\"rules\":{}{},\"lines\":{},\"out\":{},\"seconds\":{:.3},\"peak_rss_mb\":{:.1}}}",
        reqs.len(), nrec, elided_summary, lines_out.len(), jstr(out_path), t0.elapsed().as_secs_f64(), peak_mb())
}

#[cfg(test)]
mod tests {
    use super::*;
    fn rule(exc: &str) -> Vec<u8> {
        format!(r#"{{"type":"rule","chart":"11000000000000000000","index":7,"fixed":[null,null,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0],"equations":[],"target":"I(n0,n1,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0)","rhs":[["I(n0-1,n1,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0)","(n0-2)/(d-2*n1)"],["I(n0,n1-1,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0)","(n1)/(n0)"]],"exceptions":{exc}}}"#).into_bytes()
    }
    #[test]
    fn exceptions_branches_and_empty_and_list() {
        // ordinary record: branch hit, zero coefficient, irregular term
        let l = rule(r#"[["-1+n0"]]"#);
        let f = facts_for_rule(&l, &[vec![1, 3, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0], vec![2, 3, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0]], ElidedLoci::Refuse, 2);
        assert!(f[0].contains("\"branch\":0") && f[0].contains("\"regular\":\"regular\""));
        assert!(f[1].contains("\"branch\":null") && f[1].contains("\"coef\":\"zero\"") && f[1].contains("\"error\":null"));
        // an EMPTY AND-list is never skipped: typed per-record error (the engine never writes one; replay refuses it)
        let l = rule(r#"[["-1+n0"],[]]"#);
        let f = facts_for_rule(&l, &[vec![2, 3, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0]], ElidedLoci::Refuse, 2);
        assert!(f[0].contains("\"error\":\"EMPTY_EXCEPTION_BRANCH\"") && f[0].contains("\"regular\":\"unknown\""));
    }

    fn rule_wl(wl: &str, rhs1: &str) -> Vec<u8> {
        format!(r#"{{"type":"rule","chart":"11000000000000000000","index":7,"fixed":[null,null,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0],"equations":[],"target":"I(n0,n1,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0)","rhs":[["I(n0-1,n1,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0)","(n0-2)/(d-2*n1)"],["I(n0,n1-1,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0)","{rhs1}"]],"exceptions":[["-1+n0"]],"witness_loci":{wl}}}"#).into_bytes()
    }
    fn k(n0: i64, n1: i64) -> Vec<i64> { let mut v = vec![0; 20]; v[0] = n0; v[1] = n1; v }
    /// the facts of a line with the elided-mode extras removed and `regular` blanked (for comparisons)
    fn strip(l: &str) -> String {
        let mut s = l.to_string();
        for (a, b) in [(",\"witness_loci_elided\":1,\"elided_cover\":\"guards_v2\"", ""), (",\"witness_loci_elided\":1,\"elided_cover\":\"excepted\"", ""),
                       ("\"regular\":\"unknown\"", "\"regular\":_"), ("\"regular\":\"regular\"", "\"regular\":_")] { s = s.replace(a, b); }
        s
    }

    #[test]
    fn elided_loci_refused_by_default_and_instantiated_in_facts_mode() {
        let keys = vec![k(1, 3), k(2, 3), k(3, 5), k(4, 1)];
        let el = rule_wl(r#"["elided","-3+n0"]"#, "(n1)/(n0)");
        let full = rule_wl(r#"["-5+n0","-3+n0"]"#, "(n1)/(n0)"); // the elided locus stands for -5+n0 (never 0 on these keys)
        // default: the whole record is refused (fail closed), as before
        let e = crate::guarded(|| facts_for_rule(&el, &keys, ElidedLoci::Refuse, 2)).unwrap_err();
        assert!(e.starts_with("UNSUPPORTED: an elided witness locus"), "{e}");
        let f = facts_for_rule(&el, &keys, ElidedLoci::Facts, 2);
        let g = facts_for_rule(&full, &keys, ElidedLoci::Refuse, 2);
        // n0 = 1: exception branch 0 holds -> cover "excepted"; regular unknown (never "regular")
        assert!(f[0].contains("\"branch\":0") && f[0].contains("\"regular\":\"unknown\"") && f[0].contains("\"elided_cover\":\"excepted\""), "{}", f[0]);
        // n0 = 2: zero coefficient (n0-2), regular unknown, covered by the v2 guards
        assert!(f[1].contains("\"coef\":\"zero\"") && f[1].contains("\"regular\":\"unknown\"") && f[1].contains("\"elided_cover\":\"guards_v2\"") && f[1].contains("\"witness_loci_elided\":1"), "{}", f[1]);
        // n0 = 3: the RECORDED locus -3+n0 vanishes -> hidden (decided), same as for the complete record
        assert!(f[2].contains("\"regular\":\"hidden\"") && f[2].contains("\"hidden_loci\":[1]") && g[2].contains("\"regular\":\"hidden\""), "{} / {}", f[2], g[2]);
        // every loci-independent fact equals the complete record's, key by key
        for (a, b) in f.iter().zip(&g) { assert_eq!(strip(a), strip(b)); }
        // regular is never claimed with an elided locus
        assert!(f.iter().all(|l| !l.contains("\"regular\":\"regular\"")));
        // guard_version 1 table: no cover claimed
        let f1 = facts_for_rule(&el, &keys, ElidedLoci::Facts, 1);
        assert!(f1[1].contains("\"elided_cover\":\"none\""), "{}", f1[1]);
        // the numeric form `witness_loci_elided`
        let mut num = rule_wl(r#"["-3+n0"]"#, "(n1)/(n0)");
        num.truncate(num.len() - 1);
        num.extend_from_slice(br#","witness_loci_elided":2}"#);
        assert!(crate::guarded(|| facts_for_rule(&num, &keys, ElidedLoci::Refuse, 2)).is_err());
        let fnum = facts_for_rule(&num, &keys, ElidedLoci::Facts, 2);
        assert!(fnum[1].contains("\"witness_loci_elided\":2") && fnum[1].contains("\"regular\":\"unknown\""), "{}", fnum[1]);
    }

    #[test]
    fn facts_mode_is_byte_identical_without_elided_loci_and_refuses_elided_rhs() {
        let keys = vec![k(1, 3), k(2, 3), k(3, 5), k(4, 1), k(5, 0)];
        for wl in [r#"[]"#, r#"["-3+n0"]"#, r#"["-4+n0+n1", "1+n1"]"#] {
            let r = rule_wl(wl, "(n1)/(n0)");
            assert_eq!(facts_for_rule(&r, &keys, ElidedLoci::Refuse, 2), facts_for_rule(&r, &keys, ElidedLoci::Facts, 2));
        }
        // an elision marker in a fact-carrying text is refused in every mode
        let bad = rule_wl(r#"["elided"]"#, "elided");
        let e = crate::guarded(|| facts_for_rule(&bad, &keys, ElidedLoci::Facts, 2)).unwrap_err();
        assert!(e.starts_with("UNSUPPORTED: an elided rhs"), "{e}");
    }

    /// engine-line7 reader fix: a rule record with its keys in another order (T211 rule 2894 starts `{"case"...`)
    /// is found by its parsed `type` and `index`; unknown kinds and unreadable lines are typed refusals.
    #[test]
    fn key_reordered_rule_record_is_found_and_bad_lines_are_refused() {
        let dir = std::env::temp_dir().join(format!("rr_proof_keyorder_{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let rule_sorted = |idx: i64| format!(r#"{{"case":"x","chart":"11000000000000000000","equations":[],"exceptions":[],"fixed":[null,null,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0],"index":{idx},"rhs":[["I(n0-1,n1,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0)","(n0-2)/(d-2*n1)"]],"target":"I(n0,n1,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0)","type":"rule"}}"#);
        let table = dir.join("t.jsonl");
        let body = format!("{{\"guard_version\":2,\"type\":\"header\"}}\n{}\n{}\n{{\"chart\":\"11000000000000000000\",\"key\":\"I(1,1,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0)\",\"type\":\"residual\"}}\n{{\"rules\":2,\"type\":\"end\"}}\n",
            String::from_utf8(rule(r#"[]"#)).unwrap(), rule_sorted(2894));
        std::fs::write(&table, &body).unwrap();
        let req = dir.join("req.jsonl");
        std::fs::write(&req, format!("{{\"table\":\"{}\",\"rule\":2894,\"keys\":[[3,4,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0]]}}\n", table.display())).unwrap();
        let out = dir.join("out.jsonl");
        let summary = crate::guarded(|| run_instantiate(req.to_str().unwrap(), out.to_str().unwrap())).unwrap();
        assert!(summary.contains("\"rules\":1"), "{summary}");
        let o = std::fs::read_to_string(&out).unwrap();
        assert!(o.contains("\"rule\":2894") && o.contains("\"in_case\":true"), "{o}");
        assert_eq!(header_guard_version(table.to_str().unwrap()), 2);
        // an unknown record kind and an unreadable line are refused, never skipped
        for (bad, want) in [("{\"type\":\"mystery\"}", "UNKNOWN_RECORD_KIND"), ("{\"type\":\"rule\",\"index\":", "UNREADABLE_RECORD"), ("{\"type\":\"residual\",\"chart\":\"1", "UNREADABLE_RECORD"),
                            ("{\"index\":5}", "UNREADABLE_RECORD"), ("{\"type\":7}", "UNREADABLE_RECORD"), ("not json", "UNREADABLE_RECORD")] {
            std::fs::write(&table, format!("{}{}\n", body, bad)).unwrap();
            std::fs::write(&req, format!("{{\"table\":\"{}\",\"rule\":99,\"keys\":[[3,4,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0]]}}\n", table.display())).unwrap();
            let e = crate::guarded(|| run_instantiate(req.to_str().unwrap(), out.to_str().unwrap())).unwrap_err();
            assert!(e.starts_with(want), "{bad}: {e}");
        }
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn top_member_scanner() {
        use crate::core::{top_field, top_int, top_members};
        let l = br#"{"a":{"type":"inner","x":[1,"]",{"y":"}"}]},"s":"q\"x","type":"rule","index":12}"#;
        assert_eq!(top_field(l, "type"), Some(&b"rule"[..]));
        assert_eq!(top_int(l, "index"), Some(12));
        assert_eq!(top_field(l, "x"), None); // nested members are not top-level
        assert!(top_members(br#"{"a":1} trailing"#, |_, _| true).is_err());
        assert!(top_members(br#"{"a":[1,2}"#, |_, _| true).is_err());
        assert!(top_members(br#"  {}  "#, |_, _| true).is_ok());
    }

    /// Multi-chart (eps) tables restart rule indices per chart: a request without a chart that matches two records is
    /// refused (AMBIGUOUS_RULE_INDEX); with "chart" each request gets its own record.
    #[test]
    fn rule_index_is_per_chart() {
        let dir = std::env::temp_dir().join(format!("rr_proof_charts_{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let mk = |chart: &str, rhs1: &str| format!(r#"{{"type":"rule","chart":"{chart}","index":0,"fixed":[null,null,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0],"equations":[],"target":"I(n0,n1,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0)","rhs":[["I(n0-1,n1,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0)","{rhs1}"]],"exceptions":[]}}"#);
        let table = dir.join("t.jsonl");
        std::fs::write(&table, format!("{{\"type\":\"header\"}}\n{}\n{}\n", mk("11000000000000000000", "n0-3"), mk("01000000000000000000", "n0-4"))).unwrap();
        let req = dir.join("r.jsonl");
        let out = dir.join("o.jsonl");
        let key = "[3,4,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0]";
        std::fs::write(&req, format!("{{\"table\":\"{}\",\"rule\":0,\"keys\":[{key}]}}\n", table.display())).unwrap();
        let e = crate::guarded(|| run_instantiate(req.to_str().unwrap(), out.to_str().unwrap())).unwrap_err();
        assert!(e.starts_with("AMBIGUOUS_RULE_INDEX"), "{e}");
        std::fs::write(&req, format!("{{\"table\":\"{t}\",\"rule\":0,\"chart\":\"11000000000000000000\",\"keys\":[{key}]}}\n{{\"table\":\"{t}\",\"rule\":0,\"chart\":\"01000000000000000000\",\"keys\":[{key}]}}\n", t = table.display())).unwrap();
        crate::guarded(|| run_instantiate(req.to_str().unwrap(), out.to_str().unwrap())).unwrap();
        let o = std::fs::read_to_string(&out).unwrap();
        // n0 = 3: chart 11.. has coefficient n0-3 = 0, chart 01.. has n0-4 != 0
        assert!(o.contains("\"coef\":\"zero\"") && o.contains("\"coef\":\"nonzero\""), "{o}");
        std::fs::remove_dir_all(&dir).ok();
    }
}
