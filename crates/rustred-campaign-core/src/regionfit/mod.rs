//! REGION FIT (interfaces/regionfit.md; TRICKS item 2, reusable for split recipes by locus, items 12-14): partition
//! one face's keys into regions `(condition, one choice)` plus keys served per key, from the SET of acceptable
//! choices at every key. A choice is an opaque label (an element, an element + image rule, a recipe, ...).
//!
//! Conditions are conjunctions of at most `max_atoms` atoms from a closed family given as spec data
//! ([`atoms::AtomFamily`]). Objective (lexicographic): fewest pieces (regions + residual_key_cost x keys served per
//! key), simplest conditions (atoms, kind weight), best ranked choices (caller-supplied ranking), canonical text.
//! Greedy always; an exact branch and bound below a size threshold; the record says which was used.
//!
//! SOUNDNESS: a region is emitted only if its choice is acceptable at EVERY handed-in key that satisfies its condition
//! (re-checked atom by atom at emission). Keys outside the list are not claimed ("verified on N keys of the box").
//! Pure and deterministic: no file, process or clock; input order is irrelevant.

pub mod atoms;
pub mod cover;
pub mod keychoices;

use std::collections::{BTreeMap, BTreeSet, HashMap};

use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use atoms::{Atom, AtomFamily, Bits, condition_text};
use cover::{Cand, Cost};

pub const RECORD_SCHEMA: &str = "rustred.campaign.regionfit.v1";

/// One key of the face with the set of acceptable choices (empty = nothing serves it; such a key blocks every region
/// whose condition it satisfies).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FaceKey {
    pub key: Vec<i32>,
    pub acceptable: BTreeSet<String>,
    /// audit r2-planners R1: per acceptable label, the IMAGE RULE(S) it stands for at this key (the composed form,
    /// element + image rule, of the producer's choice): a label that does not fix the image rule (e.g. the element
    /// alone) keeps it here, and every region / residual record names it. Empty: the input states none.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub image_rules: BTreeMap<String, BTreeSet<String>>,
}

/// ALL keys of one unserved sub-face inside the box (any order, no duplicate key, one arity).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FaceInput {
    pub signature: String,
    pub keys: Vec<FaceKey>,
}

/// Every threshold is policy data; the defaults are documented in interfaces/regionfit.md. planfix (audit r2-planners
/// ITEM 6): each field states its default and what backs it; "default not yet backed by an experiment" = a number
/// chosen by the lane, not measured.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct RegionPolicy {
    pub atoms: AtomFamily,
    /// Atoms per condition. Default 2: the hand's region conditions use at most two atoms (TRICKS 2); not measured
    /// against 1 or 3 (default not yet backed by an experiment).
    pub max_atoms: usize,
    /// A candidate region holds at least this many face keys. Default 2. Default not yet backed by an experiment.
    pub min_region_keys: usize,
    /// Pieces charged per key served per key (a region costs 1).
    pub residual_key_cost: u64,
    /// Preferred choices first; unlisted choices after, in label order.
    pub ranking: Vec<String>,
    /// Exact search only when the candidate regions (after dominance) are at most this many ... Default 400. Default
    /// not yet backed by an experiment (the T178 key-choice faces all fitted "exact" under it, regions notes; no sweep).
    pub exact_max_candidates: usize,
    /// ... and the keys at most this many. Default 4000. Default not yet backed by an experiment.
    pub exact_max_keys: usize,
    /// Branch-and-bound node budget. Default 200000. Default not yet backed by an experiment.
    pub exact_node_budget: u64,
    /// Dominance pruning only up to this many candidates (it is quadratic). Default 20000. Default not yet backed by an
    /// experiment.
    pub dominance_max_candidates: usize,
    /// Conditions enumerated at most (deterministic cut; the record says when it was hit). Default 5000000. Default
    /// not yet backed by an experiment.
    pub max_conditions: u64,
    /// The deeper key list breaks ties among conditions with the same truth set on the face.
    pub prefer_extending: bool,
    pub max_alternatives_listed: usize,
}

