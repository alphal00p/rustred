//! Lane wireflow (interfaces/closeloop.md 26): FACTS FROM THE FRONTIER.
//!
//! With `facts_frontier` on a node of the exact tier, a pass no longer asks the coefficient facts of every key the
//! keys-only superset reaches. Its first walk is the Tier C walk that STOPS at every rule-served key without its fact
//! (core `import::check::check_frontier`): such a key is dispatched and its fact requested, but not expanded. The facts
//! asked are that walk's FRONTIER: deduplicated (record, key) pairs not asked before, in fact jobs of at most
//! `max_keys_per_job` keys (lane allnodes: <= 500,000 per rr_proof job). The next pass walks again with the answers (a
//! term with a zero fact is no edge), to a FIXED POINT: no reached key lacks its fact, and the plain Tier C walk decides
//! as before. Once, before the node's first fact job, the facts of the keys-only superset's rule-served keys OUTSIDE
//! the box (start set) are PREFETCHED (lane allnodes) when they fit the budget; otherwise the prefetch is skipped and
//! recorded.
//!
//! The FACTS BUDGET (`max_pairs`, `max_jobs`) bounds every first-route fact pair / fact job the node issues (rr_proof
//! facts + exact evaluation; ledger counts, so a resume counts what was issued before). A frontier that does not fit
//! ends the node TYPED (`facts budget`): no fact job, no final verdict, the frontier saved as records
//! (`closeloop:<n>:facts-budget:r<k>` + its pairs file `closeloop:<n>:facts-frontier:r<k>`); the run stops for the
//! operator (exit 8) until `rr_campaign control CDIR raise-facts-budget --node N --max-pairs P --max-jobs J` records a
//! larger budget (`closeloop:<n>:facts-budget-raise:<id>`), and the same campaign resumes from the saved state.
//!
//! Every pass of the mode records frontier size, pairs requested, zeros confirmed and closure size (check record
//! `frontier`). Absent setting = the keys-only first pass as before (configs and records unchanged).

use rustred_campaign_core::import::{Evidence, ImportedSet, WitnessKind};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::collections::{BTreeMap, BTreeSet};
use std::io::BufRead;
use std::path::Path;

/// Record of a facts-budget end (the frontier that did not fit).
pub const SCHEMA_BUDGET: &str = "rustred.campaign.closeloop-facts-budget.v1";
/// Record of an operator's budget raise.
pub const SCHEMA_RAISE: &str = "rustred.campaign.closeloop-facts-budget-raise.v1";
pub const ROLE_FBUDGET: &str = "closeloop_facts_budget";
pub const ROLE_FRONTIER: &str = "closeloop_facts_frontier";
pub const ROLE_FRAISE: &str = "closeloop_facts_budget_raise";
/// The control verb of a raise.
pub const VERB_RAISE: &str = "raise-facts-budget";
/// Keys per fact job of the mode, at most (lane allnodes' chunk).
pub const MAX_KEYS_PER_JOB_CAP: u64 = 500_000;

pub fn input_fbudget(n: &str, k: u32) -> String {
    format!("closeloop:{n}:facts-budget:r{k}")
}
pub fn input_frontier(n: &str, k: u32) -> String {
    format!("closeloop:{n}:facts-frontier:r{k}")
}
pub fn input_fraise_prefix(n: &str) -> String {
    format!("closeloop:{n}:facts-budget-raise:")
}
pub fn input_fbudget_lift(n: &str, id: &str) -> String {
    format!("{}{id}", input_fraise_prefix(n))
}

fn d_chunk() -> u64 {
    MAX_KEYS_PER_JOB_CAP
}
fn d_true() -> bool {
    true
}

/// The node setting `facts_frontier` (closeloop.md 26). Absent = off.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FrontierCfg {
    /// The facts budget: (record, key) pairs the node's first-route fact jobs may ask in all.
    pub max_pairs: u64,
    /// The facts budget: first-route fact jobs (rr_proof facts + exact evaluation) the node may issue in all.
    pub max_jobs: u64,
    /// Keys per fact job of the mode (<= 500,000; the proof's `max_fact_keys_per_job` applies when lower).
    #[serde(default = "d_chunk")]
    pub max_keys_per_job: u64,
    /// Prefetch the superset's rule-served keys outside the box before the node's first fact job (when they fit).
    #[serde(default = "d_true")]
    pub prefetch_outside: bool,
}

impl FrontierCfg {
    pub fn validate(&self, node: &str) -> Result<(), String> {
        if self.max_pairs == 0 || self.max_jobs == 0 {
            return Err(format!(
                "node {node}: facts_frontier.max_pairs and max_jobs must be >= 1"
            ));
        }
        if self.max_keys_per_job == 0 || self.max_keys_per_job > MAX_KEYS_PER_JOB_CAP {
            return Err(format!(
                "node {node}: facts_frontier.max_keys_per_job must be in 1..={MAX_KEYS_PER_JOB_CAP}"
            ));
        }
        Ok(())
    }
}

