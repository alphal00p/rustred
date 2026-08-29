//! Bounded Atom census and authenticated caller-owned Atom validation.

use symbolica::atom::{Atom, AtomView};
use symbolica::coefficient::CoefficientView;

use super::super::error::Error;
use super::super::limits::{Limits, Stats, check_limit, checked_add, checked_mul};
use super::super::request::AtomProject;
use super::super::symbols::{
    RESERVED_NAMES, append_pending_atoms, authenticate_symbol_properties, rustred_identifier,
    symbol_label, validate_identifier_text,
};
use super::grammar::ExpressionHeadPolicy;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(in crate::input) struct AtomResourceCensus {
    pub(in crate::input) nodes: usize,
    pub(in crate::input) maximum_depth: usize,
    /// A zero-allocation upper bound: every byte in an exact packed numeric
    /// node is charged as eight possible integer payload bits.
    pub(in crate::input) integer_bits: usize,
    pub(in crate::input) packed_bytes: usize,
}

pub(in crate::input) fn census_atom_resources<'a>(
    atom: AtomView<'a>,
    max_nodes: usize,
    max_depth: usize,
) -> Result<AtomResourceCensus, Error> {
    check_limit("Atom nodes", 1, max_nodes)?;
    let packed_bytes = atom.get_byte_size();
    let mut pending = Vec::new();
    pending
        .try_reserve_exact(1)
        .map_err(|_| Error::AllocationFailure {
            resource: "Atom census stack",
            requested: 1,
        })?;
    pending.push((atom, 0usize));
    let mut nodes = 1usize;
    let mut maximum_depth = 0usize;
    let mut integer_bits = 0usize;
    while let Some((current, depth)) = pending.pop() {
        check_limit("Atom nesting depth", depth, max_depth)?;
        maximum_depth = maximum_depth.max(depth);
        match current {
            AtomView::Fun(function) => {
                for child in function.iter() {
                    schedule_atom_census_child(
                        &mut pending,
                        child,
                        depth,
                        &mut nodes,
                        max_nodes,
                        max_depth,
                    )?;
                }
            }
            AtomView::Pow(power) => {
                for child in power.iter() {
                    schedule_atom_census_child(
                        &mut pending,
                        child,
                        depth,
                        &mut nodes,
                        max_nodes,
                        max_depth,
                    )?;
                }
            }
            AtomView::Mul(product) => {
                for child in product.iter() {
                    schedule_atom_census_child(
                        &mut pending,
                        child,
                        depth,
                        &mut nodes,
                        max_nodes,
                        max_depth,
                    )?;
                }
            }
            AtomView::Add(sum) => {
                for child in sum.iter() {
                    schedule_atom_census_child(
                        &mut pending,
                        child,
                        depth,
                        &mut nodes,
                        max_nodes,
                        max_depth,
                    )?;
                }
            }
            AtomView::Num(number) => {
                match number.get_coeff_view() {
                    CoefficientView::Natural(_, _, imaginary_numerator, _)
                        if imaginary_numerator == 0 => {}
                    CoefficientView::Large(_, imaginary) if imaginary.is_zero() => {}
                    other => {
                        return Err(Error::UnsupportedToken {
                            detail: format!(
                                "non-exact-real numeric Atom is outside the v1 grammar: {other:?}"
                            ),
                        });
                    }
                }
                integer_bits = checked_add(
                    "packed Atom integer bits",
                    integer_bits,
                    checked_mul(
                        "packed Atom integer bits",
                        current.get_byte_size(),
                        u8::BITS as usize,
                    )?,
                )?;
            }
            AtomView::Var(_) => {}
        }
    }
    Ok(AtomResourceCensus {
        nodes,
        maximum_depth,
        integer_bits,
        packed_bytes,
    })
}

pub(in crate::input) fn schedule_atom_census_child<'a>(
    pending: &mut Vec<(AtomView<'a>, usize)>,
    child: AtomView<'a>,
    parent_depth: usize,
    nodes: &mut usize,
    max_nodes: usize,
    max_depth: usize,
) -> Result<(), Error> {
    let child_depth = parent_depth
        .checked_add(1)
        .ok_or(Error::ResourceCountOverflow {
            resource: "Atom nesting depth",
        })?;
    check_limit("Atom nesting depth", child_depth, max_depth)?;
    let requested = nodes.checked_add(1).ok_or(Error::ResourceCountOverflow {
        resource: "Atom nodes",
    })?;
    check_limit("Atom nodes", requested, max_nodes)?;
    pending
        .try_reserve(1)
        .map_err(|_| Error::AllocationFailure {
            resource: "Atom census stack",
            requested,
        })?;
    pending.push((child, child_depth));
    *nodes = requested;
    Ok(())
}

