//! The flow of the IBP core (interfaces/coreseed.md 3 and 11), per support signature of the given keys:
//! CLOSE FIRST (POLES_OBJECTIVE.md 1, binding; audit r2-poles F2):
//! A. the RULE's explicit list at every key;
//! B. the ESCALATION steps for keys without a relation (shells, then shifted centres; one job per centre);
//! -> the CLOSE-FIRST REPORT (every key served or decided; full-list pieces) is PUBLISHED ([`CoreSolver::publish`])
//!    before any request of the later phases: whatever they do (budget, interruption, a request that never returns),
//!    nothing published is lost or changed; they only ADD alternatives and pieces.
//! VARIANTS (own budget `variants.max_requests`, resumable by the solver's journal like every request):
//! S. a CENTRE SCAN for keys whose relation is large (> `centre_scan.min_terms`) or not regular at d0 (mode
//!    `regular_preferred`), with the shells of the step that served them (TRICKS 13-15);
//! R. the REGULAR ESCALATION for keys still without a regular relation (mode `regular_preferred`);
//! L. per chosen rule: LISTS minimised at a pivot key (the minimiser, lane seedmin) and accepted on a domain only after
//!    the solve at EVERY key of it (no sampling); a failing key becomes the next pivot; keys left keep the full list.
//!    The pieces (rule, list, tested domain) form the SPLIT recipe of the core (the final report).
//! Every answer is recorded per key (relation / no_relation / not_in_domain / budget / failed); a key is
//! `no_relation_decided` only when every step decided NO_RELATION; any budget or failure leaves it `undecided`.
//! All certified relations are kept per key as ALTERNATIVES with their d0 verdict; `chosen` by the policy (regular
//! first in mode regular_preferred, then fewer right-hand-side terms, then fewer seeds, then the order found).

use std::collections::{BTreeMap, BTreeSet};

use serde::Serialize;
use serde_json::{Value, json};

use super::seeds::{
    Offset, dots_rank, face_domain, ordered, recipe_doc, resolve_centres, shell_offsets, signature,
    valid_at,
};
use super::spec::{CentreSpec, CoreSeedPolicy, StepSpec};
use super::{CoreSolver, D0Verdict, Key, KeyAnswer, RelationInfo, SolveReq};
use crate::seedmin::bridge::{MakeReq, minimise};

pub const REPORT_SCHEMA: &str = "rustred.campaign.coreseed-report.v1";

/// One answer of one rule / list at one key.
#[derive(Clone, Debug, Serialize)]
pub struct StepRecord {
    /// `rule` | `escalation` | `centre_scan` | `regular` | `domain_test` | `minimise`.
    pub phase: String,
    pub rule: String,
    /// `full` or the piece id of a minimised list.
    pub list: String,
    pub seeds: usize,
    pub outcome: String,
    pub detail: Option<String>,
    pub rhs_terms: Option<u64>,
    pub d0: Option<D0Verdict>,
    pub relation_digest: Option<String>,
}

/// A certified relation at a key.
#[derive(Clone, Debug, Serialize)]
pub struct Alternative {
    pub phase: String,
    pub rule: String,
    pub list: String,
    pub seeds: usize,
    pub seeds_at_key: Option<u64>,
    pub rhs_terms: u64,
    pub d0: D0Verdict,
    pub relation_digest: String,
    pub recipe_digest: String,
    pub record_sha256: String,
}

#[derive(Clone, Debug, Serialize)]
pub struct KeyReport {
    pub key: Key,
    pub dots_rank: i64,
    /// `relation` | `no_relation_decided` | `undecided` | `not_in_domain`.
    pub status: String,
    /// The phase / rule of the first certified relation (what mode `any_relation` stops at).
    pub first_solved_by: Option<String>,
    pub chosen: Option<usize>,
    pub alternatives: Vec<Alternative>,
    pub steps: Vec<StepRecord>,
}

#[derive(Clone, Debug, Serialize)]
pub struct RuleRecord {
    pub id: String,
    pub phase: String,
    /// Centre labels at the first key the rule was run at.
    pub centres: Vec<String>,
    pub shells: Vec<String>,
    /// The seed (row) order of the list (TRICKS 17 b: the relation depends on it; every size is stated with it).
    pub order: String,
    pub seeds: usize,
    pub list_sha256: String,
    pub keys_sent: usize,
    pub relation: usize,
    pub no_relation: usize,
    pub budget: usize,
    pub failed: usize,
}

/// A piece of the split recipe: a rule's list accepted on the keys where it was solved and certified.
#[derive(Clone, Debug, Serialize)]
pub struct Piece {
    pub id: String,
    pub rule: String,
    /// The seed (row) order of `list` (part of the recipe text and digest).
    pub order: String,
    /// `minimised` | `full_list`.
    pub kind: String,
    pub list: Vec<Offset>,
    pub list_sha256: String,
    pub domain: Vec<Key>,
    pub failed: Vec<Key>,
    /// The line-7 `minimise` member (+ `target_domain`, `domain_tested`), null for a full list.
    pub minimise: Value,
    /// Cost only (not part of a decision).
    pub minimise_cost: Value,
    pub d0: BTreeMap<String, usize>,
    /// Slots constant on the piece's keys but not on the signature's keys (a locus hint for the repair flow).
    pub fixed_in_piece: BTreeMap<usize, i32>,
}

#[derive(Clone, Debug, Serialize)]
pub struct GroupReport {
    pub dotted: Vec<usize>,
    pub numerators: Vec<usize>,
    pub keys: usize,
    pub domain: Value,
    pub rules: Vec<RuleRecord>,
    pub pieces: Vec<Piece>,
}

