//! REGION FORMS of the combinations (interfaces/tworow.md 4). A SHAPE = the rows of a combination as (element, shift)
//! with the weights as a function of the key: constant, or affine in the indices (exact fit through
//! `CheckAlgebra::q_rref`). Every shape found at any key of the region is evaluated at EVERY key of the region (the
//! complete list handed in; no sampling) by [`super::apply_combination`]; the report gives per key the set of acceptable
//! shape labels (lane regions' `regionfit` input `FaceKey.acceptable`) and the whole-region form when one shape is
//! acceptable at every key ("verified on N keys of the box": keys outside the list are not claimed).

use std::collections::{BTreeMap, BTreeSet};

use serde_json::{Value, json};

use super::search::{Cmp, ZeroSector, apply_combination};
use super::{KeyOutcome, KeyResult, RowSpec, TwoRowPolicy};
use crate::replay_base::algebra::{CheckAlgebra, Q, QResult};
use crate::replay_base::symrows::VerifiedGroup;
use crate::termmin::symrel::q_text;

/// A weight as a function of the key.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum WeightForm {
    Constant(Q),
    /// `constant + sum c_s n_s`
    Affine {
        constant: Q,
        terms: Vec<(usize, Q)>,
    },
}

impl WeightForm {
    pub fn eval<A: CheckAlgebra>(&self, alg: &A, key: &[i32]) -> QResult {
        match self {
            WeightForm::Constant(q) => Ok(*q),
            WeightForm::Affine { constant, terms } => {
                let mut v = *constant;
                for (s, c) in terms {
                    let n = *key.get(*s).ok_or("weight slot outside the key")?;
                    v = alg.q_add(v, alg.q_mul(*c, Q::int(n as i128))?)?;
                }
                Ok(v)
            }
        }
    }
    pub fn text(&self) -> String {
        match self {
            WeightForm::Constant(q) => q_text(q),
            WeightForm::Affine { constant, terms } => {
                let mut s = q_text(constant);
                for (slot, c) in terms {
                    s.push_str(&format!(" + ({})*n{slot}", q_text(c)));
                }
                s
            }
        }
    }
    pub fn is_constant(&self) -> bool {
        matches!(self, WeightForm::Constant(_))
    }
}

/// A shape: rows and weight forms.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Shape {
    pub rows: Vec<RowSpec>,
    pub weights: Vec<WeightForm>,
}

impl Shape {
    /// The opaque choice label (`regionfit`): `tworow:<rows>|w=<weights>`.
    pub fn label(&self) -> String {
        format!(
            "tworow:{}|w={}",
            self.rows
                .iter()
                .map(RowSpec::text)
                .collect::<Vec<_>>()
                .join("+"),
            self.weights
                .iter()
                .map(WeightForm::text)
                .collect::<Vec<_>>()
                .join(";")
        )
    }
}

/// A shape evaluated over the region.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ShapeVerdict {
    pub shape: Shape,
    /// Indices (into the region's key list) where the shape gives a valid relation.
    pub accepted: Vec<usize>,
    /// Indices where it does not, with the reason.
    pub rejected: Vec<(usize, String)>,
    pub min_rhs_terms: usize,
    pub max_rhs_terms: usize,
}

/// The region report.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RegionReport {
    pub keys: Vec<Vec<i32>>,
    /// Shapes, best first: most keys, constant weights before affine, fewer rows, smaller largest relation, label.
    pub shapes: Vec<ShapeVerdict>,
    /// Row sets whose weights fit neither form (they stay per key).
    pub unfitted: Vec<(Vec<RowSpec>, String)>,
    /// Index into `shapes` of the first shape acceptable at EVERY key of the region.
    pub whole_region: Option<usize>,
}

impl RegionReport {
    /// Per key: the labels of the acceptable shapes (the input of lane regions' `regionfit`).
    pub fn acceptable(&self) -> Vec<(Vec<i32>, BTreeSet<String>)> {
        let mut out: Vec<(Vec<i32>, BTreeSet<String>)> = self
            .keys
            .iter()
            .map(|k| (k.clone(), BTreeSet::new()))
            .collect();
        for v in &self.shapes {
            let l = v.shape.label();
            for &i in &v.accepted {
                out[i].1.insert(l.clone());
            }
        }
        out
    }
    pub fn to_json(&self) -> Value {
        json!({
            "keys": self.keys.len(),
            "whole_region": self.whole_region.map(|i| {
                let v = &self.shapes[i];
                json!({"label": v.shape.label(), "verified_on": format!("verified on {} keys of the box", v.accepted.len()),
                       "rhs_terms": [v.min_rhs_terms, v.max_rhs_terms]})
            }),
            "shapes": self.shapes.iter().map(|v| json!({
                "label": v.shape.label(),
                "accepted": v.accepted.len(),
                "rejected": v.rejected.len(),
                "first_rejections": v.rejected.iter().take(3).map(|(i, r)| json!([self.keys[*i], r])).collect::<Vec<_>>(),
                "rhs_terms": [v.min_rhs_terms, v.max_rhs_terms],
            })).collect::<Vec<_>>(),
            "unfitted": self.unfitted.iter().map(|(r, why)| json!({
                "rows": r.iter().map(RowSpec::text).collect::<Vec<_>>(), "why": why})).collect::<Vec<_>>(),
            "d_free": true,
            "regular_at_d4": "by construction (symmetry rows are d-free)",
            "engine_certificate": "none",
        })
    }
}

