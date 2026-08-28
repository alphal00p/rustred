//! Canonical compact-Atom census and deterministic rendering.

use symbolica::atom::{Atom, FunctionBuilder, Symbol};

use super::error::Error;
use super::limits::{Limits, check_limit, checked_add, checked_mul};
use super::model::{Propagator, Target};
use super::parse::{ClauseKind, IntegralSyntax};
use super::request::AtomProject;
use super::symbols::label_symbol;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct CanonicalScaffoldBase {
    nodes_without_parameters: usize,
    numeric_nodes: usize,
    extra_retained_numeric_nodes: usize,
    clause_arguments_without_parameters: usize,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct CanonicalScaffoldCensus {
    pub(super) nodes: usize,
    pub(super) numeric_nodes: usize,
    pub(super) retained_scaffold_nodes: usize,
}

impl CanonicalScaffoldBase {
    pub(super) fn with_parameter_count(
        self,
        parameters: usize,
        limits: Limits,
    ) -> Result<CanonicalScaffoldCensus, Error> {
        let nodes = checked_add(
            "canonical scaffold nodes",
            self.nodes_without_parameters,
            parameters,
        )?;
        let clause_arguments = checked_add(
            "canonical clause arguments",
            self.clause_arguments_without_parameters,
            parameters,
        )?;
        check_limit(
            "canonical clause arguments",
            clause_arguments,
            limits.max_clause_arguments,
        )?;
        Ok(CanonicalScaffoldCensus {
            nodes,
            numeric_nodes: checked_add(
                "retained scaffold numeric nodes",
                self.numeric_nodes,
                self.extra_retained_numeric_nodes,
            )?,
            retained_scaffold_nodes: checked_add(
                "retained scaffold nodes",
                nodes,
                self.extra_retained_numeric_nodes,
            )?,
        })
    }
}

pub(super) fn canonical_scaffold_base(
    parts: &AtomProject,
    limits: Limits,
) -> Result<CanonicalScaffoldBase, Error> {
    let propagators = parts.propagators.len();
    let gram = parts
        .external_momenta
        .len()
        .checked_mul(parts.external_momenta.len().checked_add(1).ok_or(
            Error::ResourceCountOverflow {
                resource: "canonical scaffold Gram entries",
            },
        )?)
        .ok_or(Error::ResourceCountOverflow {
            resource: "canonical scaffold Gram entries",
        })?
        / 2;
    let clause_count = checked_add(
        "canonical clauses",
        checked_add(
            "canonical clauses",
            6,
            checked_mul("canonical clauses", propagators, 2)?,
        )?,
        gram,
    )?;
    check_limit("canonical clauses", clause_count, limits.max_clauses)?;

    let label_nodes = checked_add(
        "canonical label nodes",
        checked_add(
            "canonical label nodes",
            checked_add(
                "canonical label nodes",
                checked_add("canonical label nodes", 1, parts.loop_momenta.len())?,
                parts.external_momenta.len(),
            )?,
            checked_mul("canonical label nodes", propagators, 2)?,
        )?,
        checked_mul("canonical label nodes", gram, 2)?,
    )?;
    let default_shift_nodes = parts
        .propagators
        .iter()
        .filter(|propagator| propagator.power_shift.is_none())
        .count();
    let default_numerator_nodes = usize::from(parts.numerator.is_none());
    let numeric_nodes = checked_add(
        "canonical numeric nodes",
        checked_add("canonical numeric nodes", propagators, default_shift_nodes)?,
        default_numerator_nodes,
    )?;
    let nodes_without_parameters = checked_add(
        "canonical scaffold nodes",
        checked_add(
            "canonical scaffold nodes",
            checked_add("canonical scaffold nodes", 1, clause_count)?,
            label_nodes,
        )?,
        numeric_nodes,
    )?;
    let clause_arguments_without_parameters = checked_add(
        "canonical clause arguments",
        checked_add(
            "canonical clause arguments",
            checked_add(
                "canonical clause arguments",
                checked_add("canonical clause arguments", 3, parts.loop_momenta.len())?,
                parts.external_momenta.len(),
            )?,
            checked_mul("canonical clause arguments", propagators, 5)?,
        )?,
        checked_mul("canonical clause arguments", gram, 3)?,
    )?;
    Ok(CanonicalScaffoldBase {
        nodes_without_parameters,
        numeric_nodes,
        extra_retained_numeric_nodes: checked_add(
            "extra retained default numeric nodes",
            default_shift_nodes,
            default_numerator_nodes,
        )?,
        clause_arguments_without_parameters,
    })
}

#[allow(clippy::too_many_arguments)]
pub(super) fn render_canonical(
    syntax: &IntegralSyntax,
    name: &str,
    parameters: &[String],
    loops: &[String],
    externals: &[String],
    dimension: &Atom,
    propagators: &[Propagator],
    gram: &[Vec<Atom>],
    target: &Target,
    limits: Limits,
) -> Result<Atom, Error> {
    let mut clauses = Vec::new();
    let gram_count = externals
        .len()
        .checked_mul(
            externals
                .len()
                .checked_add(1)
                .ok_or(Error::ResourceCountOverflow {
                    resource: "canonical clauses",
                })?,
        )
        .ok_or(Error::ResourceCountOverflow {
            resource: "canonical clauses",
        })?
        / 2;
    let prop_clauses = propagators
        .len()
        .checked_mul(2)
        .ok_or(Error::ResourceCountOverflow {
            resource: "canonical clauses",
        })?;
    let clause_count = checked_add(
        "canonical clauses",
        checked_add("canonical clauses", 6, prop_clauses)?,
        gram_count,
    )?;
    check_limit("canonical clauses", clause_count, limits.max_clauses)?;
    clauses
        .try_reserve_exact(clause_count)
        .map_err(|_| Error::AllocationFailure {
            resource: "canonical clauses",
            requested: clause_count,
        })?;
    clauses.push(function(
        syntax.head(ClauseKind::Name),
        [label_atom(name, limits)?],
    ));
    clauses.push(function(
        syntax.head(ClauseKind::Loops),
        labels_to_atoms(loops, limits)?,
    ));
    clauses.push(function(
        syntax.head(ClauseKind::Externals),
        labels_to_atoms(externals, limits)?,
    ));
    clauses.push(function(
        syntax.head(ClauseKind::Parameters),
        labels_to_atoms(parameters, limits)?,
    ));
    clauses.push(function(
        syntax.head(ClauseKind::Dimension),
        [dimension.clone()],
    ));
    for prop in propagators {
        clauses.push(function(
            syntax.head(ClauseKind::Prop),
            [
                label_atom(&prop.id, limits)?,
                prop.expression.clone(),
                Atom::num(prop.target_power),
            ],
        ));
    }
    for prop in propagators {
        clauses.push(function(
            syntax.head(ClauseKind::PowerShift),
            [label_atom(&prop.id, limits)?, prop.power_shift.clone()],
        ));
    }
    for left in 0..externals.len() {
        for right in left..externals.len() {
            clauses.push(function(
                syntax.head(ClauseKind::Gram),
                [
                    label_atom(&externals[left], limits)?,
                    label_atom(&externals[right], limits)?,
                    gram[left][right].clone(),
                ],
            ));
        }
    }
    clauses.push(function(
        syntax.head(ClauseKind::Numerator),
        [target.numerator.clone()],
    ));
    Ok(function(syntax.root, clauses))
}

pub(super) fn function(symbol: Symbol, args: impl IntoIterator<Item = Atom>) -> Atom {
    FunctionBuilder::new(symbol).add_args(args).finish()
}

pub(super) fn labels_to_atoms(labels: &[String], limits: Limits) -> Result<Vec<Atom>, Error> {
    let mut atoms = Vec::new();
    atoms
        .try_reserve_exact(labels.len())
        .map_err(|_| Error::AllocationFailure {
            resource: "canonical labels",
            requested: labels.len(),
        })?;
    for label in labels {
        atoms.push(label_atom(label, limits)?);
    }
    Ok(atoms)
}

pub(super) fn label_atom(label: &str, limits: Limits) -> Result<Atom, Error> {
    Ok(Atom::var(label_symbol(label, "label", limits)?))
}
