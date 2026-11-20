//! The d = 4 REGULARITY MEASURE of a replacement (emission contract section 13). SINCE THE USER RULING OF 2026-11-19
//! (LANE_RULES rule 21) A REPORTED NUMBER, TIE-BREAK ONLY: it never refuses, drops or deprioritises a replacement. (The
//! rule of the same morning, "a relation regular at d = 4 wins whatever its size", is withdrawn; section 13-OLD.)
//!
//! Keys: the keys the OLD record serves as FIRST MATCH in the cone D + r <= R, found by the campaign core's dispatcher
//! (`cover::Dispatcher`, the engine's find_rule order) over the OLD record's table: every key of the old record's face in
//! the cone is enumerated and dispatched; those whose first match is the old record are its first-match set. When the
//! face has more keys in the cone than `max_keys`, a STATED SAMPLE is taken instead: every shell D + r of the cone and
//! every new piece (case branch) that serves keys is represented, the rule recorded in the verdict. Each served key is
//! assigned to the FIRST new piece whose domain applies (the replacement's own dispatch order); the old relation and
//! that piece's relation are measured at the key (`emission::regularity`, exact).
//!
//! Verdict: NOT WORSE on every compared key, or worse at some keys: REPORTED (counts, per branch, first regressions) in
//! the side file every piece names; the replacement is accepted either way (under schema v1 a worse key refused it).
use rustred_campaign_core::cover::{Dispatcher, Table};
use rustred_campaign_core::domain::DomainOptions;
use rustred_campaign_core::emission::body::{body_ids, strip_derivation};
use rustred_campaign_core::emission::regularity::{
    self, KeyComparison, KeySet, Relation, Verdict, keys_sha256, regularity_at,
};
use rustred_campaign_core::record::{ReadOptions, read_table_file};
use serde_json::{Value, json};
use std::collections::BTreeMap;
use std::io::{BufRead, Write};
use std::path::{Path, PathBuf};

/// Options of one comparison.
#[derive(Clone, Debug)]
pub struct Options {
    /// the cone D + r <= cone
    pub cone: i64,
    /// at most this many face keys are dispatched; more = a stated sample
    pub max_keys: usize,
    /// keys measured in parallel (the measure is per key: the verdict does not depend on it)
    pub threads: usize,
}

impl Default for Options {
    fn default() -> Self {
        Options {
            cone: 10,
            max_keys: 20_000,
            threads: 1,
        }
    }
}

pub fn dispatcher_of(table: &Path) -> Result<(Dispatcher, String), String> {
    let tf = read_table_file(table, ReadOptions::default())
        .map_err(|e| format!("{}: {e}", table.display()))?;
    let chart = tf
        .rules
        .first()
        .and_then(|r| r.chart.clone())
        .ok_or("table without rule records")?;
    // witness loci off: the engine writes the witness poles' factors into `exceptions` (domain.rs: a redundancy check),
    // and tables written by older harnesses carry no `witness_loci` at all (every key would be blocked)
    let opts = DomainOptions {
        witness: false,
        ..DomainOptions::default()
    };
    let t = Table::from_table_file("t", table.display().to_string(), &tf, opts)
        .map_err(|e| format!("{}: {e}", table.display()))?;
    let mask: Vec<bool> = chart.bytes().map(|b| b == b'1').collect();
    let d = Dispatcher::new(vec![t], vec![0])
        .map_err(|e| e.to_string())?
        .with_chart(mask)
        .map_err(|e| e.to_string())?;
    Ok((d, chart))
}

/// Compositions of `t` into `n` non-negative parts, in lexicographic order; `f` returns false to stop.
fn comps(n: usize, t: i64, cur: &mut Vec<i64>, f: &mut dyn FnMut(&[i64]) -> bool) -> bool {
    if n == 0 {
        return t != 0 || f(cur);
    }
    if cur.len() + 1 == n {
        cur.push(t);
        let c = f(cur);
        cur.pop();
        return c;
    }
    for v in 0..=t {
        cur.push(v);
        let c = comps(n, t - v, cur, f);
        cur.pop();
        if !c {
            return false;
        }
    }
    true
}