#[derive(Clone, Debug, Serialize)]
pub struct CoreReport {
    pub schema: String,
    /// `close_first` (published after A + B, before any variant or list request) | `final` (after S, R, L).
    pub stage: String,
    pub policy_digest: String,
    pub mode: String,
    pub d0: String,
    pub groups: Vec<GroupReport>,
    pub keys: Vec<KeyReport>,
    pub totals: Value,
}

/// A seed rule: base centres (all resolved) + extra centres (resolved, filtered to one label when `label`), shells.
#[derive(Clone, Debug)]
struct Rule {
    id: String,
    phase: String,
    base: Vec<CentreSpec>,
    extra: Vec<CentreSpec>,
    label: Option<String>,
    shells: Vec<String>,
}

impl Rule {
    fn centres_at(&self, key: &[i32], lines: &[bool], lowest: &[i32]) -> Vec<(String, Offset)> {
        let mut c = resolve_centres(&self.base, key, lines, lowest);
        for (l, o) in resolve_centres(&self.extra, key, lines, lowest) {
            if self.label.as_ref().is_none_or(|x| *x == l) && !c.iter().any(|(_, y)| *y == o) {
                c.push((l, o));
            }
        }
        c
    }
    fn applies(&self, key: &[i32], lines: &[bool], lowest: &[i32]) -> bool {
        match &self.label {
            None => true,
            Some(l) => resolve_centres(&self.extra, key, lines, lowest)
                .iter()
                .any(|(x, _)| x == l),
        }
    }
    /// The explicit list at `keys` (generation order, first occurrence wins, valid at some key), in `order`.
    fn list(
        &self,
        keys: &[Key],
        lines: &[bool],
        lowest: &[i32],
        order: &str,
    ) -> Result<Vec<Offset>, String> {
        let mut seen: BTreeSet<Offset> = BTreeSet::new();
        let mut out = vec![];
        for k in keys {
            for (_, c) in self.centres_at(k, lines, lowest) {
                for o in shell_offsets(&c, lines, &self.shells)? {
                    if !seen.contains(&o) && keys.iter().any(|x| valid_at(x, &o, lines)) {
                        seen.insert(o.clone());
                        out.push(o);
                    }
                }
            }
        }
        Ok(ordered(&out, order))
    }
}

fn sha256_list(list: &[Offset]) -> String {
    let mut h = crate::closure::sha256::Sha256::new();
    for o in list {
        h.update(format!("{o:?}\n").as_bytes());
    }
    h.finish_hex()
}

#[derive(Default)]
struct KeyState {
    steps: Vec<StepRecord>,
    alts: Vec<Alternative>,
}

/// int7 (wire-in B1): THE selection among a key's certified alternatives is `poles::pole_objective` (one function for
/// every choice). Mode regular_preferred: the user's default objective (eps box excess, max pole order, sum of pole
/// orders, singular terms, size; max_size_ratio unbounded) on each alternative's d0 verdict (regular = order 0,
/// singular of known order p = p, singular of unknown order / undecided = unknown, ranked worst); otherwise economy first
/// (size, then regularity / pole order). Ties: fewer seeds, then the relation digest (the candidate id), independent of
/// the order the alternatives were found in. All alternatives are certified: the choice never decides acceptance.
fn choose(alts: &[Alternative], regular_first: bool) -> Option<usize> {
    use crate::poles::params::CriteriaOrder;
    use crate::poles::{Candidate, MaxSizeRatio, PoleObjective, pole_objective};
    let cands: Vec<Candidate> = alts
        .iter()
        .map(|a| Candidate {
            id: format!("{:010}/{}", a.seeds, a.relation_digest),
            admissible: true,
            excluded_by: None,
            evidence_level: 0,
            size: a.rhs_terms,
            pole_order: d0_order(&a.d0),
        })
        .collect();
    let obj = if regular_first {
        PoleObjective::default()
    } else {
        PoleObjective::from_criteria(MaxSizeRatio::Unbounded, CriteriaOrder::EconomyFirst)
    };
    pole_objective(&cands, &[], &obj, None)
        .ok()
        .flatten()
        .map(|c| c.index)
}

/// The pole order a d0 verdict gives the objective: regular 0, singular of known order p, else unknown (`None`).
fn d0_order(v: &D0Verdict) -> Option<u64> {
    match v {
        D0Verdict::Regular => Some(0),
        D0Verdict::Singular { order: Some(p) } => Some(u64::from(*p)),
        D0Verdict::Singular { order: None } | D0Verdict::Undecided => None,
    }
}

/// Lane polefix (audit r2-poles F11): is `new` (a minimised list's relation at a key) NOT WORSE than `old` (the
/// full list's) under `poles::pole_objective`'s pole measures (the user's default objective without the size: max pole
/// order, sum, singular terms; an unknown order ranks worst)? Ties keep `new`. The domain test of a list refuses a key
/// whose relation got worse (mode regular_preferred): no private ranking.
fn poles_not_worse(old: &D0Verdict, new: &D0Verdict) -> bool {
    use crate::poles::{Candidate, Measure, PoleObjective, pole_objective};
    let obj = PoleObjective {
        measures: PoleObjective::default()
            .measures
            .into_iter()
            .filter(|m| *m != Measure::Size)
            .collect(),
        ..PoleObjective::default()
    };
    let c = |id: &str, v: &D0Verdict| Candidate {
        id: id.into(),
        admissible: true,
        excluded_by: None,
        evidence_level: 0,
        size: 0,
        pole_order: d0_order(v),
    };
    // ids order the tie in favour of `new`
    let cands = [c("0-new", new), c("1-old", old)];
    matches!(pole_objective(&cands, &[], &obj, None), Ok(Some(ch)) if ch.index == 0)
}

