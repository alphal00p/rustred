//! Opt-in static choices for Insertion/Carpet loop-integration edge maps.
//!
//! Unlike census routing, the source and child have different loop counts.
//! Score the elementary factors AFTER tensor integration, exactly the inputs to
//! NumeratorRewrite, rather than applying an outer map to a parent ISP directly.
//! A child symmetry also permutes physical line targets; changing only the
//! momentum matrix is not a valid edge-map transformation.
//!
//! This module does not change any runtime default, rule, basis, or cache
//! identity. A package editor must bind the resulting artifact separately;
//! existing persistent step caches must not be shared with an altered package.
use super::{FormBasis, bilinear_form, compose, sector_automorphisms};
use crate::propagator::census::SignedAffineLoopMap;
use crate::propagator::error::PError;
use crate::propagator::kernel::child_row;
use crate::propagator::lane::LaneRoles;
use crate::propagator::presentation::{
    EdgeKind, EdgeMap, Presentation, Slot, check_carpet_contract, check_insertion_contract,
    verify_edge_map,
};

fn fail(detail: impl Into<String>) -> PError {
    PError::Contract(format!("outer-map choice: {}", detail.into()))
}

/// Post-integration elementary quadratic factors, in the native rewrite order:
/// contractions, residuals, free factors, then the optional insertion carrier^2.
pub fn factor_rows(
    parent: &Presentation,
    child: &Presentation,
    edge: &EdgeMap,
    roles: LaneRoles,
) -> Result<Vec<(Vec<i64>, Vec<i64>)>, PError> {
    verify_edge_map(parent, child, edge)?;
    let integrated = usize::from(edge.integrated_loop);
    let external = usize::from(parent.loops);
    let map = |row: &[i64]| child_row(&edge.outer_map, edge.kind, integrated, row);
    let (axis, carrier, insertion) = match roles {
        LaneRoles::Insertion(roles) => {
            check_insertion_contract(parent, &roles)?;
            if edge.kind != EdgeKind::Insertion || roles.loop_index != edge.integrated_loop {
                return Err(fail("insertion roles and edge disagree"));
            }
            let Slot::Line(second) = &parent.slots[usize::from(roles.lines[1])] else {
                return Err(fail("the second bubble role is not a line"));
            };
            let sign = second.momentum[integrated];
            let carrier: Vec<_> = second
                .momentum
                .iter()
                .enumerate()
                .map(|(column, c)| i64::from(column == integrated) - sign * c)
                .collect();
            (integrated, map(&carrier)?, true)
        }
        LaneRoles::Carpet(roles) => {
            check_carpet_contract(parent, &roles)?;
            if edge.kind != EdgeKind::Carpet || roles.outer_loop != edge.integrated_loop {
                return Err(fail("carpet roles and edge disagree"));
            }
            let mut carrier = vec![0; external + 1];
            carrier[integrated] = 1;
            (external, map(&carrier)?, false)
        }
        _ => {
            return Err(fail(
                "only Insertion and Carpet roles have outer-map choices",
            ));
        }
    };
    let (mut variables, mut free) = (Vec::new(), Vec::new());
    for slot in &parent.slots {
        let Slot::Isp(isp) = slot else { continue };
        match (isp.left[axis], isp.right[axis]) {
            (0, 0) => free.push((map(&isp.left)?, map(&isp.right)?)),
            (sign, 0) => variables.push(map(&isp
                .right
                .iter()
                .map(|c| sign * c)
                .collect::<Vec<_>>())?),
            (0, sign) => {
                variables.push(map(&isp.left.iter().map(|c| sign * c).collect::<Vec<_>>())?)
            }
            _ => return Err(fail("an ISP is quadratic in the integrated momentum")),
        }
    }
    let mut factors = Vec::new();
    for (s, left) in variables.iter().enumerate() {
        for right in &variables[s..] {
            factors.push((left.clone(), right.clone()));
        }
    }
    factors.extend(variables.into_iter().map(|v| (v, carrier.clone())));
    factors.extend(free);
    if insertion {
        factors.push((carrier.clone(), carrier));
    }
    Ok(factors)
}