/// Every key of the face of `fixed` (None = free) on `chart` with D + r <= cone, in shell order (stars and bars over
/// the free slots); stops after `cap` keys (returns whether it stopped).
pub fn face_keys(
    fixed: &[Option<i64>],
    chart: &str,
    cone: i64,
    cap: usize,
) -> (Vec<Vec<i64>>, bool) {
    let mask: Vec<bool> = chart.bytes().map(|b| b == b'1').collect();
    let free: Vec<usize> = (0..fixed.len()).filter(|&i| fixed[i].is_none()).collect();
    let base: i64 = fixed
        .iter()
        .zip(&mask)
        .map(|(f, &m)| match f {
            Some(v) if m => v - 1,
            Some(v) => -v,
            None => 0,
        })
        .sum();
    let mut out: Vec<Vec<i64>> = vec![];
    let mut stopped = false;
    for t in 0..=(cone - base) {
        let mut emit = |p: &[i64]| -> bool {
            let mut k: Vec<i64> = fixed.iter().map(|f| f.unwrap_or(0)).collect();
            for (j, &s) in free.iter().enumerate() {
                k[s] = if mask[s] { 1 + p[j] } else { -p[j] };
            }
            out.push(k);
            out.len() < cap
        };
        if !comps(free.len(), t, &mut vec![], &mut emit) {
            stopped = true;
            break;
        }
    }
    (out, stopped)
}

/// The header line of a table (its first `header` record).
pub fn header_of(table: &Path) -> Result<String, String> {
    let f = std::fs::File::open(table).map_err(|e| format!("{}: {e}", table.display()))?;
    for l in std::io::BufReader::new(f).lines() {
        let l = l.map_err(|e| e.to_string())?;
        if rustred_campaign_core::emission::body::string_member(&l, "type").as_deref()
            == Some("header")
        {
            return Ok(l);
        }
    }
    Err(format!("{}: no header", table.display()))
}

pub fn read_line_at(table: &Path, index: i64, offset: Option<u64>) -> Result<String, String> {
    use std::io::{Seek, SeekFrom};
    let mut f = std::fs::File::open(table).map_err(|e| format!("{}: {e}", table.display()))?;
    if let Some(o) = offset {
        f.seek(SeekFrom::Start(o)).map_err(|e| e.to_string())?;
        let mut l = String::new();
        std::io::BufReader::new(f)
            .read_line(&mut l)
            .map_err(|e| e.to_string())?;
        return Ok(l.trim_end_matches('\n').to_string());
    }
    for l in std::io::BufReader::new(f).lines() {
        let l = l.map_err(|e| e.to_string())?;
        if rustred_campaign_core::emission::body::string_member(&l, "type").as_deref()
            == Some("rule")
            && serde_json::from_str::<Value>(&l)
                .ok()
                .and_then(|v| v["index"].as_i64())
                == Some(index)
        {
            return Ok(l);
        }
    }
    Err(format!("rule {index} not in {}", table.display()))
}

/// The first-match keys of the old record (label `index`) in the cone, from the dispatcher over `table`, or a stated
/// sample of them when the face is larger than `max_keys`.
pub fn served_keys(
    table: &Path,
    index: i64,
    old_line: &str,
    o: &Options,
) -> Result<(Vec<Vec<i64>>, KeySet, String), String> {
    let (disp, chart) = dispatcher_of(table)?;
    let (k, s) = served_keys_with(&disp, &chart, index, old_line, o)?;
    Ok((k, s, chart))
}