impl Default for RegionPolicy {
    fn default() -> Self {
        RegionPolicy {
            atoms: AtomFamily::default(),
            max_atoms: 2,
            min_region_keys: 2,
            residual_key_cost: 1,
            ranking: vec![],
            exact_max_candidates: 400,
            exact_max_keys: 4000,
            exact_node_budget: 200_000,
            dominance_max_candidates: 20_000,
            max_conditions: 5_000_000,
            prefer_extending: true,
            max_alternatives_listed: 8,
        }
    }
}

impl RegionPolicy {
    /// sha256 of the canonical JSON of the policy (defaults filled in).
    pub fn digest(&self) -> String {
        let v = serde_json::to_value(self).expect("policy serialises");
        crate::closure::sha256::sha256_tag(canon(&v).as_bytes())
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RegionFitError {
    Arity {
        expected: usize,
        got: usize,
    },
    Duplicate(Vec<i32>),
    Policy(String),
    /// A soundness re-check failed (a bug, never a result).
    Internal(String),
}

impl std::fmt::Display for RegionFitError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            RegionFitError::Arity { expected, got } => {
                write!(f, "key arity {got}, expected {expected}")
            }
            RegionFitError::Duplicate(k) => write!(f, "duplicate key {k:?}"),
            RegionFitError::Policy(s) => write!(f, "policy: {s}"),
            RegionFitError::Internal(s) => write!(f, "internal: {s}"),
        }
    }
}

impl std::error::Error for RegionFitError {}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum FitMethod {
    /// Branch and bound completed: the cover is optimal for the objective among the candidate regions.
    Exact,
    /// Exact search not attempted (above the size threshold).
    Greedy,
    /// Exact search hit its node budget: the best cover found (never worse than greedy).
    ExactBudgetExhausted,
}

impl FitMethod {
    pub fn text(self) -> &'static str {
        match self {
            FitMethod::Exact => "exact",
            FitMethod::Greedy => "greedy",
            FitMethod::ExactBudgetExhausted => "exact_budget_exhausted",
        }
    }
}

