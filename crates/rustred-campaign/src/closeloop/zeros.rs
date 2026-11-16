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
//! The ROUTES: the first route is the producer of the fact the check used (`Evidence::fact_producer`: memory and
//! evidence stores alike, never a confirmed basis word); the second is the other one (rr_proof -> exact_eval;
//! exact_eval -> rr_proof with `--elided-loci facts`; an unnamed compact producer -> exact_eval). The second route's
//! answers are kept in their OWN evidence object (never merged with the first: a disagreement is reported, not a load
//! error) and compared per (record, key) over EVERY term of the record at the key; only a comparison whose two
//! producers are INDEPENDENT ([`independent`]) confirms. A first-route word no route knows (`store` of a pre-v3 store,
//! `unknown_producer`, anything else) gets NO second route and blocks every pair that relies on it (lane zerosfix3).

use rustred_campaign_core::closure::{CoefFact, FactSource, RuleId};
use rustred_campaign_core::import::zero_proofs::{
    LoadOptions as ZeroLoadOptions, Locus, ZeroProofs,
};
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
    /// lane closeloop4 (closeloop.md 21): FOLLOWED top-sector edges of rule-served keys and those NOT strictly lower
    /// under the node's full comparator (key, server, term key); zero edges that are dropped and not lower (info).
    pub followed_top: u64,
    pub non_descending: Vec<(Vec<i32>, String, Vec<i32>)>,
    pub dropped_non_descending: u64,
    pub keys_scanned: u64,
    pub zero_top: u64,
    pub zero_positive_numerator: u64,
    pub zero_other: u64,
    pub seconds: f64,
    /// lane zeros2 (closeloop.md 23): with `collect`, every rule-served pair (table, record, applied key) with the
    /// indices of its zero terms (the zeros the closure relies on), in scan order (pairs may repeat: dedup by caller).
    pub zero_pairs: Vec<ZeroPair>,
}

/// lane zeros2: one rule-served pair and the terms the first route calls zero there.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct ZeroPair {
    pub table: usize,
    pub record: usize,
    pub applied: Vec<i32>,
    pub terms: Vec<u32>,
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
        .instantiate(
            &set.disp,
            set.disp.rule_ref(table, record),
            applied,
            &mut buf,
            &mut cm,
        )
        .ok()??;
    let n = set.arity();
    Some((0..nt).map(|j| buf[j * n..(j + 1) * n].to_vec()).collect())
}