struct Group<'a> {
    policy: &'a CoreSeedPolicy,
    lines: &'a [bool],
    sig: (Vec<usize>, Vec<usize>),
    keys: Vec<Key>,
    lowest: Vec<i32>,
    domain: Value,
    tag: String,
    state: BTreeMap<Key, KeyState>,
    /// (list sha256, key) -> answer: a list is never solved twice at a key.
    cache: BTreeMap<(String, Key), KeyAnswer>,
    rules: Vec<RuleRecord>,
    pieces: Vec<Piece>,
    /// The rule (and its run list) of every alternative, by rule id.
    rule_lists: BTreeMap<String, (Rule, Vec<Offset>)>,
    scan_jobs: usize,
    requests: u64,
    /// Requests of the variant phase (S + R) so far: its own budget (`variants.max_requests`).
    variant_requests: u64,
}

impl<'a> Group<'a> {
    /// What is left of the variant budget (`None` = no budget of its own).
    fn variant_budget_left(&self) -> Option<u64> {
        self.policy
            .variants
            .max_requests
            .map(|m| m.saturating_sub(self.variant_requests))
    }

    /// Cap a variant rule set by the budget; keys of `need` left without any rule record a `budget` step.
    fn cap_variants(&mut self, phase: &str, rules: &mut Vec<Rule>, need: &[Key]) {
        let Some(left) = self.variant_budget_left() else {
            return;
        };
        let left = usize::try_from(left).unwrap_or(usize::MAX);
        if rules.len() <= left {
            return;
        }
        rules.truncate(left);
        let max = self.policy.variants.max_requests.unwrap_or(0);
        for k in need {
            if !rules.iter().any(|r| r.applies(k, self.lines, &self.lowest)) {
                let a = KeyAnswer::Budget {
                    what: format!("variant budget: variants.max_requests {max} spent"),
                };
                self.record(k, phase, "variant_budget", "none", 0, &a);
            }
        }
    }

    /// Run variant rules (S or R) under the variant budget, counting their requests.
    fn run_variant_rules(
        &mut self,
        solver: &mut dyn CoreSolver,
        rules: &[Rule],
        keys: &[Key],
    ) -> Result<(), String> {
        let before = self.requests;
        let r = self.run_rules(solver, rules, keys);
        self.variant_requests += self.requests - before;
        r
    }

    fn regular_first(&self) -> bool {
        self.policy.mode == "regular_preferred"
    }

