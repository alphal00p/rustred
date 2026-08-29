//! Bounded Atom traversal shared by service admission and projection.

use symbolica::atom::{AtomView, Symbol};

use super::error::{TensorError, TensorHeadKind, check_limit, checked_add};
use super::heads::TensorHeads;
use super::model::TensorLimits;

pub(super) fn census_atom(
    atom: AtomView<'_>,
    resource: &'static str,
    node_limit: usize,
    limits: TensorLimits,
) -> Result<usize, TensorError> {
    fn visit(
        atom: AtomView<'_>,
        depth: usize,
        resource: &'static str,
        node_limit: usize,
        depth_limit: usize,
        count: &mut usize,
    ) -> Result<(), TensorError> {
        *count = checked_add(resource, *count, 1)?;
        check_limit(resource, *count, node_limit)?;
        check_limit("tensor Atom nesting depth", depth, depth_limit)?;
        let child_depth = checked_add("tensor Atom nesting depth", depth, 1)?;
        match atom {
            AtomView::Fun(function) => {
                for argument in function.iter() {
                    visit(
                        argument,
                        child_depth,
                        resource,
                        node_limit,
                        depth_limit,
                        count,
                    )?;
                }
            }
            AtomView::Pow(power) => {
                visit(
                    power.get_base(),
                    child_depth,
                    resource,
                    node_limit,
                    depth_limit,
                    count,
                )?;
                visit(
                    power.get_exp(),
                    child_depth,
                    resource,
                    node_limit,
                    depth_limit,
                    count,
                )?;
            }
            AtomView::Mul(product) => {
                for factor in product.iter() {
                    visit(
                        factor,
                        child_depth,
                        resource,
                        node_limit,
                        depth_limit,
                        count,
                    )?;
                }
            }
            AtomView::Add(sum) => {
                for term in sum.iter() {
                    visit(term, child_depth, resource, node_limit, depth_limit, count)?;
                }
            }
            AtomView::Num(_) | AtomView::Var(_) => {}
        }
        Ok(())
    }

    let mut count = 0usize;
    visit(
        atom,
        0,
        resource,
        node_limit,
        limits.max_nesting_depth,
        &mut count,
    )?;
    Ok(count)
}

pub(super) fn first_reserved_head(
    atom: AtomView<'_>,
    heads: &TensorHeads,
) -> Option<TensorHeadKind> {
    match atom {
        AtomView::Var(variable) => heads.kind_for_symbol(variable.get_symbol()),
        AtomView::Fun(function) => heads.kind_for_symbol(function.get_symbol()).or_else(|| {
            function
                .iter()
                .find_map(|arg| first_reserved_head(arg, heads))
        }),
        AtomView::Pow(power) => first_reserved_head(power.get_base(), heads)
            .or_else(|| first_reserved_head(power.get_exp(), heads)),
        AtomView::Mul(product) => product
            .iter()
            .find_map(|factor| first_reserved_head(factor, heads)),
        AtomView::Add(sum) => sum.iter().find_map(|term| first_reserved_head(term, heads)),
        AtomView::Num(_) => None,
    }
}

/// Whether `needle` occurs as an exact subtree of `atom`.
///
/// Momentum labels are caller-owned Atoms rather than symbols, so ordinary
/// symbol-occurrence queries are insufficient for tensor-grammar admission.
pub(super) fn contains_exact_atom(atom: AtomView<'_>, needle: AtomView<'_>) -> bool {
    if atom == needle {
        return true;
    }
    match atom {
        AtomView::Fun(function) => function
            .iter()
            .any(|argument| contains_exact_atom(argument, needle)),
        AtomView::Pow(power) => {
            contains_exact_atom(power.get_base(), needle)
                || contains_exact_atom(power.get_exp(), needle)
        }
        AtomView::Mul(product) => product
            .iter()
            .any(|factor| contains_exact_atom(factor, needle)),
        AtomView::Add(sum) => sum.iter().any(|term| contains_exact_atom(term, needle)),
        AtomView::Num(_) | AtomView::Var(_) => false,
    }
}

pub(super) fn reserved_kind(symbol: Symbol, heads: &TensorHeads) -> Option<TensorHeadKind> {
    heads.kind_for_symbol(symbol)
}