/// The rule-served rows of a decided Tier C check: (key, applied, table, record). Table-rule rows come from the
/// streamed rows file (every row; `server.kind == "rule"`), credits from the kept rows (their image).
pub fn served_rule_rows(set: &ImportedSet, res: &CheckResult) -> Result<Vec<ServedRow>, String> {
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
                let v: Value = serde_json::from_str(&l)
                    .map_err(|e| format!("{} line {}: {e}", st.path, i + 1))?;
                if v["status"] != "served" || v["server"]["kind"] != "rule" {
                    continue;
                }
                let (Some(sha), Ok(key)) = (
                    v["server"]["record_sha256"].as_str(),
                    serde_json::from_value::<Vec<i32>>(v["key"].clone()),
                ) else {
                    return Err(format!("{} line {}: key / server record", st.path, i + 1));
                };
                let (t, r) = set.find_sha(sha).ok_or_else(|| {
                    format!("{} line {}: record {sha} not in the set", st.path, i + 1)
                })?;
                out.push((key.clone(), key, t, r));
            }
        }
        None => {
            for r in &res.rows {
                if let (RowStatus::Served, Some(Server::Rule { table, record })) =
                    (r.status, &r.server)
                {
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

/// A rule-served row: (key, applied key, table, record).
pub type ServedRow = (Vec<i32>, Vec<i32>, usize, usize);
/// Membership test of a closure.
pub type Membership<'a> = Box<dyn Fn(&[i32]) -> bool + Sync + 'a>;

/// Membership in the closure of a check (the packed key set of a streamed check, else the rows).
pub fn closure_membership(res: &CheckResult) -> Membership<'_> {
    // lane layers (closeloop.md 23): a key of a leaf set (an earlier, closed layer / clause) is a closure key
    let lv = res.leaves.as_ref();
    match &res.key_set {
        Some(ks) => Box::new(move |k: &[i32]| ks.contains(k) || lv.is_some_and(|h| h.contains(k))),
        None => {
            let s: HashSet<Vec<i32>> = res.rows.iter().map(|r| r.key.clone()).collect();
            Box::new(move |k: &[i32]| s.contains(k) || lv.is_some_and(|h| h.contains(k)))
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
    scan_with(set, facts, res, threads, false)
}

/// [`scan`]; lane zeros2: `collect` also lists every served pair's zero terms ([`ZeroScan::zero_pairs`]).
pub fn scan_with(
    set: &ImportedSet,
    facts: &(dyn FactSource + Sync),
    res: &CheckResult,
    threads: usize,
    collect: bool,
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
                        let mut zs: Vec<u32> = vec![];
                        for (j, tk) in terms.iter().enumerate() {
                            if facts.coefficient(&rid, applied, j, tk) != CoefFact::Zero {
                                // a FOLLOWED edge: strictly lower than the key under the full comparator?
                                if is_top(lines, tk) {
                                    z.followed_top += 1;
                                    if !set.cmp.lower(tk, key) {
                                        let i = &set.rules[*t][*r];
                                        z.non_descending.push((
                                            key.clone(),
                                            format!("{}#{}", i.source, i.label),
                                            tk.clone(),
                                        ));
                                    }
                                }
                                continue;
                            }
                            if collect {
                                zs.push(j as u32);
                            }
                            if is_top(lines, tk) && !set.cmp.lower(tk, key) {
                                z.dropped_non_descending += 1;
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
                        if !zs.is_empty() {
                            z.zero_pairs.push(ZeroPair {
                                table: *t,
                                record: *r,
                                applied: applied.clone(),
                                terms: zs,
                            });
                        }
                    }
                    z
                })
            })
            .collect();
        hs.into_iter()
            .map(|h| h.join().unwrap_or_default())
            .collect()
    });
    let mut z = ZeroScan::default();
    for p in parts {
        z.decisive.extend(p.decisive);
        z.keys_scanned += p.keys_scanned;
        z.zero_top += p.zero_top;
        z.zero_positive_numerator += p.zero_positive_numerator;
        z.zero_other += p.zero_other;
        z.followed_top += p.followed_top;
        z.non_descending.extend(p.non_descending);
        z.dropped_non_descending += p.dropped_non_descending;
        z.zero_pairs.extend(p.zero_pairs);
    }
    z.non_descending.sort();
    z.decisive.sort();
    z.decisive.dedup();
    z.seconds = t0.elapsed().as_secs_f64();
    Ok(z)
}

