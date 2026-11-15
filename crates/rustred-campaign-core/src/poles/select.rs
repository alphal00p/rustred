//! THE SELECTION among candidates for one key (or one domain) (pole-diagnostics.md section 8, addenda 4 and 7; TRICKS
//! items 9 and 12-13): wherever several ADMISSIBLE relations / rules / scan entries exist (termmin, repairflow's symbolic
//! choice, the recipe relation choice, the seed-minimisation centre scan, the relation cache), this one pure function
//! chooses.
//!
//! Order (criteria `regular_first`, the default):
//! 1. higher evidence level (the caller's rank; equal when not used);
//! 2. regular at d0 over singular, with the size guard `max_size_ratio` (addendum 7, DEFAULT `"unbounded"`: a regular
//!    candidate ALWAYS wins, whatever its size; with a finite ratio R a regular candidate wins only up to R x the
//!    smallest admissible size, beyond it the smaller singular one is kept and the choice says so);
//! 3. among regular candidates: fewer terms; among singular ones: lower pole order, then fewer terms;
//! 4. ties: the smaller `id` (a relation digest or any stable text): the result does not depend on the input order.
//!
//! Criteria `economy_first`: fewer terms first; regularity, then pole order, break exact ties only.
//!
//! GENERALISED (lane epsdepth, addendum 8; interfaces/epsdepth.md 6): [`pole_objective`] is THE ranking function: a
//! lexicographic list of [`Measure`]s (spec data, default (a0) eps box excess, (a) max pole order, (b) sum of pole orders,
//! (c) singular terms, (d) size) over the candidates' [`PoleProfile`]s (composed per terminal when every candidate has
//! one, else local per rhs term), with the size guard on the best class of the first discriminating measure.
//! [`select`] is its special case ([`PoleObjective::from_criteria`]) and keeps the behaviour described above.
//!
//! A candidate whose regularity is UNKNOWN (no D1 record, an undecided verdict) is not regular and ranks after every
//! singular candidate of known order. A NON-admissible candidate is never returned; when such a candidate is regular and
//! names the cap that removed it (`excluded_by`), the choice reports "regular candidate exists, over <cap>".
//! Pure: no I/O, no clock; the preference never decides whether a key is served or whether a node closes.

use super::params::{CriteriaOrder, MaxSizeRatio, PolicyError};
use serde_json::{Value, json};

/// One candidate with the facts the selection needs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Candidate {
    /// Stable identity (a relation digest, a rule record sha256, a scan-entry label): the last tie-break.
    pub id: String,
    /// Certified by the normal accept chain (in domain, descent, exact proof, required evidence).
    pub admissible: bool,
    /// The cap that removed a found candidate (`"max_solves"`, `"rule_size_budget"`, ...): reported, never chosen.
    pub excluded_by: Option<String>,
    /// Higher = stronger evidence (e.g. per-key = 2, imported = 1, none = 0); equal everywhere when unused.
    pub evidence_level: u32,
    /// Size measure: right-hand-side terms (the sizes compared by `max_size_ratio`).
    pub size: u64,
    /// Pole order at d0 from its D1 record: `Some(0)` regular, `Some(p)` singular, `None` unknown. Over a DOMAIN, the
    /// maximum over its keys ([`aggregate_orders`]; TRICKS 9: decided per key, never from the rule's text).
    pub pole_order: Option<u64>,
}

impl Candidate {
    pub fn is_regular(&self) -> bool {
        self.pole_order == Some(0)
    }
}

/// The pole order of a candidate over several keys: the maximum, `None` when any key is unknown (undecided).
pub fn aggregate_orders(per_key: &[Option<u64>]) -> Option<u64> {
    per_key.iter().try_fold(0u64, |m, o| o.map(|x| m.max(x)))
}

/// Why the chosen candidate won.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChoiceReason {
    /// The chosen candidate is regular at d0 (and won by regularity or was the only / smallest one).
    Regular,
    /// No regular admissible candidate: the lowest pole order, then the fewest terms.
    LowestPole,
    /// A regular admissible candidate exists but exceeds `max_size_ratio`: the smaller singular one is kept.
    SizeGuard,
    /// `economy_first`: the smallest candidate.
    Economy,
    /// Only one admissible candidate.
    Only,
}

