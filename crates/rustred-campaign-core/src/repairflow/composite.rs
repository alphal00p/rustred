//! COMPOSITE DESCENT (TRICKS 1 + 3; interfaces/repairflow.md 17): a symmetry service of key K by group element g is
//! judged as the COMPOSED relation, never as the alias edge K -> gK.
//!
//! 1. The element's expanded symmetry rows at K (a non-pure element gives a multi-term row) are solved for K: exact
//!    echelon form over Q with the columns ordered K first, then every other key from the highest to the lowest under
//!    the node comparator, so the K row keeps as few higher keys as the rows allow.
//! 2. Every term that is not strictly lower than K is replaced, HIGHEST FIRST, by the catalogue rule that serves it
//!    (first match of the table set, [`Catalogue`]); the contributions of every replacement are accumulated before a
//!    term is looked at, so an exact cancellation is seen (a composed coefficient that is exactly zero is NOT an edge).
//!    A term no rule serves = the element fails at K. Each replacement may itself produce terms that are not lower:
//!    they are replaced again while the composition depth allows (`max_depth`, spec data, default 1 = the image then
//!    one rule).
//! 3. K may reappear (from a non-pure image or a rule's right-hand side) with coefficient c: the relation is solved for
//!    K through (1 - c); c = 1 exactly = no relation from this element.
//! 4. ACCEPT iff every nonzero term of the composed relation is strictly lower than K under the caller's `lower`
//!    predicate (the import's descent test: the FULL node comparator on top-sector keys, a positive numerator never
//!    lower). What is shipped is that relation at K, bound to the records used (record sha256 of each rule, the group
//!    digest, the element): an ordinary descending relation for the import and the closing loop.
//!
//! The coefficient arithmetic is the caller's exact backend ([`ComposeAlgebra`]): rational functions in d through
//! Symbolica in `rustred-campaign-algebra` (L11). The row echelon over Q is the core's `CheckAlgebra::q_rref`. No
//! node, order, depth or size is known here (L8): all are arguments.

use std::cmp::Ordering;
use std::collections::{BTreeMap, BTreeSet, HashMap};

use crate::replay_base::algebra::{CheckAlgebra, Q};

/// The exact coefficient domain of a composition (a field: rational functions in d, or Q for constructed tests).
pub trait ComposeAlgebra {
    type E: Clone + std::fmt::Debug;
    fn from_q(&self, q: Q) -> Result<Self::E, String>;
    fn add(&self, a: &Self::E, b: &Self::E) -> Result<Self::E, String>;
    fn mul(&self, a: &Self::E, b: &Self::E) -> Result<Self::E, String>;
    fn neg(&self, a: &Self::E) -> Result<Self::E, String>;
    /// `a / b`; called only with `b` exactly nonzero.
    fn div(&self, a: &Self::E, b: &Self::E) -> Result<Self::E, String>;
    /// EXACT zero test (`Err` = the backend cannot decide: the element is refused, never guessed).
    fn is_zero(&self, a: &Self::E) -> Result<bool, String>;
    /// The canonical text of a coefficient (the shipped record's coefficient field).
    fn text(&self, a: &Self::E) -> String;
}

/// The catalogue record a term was replaced by.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct RuleUse {
    pub table: usize,
    pub record: usize,
    pub source: String,
    pub label: i64,
    pub record_sha256: String,
}

/// What serves a term (first match of the catalogue).
#[derive(Clone, Debug)]
pub enum Service<E> {
    /// The rule and its right-hand side at the term (keys and exact coefficients, record order).
    Rule {
        rule: RuleUse,
        rhs: Vec<(Vec<i32>, E)>,
    },
    /// No catalogue rule serves the term (uncovered, blocked, undecided, outside the chart): the reason.
    Unserved(String),
}

/// The first-match catalogue at a key, with exact coefficients (the backend reads the record's own texts).
pub trait Catalogue<A: ComposeAlgebra> {
    fn serve(&self, alg: &A, term: &[i32]) -> Result<Service<A::E>, String>;
}