/// [`served_keys`] over a dispatcher already built for the old table (batch use).
pub fn served_keys_with(
    disp: &Dispatcher,
    chart: &str,
    index: i64,
    old_line: &str,
    o: &Options,
) -> Result<(Vec<Vec<i64>>, KeySet), String> {
    let v: Value = serde_json::from_str(old_line).map_err(|e| format!("old record: {e}"))?;
    let fixed: Vec<Option<i64>> = v["fixed"]
        .as_array()
        .ok_or("old record: no fixed")?
        .iter()
        .map(Value::as_i64)
        .collect();
    let (face, stopped) = face_keys(&fixed, chart, o.cone, o.max_keys);
    let mut served = vec![];
    let debug = std::env::var("RR_REGCHECK_DEBUG").is_ok();
    for (n, k) in face.iter().enumerate() {
        let k32: Vec<i32> = k.iter().map(|&x| x as i32).collect();
        let d = disp.first_match(&k32);
        if debug && n < 5 {
            eprintln!("regcheck: face key {k:?}: {d:?}");
        }
        if let Ok(d) = d
            && d.winner().is_some_and(|w| w.label == index)
        {
            served.push(k.clone());
        }
    }
    if debug {
        eprintln!(
            "regcheck: face {} keys (stopped {stopped}), served {}",
            face.len(),
            served.len()
        );
    }
    let ks = KeySet {
        kind: if stopped { "sample".into() } else { "first-match set".into() },
        cone: format!("D + r <= {}", o.cone),
        served: served.len() as u64,
        compared: served.len() as u64,
        sampling: stopped.then(|| {
            format!(
                "the first {} keys of the old record's face in shell order (shells D + r ascending); first-match keys among them",
                o.max_keys
            )
        }),
        keys_sha256: keys_sha256(&served),
    };
    Ok((served, ks))
}

/// Compare the old record with the replacement pieces (in their order) at the given keys.
#[allow(clippy::too_many_arguments)]
pub fn compare(
    old_line: &str,
    pieces: &[String],
    keys: &[Vec<i64>],
    mut ks: KeySet,
    header: &str,
    chart: &str,
    work: &Path,
    threads: usize,
) -> Result<Verdict, String> {
    let (old_body, _) = strip_derivation(old_line)?;
    let old = Relation::parse(&old_body)?;
    let mut new = vec![];
    let mut bodies = vec![];
    for p in pieces {
        let (b, _) = strip_derivation(p)?;
        bodies.push(body_ids(&b)?.body_sha256);
        new.push(Relation::parse(&b)?);
    }
    // the replacement's own dispatch: a mini table of the pieces
    std::fs::create_dir_all(work).map_err(|e| e.to_string())?;
    let mini = work.join("pieces.dispatch.table.jsonl");
    let mut f = std::fs::File::create(&mini).map_err(|e| e.to_string())?;
    writeln!(f, "{header}").map_err(|e| e.to_string())?;
    for (i, p) in pieces.iter().enumerate() {
        let (b, _) = strip_derivation(p)?;
        let mut v: Value = serde_json::from_str(&b).map_err(|e| e.to_string())?;
        v["index"] = json!(i);
        v["chart"] = json!(chart);
        v.as_object_mut().map(|m| m.remove("rhs"));
        v["rhs"] = json!([]);
        writeln!(f, "{v}").map_err(|e| e.to_string())?;
    }
    drop(f);
    let (pd, _) = dispatcher_of(&mini)?;
    // the serving piece of every key (the replacement's own dispatch), then the measure per key (in parallel)
    let mut gaps = 0u64;
    let mut jobs: Vec<(Vec<i64>, usize)> = vec![];
    for k in keys {
        let k32: Vec<i32> = k.iter().map(|&x| x as i32).collect();
        match pd.first_match(&k32).ok().and_then(|d| d.winner()) {
            Some(w) => jobs.push((k.clone(), w.label as usize)),
            None => gaps += 1,
        }
    }
    let threads = threads.max(1).min(jobs.len().max(1));
    let chunk = jobs.len().div_ceil(threads).max(1);
    let results: Vec<Result<Vec<KeyComparison>, String>> = std::thread::scope(|s| {
        let hs: Vec<_> = jobs
            .chunks(chunk)
            .map(|part| {
                let (old, new) = (&old, &new);
                s.spawn(move || {
                    part.iter()
                        .map(|(k, i)| {
                            Ok(KeyComparison {
                                key: k.clone(),
                                branch: format!("piece {i}"),
                                old: regularity_at(old, k)?,
                                new: regularity_at(&new[*i], k)?,
                            })
                        })
                        .collect::<Result<Vec<_>, String>>()
                })
            })
            .collect();
        hs.into_iter()
            .map(|h| {
                h.join()
                    .unwrap_or_else(|_| Err("a measuring thread panicked".into()))
            })
            .collect()
    });
    let mut cmp = vec![];
    for r in results {
        cmp.extend(r?);
    }
    let mut structural = vec![];
    if gaps > 0 {
        structural.push(format!(
            "{gaps} served key(s) are served by NO piece (the replacement leaves them open)"
        ));
    }
    ks.compared = cmp.len() as u64;
    let mut v = regularity::verdict(
        &body_ids(&old_body)?.body_sha256,
        &bodies,
        ks,
        &cmp,
        structural,
    );
    // information: the producer's own d = 4 pole statements (pure d factors of `witness_d_poles`)
    match regularity::d4_factors(&old_body) {
        Some(f) if !f.is_empty() => v
            .d4_factors_info
            .push(format!("old record states d = 4 pole factors {f:?}")),
        Some(_) => v
            .d4_factors_info
            .push("old record states no pure d = 4 pole factor".into()),
        None => v
            .d4_factors_info
            .push("old record carries no witness_d_poles".into()),
    }
    for (i, p) in pieces.iter().enumerate() {
        let (b, _) = strip_derivation(p)?;
        if let Some(f) = regularity::d4_factors(&b)
            && !f.is_empty()
        {
            v.d4_factors_info
                .push(format!("piece {i} states d = 4 pole factors {f:?} (a pole at d = 4 wherever their terms survive)"));
        }
    }
    Ok(v)
}