/// `n` of the `m` deeper keys satisfying the condition accept the region's choice.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Extends {
    pub n: usize,
    pub m: usize,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Region {
    pub atoms: Vec<Atom>,
    pub text: String,
    pub choice: String,
    /// Keys attributed by dispatch order (first region whose condition holds).
    pub keys: usize,
    /// Face keys satisfying the condition (every one accepts the choice).
    pub verified: usize,
    pub extends: Option<Extends>,
    /// Conditions with the same truth set on the face (>= 1).
    pub equivalent_conditions: usize,
    pub alternatives: Vec<String>,
    /// audit r2-planners R1: the image rules (composed forms) the choice stands for at the face keys satisfying the
    /// condition (sorted; empty = the input states none). More than one: the label does not fix the composed relation.
    pub image_rules: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RegionFit {
    pub signature: String,
    pub keys: usize,
    pub key_list_digest: String,
    pub deeper: Option<(usize, String)>,
    pub policy_digest: String,
    pub method: FitMethod,
    pub cost: Cost,
    pub candidates: usize,
    pub exact_nodes: u64,
    pub conditions_truncated: bool,
    pub regions: Vec<Region>,
    /// (key, best ranked acceptable choice), keys in sorted order.
    pub residual: Vec<(Vec<i32>, String)>,
    /// audit r2-planners R1: the image rules of each residual key's choice (parallel to `residual`).
    pub residual_rules: Vec<Vec<String>>,
    /// Keys with no acceptable choice.
    pub unservable: Vec<Vec<i32>>,
}

fn canon(v: &Value) -> String {
    crate::schema::canon::canonical_json(v, false).expect("integers only")
}

fn key_list_digest(keys: &[FaceKey]) -> String {
    let v: Vec<Value> = keys
        .iter()
        .map(|k| json!([k.key, k.acceptable.iter().collect::<Vec<_>>()]))
        .collect();
    crate::closure::sha256::sha256_tag(canon(&Value::Array(v)).as_bytes())
}

fn normalise(face: &FaceInput) -> Result<Vec<FaceKey>, RegionFitError> {
    let mut keys = face.keys.clone();
    keys.sort_by(|a, b| a.key.cmp(&b.key));
    if let Some(first) = keys.first() {
        let ar = first.key.len();
        for k in &keys {
            if k.key.len() != ar {
                return Err(RegionFitError::Arity {
                    expected: ar,
                    got: k.key.len(),
                });
            }
        }
    }
    for w in keys.windows(2) {
        if w[0].key == w[1].key {
            return Err(RegionFitError::Duplicate(w[0].key.clone()));
        }
    }
    Ok(keys)
}

/// Enumerate conjunctions of at most `max_atoms` of `atoms` (index-increasing), skipping an atom that does not shrink
/// the truth set and stopping below `min_keys`; `visit(atom indices, truth set)`. Returns false when the budget cut.
fn enum_conditions(
    atoms: &[(Atom, Bits)],
    max_atoms: usize,
    min_keys: usize,
    full: &Bits,
    budget: &mut u64,
    keep: &dyn Fn(&Bits) -> bool,
    visit: &mut dyn FnMut(&[usize], &Bits),
) -> bool {
    struct Walk<'a> {
        atoms: &'a [(Atom, Bits)],
        max_atoms: usize,
        min_keys: usize,
        budget: &'a mut u64,
        keep: &'a dyn Fn(&Bits) -> bool,
        visit: &'a mut dyn FnMut(&[usize], &Bits),
    }
    impl Walk<'_> {
        fn rec(&mut self, start: usize, cur: &mut Vec<usize>, t: &Bits) -> bool {
            if *self.budget == 0 {
                return false;
            }
            *self.budget -= 1;
            (self.visit)(cur, t);
            if cur.len() == self.max_atoms {
                return true;
            }
            let tc = t.count();
            for a in start..self.atoms.len() {
                let t2 = t.and(&self.atoms[a].1);
                let c = t2.count();
                if c == tc || c < self.min_keys || !(self.keep)(&t2) {
                    continue;
                }
                cur.push(a);
                let ok = self.rec(a + 1, cur, &t2);
                cur.pop();
                if !ok {
                    return false;
                }
            }
            true
        }
    }
    if full.count() < min_keys {
        return true;
    }
    let mut w = Walk {
        atoms,
        max_atoms,
        min_keys,
        budget,
        keep,
        visit,
    };
    w.rec(0, &mut vec![], full)
}