/// Options of a composition (spec data; interfaces/repairflow.md 17).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ComposeOptions {
    /// How many catalogue replacements may be chained below one image term (1 = the image, then one rule).
    pub max_depth: usize,
    /// Safety bound of the composed relation's size during the composition (terms alive at once).
    pub max_terms: usize,
    /// Safety bound of the number of replacements of one composition.
    pub max_replacements: usize,
}

impl Default for ComposeOptions {
    fn default() -> Self {
        ComposeOptions {
            max_depth: 1,
            max_terms: 100_000,
            max_replacements: 10_000,
        }
    }
}

/// One replacement made by a composition: the term, its depth below the image, the rule.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Replacement {
    pub term: Vec<i32>,
    pub depth: usize,
    pub rule: RuleUse,
}

/// An ACCEPTED composed relation of one element at the key.
#[derive(Clone, Debug)]
pub struct Composite<E> {
    pub element: usize,
    /// `I(key) = sum c I(k)`, every `k` strictly lower than the key; highest key first.
    pub rhs: Vec<(Vec<i32>, E)>,
    /// The image terms (after solving the element's rows for the key), highest first: `I(key) = sum a I(t)`.
    pub image: Vec<(Vec<i32>, Q)>,
    /// The catalogue replacements, in the order made (highest term first).
    pub replacements: Vec<Replacement>,
    /// The coefficient of the key on the right before solving for it (None = the key did not reappear).
    pub key_coefficient: Option<E>,
    /// Terms whose composed coefficient was exactly zero (dropped: not edges).
    pub dropped_zero: Vec<Vec<i32>>,
}

impl<E> Composite<E> {
    /// The element's image alone already descends (route 0b's plain relation).
    pub fn is_plain(&self) -> bool {
        self.replacements.is_empty()
    }
}

/// Why an element gives no accepted relation at the key.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Refusal {
    /// The element's rows do not contain the key as a pivot (no relation for the key from this element).
    NoKeyRow,
    /// A term with a nonzero composed coefficient is not strictly lower than the key and the depth is exhausted.
    NotLower { term: Vec<i32>, depth: usize },
    /// A term that is not lower has no serving catalogue rule.
    Unserved { term: Vec<i32>, why: String },
    /// The key reappears with coefficient exactly 1: (1 - c) = 0.
    KeyCancels,
    /// A catalogue rule produced a term that is not lower than the term it replaced (the rule does not descend).
    RuleNotDescending { term: Vec<i32>, produced: Vec<i32> },
    /// A safety bound of [`ComposeOptions`] was hit.
    TooLarge(String),
    /// The exact backend failed or could not decide (never guessed).
    Algebra(String),
}

impl Refusal {
    /// The class word of the refusal (records, counts).
    pub fn class(&self) -> &'static str {
        match self {
            Refusal::NoKeyRow => "no_key_row",
            Refusal::NotLower { .. } => "not_lower",
            Refusal::Unserved { .. } => "unserved_image_term",
            Refusal::KeyCancels => "key_cancels",
            Refusal::RuleNotDescending { .. } => "rule_not_descending",
            Refusal::TooLarge(_) => "too_large",
            Refusal::Algebra(_) => "algebra",
        }
    }
}

/// The outcome of one element at the key.
#[derive(Clone, Debug)]
pub enum ElementOutcome<E> {
    Accepted(Composite<E>),
    Refused { element: usize, why: Refusal },
}

impl<E> ElementOutcome<E> {
    pub fn element(&self) -> usize {
        match self {
            ElementOutcome::Accepted(c) => c.element,
            ElementOutcome::Refused { element, .. } => *element,
        }
    }
}