pub(in crate::input) fn census_atom(
    atom: AtomView<'_>,
    max_nodes: usize,
    max_depth: usize,
) -> Result<(usize, usize), Error> {
    let census = census_atom_resources(atom, max_nodes, max_depth)?;
    Ok((census.nodes, census.maximum_depth))
}

pub(in crate::input) fn census_project_parts(
    parts: &AtomProject,
    limits: Limits,
) -> Result<Stats, Error> {
    let mut stats = Stats::default();
    let mut inspect = |atom: &Atom| -> Result<(), Error> {
        let census = census_atom_resources(
            atom.as_view(),
            limits.max_atom_nodes,
            limits.max_nesting_depth,
        )?;
        stats.atom_nodes = checked_add(
            "explicit project Atom nodes",
            stats.atom_nodes,
            census.nodes,
        )?;
        check_limit(
            "explicit project Atom nodes",
            stats.atom_nodes,
            limits.max_atom_nodes,
        )?;
        stats.maximum_depth = stats.maximum_depth.max(census.maximum_depth);
        stats.retained_atom_integer_bits = checked_add(
            "aggregate project Atom integer bits",
            stats.retained_atom_integer_bits,
            census.integer_bits,
        )?;
        check_limit(
            "aggregate project Atom integer bits",
            stats.retained_atom_integer_bits,
            limits.max_retained_atom_integer_bits,
        )?;
        stats.retained_atom_bytes = checked_add(
            "aggregate project Atom bytes",
            stats.retained_atom_bytes,
            census.packed_bytes,
        )?;
        check_limit(
            "aggregate project Atom bytes",
            stats.retained_atom_bytes,
            limits.max_retained_atom_bytes,
        )?;
        Ok(())
    };
    inspect(&parts.dimension)?;
    for propagator in &parts.propagators {
        inspect(&propagator.expression)?;
        if let Some(shift) = &propagator.power_shift {
            inspect(shift)?;
        }
    }
    for entry in &parts.external_gram {
        inspect(&entry.value)?;
    }
    if let Some(numerator) = &parts.numerator {
        inspect(numerator)?;
    }
    Ok(stats)
}

pub(in crate::input) fn authenticate_project_parts(
    parts: &AtomProject,
    limits: Limits,
) -> Result<(), Error> {
    authenticate_atom_tree(parts.dimension.as_view(), limits)?;
    validate_expression_atom_tree(
        parts.dimension.as_view(),
        ExpressionHeadPolicy::BaseCoefficient,
        &parts.loop_momenta,
        &parts.external_momenta,
        limits,
    )?;
    for propagator in &parts.propagators {
        authenticate_atom_tree(propagator.expression.as_view(), limits)?;
        validate_expression_atom_tree(
            propagator.expression.as_view(),
            ExpressionHeadPolicy::Denominator,
            &parts.loop_momenta,
            &parts.external_momenta,
            limits,
        )?;
        if let Some(shift) = &propagator.power_shift {
            authenticate_atom_tree(shift.as_view(), limits)?;
            validate_expression_atom_tree(
                shift.as_view(),
                ExpressionHeadPolicy::BaseCoefficient,
                &parts.loop_momenta,
                &parts.external_momenta,
                limits,
            )?;
        }
    }
    for entry in &parts.external_gram {
        authenticate_atom_tree(entry.value.as_view(), limits)?;
        validate_expression_atom_tree(
            entry.value.as_view(),
            ExpressionHeadPolicy::BaseCoefficient,
            &parts.loop_momenta,
            &parts.external_momenta,
            limits,
        )?;
    }
    if let Some(numerator) = &parts.numerator {
        authenticate_atom_tree(numerator.as_view(), limits)?;
        validate_expression_atom_tree(
            numerator.as_view(),
            ExpressionHeadPolicy::Tensor,
            &parts.loop_momenta,
            &parts.external_momenta,
            limits,
        )?;
    }
    Ok(())
}

