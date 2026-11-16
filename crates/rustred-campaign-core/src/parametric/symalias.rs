//! Symmetry ALIASES for gap pieces (lane t162face; TRICKS items 1-4 in parametric form; interfaces/parametric.md
//! section "symalias").
//!
//! A gap piece P (a case: fixed slots, affine case equations, domain bounds; typically a LOCUS piece of the coverage
//! partition that no record serves, e.g. where the covering rule's coefficient denominator vanishes identically in d)
//! is served by the one-term identity `I(k) = I(g k)` when
//!   (a) EXACTNESS: g is an element of the node's verified automorphism group whose action is PURE on every slot that
//!       can be non-zero on P (every chart line slot, every numerator slot not fixed at 0): each such slot i has an
//!       exact, unsigned image `D_i(k T) = D_t(k)`, chart side kept, targets distinct; every other slot is fixed at 0
//!       on P. Then the integrand maps factor by factor under `k -> k T` (unit Jacobian) and `I(k) = I(g k)` holds at
//!       every key of P. The equivalent engine `sym` record is written as well, so that the consumer's independent
//!       verifier (`import::symmetry::verify_sym_record`, interfaces/coord-sym-records.md section 5, steps 1-6) and
//!       the engine's own replay check (`rr_node replay`, `check_symmetry_record`) certify it;
//!   (b) DESCENT on P's WHOLE domain under the node comparator: [`permuted_descent`] of the image pattern against the
//!       target, on the [`CaseDomain`] the core reader builds from the alias record itself (so the checked domain IS
//!       the shipped one: case, equations, `domain_bounds`);
//!   (c) the image g P is served at first match on its whole extent: decided by the CALLER's coverage certificate
//!       re-run with the alias records appended (they are ordinary `rule` records: one right-hand-side term, unit
//!       coefficient, the piece's case and bounds, provenance in `symmetry_origin` / `kind`, both informational
//!       members of the reader). Appended records change no first match where an earlier record applies.
//!
//! The engine's own sym-gen (`rr_node generate --symmetry`) cannot serve such a piece when its case needs BOUNDS: an
//! engine case is fixed slots + affine equations only, so a locus case contains its degenerate end point (every
//! locus line at power 1), which is a fixed point of the elements that keep the locus' numerators, hence never
//! "strictly lower" there. This module carries the bounds (`domain_bounds`), as the helper's one-term symmetry rules
//! did (F3 / F4 on T178, proved by `permuted`).
//!
//! Generic (L8): no node, arity, slot or count is named; the group, the comparator, the frame and the pieces are input.

use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use crate::domain::{CaseDomain, DomainOptions};
use crate::key::NodeFrame;
use crate::loci::contain::ProofBudget;
use crate::order::NodeComparator;
use crate::parametric::descent::{EmptyState, emptiness};
use crate::parametric::permuted::{PermutedVerdict, permuted_descent};
use crate::poly::Poly;
use crate::record::{ReadOptions, RecordReader, RhsMode, TableLine};
use crate::replay_base::symrows::{SlotImage, SymElement, SymGroup};

/// Schema of a symalias evidence file (JSONL: header, one line per candidate, summary).
pub const SYMALIAS_SCHEMA: &str = "rustred.campaign.symalias.v1";
/// Version of the decision (bump when a verdict can change for the same input).
pub const SYMALIAS_VERSION: u32 = 1;
/// `kind` member of an alias rule record (informational for the reader and the engine).
pub const ALIAS_KIND: &str = "symmetry_alias";

/// One gap piece to serve: the members of a table rule's CASE (as the engine writes them).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Piece {
    /// Free-text name (provenance only).
    pub name: String,
    /// `null` = free slot (symbol `n<slot>`).
    pub fixed: Vec<Option<i64>>,
    /// Affine case equations in the slot symbols (engine syntax, e.g. `n9+2*n1-1`).
    #[serde(default)]
    pub equations: Vec<String>,
    /// The engine's `domain_bounds` object (`{"min": [..], "max": [..]}`), inclusive bounds on the case variables.
    #[serde(default)]
    pub domain_bounds: Option<Value>,
}

/// The canonical target pattern of a case: the fixed value, or `n<slot>` on a free slot.
pub fn canonical_target(fixed: &[Option<i64>]) -> String {
    let parts: Vec<String> = fixed
        .iter()
        .enumerate()
        .map(|(i, f)| match f {
            Some(v) => v.to_string(),
            None => format!("n{i}"),
        })
        .collect();
    format!("I({})", parts.join(","))
}

/// Slots that can be non-zero on a case: every chart line slot, every numerator slot not fixed at 0 (a case equation
/// that would force a free numerator to 0 is not used: stricter, never unsound).
pub fn may_be_nonzero(fixed: &[Option<i64>], chart: &[bool]) -> Vec<bool> {
    fixed
        .iter()
        .zip(chart)
        .map(|(f, &line)| line || *f != Some(0))
        .collect()
}