/// Fit regions on one face. `deeper` (optional) = a further key list (e.g. the next layer) tested for `extends` only.
pub fn fit_regions(
    face: &FaceInput,
    deeper: Option<&[FaceKey]>,
    policy: &RegionPolicy,
) -> Result<RegionFit, RegionFitError> {
    if policy.residual_key_cost == 0 {
        return Err(RegionFitError::Policy(
            "residual_key_cost must be >= 1".into(),
        ));
    }
    let keys = normalise(face)?;
    let n = keys.len();
    let raw: Vec<Vec<i32>> = keys.iter().map(|k| k.key.clone()).collect();
    if let (Some(d), Some(f)) = (deeper, raw.first())
        && let Some(bad) = d.iter().find(|k| k.key.len() != f.len())
    {
        return Err(RegionFitError::Arity {
            expected: f.len(),
            got: bad.key.len(),
        });
    }
    // choice universe, ranks, per-choice key sets
    let universe: Vec<String> = keys
        .iter()
        .flat_map(|k| k.acceptable.iter().cloned())
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect();
    let pos: HashMap<&str, usize> = policy
        .ranking
        .iter()
        .enumerate()
        .rev()
        .map(|(i, s)| (s.as_str(), i))
        .collect();
    let rank: Vec<u64> = universe
        .iter()
        .enumerate()
        .map(|(u, s)| match pos.get(s.as_str()) {
            Some(&p) => p as u64,
            None => (policy.ranking.len() + u) as u64,
        })
        .collect();
    let index: HashMap<&str, usize> = universe
        .iter()
        .enumerate()
        .map(|(i, s)| (s.as_str(), i))
        .collect();
    let mut acc: Vec<Bits> = vec![Bits::empty(n); universe.len()];
    let mut coverable = Bits::empty(n);
    let mut key_best: Vec<Option<usize>> = vec![None; n];
    for (k, fk) in keys.iter().enumerate() {
        for c in &fk.acceptable {
            let u = index[c.as_str()];
            acc[u].set(k);
            if key_best[k].is_none_or(|b| rank[u] < rank[b]) {
                key_best[k] = Some(u);
            }
        }
        if !fk.acceptable.is_empty() {
            coverable.set(k);
        }
    }
    let key_rank: Vec<u64> = key_best.iter().map(|b| b.map_or(0, |u| rank[u])).collect();
    let best_choice = |t: &Bits| -> Option<usize> {
        (0..universe.len())
            .filter(|&u| t.subset_of(&acc[u]))
            .min_by_key(|&u| (rank[u], u))
    };

    // atoms and candidate regions
    let w = &policy.atoms.kind_weight;
    let all_atoms = atoms::enumerate(&policy.atoms, &raw).map_err(RegionFitError::Policy)?;
    let reps = atoms::representatives(&all_atoms, w);
    let full = Bits::full(n);
    let min_keys = policy.min_region_keys.max(1);
    let mut map: HashMap<Bits, Cand> = HashMap::new();
    let mut budget = policy.max_conditions;
    let complete = enum_conditions(
        &reps,
        policy.max_atoms,
        min_keys,
        &full,
        &mut budget,
        &|_| true,
        &mut |idx, t| {
            let mut at: Vec<Atom> = idx.iter().map(|&i| reps[i].0).collect();
            at.sort();
            let weight: u64 = at.iter().map(|a| a.weight(w)).sum();
            if let Some(c) = map.get_mut(t) {
                if (at.len(), weight, &at) < (c.atoms.len(), c.weight, &c.atoms) {
                    c.atoms = at;
                    c.weight = weight;
                }
                return;
            }
            if let Some(u) = best_choice(t) {
                map.insert(
                    t.clone(),
                    Cand {
                        bits: t.clone(),
                        size: t.count(),
                        atoms: at,
                        weight,
                        choice: u,
                        rank: rank[u],
                    },
                );
            }
        },
    );
    let mut cands: Vec<Cand> = map.into_values().collect();
    cands.sort_by(|a, b| {
        (std::cmp::Reverse(a.size), a.simplicity(), &a.bits).cmp(&(
            std::cmp::Reverse(b.size),
            b.simplicity(),
            &b.bits,
        ))
    });
    if cands.len() <= policy.dominance_max_candidates {
        cands = cover::prune_dominated(cands);
    }

    // search
    let g = cover::greedy(&cands, &coverable, &key_rank, policy.residual_key_cost);
    let (method, sol, nodes) =
        if cands.len() <= policy.exact_max_candidates && n <= policy.exact_max_keys {
            let ex = cover::exact(
                &cands,
                &coverable,
                &key_rank,
                policy.residual_key_cost,
                g.cost,
                policy.exact_node_budget,
            );
            let m = if ex.exhausted {
                FitMethod::ExactBudgetExhausted
            } else {
                FitMethod::Exact
            };
            match ex.solution {
                Some(mut s) => {
                    s.regions.sort_by(|&a, &b| {
                        let (x, y) = (&cands[a], &cands[b]);
                        (std::cmp::Reverse(x.size), x.simplicity(), a).cmp(&(
                            std::cmp::Reverse(y.size),
                            y.simplicity(),
                            b,
                        ))
                    });
                    (m, s, ex.nodes)
                }
                None => (m, g, ex.nodes),
            }
        } else {
            (FitMethod::Greedy, g, 0)
        };

    // emitted regions: dispatch attribution, equivalent conditions, extends, soundness re-check
    let chosen: Vec<&Cand> = sol.regions.iter().map(|&i| &cands[i]).collect();
    let targets: Vec<&Bits> = chosen.iter().map(|c| &c.bits).collect();
    let mut equiv: Vec<Vec<Vec<Atom>>> = vec![vec![]; chosen.len()];
    {
        let mut b2 = policy.max_conditions;
        let all_ref = &all_atoms;
        enum_conditions(
            all_ref,
            policy.max_atoms,
            min_keys,
            &full,
            &mut b2,
            &|t| targets.iter().any(|x| x.subset_of(t)),
            &mut |idx, t| {
                for (r, x) in targets.iter().enumerate() {
                    if *x == t {
                        let mut at: Vec<Atom> = idx.iter().map(|&i| all_ref[i].0).collect();
                        at.sort();
                        equiv[r].push(at);
                    }
                }
            },
        );
    }
    let deeper_sorted: Option<Vec<FaceKey>> = deeper.map(|d| {
        let mut v = d.to_vec();
        v.sort_by(|a, b| a.key.cmp(&b.key));
        v
    });
    let extends_of = |at: &[Atom], choice: &str| -> Option<Extends> {
        deeper_sorted.as_ref().map(|d| {
            let sat: Vec<&FaceKey> = d
                .iter()
                .filter(|k| at.iter().all(|a| a.eval(&k.key)))
                .collect();
            Extends {
                n: sat.iter().filter(|k| k.acceptable.contains(choice)).count(),
                m: sat.len(),
            }
        })
    };
    let mut assigned = Bits::empty(n);
    let mut regions = vec![];
    for (r, c) in chosen.iter().enumerate() {
        let choice = universe[c.choice].clone();
        let mut eq = std::mem::take(&mut equiv[r]);
        if !eq.contains(&c.atoms) {
            eq.push(c.atoms.clone());
        }
        let simp = |a: &Vec<Atom>| {
            (
                a.len(),
                a.iter().map(|x| x.weight(w)).sum::<u64>(),
                a.clone(),
            )
        };
        eq.sort_by_key(simp);
        eq.dedup();
        let shown: Vec<Atom> = if policy.prefer_extending && deeper_sorted.is_some() {
            eq.iter()
                .min_by_key(|a| {
                    let e = extends_of(a, &choice).expect("deeper given");
                    (e.n != e.m, std::cmp::Reverse(e.n), simp(a))
                })
                .cloned()
                .unwrap_or_else(|| c.atoms.clone())
        } else {
            c.atoms.clone()
        };
        // soundness: atom by atom over the complete list
        let mut verified = 0;
        let mut image_rules: BTreeSet<String> = BTreeSet::new();
        for fk in &keys {
            if shown.iter().all(|a| a.eval(&fk.key)) {
                if let Some(f) = fk.image_rules.get(&choice) {
                    image_rules.extend(f.iter().cloned());
                }
                if !fk.acceptable.contains(&choice) {
                    return Err(RegionFitError::Internal(format!(
                        "region {} / {choice} fails at {:?}",
                        condition_text(&shown),
                        fk.key
                    )));
                }
                verified += 1;
            }
        }
        if verified != c.size {
            return Err(RegionFitError::Internal("truth set size mismatch".into()));
        }
        let own = c.bits.and_not(&assigned);
        assigned = Bits(
            assigned
                .0
                .iter()
                .zip(&c.bits.0)
                .map(|(a, b)| a | b)
                .collect(),
        );
        let alternatives: Vec<String> = eq
            .iter()
            .filter(|a| **a != shown)
            .take(policy.max_alternatives_listed)
            .map(|a| condition_text(a))
            .collect();
        regions.push(Region {
            text: condition_text(&shown),
            extends: extends_of(&shown, &choice),
            atoms: shown,
            choice,
            keys: own.count(),
            verified,
            equivalent_conditions: eq.len(),
            alternatives,
            image_rules: image_rules.into_iter().collect(),
        });
    }
    let mut residual = vec![];
    let mut residual_rules: Vec<Vec<String>> = vec![];
    let mut unservable = vec![];
    for (k, fk) in keys.iter().enumerate() {
        if assigned.get(k) {
            continue;
        }
        match key_best[k] {
            Some(u) => {
                residual_rules.push(
                    fk.image_rules
                        .get(&universe[u])
                        .map(|f| f.iter().cloned().collect())
                        .unwrap_or_default(),
                );
                residual.push((fk.key.clone(), universe[u].clone()));
            }
            None => unservable.push(fk.key.clone()),
        }
    }
    Ok(RegionFit {
        signature: face.signature.clone(),
        keys: n,
        key_list_digest: key_list_digest(&keys),
        deeper: deeper_sorted
            .as_ref()
            .map(|d| (d.len(), key_list_digest(d))),
        policy_digest: policy.digest(),
        method,
        cost: sol.cost,
        candidates: cands.len(),
        exact_nodes: nodes,
        conditions_truncated: !complete,
        regions,
        residual,
        residual_rules,
        unservable,
    })
}