pub(in crate::input) fn validate_expression_atom_tree(
    atom: AtomView<'_>,
    policy: ExpressionHeadPolicy,
    loop_momenta: &[String],
    external_momenta: &[String],
    limits: Limits,
) -> Result<(), Error> {
    let mut pending = Vec::<AtomView<'_>>::new();
    pending
        .try_reserve_exact(1)
        .map_err(|_| Error::AllocationFailure {
            resource: "position-sensitive Atom validation",
            requested: 1,
        })?;
    pending.push(atom);
    while let Some(current) = pending.pop() {
        match current {
            AtomView::Fun(function) => {
                let head = symbol_label(function.get_symbol(), "expression head", limits)?;
                let allowed = match policy {
                    ExpressionHeadPolicy::BaseCoefficient => false,
                    ExpressionHeadPolicy::Denominator => head == "sp",
                    ExpressionHeadPolicy::Tensor => {
                        matches!(head.as_str(), "sp" | "vec" | "metric" | "J")
                    }
                };
                if !allowed {
                    return Err(Error::UnsupportedToken {
                        detail: format!(
                            "function head {head:?} is not allowed in a {policy:?} expression"
                        ),
                    });
                }
                if matches!(head.as_str(), "sp" | "vec" | "metric") && function.get_nargs() != 2 {
                    return Err(Error::UnsupportedToken {
                        detail: format!("expression head {head:?} needs exactly 2 arguments"),
                    });
                }
                append_pending_atoms(&mut pending, function.iter(), limits)?;
            }
            AtomView::Pow(power) => append_pending_atoms(&mut pending, power.iter(), limits)?,
            AtomView::Mul(product) => {
                append_pending_atoms(&mut pending, product.iter(), limits)?;
            }
            AtomView::Add(sum) => append_pending_atoms(&mut pending, sum.iter(), limits)?,
            AtomView::Var(variable) => {
                if policy == ExpressionHeadPolicy::BaseCoefficient {
                    let label = symbol_label(variable.get_symbol(), "base coefficient", limits)?;
                    if loop_momenta.iter().any(|momentum| momentum == &label)
                        || external_momenta.iter().any(|momentum| momentum == &label)
                    {
                        return Err(Error::UnsupportedToken {
                            detail: format!(
                                "momentum {label:?} is not allowed in a base-coefficient field"
                            ),
                        });
                    }
                }
            }
            AtomView::Num(_) => {}
        }
    }
    Ok(())
}

pub(in crate::input) fn authenticate_atom_tree(
    atom: AtomView<'_>,
    limits: Limits,
) -> Result<(), Error> {
    let mut pending = Vec::<AtomView<'_>>::new();
    pending
        .try_reserve_exact(1)
        .map_err(|_| Error::AllocationFailure {
            resource: "Atom symbol authentication",
            requested: 1,
        })?;
    pending.push(atom);
    let mut inspected = 0usize;
    while let Some(current) = pending.pop() {
        inspected = checked_add("Atom symbol authentication", inspected, 1)?;
        check_limit(
            "Atom symbol authentication",
            inspected,
            limits.max_symbol_inspections,
        )?;
        match current {
            AtomView::Var(variable) => {
                let symbol = variable.get_symbol();
                let qualified = symbol.get_name();
                let logical = rustred_identifier(qualified)?;
                validate_identifier_text(logical, limits)?;
                authenticate_symbol_properties(symbol, qualified, 0)?;
            }
            AtomView::Fun(function) => {
                let symbol = function.get_symbol();
                let qualified = symbol.get_name();
                let head = rustred_identifier(qualified)?;
                if !RESERVED_NAMES.contains(&head) {
                    return Err(Error::UnsupportedToken {
                        detail: format!("function head {head:?} is outside the v1 grammar"),
                    });
                }
                authenticate_symbol_properties(symbol, qualified, 0)?;
                append_pending_atoms(&mut pending, function.iter(), limits)?;
            }
            AtomView::Pow(power) => append_pending_atoms(&mut pending, power.iter(), limits)?,
            AtomView::Mul(product) => append_pending_atoms(&mut pending, product.iter(), limits)?,
            AtomView::Add(sum) => append_pending_atoms(&mut pending, sum.iter(), limits)?,
            AtomView::Num(_) => {}
        }
    }
    Ok(())
}
