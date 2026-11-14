//! lane closeloop4 (interfaces/closeloop.md 18): the STANDING SECOND ROUTE of the exact tier's zero test.
//!
//! A term of a rule at a key is dropped as an edge only when its coefficient is identically zero in d at the key (a
//! coefficient FACT). Every zero that CHANGES THE RESULT must be confirmed by two independent routes: rr_proof
//! instantiate (`--elided-loci facts` for records with elided loci) and our exact evaluation (`rr_coef_facts`,
//! Symbolica). This module defines that set exactly and compares the two routes term by term.
//!
//! DECISIVE ZEROS (definition). Let C be the exact (Tier C) closure: the least key set that contains the start set and,
//! for every rule-served key u in C (a table rule at u, or a symmetry credit through the image's rule), the key of
//! every TOP-sector term of the rule instantiated at u whose coefficient is not a confirmed zero. A zero-coefficient
//! fact (u, term j, term key t) with u a rule-served key of C is DECISIVE iff t is a top-sector key and
//!   (reach)   t is not in C: treating the term as non-zero adds t (and what it reaches) to the closure, or
//!   (descent) t is not lower than u under the node comparator: treating it as non-zero is a descent violation.
//! Every other zero is NOT decisive: adding its edge back leaves C and every verdict unchanged, because C is closed
//! under all edges from C into C (an edge u -> t with t in C adds nothing to the least closed set) and edges from
//! keys outside C are never expanded (proved by `tests::adding_back_non_decisive_zero_edges_leaves_the_closure`; the
//! fake test `closeloop4_fake` shows a decisive one changes it). Zero terms of positive-numerator, zero-sector and
//! lower-sector (hand-down) class are counted, not confirmed (hand-downs: covered by the lower nodes' own closures;
//! positive numerators: the per-key acceptance of the rule; see 18.6). "Transitively": the zero edges among the keys
//! of K \ C (K the keys-only closure) matter only if a decisive zero were wrong; a refuted decisive zero blocks the node
//! (the key stays in the closure), so the next pass recomputes the set on the larger closure.
//!
//! The ROUTES: the first route is the producer of the fact the check used (`Evidence::fact_basis`); the second is the
//! other one (rr_proof -> exact_eval; exact_eval -> rr_proof with `--elided-loci facts`; an unnamed compact producer
//! -> exact_eval). The second route's answers are kept in their OWN evidence object (never merged with the first: a
//! disagreement is reported, not a load error) and compared per (record, key) over EVERY term of the record at the key.

use rustred_campaign_core::closure::{CoefFact, FactSource, RuleId};
use rustred_campaign_core::import::{CheckResult, Evidence, ImportedSet, RowStatus, Server};
use serde_json::{Value, json};
use std::collections::{BTreeMap, BTreeSet, HashSet};
use std::io::BufRead;
use std::path::Path;

pub const DECISIVE_SCHEMA: &str = "rustred.campaign.closeloop-decisive-zero.v1";

/// One decisive zero (see the module documentation).
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct DecisiveZero {
    /// The closure key whose rule drops the term.
    pub key: Vec<i32>,
    /// The key the rule is instantiated at (the image for a symmetry credit; else `key`).
    pub applied: Vec<i32>,
    pub table: usize,
    pub record: usize,
    pub term: usize,
    pub term_key: Vec<i32>,
    /// `reach` | `descent`.
    pub kind: &'static str,
}

/// The scan of a decided exact-tier check.
#[derive(Clone, Debug, Default)]
pub struct ZeroScan {
    pub decisive: Vec<DecisiveZero>,
    pub keys_scanned: u64,
    pub zero_top: u64,
    pub zero_positive_numerator: u64,
    pub zero_other: u64,
    pub seconds: f64,
}

fn is_top(lines: &[bool], k: &[i32]) -> bool {
    k.iter()
        .zip(lines)
        .all(|(&x, &l)| if l { x >= 1 } else { x <= 0 })
}

fn positive_numerator(lines: &[bool], k: &[i32]) -> bool {
    k.iter().zip(lines).any(|(&x, &l)| !l && x > 0)
}