impl ChoiceReason {
    pub fn word(&self) -> &'static str {
        match self {
            ChoiceReason::Regular => "regular",
            ChoiceReason::LowestPole => "lowest_pole",
            ChoiceReason::SizeGuard => "size_guard",
            ChoiceReason::Economy => "economy",
            ChoiceReason::Only => "only",
        }
    }
}

/// What happened to the best regular alternative when the chosen candidate is not regular.
#[derive(Debug, Clone, PartialEq)]
pub enum RegularAlternative {
    /// The chosen candidate is regular.
    Chosen,
    /// A regular admissible candidate was rejected by the size guard: its size and the ratio to the smallest size.
    RejectedForSize {
        id: String,
        size: u64,
        ratio: String,
    },
    /// A regular candidate exists but another cap removed it (not admissible).
    OverCap { id: String, cap: String, size: u64 },
    /// No regular candidate among those given.
    None,
}

/// The selection's result and its report line.
#[derive(Debug, Clone, PartialEq)]
pub struct Choice {
    /// Index into the input slice.
    pub index: usize,
    pub id: String,
    pub reason: ChoiceReason,
    pub chosen_size: u64,
    /// The smallest size among the admissible candidates of the chosen evidence level.
    pub smallest_size: u64,
    pub chosen_pole_order: Option<u64>,
    pub max_size_ratio: String,
    pub regular_alternative: RegularAlternative,
    pub criteria_order: CriteriaOrder,
    /// [`pole_objective`]: which profile decided (`composed` | `local`), the measures and the chosen candidate's values
    /// (`u64::MAX` = unknown), and the candidates of the class rejected by the size guard (id, ratio to the smallest).
    pub profile_used: &'static str,
    pub measures: Vec<&'static str>,
    pub chosen_measures: Vec<u64>,
    pub rejected_for_size: Vec<(String, String)>,
    /// Pooled candidates without a fully decided composed profile when the local profile had to decide for all.
    pub lacking_composed: Vec<String>,
}

fn ratio_text(size: u64, smallest: u64) -> String {
    if smallest == 0 {
        return "unbounded".into();
    }
    let g = {
        let (mut a, mut b) = (size, smallest);
        while b != 0 {
            (a, b) = (b, a % b);
        }
        a.max(1)
    };
    let (n, d) = (size / g, smallest / g);
    if d == 1 {
        n.to_string()
    } else {
        format!("{n}/{d}")
    }
}

/// Choose among `cands`. `None` iff no candidate is admissible. THE SPECIAL CASE of [`pole_objective`] with the
/// measures of [`PoleObjective::from_criteria`] (lane epsdepth generalised it; behaviour unchanged).
pub fn select(cands: &[Candidate], ratio: MaxSizeRatio, criteria: CriteriaOrder) -> Option<Choice> {
    pole_objective(cands, &[], &PoleObjective::from_criteria(ratio, criteria), None)
        .expect("no eps box, no composed profile: the special case cannot fail")
}

/// One measure of the lexicographic pole objective (addendum 8.1; interfaces/epsdepth.md 6). Lower is better.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Measure {
    /// (a0) only with an eps box and a composed profile: terminals whose required depth (needed_order + p) exceeds the
    /// available order of the box; 0 otherwise (recorded).
    EpsBoxExcess,
    /// 0 regular at d0, 1 singular or unknown (the two-valued preference of addenda 4 / 7).
    Regular,
    /// (a) max pole order: over terminals (composed) or rhs terms (local); unknown = worst.
    MaxPoleOrder,
    /// (b) sum of pole orders: over terminals = extra eps coefficients needed (composed), over rhs terms (local).
    SumPoleOrders,
    /// (c) number of singular entries (terminals with a pole, resp. rhs terms with a pole).
    SingularTerms,
    /// (d) size (rhs terms).
    Size,
}