/// Batch: one old table, many replacements. `spec` = JSON lines {"label", "rule", "pieces": FILE}; writes
/// out/<label>/{regularity.json,keys.jsonl} and out/summary.jsonl (one line per item). The table is read once.
pub fn batch(table: &Path, spec: &Path, o: &Options, out: &Path) -> Result<(Value, bool), String> {
    let (disp, chart) = dispatcher_of(table)?;
    let header = header_of(table)?;
    // the old rule lines by index (one scan)
    let items: Vec<Value> = std::fs::read_to_string(spec)
        .map_err(|e| format!("{}: {e}", spec.display()))?
        .lines()
        .filter(|l| !l.trim().is_empty())
        .map(|l| serde_json::from_str(l).map_err(|e| format!("batch spec: {e}")))
        .collect::<Result<_, _>>()?;
    let want: std::collections::BTreeSet<i64> =
        items.iter().filter_map(|x| x["rule"].as_i64()).collect();
    let mut old: BTreeMap<i64, String> = BTreeMap::new();
    let f = std::fs::File::open(table).map_err(|e| format!("{}: {e}", table.display()))?;
    for l in std::io::BufReader::new(f).lines() {
        let l = l.map_err(|e| e.to_string())?;
        if rustred_campaign_core::emission::body::string_member(&l, "type").as_deref()
            != Some("rule")
        {
            continue;
        }
        let ix = l
            .find("\"index\"")
            .and_then(|i| l[i + 7..].trim_start().strip_prefix(':'))
            .and_then(|r| r.trim_start().split(|c: char| !c.is_ascii_digit()).next())
            .and_then(|d| d.parse::<i64>().ok());
        if let Some(ix) = ix
            && want.contains(&ix)
        {
            old.insert(ix, l);
        }
    }
    std::fs::create_dir_all(out).map_err(|e| e.to_string())?;
    let mut summary =
        std::fs::File::create(out.join("summary.jsonl")).map_err(|e| e.to_string())?;
    let mut all_ok = true;
    let mut rows = vec![];
    for it in &items {
        let label = it["label"].as_str().unwrap_or("?").to_string();
        let rule = it["rule"].as_i64().ok_or("batch item without rule")?;
        let pf = PathBuf::from(it["pieces"].as_str().ok_or("batch item without pieces")?);
        let t0 = std::time::Instant::now();
        let dir = out.join(&label);
        std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
        let old_line = old
            .get(&rule)
            .ok_or_else(|| format!("rule {rule} not in the old table"))?;
        let lines: Vec<String> = std::fs::read_to_string(&pf)
            .map_err(|e| format!("{}: {e}", pf.display()))?
            .lines()
            .filter(|l| {
                rustred_campaign_core::emission::body::string_member(l, "type").as_deref()
                    == Some("rule")
            })
            .map(str::to_string)
            .collect();
        let (keys, ks) = served_keys_with(&disp, &chart, rule, old_line, o)?;
        let v = compare(
            old_line,
            &lines,
            &keys,
            ks,
            &header,
            &chart,
            &dir.join("work"),
            o.threads,
        )?;
        let accepted = v.not_worse && v.keys.compared > 0;
        all_ok &= accepted;
        let mut kf = String::new();
        for k in &keys {
            kf.push_str(&serde_json::to_string(k).unwrap_or_default());
            kf.push('\n');
        }
        std::fs::write(dir.join("keys.jsonl"), kf).map_err(|e| e.to_string())?;
        let mut j = serde_json::to_value(&v).map_err(|e| e.to_string())?;
        j["seconds"] = json!(t0.elapsed().as_secs_f64());
        j["old"] = json!({"table": table.display().to_string(), "rule": rule});
        j["pieces"] = json!(pf.display().to_string());
        j["accepted"] = json!(accepted);
        std::fs::write(
            dir.join("regularity.json"),
            serde_json::to_string_pretty(&j).map_err(|e| e.to_string())?,
        )
        .map_err(|e| e.to_string())?;
        let row = json!({"label": label, "rule": rule, "pieces": lines.len(), "accepted": accepted, "keys": v.keys,
                         "counts": v.counts, "by_branch": v.by_branch, "first_regression": v.first_regressions.first(),
                         "structural": v.structural, "seconds": t0.elapsed().as_secs_f64()});
        writeln!(summary, "{row}").map_err(|e| e.to_string())?;
        summary.flush().map_err(|e| e.to_string())?;
        rows.push(row);
    }
    Ok((json!({"items": rows.len(), "all_accepted": all_ok}), all_ok))
}