/// Fit one weight component over (key, value) pairs: constant if all equal, else affine in the slots that vary
/// (exact RREF; free unknowns set to 0), re-checked at every pair. `None` if neither fits.
fn fit<A: CheckAlgebra>(alg: &A, pts: &[(&[i32], Q)]) -> Option<WeightForm> {
    let first = pts.first()?.1;
    if pts.iter().all(|(_, w)| *w == first) {
        return Some(WeightForm::Constant(first));
    }
    let n = pts[0].0.len();
    let vary: Vec<usize> = (0..n)
        .filter(|&s| pts.iter().any(|(k, _)| k[s] != pts[0].0[s]))
        .collect();
    let nc = 1 + vary.len();
    let rows: Vec<Vec<Q>> = pts
        .iter()
        .map(|(k, w)| {
            let mut r = vec![Q::ONE];
            r.extend(vary.iter().map(|&s| Q::int(k[s] as i128)));
            r.push(*w);
            r
        })
        .collect();
    let (rref, piv) = alg.q_rref(&rows, nc).ok()?;
    if rref
        .iter()
        .any(|r| r[..nc].iter().all(Q::is_zero) && !r[nc].is_zero())
    {
        return None;
    }
    let mut sol = vec![Q::ZERO; nc];
    for (r, &p) in piv.iter().enumerate() {
        sol[p] = rref[r][nc];
    }
    let form = WeightForm::Affine {
        constant: sol[0],
        terms: vary
            .iter()
            .zip(&sol[1..])
            .filter(|(_, c)| !c.is_zero())
            .map(|(&s, c)| (s, *c))
            .collect(),
    };
    pts.iter()
        .all(|(k, w)| form.eval(alg, k).ok() == Some(*w))
        .then_some(form)
}

/// The region forms over `region_keys` (the region's complete key list in the box) from the per-key search results
/// (any keys; their relations propose the shapes). See the module documentation.
pub fn region_forms<A: CheckAlgebra>(
    alg: &A,
    group: &VerifiedGroup,
    region_keys: &[Vec<i32>],
    results: &[KeyResult],
    cmp: Cmp,
    policy: &TwoRowPolicy,
    zero: ZeroSector,
) -> RegionReport {
    // row sets -> (key, weights) observed
    let mut groups: BTreeMap<Vec<RowSpec>, Vec<(Vec<i32>, Vec<Q>)>> = BTreeMap::new();
    for r in results {
        if let KeyOutcome::Served { relations } = &r.outcome {
            for c in relations {
                groups
                    .entry(c.shape())
                    .or_default()
                    .push((c.key.clone(), c.weights.clone()));
            }
        }
    }
    let mut shapes: Vec<Shape> = vec![];
    let mut unfitted = vec![];
    for (rows, obs) in groups {
        let m = rows.len();
        let mut forms = vec![];
        for i in 0..m {
            let pts: Vec<(&[i32], Q)> = obs.iter().map(|(k, w)| (k.as_slice(), w[i])).collect();
            match fit(alg, &pts) {
                Some(f) => forms.push(f),
                None => break,
            }
        }
        if forms.len() == m {
            shapes.push(Shape {
                rows,
                weights: forms,
            });
        } else {
            unfitted.push((
                rows,
                format!(
                    "weights neither constant nor affine in the indices on {} keys",
                    obs.len()
                ),
            ));
        }
    }
    let mut verdicts: Vec<ShapeVerdict> = shapes
        .into_iter()
        .map(|shape| {
            let mut v = ShapeVerdict {
                shape,
                accepted: vec![],
                rejected: vec![],
                min_rhs_terms: usize::MAX,
                max_rhs_terms: 0,
            };
            for (i, k) in region_keys.iter().enumerate() {
                let w: Result<Vec<Q>, String> =
                    v.shape.weights.iter().map(|f| f.eval(alg, k)).collect();
                let res = match w {
                    Err(e) => Err(e),
                    Ok(w) => apply_combination(alg, group, k, &v.shape.rows, &w, cmp, policy, zero)
                        .map_err(|e| e.to_string()),
                };
                match res {
                    Ok(c) => {
                        v.accepted.push(i);
                        v.min_rhs_terms = v.min_rhs_terms.min(c.rhs.len());
                        v.max_rhs_terms = v.max_rhs_terms.max(c.rhs.len());
                    }
                    Err(e) => v.rejected.push((i, e)),
                }
            }
            if v.accepted.is_empty() {
                v.min_rhs_terms = 0;
            }
            v
        })
        .collect();
    verdicts.sort_by(|a, b| {
        b.accepted
            .len()
            .cmp(&a.accepted.len())
            .then_with(|| {
                let c = |v: &ShapeVerdict| v.shape.weights.iter().all(WeightForm::is_constant);
                c(b).cmp(&c(a))
            })
            .then(a.shape.rows.len().cmp(&b.shape.rows.len()))
            .then(a.max_rhs_terms.cmp(&b.max_rhs_terms))
            .then_with(|| a.shape.label().cmp(&b.shape.label()))
    });
    let whole_region = verdicts
        .iter()
        .position(|v| !region_keys.is_empty() && v.accepted.len() == region_keys.len());
    RegionReport {
        keys: region_keys.to_vec(),
        shapes: verdicts,
        unfitted,
        whole_region,
    }
}