/// lane closeloop4 (closeloop.md 21): the edges of RELATION-served keys (supplied point relations, imported or
/// derived symmetry relations, our recipe relations; a credit through a relation is compared with the ORIGINAL key):
/// (followed top edges, non-descending ones as (key, server, term key)).
#[allow(clippy::type_complexity)]
pub fn relation_descent(
    set: &ImportedSet,
    ev: &Evidence,
    res: &CheckResult,
) -> (u64, Vec<(Vec<i32>, String, Vec<i32>)>) {
    let lines = set.frame.mask().to_vec();
    let (mut n, mut bad) = (0u64, vec![]);
    for r in &res.rows {
        // a relation the import REFUSED for not descending (a supplied / symmetry relation left without its
        // `descends` verdict bit: the key is open `recipeunverified`) is a non-descending edge as well
        if r.status == RowStatus::Open && r.open_status.as_deref() == Some("recipeunverified") {
            for rs in &set.supplied {
                if let Some(i) = rs.relations.find(&r.key) {
                    let st = &rs.relations;
                    let w = st.codec().words();
                    let mut buf = vec![0i32; set.arity()];
                    for c in st.rhs_codes(i).chunks_exact(w) {
                        st.codec().decode(c, &mut buf);
                        if is_top(&lines, &buf) && !set.cmp.lower(&buf, &r.key) {
                            bad.push((r.key.clone(), rs.name.clone(), buf.clone()));
                        }
                    }
                }
            }
            continue;
        }
        let (RowStatus::Served, Some(Server::Relation { set: name, recipe })) =
            (r.status, &r.server)
        else {
            continue;
        };
        let at = r.image.clone().unwrap_or_else(|| r.key.clone());
        let rhs: Vec<Vec<i32>> = if *recipe {
            ev.solved
                .get(name)
                .and_then(|m| m.get(&at))
                .map(|s| s.rhs.clone())
                .unwrap_or_default()
        } else {
            match set.supplied.iter().find(|x| &x.name == name) {
                Some(rs) => match rs.relations.find(&at) {
                    Some(i) => {
                        let st = &rs.relations;
                        let w = st.codec().words();
                        let mut buf = vec![0i32; set.arity()];
                        st.rhs_codes(i)
                            .chunks_exact(w)
                            .map(|c| {
                                st.codec().decode(c, &mut buf);
                                buf.clone()
                            })
                            .collect()
                    }
                    None => vec![],
                },
                None => vec![],
            }
        };
        for t in rhs {
            if is_top(&lines, &t) {
                n += 1;
                if !set.cmp.lower(&t, &r.key) {
                    bad.push((r.key.clone(), name.clone(), t));
                }
            }
        }
    }
    // our recipe relations the import refused because a right-hand-side key is not lower than the key
    for rf in &res.solve_refusals {
        if rf.reason.contains("is not lower than the key")
            && let Some(rel) = ev.solved.get(&rf.source).and_then(|m| m.get(&rf.key))
        {
            for t in &rel.rhs {
                if is_top(&lines, t) && !set.cmp.lower(t, &rf.key) {
                    bad.push((rf.key.clone(), rf.source.clone(), t.clone()));
                }
            }
        }
    }
    bad.sort();
    bad.dedup();
    (n, bad)
}

/// The second route of a first-route PRODUCER: `exact_eval` for rr_proof (incl. elided-loci facts) and unnamed compact
/// facts, `rr_proof` (elided loci in facts mode) for exact-evaluation facts. None = a word that is no known producer:
/// `none` (no fact bound), a confirmed basis word, `store` (an entry of a pre-v3 evidence store), `unknown_producer`
/// (a store code this build does not know), anything else: the caller BLOCKS such a pair (lane zerosfix3, fail closed).
pub fn second_route_of(first: &str) -> Option<&'static str> {
    match first {
        "rr_proof" | "compact" | "rr_proof_elided" => Some("exact_eval"),
        "exact_eval" => Some("rr_proof"),
        _ => None,
    }
}

/// lane zerosfix3: whether a second-route answer of producer `second` is INDEPENDENT of the first route's producer
/// `first` (the two share no code below the coefficient text): rr_proof (its own std-only text evaluator) and an
/// unnamed compact producer against the exact evaluation (Symbolica), and back. Only such a comparison confirms a fact.
pub fn independent(first: &str, second: &str) -> bool {
    match first {
        "rr_proof" | "compact" | "rr_proof_elided" => second == "exact_eval",
        "exact_eval" => second == "rr_proof" || second == "rr_proof_elided",
        _ => false,
    }
}

/// lane zerosfix3: the second route of a served pair with first-route ZERO terms (closeloop.md 23.3 + 18.2).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ZeroRoute {
    /// closeloop4's term-by-term route to rr_proof: the first route IS the exact evaluation (the zeros route runs on the
    /// same algebra, Symbolica, and would not be independent).
    Compare,
    /// the zeros route (`rr_coef_facts`, Symbolica): independent of rr_proof and of an unnamed compact producer.
    Zeros,
    /// no route knows the first route's word: the pair BLOCKS (`second_route_basis_unknown`).
    Unknown,
}