    fn make_req(
        &self,
        rule: &Rule,
    ) -> impl Fn(&[Offset], &[Key], &str, Option<Value>) -> Result<SolveReq, String> + '_ {
        let rid = rule.id.clone();
        let shells = rule.shells.clone();
        move |list: &[Offset], keys: &[Key], label: &str, eng: Option<Value>| {
            let id = format!("coreseed/{}/{rid}/{label}", self.tag);
            let prov = json!({"lane": "coreseed", "rule": rid, "shells_harder_unit": shells,
                              "form": "explicit offsets from the key: one centre per seed, shell S0, no floor",
                              "order": self.policy.minimise.order});
            let rr = self.policy.relation_rule();
            let (recipe, text) = recipe_doc(&id, self.lines, &self.domain, list, rr, prov, eng)?;
            Ok(SolveReq {
                label: id,
                recipe,
                text,
                keys: keys.to_vec(),
                relation_rule: rr.to_string(),
            })
        }
    }

    fn best(&self, k: &Key) -> Option<&Alternative> {
        let s = self.state.get(k)?;
        choose(&s.alts, self.regular_first()).map(|i| &s.alts[i])
    }

    fn has_relation(&self, k: &Key) -> bool {
        self.state.get(k).is_some_and(|s| !s.alts.is_empty())
    }

    fn record(
        &mut self,
        k: &Key,
        phase: &str,
        rule: &str,
        list: &str,
        seeds: usize,
        a: &KeyAnswer,
    ) {
        let (detail, rhs, d0, dig) = match a {
            KeyAnswer::Relation(r) => (
                None,
                Some(r.rhs_count),
                Some(r.d0.clone()),
                Some(r.relation_digest.clone()),
            ),
            KeyAnswer::Budget { what } => (Some(what.clone()), None, None, None),
            KeyAnswer::Failed { class, message } => {
                (Some(format!("{class}: {message}")), None, None, None)
            }
            _ => (None, None, None, None),
        };
        let st = self.state.entry(k.clone()).or_default();
        st.steps.push(StepRecord {
            phase: phase.into(),
            rule: rule.into(),
            list: list.into(),
            seeds,
            outcome: a.word().into(),
            detail,
            rhs_terms: rhs,
            d0: d0.clone(),
            relation_digest: dig,
        });
        if let KeyAnswer::Relation(r) = a
            && !st
                .alts
                .iter()
                .any(|x| x.relation_digest == r.relation_digest)
        {
            st.alts.push(Alternative {
                phase: phase.into(),
                rule: rule.into(),
                list: list.into(),
                seeds,
                seeds_at_key: r.seeds_total,
                rhs_terms: r.rhs_count,
                d0: r.d0.clone(),
                relation_digest: r.relation_digest.clone(),
                recipe_digest: r.recipe_digest.clone(),
                record_sha256: r.record_sha256.clone(),
            });
        }
    }

    /// Run several rules (one request each, one batch) at the keys where each applies; record every answer.
    fn run_rules(
        &mut self,
        solver: &mut dyn CoreSolver,
        rules: &[Rule],
        keys: &[Key],
    ) -> Result<(), String> {
        let mut reqs: Vec<SolveReq> = vec![];
        let mut meta: Vec<(usize, Vec<Offset>, Vec<Key>)> = vec![];
        for (ri, r) in rules.iter().enumerate() {
            let ks: Vec<Key> = keys
                .iter()
                .filter(|k| r.applies(k, self.lines, &self.lowest))
                .cloned()
                .collect();
            if ks.is_empty() {
                continue;
            }
            let list = r.list(&ks, self.lines, &self.lowest, &self.policy.minimise.order)?;
            let lsha = sha256_list(&list);
            let centres: Vec<String> = r
                .centres_at(&ks[0], self.lines, &self.lowest)
                .into_iter()
                .map(|x| x.0)
                .collect();
            self.rules.push(RuleRecord {
                id: r.id.clone(),
                phase: r.phase.clone(),
                centres,
                shells: r.shells.clone(),
                order: self.policy.minimise.order.clone(),
                seeds: list.len(),
                list_sha256: lsha.clone(),
                keys_sent: ks.len(),
                relation: 0,
                no_relation: 0,
                budget: 0,
                failed: 0,
            });
            self.rule_lists
                .insert(r.id.clone(), (r.clone(), list.clone()));
            if list.len() > self.policy.caps.max_seeds {
                for k in &ks {
                    let a = KeyAnswer::Budget {
                        what: format!(
                            "over_seed_cap {} > {}",
                            list.len(),
                            self.policy.caps.max_seeds
                        ),
                    };
                    self.record(k, &r.phase, &r.id, "full", list.len(), &a);
                }
                continue;
            }
            let todo: Vec<Key> = ks
                .iter()
                .filter(|k| !self.cache.contains_key(&(lsha.clone(), (*k).clone())))
                .cloned()
                .collect();
            if !todo.is_empty() {
                let mk = self.make_req(r);
                reqs.push(mk(&list, &todo, "full", None)?);
                meta.push((ri, list.clone(), todo));
            }
            // keys answered before by the same list (another rule with the same seeds): recorded from the cache
            let cached: Vec<(Key, KeyAnswer)> = ks
                .iter()
                .filter_map(|k| {
                    self.cache
                        .get(&(lsha.clone(), k.clone()))
                        .map(|a| (k.clone(), a.clone()))
                })
                .collect();
            for (k, a) in cached {
                self.record(&k, &r.phase, &r.id, "full", list.len(), &a);
            }
        }
        if reqs.is_empty() {
            return Ok(());
        }
        self.requests += reqs.len() as u64;
        let ans = solver.solve(&reqs)?;
        for ((ri, list, todo), a) in meta.into_iter().zip(ans) {
            let r = &rules[ri];
            let lsha = sha256_list(&list);
            for (k, x) in todo.iter().zip(a) {
                self.cache.insert((lsha.clone(), k.clone()), x.clone());
                self.record(k, &r.phase, &r.id, "full", list.len(), &x);
                if let Some(rr) = self.rules.iter_mut().rev().find(|y| y.id == r.id) {
                    match &x {
                        KeyAnswer::Relation(_) => rr.relation += 1,
                        KeyAnswer::NoRelation => rr.no_relation += 1,
                        KeyAnswer::Budget { .. } => rr.budget += 1,
                        _ => rr.failed += 1,
                    }
                }
            }
        }
        Ok(())
    }

    /// Rules of one step for `keys` (one rule, or one per resolved centre label). The variant phases (centre scan,
    /// regular escalation) run once per shell set, so their ids carry the shells.
    fn step_rules(
        &self,
        step: &StepSpec,
        phase: &str,
        shells: &[String],
        keys: &[Key],
    ) -> Vec<Rule> {
        let at = if phase == "escalation" {
            String::new()
        } else {
            format!("@{}", shells.join("+"))
        };
        let base = if step.centres.is_empty() || step.keep_base_centre {
            self.policy.rule.centres.clone()
        } else {
            vec![]
        };
        if step.centres.is_empty() || !step.per_centre {
            return vec![Rule {
                id: format!("{phase}:{}{at}", step.id),
                phase: phase.into(),
                base,
                extra: step.centres.clone(),
                label: None,
                shells: shells.to_vec(),
            }];
        }
        let mut labels: Vec<String> = vec![];
        for k in keys {
            for (l, _) in resolve_centres(&step.centres, k, self.lines, &self.lowest) {
                if !labels.contains(&l) {
                    labels.push(l);
                }
            }
        }
        labels
            .into_iter()
            .map(|l| Rule {
                id: format!("{phase}:{}:{l}{at}", step.id),
                phase: phase.into(),
                base: base.clone(),
                extra: step.centres.clone(),
                label: Some(l),
                shells: shells.to_vec(),
            })
            .collect()
    }

    /// The centre scan (S) for `keys` just solved with `shells`: keys whose best relation is large or not regular.
    fn centre_scan(
        &mut self,
        solver: &mut dyn CoreSolver,
        keys: &[Key],
        shells: &[String],
    ) -> Result<(), String> {
        let cs = &self.policy.centre_scan;
        if !cs.enabled {
            return Ok(());
        }
        let regular_first = self.regular_first();
        let need: Vec<Key> = keys
            .iter()
            .filter(|k| {
                self.best(k).is_some_and(|b| {
                    b.rhs_terms > cs.min_terms
                        || (cs.on_singular && regular_first && !b.d0.is_regular())
                })
            })
            .cloned()
            .collect();
        if need.is_empty() {
            return Ok(());
        }
        let step = StepSpec {
            id: "scan".into(),
            add_shells: vec![],
            centres: cs.centres.clone(),
            keep_base_centre: cs.keep_base_centre,
            per_centre: true,
            cumulative: true,
        };
        let mut rules = self.step_rules(&step, "centre_scan", shells, &need);
        let left = cs.max_jobs_per_face.saturating_sub(self.scan_jobs);
        rules.truncate(left);
        self.cap_variants("centre_scan", &mut rules, &need);
        self.scan_jobs += rules.len();
        // per key: only its first `max_candidates_per_key` labels (in scan order)
        let per_key = cs.max_candidates_per_key;
        let mut sent: BTreeMap<String, Vec<Key>> = BTreeMap::new();
        for k in &need {
            let mut n = 0;
            for r in &rules {
                if n >= per_key {
                    break;
                }
                if r.applies(k, self.lines, &self.lowest) {
                    sent.entry(r.id.clone()).or_default().push(k.clone());
                    n += 1;
                }
            }
        }
        for r in &rules {
            if let Some(ks) = sent.get(&r.id) {
                let ks = ks.clone();
                self.run_variant_rules(solver, std::slice::from_ref(r), &ks)?;
            }
        }
        Ok(())
    }

    /// The regular escalation (R) for keys whose best relation is not regular (mode regular_preferred).
    fn regular_escalation(
        &mut self,
        solver: &mut dyn CoreSolver,
        keys: &[Key],
        shells: &[String],
    ) -> Result<(), String> {
        if !self.regular_first() {
            return Ok(());
        }
        for step in self.policy.regular_escalation.clone() {
            let need: Vec<Key> = keys
                .iter()
                .filter(|k| self.best(k).is_some_and(|b| !b.d0.is_regular()))
                .cloned()
                .collect();
            if need.is_empty() {
                break;
            }
            let mut sh = shells.to_vec();
            for s in &step.add_shells {
                if !sh.contains(s) {
                    sh.push(s.clone());
                }
            }
            let mut rules = self.step_rules(&step, "regular", &sh, &need);
            self.cap_variants("regular", &mut rules, &need);
            self.run_variant_rules(solver, &rules, &need)?;
        }
        Ok(())
    }

    /// Lists + domain test (L) for the keys whose chosen alternative comes from `rule_id`'s full list.
    fn lists_for_rule(
        &mut self,
        solver: &mut dyn CoreSolver,
        rule_id: &str,
        keys: &[Key],
    ) -> Result<(), String> {
        let Some((rule, full)) = self.rule_lists.get(rule_id).cloned() else {
            return Ok(());
        };
        let lsha = sha256_list(&full);
        let mut remaining: Vec<Key> = keys.to_vec();
        let mut n_lists = 0;
        let regular_first = self.regular_first();
        while !remaining.is_empty()
            && n_lists < self.policy.domain.max_lists
            && self.policy.minimise.mode != "off"
        {
            let pivot = self.pivot(&remaining);
            let known: BTreeMap<Key, RelationInfo> =
                match self.cache.get(&(lsha.clone(), pivot.clone())) {
                    Some(KeyAnswer::Relation(r)) => {
                        [(pivot.clone(), (**r).clone())].into_iter().collect()
                    }
                    _ => BTreeMap::new(),
                };
            let label = format!("min{n_lists}");
            let mk = self.make_req(&rule);
            let mk_dyn: &MakeReq<'_> = &mk;
            let res = minimise(
                solver,
                self.policy,
                mk_dyn,
                &full,
                std::slice::from_ref(&pivot),
                Some(&known),
                &label,
            )?;
            drop(mk);
            n_lists += 1;
            let Some(res) = res else {
                // the pivot is not certified by the full list any more (budget at a re-solve): it keeps the full list
                remaining.retain(|k| *k != pivot);
                self.pieces
                    .push(self.full_piece(&rule, &full, std::slice::from_ref(&pivot)));
                continue;
            };
            let piece_id = format!("{}#{}", rule.id, self.pieces.len());
            let test: Vec<Key> = remaining.iter().filter(|k| **k != pivot).cloned().collect();
            let mut answers: BTreeMap<Key, KeyAnswer> = BTreeMap::new();
            if !test.is_empty() {
                let mk = self.make_req(&rule);
                let req = mk(&res.list, &test, &format!("{label}/domain"), None)?;
                drop(mk);
                self.requests += 1;
                let a = solver.solve(std::slice::from_ref(&req))?;
                for (k, x) in test.iter().zip(a.into_iter().next().unwrap_or_default()) {
                    answers.insert(k.clone(), x);
                }
            }
            let mut domain = vec![pivot.clone()];
            let mut failed = vec![];
            let pr = res.per_key.get(&pivot).cloned();
            if let Some(r) = &pr {
                self.record(
                    &pivot,
                    "minimise",
                    &rule.id,
                    &piece_id,
                    res.list.len(),
                    &KeyAnswer::Relation(Box::new(r.clone())),
                );
            }
            for k in &test {
                let a = answers.remove(k).unwrap_or(KeyAnswer::Failed {
                    class: "missing".into(),
                    message: "no answer".into(),
                });
                // in mode regular_preferred a key's relation must not get WORSE under the pole objective than its
                // full-list relation (regular stays regular; a known order does not rise; F11: pole_objective)
                let was = match self.cache.get(&(lsha.clone(), k.clone())) {
                    Some(KeyAnswer::Relation(r)) => Some(r.d0.clone()),
                    _ => None,
                };
                let ok = match (&a, &was) {
                    (KeyAnswer::Relation(r), Some(w)) => {
                        !regular_first || poles_not_worse(w, &r.d0)
                    }
                    (KeyAnswer::Relation(_), None) => true,
                    _ => false,
                };
                self.record(k, "domain_test", &rule.id, &piece_id, res.list.len(), &a);
                if ok {
                    domain.push(k.clone());
                } else {
                    failed.push(k.clone());
                }
            }
            let mut member = res.member.clone();
            member["target_domain"] = self.domain.clone();
            let td = {
                let mut h = crate::closure::sha256::Sha256::new();
                h.update(
                    crate::schema::canon::canonical_json(&self.domain, false)
                        .unwrap_or_default()
                        .as_bytes(),
                );
                h.finish_hex()
            };
            member["target_domain_sha256"] = json!(td);
            member["domain_tested"] = json!({"keys": test.len() + 1, "solved": domain.len(),
                "failed": failed, "layers": "the given core keys"});
            let d0 = self.d0_counts(&domain, &rule.id, &piece_id);
            let fixed = self.fixed_in(&domain);
            self.pieces.push(Piece {
                id: piece_id,
                rule: rule.id.clone(),
                order: self.policy.minimise.order.clone(),
                kind: "minimised".into(),
                list_sha256: sha256_list(&res.list),
                list: res.list,
                domain,
                failed: failed.clone(),
                minimise: member,
                minimise_cost: res.cost,
                d0,
                fixed_in_piece: fixed,
            });
            remaining = failed;
        }
        if !remaining.is_empty() {
            let p = self.full_piece(&rule, &full, &remaining);
            self.pieces.push(p);
        }
        Ok(())
    }

    fn full_piece(&self, rule: &Rule, full: &[Offset], keys: &[Key]) -> Piece {
        self.full_piece_n(rule, full, keys, self.pieces.len())
    }

    fn full_piece_n(&self, rule: &Rule, full: &[Offset], keys: &[Key], n: usize) -> Piece {
        let id = format!("{}#{n}", rule.id);
        Piece {
            d0: self.d0_counts(keys, &rule.id, "full"),
            fixed_in_piece: self.fixed_in(keys),
            id,
            rule: rule.id.clone(),
            order: self.policy.minimise.order.clone(),
            kind: "full_list".into(),
            list: full.to_vec(),
            list_sha256: sha256_list(full),
            domain: keys.to_vec(),
            failed: vec![],
            minimise: Value::Null,
            minimise_cost: Value::Null,
        }
    }

    /// d0 verdicts of the piece's relations (the alternative of each key from `rule` / `list`).
    fn d0_counts(&self, keys: &[Key], rule: &str, list: &str) -> BTreeMap<String, usize> {
        let mut c: BTreeMap<String, usize> = BTreeMap::new();
        for k in keys {
            let v = self.state.get(k).and_then(|s| {
                s.alts
                    .iter()
                    .find(|a| a.rule == rule && a.list == list)
                    .map(|a| a.d0.word().to_string())
            });
            *c.entry(v.unwrap_or_else(|| "none".into())).or_default() += 1;
        }
        c
    }

    fn fixed_in(&self, keys: &[Key]) -> BTreeMap<usize, i32> {
        let mut out = BTreeMap::new();
        let Some(f) = keys.first() else { return out };
        for s in 0..f.len() {
            let on_piece = keys.iter().all(|k| k[s] == f[s]);
            let on_group = self.keys.iter().all(|k| k[s] == f[s]);
            if on_piece && !on_group {
                out.insert(s, f[s]);
            }
        }
        out
    }

    fn pivot(&self, keys: &[Key]) -> Key {
        match self.policy.domain.pivot.as_str() {
            "first" => keys[0].clone(),
            "lowest_dr" => keys
                .iter()
                .min_by_key(|k| (dots_rank(k, self.lines), (*k).clone()))
                .cloned()
                .unwrap_or_default(),
            _ => keys
                .iter()
                .max_by_key(|k| (dots_rank(k, self.lines), (*k).clone()))
                .cloned()
                .unwrap_or_default(),
        }
    }

    /// The keys of each rule whose alternative is chosen now, in the order the rules were run (full lists only).
    fn chosen_full_by_rule(&self) -> Vec<(String, Vec<Key>)> {
        let regular_first = self.regular_first();
        let mut by_rule: BTreeMap<String, Vec<Key>> = BTreeMap::new();
        for k in &self.keys {
            if let Some(i) = self
                .state
                .get(k)
                .and_then(|s| choose(&s.alts, regular_first))
            {
                let a = &self.state[k].alts[i];
                if a.list == "full" {
                    by_rule.entry(a.rule.clone()).or_default().push(k.clone());
                }
            }
        }
        let mut out = vec![];
        let mut done: BTreeSet<String> = BTreeSet::new();
        for r in &self.rules {
            if done.insert(r.id.clone())
                && let Some(ks) = by_rule.remove(&r.id)
            {
                out.push((r.id.clone(), ks));
            }
        }
        out
    }

    /// The close-first pieces: per rule of a chosen relation, its FULL list on the keys it serves (no minimisation:
    /// lists come after the variant search; a full-list piece is a valid split recipe on its keys).
    fn close_first_pieces(&self) -> Vec<Piece> {
        let mut out = vec![];
        for (rid, ks) in self.chosen_full_by_rule() {
            if let Some((rule, full)) = self.rule_lists.get(&rid) {
                let p = self.full_piece_n(rule, full, &ks, out.len());
                out.push(p);
            }
        }
        out
    }

    /// The report of every key of the group as it stands (not consuming the state).
    fn key_reports(&self) -> Vec<KeyReport> {
        let regular_first = self.regular_first();
        self.keys
            .iter()
            .map(|k| match self.state.get(k) {
                Some(st) => key_report(k, st, self.lines, regular_first),
                None => key_report(k, &KeyState::default(), self.lines, regular_first),
            })
            .collect()
    }

    fn group_report(&self, pieces: Vec<Piece>) -> GroupReport {
        GroupReport {
            dotted: self.sig.0.clone(),
            numerators: self.sig.1.clone(),
            keys: self.keys.len(),
            domain: self.domain.clone(),
            rules: self.rules.clone(),
            pieces,
        }
    }

    fn partial(&self) -> Value {
        let mut c: BTreeMap<&str, usize> = BTreeMap::new();
        for k in &self.keys {
            let s = if self.has_relation(k) {
                "relation"
            } else {
                "open"
            };
            *c.entry(s).or_default() += 1;
        }
        json!({"group": self.tag, "keys": self.keys.len(), "counts": c, "requests": self.requests})
    }
}