impl Measure {
    pub fn word(&self) -> &'static str {
        match self {
            Measure::EpsBoxExcess => "eps_box_excess",
            Measure::Regular => "regular",
            Measure::MaxPoleOrder => "max_pole_order",
            Measure::SumPoleOrders => "sum_pole_orders",
            Measure::SingularTerms => "singular_terms",
            Measure::Size => "size",
        }
    }
    pub fn parse(w: &str) -> Result<Measure, PolicyError> {
        Ok(match w {
            "eps_box_excess" => Measure::EpsBoxExcess,
            "regular" => Measure::Regular,
            "max_pole_order" => Measure::MaxPoleOrder,
            "sum_pole_orders" => Measure::SumPoleOrders,
            "singular_terms" => Measure::SingularTerms,
            "size" => Measure::Size,
            _ => {
                return Err(PolicyError(format!(
                    "pole_objective: unknown measure {w:?} (allowed: eps_box_excess, regular, max_pole_order, \
                     sum_pole_orders, singular_terms, size)"
                )));
            }
        })
    }
}

/// The objective: measures applied lexicographically (then the id) and the size guard. Spec data
/// (`pole_objective: {"measures": [..], "max_size_ratio": ..}`), defaults [`PoleObjective::default`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PoleObjective {
    pub measures: Vec<Measure>,
    pub max_size_ratio: MaxSizeRatio,
}

impl Default for PoleObjective {
    /// Addendum 8 + 7: (a0) eps box excess, (a) max pole order, (b) sum of pole orders, (c) singular terms, (d) size;
    /// `max_size_ratio` "unbounded".
    fn default() -> Self {
        PoleObjective {
            measures: vec![
                Measure::EpsBoxExcess,
                Measure::MaxPoleOrder,
                Measure::SumPoleOrders,
                Measure::SingularTerms,
                Measure::Size,
            ],
            max_size_ratio: MaxSizeRatio::Unbounded,
        }
    }
}

impl PoleObjective {
    /// Lane poles' B3 orders as objectives: `regular_first` = [regular, max_pole_order, size] (the guard bounds the regular
    /// class only), `economy_first` = [size, regular, max_pole_order].
    pub fn from_criteria(ratio: MaxSizeRatio, criteria: CriteriaOrder) -> PoleObjective {
        let measures = match criteria {
            CriteriaOrder::RegularFirst => vec![Measure::Regular, Measure::MaxPoleOrder, Measure::Size],
            CriteriaOrder::EconomyFirst => vec![Measure::Size, Measure::Regular, Measure::MaxPoleOrder],
        };
        PoleObjective { measures, max_size_ratio: ratio }
    }

    /// Parse `{"measures": [words], "max_size_ratio": ..}` (both optional; unknown members refused; an empty or repeated
    /// measure list refused). `inherited_ratio`: the guard of the policy that carries this objective
    /// (`repair_policy.prefer_regular.max_size_ratio`) when the object names none.
    pub fn from_value(v: &Value, inherited_ratio: MaxSizeRatio) -> Result<PoleObjective, PolicyError> {
        let o = v
            .as_object()
            .ok_or_else(|| PolicyError("pole_objective must be an object".into()))?;
        for k in o.keys() {
            if k != "measures" && k != "max_size_ratio" {
                return Err(PolicyError(format!(
                    "pole_objective: unknown member {k:?} (allowed: measures, max_size_ratio)"
                )));
            }
        }
        let mut obj = PoleObjective { max_size_ratio: inherited_ratio, ..PoleObjective::default() };
        if let Some(m) = o.get("measures") {
            let a = m
                .as_array()
                .ok_or_else(|| PolicyError("pole_objective.measures must be a list of words".into()))?;
            let mut out = vec![];
            for w in a {
                let w = w
                    .as_str()
                    .ok_or_else(|| PolicyError("pole_objective.measures must be a list of words".into()))?;
                let m = Measure::parse(w)?;
                if out.contains(&m) {
                    return Err(PolicyError(format!("pole_objective.measures: {w:?} listed twice")));
                }
                out.push(m);
            }
            if out.is_empty() {
                return Err(PolicyError("pole_objective.measures is empty".into()));
            }
            obj.measures = out;
        }
        if let Some(r) = o.get("max_size_ratio") {
            obj.max_size_ratio = MaxSizeRatio::parse(r)?;
        }
        Ok(obj)
    }

