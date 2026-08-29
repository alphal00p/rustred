use symbolica::prelude::AtomView;

use super::super::error::SymbolicaAffineDenominatorError;
use super::super::limits::SymbolicaAffineDenominatorLimits;
use super::check_limit;

#[allow(clippy::too_many_arguments)]
fn schedule_atom_views_with_depth<'a>(
    pending: &mut Vec<(AtomView<'a>, usize)>,
    children: impl Iterator<Item = AtomView<'a>>,
    child_count: usize,
    depth: usize,
    inspected: usize,
    node_limit: usize,
    allocation_resource: &'static str,
) -> Result<(), SymbolicaAffineDenominatorError> {
    let scheduled = inspected
        .checked_add(pending.len())
        .and_then(|value| value.checked_add(child_count))
        .ok_or(SymbolicaAffineDenominatorError::ResourceCountOverflow {
            resource: "input Atom nodes",
        })?;
    // `scheduled` is a census of every inspected or pending Atom, so this is
    // the public node-limit gate. Keep the traversal-stack label solely for
    // an allocator failure after that logical admission check.
    check_limit("input Atom nodes", scheduled, node_limit)?;
    pending.try_reserve(child_count).map_err(|_| {
        SymbolicaAffineDenominatorError::AllocationFailure {
            resource: allocation_resource,
            requested: child_count,
        }
    })?;
    let before = pending.len();
    pending.extend(children.map(|child| (child, depth)));
    if pending.len() != before + child_count {
        return Err(
            SymbolicaAffineDenominatorError::InternalVerificationFailure {
                detail: "Atom child iterator disagrees with its authenticated arity",
            },
        );
    }
    Ok(())
}

pub(in crate::input::affine) fn checked_atom_shape(
    atom: AtomView<'_>,
    limits: SymbolicaAffineDenominatorLimits,
) -> Result<(usize, usize), SymbolicaAffineDenominatorError> {
    let mut count = 0usize;
    let mut maximum_depth = 0usize;
    let mut pending = vec![(atom, 0usize)];
    while let Some((current, depth)) = pending.pop() {
        count =
            count
                .checked_add(1)
                .ok_or(SymbolicaAffineDenominatorError::ResourceCountOverflow {
                    resource: "input Atom nodes",
                })?;
        check_limit("input Atom nodes", count, limits.max_input_nodes)?;
        if depth > limits.max_nesting_depth {
            return Err(SymbolicaAffineDenominatorError::ResourceLimit {
                resource: "input Atom nesting depth",
                requested: depth as u128,
                limit: limits.max_nesting_depth as u128,
            });
        }
        maximum_depth = maximum_depth.max(depth);
        let next_depth =
            depth
                .checked_add(1)
                .ok_or(SymbolicaAffineDenominatorError::ResourceCountOverflow {
                    resource: "input Atom nesting depth",
                })?;
        match current {
            AtomView::Fun(function) => schedule_atom_views_with_depth(
                &mut pending,
                function.iter(),
                function.get_nargs(),
                next_depth,
                count,
                limits.max_input_nodes,
                "input Atom traversal stack",
            )?,
            AtomView::Pow(power) => schedule_atom_views_with_depth(
                &mut pending,
                power.iter(),
                2,
                next_depth,
                count,
                limits.max_input_nodes,
                "input Atom traversal stack",
            )?,
            AtomView::Mul(product) => schedule_atom_views_with_depth(
                &mut pending,
                product.iter(),
                product.get_nargs(),
                next_depth,
                count,
                limits.max_input_nodes,
                "input Atom traversal stack",
            )?,
            AtomView::Add(sum) => schedule_atom_views_with_depth(
                &mut pending,
                sum.iter(),
                sum.get_nargs(),
                next_depth,
                count,
                limits.max_input_nodes,
                "input Atom traversal stack",
            )?,
            AtomView::Num(_) | AtomView::Var(_) => {}
        }
    }
    Ok((count, maximum_depth))
}