/// The slot map of `el` on a case when the element is PURE there: `Some(map)` with `map[i] = Some(t)` for every slot
/// that can be non-zero (exact unsigned image onto a slot of the same chart side, targets distinct) and `None` on the
/// other slots; `None` when some slot that can be non-zero has no such image.
pub fn pure_map(
    el: &SymElement,
    fixed: &[Option<i64>],
    chart: &[bool],
) -> Option<Vec<Option<usize>>> {
    let n = fixed.len();
    if el.action.len() != n || chart.len() != n {
        return None;
    }
    let nz = may_be_nonzero(fixed, chart);
    let mut map = vec![None; n];
    let mut used = vec![false; n];
    for i in 0..n {
        if !nz[i] {
            continue;
        }
        match &el.action[i] {
            SlotImage::Exact {
                target,
                flip: false,
            } if *target < n && chart[*target] == chart[i] && !used[*target] => {
                used[*target] = true;
                map[i] = Some(*target);
            }
            _ => return None,
        }
    }
    Some(map)
}

/// The image pattern `g(target)`: slot `t = map[i]` carries the target's entry of slot i; every other slot is 0.
pub fn image_pattern(fixed: &[Option<i64>], map: &[Option<usize>]) -> String {
    let n = fixed.len();
    let mut out = vec!["0".to_string(); n];
    for (i, f) in fixed.iter().enumerate() {
        if let Some(t) = map.get(i).copied().flatten()
            && t < n
        {
            out[t] = match f {
                Some(v) => v.to_string(),
                None => format!("n{i}"),
            };
        }
    }
    format!("I({})", out.join(","))
}

fn action_json(el: &SymElement) -> Value {
    Value::Array(
        el.action
            .iter()
            .map(|a| match a {
                SlotImage::Exact { target, flip } => json!([target, flip]),
                _ => Value::Null,
            })
            .collect(),
    )
}

/// The alias as a `rule` record: one right-hand-side term `image` with coefficient `1`, the piece's case and bounds,
/// no exceptions; provenance in `kind` / `symmetry_origin` (informational members).
pub fn alias_rule_record(
    piece: &Piece,
    chart_text: &str,
    index: i64,
    element: usize,
    el: &SymElement,
    image: &str,
) -> Value {
    let mut r = json!({
        "type": "rule",
        "chart": chart_text,
        "index": index,
        "fixed": piece.fixed,
        "equations": piece.equations,
        "target": canonical_target(&piece.fixed),
        "rhs": [[image, "1"]],
        "exceptions": [],
        "dispatch": "Partition",
        "numerical": false,
        "kind": ALIAS_KIND,
        "symmetry_origin": {
            "schema": SYMALIAS_SCHEMA,
            "version": SYMALIAS_VERSION,
            "piece": piece.name,
            "element": element,
            "action": action_json(el),
            "matrix": el.matrix,
            "identity": "I(key) = I(image): verified automorphism, pure on every slot that can be non-zero on the case",
        },
    });
    if let Some(b) = &piece.domain_bounds {
        r["domain_bounds"] = b.clone();
    }
    r
}

/// The equivalent engine `sym` record (interfaces/coord-sym-records.md section 1; no bounds: the engine kind has none),
/// for the exactness checks (the consumer's `verify_sym_record`, the engine's `rr_node replay`). Its case contains the
/// piece's case, so exactness on it implies exactness on the piece.
pub fn engine_sym_record(
    piece: &Piece,
    chart_text: &str,
    index: i64,
    element: usize,
    el: &SymElement,
    image: &str,
) -> Value {
    json!({
        "type": "sym",
        "chart": chart_text,
        "index": index,
        "fixed": piece.fixed,
        "equations": piece.equations,
        "target": canonical_target(&piece.fixed),
        "image": image,
        "element": element,
        "action": action_json(el),
        "matrix": el.matrix,
        "sign": "+",
        "numerical": piece.fixed.iter().all(Option::is_some),
    })
}

/// The case domain the core reader builds from a `rule` record (the shipped domain).
pub fn record_domain(rec: &Value, frame: &NodeFrame) -> Result<CaseDomain, String> {
    let line = serde_json::to_vec(rec).map_err(|e| e.to_string())?;
    let mut rd = RecordReader::new(
        std::io::Cursor::new(line),
        ReadOptions {
            rhs: RhsMode::KeysOnly,
        },
    );
    match rd.next() {
        Some(Ok(TableLine::Rule(r))) => {
            if !r.unknown_members.is_empty() {
                return Err(format!("unknown members {:?}", r.unknown_members));
            }
            CaseDomain::from_rule_record(&r, frame, DomainOptions::default())
                .map_err(|e| e.to_string())
        }
        Some(Ok(other)) => Err(format!("not a rule line: {other:?}")),
        Some(Err(e)) => Err(format!("{e:?}")),
        None => Err("empty record".into()),
    }
}

