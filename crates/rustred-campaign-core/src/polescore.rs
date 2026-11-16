//! POLE SCORE and POLE OBJECTIVE of the generation choice and of improve mode (lane sizegen ITEM 2, director
//! 2026-11-16; plan/auto-campaign/POLES_OBJECTIVE.md; interfaces/genchoice.md section 6). PURE: no process, no file.
//!
//! SCORE ([`PoleScore`]) from the ROUTE-POLE CENSUS of `rr_poles census` (an existing tool, lane poles B4: the exact
//! order at d0 of the relation SERVING each key, evaluated at the key): per candidate TABLE the `box` section of its
//! `census.summary.json`; per candidate RULE its line of `rules.jsonl` (the keys it serves). A key whose order is
//! undecided makes the score UNDECIDED (`max_order` None): never read as zero. The COMPOSED per-terminal order (route
//! poles cancel on a regular terminal set, POLES_OBJECTIVE 2(A)/(B)) is not produced by the existing tools for whole
//! tables: it is recorded as undecided (`composed_max_order` None) and the selection reads the route (local) profile,
//! exactly as `poles::select::pole_objective` does whenever a composed profile is missing.
//!
//! OBJECTIVE ([`choose_poles`]): `poles::select::pole_objective` (THE ranking function of every pole choice) over the
//! ADMISSIBLE candidates only (an acceptable full table; a structurally valid replacement rule): a pole never changes a
//! verdict. Measures from [`PolePolicy`] (default `regular`, `max_pole_order`, `size`), the size guard `max_size_ratio`
//! (the SIZE TOLERANCE: a better pole class wins only while one of its members is <= ratio x the smallest admissible
//! size), then TIME (cpu ms), then the candidate index. `use: false` = poles IGNORED: the callers keep their size choice
//! untouched and write nothing about poles (bit-identical records).

use crate::economy::genchoice::FileEntry;
use crate::economy::improve::TableRule;
use crate::poles::{Candidate, MaxSizeRatio, PoleObjective, PoleProfile, pole_objective};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::collections::BTreeMap;

pub const SCORE_SCHEMA: &str = "rustred.campaign.pole-score.v1";
pub const DECISION_SCHEMA: &str = "rustred.campaign.pole-choice.v1";

/// The route-pole score of a candidate table (box keys) or rule (the keys it serves).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PoleScore {
    /// Keys evaluated.
    pub keys: u64,
    /// Keys whose serving relation has a pole at d0.
    pub keys_with_pole: u64,
    /// Keys whose order could not be decided.
    pub undecided: u64,
    /// Maximal route pole order; None = UNDECIDED (some key undecided): never zero.
    pub max_order: Option<u32>,
    /// Keys per pole order > 0 (table scope: the census' `keys_by_order`); empty when not stated (rule scope).
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub keys_by_order: BTreeMap<u32, u64>,
    /// Rules with a pole somewhere in the box (table scope).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rules_with_pole: Option<u64>,
    /// The composed per-terminal maximum after composition down to the terminals: None = not computed (undecided).
    #[serde(default)]
    pub composed_max_order: Option<u32>,
}

fn u(v: &Value, k: &str) -> Result<u64, String> {
    v.get(k)
        .and_then(Value::as_u64)
        .ok_or_else(|| format!("pole score: field {k:?} missing or not a count"))
}

impl PoleScore {
    pub fn decided(&self) -> bool {
        self.max_order.is_some()
    }

    /// The `box` section of a `rustred.campaign.route-pole-census.v1` summary.
    pub fn from_census_summary(v: &Value) -> Result<PoleScore, String> {
        if v.get("schema").and_then(Value::as_str) != Some("rustred.campaign.route-pole-census.v1")
        {
            return Err("pole score: not a route-pole census summary".into());
        }
        let b = v
            .get("box")
            .ok_or("pole score: census summary without a box section")?;
        let undecided = u(b, "undecided")?;
        let mut keys_by_order = BTreeMap::new();
        if let Some(m) = b.get("keys_by_order").and_then(Value::as_object) {
            for (k, n) in m {
                let o: u32 = k.parse().map_err(|_| format!("pole score: order {k:?}"))?;
                let n = n.as_u64().ok_or("pole score: keys_by_order count")?;
                if o > 0 && n > 0 {
                    keys_by_order.insert(o, n);
                }
            }
        }
        let max = u(b, "max_order")?;
        Ok(PoleScore {
            keys: u(b, "keys")?,
            keys_with_pole: u(b, "keys_with_pole")?,
            undecided,
            max_order: (undecided == 0).then(|| u32::try_from(max).unwrap_or(u32::MAX)),
            keys_by_order,
            rules_with_pole: b.get("rules_with_pole").and_then(Value::as_u64),
            composed_max_order: None,
        })
    }

