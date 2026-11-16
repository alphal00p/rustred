//! The STREAMING fact pass (interfaces/import.md section 12, phase B; section 13.8): after a Tier K check (a superset
//! closure; [`super::check::check_streamed`] for large nodes), the compact fact verdict lines are read ONE AT A TIME
//! (bounded memory: a few bits per closure key, never a fact store) and every served (rule, key) of the closure is
//! checked against its own line:
//! - the line binds by (record_sha256, key) to the key's WINNING record (else it is counted and unused);
//! - `regular` -> the key's regularity is established (a witness-absent record is lifted at exactly that key);
//!   `irregular` -> the key fails (pole);
//! - every term of the winner at the key is classified (core `Sectors`): a positive-numerator term or a top term that is
//!   not lower than the key must be ABSENT from the line's non-zero digests (else the closure FAILS there: the term
//!   would need a vanishing coefficient); a lower top term listed non-zero is an EDGE (its key is marked reached); a term
//!   not listed has a coefficient identically zero in d at the key and is NOT an edge.
//!
//! After the pass: served keys without a regular line, failures, and the keys of the Tier K closure that no non-zero
//! edge reaches and that are not start keys (one level: they lie outside the Tier C closure unless reached through a
//! key that is itself only Tier-K-reached; reported, never silently dropped).
//!
//! Sound summary: if every served key has its regular line, no failure occurs and every open key of the Tier K superset
//! is unreachable by non-zero edges, the Tier C closure (a subset) is served with exact facts.

use std::io::BufRead;
use std::path::Path;

use serde::Serialize;
use serde_json::Value;

use super::check::{CheckResult, RowStatus, Server};
use super::error::{ImportError, io_err};
use super::evidence::FACT_VERDICT_SCHEMA;
use super::lines::{parse_key_text, parse_named_key_text, parse_term_key_digest, term_key_digest};
use super::load::ImportedSet;
use crate::closure::{KeySet, Sectors, TermClass};

/// What the pass established.
#[derive(Clone, Debug, Default, Serialize)]
pub struct FactPassReport {
    pub keys: u64,
    pub served_by_rule: u64,
    pub lines: u64,
    pub lines_foreign: u64,
    pub lines_outside_closure: u64,
    pub lines_not_winner: u64,
    pub lines_unusable: u64,
    pub regular: u64,
    pub irregular: u64,
    /// Served (or witness-blocked) keys whose winning record has no `regular` line.
    pub served_without_regularity: u64,
    /// Keys the Tier K check reported blocked by a witness-absent / elided record that a `regular` line lifts here.
    pub blocked_lifted: u64,
    /// (key, term index, term key, why) of terms that must vanish but are listed non-zero.
    pub failures: Vec<(Vec<i32>, usize, Vec<i32>, String)>,
    pub failures_total: u64,
    pub edges_nonzero: u64,
    pub terms_zero: u64,
    /// Non-start keys of the closure that no non-zero edge reaches (first examples) and their count.
    pub unreached: Vec<Vec<i32>>,
    pub unreached_total: u64,
    /// Open keys of the closure among the unreached (a Tier K defect that the facts remove).
    pub open_unreached: u64,
    pub open_total: u64,
}

/// How many example keys / failures are kept.
pub const EXAMPLES: usize = 64;

fn set_bit(v: &mut [u64], i: usize) {
    v[i / 64] |= 1 << (i % 64);
}
fn bit(v: &[u64], i: usize) -> bool {
    v[i / 64] >> (i % 64) & 1 == 1
}