/// CLI: `regularity-compare --old-table T --rule N [--offset B] --pieces P.jsonl [--cone R] [--max-keys K] --out DIR`:
/// the old record N of T against the replacement pieces (one rule line each, in order). Writes DIR/regularity.json and
/// DIR/keys.jsonl. Exit 0 NOT WORSE, 3 worse on a served key.
pub fn cmd(rest: &[String]) -> Result<(String, bool), String> {
    let (mut table, mut pieces, mut out) = (None::<PathBuf>, None::<PathBuf>, None::<PathBuf>);
    let mut rule: Option<i64> = None;
    let mut offset: Option<u64> = None;
    let mut o = Options::default();
    let mut batch_spec: Option<PathBuf> = None;
    let mut i = 0;
    let val = |i: usize| {
        rest.get(i + 1)
            .cloned()
            .ok_or(format!("{} needs a value", rest[i]))
    };
    while i < rest.len() {
        match rest[i].as_str() {
            "--old-table" => table = Some(val(i)?.into()),
            "--rule" => rule = Some(val(i)?.parse().map_err(|e| format!("--rule: {e}"))?),
            "--offset" => offset = Some(val(i)?.parse().map_err(|e| format!("--offset: {e}"))?),
            "--pieces" => pieces = Some(val(i)?.into()),
            "--cone" => o.cone = val(i)?.parse().map_err(|e| format!("--cone: {e}"))?,
            "--max-keys" => o.max_keys = val(i)?.parse().map_err(|e| format!("--max-keys: {e}"))?,
            "--threads" => o.threads = val(i)?.parse().map_err(|e| format!("--threads: {e}"))?,
            "--out" => out = Some(val(i)?.into()),
            "--batch" => batch_spec = Some(PathBuf::from(val(i)?)),
            x => return Err(format!("unknown flag {x}")),
        }
        i += 2;
    }
    if let (Some(t), Some(b), Some(out)) = (&table, &batch_spec, &out) {
        let (s, ok) = batch(t, b, &o, out)?;
        return Ok((s.to_string(), ok));
    }
    let (Some(table), Some(rule), Some(pieces), Some(out)) = (table, rule, pieces, out) else {
        return Err("regularity-compare --old-table T --rule N [--offset B] --pieces P.jsonl [--cone R] [--max-keys K] --out DIR".into());
    };
    std::fs::create_dir_all(&out).map_err(|e| e.to_string())?;
    let old_line = read_line_at(&table, rule, offset)?;
    let lines: Vec<String> = std::fs::read_to_string(&pieces)
        .map_err(|e| format!("{}: {e}", pieces.display()))?
        .lines()
        .filter(|l| {
            rustred_campaign_core::emission::body::string_member(l, "type").as_deref()
                == Some("rule")
        })
        .map(str::to_string)
        .collect();
    let t0 = std::time::Instant::now();
    let (keys, ks, chart) = served_keys(&table, rule, &old_line, &o)?;
    let header = header_of(&table)?;
    let v = compare(
        &old_line,
        &lines,
        &keys,
        ks,
        &header,
        &chart,
        &out.join("work"),
        o.threads,
    )?;
    let mut kf = String::new();
    for k in &keys {
        kf.push_str(&serde_json::to_string(k).unwrap_or_default());
        kf.push('\n');
    }
    std::fs::write(out.join("keys.jsonl"), kf).map_err(|e| e.to_string())?;
    let mut j = serde_json::to_value(&v).map_err(|e| e.to_string())?;
    j["seconds"] = json!(t0.elapsed().as_secs_f64());
    j["old"] = json!({"table": table.display().to_string(), "rule": rule});
    j["pieces"] = json!(pieces.display().to_string());
    std::fs::write(
        out.join("regularity.json"),
        serde_json::to_string_pretty(&j).map_err(|e| e.to_string())?,
    )
    .map_err(|e| e.to_string())?;
    Ok((
        json!({"not_worse": v.not_worse, "keys": v.keys, "counts": v.counts, "first_regressions": v.first_regressions,
               "structural": v.structural, "seconds": t0.elapsed().as_secs_f64()})
        .to_string(),
        v.not_worse,
    ))
}