/// Solve the element's rows for the key: `I(key) = sum a I(t)` (None = the key is not a pivot of the rows).
pub fn image_of<QA: CheckAlgebra>(
    qalg: &QA,
    rows: &[Vec<(Vec<i32>, Q)>],
    key: &[i32],
    cmp: &dyn Fn(&[i32], &[i32]) -> Ordering,
) -> Result<Option<Vec<(Vec<i32>, Q)>>, String> {
    let mut cols: BTreeSet<Vec<i32>> = BTreeSet::new();
    for r in rows {
        for (k, _) in r {
            cols.insert(k.clone());
        }
    }
    if !cols.contains(key) {
        return Ok(None);
    }
    let mut order: Vec<Vec<i32>> = cols.into_iter().filter(|k| k.as_slice() != key).collect();
    order.sort_by(|a, b| cmp(b, a));
    order.insert(0, key.to_vec());
    let index: HashMap<&Vec<i32>, usize> = order.iter().enumerate().map(|(i, k)| (k, i)).collect();
    let n = order.len();
    let mut dense: Vec<Vec<Q>> = Vec::with_capacity(rows.len());
    for v in rows {
        let mut row = vec![Q::ZERO; n];
        for (k, c) in v {
            let j = index[k];
            row[j] = qalg.q_add(row[j], *c)?;
        }
        dense.push(row);
    }
    let (rref, pivots) = qalg.q_rref(&dense, n).map_err(|e| format!("{e:?}"))?;
    let Some(pr) = pivots.iter().position(|&p| p == 0) else {
        return Ok(None);
    };
    let mut out = vec![];
    for (j, c) in rref[pr].iter().enumerate().skip(1) {
        if !c.is_zero() {
            out.push((order[j].clone(), qalg.q_neg(*c)?));
        }
    }
    Ok(Some(out))
}

struct Acc<E> {
    coef: E,
    depth: usize,
}

/// Compose ONE element at the key (module documentation). `rows` = the element's expanded rows at the key.
#[allow(clippy::too_many_arguments)]
pub fn compose_element<QA: CheckAlgebra, A: ComposeAlgebra, C: Catalogue<A>>(
    qalg: &QA,
    alg: &A,
    cat: &C,
    element: usize,
    rows: &[Vec<(Vec<i32>, Q)>],
    key: &[i32],
    lower: &dyn Fn(&[i32], &[i32]) -> bool,
    cmp: &dyn Fn(&[i32], &[i32]) -> Ordering,
    opts: &ComposeOptions,
) -> ElementOutcome<A::E> {
    let refuse = |why: Refusal| ElementOutcome::Refused { element, why };
    let image = match image_of(qalg, rows, key, cmp) {
        Ok(Some(i)) => i,
        Ok(None) => return refuse(Refusal::NoKeyRow),
        Err(e) => return refuse(Refusal::Algebra(e)),
    };
    match compose_image(alg, cat, &image, key, lower, cmp, opts) {
        Ok(Ok(mut c)) => {
            c.element = element;
            ElementOutcome::Accepted(c)
        }
        Ok(Err(why)) => refuse(why),
        Err(e) => refuse(Refusal::Algebra(e)),
    }
}