/// The rule id the evidence answers for (table name + label).
pub fn rule_id(set: &ImportedSet, table: usize, record: usize) -> RuleId<'_> {
    let i = &set.rules[table][record];
    RuleId {
        table: &i.table_name,
        record,
        label: i.label,
        digest: "",
    }
}

/// The instantiated term keys of (table, record) at `applied` (None: the record has no body / no rule point).
pub fn term_keys(
    set: &ImportedSet,
    table: usize,
    record: usize,
    applied: &[i32],
) -> Option<Vec<Vec<i32>>> {
    let (mut buf, mut cm) = (vec![], vec![]);
    let nt = set
        .bodies
        .instantiate(&set.disp, set.disp.rule_ref(table, record), applied, &mut buf, &mut cm)
        .ok()??;
    let n = set.arity();
    Some((0..nt).map(|j| buf[j * n..(j + 1) * n].to_vec()).collect())
}

/// The rule-served rows of a decided Tier C check: (key, applied, table, record). Table-rule rows come from the
/// streamed rows file (every row; `server.kind == "rule"`), credits from the kept rows (their image).
pub fn served_rule_rows(
    set: &ImportedSet,
    res: &CheckResult,
) -> Result<Vec<(Vec<i32>, Vec<i32>, usize, usize)>, String> {
    let mut out = vec![];
    match &res.streamed {
        Some(st) => {
            let f = std::fs::File::open(&st.path).map_err(|e| format!("{}: {e}", st.path))?;
            for (i, l) in std::io::BufReader::with_capacity(1 << 20, f)
                .lines()
                .enumerate()
            {
                let l = l.map_err(|e| format!("{}: {e}", st.path))?;
                if l.trim().is_empty() {
                    continue;
                }
                let v: Value =
                    serde_json::from_str(&l).map_err(|e| format!("{} line {}: {e}", st.path, i + 1))?;
                if v["status"] != "served" || v["server"]["kind"] != "rule" {
                    continue;
                }
                let (Some(sha), Ok(key)) = (
                    v["server"]["record_sha256"].as_str(),
                    serde_json::from_value::<Vec<i32>>(v["key"].clone()),
                ) else {
                    return Err(format!("{} line {}: key / server record", st.path, i + 1));
                };
                let (t, r) = set
                    .find_sha(sha)
                    .ok_or_else(|| format!("{} line {}: record {sha} not in the set", st.path, i + 1))?;
                out.push((key.clone(), key, t, r));
            }
        }
        None => {
            for r in &res.rows {
                if let (RowStatus::Served, Some(Server::Rule { table, record })) = (r.status, &r.server) {
                    out.push((r.key.clone(), r.key.clone(), *table, *record));
                }
            }
        }
    }
    for r in &res.rows {
        if let (RowStatus::Served, Some(Server::Credit { table, record }), Some(img)) =
            (r.status, &r.server, &r.image)
        {
            out.push((r.key.clone(), img.clone(), *table, *record));
        }
    }
    Ok(out)
}

/// Membership in the closure of a check (the packed key set of a streamed check, else the rows).
pub fn closure_membership(res: &CheckResult) -> Box<dyn Fn(&[i32]) -> bool + Sync + '_> {
    match &res.key_set {
        Some(ks) => Box::new(move |k: &[i32]| ks.contains(k)),
        None => {
            let s: HashSet<Vec<i32>> = res.rows.iter().map(|r| r.key.clone()).collect();
            Box::new(move |k: &[i32]| s.contains(k))
        }
    }
}

