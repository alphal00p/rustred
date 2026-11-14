//! ORBITS FIRST (R46 (2), addendum 2 of the directing session): the node's symmetry group as INPUT DATA (schema
//! `c-fix.symmetry-group.v1`, the coordinator's `SYMMETRY.json`; interfaces/coord-fixtures.md), its action on keys and
//! cases, the reduction of a work list to ORBIT REPRESENTATIVES, and the "serve by image" step for the other members
//! (R44: a member key is served by an image CREDIT onto a rule of the campaign's own accepted rules when the credit
//! check passes; never image to image; credits are records, never rules).
//!
//! An element acts on a KEY as a permutation only where it is PURE: every line goes to a line (line permutation),
//! and every numerator slot with a NON-ZERO power goes to a single numerator slot with coefficient 1 and no constant.
//! Where a numerator with a non-zero power has a mixed image (a combination of propagators), the image of the key is
//! not a key: [`SymmetryGroup::image_key`] returns `None` and that element is not used for the key (the image would be a
//! multi-term identity: a symmetry RELATION, not a dispatch). Nothing here is node-specific (rule L8).

use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

use super::affine::AffineEq;
use super::items::{CaseSpec, Face, Item, ItemKind, Key};
use super::ladder::Tri;
use crate::repair::Descent;

/// One group element: for each slot, the slot its propagator maps to when the map is pure (`None` = mixed image).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct GroupElement {
    pub slot_image: Vec<Option<usize>>,
}

impl GroupElement {
    pub fn is_identity(&self) -> bool {
        self.slot_image
            .iter()
            .enumerate()
            .all(|(i, t)| *t == Some(i))
    }
}

/// The node's symmetry group (input data). `verified` = the source states the group is closed, every map was found
/// and every element agrees with the engine (the condition for using it in a one-term symmetry RELATION, terminal
/// merge, R46 (6)).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SymmetryGroup {
    pub arity: usize,
    pub elements: Vec<GroupElement>,
    pub verified: bool,
    pub source: String,
}

impl SymmetryGroup {
    /// The trivial group of a node of this arity.
    pub fn trivial(arity: usize) -> Self {
        SymmetryGroup {
            arity,
            elements: vec![GroupElement {
                slot_image: (0..arity).map(Some).collect(),
            }],
            verified: true,
            source: "trivial".into(),
        }
    }

    /// More than the identity.
    pub fn nontrivial(&self) -> bool {
        self.elements.iter().any(|e| !e.is_identity())
    }