/// The report of one key from its state.
fn key_report(k: &Key, st: &KeyState, lines: &[bool], regular_first: bool) -> KeyReport {
    let decided_none = !st.steps.is_empty() && st.steps.iter().all(|s| s.outcome == "no_relation");
    let nid = !st.steps.is_empty() && st.steps.iter().all(|s| s.outcome == "not_in_domain");
    let status = if !st.alts.is_empty() {
        "relation"
    } else if decided_none {
        "no_relation_decided"
    } else if nid {
        "not_in_domain"
    } else {
        "undecided"
    };
    let first = st
        .steps
        .iter()
        .find(|s| s.outcome == "relation")
        .map(|s| format!("{}:{}", s.phase, s.rule));
    KeyReport {
        key: k.clone(),
        dots_rank: dots_rank(k, lines),
        status: status.into(),
        first_solved_by: first,
        chosen: choose(&st.alts, regular_first),
        alternatives: st.alts.clone(),
        steps: st.steps.clone(),
    }
}

/// Assemble a report (`stage` `close_first` | `final`).
fn assemble(
    policy: &CoreSeedPolicy,
    stage: &str,
    groups: Vec<GroupReport>,
    keys: Vec<KeyReport>,
    requests: u64,
) -> CoreReport {
    let mut status: BTreeMap<String, usize> = BTreeMap::new();
    let mut first: BTreeMap<String, usize> = BTreeMap::new();
    let mut chosen_d0: BTreeMap<String, usize> = BTreeMap::new();
    for k in &keys {
        *status.entry(k.status.clone()).or_default() += 1;
        if let Some(f) = &k.first_solved_by {
            *first.entry(f.clone()).or_default() += 1;
        }
        if let Some(i) = k.chosen {
            *chosen_d0
                .entry(k.alternatives[i].d0.word().into())
                .or_default() += 1;
        }
    }
    let totals = json!({"keys": keys.len(), "status": status, "first_solved_by": first,
        "chosen_d0": chosen_d0, "requests": requests});
    CoreReport {
        schema: REPORT_SCHEMA.into(),
        stage: stage.into(),
        policy_digest: policy.digest(),
        mode: policy.mode.clone(),
        d0: policy.d0.clone(),
        groups,
        keys,
        totals,
    }
}