/// lane zerosfix3: the [`ZeroRoute`] of a first-route PRODUCER word (never a confirmed basis word).
pub fn zero_route_of(first: &str) -> ZeroRoute {
    match first {
        "exact_eval" => ZeroRoute::Compare,
        "rr_proof" | "rr_proof_elided" | "compact" => ZeroRoute::Zeros,
        _ => ZeroRoute::Unknown,
    }
}

/// The confirmed basis word of a pair whose two routes agree.
pub fn confirmed_basis(first: &str, elided: bool) -> &'static str {
    match (first, elided) {
        ("exact_eval", true) => "exact_eval + rr_proof_elided",
        ("exact_eval", false) => "exact_eval + rr_proof",
        ("rr_proof_elided", _) => "rr_proof_elided + exact_eval",
        ("compact", _) => "compact + exact_eval",
        ("rr_proof", _) => "rr_proof + exact_eval",
        // lane zerosfix3: never reached (only an independent comparison confirms); no known word is invented
        _ => "unknown_basis + second_route",
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
    /// lane zerosfix3: the comparison CONFIRMS the first route: every term agrees AND the two producers are
    /// independent ([`independent`]); agreement of two dependent (or unknown) producers confirms nothing.
    pub fn confirms(&self) -> bool {
        self.agree() && independent(self.first, self.second)
    }
    pub fn json(&self) -> Value {
        json!({"record_sha256": self.sha, "key": self.key, "first": self.first, "second": self.second,
               "independent": independent(self.first, self.second),
               "n_terms": self.n_terms, "zeros_first": self.zeros, "agree": self.agree(),
               "differ": self.differ.iter().map(|(j, k, a, b)| json!({"term": j, "term_key": k, "first": a, "second": b}))
                   .collect::<Vec<_>>()})
    }
}

/// Compare the two routes on (record `sha`, `key`) over every term of the record at the key. `first` = the evidence
/// the check uses, `second` = the second route's own evidence. None = the record is not in the set or has no body.
/// lane zerosfix3: `first` / `second` of the result are the PRODUCERS (`Evidence::fact_producer`), in big-node mode too.
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
    let fb = first.fact_producer(&rid, key).unwrap_or("none");
    let sb = second.fact_producer(&rid, key).unwrap_or("none");
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

// ---------------------------------------------------------------------------------------------------------------------
// lane zeros2 (interfaces/closeloop.md 23, interfaces/zero-proofs.md): the second route of EVERY zero term
// ---------------------------------------------------------------------------------------------------------------------

/// One asked term's answer: (term, word, how it was decided).
pub type ZeroWord = (u32, CoefFact, String);
/// Zero terms to ask per record: (table, record) -> [(applied key, term indices)].
pub type ZerosMissing = BTreeMap<(usize, usize), Vec<(Vec<i32>, Vec<u32>)>>;

/// The answers of zeros asks: per (record_sha256, applied key) the asked terms' words and how each was decided, and the
/// structural proofs (verified on load by the core's fact source).
#[derive(Default)]
pub struct ZerosAnswers {
    /// (sha, key) -> (term, word, by) where by = `plane:p=v` | `domain` (optionally `+den`) | `key` | other.
    pub verdicts: BTreeMap<(String, Vec<i32>), Vec<ZeroWord>>,
    pub proofs: ZeroProofs,
    /// Proof references that no loaded proof line backs (record, key, term, by): such a term is not confirmed.
    pub unbound: Vec<(String, Vec<i32>, u32, String)>,
}

fn word_of(w: &str) -> CoefFact {
    match w {
        "zero" => CoefFact::Zero,
        "nonzero" => CoefFact::NonZero,
        _ => CoefFact::Unknown,
    }
}

/// The locus named by a verdict's `by` (`plane:p=v[+den]`, `domain[+den]`); None = decided at the key.
pub fn by_locus(by: &str) -> Option<Locus> {
    let b = by.strip_suffix("+den").unwrap_or(by);
    if b == "domain" {
        return Some(Locus::Domain);
    }
    let (p, v) = b.strip_prefix("plane:")?.split_once('=')?;
    Some(Locus::Plane {
        param: p.parse().ok()?,
        value: v.parse().ok()?,
    })
}