/// Run the pass over `fact_files` (compact `rustred.campaign.fact-verdict.v1` lines) for the closure of `res`
/// (`res.key_set` after `check_streamed`, else the keys of `res.rows`).
pub fn fact_pass(
    set: &ImportedSet,
    res: &CheckResult,
    fact_files: &[&Path],
) -> Result<FactPassReport, ImportError> {
    let n = set.arity();
    let keys: KeySet = match &res.key_set {
        Some(k) => k.clone(),
        None => {
            let mut b = crate::closure::KeySetBuilder::new(set.codec);
            for r in &res.rows {
                b.push(&r.key)
                    .map_err(|e| ImportError::Core(format!("{e:?}")))?;
            }
            b.finish()
        }
    };
    let len = keys.len();
    let words = len.div_ceil(64);
    // per closure key: winner record (global index + 1; 0 = not served by a rule), status bits
    let offsets: Vec<usize> = set
        .rules
        .iter()
        .scan(0usize, |acc, t| {
            let o = *acc;
            *acc += t.len();
            Some(o)
        })
        .collect();
    let mut winner = vec![0u32; len];
    let mut open = vec![0u64; words];
    let mut blocked = vec![0u64; words];
    let mut start = vec![0u64; words];
    let mut visit_row = |key: &[i32], status: &str, in_start: bool, sha: Option<&str>| {
        if let Some(i) = keys.index_of(key) {
            if in_start {
                set_bit(&mut start, i);
            }
            if status == "open" {
                set_bit(&mut open, i);
            }
            if status == "blocked" {
                set_bit(&mut blocked, i);
            }
            if let Some(s) = sha
                && let Some((t, r)) = set.find_sha(s)
            {
                winner[i] = (offsets[t] + r + 1) as u32;
            }
        }
    };
    match &res.streamed {
        Some(st) => {
            let f = std::fs::File::open(&st.path).map_err(|e| io_err(Path::new(&st.path), e))?;
            for (no, l) in std::io::BufReader::with_capacity(1 << 20, f)
                .lines()
                .enumerate()
            {
                let l = l.map_err(|e| io_err(Path::new(&st.path), e))?;
                let v: Value = serde_json::from_str(&l).map_err(|e| ImportError::Evidence {
                    path: st.path.clone(),
                    line: no + 1,
                    msg: e.to_string(),
                })?;
                let key: Vec<i32> = serde_json::from_value(v["key"].clone()).unwrap_or_default();
                // the winner: the serving rule, or the record that blocks an open key (witness absent / elided:
                // exactly what a regular line lifts at that key)
                let sha = if v["server"]["kind"] == "rule" {
                    v["server"]["record_sha256"].as_str()
                } else if v["open"]["status"] == "blocked" {
                    v["open"]["rule"]["record_sha256"].as_str()
                } else {
                    None
                };
                let status = if v["open"]["status"] == "blocked" && sha.is_some() {
                    "blocked"
                } else {
                    v["status"].as_str().unwrap_or("")
                };
                visit_row(&key, status, v["in_start"] == true, sha);
            }
        }
        None => {
            for r in &res.rows {
                let blocked = r.open_status.as_deref() == Some("blocked") && r.open_rule.is_some();
                let st = match r.status {
                    RowStatus::Open if blocked => "blocked",
                    RowStatus::Open => "open",
                    RowStatus::Served => "served",
                    RowStatus::Terminal => "terminal",
                };
                let sha = match (r.server.as_ref(), r.open_rule) {
                    (Some(Server::Rule { table, record }), _) => {
                        Some(set.rules[*table][*record].record_sha256.as_str())
                    }
                    (None, Some((t, k))) if blocked => Some(set.rules[t][k].record_sha256.as_str()),
                    _ => None,
                };
                visit_row(&r.key, st, r.in_start, sha);
            }
        }
    }
    let locate = |g: u32| -> (usize, usize) {
        let g = (g - 1) as usize;
        let t = offsets.partition_point(|&o| o <= g) - 1;
        (t, g - offsets[t])
    };
    let sectors = Sectors::new(set.frame.mask().to_vec());
    let mut rep = FactPassReport {
        keys: len as u64,
        served_by_rule: winner.iter().filter(|&&w| w != 0).count() as u64,
        open_total: open.iter().map(|w| w.count_ones() as u64).sum(),
        ..Default::default()
    };
    let mut regular = vec![0u64; words];
    let mut reached = vec![0u64; words];
    let (mut ib, mut cm) = (vec![], vec![]);
    for path in fact_files {
        let f = std::fs::File::open(path).map_err(|e| io_err(path, e))?;
        for (no, l) in std::io::BufReader::with_capacity(1 << 20, f)
            .lines()
            .enumerate()
        {
            let l = l.map_err(|e| io_err(path, e))?;
            if l.trim().is_empty() {
                continue;
            }
            let bad = |m: String| ImportError::Evidence {
                path: path.display().to_string(),
                line: no + 1,
                msg: m,
            };
            let v: Value = serde_json::from_str(&l).map_err(|e| bad(e.to_string()))?;
            if v.get("schema").and_then(Value::as_str) != Some(FACT_VERDICT_SCHEMA) {
                continue;
            }
            rep.lines += 1;
            let Some((t, r)) = v
                .get("record_sha256")
                .and_then(Value::as_str)
                .and_then(|s| set.find_sha(s))
            else {
                rep.lines_foreign += 1;
                continue;
            };
            let mut key = match v.get("key") {
                Some(Value::String(s)) if s.contains('=') => parse_named_key_text(s, n),
                Some(Value::String(s)) => parse_key_text(s, n),
                Some(k) => serde_json::from_value::<Vec<i32>>(k.clone()).ok(),
                None => None,
            }
            .filter(|k| k.len() == n)
            .ok_or_else(|| bad("key".into()))?;
            if key.contains(&i32::MIN) {
                let fixed = &set.disp.tables()[t].domains[r].fixed;
                for (i, x) in key.iter_mut().enumerate() {
                    if *x == i32::MIN {
                        *x = fixed[i]
                            .ok_or_else(|| bad(format!("key misses the free slot n{i}")))?;
                    }
                }
            }
            let Some(i) = keys.index_of(&key) else {
                rep.lines_outside_closure += 1;
                continue;
            };
            if winner[i] == 0 || locate(winner[i]) != (t, r) {
                rep.lines_not_winner += 1;
                continue;
            }
            match v.get("verdict").and_then(Value::as_str) {
                Some("regular") => {
                    if !bit(&regular, i) {
                        rep.regular += 1;
                    }
                    set_bit(&mut regular, i);
                }
                Some("irregular") => {
                    rep.irregular += 1;
                    continue;
                }
                Some("hidden") => {}
                _ => {
                    rep.lines_unusable += 1;
                    continue;
                }
            }
            let mut nonzero: Vec<u64> = v
                .get("nonzero")
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
                .map(|x| {
                    x.as_str()
                        .and_then(parse_term_key_digest)
                        .ok_or_else(|| bad(format!("nonzero digest {x}")))
                })
                .collect::<Result<_, _>>()?;
            nonzero.sort_unstable();
            ib.clear();
            cm.clear();
            let rr = set.disp.rule_ref(t, r);
            let nt = set
                .bodies
                .instantiate(&set.disp, rr, &key, &mut ib, &mut cm)
                .map_err(|e| ImportError::Core(format!("{e:?}")))?
                .unwrap_or(0);
            for j in 0..nt {
                let tk = &ib[j * n..(j + 1) * n];
                let listed = nonzero.binary_search(&term_key_digest(tk)).is_ok();
                if !listed {
                    rep.terms_zero += 1;
                    continue;
                }
                let why = match sectors.classify_masked(tk, cm[j]) {
                    TermClass::PositiveNumerator { .. } => Some("positive numerator, non-zero"),
                    TermClass::Top if !set.cmp.lower(tk, &key) => {
                        Some("top term not lower, non-zero")
                    }
                    TermClass::Top => {
                        rep.edges_nonzero += 1;
                        if let Some(s) = keys.index_of(tk) {
                            set_bit(&mut reached, s);
                        }
                        None
                    }
                    _ => None,
                };
                if let Some(w) = why {
                    rep.failures_total += 1;
                    if rep.failures.len() < EXAMPLES {
                        rep.failures
                            .push((key.clone(), j, tk.to_vec(), w.to_string()));
                    }
                }
            }
        }
    }
    for (i, &w) in winner.iter().enumerate() {
        if w != 0 && !bit(&regular, i) {
            rep.served_without_regularity += 1;
        }
        if bit(&blocked, i) && bit(&regular, i) {
            rep.blocked_lifted += 1;
        }
        if !bit(&start, i) && !bit(&reached, i) {
            rep.unreached_total += 1;
            if bit(&open, i) {
                rep.open_unreached += 1;
            }
            if rep.unreached.len() < EXAMPLES {
                rep.unreached.push(keys.get(i));
            }
        }
    }
    Ok(rep)
}