/// What a pass of the mode gets from the source.
#[derive(Clone, Debug)]
pub struct FrontierIn {
    pub cfg: FrontierCfg,
    /// The effective budget: the config's, or the latest operator raise.
    pub max_pairs: u64,
    pub max_jobs: u64,
    /// The raise in force (record id), if any.
    pub raised_by: Option<String>,
    /// First-route fact jobs the node issued so far (ledger).
    pub jobs_used: u64,
}

impl FrontierIn {
    /// The budget in force from the config and the recorded raises (ledger order: the last one counts).
    pub fn new(cfg: &FrontierCfg, raises: &[Value], jobs_used: u64) -> FrontierIn {
        let last = raises.last();
        FrontierIn {
            cfg: cfg.clone(),
            max_pairs: last
                .and_then(|r| r["max_pairs"].as_u64())
                .unwrap_or(cfg.max_pairs),
            max_jobs: last
                .and_then(|r| r["max_jobs"].as_u64())
                .unwrap_or(cfg.max_jobs),
            raised_by: last.and_then(|r| r["id"].as_str().map(String::from)),
            jobs_used,
        }
    }

    pub fn budget_json(&self, pairs_used: u64) -> Value {
        json!({"max_pairs": self.max_pairs, "max_jobs": self.max_jobs, "raised_by": self.raised_by,
               "pairs_used_before": pairs_used, "jobs_used_before": self.jobs_used})
    }
}

/// What fits the budget this pass.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Fit {
    /// The frontier and the prefetch.
    All,
    /// The frontier only (the prefetch is skipped).
    FrontierOnly,
    /// Not even the frontier: the typed end.
    Stop,
}

/// `with` / `without` = (pairs, jobs) with / without the prefetch.
pub fn fit(fin: &FrontierIn, pairs_used: u64, with: (u64, u64), without: (u64, u64)) -> Fit {
    let ok = |(p, j): (u64, u64)| {
        pairs_used.saturating_add(p) <= fin.max_pairs
            && fin.jobs_used.saturating_add(j) <= fin.max_jobs
    };
    if ok(with) {
        Fit::All
    } else if ok(without) {
        Fit::FrontierOnly
    } else {
        Fit::Stop
    }
}

type Pairs = BTreeMap<(usize, usize), BTreeSet<Vec<i32>>>;

/// The prefetch: the rule-served keys OUTSIDE the box of a keys-only rows file (statement lines) whose fact is not
/// bound and was not asked; elided records go to exact evaluation when it is configured (else they are skipped).
pub fn outside_pairs(
    set: &ImportedSet,
    ev: &Evidence,
    rows: &Path,
    issued: &BTreeSet<(String, Vec<i32>)>,
    exact_eval: bool,
) -> Result<(Pairs, Pairs), String> {
    let (mut facts, mut exact) = (Pairs::new(), Pairs::new());
    let f = std::fs::File::open(rows).map_err(|e| format!("{}: {e}", rows.display()))?;
    for (i, l) in std::io::BufReader::with_capacity(1 << 22, f)
        .lines()
        .enumerate()
    {
        let l = l.map_err(|e| format!("{}: {e}", rows.display()))?;
        if l.trim().is_empty() {
            continue;
        }
        let v: Value = serde_json::from_str(&l)
            .map_err(|e| format!("{} line {}: {e}", rows.display(), i + 1))?;
        if v["in_start"] != false || v["status"] != "served" || v["server"]["kind"] != "rule" {
            continue;
        }
        let Some(sha) = v["server"]["record_sha256"].as_str() else {
            continue;
        };
        let (t, r) = set.find_sha(sha).ok_or_else(|| {
            format!(
                "{} line {}: served by a record the set does not hold ({sha})",
                rows.display(),
                i + 1
            )
        })?;
        let key: Vec<i32> = serde_json::from_value(v["key"].clone())
            .map_err(|e| format!("{} line {}: key: {e}", rows.display(), i + 1))?;
        if issued.contains(&(sha.to_string(), key.clone())) {
            continue;
        }
        if set.rules[t][r].witness == WitnessKind::Elided {
            if exact_eval && !ev.has_fact(set, t, r, &key) {
                exact.entry((t, r)).or_default().insert(key);
            }
        } else if !ev.has_lift(set, t, r, &key) {
            facts.entry((t, r)).or_default().insert(key);
        }
    }
    Ok((facts, exact))
}

/// `a` minus `b` (pairs).
pub fn minus(a: &Pairs, b: &Pairs) -> Pairs {
    let mut out = Pairs::new();
    for (tr, ks) in a {
        let rest: BTreeSet<Vec<i32>> = match b.get(tr) {
            Some(o) => ks.difference(o).cloned().collect(),
            None => ks.clone(),
        };
        if !rest.is_empty() {
            out.insert(*tr, rest);
        }
    }
    out
}

/// `a` with `b` added.
pub fn union(a: &Pairs, b: &Pairs) -> Pairs {
    let mut out = a.clone();
    for (tr, ks) in b {
        out.entry(*tr).or_default().extend(ks.iter().cloned());
    }
    out
}

pub fn count(p: &Pairs) -> u64 {
    p.values().map(|s| s.len() as u64).sum()
}