    pub fn to_json(&self) -> Value {
        json!({"measures": self.measures.iter().map(|m| m.word()).collect::<Vec<_>>(),
               "max_size_ratio": self.max_size_ratio.text()})
    }
}

/// The pole profile of a candidate (addendum 8.1). `local`: the pole order of every non-zero rhs term (empty = use the
/// candidate's `pole_order`); `composed`: per terminal (key, pole order; `None` = undecided) after composition down to
/// the terminal set, when available.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct PoleProfile {
    pub local: Vec<u32>,
    pub composed: Option<Vec<(Vec<i32>, Option<u32>)>>,
}

impl PoleProfile {
    fn composed_decided(&self) -> Option<Vec<(&[i32], u32)>> {
        self.composed
            .as_ref()?
            .iter()
            .map(|(k, o)| o.map(|o| (k.as_slice(), o)))
            .collect()
    }
}

const UNKNOWN: u64 = u64::MAX;

/// The values of every (evaluated) measure for one candidate. `orders`: the pole orders the measures read (terminals or rhs terms),
/// `None` = unknown.
fn measure_values(
    obj: &PoleObjective,
    c: &Candidate,
    orders: Option<&[u32]>,
    terminals: Option<&[&[i32]]>,
    eps_box: Option<&crate::epsdepth::epsbox::EpsBoxChecked>,
) -> Result<Vec<u64>, String> {
    let mut out = Vec::with_capacity(obj.measures.len());
    for m in &obj.measures {
        let v = match (m, orders) {
            (Measure::Size, _) => c.size,
            (Measure::EpsBoxExcess, Some(o)) => match (eps_box, terminals) {
                (Some(b), Some(ts)) => {
                    let mut n = 0u64;
                    for (t, &p) in ts.iter().zip(o) {
                        if matches!(b.status(t, p)?, crate::epsdepth::epsbox::BoxStatus::Exceeds(_)) {
                            n += 1;
                        }
                    }
                    n
                }
                _ => 0,
            },
            (Measure::EpsBoxExcess, None) => 0,
            (_, None) => UNKNOWN,
            (Measure::Regular, Some(o)) => u64::from(o.iter().any(|&p| p > 0)),
            (Measure::MaxPoleOrder, Some(o)) => o.iter().copied().max().map_or(0, u64::from),
            (Measure::SumPoleOrders, Some(o)) => o.iter().map(|&p| u64::from(p)).sum(),
            (Measure::SingularTerms, Some(o)) => o.iter().filter(|&&p| p > 0).count() as u64,
        };
        out.push(v);
    }
    Ok(out)
}