/// Static rank-one/rank-two term proxy on the actual elementary factors. It is
/// not a traffic measurement and does not assume a performance improvement.
pub fn static_score(
    parent: &Presentation,
    child: &Presentation,
    edge: &EdgeMap,
    roles: LaneRoles,
) -> Result<((u64, u64), Vec<usize>), PError> {
    let basis = FormBasis::of(child)?.ok_or_else(|| fail("incomplete child form basis"))?;
    let terms = factor_rows(parent, child, edge, roles)?
        .into_iter()
        .map(|(left, right)| basis.terms(&bilinear_form(&left, &right)))
        .collect::<Result<Vec<_>, _>>()?;
    let mut score = (0, 0);
    for &n in &terms {
        let n = n as u64;
        score.0 += n + n * (n + 1) / 2;
        score.1 += n * n;
    }
    Ok((score, terms))
}

/// Compose an edge with a full marked child automorphism, deriving the physical
/// target permutation as well as the new matrix. Every candidate is rechecked
/// by the native edge admission (Jacobian, orientation, line targets and eps).
pub fn composed_edge(
    parent: &Presentation,
    child: &Presentation,
    stored: &EdgeMap,
    automorphism: &SignedAffineLoopMap,
) -> Result<(EdgeMap, Vec<Option<u16>>), PError> {
    verify_edge_map(parent, child, stored)?;
    if automorphism.loops_in != child.loops
        || automorphism.loops_out != child.loops
        || !automorphism.is_unimodular()
    {
        return Err(fail(
            "child automorphism is not unimodular on the child loops",
        ));
    }
    let mut permutation = vec![None; child.slots.len()];
    let mut used = vec![false; child.slots.len()];
    for (j, slot) in child.slots.iter().enumerate() {
        let Slot::Line(line) = slot else { continue };
        let image = automorphism.map_momentum(&line.momentum)?;
        let k = child
            .slots
            .iter()
            .position(|slot| match slot {
                Slot::Line(target) => {
                    target.eps_units == line.eps_units
                        && (image == target.momentum
                            || image.iter().zip(&target.momentum).all(|(a, b)| *a == -*b))
                }
                Slot::Isp(_) => false,
            })
            .ok_or_else(|| fail(format!("child line {j} has no equal-marked image")))?;
        if used[k] {
            return Err(fail("child physical line action is not a permutation"));
        }
        used[k] = true;
        permutation[j] = Some(u16::try_from(k).map_err(|_| fail("child slot overflow"))?);
    }
    let remap = |target: u16| {
        permutation
            .get(usize::from(target))
            .copied()
            .flatten()
            .ok_or_else(|| fail("stored edge targets a non-line child slot"))
    };
    let mut edge = stored.clone();
    edge.outer_map = compose(&stored.outer_map, automorphism);
    edge.line_slots = stored
        .line_slots
        .iter()
        .map(|target| target.map(remap).transpose())
        .collect::<Result<_, _>>()?;
    edge.merged_slot = stored.merged_slot.map(remap).transpose()?;
    verify_edge_map(parent, child, &edge)?;
    Ok((edge, permutation))
}

#[derive(Clone, Debug)]
pub struct Candidate {
    pub automorphism: SignedAffineLoopMap,
    pub edge: EdgeMap,
    pub physical_permutation: Vec<Option<u16>>,
    pub score: (u64, u64),
    pub factor_terms: Vec<usize>,
}

#[derive(Clone, Debug)]
pub struct Choice {
    pub edge: EdgeMap,
    pub score_before: (u64, u64),
    pub score_after: (u64, u64),
    pub candidates: Vec<Candidate>,
}

/// Keep the stored edge on ties. Only a strict static-score improvement can
/// alter it. The full physical support and eps marks define the child group.
pub fn choose_static(
    parent: &Presentation,
    child: &Presentation,
    stored: &EdgeMap,
    roles: LaneRoles,
) -> Result<Choice, PError> {
    let support = child.slots.iter().enumerate().fold(0_u64, |s, (j, slot)| {
        s | if matches!(slot, Slot::Line(_)) {
            1_u64 << j
        } else {
            0
        }
    });
    let group = sector_automorphisms(child, support)?;
    let (before, _) = static_score(parent, child, stored, roles)?;
    let (mut edge, mut after, mut candidates) = (stored.clone(), before, Vec::new());
    for automorphism in group {
        let (candidate, permutation) = composed_edge(parent, child, stored, &automorphism)?;
        let (score, factor_terms) = static_score(parent, child, &candidate, roles)?;
        if score < after {
            edge = candidate.clone();
            after = score;
        }
        candidates.push(Candidate {
            automorphism,
            edge: candidate,
            physical_permutation: permutation,
            score,
            factor_terms,
        });
    }
    Ok(Choice {
        edge,
        score_before: before,
        score_after: after,
        candidates,
    })
}