/// The saved frontier: one canonical line per pair (record sha256, table path, key, route).
pub fn pairs_jsonl(set: &ImportedSet, facts: &Pairs, exact: &Pairs) -> Vec<u8> {
    let mut out = Vec::new();
    for (route, m) in [("facts", facts), ("exact", exact)] {
        for ((t, r), ks) in m {
            let info = &set.rules[*t][*r];
            for k in ks {
                out.extend_from_slice(
                    json!({"record_sha256": info.record_sha256, "table": info.path, "key": k, "route": route})
                        .to_string()
                        .as_bytes(),
                );
                out.push(b'\n');
            }
        }
    }
    out
}

/// The record of a facts-budget end.
#[allow(clippy::too_many_arguments)]
pub fn budget_record(
    node: &str,
    round: u32,
    fin: &FrontierIn,
    pairs_used: u64,
    wanted: (u64, u64),
    frontier_keys: u64,
    prefetch: &Value,
    pairs_sha256: &str,
) -> Value {
    json!({"schema": SCHEMA_BUDGET, "node": node, "round": round, "end": "facts budget",
        "budget": fin.budget_json(pairs_used),
        "wanted": {"pairs": wanted.0, "jobs": wanted.1, "frontier_keys": frontier_keys},
        "prefetch": prefetch,
        "frontier": {"file": format!("facts-frontier.r{round}.jsonl"), "pairs": wanted.0, "sha256": pairs_sha256},
        "resume": format!("rr_campaign control CDIR {VERB_RAISE} --node {node} --max-pairs P --max-jobs J \
                           (P >= {} and J >= {} for this frontier)",
                          pairs_used.saturating_add(wanted.0), fin.jobs_used.saturating_add(wanted.1))})
}

/// The record of an operator raise (request payload `{"max_pairs": P, "max_jobs": J}`).
pub fn raise_record(
    node: &str,
    id: &str,
    by: &str,
    reason: &str,
    payload: &Value,
) -> Result<Value, String> {
    let get = |k: &str| {
        payload[k]
            .as_u64()
            .filter(|&x| x >= 1)
            .ok_or_else(|| format!("{VERB_RAISE}: payload {k} must be an integer >= 1"))
    };
    Ok(
        json!({"schema": SCHEMA_RAISE, "node": node, "id": id, "by": by, "reason": reason,
               "max_pairs": get("max_pairs")?, "max_jobs": get("max_jobs")?}),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cfg() -> FrontierCfg {
        serde_json::from_value(json!({"max_pairs": 10, "max_jobs": 2})).unwrap()
    }

    #[test]
    fn defaults_validation_and_the_raise_in_force() {
        let c = cfg();
        assert_eq!(c.max_keys_per_job, MAX_KEYS_PER_JOB_CAP);
        assert!(c.prefetch_outside);
        assert!(c.validate("N").is_ok());
        let mut bad = c.clone();
        bad.max_keys_per_job = MAX_KEYS_PER_JOB_CAP + 1;
        assert!(bad.validate("N").is_err());
        assert!(
            serde_json::from_value::<FrontierCfg>(
                json!({"max_pairs": 1, "max_jobs": 1, "maxjobs": 3})
            )
            .is_err()
        );
        let f = FrontierIn::new(&c, &[], 1);
        assert_eq!(
            (f.max_pairs, f.max_jobs, f.raised_by.clone()),
            (10, 2, None)
        );
        let r1 =
            raise_record("N", "a", "op", "", &json!({"max_pairs": 50, "max_jobs": 5})).unwrap();
        let r2 =
            raise_record("N", "b", "op", "", &json!({"max_pairs": 70, "max_jobs": 6})).unwrap();
        let f = FrontierIn::new(&c, &[r1, r2], 1);
        assert_eq!((f.max_pairs, f.max_jobs), (70, 6));
        assert_eq!(f.raised_by.as_deref(), Some("b"));
        assert!(raise_record("N", "c", "op", "", &json!({"max_pairs": 0, "max_jobs": 5})).is_err());
    }

    #[test]
    fn the_budget_drops_the_prefetch_first_then_stops() {
        let f = FrontierIn::new(&cfg(), &[], 1);
        assert_eq!(fit(&f, 2, (8, 1), (5, 1)), Fit::All);
        assert_eq!(fit(&f, 2, (9, 1), (5, 1)), Fit::FrontierOnly);
        assert_eq!(fit(&f, 2, (5, 2), (5, 1)), Fit::FrontierOnly);
        assert_eq!(fit(&f, 2, (9, 1), (9, 1)), Fit::Stop);
        assert_eq!(fit(&f, 0, (1, 2), (1, 2)), Fit::Stop);
    }

    #[test]
    fn pair_set_algebra() {
        let mut a = Pairs::new();
        a.entry((0, 1)).or_default().extend([vec![1], vec![2]]);
        let mut b = Pairs::new();
        b.entry((0, 1)).or_default().insert(vec![2]);
        b.entry((1, 0)).or_default().insert(vec![3]);
        assert_eq!(count(&minus(&a, &b)), 1);
        assert_eq!(count(&union(&a, &b)), 3);
        assert!(minus(&b, &union(&a, &b)).is_empty());
    }
}
