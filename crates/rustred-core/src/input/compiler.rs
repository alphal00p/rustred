//! Public compiler facade and shared checked resource primitives.

use std::panic::{AssertUnwindSafe, catch_unwind};

use super::error::Error;
use super::limits::{Limits, Stats, check_limit};
use super::model::{Project, ProjectSource};
use super::normalize::normalize_parts;
use super::parse::{IntegralSyntax, RawSourceKind, parse_expression_accumulating};
use super::request::{AtomGramEntry, AtomProject, AtomPropagator, TextProject};

/// Compiler for compact, textual-field, and authenticated-Atom project input.
pub struct Compiler {
    pub(super) syntax: IntegralSyntax,
    pub(super) limits: Limits,
}

impl Compiler {
    pub fn new(limits: Limits) -> Result<Self, Error> {
        guarded_symbolica("grammar initialization", || {
            Ok(Self {
                syntax: IntegralSyntax::try_new()?,
                limits,
            })
        })
    }

    pub const fn limits(&self) -> Limits {
        self.limits
    }

    /// Compile fully explicit textual fields into the common normalized model.
    pub fn compile_text(&self, parts: TextProject) -> Result<Project, Error> {
        guarded_symbolica("explicit project expression parsing", || {
            let TextProject {
                name,
                parameters,
                loop_momenta,
                external_momenta,
                dimension,
                propagators,
                external_gram,
                numerator,
            } = parts;
            check_limit(
                "propagators",
                propagators.len(),
                self.limits.max_propagators,
            )?;
            check_limit(
                "external Gram entries",
                external_gram.len(),
                self.limits.max_gram_entries,
            )?;
            let mut stats = Stats::default();
            let dimension = parse_expression_accumulating(
                &dimension,
                RawSourceKind::BaseCoefficientExpression,
                &mut stats,
                self.limits,
            )?;

            let mut parsed_propagators = Vec::new();
            parsed_propagators
                .try_reserve_exact(propagators.len())
                .map_err(|_| Error::AllocationFailure {
                    resource: "explicit propagators",
                    requested: propagators.len(),
                })?;
            for propagator in propagators {
                let expression = parse_expression_accumulating(
                    &propagator.expression,
                    RawSourceKind::DenominatorExpression,
                    &mut stats,
                    self.limits,
                )?;
                let power_shift = match propagator.power_shift {
                    Some(source) => Some(parse_expression_accumulating(
                        &source,
                        RawSourceKind::BaseCoefficientExpression,
                        &mut stats,
                        self.limits,
                    )?),
                    None => None,
                };
                parsed_propagators.push(AtomPropagator {
                    id: propagator.id,
                    expression,
                    target_power: propagator.target_power,
                    power_shift,
                });
            }

            let mut parsed_gram = Vec::new();
            parsed_gram
                .try_reserve_exact(external_gram.len())
                .map_err(|_| Error::AllocationFailure {
                    resource: "explicit external Gram entries",
                    requested: external_gram.len(),
                })?;
            for entry in external_gram {
                let value = parse_expression_accumulating(
                    &entry.value,
                    RawSourceKind::BaseCoefficientExpression,
                    &mut stats,
                    self.limits,
                )?;
                parsed_gram.push(AtomGramEntry {
                    left: entry.left,
                    right: entry.right,
                    value,
                });
            }
            let numerator = match numerator {
                Some(source) => Some(parse_expression_accumulating(
                    &source,
                    RawSourceKind::TensorExpression,
                    &mut stats,
                    self.limits,
                )?),
                None => None,
            };
            normalize_parts(
                AtomProject {
                    name,
                    parameters,
                    loop_momenta,
                    external_momenta,
                    dimension,
                    propagators: parsed_propagators,
                    external_gram: parsed_gram,
                    numerator,
                },
                ProjectSource::Explicit,
                &self.syntax,
                stats,
                self.limits,
            )
        })
    }

    /// Compile caller-owned Symbolica atoms into the common normalized model.
    pub fn compile_atoms(&self, parts: AtomProject) -> Result<Project, Error> {
        guarded_symbolica("explicit input normalization", || {
            normalize_parts(
                parts,
                ProjectSource::Explicit,
                &self.syntax,
                Stats::default(),
                self.limits,
            )
        })
    }
}

pub(super) fn guarded_symbolica<T>(
    operation: &'static str,
    work: impl FnOnce() -> Result<T, Error>,
) -> Result<T, Error> {
    catch_unwind(AssertUnwindSafe(work)).map_err(|_| Error::SymbolicaPanic { operation })?
}