    /// Read `c-fix.symmetry-group.v1` (fields `slots`, `lines`, `elements[].line_perm`, `elements[].numerator_images`,
    /// `closed`, `all_maps_found`, `all_agree_with_engine`). Slot NAMES are mapped to presented slots by their
    /// position in `slots`; `lines` must match the node's line mask.
    pub fn from_symmetry_json(text: &str, lines: &[bool]) -> Result<Self, String> {
        let v: serde_json::Value = serde_json::from_str(text).map_err(|e| e.to_string())?;
        if v.get("schema").and_then(|s| s.as_str()) != Some("c-fix.symmetry-group.v1") {
            return Err("schema is not c-fix.symmetry-group.v1".into());
        }
        let names: Vec<String> = v
            .get("slots")
            .and_then(|s| s.as_array())
            .ok_or("no slots")?
            .iter()
            .map(|x| x.as_str().map(String::from).ok_or("slot name"))
            .collect::<Result<_, _>>()?;
        if names.len() != lines.len() {
            return Err(format!("{} slots, node arity {}", names.len(), lines.len()));
        }
        let idx: BTreeMap<&str, usize> = names
            .iter()
            .enumerate()
            .map(|(i, n)| (n.as_str(), i))
            .collect();
        let line_names: BTreeSet<usize> = v
            .get("lines")
            .and_then(|s| s.as_array())
            .ok_or("no lines")?
            .iter()
            .map(|x| {
                x.as_str()
                    .and_then(|n| idx.get(n).copied())
                    .ok_or_else(|| "unknown line".to_string())
            })
            .collect::<Result<_, _>>()?;
        for (i, &l) in lines.iter().enumerate() {
            if l != line_names.contains(&i) {
                return Err(format!("line mask differs at slot {i}"));
            }
        }
        let mut elements = Vec::new();
        for e in v
            .get("elements")
            .and_then(|s| s.as_array())
            .ok_or("no elements")?
        {
            let mut img: Vec<Option<usize>> = (0..lines.len()).map(|_| None).collect();
            for (a, b) in e
                .get("line_perm")
                .and_then(|m| m.as_object())
                .ok_or("no line_perm")?
            {
                let (Some(&ia), Some(&ib)) =
                    (idx.get(a.as_str()), b.as_str().and_then(|b| idx.get(b)))
                else {
                    return Err(format!("line_perm {a} -> {b}"));
                };
                img[ia] = Some(ib);
            }
            if let Some(nm) = e.get("numerator_images").and_then(|m| m.as_object()) {
                for (a, combo) in nm {
                    let Some(&ia) = idx.get(a.as_str()) else {
                        return Err(format!("numerator {a}"));
                    };
                    let c = combo.as_object().ok_or("numerator image")?;
                    if c.len() == 1
                        && let Some((t, coef)) = c.iter().next()
                        && coef.as_str() == Some("1")
                        && let Some(&it) = idx.get(t.as_str())
                    {
                        img[ia] = Some(it);
                    }
                }
            }
            // numerator slots absent from numerator_images map to themselves (identity entries are omitted by
            // some writers); lines absent from line_perm are fixed
            for (i, slot) in img.iter_mut().enumerate() {
                if slot.is_none()
                    && e.get("numerator_images")
                        .and_then(|m| m.get(names[i].as_str()))
                        .is_none()
                {
                    *slot = Some(i);
                }
            }
            elements.push(GroupElement { slot_image: img });
        }
        let flag = |k: &str| v.get(k).and_then(|x| x.as_bool()).unwrap_or(false);
        Ok(SymmetryGroup {
            arity: lines.len(),
            elements,
            verified: flag("closed") && flag("all_maps_found") && flag("all_agree_with_engine"),
            source: v
                .get("source")
                .and_then(|s| s.as_str())
                .unwrap_or("")
                .to_string(),
        })
    }

    /// Image of a key under element `g` (`None` if a slot with a non-zero power has a mixed image or two slots
    /// collide). Slots of the image that receive nothing are at their floor (lines 1, numerators 0).
    pub fn image_key(&self, g: usize, key: &[i32], lines: &[bool]) -> Option<Key> {
        let e = self.elements.get(g)?;
        if key.len() != self.arity {
            return None;
        }
        let mut out: Vec<Option<i32>> = vec![None; key.len()];
        for (i, &v) in key.iter().enumerate() {
            let floor = if lines[i] { 1 } else { 0 };
            match e.slot_image[i] {
                Some(t) => {
                    if lines[t] != lines[i] || out[t].is_some() {
                        return None;
                    }
                    out[t] = Some(v);
                }
                None if v == floor => {}
                None => return None,
            }
        }
        Some(
            out.iter()
                .enumerate()
                .map(|(i, v)| v.unwrap_or(if lines[i] { 1 } else { 0 }))
                .collect(),
        )
    }

    /// Image of a CASE under `g` (face slots, fixed values and equations permuted); `None` if a face slot, a fixed
    /// off-floor slot or an equation slot has a mixed image.
    pub fn image_case(&self, g: usize, case: &CaseSpec, lines: &[bool]) -> Option<CaseSpec> {
        let e = self.elements.get(g)?;
        let map = |s: usize| e.slot_image.get(s).copied().flatten();
        let sd: Option<Vec<usize>> = case.face.sd.iter().map(|&s| map(s)).collect();
        let sn: Option<Vec<usize>> = case.face.sn.iter().map(|&s| map(s)).collect();
        let fixed: Option<BTreeMap<usize, i32>> = case
            .fixed
            .iter()
            .map(|(&s, &v)| map(s).map(|t| (t, v)))
            .collect();
        let eqs: Option<Vec<AffineEq>> = case
            .equations
            .iter()
            .map(|q| {
                let c: Option<Vec<(usize, i64)>> = q
                    .coeffs
                    .iter()
                    .map(|&(s, c)| map(s).map(|t| (t, c)))
                    .collect();
                c.and_then(|c| AffineEq::new(c, q.constant))
            })
            .collect();
        let (sd, sn, fixed, eqs) = (sd?, sn?, fixed?, eqs?);
        if sd.iter().any(|&s| !lines[s]) || sn.iter().any(|&s| lines[s]) {
            return None;
        }
        CaseSpec::new(Face::new(sd, sn), fixed, eqs).ok()
    }
}