/// The fixed points of g on the shipped domain (TRICKS 23): where `g k = k` the one-term record `I(k) = I(g k)` is a
/// tautology and dispatching it loops; a record may be emitted only on a domain WITHOUT fixed points.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub enum FixedPoints {
    /// Exact: no key of the domain is fixed by g.
    None,
    /// A key of the domain with `g k = k`.
    Exists { witness: Vec<i32> },
    /// The exact test did not decide.
    Unknown { reason: String },
}

/// The fixed-point set of `map` on `dom`: the equations `image(k)_t - k_t = 0` for every slot t (image slot t carries
/// the target's entry of slot i with `map[i] = t`, every other image slot is 0), decided by the exact emptiness test
/// on the domain.
pub fn fixed_points(
    dom: &CaseDomain,
    frame: &NodeFrame,
    fixed: &[Option<i64>],
    map: &[Option<usize>],
    budget: &ProofBudget,
) -> FixedPoints {
    let n = fixed.len();
    let entry = |i: usize| -> Poly {
        match fixed[i] {
            Some(v) => Poly::constant(i128::from(v)),
            None => Poly::var(i),
        }
    };
    let mut eqs = Vec::new();
    for t in 0..n {
        let img = match (0..n).find(|&i| map.get(i).copied().flatten() == Some(t)) {
            Some(i) => entry(i),
            None => Poly::constant(0),
        };
        let d = match img.sub(&entry(t)) {
            Ok(d) => d,
            Err(e) => {
                return FixedPoints::Unknown {
                    reason: format!("{e:?}"),
                };
            }
        };
        if d.is_zero() {
            continue;
        }
        if d.as_constant().is_some() {
            return FixedPoints::None; // a slot that always moves: no fixed point anywhere
        }
        eqs.push(d);
    }
    match emptiness(dom, frame, budget, &eqs, &[]) {
        (EmptyState::Empty, _, _) => FixedPoints::None,
        (EmptyState::NonEmpty, Some(w), _) => FixedPoints::Exists { witness: w },
        (st, _, why) => FixedPoints::Unknown {
            reason: format!("{st:?} {}", why.unwrap_or_default()),
        },
    }
}

/// One candidate alias of a piece.
#[derive(Clone, Debug, Serialize)]
pub struct Candidate {
    pub piece: String,
    /// Index into the group's (non-identity) elements.
    pub element: usize,
    pub target: String,
    pub image: String,
    /// The exact descent verdict of the alias' one term on the piece's whole domain.
    pub descent: PermutedVerdict,
    /// The fixed points of g on the shipped domain (TRICKS 23).
    pub fixed_points: FixedPoints,
    /// The `rule` record to ship.
    pub rule: Value,
    /// The equivalent engine `sym` record (exactness checks).
    pub sym: Value,
}

impl Candidate {
    pub fn lower(&self) -> bool {
        matches!(self.descent, PermutedVerdict::Lower { .. })
    }
    /// May be emitted as a shipped record (TRICKS 23): the term is strictly lower on the WHOLE shipped domain AND g
    /// has no fixed point there (exact). The engine `sym` record (no bounds) is for the exactness checks only, never
    /// for dispatch.
    pub fn shippable(&self) -> bool {
        self.lower() && self.fixed_points == FixedPoints::None
    }
}

/// Every non-identity element of `group` that is pure on the piece, in group order, with its alias records and the
/// exact descent verdict on the piece's whole domain. `index` is written into the records (the caller renumbers when
/// it appends them to a table). Err: the piece does not fit the frame, or the reader refuses the alias record.
pub fn candidates(
    group: &SymGroup,
    piece: &Piece,
    frame: &NodeFrame,
    cmp: &NodeComparator,
    budget: &ProofBudget,
    index: i64,
) -> Result<Vec<Candidate>, String> {
    let n = frame.arity();
    if piece.fixed.len() != n || group.arity != n {
        return Err(format!(
            "piece {} of {} slots, group of {}, frame of {n}",
            piece.name,
            piece.fixed.len(),
            group.arity
        ));
    }
    let chart: Vec<bool> = (0..n).map(|s| frame.is_line(s)).collect();
    if group.chart != chart {
        return Err("the group's chart is not the frame's".into());
    }
    let chart_text = frame.mask_string();
    let target = canonical_target(&piece.fixed);
    let mut out = Vec::new();
    for (e, el) in group.elements.iter().enumerate() {
        let Some(map) = pure_map(el, &piece.fixed, &chart) else {
            continue;
        };
        let image = image_pattern(&piece.fixed, &map);
        let rule = alias_rule_record(piece, &chart_text, index, e, el, &image);
        let sym = engine_sym_record(piece, &chart_text, index, e, el, &image);
        let dom = record_domain(&rule, frame)?;
        let descent = permuted_descent(cmp, frame, &dom, &target, &image, budget);
        let fixed_points = fixed_points(&dom, frame, &piece.fixed, &map, budget);
        out.push(Candidate {
            piece: piece.name.clone(),
            element: e,
            target: target.clone(),
            image,
            descent,
            fixed_points,
            rule,
            sym,
        });
    }
    Ok(out)
}