/// THE ranking function of every choice with a pole profile (addendum 8; interfaces/epsdepth.md 6). `profiles` is
/// empty (every candidate is judged by its `pole_order`) or parallel to `cands`.
///
/// 1. Pool: the admissible candidates of the highest evidence level (a non-admissible one is never returned).
/// 2. Profile used: COMPOSED when every pooled candidate has a fully decided composed profile, else LOCAL for all
///    (local = the rhs-term orders, or the candidate's `pole_order`; unknown = worst in every pole measure).
/// 3. Key: the measures in order, then the id (order-independent).
/// 4. Size guard (`max_size_ratio`, default "unbounded"): the FIRST measure that discriminates among the pool splits it
///    into classes; if every member of the best class is larger than ratio x the smallest pooled size, that class is
///    rejected for size (recorded) and the best of the others is chosen. The smallest candidate is never rejected, so a
///    choice always exists. With [`PoleObjective::from_criteria`] this is exactly lane poles' B3 order.
///
/// `Err` only for an eps box that does not cover a terminal of a composed profile.
pub fn pole_objective(
    cands: &[Candidate],
    profiles: &[PoleProfile],
    obj: &PoleObjective,
    eps_box: Option<&crate::epsdepth::epsbox::EpsBoxChecked>,
) -> Result<Option<Choice>, String> {
    if !profiles.is_empty() && profiles.len() != cands.len() {
        return Err(format!("pole_objective: {} profiles for {} candidates", profiles.len(), cands.len()));
    }
    let adm: Vec<usize> = (0..cands.len()).filter(|&i| cands[i].admissible).collect();
    let Some(top_level) = adm.iter().map(|&i| cands[i].evidence_level).max() else {
        return Ok(None);
    };
    let pool: Vec<usize> = adm
        .into_iter()
        .filter(|&i| cands[i].evidence_level == top_level)
        .collect();
    let smallest = pool.iter().map(|&i| cands[i].size).min().unwrap_or(0);
    let composed: Option<Vec<Vec<(&[i32], u32)>>> = if profiles.is_empty() {
        None
    } else {
        pool.iter().map(|&i| profiles[i].composed_decided()).collect()
    };
    let profile_used = if composed.is_some() { "composed" } else { "local" };
    // pooled candidates without a fully decided composed profile (the caller may ask for them)
    let lacking_composed: Vec<String> = if profiles.is_empty() || composed.is_some() {
        vec![]
    } else {
        pool.iter()
            .filter(|&&i| profiles[i].composed_decided().is_none())
            .map(|&i| cands[i].id.clone())
            .collect()
    };
    // eps_box_excess is evaluated only with an eps box AND composed profiles; otherwise it is no measure at all
    let evaluated = PoleObjective {
        measures: obj
            .measures
            .iter()
            .copied()
            .filter(|m| *m != Measure::EpsBoxExcess || (eps_box.is_some() && composed.is_some()))
            .collect(),
        max_size_ratio: obj.max_size_ratio,
    };
    let obj = &evaluated;
    let mut keys: Vec<(Vec<u64>, usize)> = Vec::with_capacity(pool.len());
    for (j, &i) in pool.iter().enumerate() {
        let c = &cands[i];
        let vals = match &composed {
            Some(all) => {
                let ts: Vec<&[i32]> = all[j].iter().map(|(t, _)| *t).collect();
                let os: Vec<u32> = all[j].iter().map(|(_, o)| *o).collect();
                measure_values(obj, c, Some(&os), Some(&ts), eps_box)?
            }
            None => {
                let local = profiles.get(i).filter(|p| !p.local.is_empty()).map(|p| p.local.clone());
                let local = local.or_else(|| c.pole_order.map(|p| vec![u32::try_from(p).unwrap_or(u32::MAX)]));
                measure_values(obj, c, local.as_deref(), None, eps_box)?
            }
        };
        keys.push((vals, i));
    }
    keys.sort_by(|a, b| a.0.cmp(&b.0).then_with(|| cands[a.1].id.cmp(&cands[b.1].id)));
    // the size guard on the best class of the first discriminating measure
    let first_disc = (0..obj.measures.len()).find(|&m| keys.iter().any(|k| k.0[m] != keys[0].0[m]));
    let mut rejected: Vec<usize> = vec![];
    if let Some(m) = first_disc
        && obj.measures[m] != Measure::Size
    {
        let best = keys[0].0[m];
        let class: Vec<usize> = keys.iter().filter(|k| k.0[m] == best).map(|k| k.1).collect();
        if class.iter().all(|&i| !obj.max_size_ratio.admits(cands[i].size, smallest)) {
            rejected = class;
        }
    }
    let index = keys
        .iter()
        .map(|k| k.1)
        .find(|i| !rejected.contains(i))
        .expect("the smallest pooled candidate is never rejected");
    let chosen = &cands[index];
    let chosen_values = keys.iter().find(|k| k.1 == index).map(|k| k.0.clone()).unwrap_or_default();

    let best_regular = pool
        .iter()
        .copied()
        .filter(|&i| cands[i].is_regular())
        .min_by(|&a, &b| (cands[a].size, cands[a].id.as_str()).cmp(&(cands[b].size, cands[b].id.as_str())));
    let over_cap = cands
        .iter()
        .filter(|c| !c.admissible && c.is_regular() && c.excluded_by.is_some())
        .min_by(|x, y| (x.size, x.id.as_str()).cmp(&(y.size, y.id.as_str())));
    let reason = if !rejected.is_empty() {
        ChoiceReason::SizeGuard
    } else if obj.measures.first() == Some(&Measure::Size) {
        ChoiceReason::Economy
    } else if chosen.is_regular()
        || (profile_used == "composed"
            && composed.as_ref().is_some_and(|all| {
                pool.iter()
                    .position(|&i| i == index)
                    .is_some_and(|j| all[j].iter().all(|(_, o)| *o == 0))
            }))
    {
        ChoiceReason::Regular
    } else {
        ChoiceReason::LowestPole
    };
    let reason = if pool.len() == 1 && reason != ChoiceReason::SizeGuard { ChoiceReason::Only } else { reason };
    let regular_alternative = if chosen.is_regular() {
        RegularAlternative::Chosen
    } else if let Some(r) = best_regular {
        RegularAlternative::RejectedForSize {
            id: cands[r].id.clone(),
            size: cands[r].size,
            ratio: ratio_text(cands[r].size, smallest),
        }
    } else if let Some(c) = over_cap {
        RegularAlternative::OverCap { id: c.id.clone(), cap: c.excluded_by.clone().unwrap_or_default(), size: c.size }
    } else {
        RegularAlternative::None
    };
    let criteria_order = if obj.measures.first() == Some(&Measure::Size) {
        CriteriaOrder::EconomyFirst
    } else {
        CriteriaOrder::RegularFirst
    };
    Ok(Some(Choice {
        index,
        id: chosen.id.clone(),
        reason,
        chosen_size: chosen.size,
        smallest_size: smallest,
        chosen_pole_order: chosen.pole_order,
        max_size_ratio: obj.max_size_ratio.text(),
        regular_alternative,
        criteria_order,
        profile_used,
        measures: obj.measures.iter().map(|m| m.word()).collect(),
        chosen_measures: chosen_values,
        rejected_for_size: rejected
            .iter()
            .map(|&i| (cands[i].id.clone(), ratio_text(cands[i].size, smallest)))
            .collect(),
        lacking_composed,
    }))
}