impl ZerosAnswers {
    /// Read one answer file (verdict lines and proof lines; proof lines verified against the set: any violation refuses).
    pub fn load(&mut self, set: &ImportedSet, p: &Path) -> Result<(), String> {
        self.proofs
            .load_files(set, &[p.to_path_buf()], &ZeroLoadOptions::default())?;
        let t = std::fs::read_to_string(p).map_err(|e| format!("{}: {e}", p.display()))?;
        for (i, l) in t.lines().enumerate().filter(|(_, l)| !l.trim().is_empty()) {
            let v: Value = serde_json::from_str(l)
                .map_err(|e| format!("{} line {}: {e}", p.display(), i + 1))?;
            if v["schema"] != super::proofs::ZERO_VERDICT_SCHEMA {
                continue;
            }
            let (Some(sha), Ok(key)) = (
                v["record_sha256"].as_str(),
                serde_json::from_value::<Vec<i32>>(v["key"].clone()),
            ) else {
                return Err(format!("{} line {}: record / key", p.display(), i + 1));
            };
            let mut words = vec![];
            for x in v["terms"].as_array().into_iter().flatten() {
                let (Some(j), Some(w), Some(by)) = (x[0].as_u64(), x[1].as_str(), x[2].as_str())
                else {
                    return Err(format!("{} line {}: terms entry {x}", p.display(), i + 1));
                };
                words.push((j as u32, word_of(w), by.to_string()));
            }
            self.verdicts.insert((sha.to_string(), key), words);
        }
        Ok(())
    }

    /// Bind every structural reference to a loaded proof whose locus holds the key's rule point (after all files are
    /// loaded): an unbacked reference is recorded in `unbound` and its word becomes `unknown`. lane zerosfix3 (F4): the
    /// proof must answer `Proved` there (`ZeroProofs::line_status_at_point`: the denominator check included).
    pub fn bind(&mut self, set: &ImportedSet) {
        let mut unbound = vec![];
        for ((sha, key), words) in self.verdicts.iter_mut() {
            let Some((t, r)) = set.find_sha(sha) else {
                continue;
            };
            let mut pt = vec![];
            let has_pt = rustred_campaign_core::closure::bodies::rule_point(
                &set.disp.tables()[t].domains[r],
                key,
                &mut pt,
            );
            for (j, w, by) in words.iter_mut() {
                let Some(l) = by_locus(by) else {
                    continue;
                };
                // lane zerosfix3 (audit F4): a `zero` word relies on the named line, which must PROVE the term at the
                // key's rule point with the ONE semantics (its locus holds the point AND its denominator check passes
                // there: a where_regular proof at a 0/0 point is not a proof), not merely exist; any other word only
                // names the line it looked at (it must exist and hold the point)
                let backed = if *w == CoefFact::Zero {
                    has_pt
                        && self.proofs.line_status_at_point(t, r, *j as usize, l, &pt)
                            == rustred_campaign_core::import::zero_proofs::ZeroStatus::Proved
                } else {
                    has_pt
                        && l.contains(&pt)
                        && self
                            .proofs
                            .of_record(t, r)
                            .iter()
                            .any(|(pj, pl, _)| *pj == *j && *pl == l)
                };
                if !backed {
                    unbound.push((sha.clone(), key.clone(), *j, by.clone()));
                    *w = CoefFact::Unknown;
                }
            }
        }
        self.unbound = unbound;
    }
}