/// Compose a given image `I(key) = sum a I(t)` (the element field of the result is 0: set by the caller).
pub fn compose_image<A: ComposeAlgebra, C: Catalogue<A>>(
    alg: &A,
    cat: &C,
    image: &[(Vec<i32>, Q)],
    key: &[i32],
    lower: &dyn Fn(&[i32], &[i32]) -> bool,
    cmp: &dyn Fn(&[i32], &[i32]) -> Ordering,
    opts: &ComposeOptions,
) -> Result<Result<Composite<A::E>, Refusal>, String> {
    let mut acc: BTreeMap<Vec<i32>, Acc<A::E>> = BTreeMap::new();
    let mut kc: Option<A::E> = None;
    let add_to = |acc: &mut BTreeMap<Vec<i32>, Acc<A::E>>,
                  kc: &mut Option<A::E>,
                  k: &[i32],
                  c: A::E,
                  depth: usize|
     -> Result<(), String> {
        if k == key {
            *kc = Some(match kc.take() {
                None => c,
                Some(o) => alg.add(&o, &c)?,
            });
            return Ok(());
        }
        match acc.get_mut(k) {
            Some(a) => {
                a.coef = alg.add(&a.coef, &c)?;
                a.depth = a.depth.min(depth);
            }
            None => {
                acc.insert(k.to_vec(), Acc { coef: c, depth });
            }
        }
        Ok(())
    };
    for (t, a) in image {
        add_to(&mut acc, &mut kc, t, alg.from_q(*a)?, 0)?;
    }
    let mut replacements: Vec<Replacement> = vec![];
    let mut dropped: Vec<Vec<i32>> = vec![];
    loop {
        if acc.len() > opts.max_terms {
            return Ok(Err(Refusal::TooLarge(format!(
                "{} terms alive > max_terms {}",
                acc.len(),
                opts.max_terms
            ))));
        }
        // the highest term that is not lower than the key
        let top = acc
            .keys()
            .filter(|t| !lower(t, key))
            .max_by(|a, b| cmp(a, b))
            .cloned();
        let Some(t) = top else { break };
        let a = acc.remove(&t).expect("present");
        if alg.is_zero(&a.coef)? {
            dropped.push(t);
            continue;
        }
        if a.depth >= opts.max_depth {
            return Ok(Err(Refusal::NotLower {
                term: t,
                depth: a.depth,
            }));
        }
        if replacements.len() >= opts.max_replacements {
            return Ok(Err(Refusal::TooLarge(format!(
                "more than {} replacements",
                opts.max_replacements
            ))));
        }
        match cat.serve(alg, &t)? {
            Service::Unserved(why) => return Ok(Err(Refusal::Unserved { term: t, why })),
            Service::Rule { rule, rhs } => {
                for (u, r) in rhs {
                    // a rule term whose coefficient is exactly zero at the term is NOT an edge (e.g. a positive
                    // numerator term, TRICKS 10): skipped before any test
                    if alg.is_zero(&r)? {
                        continue;
                    }
                    // termination: a produced term that must be replaced again is lower than the one it came from
                    if u != key && !lower(&u, key) && cmp(&u, &t) != Ordering::Less {
                        return Ok(Err(Refusal::RuleNotDescending {
                            term: t,
                            produced: u,
                        }));
                    }
                    let c = alg.mul(&a.coef, &r)?;
                    add_to(&mut acc, &mut kc, &u, c, a.depth + 1)?;
                }
                replacements.push(Replacement {
                    term: t,
                    depth: a.depth,
                    rule,
                });
            }
        }
    }
    // every remaining term is lower than the key: drop the exact zeros, solve for the key
    let mut rhs: Vec<(Vec<i32>, A::E)> = vec![];
    for (k, a) in acc {
        if alg.is_zero(&a.coef)? {
            dropped.push(k);
        } else {
            rhs.push((k, a.coef));
        }
    }
    if let Some(c) = &kc {
        let one = alg.from_q(Q::ONE)?;
        let den = alg.add(&one, &alg.neg(c)?)?;
        if alg.is_zero(&den)? {
            return Ok(Err(Refusal::KeyCancels));
        }
        for (_, v) in rhs.iter_mut() {
            *v = alg.div(v, &den)?;
        }
    }
    rhs.sort_by(|a, b| cmp(&b.0, &a.0));
    let mut image: Vec<(Vec<i32>, Q)> = image.to_vec();
    image.sort_by(|a, b| cmp(&b.0, &a.0));
    Ok(Ok(Composite {
        element: 0,
        rhs,
        image,
        replacements,
        key_coefficient: kc,
        dropped_zero: dropped,
    }))
}

/// Every element at the key: all outcomes (every admissible alternative is kept for the selection function of lane
/// poles) and the index of the preferred accepted one (fewest terms; ties: the lower highest term under the
/// comparator, then fewer replacements, then the element index).
#[derive(Clone, Debug)]
pub struct KeyComposites<E> {
    pub outcomes: Vec<ElementOutcome<E>>,
    pub best: Option<usize>,
}

impl<E> KeyComposites<E> {
    pub fn best(&self) -> Option<&Composite<E>> {
        self.best.and_then(|i| match &self.outcomes[i] {
            ElementOutcome::Accepted(c) => Some(c),
            ElementOutcome::Refused { .. } => None,
        })
    }
    /// The accepted alternatives.
    pub fn accepted(&self) -> impl Iterator<Item = &Composite<E>> {
        self.outcomes.iter().filter_map(|o| match o {
            ElementOutcome::Accepted(c) => Some(c),
            ElementOutcome::Refused { .. } => None,
        })
    }
}