impl RegionFit {
    /// The canonical record (interfaces/regionfit.md 4) with its digest.
    pub fn record(&self) -> Value {
        let regions: Vec<Value> = self
            .regions
            .iter()
            .map(|r| {
                json!({
                    "condition": {"atoms": r.atoms.iter().map(Atom::json).collect::<Vec<_>>(), "text": r.text},
                    "choice": r.choice, "keys": r.keys, "verified": r.verified,
                    "verified_on": format!("verified on {} keys of the box", self.keys),
                    "extends": r.extends.map(|e| json!({"n": e.n, "m": e.m})),
                    "equivalent_conditions": r.equivalent_conditions, "alternatives": r.alternatives,
                    // audit r2-planners R1: what the choice label stands for (null: the input states no image rule)
                    "image_rules": (!r.image_rules.is_empty()).then_some(&r.image_rules),
                    "image_rule_uniform": (!r.image_rules.is_empty()).then_some(r.image_rules.len() == 1),
                })
            })
            .collect();
        let mut v = json!({
            "schema": RECORD_SCHEMA, "face": self.signature, "keys": self.keys,
            "key_list_digest": self.key_list_digest,
            "deeper": self.deeper.as_ref().map(|(m, d)| json!({"keys": m, "key_list_digest": d})),
            "policy_digest": self.policy_digest,
            "fit": {"method": self.method.text(), "pieces": self.cost.pieces, "regions": self.regions.len(),
                "residual": self.residual.len(), "unservable": self.unservable.len(),
                "atoms": self.cost.atoms, "candidates": self.candidates, "exact_nodes": self.exact_nodes,
                "conditions_truncated": self.conditions_truncated},
            "regions": regions,
            "residual": self.residual.iter().zip(&self.residual_rules).map(|((k, c), f)| {
                let mut e = json!({"key": k, "choice": c});
                if !f.is_empty() {
                    e["image_rules"] = json!(f);
                }
                e
            }).collect::<Vec<_>>(),
            "unservable": self.unservable,
        });
        let d = crate::closure::sha256::sha256_tag(canon(&v).as_bytes());
        v["digest"] = json!(d);
        v
    }

    /// Canonical text of [`RegionFit::record`].
    pub fn record_text(&self) -> String {
        canon(&self.record())
    }

    /// Regions by choice (summary helper).
    pub fn by_choice(&self) -> BTreeMap<String, Vec<String>> {
        let mut m: BTreeMap<String, Vec<String>> = BTreeMap::new();
        for r in &self.regions {
            m.entry(r.choice.clone()).or_default().push(r.text.clone());
        }
        m
    }
}