    /// One line of the census' `rules.jsonl` (the keys the rule serves).
    pub fn from_census_rule(v: &Value) -> Result<PoleScore, String> {
        let undecided = u(v, "undecided")?;
        let max = u(v, "max_order")?;
        Ok(PoleScore {
            keys: u(v, "keys")?,
            keys_with_pole: u(v, "keys_with_pole")?,
            undecided,
            max_order: (undecided == 0).then(|| u32::try_from(max).unwrap_or(u32::MAX)),
            keys_by_order: BTreeMap::new(),
            rules_with_pole: None,
            composed_max_order: None,
        })
    }

    /// The local (route) profile handed to `pole_objective`: one entry per singular key when the distribution is
    /// known, else the maximum; empty for a regular score (the candidate's `pole_order` 0 then speaks).
    fn local(&self) -> Vec<u32> {
        match self.max_order {
            Some(0) | None => vec![],
            Some(m) if self.keys_by_order.is_empty() => vec![m],
            Some(_) => self
                .keys_by_order
                .iter()
                .flat_map(|(&o, &n)| std::iter::repeat_n(o, n as usize))
                .collect(),
        }
    }
}

/// One line of a pole-score file (input of a genchoice / improve config: `pole_scores`).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ScoreLine {
    pub schema: String,
    pub node: String,
    /// `table` (a whole candidate table, box keys) or `rule` (one rule at the keys the table's rule serves).
    pub scope: String,
    /// Table scope: the genchoice candidate id (`lines_rev-rung-std`); rule scope: `original` (the input table's rule
    /// at `rule_index`) or the improve candidate name.
    pub cand: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rule_index: Option<i64>,
    /// Rule scope, candidate: the candidate job's sealed table (path, sha256).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub table: Option<FileEntry>,
    pub score: PoleScore,
    /// Provenance (census summary / rules file and its sha256).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source: Option<String>,
}

pub fn parse_scores(text: &str) -> Result<Vec<ScoreLine>, String> {
    let mut out = vec![];
    for (n, l) in text.lines().enumerate() {
        if l.trim().is_empty() {
            continue;
        }
        let s: ScoreLine =
            serde_json::from_str(l).map_err(|e| format!("pole scores line {}: {e}", n + 1))?;
        if s.schema != SCORE_SCHEMA {
            return Err(format!("pole scores line {}: schema {:?}", n + 1, s.schema));
        }
        if s.scope != "table" && s.scope != "rule" {
            return Err(format!("pole scores line {}: scope {:?}", n + 1, s.scope));
        }
        if s.scope == "rule" && s.rule_index.is_none() {
            return Err(format!(
                "pole scores line {}: rule scope without rule_index",
                n + 1
            ));
        }
        out.push(s);
    }
    Ok(out)
}

/// The pole objective of a choice (config `policy.poles` / `improve.poles`).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, default)]
pub struct PolePolicy {
    /// false = poles IGNORED (the size choice exactly as before; nothing about poles is written).
    #[serde(rename = "use")]
    pub use_poles: bool,
    /// Lexicographic measures of `poles::select` (words: regular, max_pole_order, sum_pole_orders, singular_terms, size);
    /// time (cpu ms) and the candidate index break the remaining ties.
    pub measures: Vec<String>,
    /// SIZE TOLERANCE: `"unbounded"` or a rational >= 1 (`"2"`, `"5/2"`): a better pole class is rejected when every
    /// member is larger than ratio x the smallest admissible size.
    pub max_size_ratio: Value,
}