/// Compose every element (rows grouped by element) at the key and pick the preferred accepted relation.
#[allow(clippy::too_many_arguments)]
pub fn compose_all<QA: CheckAlgebra, A: ComposeAlgebra, C: Catalogue<A>>(
    qalg: &QA,
    alg: &A,
    cat: &C,
    rows_by_element: &BTreeMap<usize, Vec<Vec<(Vec<i32>, Q)>>>,
    key: &[i32],
    lower: &dyn Fn(&[i32], &[i32]) -> bool,
    cmp: &dyn Fn(&[i32], &[i32]) -> Ordering,
    opts: &ComposeOptions,
) -> KeyComposites<A::E> {
    let outcomes: Vec<ElementOutcome<A::E>> = rows_by_element
        .iter()
        .map(|(e, rs)| compose_element(qalg, alg, cat, *e, rs, key, lower, cmp, opts))
        .collect();
    let best = preferred(&outcomes, cmp);
    KeyComposites { outcomes, best }
}

/// The preferred accepted outcome (see [`KeyComposites`]).
pub fn preferred<E>(
    outcomes: &[ElementOutcome<E>],
    cmp: &dyn Fn(&[i32], &[i32]) -> Ordering,
) -> Option<usize> {
    let mut best: Option<usize> = None;
    for (i, o) in outcomes.iter().enumerate() {
        let ElementOutcome::Accepted(c) = o else {
            continue;
        };
        let better = match best {
            None => true,
            Some(b) => {
                let ElementOutcome::Accepted(bc) = &outcomes[b] else {
                    unreachable!()
                };
                let top_cmp = match (c.rhs.first(), bc.rhs.first()) {
                    (Some(x), Some(y)) => cmp(&x.0, &y.0),
                    (None, Some(_)) => Ordering::Less,
                    (Some(_), None) => Ordering::Greater,
                    (None, None) => Ordering::Equal,
                };
                c.rhs
                    .len()
                    .cmp(&bc.rhs.len())
                    .then(top_cmp)
                    .then(c.replacements.len().cmp(&bc.replacements.len()))
                    .then(c.element.cmp(&bc.element))
                    == Ordering::Less
            }
        };
        if better {
            best = Some(i);
        }
    }
    best
}

/// The import's descent test of a right-hand-side key `k` against the target (interfaces/import.md 5.3): a positive
/// numerator is never lower; a key outside the top sector (some line <= 0) is lower; a top-sector key is lower iff the
/// node comparator says so. `is_line[i]` = slot i is a propagator line of the frame.
pub fn import_lower(
    is_line: &[bool],
    k: &[i32],
    target: &[i32],
    cmp: &dyn Fn(&[i32], &[i32]) -> Ordering,
) -> bool {
    let posnum = k.iter().zip(is_line).any(|(&x, &l)| !l && x > 0);
    if posnum {
        return false;
    }
    let top = k
        .iter()
        .zip(is_line)
        .all(|(&x, &l)| if l { x >= 1 } else { x <= 0 });
    !top || cmp(k, target) == Ordering::Less
}

/// Exact rationals as a [`ComposeAlgebra`] (constructed tests; compositions whose rules have constant coefficients).
pub struct QField<'a, QA: CheckAlgebra>(pub &'a QA);

impl<QA: CheckAlgebra> ComposeAlgebra for QField<'_, QA> {
    type E = Q;
    fn from_q(&self, q: Q) -> Result<Q, String> {
        Ok(q)
    }
    fn add(&self, a: &Q, b: &Q) -> Result<Q, String> {
        self.0.q_add(*a, *b)
    }
    fn mul(&self, a: &Q, b: &Q) -> Result<Q, String> {
        self.0.q_mul(*a, *b)
    }
    fn neg(&self, a: &Q) -> Result<Q, String> {
        self.0.q_neg(*a)
    }
    fn div(&self, a: &Q, b: &Q) -> Result<Q, String> {
        self.0.q_div(*a, *b)
    }
    fn is_zero(&self, a: &Q) -> Result<bool, String> {
        Ok(a.is_zero())
    }
    fn text(&self, a: &Q) -> String {
        if a.d == 1 {
            a.n.to_string()
        } else {
            format!("{}/{}", a.n, a.d)
        }
    }
}