/// The comparison of one zeros-answered pair with the first route.
#[derive(Clone, Debug, Default)]
pub struct ZeroCompare {
    /// Terms the first route calls zero.
    pub first_zero: u64,
    /// ... confirmed by the answer, by how: structural proof (nowhere-zero denominator) / structural + per-key
    /// denominator / per-key evaluation.
    pub by_structural: u64,
    pub by_structural_den: u64,
    pub by_key: u64,
    /// First-route zeros the answer contradicts: (term, term key, second word).
    pub differ: Vec<(u32, Vec<i32>, &'static str)>,
    /// First-route zeros the answer does not cover (asked set differs).
    pub missing: Vec<u32>,
    /// confirmed zero terms by class: positive numerator, top, other.
    pub pos: u64,
    pub top: u64,
    pub other: u64,
}

impl ZeroCompare {
    pub fn agree(&self) -> bool {
        self.differ.is_empty() && self.missing.is_empty()
    }
}

/// Compare a zeros answer with the first route at (record, applied key): every term the first route calls zero must be
/// answered `zero`. None: the record is not in the set or has no body.
pub fn compare_zeros(
    set: &ImportedSet,
    first: &Evidence,
    answers: &ZerosAnswers,
    sha: &str,
    key: &[i32],
) -> Option<ZeroCompare> {
    let (t, r) = set.find_sha(sha)?;
    let words = answers.verdicts.get(&(sha.to_string(), key.to_vec()))?;
    let rid = rule_id(set, t, r);
    let terms = term_keys(set, t, r, key)?;
    let lines = set.frame.mask();
    let by: BTreeMap<u32, (CoefFact, &str)> = words
        .iter()
        .map(|(j, w, b)| (*j, (*w, b.as_str())))
        .collect();
    let mut c = ZeroCompare::default();
    for (j, tk) in terms.iter().enumerate() {
        if first.coefficient(&rid, key, j, tk) != CoefFact::Zero {
            continue;
        }
        c.first_zero += 1;
        match by.get(&(j as u32)) {
            None => c.missing.push(j as u32),
            Some((CoefFact::Zero, b)) => {
                if b.ends_with("+den") {
                    c.by_structural_den += 1;
                } else if by_locus(b).is_some() {
                    c.by_structural += 1;
                } else {
                    c.by_key += 1;
                }
                if positive_numerator(lines, tk) {
                    c.pos += 1;
                } else if is_top(lines, tk) {
                    c.top += 1;
                } else {
                    c.other += 1;
                }
            }
            Some((w, _)) => c.differ.push((j as u32, tk.clone(), coef_word(*w))),
        }
    }
    Some(c)
}

/// The confirmed basis word of a pair whose zeros were all confirmed by the zeros route (first producer, how).
pub fn zeros_basis(first: &str, c: &ZeroCompare) -> &'static str {
    let structural = c.by_structural + c.by_structural_den > 0;
    let per_key = c.by_key > 0;
    match (first, structural, per_key) {
        ("compact", true, false) => "compact + zeros:structural",
        ("compact", false, _) => "compact + zeros:per_key",
        ("compact", true, true) => "compact + zeros:mixed",
        ("rr_proof_elided", true, false) => "rr_proof_elided + zeros:structural",
        ("rr_proof_elided", false, _) => "rr_proof_elided + zeros:per_key",
        ("rr_proof_elided", true, true) => "rr_proof_elided + zeros:mixed",
        (_, true, false) => "rr_proof + zeros:structural",
        (_, false, _) => "rr_proof + zeros:per_key",
        (_, true, true) => "rr_proof + zeros:mixed",
    }
}