/// Default size tolerance, from the experiment of 2026-11-16 (SIZE-BLIND-RESULTS.md "POLE OBJECTIVE"; interfaces/genchoice.md
/// 6): the only regular-vs-singular choice met (T228 rule 1248: the regular rule is 2.8x the smaller singular
/// candidate) needs >= 2.8 to keep the regular rule; whole-table alternatives spread 2.2-5.6x (T228 32.3 / 14.9 MB,
/// T186 100.5 / 18.1 MB). 4 keeps the regular rule and rejects the 5.6x table. `"unbounded"` = the user's
/// "regular always wins" (POLES_OBJECTIVE 2(A)).
pub const DEFAULT_MAX_SIZE_RATIO: &str = "4";

impl Default for PolePolicy {
    fn default() -> Self {
        PolePolicy {
            use_poles: true,
            measures: vec!["regular".into(), "max_pole_order".into(), "size".into()],
            max_size_ratio: Value::String(DEFAULT_MAX_SIZE_RATIO.into()),
        }
    }
}

impl PolePolicy {
    pub fn is_ignored(&self) -> bool {
        !self.use_poles
    }
    /// The objective of `poles::select` (refuses unknown / repeated measures, a bad ratio).
    pub fn objective(&self) -> Result<PoleObjective, String> {
        PoleObjective::from_value(
            &json!({"measures": self.measures, "max_size_ratio": self.max_size_ratio}),
            MaxSizeRatio::Unbounded,
        )
        .map_err(|e| format!("poles policy: {}", e.0))
    }
    pub fn check(&self) -> Result<(), String> {
        self.objective().map(|_| ())
    }
}

/// One candidate of a pole choice.
#[derive(Clone, Debug, PartialEq)]
pub struct PoleCand {
    /// The caller's index (genchoice candidate index; 0 = old rule / 1 = new rule in improve).
    pub cand: usize,
    pub admissible: bool,
    /// The caller's size measure (use cost / bytes / monomials).
    pub size: u64,
    pub time_ms: u64,
    pub score: Option<PoleScore>,
}

/// The result of [`choose_poles`].
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct PoleDecision {
    pub winner: usize,
    pub size_winner: Option<usize>,
    /// The pole objective chose another candidate than the size choice.
    pub changed: bool,
    /// `regular` | `lowest_pole` | `size_guard` | `economy` | `only` | `no_decided_pole_score`.
    pub reason: String,
    /// `local` (route poles) | `none`.
    pub profile: String,
    pub measures: Vec<String>,
    pub max_size_ratio: String,
    /// The winner's measure values (u64::MAX = unknown).
    pub winner_values: Vec<u64>,
    /// Candidates of the best pole class rejected by the size guard.
    pub rejected_for_size: Vec<usize>,
    /// Admissible candidates without a decided score (ranked after every decided one).
    pub undecided: Vec<usize>,
}

fn tie_id(c: &PoleCand) -> String {
    format!("{:020}.{:06}", c.time_ms, c.cand)
}