impl Choice {
    /// The report line (addendum 7: the parameter value in force and both sizes, chosen / smallest admissible).
    pub fn report_line(&self) -> String {
        let alt = match &self.regular_alternative {
            RegularAlternative::Chosen => "chosen is regular".to_string(),
            RegularAlternative::RejectedForSize { size, ratio, .. } => {
                format!(
                    "regular alternative exists, rejected for size: {size} terms, ratio {ratio}"
                )
            }
            RegularAlternative::OverCap { cap, size, .. } => {
                format!("regular candidate exists, over {cap} ({size} terms)")
            }
            RegularAlternative::None => "no regular candidate".to_string(),
        };
        let pole = self
            .chosen_pole_order
            .map_or("unknown".to_string(), |p| p.to_string());
        format!(
            "choice {}: reason {}, pole order {pole}, max_size_ratio {}, chosen size {}, smallest admissible size {}; {alt}; \
             profile {} (measures {})",
            self.id,
            self.reason.word(),
            self.max_size_ratio,
            self.chosen_size,
            self.smallest_size,
            self.profile_used,
            self.measures.join(",")
        )
    }

    /// The same as a JSON record (decision files, `centre_scan.chosen`, relation-entry diagnostics).
    pub fn to_json(&self) -> Value {
        let alt = match &self.regular_alternative {
            RegularAlternative::Chosen => json!({"status": "chosen"}),
            RegularAlternative::RejectedForSize { id, size, ratio } => {
                json!({"status": "rejected_for_size", "id": id, "size": size, "ratio": ratio})
            }
            RegularAlternative::OverCap { id, cap, size } => {
                json!({"status": "over_cap", "id": id, "lost_to": cap, "size": size})
            }
            RegularAlternative::None => json!({"status": "none"}),
        };
        json!({"chosen": self.id, "reason": self.reason.word(), "pole_order": self.chosen_pole_order,
            "max_size_ratio": self.max_size_ratio, "chosen_size": self.chosen_size, "smallest_size": self.smallest_size,
            "criteria_order": match self.criteria_order { CriteriaOrder::RegularFirst => "regular_first", CriteriaOrder::EconomyFirst => "economy_first" },
            "regular_alternative": alt, "profile_used": self.profile_used, "measures": self.measures,
            "chosen_measures": self.chosen_measures, "lacking_composed": self.lacking_composed,
            "rejected_for_size": self.rejected_for_size.iter().map(|(i, r)| json!({"id": i, "ratio": r})).collect::<Vec<_>>()})
    }
}