/// The corner key of a case (dotted lines at 2, numerators at -1, fixed values as fixed, other slots at the floor):
/// the key by which representatives are compared.
fn corner(case: &CaseSpec, lines: &[bool]) -> Key {
    (0..lines.len())
        .map(|i| {
            if let Some(&v) = case.fixed.get(&i) {
                v
            } else if case.face.sd.contains(&i) {
                2
            } else if case.face.sn.contains(&i) {
                -1
            } else if lines[i] {
                1
            } else {
                0
            }
        })
        .collect()
}

/// One member of an orbit: the item and the element mapping it ONTO the representative's case.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OrbitMember {
    pub item: Item,
    pub element: usize,
}

/// An orbit of items under the group: its representative (searched first) and the other members (served by image,
/// else searched on their own). `rule` records how the representative was chosen.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Orbit {
    pub representative: Item,
    pub members: Vec<OrbitMember>,
    pub rule: String,
}

/// Reduce a work list to orbit representatives (R46 (2)). Items whose cases are images of each other under the group
/// form one orbit; the representative is the member whose corner key is least under the node comparator (ties: the
/// canonical case text), recorded in `Orbit::rule`. Optimise / box items and items whose case has no pure image are
/// singletons. Deterministic.
pub fn orbit_reduce(
    items: Vec<Item>,
    group: &SymmetryGroup,
    lines: &[bool],
    descent: &dyn Descent,
) -> Vec<Orbit> {
    let mut left: Vec<Item> = items;
    let mut out = Vec::new();
    while !left.is_empty() {
        let first = left.remove(0);
        let mut members: Vec<Item> = vec![first.clone()];
        if group.nontrivial() && !matches!(first.kind, ItemKind::Optimise | ItemKind::BoxRestricted)
        {
            let images: BTreeSet<CaseSpec> = (0..group.elements.len())
                .filter_map(|g| group.image_case(g, &first.case, lines))
                .collect();
            let mut i = 0;
            while i < left.len() {
                if left[i].kind == first.kind && images.contains(&left[i].case) {
                    members.push(left.remove(i));
                } else {
                    i += 1;
                }
            }
        }
        let rep_pos = (0..members.len())
            .min_by(|&a, &b| {
                let (ka, kb) = (
                    corner(&members[a].case, lines),
                    corner(&members[b].case, lines),
                );
                let ord = if descent.lower(&ka, &kb) && !descent.lower(&kb, &ka) {
                    std::cmp::Ordering::Less
                } else if descent.lower(&kb, &ka) && !descent.lower(&ka, &kb) {
                    std::cmp::Ordering::Greater
                } else {
                    std::cmp::Ordering::Equal
                };
                ord.then(
                    members[a]
                        .case
                        .canonical_text()
                        .cmp(&members[b].case.canonical_text()),
                )
            })
            .unwrap_or(0);
        let rep = members.remove(rep_pos);
        let mut ms = Vec::new();
        for m in members {
            let g = (0..group.elements.len())
                .find(|&g| group.image_case(g, &m.case, lines).as_ref() == Some(&rep.case));
            match g {
                Some(element) => ms.push(OrbitMember { item: m, element }),
                None => out.push(Orbit {
                    representative: m,
                    members: vec![],
                    rule: "singleton (no element maps it onto the representative)".into(),
                }),
            }
        }
        out.push(Orbit {
            representative: rep,
            members: ms,
            rule: "comparator-least corner key, then canonical case text".into(),
        });
    }
    out
}