/// The verified group's expanded symmetry rows at the key, grouped by element (the import's derivation,
/// `replay_base::symrows::rows_at_with`, as route 0b).
pub fn rows_by_element<QA: CheckAlgebra>(
    qalg: &QA,
    group: &crate::replay_base::symrows::VerifiedGroup,
    key: &[i32],
    max_terms: usize,
) -> Result<BTreeMap<usize, Vec<Vec<(Vec<i32>, Q)>>>, String> {
    use crate::replay_base::symrows::{SymRowMode, rows_at_with};
    let seed = (0u64, key.iter().map(|&x| x as i64).collect::<Vec<i64>>());
    let rows = rows_at_with(qalg, group, &seed, SymRowMode::Expanded, max_terms)
        .map_err(|e| format!("symmetry rows at the key: {e}"))?;
    let mut by: BTreeMap<usize, Vec<Vec<(Vec<i32>, Q)>>> = BTreeMap::new();
    for r in &rows {
        let mut v = vec![];
        for ((mask, k), c) in &r.terms {
            if *mask != 0 {
                return Err("a symbolic term at an integer seed".into());
            }
            let k32: Vec<i32> = k
                .iter()
                .map(|&x| i32::try_from(x).map_err(|_| "a row key outside i32".to_string()))
                .collect::<Result<_, _>>()?;
            v.push((k32, *c));
        }
        if !v.is_empty() {
            by.entry(r.element).or_default().push(v);
        }
    }
    Ok(by)
}

fn key_text(k: &[i32]) -> String {
    crate::termmin::key_text(k)
}

fn q_text(q: &Q) -> String {
    if q.d == 1 {
        q.n.to_string()
    } else {
        format!("{}/{}", q.n, q.d)
    }
}

/// The SHIPPED form of an accepted composite: one point record of a `supplied_relations` table (the import reads the
/// keys and checks descent itself; the coefficients are the exact composed ones). `chart` = the frame's mask string.
pub fn shipped_record<A: ComposeAlgebra>(
    alg: &A,
    key: &[i32],
    c: &Composite<A::E>,
    chart: &str,
    index: usize,
) -> serde_json::Value {
    let rhs: Vec<serde_json::Value> = c
        .rhs
        .iter()
        .map(|(k, v)| serde_json::json!([key_text(k), alg.text(v)]))
        .collect();
    serde_json::json!({"type": "rule", "chart": chart, "index": index, "kind": "symmetry-composite",
        "case": "point", "fixed": key, "equations": [], "target": key_text(key), "rhs": rhs, "exceptions": [],
        "dispatch": "Point", "witness_loci": [], "symmetry_elements": [c.element]})
}

/// The VERIFICATION line of a shipped composite (what it is bound to: the group digest, the element, the image, every
/// catalogue record used by its record sha256, the key coefficient solved for, the exact zeros dropped).
pub fn verification_record<A: ComposeAlgebra>(
    alg: &A,
    key: &[i32],
    c: &Composite<A::E>,
    group_digest: &str,
    producer: &str,
) -> serde_json::Value {
    serde_json::json!({"schema": COMPOSITE_SCHEMA, "target": key_text(key), "group_digest": group_digest,
        "element": c.element,
        "image": c.image.iter().map(|(k, q)| serde_json::json!([key_text(k), q_text(q)])).collect::<Vec<_>>(),
        "replacements": c.replacements.iter().map(|r| serde_json::json!({"term": key_text(&r.term), "depth": r.depth,
            "source": r.rule.source, "label": r.rule.label, "record_sha256": r.rule.record_sha256})).collect::<Vec<_>>(),
        "key_coefficient": c.key_coefficient.as_ref().map(|v| alg.text(v)),
        "dropped_zero": c.dropped_zero.iter().map(|k| key_text(k)).collect::<Vec<_>>(),
        "rhs_terms": c.rhs.len(), "plain": c.is_plain(), "producer": producer})
}

/// Schema of a composite's verification line.
pub const COMPOSITE_SCHEMA: &str = "rustred.campaign.symmetry-composite.v1";

/// Schema of the per-key record of ALL acceptable choices (interfaces/repairflow.md 17.3; input of lanes regions /
/// pieces / poles).
pub const KEY_CHOICES_SCHEMA: &str = "rustred.campaign.repairflow-keychoices.v1";