/// THE pole choice (see the module doc). `Ok(None)`: poles ignored or no admissible candidate (the caller keeps its
/// size choice). Undecided scores are never read as zero: when no admissible candidate has a decided score the size
/// choice stands (`no_decided_pole_score`).
pub fn choose_poles(
    cands: &[PoleCand],
    size_winner: Option<usize>,
    pol: &PolePolicy,
) -> Result<Option<PoleDecision>, String> {
    if pol.is_ignored() {
        return Ok(None);
    }
    let obj = pol.objective()?;
    let adm: Vec<&PoleCand> = cands.iter().filter(|c| c.admissible).collect();
    if adm.is_empty() {
        return Ok(None);
    }
    let undecided: Vec<usize> = adm
        .iter()
        .filter(|c| !c.score.as_ref().is_some_and(PoleScore::decided))
        .map(|c| c.cand)
        .collect();
    let measures: Vec<String> = obj.measures.iter().map(|m| m.word().to_string()).collect();
    if undecided.len() == adm.len() {
        let w = size_winner
            .filter(|w| adm.iter().any(|c| c.cand == *w))
            .unwrap_or(adm[0].cand);
        return Ok(Some(PoleDecision {
            winner: w,
            size_winner,
            changed: false,
            reason: "no_decided_pole_score".into(),
            profile: "none".into(),
            measures,
            max_size_ratio: obj.max_size_ratio.text(),
            winner_values: vec![],
            rejected_for_size: vec![],
            undecided,
        }));
    }
    let sel: Vec<Candidate> = adm
        .iter()
        .map(|c| Candidate {
            id: tie_id(c),
            admissible: true,
            excluded_by: None,
            evidence_level: 0,
            size: c.size,
            pole_order: c.score.as_ref().and_then(|s| s.max_order).map(u64::from),
        })
        .collect();
    let profiles: Vec<PoleProfile> = adm
        .iter()
        .map(|c| PoleProfile {
            local: c.score.as_ref().map(PoleScore::local).unwrap_or_default(),
            composed: None,
        })
        .collect();
    let ch = pole_objective(&sel, &profiles, &obj, None)?
        .ok_or("pole objective: no admissible candidate")?;
    let winner = adm[ch.index].cand;
    let by_id: BTreeMap<String, usize> = adm.iter().map(|c| (tie_id(c), c.cand)).collect();
    // every admissible candidate decided and equal on the pole measures: the size (then time) decided
    let pv: Vec<Option<Vec<u64>>> = adm
        .iter()
        .map(|c| {
            c.score
                .as_ref()
                .filter(|s| s.decided())
                .map(|s| pole_values(s, pol))
        })
        .collect();
    let tied = adm.len() > 1 && pv.iter().all(|v| v.is_some() && *v == pv[0]);
    Ok(Some(PoleDecision {
        winner,
        size_winner,
        changed: size_winner.is_some_and(|w| w != winner),
        reason: if tied && ch.reason.word() != "size_guard" {
            "poles_tied_size_decided".into()
        } else {
            ch.reason.word().into()
        },
        profile: ch.profile_used.into(),
        measures: ch.measures.iter().map(|m| m.to_string()).collect(),
        max_size_ratio: ch.max_size_ratio,
        winner_values: ch.chosen_measures,
        rejected_for_size: ch
            .rejected_for_size
            .iter()
            .filter_map(|(id, _)| by_id.get(id).copied())
            .collect(),
        undecided,
    }))
}

/// The pole measures of a score other than `size` (lower is better), for "strictly better on poles" tests.
pub fn pole_values(score: &PoleScore, pol: &PolePolicy) -> Vec<u64> {
    let max = score.max_order.map_or(u64::MAX, u64::from);
    pol.measures
        .iter()
        .filter_map(|m| match m.as_str() {
            "regular" => Some(if score.decided() {
                u64::from(max > 0)
            } else {
                u64::MAX
            }),
            "max_pole_order" => Some(max),
            "sum_pole_orders" => Some(if !score.decided() {
                u64::MAX
            } else if score.keys_by_order.is_empty() {
                max
            } else {
                score
                    .keys_by_order
                    .iter()
                    .map(|(&o, &n)| u64::from(o) * n)
                    .sum()
            }),
            "singular_terms" => Some(if score.decided() {
                score.keys_with_pole
            } else {
                u64::MAX
            }),
            _ => None,
        })
        .collect()
}

/// IMPROVE under the pole objective: may the structurally admissible rule `new` replace `old`? `Ok(verdict)`:
/// `replaced_poles` (strictly better on the pole measures, within the size guard), `replaced_size` (equal on poles,
/// smaller under [`crate::economy::improve::replacement_verdict`]) or `replaced_size_guard` (worse on poles, but the old
/// rule's better pole class is beyond the size tolerance and the candidate passes the size rule); `Err(reason)`
/// otherwise. Both scores must be
/// decided (an undecided score never wins and never loses: `pole_undecided`). Size = monomials; old wins every tie.
pub fn rule_pole_verdict(
    old: &TableRule,
    old_score: Option<&PoleScore>,
    new: &TableRule,
    new_score: Option<&PoleScore>,
    min_gain: f64,
    pol: &PolePolicy,
) -> Result<String, String> {
    crate::economy::improve::structural_verdict(old, new)?;
    let (Some(os), Some(ns)) = (
        old_score.filter(|s| s.decided()),
        new_score.filter(|s| s.decided()),
    ) else {
        return Err(format!(
            "pole_undecided ({})",
            if old_score.is_some_and(PoleScore::decided) {
                "candidate"
            } else {
                "table rule"
            }
        ));
    };
    let (ov, nv) = (pole_values(os, pol), pole_values(ns, pol));
    let cands = [
        PoleCand {
            cand: 0,
            admissible: true,
            size: old.monomials,
            time_ms: 0,
            score: Some(os.clone()),
        },
        PoleCand {
            cand: 1,
            admissible: true,
            size: new.monomials,
            time_ms: 0,
            score: Some(ns.clone()),
        },
    ];
    let d = choose_poles(&cands, Some(0), pol)?.ok_or("pole objective: no decision")?;
    let tag = |s: &PoleScore| {
        format!(
            "max {} / {} keys",
            s.max_order.unwrap_or(0),
            s.keys_with_pole
        )
    };
    if d.winner != 1 {
        return Err(if !d.rejected_for_size.is_empty() {
            format!(
                "size_guard (poles {} vs {}, ratio {})",
                tag(ns),
                tag(os),
                d.max_size_ratio
            )
        } else {
            format!("poles_not_better ({} vs {})", tag(ns), tag(os))
        });
    }
    if nv < ov {
        return Ok("replaced_poles".into());
    }
    crate::economy::improve::replacement_verdict(old, new, min_gain)?;
    // equal poles: the size rule decided; worse poles: the old rule's better class exceeded the size tolerance
    Ok(if nv == ov {
        "replaced_size"
    } else {
        "replaced_size_guard"
    }
    .into())
}