/// The OLD table of a replacement, loaded once (dispatcher over its domains, chart, header): reused for every
/// replacement of a run.
pub struct OldTable {
    pub path: PathBuf,
    pub disp: Dispatcher,
    pub chart: String,
    pub header: String,
}

impl OldTable {
    pub fn load(path: &Path) -> Result<OldTable, String> {
        let (disp, chart) = dispatcher_of(path)?;
        Ok(OldTable {
            path: path.to_path_buf(),
            disp,
            chart,
            header: header_of(path)?,
        })
    }
    /// The rule lines of the given indices (one scan of the table).
    pub fn lines_of(
        &self,
        want: &std::collections::BTreeSet<i64>,
    ) -> Result<BTreeMap<i64, String>, String> {
        let mut old = BTreeMap::new();
        let f =
            std::fs::File::open(&self.path).map_err(|e| format!("{}: {e}", self.path.display()))?;
        for l in std::io::BufReader::new(f).lines() {
            let l = l.map_err(|e| e.to_string())?;
            if rustred_campaign_core::emission::body::string_member(&l, "type").as_deref()
                != Some("rule")
            {
                continue;
            }
            let ix = l
                .find("\"index\"")
                .and_then(|i| l[i + 7..].trim_start().strip_prefix(':'))
                .and_then(|r| r.trim_start().split(|c: char| !c.is_ascii_digit()).next())
                .and_then(|d| d.parse::<i64>().ok());
            if let Some(ix) = ix
                && want.contains(&ix)
            {
                old.insert(ix, l);
            }
        }
        Ok(old)
    }
}

/// The regularity ACCEPTANCE of a replacement (what emit-replace and the ladder decide with).
#[derive(Clone, Debug)]
pub struct Acceptance {
    pub accepted: bool,
    pub reason: Option<String>,
    pub verdict: Option<Verdict>,
}