/// Solve the IBP core given by `keys` (any order; duplicates ignored) under `policy` (interfaces/coreseed.md 3, 11).
///
/// CLOSE FIRST (audit r2-poles F2): A and B run for EVERY group; then the close-first report (stage `close_first`: every
/// key served or decided, full-list pieces) goes to [`CoreSolver::publish`] BEFORE any variant or list request. Then
/// the variant phase (S + R, own budget `variants.max_requests`) and the lists (L) run and the FINAL report (stage
/// `final`) is returned; they only add alternatives and pieces: every key's status, `first_solved_by` and close-first
/// alternatives are those published. An error of the later phases is returned AFTER the publication (a caller that
/// persisted the published report loses nothing); a restart replays the journal: the same publication, then the later
/// phases continue.
pub fn solve_core(
    keys: &[Key],
    lines: &[bool],
    policy: &CoreSeedPolicy,
    solver: &mut dyn CoreSolver,
) -> Result<CoreReport, String> {
    policy.validate()?;
    let n = lines.len();
    let mut ks: Vec<Key> = keys.to_vec();
    ks.sort();
    ks.dedup();
    if let Some(k) = ks
        .iter()
        .find(|k| k.len() != n || !valid_at(k, &vec![0; n], lines))
    {
        return Err(format!("key {k:?} is not a top-sector key of the chart"));
    }
    let mut groups: BTreeMap<(Vec<usize>, Vec<usize>), Vec<Key>> = BTreeMap::new();
    for k in &ks {
        groups
            .entry(signature(k, lines))
            .or_default()
            .push(k.clone());
    }
    // ---- CLOSE FIRST: A + B for every group (a certified relation serves its key at once, singular at d0 or not) ----
    let mut gs: Vec<Group<'_>> = vec![];
    for (gi, (sig, gkeys)) in groups.into_iter().enumerate() {
        let lowest: Vec<i32> = (0..n)
            .map(|s| gkeys.iter().map(|k| k[s]).min().unwrap_or(0))
            .collect();
        let tag = format!(
            "g{gi}-L{}-N{}",
            sig.0
                .iter()
                .map(|x| x.to_string())
                .collect::<Vec<_>>()
                .join("."),
            sig.1
                .iter()
                .map(|x| x.to_string())
                .collect::<Vec<_>>()
                .join(".")
        );
        let mut g = Group {
            policy,
            lines,
            domain: face_domain(&sig, lines),
            sig,
            keys: gkeys.clone(),
            lowest,
            tag,
            state: BTreeMap::new(),
            cache: BTreeMap::new(),
            rules: vec![],
            pieces: vec![],
            rule_lists: BTreeMap::new(),
            scan_jobs: 0,
            requests: 0,
            variant_requests: 0,
        };
        // A: the rule at every key
        let r0 = Rule {
            id: "rule".into(),
            phase: "rule".into(),
            base: policy.rule.centres.clone(),
            extra: vec![],
            label: None,
            shells: policy.rule.shells.clone(),
        };
        g.run_rules(solver, std::slice::from_ref(&r0), &gkeys)?;
        solver.progress("rule", &g.partial());
        // B: escalation for the keys without a relation
        let mut shells = policy.rule.shells.clone();
        for step in &policy.escalation {
            let open: Vec<Key> = gkeys
                .iter()
                .filter(|k| !g.has_relation(k))
                .cloned()
                .collect();
            if open.is_empty() {
                break;
            }
            for s in &step.add_shells {
                if !shells.contains(s) {
                    shells.push(s.clone());
                }
            }
            // a non-cumulative step: the rule's shells + its own (shell subsets, smallest first)
            let step_shells: Vec<String> = if step.cumulative {
                shells.clone()
            } else {
                let mut v = policy.rule.shells.clone();
                for s in &step.add_shells {
                    if !v.contains(s) {
                        v.push(s.clone());
                    }
                }
                v
            };
            let rules = g.step_rules(step, "escalation", &step_shells, &open);
            g.run_rules(solver, &rules, &open)?;
            solver.progress(&format!("escalation:{}", step.id), &g.partial());
        }
        gs.push(g);
    }
    // ---- PUBLISH the close-first report: nothing after this point can hold it back or change it ----
    {
        let mut keys_cf = vec![];
        let mut groups_cf = vec![];
        for g in &gs {
            keys_cf.extend(g.key_reports());
            groups_cf.push(g.group_report(g.close_first_pieces()));
        }
        let requests: u64 = gs.iter().map(|g| g.requests).sum();
        solver.publish(&assemble(
            policy,
            "close_first",
            groups_cf,
            keys_cf,
            requests,
        ));
    }
    // ---- VARIANTS (S + R): variants of SERVED keys, with the shells of the rule that served each key first; own
    // budget; they only add alternatives ----
    for g in &mut gs {
        let mut by_shells: BTreeMap<Vec<String>, Vec<Key>> = BTreeMap::new();
        for k in &g.keys {
            let first_rule = g
                .state
                .get(k)
                .and_then(|s| s.alts.first())
                .map(|a| a.rule.clone());
            if let Some(sh) = first_rule
                .and_then(|r| g.rule_lists.get(&r))
                .map(|(r, _)| r.shells.clone())
            {
                by_shells.entry(sh).or_default().push(k.clone());
            }
        }
        for (sh, ks) in by_shells {
            g.centre_scan(solver, &ks, &sh)?;
            g.regular_escalation(solver, &ks, &sh)?;
        }
        solver.progress("variants", &g.partial());
    }
    // ---- L: lists + domain test per chosen rule (full-list alternatives) ----
    let mut out_groups = vec![];
    let mut out_keys = vec![];
    let mut requests = 0u64;
    for mut g in gs {
        for (rid, ks) in g.chosen_full_by_rule() {
            g.lists_for_rule(solver, &rid, &ks)?;
        }
        solver.progress("lists", &g.partial());
        requests += g.requests;
        out_keys.extend(g.key_reports());
        let pieces = std::mem::take(&mut g.pieces);
        out_groups.push(g.group_report(pieces));
    }
    Ok(assemble(policy, "final", out_groups, out_keys, requests))
}

