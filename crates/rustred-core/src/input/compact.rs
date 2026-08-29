//! Compact `I(...)` frontend with direct authenticated head/arity dispatch.

use symbolica::atom::{Atom, AtomView};

use super::compiler::{Compiler, guarded_symbolica};
use super::error::Error;
use super::limits::{Stats, check_limit, checked_add, checked_mul};
use super::model::{Project, ProjectSource};
use super::normalize::normalize_parts;
use super::parse::{
    ClauseKind, RawSourceKind, authenticate_atom_tree, census_atom_resources,
    parse_authenticated_source, validate_clause_arity,
};
use super::request::{AtomGramEntry, AtomProject, AtomPropagator};
use super::symbols::{atom_i64, atom_label, collect_atom_views, collect_labels};

const MAX_COMPACT_SOURCE_ATOM_COPIES: usize = 6;
const PACKED_ATOM_SCAFFOLD_BYTES_PER_NODE: usize = 64;

/// Stable schema identifier for compact `I(...)` syntax.
pub const COMPACT_SCHEMA: &str = "rustred.symbolica-integral.v1";

impl Compiler {
    /// Compile compact `I(...)` syntax.
    ///
    /// An optional outer parameter list supplements an omitted
    /// `parameters(...)` clause. If both are present, their ordered contents
    /// must agree exactly.
    pub fn compile_compact(
        &self,
        source: &str,
        parameter_override: Option<Vec<String>>,
    ) -> Result<Project, Error> {
        guarded_symbolica("compact integral parsing", || {
            check_limit(
                "Symbolica integral input bytes",
                source.len(),
                self.limits.max_input_bytes,
            )?;
            let parsed =
                parse_authenticated_source(source, RawSourceKind::CompactIntegral, self.limits)?;
            self.compile_compact_atom(
                parsed.atom.as_view(),
                source.len(),
                parsed.preconversion_integer_bits,
                parameter_override,
            )
        })
    }