/// Batches of zeros asks: one table per batch, at most `keys_cap` (record, key) pairs and `terms_cap` asked terms.
/// `missing`: (table, record) -> [(applied key, terms)].
pub fn zeros_batches(
    set: &ImportedSet,
    missing: &ZerosMissing,
    keys_cap: usize,
    terms_cap: usize,
    reason: &str,
) -> Vec<super::proofs::ProofBatch> {
    use super::proofs::{ProofBatch, RecordReq};
    let (keys_cap, terms_cap) = (keys_cap.max(1), terms_cap.max(1));
    let mut by_table: BTreeMap<String, Vec<(usize, usize)>> = BTreeMap::new();
    for (t, r) in missing.keys() {
        by_table
            .entry(set.rules[*t][*r].path.clone())
            .or_default()
            .push((*t, *r));
    }
    let mut out = vec![];
    for (table, recs) in by_table {
        let mut cur: Vec<RecordReq> = vec![];
        let (mut nk, mut nt) = (0usize, 0usize);
        for (t, r) in recs {
            let info = &set.rules[t][r];
            let mut req = RecordReq {
                rule: info.index.unwrap_or(info.label),
                offset: info.offset,
                record_sha256: info.record_sha256.clone(),
                ..Default::default()
            };
            for (k, ts) in &missing[&(t, r)] {
                if (nk + 1 > keys_cap || nt + ts.len() > terms_cap) && (nk > 0) {
                    if !req.keys.is_empty() {
                        cur.push(std::mem::replace(
                            &mut req,
                            RecordReq {
                                rule: info.index.unwrap_or(info.label),
                                offset: info.offset,
                                record_sha256: info.record_sha256.clone(),
                                ..Default::default()
                            },
                        ));
                    }
                    out.push(ProofBatch {
                        kind: "exact".into(),
                        reason: reason.into(),
                        table: table.clone(),
                        records: std::mem::take(&mut cur),
                        relation_source: None,
                    });
                    nk = 0;
                    nt = 0;
                }
                req.keys.push(k.clone());
                req.terms.push(ts.clone());
                nk += 1;
                nt += ts.len();
            }
            if !req.keys.is_empty() {
                cur.push(req);
            }
        }
        if !cur.is_empty() {
            out.push(ProofBatch {
                kind: "exact".into(),
                reason: reason.into(),
                table: table.clone(),
                records: cur,
                relation_source: None,
            });
        }
    }
    out
}

/// One line of the statement's zero-term confirmation table (`statement/zero_terms.jsonl`): per confirmed or disputed
/// pair, the counts by route.
pub fn zeros_pair_line(
    set: &ImportedSet,
    sha: &str,
    key: &[i32],
    first: &str,
    c: &ZeroCompare,
) -> Value {
    let (label, source) = set
        .find_sha(sha)
        .map(|(t, r)| (set.rules[t][r].label, set.rules[t][r].source.clone()))
        .unwrap_or((0, String::new()));
    json!({"schema": ZEROS_PAIR_SCHEMA, "record_sha256": sha, "source": source, "label": label, "applied": key,
        "first": first, "zero_terms": c.first_zero, "structural": c.by_structural,
        "structural_den_at_key": c.by_structural_den, "per_key": c.by_key,
        "positive_numerator": c.pos, "top": c.top, "other": c.other,
        "status": if c.agree() { "confirmed" } else { "disagreement" },
        "differ": c.differ.iter().map(|(j, tk, w)| json!({"term": j, "term_key": tk, "first": "zero", "second": w}))
            .collect::<Vec<_>>(), "missing": c.missing})
}

/// Schema of a line of `statement/zero_terms.jsonl`.
pub const ZEROS_PAIR_SCHEMA: &str = "rustred.campaign.closeloop-zero-terms.v1";

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn routes_and_basis_words() {
        assert_eq!(second_route_of("rr_proof"), Some("exact_eval"));
        assert_eq!(second_route_of("exact_eval"), Some("rr_proof"));
        assert_eq!(second_route_of("compact"), Some("exact_eval"));
        assert_eq!(second_route_of("none"), None);
        // lane zerosfix3: no route for a word that is no producer (fail closed)
        for w in ["store", "unknown_producer", "rr_proof + exact_eval", ""] {
            assert_eq!(second_route_of(w), None, "{w}");
            assert_eq!(zero_route_of(w), ZeroRoute::Unknown, "{w}");
            assert!(!independent(w, "exact_eval") && !independent(w, "rr_proof"));
        }
        assert_eq!(zero_route_of("exact_eval"), ZeroRoute::Compare);
        for w in ["rr_proof", "rr_proof_elided", "compact"] {
            assert_eq!(zero_route_of(w), ZeroRoute::Zeros);
            assert!(independent(w, "exact_eval") && !independent(w, w));
        }
        assert!(
            independent("exact_eval", "rr_proof") && independent("exact_eval", "rr_proof_elided")
        );
        assert!(!independent("exact_eval", "exact_eval") && !independent("exact_eval", "compact"));
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