/// Decide whether the replacement `pieces` of the old record `index` of `table` may be accepted: the d = 4 regularity
/// of the old record and of the serving piece, compared at every key the old record serves as first match in the cone
/// (or the stated sample); accepted iff NOT WORSE on every compared key. Not switchable for a live replacement.
pub fn acceptance(
    table: &OldTable,
    index: i64,
    old_line: &str,
    pieces: &[String],
    o: &Options,
    work: &Path,
) -> Result<Acceptance, String> {
    let (keys, ks) = served_keys_with(&table.disp, &table.chart, index, old_line, o)?;
    let v = compare(
        old_line,
        pieces,
        &keys,
        ks,
        &table.header,
        &table.chart,
        work,
        o.threads,
    )?;
    // USER RULING 2026-11-19 (LANE_RULES rule 21): the d = 4 comparison is a REPORTED NUMBER, tie-break only. The replacement
    // is ACCEPTED whatever it says (validity: derivation, descent, partition, is decided elsewhere); `reason` carries the report
    let reason = Some(if v.keys.compared == 0 {
        format!(
            "regularity_d4_not_compared (the old record serves no key in {}; reported only)",
            v.keys.cone
        )
    } else if v.not_worse {
        format!(
            "regularity_d4_reported: not worse at {} compared keys ({} better)",
            v.counts.keys, v.counts.better
        )
    } else {
        format!(
            "regularity_d4_reported: {} of {} served keys worse at d = 4 ({} regular -> singular{}); reported only, not a refusal",
            v.counts.worse,
            v.counts.keys,
            v.counts.regular_to_singular,
            if v.structural.is_empty() {
                String::new()
            } else {
                format!("; {}", v.structural.join("; "))
            }
        )
    });
    Ok(Acceptance {
        accepted: true,
        reason,
        verdict: Some(v),
    })
}

/// What [`accept_and_stamp`] decided.
#[derive(Clone, Debug)]
pub struct Stamped {
    pub accepted: bool,
    pub reason: Option<String>,
    /// the verdict (side file `regularity/<sha256>.json`)
    pub verdict: Value,
    pub sha256: String,
    pub same_relation: bool,
    /// the piece lines with `derivation.replaces` = {old_body_sha256, regularity_sha256} (only when accepted)
    pub lines: Option<Vec<String>>,
}

/// THE REPORTING STEP of a closed replacement (emit-replace, the ladder, the improve writer): the d = 4 comparison of the
/// old record (`index` of `table`) with the emitted piece lines (an identical relation: same relation, nothing compared),
/// the verdict stored beside the table (`emitter`'s side directory), and every piece's derivation stamped with `replaces`.
/// Since the user ruling of 2026-11-19 (LANE_RULES rule 21) the replacement is ALWAYS accepted here: the verdict is a
/// reported number, a tie-break only.
pub fn accept_and_stamp(
    emitter: &super::emitter::Emitter,
    table: &OldTable,
    index: i64,
    old_line: &str,
    lines: &[String],
    o: &Options,
    work: &Path,
) -> Result<Stamped, String> {
    use rustred_campaign_core::emission::body::attach_derivation;
    let (old_body, _) = strip_derivation(old_line)?;
    let old_ids = body_ids(&old_body)?;
    let new_ids: Vec<_> = lines
        .iter()
        .map(|l| body_ids(l))
        .collect::<Result<Vec<_>, _>>()?;
    let same = new_ids.len() == 1 && new_ids[0].relation_sha256 == old_ids.relation_sha256;
    let (verdict, ok, reason) = if same {
        (
            json!({"schema": regularity::SCHEMA, "tool": regularity::TOOL, "role": regularity::ROLE, "same_relation": true, "not_worse": true,
                   "old_body_sha256": old_ids.body_sha256, "new_body_sha256": [new_ids[0].body_sha256],
                   "note": "identical relation (target + rhs): identical coefficients at every key, nothing to compare"}),
            true,
            None,
        )
    } else {
        let a = acceptance(table, index, old_line, lines, o, work)?;
        let mut v = serde_json::to_value(&a.verdict).map_err(|e| e.to_string())?;
        v["old_table"] = json!(table.path.display().to_string());
        (v, a.accepted, a.reason)
    };
    let sha = emitter.put_side(
        rustred_campaign_core::emission::AREA_REGULARITY,
        &serde_json::to_vec(&verdict).map_err(|e| e.to_string())?,
    )?;
    let lines = if ok {
        let mut out = vec![];
        for l in lines {
            let (b, d) = strip_derivation(l)?;
            let mut dv: Value = serde_json::from_str(&d.ok_or("a piece without derivation")?)
                .map_err(|e| e.to_string())?;
            dv["replaces"] =
                json!({"old_body_sha256": old_ids.body_sha256, "regularity_sha256": sha});
            out.push(attach_derivation(&b, &dv)?);
        }
        Some(out)
    } else {
        None
    };
    Ok(Stamped {
        accepted: ok,
        reason,
        verdict,
        sha256: sha,
        same_relation: same,
        lines,
    })
}