/// One acceptable choice of a key (keys only + identities; the coefficients stay with the producer's shipped record).
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Choice {
    /// Stable id within the key: `sym:e<element>` (route 0b / composite), `tworow:e<element>:<shift>`, `core:<step>`.
    pub id: String,
    /// `plain` | `composite` | `two_row` | `core` | ...
    pub kind: String,
    /// The group element (symmetry kinds), else None.
    pub element: Option<usize>,
    /// The composed-rule FORM: comparable across keys (`e<element>` or `e<element>+<source>#<label>,..` with the
    /// distinct catalogue records used, sorted): what a region form is fitted on.
    pub form: String,
    /// Right-hand-side keys, highest first (every one strictly lower than the key: checked by the producer).
    pub rhs: Vec<Vec<i32>>,
    /// Catalogue records used (`record_sha256`), sorted, distinct.
    pub records: Vec<String>,
    /// The key reappeared and was solved for ((1 - c) divided out).
    pub key_solved: bool,
    /// Regularity at d = 4 of this choice (lane poles fills it; None = not evaluated).
    pub regular_at_4: Option<bool>,
}

/// The per-key record: every acceptable choice, every refusal, the preferred choice (fewest terms; lane poles'
/// selection function may override among `choices`).
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct KeyChoices {
    pub schema: String,
    pub key: Vec<i32>,
    /// Face label (`worklist::items::Face::of_key`).
    pub face: String,
    /// The face's free slots (dotted lines >= 2, numerators <= -1 of the key).
    pub face_slots: Vec<usize>,
    pub choices: Vec<Choice>,
    /// `[element, refusal class]` of every element that gave no acceptable choice.
    pub refused: Vec<(usize, String)>,
    /// Index into `choices`.
    pub preferred: Option<usize>,
}

impl Choice {
    /// The choice of an accepted composite.
    pub fn of_composite<E>(c: &Composite<E>) -> Choice {
        let mut recs: Vec<&RuleUse> = c.replacements.iter().map(|r| &r.rule).collect();
        recs.sort();
        recs.dedup();
        let form = if recs.is_empty() {
            format!("e{}", c.element)
        } else {
            format!(
                "e{}+{}",
                c.element,
                recs.iter()
                    .map(|r| format!("{}#{}", r.source, r.label))
                    .collect::<Vec<_>>()
                    .join(",")
            )
        };
        let mut records: Vec<String> = recs.iter().map(|r| r.record_sha256.clone()).collect();
        records.dedup();
        Choice {
            id: format!("sym:e{}", c.element),
            kind: if c.is_plain() { "plain" } else { "composite" }.into(),
            element: Some(c.element),
            form,
            rhs: c.rhs.iter().map(|(k, _)| k.clone()).collect(),
            records,
            key_solved: c.key_coefficient.is_some(),
            regular_at_4: None,
        }
    }
}

impl KeyChoices {
    /// The record of a key's composite outcomes (`lines[i]` = slot i is a line).
    pub fn of_composites<E>(key: &[i32], kc: &KeyComposites<E>, lines: &[bool]) -> KeyChoices {
        let mut choices = vec![];
        let mut refused = vec![];
        let mut preferred = None;
        for (i, o) in kc.outcomes.iter().enumerate() {
            match o {
                ElementOutcome::Accepted(c) => {
                    if kc.best == Some(i) {
                        preferred = Some(choices.len());
                    }
                    choices.push(Choice::of_composite(c));
                }
                ElementOutcome::Refused { element, why } => {
                    refused.push((*element, why.class().to_string()))
                }
            }
        }
        let face_slots: Vec<usize> = (0..key.len())
            .filter(|&i| if lines[i] { key[i] >= 2 } else { key[i] <= -1 })
            .collect();
        KeyChoices {
            schema: KEY_CHOICES_SCHEMA.into(),
            key: key.to_vec(),
            face: crate::worklist::items::Face::of_key(key, lines)
                .map(|f| f.label())
                .unwrap_or_default(),
            face_slots,
            choices,
            refused,
            preferred,
        }
    }
}
