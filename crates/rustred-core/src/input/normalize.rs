//! Common authentication and normalization shared by all input frontends.

use symbolica::atom::Atom;

use super::canonical::{canonical_scaffold_base, render_canonical};
use super::error::Error;
use super::gram::build_external_gram;
use super::limits::{Limits, Stats, check_limit, checked_add, checked_mul};
use super::model::{ParameterSource, Project, ProjectSource, Propagator, Target};
use super::parse::{
    IntegralSyntax, authenticate_project_parts, census_atom, census_atom_resources,
    census_project_parts,
};
use super::request::AtomProject;
use super::symbols::{
    discover_scalar_symbols, family_scalar_atoms, validate_label_text, validate_ordered_labels,
};

const DEFAULT_FAMILY_NAME: &str = "symbolica_integral";
const MAX_NORMALIZED_FIELD_ATOM_COPIES: usize = 4;
const PACKED_ATOM_SCAFFOLD_BYTES_PER_NODE: usize = 64;

pub(super) fn normalize_parts(
    parts: AtomProject,
    source: ProjectSource,
    syntax: &IntegralSyntax,
    mut stats: Stats,
    limits: Limits,
) -> Result<Project, Error> {
    check_limit(
        "propagators",
        parts.propagators.len(),
        limits.max_propagators,
    )?;
    check_limit(
        "external Gram entries",
        parts.external_gram.len(),
        limits.max_gram_entries,
    )?;
    check_limit("loop momenta", parts.loop_momenta.len(), limits.max_momenta)?;
    check_limit(
        "external momenta",
        parts.external_momenta.len(),
        limits.max_momenta,
    )?;
    if let Some(parameters) = &parts.parameters {
        check_limit("parameters", parameters.len(), limits.max_parameters)?;
    }
    // Count caller-owned or independently parsed field Atoms before Gram
    // symmetry and canonical rendering clone any of their packed payloads.
    let project_census = census_project_parts(&parts, limits)?;
    let canonical_scaffold_base = canonical_scaffold_base(&parts, limits)?;
    let source_census = match &source {
        ProjectSource::Symbolica { source } => Some(census_atom_resources(
            source.as_view(),
            limits.max_atom_nodes,
            limits.max_nesting_depth,
        )?),
        ProjectSource::Explicit => None,
    };
    let source_integer_bits = source_census.map_or(0, |census| census.integer_bits);
    let source_packed_bytes = source_census.map_or(0, |census| census.packed_bytes);
    let retained_atom_integer_bits = checked_add(
        "retained project Atom integer bits",
        source_integer_bits,
        checked_mul(
            "retained project Atom integer bits",
            project_census.retained_atom_integer_bits,
            MAX_NORMALIZED_FIELD_ATOM_COPIES,
        )?,
    )?;
    check_limit(
        "retained project Atom integer bits",
        retained_atom_integer_bits,
        limits.max_retained_atom_integer_bits,
    )?;
    let retained_atom_base_bytes = checked_add(
        "retained project Atom bytes",
        source_packed_bytes,
        checked_mul(
            "retained project Atom bytes",
            project_census.retained_atom_bytes,
            MAX_NORMALIZED_FIELD_ATOM_COPIES,
        )?,
    )?;
    check_limit(
        "retained project Atom bytes",
        retained_atom_base_bytes,
        limits.max_retained_atom_bytes,
    )?;
    authenticate_project_parts(&parts, limits)?;
    if stats.atom_nodes == 0 {
        stats.atom_nodes = project_census.atom_nodes;
        stats.maximum_depth = project_census.maximum_depth;
    }
    stats.retained_atom_integer_bits = retained_atom_integer_bits;
    stats.retained_atom_bytes = retained_atom_base_bytes;
    let name_explicit = parts.name.is_some();
    let name = parts.name.unwrap_or_else(|| DEFAULT_FAMILY_NAME.to_owned());
    validate_label_text(&name, "family name", limits)?;
    validate_ordered_labels(
        &parts.loop_momenta,
        "loop momentum",
        limits.max_momenta,
        limits,
    )?;
    validate_ordered_labels(
        &parts.external_momenta,
        "external momentum",
        limits.max_momenta,
        limits,
    )?;
    if parts.loop_momenta.is_empty() {
        return Err(Error::NoLoopMomenta);
    }
    let momentum_count = parts
        .loop_momenta
        .len()
        .checked_add(parts.external_momenta.len())
        .ok_or(Error::ResourceCountOverflow {
            resource: "momenta",
        })?;
    check_limit("momenta", momentum_count, limits.max_momenta)?;

    let mut momentum_names = Vec::<&str>::new();
    momentum_names
        .try_reserve_exact(momentum_count)
        .map_err(|_| Error::AllocationFailure {
            resource: "momentum-name index",
            requested: momentum_count,
        })?;
    for label in parts.loop_momenta.iter().chain(&parts.external_momenta) {
        if momentum_names.iter().any(|candidate| *candidate == label) {
            return Err(Error::CrossClassLabelCollision {
                label: label.clone(),
            });
        }
        momentum_names.push(label);
    }
    if momentum_names.iter().any(|candidate| *candidate == name) {
        return Err(Error::CrossClassLabelCollision {
            label: name.clone(),
        });
    }

    let scalar_products =
        checked_scalar_product_count(parts.loop_momenta.len(), parts.external_momenta.len())?;
    if parts.propagators.len() != scalar_products {
        return Err(Error::WrongPropagatorCount {
            expected: scalar_products,
            actual: parts.propagators.len(),
        });
    }
    let mut denominator_ids = Vec::<&str>::new();
    denominator_ids
        .try_reserve_exact(parts.propagators.len())
        .map_err(|_| Error::AllocationFailure {
            resource: "propagator-name index",
            requested: parts.propagators.len(),
        })?;
    for prop in &parts.propagators {
        validate_label_text(&prop.id, "propagator", limits)?;
        if denominator_ids
            .iter()
            .any(|candidate| *candidate == prop.id)
        {
            return Err(Error::DuplicateLabel {
                role: "propagator",
                label: prop.id.clone(),
            });
        }
        if momentum_names.iter().any(|candidate| *candidate == prop.id) || prop.id == name {
            return Err(Error::CrossClassLabelCollision {
                label: prop.id.clone(),
            });
        }
        denominator_ids.push(&prop.id);
    }

    let (external_gram, ordered_gram_atoms) =
        build_external_gram(&parts.external_momenta, parts.external_gram, limits)?;
    let scalar_atoms =
        family_scalar_atoms(&parts.dimension, &parts.propagators, &ordered_gram_atoms)?;
    let mut forbidden_identifiers = Vec::<&str>::new();
    let forbidden_count = checked_add("family identifiers", denominator_ids.len(), 1)?;
    forbidden_identifiers
        .try_reserve_exact(forbidden_count)
        .map_err(|_| Error::AllocationFailure {
            resource: "family-identifier index",
            requested: forbidden_count,
        })?;
    forbidden_identifiers.extend(denominator_ids.iter().copied());
    forbidden_identifiers.push(&name);
    let discovered = discover_scalar_symbols(
        &scalar_atoms,
        &momentum_names,
        &forbidden_identifiers,
        &mut stats,
        limits,
    )?;
    let (parameter_names, operational_parameter_names, parameter_source) = match parts.parameters {
        Some(parameters) => {
            validate_ordered_labels(&parameters, "parameter", limits.max_parameters, limits)?;
            for parameter in &parameters {
                if momentum_names
                    .iter()
                    .any(|candidate| *candidate == parameter)
                    || forbidden_identifiers
                        .iter()
                        .any(|candidate| *candidate == parameter)
                {
                    return Err(Error::CrossClassLabelCollision {
                        label: parameter.clone(),
                    });
                }
            }
            for symbol in &discovered {
                if !parameters.iter().any(|declared| declared == symbol) {
                    return Err(Error::UndeclaredScalarSymbol {
                        symbol: symbol.clone(),
                    });
                }
            }
            (parameters, discovered, ParameterSource::Declared)
        }
        None => {
            let parameters = discovered;
            stats.inferred_parameters = parameters.len();
            check_limit(
                "inferred parameters",
                parameters.len(),
                limits.max_parameters,
            )?;
            (parameters.clone(), parameters, ParameterSource::Inferred)
        }
    };
    let mut operational_parameter_names = operational_parameter_names;
    operational_parameter_names.sort_unstable();

    let canonical_scaffold =
        canonical_scaffold_base.with_parameter_count(parameter_names.len(), limits)?;
    let prospective_canonical_nodes = checked_add(
        "canonical nodes",
        project_census.atom_nodes,
        canonical_scaffold.nodes,
    )?;
    check_limit(
        "canonical nodes",
        prospective_canonical_nodes,
        limits.max_canonical_nodes,
    )?;
    let canonical_scaffold_bytes = checked_mul(
        "canonical Atom scaffold bytes",
        canonical_scaffold.retained_scaffold_nodes,
        PACKED_ATOM_SCAFFOLD_BYTES_PER_NODE,
    )?;
    stats.retained_atom_bytes = checked_add(
        "retained project Atom bytes",
        stats.retained_atom_bytes,
        canonical_scaffold_bytes,
    )?;
    check_limit(
        "retained project Atom bytes",
        stats.retained_atom_bytes,
        limits.max_retained_atom_bytes,
    )?;
    stats.retained_atom_integer_bits = checked_add(
        "retained project Atom integer bits",
        stats.retained_atom_integer_bits,
        checked_mul(
            "canonical Atom scaffold integer bits",
            canonical_scaffold.numeric_nodes,
            u64::BITS as usize,
        )?,
    )?;
    check_limit(
        "retained project Atom integer bits",
        stats.retained_atom_integer_bits,
        limits.max_retained_atom_integer_bits,
    )?;

    let mut propagators = Vec::new();
    propagators
        .try_reserve_exact(parts.propagators.len())
        .map_err(|_| Error::AllocationFailure {
            resource: "normalized propagators",
            requested: parts.propagators.len(),
        })?;
    for prop in parts.propagators {
        let explicit = prop.power_shift.is_some();
        propagators.push(Propagator {
            id: prop.id,
            expression: prop.expression,
            target_power: prop.target_power,
            power_shift: prop.power_shift.unwrap_or_else(|| Atom::num(0)),
            power_shift_explicit: explicit,
        });
    }
    let numerator_explicit = parts.numerator.is_some();
    let mut target_powers = Vec::new();
    target_powers
        .try_reserve_exact(propagators.len())
        .map_err(|_| Error::AllocationFailure {
            resource: "target powers",
            requested: propagators.len(),
        })?;
    target_powers.extend(propagators.iter().map(|prop| prop.target_power));
    let target = Target {
        powers: target_powers,
        numerator: parts.numerator.unwrap_or_else(|| Atom::num(1)),
        numerator_explicit,
    };
    let canonical = render_canonical(
        syntax,
        &name,
        &parameter_names,
        &parts.loop_momenta,
        &parts.external_momenta,
        &parts.dimension,
        &propagators,
        &external_gram,
        &target,
        limits,
    )?;
    let (canonical_nodes, _) = census_atom(
        canonical.as_view(),
        limits.max_canonical_nodes,
        limits.max_nesting_depth,
    )?;
    stats.canonical_nodes = canonical_nodes;
    Ok(Project {
        source,
        name,
        name_explicit,
        parameter_names,
        operational_parameter_names,
        parameter_source,
        loop_momenta: parts.loop_momenta,
        external_momenta: parts.external_momenta,
        dimension: parts.dimension,
        propagators,
        external_gram,
        target,
        canonical,
        stats,
        limits,
    })
}

fn checked_scalar_product_count(loops: usize, externals: usize) -> Result<usize, Error> {
    let successor = loops.checked_add(1).ok_or(Error::ResourceCountOverflow {
        resource: "scalar products",
    })?;
    let triangular = if loops % 2 == 0 {
        (loops / 2).checked_mul(successor)
    } else {
        loops.checked_mul(successor / 2)
    }
    .ok_or(Error::ResourceCountOverflow {
        resource: "scalar products",
    })?;
    triangular
        .checked_add(
            loops
                .checked_mul(externals)
                .ok_or(Error::ResourceCountOverflow {
                    resource: "scalar products",
                })?,
        )
        .ok_or(Error::ResourceCountOverflow {
            resource: "scalar products",
        })
}