/// The pole re-choice of a FINISHED generation choice (`choice.json` of schema genchoice-choice.v1) from table-scope
/// scores: every acceptable full table with its size (the choice's `select` measure), time and score; the decision.
/// `pol.use = false`: `Ok(None)`.
pub fn rechoice(
    choice: &Value,
    scores: &[ScoreLine],
    pol: &PolePolicy,
) -> Result<Option<Value>, String> {
    if pol.is_ignored() {
        return Ok(None);
    }
    let node = choice
        .get("node")
        .and_then(Value::as_str)
        .ok_or("choice: no node")?;
    let select = choice
        .get("select")
        .and_then(Value::as_str)
        .unwrap_or("use_cost");
    let cands = choice
        .get("candidates")
        .and_then(Value::as_array)
        .ok_or("choice: no candidates")?;
    let mut pc = vec![];
    let mut rows = vec![];
    for c in cands {
        let idx = c
            .get("index")
            .and_then(Value::as_u64)
            .ok_or("choice: candidate index")? as usize;
        let id = c
            .get("id")
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_string();
        for r in c
            .get("full")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
        {
            if r.get("acceptable").and_then(Value::as_bool) != Some(true) {
                continue;
            }
            let Some(m) = r.get("measure").filter(|m| m.is_object()) else {
                continue;
            };
            let size = match select {
                "bytes" => m
                    .get("table_bytes")
                    .and_then(Value::as_u64)
                    .unwrap_or(u64::MAX),
                "max_monomials" => m
                    .get("max_monomials")
                    .and_then(Value::as_u64)
                    .unwrap_or(u64::MAX),
                _ => m
                    .get("use_cost_ns")
                    .and_then(Value::as_f64)
                    .map_or(u64::MAX, |x| x.round() as u64),
            };
            let time = r.get("cpu_ms").and_then(Value::as_u64).unwrap_or(0);
            let score = scores
                .iter()
                .find(|s| s.scope == "table" && s.node == node && s.cand == id)
                .map(|s| s.score.clone());
            rows.push(
                json!({"cand": idx, "id": id, "size": size, "table_bytes": m.get("table_bytes"),
                "rules": m.get("rules"), "max_monomials": m.get("max_monomials"), "cpu_ms": time,
                "score": score.as_ref().map_or(Value::String("undecided".into()), |s| json!(s))}),
            );
            pc.push(PoleCand {
                cand: idx,
                admissible: true,
                size,
                time_ms: time,
                score,
            });
        }
    }
    let size_winner = choice
        .get("winner")
        .and_then(|w| w.get("cand").or(Some(w)))
        .and_then(|w| match w {
            Value::Number(n) => n.as_u64().map(|x| x as usize),
            Value::String(s) => cands
                .iter()
                .find(|c| c.get("id").and_then(Value::as_str) == Some(s))
                .and_then(|c| c.get("index").and_then(Value::as_u64))
                .map(|x| x as usize),
            _ => None,
        });
    let d = choose_poles(&pc, size_winner, pol)?;
    Ok(Some(
        json!({"schema": DECISION_SCHEMA, "node": node, "select": select, "policy": pol,
        "candidates": rows, "decision": d}),
    ))
}