#[cfg(test)]
mod int7_choose_tests {
    use super::*;

    fn alt(digest: &str, terms: u64, seeds: usize, d0: D0Verdict) -> Alternative {
        Alternative {
            phase: "rule".into(),
            rule: "r".into(),
            list: "l".into(),
            seeds,
            seeds_at_key: None,
            rhs_terms: terms,
            d0,
            relation_digest: digest.into(),
            recipe_digest: String::new(),
            record_sha256: String::new(),
        }
    }

    /// int7 (B1): the choice among certified alternatives is poles::pole_objective with the user's objective.
    #[test]
    fn the_choice_is_the_pole_objective() {
        let s = |p| D0Verdict::Singular { order: Some(p) };
        // regular wins whatever its size (max_size_ratio unbounded)
        let a = [alt("a", 10, 5, s(1)), alt("b", 1000, 9, D0Verdict::Regular)];
        assert_eq!(choose(&a, true), Some(1));
        // no regular one: the lower pole order, then the size
        let a = [
            alt("a", 10, 5, s(2)),
            alt("b", 1000, 9, s(1)),
            alt("c", 5, 1, D0Verdict::Undecided),
        ];
        assert_eq!(
            choose(&a, true),
            Some(1),
            "unknown ranks after every known order"
        );
        // economy mode: the smallest
        assert_eq!(choose(&a, false), Some(2));
        // ties: fewer seeds, then the digest, whatever the order found
        let a = [
            alt("z", 10, 7, D0Verdict::Regular),
            alt("y", 10, 3, D0Verdict::Regular),
        ];
        assert_eq!(choose(&a, true), Some(1));
        let b = [a[1].clone(), a[0].clone()];
        assert_eq!(choose(&b, true), Some(0));
        // all undecided (the stub judge): the size decides, as before
        let a = [
            alt("a", 30, 1, D0Verdict::Undecided),
            alt("b", 20, 9, D0Verdict::Undecided),
        ];
        assert_eq!(choose(&a, true), Some(1));
        assert_eq!(choose(&[], true), None);
    }
}