/// The credit check of an image (R44 condition, through the closure / coverage lanes): is the rule `rule` of the
/// representative, credited at `image`, usable for `key` = every top RHS key of the rule at the image, mapped back,
/// strictly below the ORIGINAL key under the node comparator (and the rule's own acceptance at the image holds)?
pub trait ImageCredit {
    fn credit(&mut self, key: &[i32], image: &[i32], element: usize, rule: &str) -> Tri;
}

/// One image credit (a record, never a rule).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ImageServed {
    pub key: Key,
    pub image: Key,
    pub element: usize,
    pub rule: String,
}

/// Result of serving one orbit member by image.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ServeByImage {
    pub served: Vec<ImageServed>,
    /// The member's gap points no valid credit serves: they need their own search (the member item with only those
    /// points), `None` if every point is served.
    pub own_search: Option<Item>,
}

/// SERVE BY IMAGE (R46 (2), R44): each gap key of the member is mapped by the member's element onto the
/// representative's case; it is served iff a rule of the CAMPAIGN'S OWN accepted rules is credited at the image
/// (`credited`: rule digest -> keys it is credited at; image credits are never targets: no image to image) and the
/// credit check passes (`Tri::Pass` only; Unknown / Fail are not a justification). The other keys stay for the
/// member's own search.
pub fn serve_by_image(
    member: &OrbitMember,
    group: &SymmetryGroup,
    lines: &[bool],
    credited: &BTreeMap<String, BTreeSet<Key>>,
    check: &mut dyn ImageCredit,
) -> ServeByImage {
    let mut served = Vec::new();
    let mut rest = Vec::new();
    for k in &member.item.gap_points {
        let mut done = false;
        if let Some(img) = group.image_key(member.element, k, lines) {
            for (rule, keys) in credited {
                if keys.contains(&img) && check.credit(k, &img, member.element, rule) == Tri::Pass {
                    served.push(ImageServed {
                        key: k.clone(),
                        image: img.clone(),
                        element: member.element,
                        rule: rule.clone(),
                    });
                    done = true;
                    break;
                }
            }
        }
        if !done {
            rest.push(k.clone());
        }
    }
    let own_search = if rest.is_empty() {
        None
    } else {
        let mut it = member.item.clone();
        it.gap_points = rest;
        it.weight = it.gap_points.len() as u64;
        Some(it)
    };
    ServeByImage { served, own_search }
}

/// Terminal candidates related by the group become ONE terminal (R46 (6)): the comparator-least key of each class
/// is kept; the others are listed with the element relating them. This is a one-term symmetry RELATION: it is used
/// only with a VERIFIED group (`SymmetryGroup::verified`), never on the strength of a rule certificate; with an
/// unverified group nothing is merged.
pub fn merge_terminals_by_symmetry(
    terminals: &[Key],
    group: &SymmetryGroup,
    lines: &[bool],
    descent: &dyn Descent,
) -> (Vec<Key>, Vec<(Key, Key, usize)>) {
    if !group.verified || !group.nontrivial() {
        return (terminals.to_vec(), vec![]);
    }
    let mut keep: Vec<Key> = Vec::new();
    let mut related = Vec::new();
    let mut sorted: Vec<Key> = terminals.to_vec();
    sorted.sort_by(|a, b| {
        if descent.lower(a, b) && !descent.lower(b, a) {
            std::cmp::Ordering::Less
        } else if descent.lower(b, a) && !descent.lower(a, b) {
            std::cmp::Ordering::Greater
        } else {
            a.cmp(b)
        }
    });
    sorted.dedup();
    for t in sorted {
        let hit = keep.iter().find_map(|k| {
            (0..group.elements.len()).find_map(|g| {
                (group.image_key(g, &t, lines).as_ref() == Some(k)).then(|| (k.clone(), g))
            })
        });
        match hit {
            Some((k, g)) => related.push((t, k, g)),
            None => keep.push(t),
        }
    }
    (keep, related)
}