/// The decisive zeros of a decided Tier C check under `facts` (the evidence the check used), on `threads` threads.
pub fn scan(
    set: &ImportedSet,
    facts: &(dyn FactSource + Sync),
    res: &CheckResult,
    threads: usize,
) -> Result<ZeroScan, String> {
    let t0 = std::time::Instant::now();
    let rows = served_rule_rows(set, res)?;
    let member = closure_membership(res);
    let lines = set.frame.mask().to_vec();
    let chunk = rows.len().div_ceil(threads.max(1)).max(1);
    let parts: Vec<ZeroScan> = std::thread::scope(|sc| {
        let hs: Vec<_> = rows
            .chunks(chunk)
            .map(|part| {
                let member = &member;
                let lines = &lines;
                sc.spawn(move || {
                    let mut z = ZeroScan::default();
                    for (key, applied, t, r) in part {
                        z.keys_scanned += 1;
                        let Some(terms) = term_keys(set, *t, *r, applied) else {
                            continue;
                        };
                        let rid = rule_id(set, *t, *r);
                        for (j, tk) in terms.iter().enumerate() {
                            if facts.coefficient(&rid, applied, j, tk) != CoefFact::Zero {
                                continue;
                            }
                            if is_top(lines, tk) {
                                z.zero_top += 1;
                                let kind = if !member(tk) {
                                    Some("reach")
                                } else if !set.cmp.lower(tk, key) {
                                    Some("descent")
                                } else {
                                    None
                                };
                                if let Some(kind) = kind {
                                    z.decisive.push(DecisiveZero {
                                        key: key.clone(),
                                        applied: applied.clone(),
                                        table: *t,
                                        record: *r,
                                        term: j,
                                        term_key: tk.clone(),
                                        kind,
                                    });
                                }
                            } else if positive_numerator(lines, tk) {
                                z.zero_positive_numerator += 1;
                            } else {
                                z.zero_other += 1;
                            }
                        }
                    }
                    z
                })
            })
            .collect();
        hs.into_iter().map(|h| h.join().unwrap_or_default()).collect()
    });
    let mut z = ZeroScan::default();
    for p in parts {
        z.decisive.extend(p.decisive);
        z.keys_scanned += p.keys_scanned;
        z.zero_top += p.zero_top;
        z.zero_positive_numerator += p.zero_positive_numerator;
        z.zero_other += p.zero_other;
    }
    z.decisive.sort();
    z.decisive.dedup();
    z.seconds = t0.elapsed().as_secs_f64();
    Ok(z)
}

/// The second route of a first-route producer: `exact_eval` for rr_proof and unnamed compact facts, `rr_proof`
/// (elided loci in facts mode) for exact-evaluation facts. None = no first-route fact (nothing to confirm).
pub fn second_route_of(first: &str) -> Option<&'static str> {
    match first {
        "rr_proof" | "compact" | "rr_proof_elided" => Some("exact_eval"),
        "exact_eval" => Some("rr_proof"),
        _ => None,
    }
}

/// The confirmed basis word of a pair whose two routes agree.
pub fn confirmed_basis(first: &str, elided: bool) -> &'static str {
    match (first, elided) {
        ("exact_eval", true) => "exact_eval + rr_proof_elided",
        ("exact_eval", false) => "exact_eval + rr_proof",
        ("rr_proof_elided", _) => "rr_proof_elided + exact_eval",
        ("compact", _) => "compact + exact_eval",
        _ => "rr_proof + exact_eval",
    }
}

fn coef_word(c: CoefFact) -> &'static str {
    match c {
        CoefFact::Zero => "zero",
        CoefFact::NonZero => "nonzero",
        CoefFact::Unknown => "unknown",
    }
}

/// The (record_sha256, key) pairs answered in a second-route facts file (compact fact verdict lines).
pub fn answered_pairs(p: &Path) -> Result<BTreeSet<(String, Vec<i32>)>, String> {
    let mut out = BTreeSet::new();
    let t = std::fs::read_to_string(p).map_err(|e| format!("{}: {e}", p.display()))?;
    for l in t.lines().filter(|l| !l.trim().is_empty()) {
        let v: Value = serde_json::from_str(l).map_err(|e| format!("{}: {e}", p.display()))?;
        if let (Some(sha), Ok(key)) = (
            v["record_sha256"].as_str(),
            serde_json::from_value::<Vec<i32>>(v["key"].clone()),
        ) {
            out.insert((sha.to_string(), key));
        }
    }
    Ok(out)
}

/// The term-by-term comparison of one (record, key) pair.
#[derive(Clone, Debug)]
pub struct PairCompare {
    pub sha: String,
    pub key: Vec<i32>,
    pub first: &'static str,
    pub second: &'static str,
    pub n_terms: usize,
    /// Terms whose words differ: (term, term key, first word, second word).
    pub differ: Vec<(usize, Vec<i32>, &'static str, &'static str)>,
    /// Zero terms of the first route (confirmed when `differ` is empty).
    pub zeros: usize,
}