    fn compile_compact_atom(
        &self,
        source: AtomView<'_>,
        input_bytes: usize,
        preconversion_integer_bits: usize,
        parameter_override: Option<Vec<String>>,
    ) -> Result<Project, Error> {
        let mut stats = Stats {
            input_bytes,
            preconversion_integer_bits,
            ..Default::default()
        };
        let source_census = census_atom_resources(
            source,
            self.limits.max_atom_nodes,
            self.limits.max_nesting_depth,
        )?;
        check_limit(
            "source Atom integer-bit copy envelope",
            checked_mul(
                "source Atom integer-bit copy envelope",
                source_census.integer_bits,
                MAX_COMPACT_SOURCE_ATOM_COPIES,
            )?,
            self.limits.max_retained_atom_integer_bits,
        )?;
        let source_copy_bytes = checked_add(
            "source Atom copy bytes",
            checked_mul(
                "source Atom copy bytes",
                source_census.packed_bytes,
                MAX_COMPACT_SOURCE_ATOM_COPIES,
            )?,
            checked_mul(
                "source Atom copy bytes",
                source_census.nodes,
                PACKED_ATOM_SCAFFOLD_BYTES_PER_NODE,
            )?,
        )?;
        check_limit(
            "source Atom copy bytes",
            source_copy_bytes,
            self.limits.max_retained_atom_bytes,
        )?;
        stats.atom_nodes = source_census.nodes;
        stats.maximum_depth = source_census.maximum_depth;
        stats.retained_atom_integer_bits = source_census.integer_bits;
        stats.retained_atom_bytes = source_copy_bytes;
        authenticate_atom_tree(source, self.limits)?;

        let AtomView::Fun(root) = source else {
            return Err(Error::WrongRoot);
        };
        if root.get_symbol() != self.syntax.root
            || !root.get_symbol().get_attributes().is_empty()
            || root.get_nargs() == 0
        {
            return Err(Error::WrongRoot);
        }
        check_limit("I clauses", root.get_nargs(), self.limits.max_clauses)?;
        stats.clauses = root.get_nargs();

        let mut name: Option<String> = None;
        let mut loops: Option<Vec<String>> = None;
        let mut externals: Option<Vec<String>> = None;
        let mut parameters: Option<Vec<String>> = None;
        let mut dimension: Option<Atom> = None;
        let mut props = Vec::<AtomPropagator>::new();
        let mut shifts = Vec::<(String, Atom)>::new();
        let mut grams = Vec::<AtomGramEntry>::new();
        let mut numerator: Option<Atom> = None;

        props
            .try_reserve(root.get_nargs().min(self.limits.max_propagators))
            .map_err(|_| Error::AllocationFailure {
                resource: "propagator clauses",
                requested: root.get_nargs().min(self.limits.max_propagators),
            })?;
        shifts
            .try_reserve(root.get_nargs().min(self.limits.max_propagators))
            .map_err(|_| Error::AllocationFailure {
                resource: "power-shift clauses",
                requested: root.get_nargs().min(self.limits.max_propagators),
            })?;
        grams
            .try_reserve(root.get_nargs().min(self.limits.max_gram_entries))
            .map_err(|_| Error::AllocationFailure {
                resource: "external Gram clauses",
                requested: root.get_nargs().min(self.limits.max_gram_entries),
            })?;

        for (clause_ordinal, clause) in root.iter().enumerate() {
            let AtomView::Fun(function) = clause else {
                return Err(Error::UnknownClause {
                    clause: clause_ordinal,
                    expression: clause.to_owned(),
                });
            };
            let Some(kind) = self.syntax.classify(function.get_symbol()) else {
                return Err(Error::UnknownClause {
                    clause: clause_ordinal,
                    expression: clause.to_owned(),
                });
            };
            if !function.get_symbol().get_attributes().is_empty() {
                return Err(Error::UnknownClause {
                    clause: clause_ordinal,
                    expression: clause.to_owned(),
                });
            }
            let nargs = function.get_nargs();
            stats.clause_arguments =
                checked_add("clause arguments", stats.clause_arguments, nargs)?;
            check_limit(
                "clause arguments",
                stats.clause_arguments,
                self.limits.max_clause_arguments,
            )?;
            validate_clause_arity(kind, nargs, clause_ordinal)?;
            let args = collect_atom_views(function.iter(), nargs)?;
            match kind {
                ClauseKind::Name => set_singleton(
                    &mut name,
                    atom_label(args[0], "family name", self.limits)?,
                    "name",
                )?,
                ClauseKind::Loops => {
                    if loops.is_some() {
                        return Err(Error::DuplicateClause { kind: "loops" });
                    }
                    loops = Some(collect_labels(&args, "loop momentum", self.limits)?);
                }
                ClauseKind::Externals => {
                    if externals.is_some() {
                        return Err(Error::DuplicateClause { kind: "externals" });
                    }
                    externals = Some(collect_labels(&args, "external momentum", self.limits)?);
                }
                ClauseKind::Parameters => {
                    if parameters.is_some() {
                        return Err(Error::DuplicateClause { kind: "parameters" });
                    }
                    parameters = Some(collect_labels(&args, "parameter", self.limits)?);
                }
                ClauseKind::Dimension => {
                    set_singleton(&mut dimension, args[0].to_owned(), "dimension")?
                }
                ClauseKind::Prop => {
                    let requested = checked_add("propagators", props.len(), 1)?;
                    check_limit("propagators", requested, self.limits.max_propagators)?;
                    let id = atom_label(args[0], "propagator", self.limits)?;
                    let target_power =
                        atom_i64(args[2]).ok_or_else(|| Error::InvalidTargetPower {
                            denominator: id.clone(),
                            expression: args[2].to_owned(),
                        })?;
                    props.push(AtomPropagator {
                        id,
                        expression: args[1].to_owned(),
                        target_power,
                        power_shift: None,
                    });
                }
                ClauseKind::PowerShift => {
                    let id = atom_label(args[0], "power-shift propagator", self.limits)?;
                    if shifts.iter().any(|(candidate, _)| candidate == &id) {
                        return Err(Error::DuplicatePowerShift { denominator: id });
                    }
                    shifts.push((id, args[1].to_owned()));
                }
                ClauseKind::Gram => {
                    let requested = checked_add("external Gram entries", grams.len(), 1)?;
                    check_limit(
                        "external Gram entries",
                        requested,
                        self.limits.max_gram_entries,
                    )?;
                    grams.push(AtomGramEntry {
                        left: atom_label(args[0], "Gram momentum", self.limits)?,
                        right: atom_label(args[1], "Gram momentum", self.limits)?,
                        value: args[2].to_owned(),
                    });
                }
                ClauseKind::Numerator => {
                    set_singleton(&mut numerator, args[0].to_owned(), "numerator")?
                }
            }
        }

        let loops = loops.ok_or(Error::MissingClause { kind: "loops" })?;
        let externals = externals.ok_or(Error::MissingClause { kind: "externals" })?;
        let dimension = dimension.ok_or(Error::MissingClause { kind: "dimension" })?;
        if props.is_empty() {
            return Err(Error::MissingClause { kind: "prop" });
        }
        if loops.is_empty() {
            return Err(Error::NoLoopMomenta);
        }
        if let Some(override_names) = parameter_override {
            match &parameters {
                Some(internal) if *internal != override_names => {
                    return Err(Error::ConflictingParameterOverride);
                }
                Some(_) => {}
                None => parameters = Some(override_names),
            }
        }
        for prop in &mut props {
            if let Some(position) = shifts.iter().position(|(id, _)| id == &prop.id) {
                let (_, shift) = shifts.remove(position);
                prop.power_shift = Some(shift);
            }
        }
        if let Some((denominator, _)) = shifts.into_iter().next() {
            return Err(Error::UnknownPowerShift { denominator });
        }

        normalize_parts(
            AtomProject {
                name,
                parameters,
                loop_momenta: loops,
                external_momenta: externals,
                dimension,
                propagators: props,
                external_gram: grams,
                numerator,
            },
            ProjectSource::Symbolica {
                source: source.to_owned(),
            },
            &self.syntax,
            stats,
            self.limits,
        )
    }
}

fn set_singleton<T>(slot: &mut Option<T>, value: T, kind: &'static str) -> Result<(), Error> {
    if slot.replace(value).is_some() {
        Err(Error::DuplicateClause { kind })
    } else {
        Ok(())
    }
}
