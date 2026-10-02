//! Uniform physical-degree prefixes on a support-changing sign cell.
//!
//! A symbolic shift key has only one fixed support and cannot justify a
//! comparison across a sign boundary. Here each physical absolute power is
//! affine in its original local coordinate. Maximizing their weighted
//! difference on the existing exact box needs no CAS or additional partition.

use super::{LatticeBox, Ordering, OrderingPolicy, SourcePortAuditError, error};
use crate::order::DegreeRow;

pub(super) fn proves_lower(
    ordering: &OrderingPolicy,
    cell: &LatticeBox,
    source: &[bool],
    target: &[bool],
    shifts: &[i64],
) -> Result<bool, SourcePortAuditError> {
    if let Some(program) = ordering.program() {
        for row in &program.descriptor().pre_support_degree_rows {
            match maximum_delta(row, cell, source, target, shifts)? {
                Some(maximum) if maximum < 0 => return Ok(true),
                // A nonpositive row can be strict on part of the box. The
                // remaining prefix/suffix must also be lower on its tie face;
                // proving that on the whole box is sufficient and conservative.
                Some(0) => {}
                // No sampling or guessed affine equality: an unresolved
                // lexicographic tie face remains a failed descent proof.
                _ => return Ok(false),
            }
        }
    }
    // This is a suffix comparison only. With no pre-support rows it is
    // exactly the historical support-first proof, including legacy policies.
    Ok(ordering.compare_support(target, source).map_err(error)? == Ordering::Less)
}

fn maximum_delta(
    row: &DegreeRow,
    cell: &LatticeBox,
    source: &[bool],
    target: &[bool],
    shifts: &[i64],
) -> Result<Option<i128>, SourcePortAuditError> {
    let overflow = || error("pre-support descent degree arithmetic overflow");
    let mut maximum = 0_i128;
    for axis in 0..source.len() {
        let source_sign = if source[axis] { 1_i128 } else { -1 };
        let target_sign = if target[axis] { 1_i128 } else { -1 };
        let source_constant = i128::from(source[axis]);
        let source_weight = i128::from(if source[axis] {
            row.active[axis]
        } else {
            row.inactive[axis]
        });
        let target_weight = i128::from(if target[axis] {
            row.active[axis]
        } else {
            row.inactive[axis]
        });
        // n = source_sign * (x + source_constant), n' = n + shift.
        // The sign partition authenticates |n'| = target_sign * n'.
        let slope = target_weight * target_sign * source_sign - source_weight;
        let constant = (source_sign * source_constant + i128::from(shifts[axis]))
            .checked_mul(target_weight * target_sign)
            .and_then(|value| value.checked_sub(source_weight * source_constant))
            .ok_or_else(overflow)?;
        let endpoint = if slope > 0 {
            let Some(upper) = cell.upper()[axis] else {
                return Ok(None);
            };
            upper
        } else {
            cell.lower()[axis]
        };
        let contribution = slope
            .checked_mul(i128::from(endpoint))
            .and_then(|value| value.checked_add(constant))
            .ok_or_else(overflow)?;
        maximum = maximum.checked_add(contribution).ok_or_else(overflow)?;
    }
    Ok(Some(maximum))
}

#[cfg(test)]
mod tests;