impl PairCompare {
    pub fn agree(&self) -> bool {
        self.differ.is_empty()
    }
    pub fn json(&self) -> Value {
        json!({"record_sha256": self.sha, "key": self.key, "first": self.first, "second": self.second,
               "n_terms": self.n_terms, "zeros_first": self.zeros, "agree": self.agree(),
               "differ": self.differ.iter().map(|(j, k, a, b)| json!({"term": j, "term_key": k, "first": a, "second": b}))
                   .collect::<Vec<_>>()})
    }
}

/// Compare the two routes on (record `sha`, `key`) over every term of the record at the key. `first` = the evidence
/// the check uses, `second` = the second route's own evidence. None = the record is not in the set or has no body.
pub fn compare_pair(
    set: &ImportedSet,
    first: &Evidence,
    second: &Evidence,
    sha: &str,
    key: &[i32],
) -> Option<PairCompare> {
    let (t, r) = set.find_sha(sha)?;
    let rid = rule_id(set, t, r);
    let terms = term_keys(set, t, r, key)?;
    let fb = first.fact_basis(&rid, key).unwrap_or("none");
    let sb = second.fact_basis(&rid, key).unwrap_or("none");
    let mut pc = PairCompare {
        sha: sha.to_string(),
        key: key.to_vec(),
        first: fb,
        second: sb,
        n_terms: terms.len(),
        differ: vec![],
        zeros: 0,
    };
    for (j, tk) in terms.iter().enumerate() {
        let a = first.coefficient(&rid, key, j, tk);
        let b = second.coefficient(&rid, key, j, tk);
        if a == CoefFact::Zero {
            pc.zeros += 1;
        }
        if a != b {
            pc.differ.push((j, tk.clone(), coef_word(a), coef_word(b)));
        }
    }
    Some(pc)
}

/// What the pass does with the second route (inputs and results; closeloop.md 18.3).
#[derive(Clone, Debug, Default)]
pub struct SecondRoute {
    /// Pairs compared (second answer present), by (sha, key).
    pub compared: BTreeMap<(String, Vec<i32>), PairCompare>,
    pub seconds: f64,
}

impl SecondRoute {
    pub fn disagreements(&self) -> impl Iterator<Item = &PairCompare> {
        self.compared.values().filter(|c| !c.agree())
    }
}

/// One line of the statement's decisive-zero table (both sources).
pub fn decisive_line(
    set: &ImportedSet,
    z: &DecisiveZero,
    first: &str,
    cmp: Option<&PairCompare>,
    status: &str,
) -> Value {
    let info = &set.rules[z.table][z.record];
    let second = cmp.map(|c| {
        let w = c
            .differ
            .iter()
            .find(|d| d.0 == z.term)
            .map(|d| d.3)
            .unwrap_or("zero");
        json!({"producer": c.second, "coef": w})
    });
    json!({"schema": DECISIVE_SCHEMA, "key": z.key, "applied": z.applied, "kind": z.kind,
           "record_sha256": info.record_sha256, "source": info.source, "label": info.label,
           "term": z.term, "term_key": z.term_key,
           "first": {"producer": first, "coef": "zero"}, "second": second, "status": status})
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn routes_and_basis_words() {
        assert_eq!(second_route_of("rr_proof"), Some("exact_eval"));
        assert_eq!(second_route_of("exact_eval"), Some("rr_proof"));
        assert_eq!(second_route_of("compact"), Some("exact_eval"));
        assert_eq!(second_route_of("none"), None);
        assert_eq!(
            confirmed_basis("exact_eval", true),
            "exact_eval + rr_proof_elided"
        );
        assert_eq!(confirmed_basis("rr_proof", false), "rr_proof + exact_eval");
    }

    #[test]
    fn top_and_positive_numerator_classes() {
        let lines = [true, true, false];
        assert!(is_top(&lines, &[1, 2, 0]));
        assert!(is_top(&lines, &[1, 2, -3]));
        assert!(!is_top(&lines, &[0, 2, 0]));
        assert!(!is_top(&lines, &[1, 2, 1]));
        assert!(positive_numerator(&lines, &[1, 2, 1]));
        assert!(!positive_numerator(&lines, &[0, 2, -1]));
    }
}
