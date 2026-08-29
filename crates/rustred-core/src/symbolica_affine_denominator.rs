//! Bounded Symbolica expression boundary for topology-independent affine denominators.
//!
//! A denominator is supplied as an exact Symbolica [`Atom`], together with an
//! ordered coefficient field, loop momenta, external momenta, and the complete
//! external Gram matrix.  The compiler interprets ordinary products of momentum
//! symbols, powers such as `(k1+k2)^2`, and the explicitly validated
//! `rustred::sp(vector_linear, vector_linear)` head.  It then lowers the result
//! to [`AffineDenominator`] coordinates without formatting and reparsing any
//! coefficient.
//!
//! The production implementation contains no topology names and no dispatch on
//! loop count.  All Symbolica arithmetic happens on one predeclared combined
//! rational-polynomial map and is checked before and authenticated after every
//! operation.  In particular, an undeclared symbol cannot silently extend that
//! map.
//!
//! The byte policies below are preconversion, logical-result, and retained
//! payload envelopes.  Symbolica does not expose or meter the transient
//! workspace of its native multivariate-GCD implementation, so these limits do
//! not claim to be a hard allocator cap on that private scratch space.

use std::collections::{BTreeMap, BTreeSet};
use std::error::Error;
use std::fmt;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::sync::Arc;

use symbolica::atom::{NamespacedSymbol, SymbolBuilder};
use symbolica::coefficient::SerializedRational;
use symbolica::domains::rational_polynomial::FromNumeratorAndDenominator;
use symbolica::prelude::*;

use crate::algebra::{
    Coefficient, CoefficientContext, CoefficientContextError, CoefficientPolynomial,
    ExactAlgebraError, ExactAlgebraLimits,
};
use crate::family::{AffineDenominator, ScalarProductCoordinate};

const RUSTRED_NAMESPACE: &str = "rustred";
const SCALAR_PRODUCT_NAME: &str = "rustred::sp";
const CONSERVATIVE_GMP_CAPACITY_FACTOR: usize = 2;

/// Resource policy for exact evaluation, projection, and retention.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SymbolicaAffineDenominatorLimits {
    pub exact_algebra: ExactAlgebraLimits,
    pub max_input_bytes: usize,
    pub max_input_nodes: usize,
    pub max_nesting_depth: usize,
    pub max_label_bytes: usize,
    pub max_total_label_bytes: usize,
    pub max_base_parameters: usize,
    pub max_momenta: usize,
    pub max_combined_variables: usize,
    pub max_scalar_product_coordinates: usize,
    pub max_external_gram_entries: usize,
    pub max_external_gram_polynomial_terms: usize,
    pub max_external_gram_exponent_entries: usize,
    pub max_external_gram_integer_bits: usize,
    pub max_abs_power: u32,
    pub max_arithmetic_operations: u64,
    pub max_combined_polynomial_terms: usize,
    pub max_combined_exponent_entries: usize,
    pub max_coefficient_integer_bits: usize,
    pub max_combined_retained_bytes: usize,
    pub max_dense_degree_box_terms: usize,
    pub max_dense_degree_box_exponent_entries: usize,
    pub max_aggregate_dense_degree_box_terms: usize,
    pub max_aggregate_dense_degree_box_exponent_entries: usize,
    pub max_projection_groups: usize,
    pub max_projection_denominator_replication_terms: usize,
    pub max_projection_denominator_replication_exponent_entries: usize,
    pub max_projection_gram_operations: usize,
    pub max_projected_polynomial_terms: usize,
    pub max_projected_exponent_entries: usize,
    pub max_projected_integer_bits: usize,
    pub max_projected_retained_bytes: usize,
    pub max_normalized_expression_nodes: usize,
    pub max_normalized_expression_integer_bits: usize,
    pub max_normalized_expression_bytes: usize,
    pub max_compiled_retained_bytes: usize,
}

impl Default for SymbolicaAffineDenominatorLimits {
    fn default() -> Self {
        Self {
            exact_algebra: ExactAlgebraLimits {
                max_exponent: u16::MAX,
                max_polynomial_terms: 100_000,
                max_term_operations: 1_000_000,
            },
            max_input_bytes: 16 * 1024 * 1024,
            max_input_nodes: 100_000,
            max_nesting_depth: 256,
            max_label_bytes: 1_024,
            max_total_label_bytes: 1024 * 1024,
            max_base_parameters: 4_096,
            max_momenta: 1_024,
            max_combined_variables: 4_096,
            max_scalar_product_coordinates: 1_000_000,
            max_external_gram_entries: 1_000_000,
            max_external_gram_polynomial_terms: 4_000_000,
            max_external_gram_exponent_entries: 16_000_000,
            max_external_gram_integer_bits: 256_000_000,
            max_abs_power: 256,
            max_arithmetic_operations: 10_000_000,
            max_combined_polynomial_terms: 100_000,
            max_combined_exponent_entries: 8_000_000,
            max_coefficient_integer_bits: 64_000_000,
            max_combined_retained_bytes: 256 * 1024 * 1024,
            max_dense_degree_box_terms: 16_000_000,
            max_dense_degree_box_exponent_entries: 64_000_000,
            max_aggregate_dense_degree_box_terms: 100_000_000,
            max_aggregate_dense_degree_box_exponent_entries: 400_000_000,
            max_projection_groups: 100_000,
            max_projection_denominator_replication_terms: 16_000_000,
            max_projection_denominator_replication_exponent_entries: 64_000_000,
            max_projection_gram_operations: 1_000_000,
            max_projected_polynomial_terms: 4_000_000,
            max_projected_exponent_entries: 64_000_000,
            max_projected_integer_bits: 256_000_000,
            max_projected_retained_bytes: 512 * 1024 * 1024,
            max_normalized_expression_nodes: 4_000_000,
            max_normalized_expression_integer_bits: 64_000_000,
            max_normalized_expression_bytes: 64 * 1024 * 1024,
            max_compiled_retained_bytes: 1024 * 1024 * 1024,
        }
    }
}

/// Typed failures at the untrusted Symbolica-expression boundary.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SymbolicaAffineDenominatorError {
    CoefficientContext(CoefficientContextError),
    ExactAlgebra(ExactAlgebraError),
    Parse(String),
    NoLoopMomenta,
    EmptyLabel {
        role: &'static str,
        position: usize,
    },
    DuplicateLabel {
        label: String,
        first_role: &'static str,
        second_role: &'static str,
    },
    ReservedLabel(String),
    ImpureDeclaredSymbol {
        label: String,
        violation: &'static str,
    },
    WrongExternalGramRowCount {
        expected: usize,
        actual: usize,
    },
    WrongExternalGramColumnCount {
        row: usize,
        expected: usize,
        actual: usize,
    },
    AsymmetricExternalGram {
        row: usize,
        column: usize,
    },
    InvalidExternalGramCoefficient {
        row: usize,
        column: usize,
        error: ExactAlgebraError,
    },
    CombinedVariableMapMismatch {
        position: usize,
        label: String,
    },
    UnsupportedCombinedVariable {
        position: usize,
        label: String,
    },
    ScalarProductHeadHasAttributes,
    ResourceLimit {
        resource: &'static str,
        requested: u128,
        limit: u128,
    },
    ResourceCountOverflow {
        resource: &'static str,
    },
    AllocationFailure {
        resource: &'static str,
        requested: usize,
    },
    UnknownSymbol(Atom),
    UnsupportedFunction(Atom),
    MalformedScalarProduct {
        atom: Atom,
        arguments: usize,
    },
    NestedScalarProduct(Atom),
    InvalidScalarProductArgument {
        argument: usize,
        atom: Atom,
    },
    UnsupportedPower(Atom),
    NegativeMomentumPower {
        atom: Atom,
        exponent: i64,
    },
    UnsupportedNumericAtom(Atom),
    MomentumDependentRationalDenominator,
    MomentumDegreeOne {
        numerator_term: usize,
    },
    MomentumDegreeTooHigh {
        numerator_term: usize,
        degree: u32,
    },
    InvalidQuadraticMomentumMonomial {
        numerator_term: usize,
    },
    BaseCoefficientContainsMomentum,
    NormalizedExpressionTooLarge {
        requested: usize,
        limit: usize,
    },
    SymbolicaPanic {
        stage: &'static str,
    },
    InternalVerificationFailure {
        detail: &'static str,
    },
}

impl fmt::Display for SymbolicaAffineDenominatorError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::CoefficientContext(error) => {
                write!(formatter, "invalid coefficient context: {error}")
            }
            Self::ExactAlgebra(error) => {
                write!(formatter, "exact coefficient arithmetic failed: {error}")
            }
            Self::Parse(error) => {
                write!(formatter, "could not parse Symbolica denominator: {error}")
            }
            Self::NoLoopMomenta => formatter
                .write_str("an affine denominator compiler needs at least one loop momentum"),
            Self::EmptyLabel { role, position } => {
                write!(formatter, "{role} label {position} is empty")
            }
            Self::DuplicateLabel {
                label,
                first_role,
                second_role,
            } => write!(
                formatter,
                "label {label:?} is used by both {first_role} and {second_role} declarations"
            ),
            Self::ReservedLabel(label) => write!(
                formatter,
                "label {label:?} collides with the reserved scalar-product head {SCALAR_PRODUCT_NAME}"
            ),
            Self::ImpureDeclaredSymbol { label, violation } => write!(
                formatter,
                "declared Symbolica symbol {label:?} is not a plain authenticated symbol: {violation}"
            ),
            Self::WrongExternalGramRowCount { expected, actual } => write!(
                formatter,
                "external Gram matrix has {actual} rows, expected {expected}"
            ),
            Self::WrongExternalGramColumnCount {
                row,
                expected,
                actual,
            } => write!(
                formatter,
                "external Gram row {row} has {actual} columns, expected {expected}"
            ),
            Self::AsymmetricExternalGram { row, column } => write!(
                formatter,
                "external Gram entries ({row},{column}) and ({column},{row}) differ"
            ),
            Self::InvalidExternalGramCoefficient { row, column, error } => write!(
                formatter,
                "external Gram entry ({row},{column}) is invalid: {error}"
            ),
            Self::CombinedVariableMapMismatch { position, label } => write!(
                formatter,
                "combined Symbolica variable {position} for {label:?} does not preserve the base coefficient map"
            ),
            Self::UnsupportedCombinedVariable { position, label } => write!(
                formatter,
                "combined Symbolica variable {position} for {label:?} is not a plain symbol"
            ),
            Self::ScalarProductHeadHasAttributes => write!(
                formatter,
                "reserved scalar-product head {SCALAR_PRODUCT_NAME} already has Symbolica attributes"
            ),
            Self::ResourceLimit {
                resource,
                requested,
                limit,
            } => write!(
                formatter,
                "{resource} needs {requested} units, exceeding the configured limit {limit}"
            ),
            Self::ResourceCountOverflow { resource } => {
                write!(formatter, "{resource} count overflowed its representation")
            }
            Self::AllocationFailure {
                resource,
                requested,
            } => write!(
                formatter,
                "could not reserve {requested} units for {resource}"
            ),
            Self::UnknownSymbol(atom) => {
                write!(formatter, "denominator contains undeclared symbol {atom}")
            }
            Self::UnsupportedFunction(atom) => write!(
                formatter,
                "denominator contains unsupported function {atom}"
            ),
            Self::MalformedScalarProduct { atom, arguments } => write!(
                formatter,
                "scalar product {atom} has {arguments} arguments, expected 2"
            ),
            Self::NestedScalarProduct(atom) => write!(
                formatter,
                "scalar-product argument contains nested scalar product {atom}"
            ),
            Self::InvalidScalarProductArgument { argument, atom } => write!(
                formatter,
                "scalar-product argument {argument} is not homogeneous and vector-linear: {atom}"
            ),
            Self::UnsupportedPower(atom) => write!(
                formatter,
                "denominator contains unsupported noninteger or oversized power {atom}"
            ),
            Self::NegativeMomentumPower { atom, exponent } => write!(
                formatter,
                "momentum-dependent base {atom} has negative power {exponent}"
            ),
            Self::UnsupportedNumericAtom(atom) => write!(
                formatter,
                "numeric atom {atom} is not an exact rational number"
            ),
            Self::MomentumDependentRationalDenominator => formatter
                .write_str("the rational denominator depends on a loop or external momentum"),
            Self::MomentumDegreeOne { numerator_term } => write!(
                formatter,
                "expanded numerator term {numerator_term} has momentum degree 1; denominators must be affine in scalar products"
            ),
            Self::MomentumDegreeTooHigh {
                numerator_term,
                degree,
            } => write!(
                formatter,
                "expanded numerator term {numerator_term} has momentum degree {degree}, above 2"
            ),
            Self::InvalidQuadraticMomentumMonomial { numerator_term } => write!(
                formatter,
                "expanded numerator term {numerator_term} is not a quadratic momentum monomial"
            ),
            Self::BaseCoefficientContainsMomentum => formatter
                .write_str("base coefficient expression contains a loop or external momentum"),
            Self::NormalizedExpressionTooLarge { requested, limit } => write!(
                formatter,
                "normalized expression retains {requested} bytes, exceeding the configured limit {limit}"
            ),
            Self::SymbolicaPanic { stage } => write!(
                formatter,
                "Symbolica panicked during affine-denominator {stage}"
            ),
            Self::InternalVerificationFailure { detail } => write!(
                formatter,
                "internal affine-denominator verification failed: {detail}"
            ),
        }
    }
}

impl Error for SymbolicaAffineDenominatorError {}

impl From<CoefficientContextError> for SymbolicaAffineDenominatorError {
    fn from(value: CoefficientContextError) -> Self {
        Self::CoefficientContext(value)
    }
}

impl From<ExactAlgebraError> for SymbolicaAffineDenominatorError {
    fn from(value: ExactAlgebraError) -> Self {
        Self::ExactAlgebra(value)
    }
}

/// Retained source, canonical normalized expression, and authenticated affine row.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CompiledSymbolicaAffineDenominator {
    source: Atom,
    normalized_expression: Atom,
    affine_denominator: AffineDenominator,
}

impl CompiledSymbolicaAffineDenominator {
    pub const fn source(&self) -> &Atom {
        &self.source
    }

    pub const fn normalized_expression(&self) -> &Atom {
        &self.normalized_expression
    }

    pub const fn affine_denominator(&self) -> &AffineDenominator {
        &self.affine_denominator
    }
}

/// One reusable, topology-neutral denominator-expression compiler.
#[derive(Debug)]
pub struct SymbolicaAffineDenominatorCompiler {
    coefficients: CoefficientContext,
    loop_momenta: Vec<String>,
    external_momenta: Vec<String>,
    external_gram: Vec<Vec<Coefficient>>,
    combined: CoefficientContext,
    symbol_positions: BTreeMap<Symbol, usize>,
    scalar_product: Symbol,
    coordinates: Vec<ScalarProductCoordinate>,
    limits: SymbolicaAffineDenominatorLimits,
}

impl SymbolicaAffineDenominatorCompiler {
    /// Authenticate one already-normalized ordered declaration.
    ///
    /// The base parameter list may have been explicit or inferred by a caller;
    /// this layer deliberately does not distinguish those provenance paths.
    pub fn try_new(
        coefficients: CoefficientContext,
        loop_momenta: Vec<String>,
        external_momenta: Vec<String>,
        external_gram: Vec<Vec<Coefficient>>,
        limits: SymbolicaAffineDenominatorLimits,
    ) -> Result<Self, SymbolicaAffineDenominatorError> {
        catch_unwind(AssertUnwindSafe(|| {
            Self::try_new_inner(
                coefficients,
                loop_momenta,
                external_momenta,
                external_gram,
                limits,
            )
        }))
        .map_err(|_| SymbolicaAffineDenominatorError::SymbolicaPanic {
            stage: "compiler construction",
        })?
    }

    fn try_new_inner(
        coefficients: CoefficientContext,
        loop_momenta: Vec<String>,
        external_momenta: Vec<String>,
        external_gram: Vec<Vec<Coefficient>>,
        limits: SymbolicaAffineDenominatorLimits,
    ) -> Result<Self, SymbolicaAffineDenominatorError> {
        check_limit(
            "base parameters",
            coefficients.parameter_names().len(),
            limits.max_base_parameters,
        )?;
        // Authenticate the already-retained template without constructing an
        // additional zero coefficient before its storage policy is known.
        coefficients.validate_with_limits(coefficients.template(), limits.exact_algebra)?;
        if loop_momenta.is_empty() {
            return Err(SymbolicaAffineDenominatorError::NoLoopMomenta);
        }

        let momentum_count = loop_momenta
            .len()
            .checked_add(external_momenta.len())
            .ok_or(SymbolicaAffineDenominatorError::ResourceCountOverflow {
                resource: "declared momenta",
            })?;
        check_limit("declared momenta", momentum_count, limits.max_momenta)?;
        let coordinate_count =
            scalar_product_coordinate_count(loop_momenta.len(), external_momenta.len())?;
        check_limit(
            "scalar-product coordinates",
            coordinate_count,
            limits.max_scalar_product_coordinates,
        )?;

        let mut roles = BTreeMap::<String, &'static str>::new();
        let mut total_label_bytes = 0usize;
        for (role, labels) in [
            ("base parameter", coefficients.parameter_names()),
            ("loop momentum", loop_momenta.as_slice()),
            ("external momentum", external_momenta.as_slice()),
        ] {
            for (position, label) in labels.iter().enumerate() {
                if label.is_empty() {
                    return Err(SymbolicaAffineDenominatorError::EmptyLabel { role, position });
                }
                check_limit("label bytes", label.len(), limits.max_label_bytes)?;
                total_label_bytes = total_label_bytes.checked_add(label.len()).ok_or(
                    SymbolicaAffineDenominatorError::ResourceCountOverflow {
                        resource: "total label bytes",
                    },
                )?;
                check_limit(
                    "total label bytes",
                    total_label_bytes,
                    limits.max_total_label_bytes,
                )?;
                if label == "sp" || label == SCALAR_PRODUCT_NAME {
                    return Err(SymbolicaAffineDenominatorError::ReservedLabel(
                        label.clone(),
                    ));
                }
                if let Some(first_role) = roles.insert(label.clone(), role) {
                    return Err(SymbolicaAffineDenominatorError::DuplicateLabel {
                        label: label.clone(),
                        first_role,
                        second_role: role,
                    });
                }
            }
        }

        validate_external_gram(&coefficients, &external_momenta, &external_gram, limits)?;

        let combined_count = coefficients
            .parameter_names()
            .len()
            .checked_add(momentum_count)
            .ok_or(SymbolicaAffineDenominatorError::ResourceCountOverflow {
                resource: "combined Symbolica variables",
            })?;
        check_limit(
            "combined Symbolica variables",
            combined_count,
            limits.max_combined_variables,
        )?;
        check_limit(
            "combined variable-map exponent width",
            combined_count,
            limits.max_combined_exponent_entries,
        )?;
        let mut combined_names = Vec::new();
        combined_names
            .try_reserve_exact(combined_count)
            .map_err(|_| SymbolicaAffineDenominatorError::AllocationFailure {
                resource: "combined Symbolica variable names",
                requested: combined_count,
            })?;
        combined_names.extend(coefficients.parameter_names().iter().cloned());
        combined_names.extend(loop_momenta.iter().cloned());
        combined_names.extend(external_momenta.iter().cloned());
        let combined = CoefficientContext::try_new(combined_names.clone())?;
        let combined_template_census = planned_coefficient_clone_census(
            combined.template(),
            combined.parameter_names().len(),
        )?;
        check_limit(
            "combined template retained bytes",
            combined_template_census.retained_bytes,
            limits.max_combined_retained_bytes,
        )?;

        for (position, label) in coefficients.parameter_names().iter().enumerate() {
            if combined.variables()[position] != coefficients.variables()[position] {
                return Err(
                    SymbolicaAffineDenominatorError::CombinedVariableMapMismatch {
                        position,
                        label: label.clone(),
                    },
                );
            }
        }

        let mut symbol_positions = BTreeMap::new();
        for (position, (variable, label)) in
            combined.variables().iter().zip(&combined_names).enumerate()
        {
            let PolyVariable::Symbol(symbol) = variable else {
                return Err(
                    SymbolicaAffineDenominatorError::UnsupportedCombinedVariable {
                        position,
                        label: label.clone(),
                    },
                );
            };
            let expected_name = format!("{RUSTRED_NAMESPACE}::{label}");
            authenticate_plain_symbol(*symbol, label, &expected_name)?;
            symbol_positions.insert(*symbol, position);
        }
        if symbol_positions.len() != combined_count {
            return Err(
                SymbolicaAffineDenominatorError::InternalVerificationFailure {
                    detail: "combined symbol map lost a declared variable",
                },
            );
        }
        let scalar_product = plain_symbol(SCALAR_PRODUCT_NAME)?;
        authenticate_plain_symbol(scalar_product, "sp", SCALAR_PRODUCT_NAME)?;
        if symbol_positions.contains_key(&scalar_product) {
            return Err(SymbolicaAffineDenominatorError::ReservedLabel(
                "sp".to_owned(),
            ));
        }
        let coordinates = scalar_product_coordinates(
            loop_momenta.len(),
            external_momenta.len(),
            coordinate_count,
        )?;
        Ok(Self {
            coefficients,
            loop_momenta,
            external_momenta,
            external_gram,
            combined,
            symbol_positions,
            scalar_product,
            coordinates,
            limits,
        })
    }

    /// Compile an already parsed Atom on the authenticated combined map.
    pub fn compile(
        &self,
        source: AtomView<'_>,
    ) -> Result<CompiledSymbolicaAffineDenominator, SymbolicaAffineDenominatorError> {
        catch_unwind(AssertUnwindSafe(|| self.compile_inner(source))).map_err(|_| {
            SymbolicaAffineDenominatorError::SymbolicaPanic {
                stage: "checked expression evaluation",
            }
        })?
    }

    fn compile_inner(
        &self,
        source: AtomView<'_>,
    ) -> Result<CompiledSymbolicaAffineDenominator, SymbolicaAffineDenominatorError> {
        let input_bytes = source.get_byte_size();
        check_limit(
            "input expression bytes",
            input_bytes,
            self.limits.max_input_bytes,
        )?;
        checked_atom_shape(source, self.limits)?;
        let fixed_retained_bytes = compiled_retained_byte_bound(input_bytes, 0, 0, 0)?;
        check_limit(
            "compiled fixed retained bytes",
            fixed_retained_bytes,
            self.limits.max_compiled_retained_bytes,
        )?;

        let mut evaluator = CheckedEvaluator::new(self);
        let evaluated = evaluator.evaluate(source, true)?;
        self.combined
            .validate_with_limits(&evaluated, self.limits.exact_algebra)?;
        self.validate_retained_shape(&evaluated)?;
        reject_momentum_denominator(&evaluated, self.base_count())?;

        // Bound the Atom that rational-polynomial conversion will construct
        // before asking Symbolica to allocate it.
        let normalized_census = normalized_expression_census(&evaluated)?;
        check_limit(
            "normalized expression nodes",
            normalized_census.nodes,
            self.limits.max_normalized_expression_nodes,
        )?;
        check_limit(
            "normalized expression integer bits",
            normalized_census.integer_bits,
            self.limits.max_normalized_expression_integer_bits,
        )?;
        let maximum_symbol_bytes = maximum_combined_symbol_bytes(&self.combined)?;
        let normalized_render_byte_bound =
            normalized_expression_render_byte_bound(normalized_census, maximum_symbol_bytes)?;
        if normalized_render_byte_bound > self.limits.max_normalized_expression_bytes {
            return Err(
                SymbolicaAffineDenominatorError::NormalizedExpressionTooLarge {
                    requested: normalized_render_byte_bound,
                    limit: self.limits.max_normalized_expression_bytes,
                },
            );
        }
        let normalized_expression = evaluated.to_expression();
        let normalized_expression_bytes = normalized_expression.as_view().get_byte_size();
        if normalized_expression_bytes > self.limits.max_normalized_expression_bytes {
            return Err(
                SymbolicaAffineDenominatorError::NormalizedExpressionTooLarge {
                    requested: normalized_expression_bytes,
                    limit: self.limits.max_normalized_expression_bytes,
                },
            );
        }

        let (affine_denominator, projection_stats) = self.project_affine_denominator(
            &evaluated,
            &mut evaluator.work,
            &mut evaluator.projection_work,
        )?;
        let variable_map_arc_bytes = retained_variable_map_arc_bytes(
            std::iter::once(affine_denominator.constant())
                .chain(affine_denominator.coefficients().iter()),
        )?;
        let compiled_retained_bytes = compiled_retained_byte_bound(
            input_bytes,
            normalized_expression_bytes,
            projection_stats.projected_retained_bytes,
            variable_map_arc_bytes,
        )?;
        check_limit(
            "compiled retained bytes",
            compiled_retained_bytes,
            self.limits.max_compiled_retained_bytes,
        )?;
        Ok(CompiledSymbolicaAffineDenominator {
            source: source.to_owned(),
            normalized_expression,
            affine_denominator,
        })
    }

    /// Evaluate an Atom in the same checked parser, proving it is momentum free.
    pub fn parse_base_coefficient(
        &self,
        source: AtomView<'_>,
    ) -> Result<Coefficient, SymbolicaAffineDenominatorError> {
        catch_unwind(AssertUnwindSafe(|| {
            let input_bytes = source.get_byte_size();
            check_limit(
                "input expression bytes",
                input_bytes,
                self.limits.max_input_bytes,
            )?;
            checked_atom_shape(source, self.limits)?;
            let mut evaluator = CheckedEvaluator::new(self);
            let value = evaluator.evaluate(source, false)?;
            self.combined
                .validate_with_limits(&value, self.limits.exact_algebra)?;
            self.validate_retained_shape(&value)?;
            if coefficient_contains_momentum(&value, self.base_count())? {
                return Err(SymbolicaAffineDenominatorError::BaseCoefficientContainsMomentum);
            }
            self.project_complete_coefficient(
                &value,
                &mut evaluator.work,
                &mut evaluator.projection_work,
            )
        }))
        .map_err(|_| SymbolicaAffineDenominatorError::SymbolicaPanic {
            stage: "base-coefficient evaluation",
        })?
    }

    fn base_count(&self) -> usize {
        self.coefficients.parameter_names().len()
    }

    fn validate_retained_shape(
        &self,
        coefficient: &Coefficient,
    ) -> Result<CoefficientCensus, SymbolicaAffineDenominatorError> {
        let numerator_terms = coefficient.numerator.nterms();
        let denominator_terms = coefficient.denominator.nterms();
        check_limit(
            "combined numerator terms",
            numerator_terms,
            self.limits.max_combined_polynomial_terms,
        )?;
        check_limit(
            "combined denominator terms",
            denominator_terms,
            self.limits.max_combined_polynomial_terms,
        )?;
        let all_terms = numerator_terms.checked_add(denominator_terms).ok_or(
            SymbolicaAffineDenominatorError::ResourceCountOverflow {
                resource: "combined polynomial terms",
            },
        )?;
        let exponent_entries = all_terms
            .checked_mul(self.combined.parameter_names().len())
            .ok_or(SymbolicaAffineDenominatorError::ResourceCountOverflow {
                resource: "combined exponent entries",
            })?;
        check_limit(
            "combined exponent entries",
            exponent_entries,
            self.limits.max_combined_exponent_entries,
        )?;
        let census = coefficient_census(coefficient)?;
        check_limit(
            "combined coefficient integer bits",
            census.integer_bits,
            self.limits.max_coefficient_integer_bits,
        )?;
        check_limit(
            "combined retained bytes",
            census.retained_bytes,
            self.limits.max_combined_retained_bytes,
        )?;
        Ok(census)
    }

    fn preflight_binary_shape(
        &self,
        left: &Coefficient,
        right: &Coefficient,
        operation: BinaryOperation,
        work: &mut ExactWorkBudget,
    ) -> Result<ExactOperationAllocationEnvelope, SymbolicaAffineDenominatorError> {
        charge_dense_degree_box(
            left,
            right,
            operation,
            self.combined.parameter_names().len(),
            self.limits,
            work,
        )?;
        let allocation = exact_operation_allocation_envelope(
            left,
            right,
            operation,
            self.combined.parameter_names().len(),
        )?;
        check_limit(
            "combined exact-operation numerator term envelope",
            allocation.numerator_terms,
            self.limits.max_combined_polynomial_terms,
        )?;
        check_limit(
            "combined exact-operation denominator term envelope",
            allocation.denominator_terms,
            self.limits.max_combined_polynomial_terms,
        )?;
        check_limit(
            "combined exact-operation exponent-entry envelope",
            allocation.census.exponent_entries,
            self.limits.max_combined_exponent_entries,
        )?;
        check_limit(
            "combined exact-operation integer bits",
            allocation.census.integer_bits,
            self.limits.max_coefficient_integer_bits,
        )?;
        check_limit(
            "combined exact-operation retained bytes",
            allocation.census.retained_bytes,
            self.limits.max_combined_retained_bytes,
        )?;
        Ok(allocation)
    }

    fn checked_add(
        &self,
        left: &Coefficient,
        right: &Coefficient,
        work: &mut ExactWorkBudget,
    ) -> Result<Coefficient, SymbolicaAffineDenominatorError> {
        let allocation = self.preflight_binary_shape(left, right, BinaryOperation::Add, work)?;
        let result = self
            .combined
            .try_add(left, right, self.limits.exact_algebra)?;
        let actual = self.validate_retained_shape(&result)?;
        verify_operation_result_envelope(&result, actual, allocation)?;
        Ok(result)
    }

    fn checked_mul(
        &self,
        left: &Coefficient,
        right: &Coefficient,
        work: &mut ExactWorkBudget,
    ) -> Result<Coefficient, SymbolicaAffineDenominatorError> {
        let allocation =
            self.preflight_binary_shape(left, right, BinaryOperation::Multiply, work)?;
        let result = self
            .combined
            .try_mul(left, right, self.limits.exact_algebra)?;
        let actual = self.validate_retained_shape(&result)?;
        verify_operation_result_envelope(&result, actual, allocation)?;
        Ok(result)
    }

    fn checked_div(
        &self,
        numerator: &Coefficient,
        denominator: &Coefficient,
        work: &mut ExactWorkBudget,
    ) -> Result<Coefficient, SymbolicaAffineDenominatorError> {
        let allocation =
            self.preflight_binary_shape(numerator, denominator, BinaryOperation::Divide, work)?;
        let result = self
            .combined
            .try_div(numerator, denominator, self.limits.exact_algebra)?;
        let actual = self.validate_retained_shape(&result)?;
        verify_operation_result_envelope(&result, actual, allocation)?;
        Ok(result)
    }

    fn numeric_coefficient(
        &self,
        atom: AtomView<'_>,
    ) -> Result<Coefficient, SymbolicaAffineDenominatorError> {
        let AtomView::Num(number) = atom else {
            return Err(SymbolicaAffineDenominatorError::UnsupportedNumericAtom(
                atom.to_owned(),
            ));
        };
        let (numerator_bits, denominator_bits) = match number.get_coeff_view() {
            CoefficientView::Natural(real_numerator, real_denominator, imaginary, _)
                if imaginary == 0 =>
            {
                (
                    signed_i64_magnitude_bits(real_numerator),
                    signed_i64_magnitude_bits(real_denominator),
                )
            }
            CoefficientView::Large(real, imaginary) if imaginary.is_zero() => match real {
                SerializedRational::Natural(numerator, denominator) => (
                    signed_i64_magnitude_bits(numerator),
                    signed_i64_magnitude_bits(denominator),
                ),
                // The packed large-rational fields are intentionally opaque.
                // Their complete serialized Atom size is a conservative bit
                // envelope and can be inspected without cloning GMP storage.
                SerializedRational::Large(_) => {
                    let bits = atom.get_byte_size().checked_mul(8).ok_or(
                        SymbolicaAffineDenominatorError::ResourceCountOverflow {
                            resource: "numeric Atom magnitude bits",
                        },
                    )?;
                    (bits, bits)
                }
            },
            _ => {
                return Err(SymbolicaAffineDenominatorError::UnsupportedNumericAtom(
                    atom.to_owned(),
                ));
            }
        };
        let mut planned = planned_operation_polynomial_census(
            1,
            self.combined.parameter_names().len(),
            numerator_bits,
        )?;
        planned.checked_add_assign(
            planned_operation_polynomial_census(
                1,
                self.combined.parameter_names().len(),
                denominator_bits,
            )?,
            "numeric Atom allocation envelope",
        )?;
        check_limit(
            "numeric Atom integer bits",
            planned.integer_bits,
            self.limits.max_coefficient_integer_bits,
        )?;
        check_limit(
            "numeric Atom retained bytes",
            planned.retained_bytes,
            self.limits.max_combined_retained_bytes,
        )?;
        let result = atom
            .try_to_rational_polynomial(&Q, &Z, Some(self.combined.variables().clone()))
            .map_err(|_| {
                SymbolicaAffineDenominatorError::UnsupportedNumericAtom(atom.to_owned())
            })?;
        self.combined
            .validate_with_limits(&result, self.limits.exact_algebra)?;
        self.validate_retained_shape(&result)?;
        Ok(result)
    }

    fn validate_vector_linear(
        &self,
        coefficient: &Coefficient,
        argument: usize,
        atom: AtomView<'_>,
    ) -> Result<(), SymbolicaAffineDenominatorError> {
        if polynomial_contains_momentum(&coefficient.denominator, self.base_count())? {
            return Err(
                SymbolicaAffineDenominatorError::InvalidScalarProductArgument {
                    argument,
                    atom: atom.to_owned(),
                },
            );
        }
        for exponents in coefficient.numerator.exponents_iter() {
            if momentum_degree(exponents, self.base_count())? != 1 {
                return Err(
                    SymbolicaAffineDenominatorError::InvalidScalarProductArgument {
                        argument,
                        atom: atom.to_owned(),
                    },
                );
            }
        }
        Ok(())
    }

    fn contract_explicit_scalar_product(
        &self,
        coefficient: Coefficient,
        work: &mut ExactWorkBudget,
        projection_work: &mut ProjectionAllocationBudget,
    ) -> Result<Coefficient, SymbolicaAffineDenominatorError> {
        let base_count = self.base_count();
        let loops = self.loop_momenta.len();
        let externals = self.external_momenta.len();
        let loop_loop_count = upper_triangular_count(loops)?;
        let mut external_counts = BTreeMap::<(usize, usize), usize>::new();
        let mut residual_terms = 0usize;
        for (term, exponents) in coefficient.numerator.exponents_iter().enumerate() {
            match classify_numerator_term(
                exponents,
                self.combined.parameter_names().len(),
                base_count,
                loops,
                externals,
                loop_loop_count,
                term,
            )? {
                ProjectionGroup::ExternalPair(left, right) => {
                    let pair = (left, right);
                    if let Some(count) = external_counts.get_mut(&pair) {
                        *count = count.checked_add(1).ok_or(
                            SymbolicaAffineDenominatorError::ResourceCountOverflow {
                                resource: "explicit scalar-product external terms",
                            },
                        )?;
                    } else {
                        let requested = projection_work
                            .groups
                            .checked_add(external_counts.len())
                            .and_then(|value| value.checked_add(1))
                            .ok_or(SymbolicaAffineDenominatorError::ResourceCountOverflow {
                                resource: "aggregate projection groups",
                            })?;
                        check_limit(
                            "aggregate projection groups",
                            requested,
                            self.limits.max_projection_groups,
                        )?;
                        projection_work.charge(
                            CoefficientCensus {
                                retained_bytes: std::mem::size_of::<((usize, usize), usize)>()
                                    .checked_add(64)
                                    .ok_or(
                                        SymbolicaAffineDenominatorError::ResourceCountOverflow {
                                            resource: "explicit scalar-product group metadata bytes",
                                        },
                                    )?,
                                ..CoefficientCensus::default()
                            },
                            self.limits,
                            "aggregate explicit scalar-product group metadata terms",
                        )?;
                        // `BTreeMap::insert` is the first allocation for this
                        // unique group and happens only after admission.
                        external_counts.insert(pair, 1);
                    }
                }
                ProjectionGroup::Coordinate(_) => {
                    residual_terms = residual_terms.checked_add(1).ok_or(
                        SymbolicaAffineDenominatorError::ResourceCountOverflow {
                            resource: "explicit scalar-product residual terms",
                        },
                    )?;
                }
                ProjectionGroup::Constant => {
                    return Err(
                        SymbolicaAffineDenominatorError::InternalVerificationFailure {
                            detail: "homogeneous scalar product produced a constant numerator term",
                        },
                    );
                }
            }
        }
        if external_counts.is_empty() {
            return Ok(coefficient);
        }

        let groups = external_counts.len();
        let denominator_terms = coefficient.denominator.nterms();
        let denominator_replication_terms = groups.checked_mul(denominator_terms).ok_or(
            SymbolicaAffineDenominatorError::ResourceCountOverflow {
                resource: "explicit scalar-product denominator replication terms",
            },
        )?;
        let gram_operations = groups.checked_mul(2).ok_or(
            SymbolicaAffineDenominatorError::ResourceCountOverflow {
                resource: "explicit scalar-product Gram operations",
            },
        )?;
        projection_work.charge_structure(
            groups,
            denominator_replication_terms,
            gram_operations,
            self.limits,
        )?;
        projection_work.charge(
            planned_polynomial_clone_census(&coefficient.denominator, base_count)?,
            self.limits,
            "aggregate explicit scalar-product denominator terms",
        )?;
        let denominator = project_polynomial_prefix(
            &coefficient.denominator,
            &self.coefficients.template().denominator,
            base_count,
            self.limits.max_projected_exponent_entries,
        )?;
        projection_work.charge(
            multiply_census(
                polynomial_census(&denominator)?,
                groups,
                "explicit scalar-product denominator replication census",
            )?,
            self.limits,
            "aggregate explicit scalar-product denominator replication terms",
        )?;
        // Both allocations below are charged with the complete source support;
        // this safely overbounds the disjoint external/residual partition.
        projection_work.charge(
            planned_polynomial_clone_census(&coefficient.numerator, base_count)?,
            self.limits,
            "aggregate explicit scalar-product external group terms",
        )?;
        projection_work.charge(
            planned_polynomial_clone_census(
                &coefficient.numerator,
                self.combined.parameter_names().len(),
            )?,
            self.limits,
            "aggregate explicit scalar-product residual terms",
        )?;
        let external_group_metadata_bytes = groups
            .checked_mul(
                std::mem::size_of::<((usize, usize), CoefficientPolynomial)>()
                    .checked_add(64)
                    .ok_or(SymbolicaAffineDenominatorError::ResourceCountOverflow {
                        resource: "explicit scalar-product group metadata bytes",
                    })?,
            )
            .ok_or(SymbolicaAffineDenominatorError::ResourceCountOverflow {
                resource: "explicit scalar-product group metadata bytes",
            })?;
        projection_work.charge(
            CoefficientCensus {
                retained_bytes: external_group_metadata_bytes,
                ..CoefficientCensus::default()
            },
            self.limits,
            "aggregate explicit scalar-product group metadata terms",
        )?;

        let mut external_groups = BTreeMap::<(usize, usize), CoefficientPolynomial>::new();
        for (pair, count) in external_counts {
            external_groups.insert(
                pair,
                self.coefficients
                    .template()
                    .numerator
                    .zero_with_capacity(count),
            );
        }
        let mut residual_numerator = coefficient.numerator.zero_with_capacity(residual_terms);
        for (term, (integer, exponents)) in coefficient
            .numerator
            .coefficients
            .iter()
            .zip(coefficient.numerator.exponents_iter())
            .enumerate()
        {
            match classify_numerator_term(
                exponents,
                self.combined.parameter_names().len(),
                base_count,
                loops,
                externals,
                loop_loop_count,
                term,
            )? {
                ProjectionGroup::ExternalPair(left, right) => external_groups
                    .get_mut(&(left, right))
                    .ok_or(
                        SymbolicaAffineDenominatorError::InternalVerificationFailure {
                            detail: "explicit scalar-product group count was not retained",
                        },
                    )?
                    .append_monomial(integer.clone(), &exponents[..base_count]),
                ProjectionGroup::Coordinate(_) => {
                    residual_numerator.append_monomial(integer.clone(), exponents)
                }
                ProjectionGroup::Constant => {
                    return Err(
                        SymbolicaAffineDenominatorError::InternalVerificationFailure {
                            detail: "homogeneous scalar product produced a constant numerator term",
                        },
                    );
                }
            }
        }

        let mut residual = if residual_numerator.is_zero() {
            self.combined.zero()
        } else {
            let numerator: Coefficient = residual_numerator.into();
            projection_work.charge(
                planned_polynomial_clone_census(
                    &coefficient.denominator,
                    self.combined.parameter_names().len(),
                )?,
                self.limits,
                "aggregate explicit scalar-product residual denominator terms",
            )?;
            let denominator_coefficient: Coefficient = coefficient.denominator.clone().into();
            projection_work.charge(
                exact_operation_allocation_envelope(
                    &numerator,
                    &denominator_coefficient,
                    BinaryOperation::Divide,
                    self.combined.parameter_names().len(),
                )?
                .census,
                self.limits,
                "aggregate explicit scalar-product residual division terms",
            )?;
            self.checked_div(&numerator, &denominator_coefficient, work)?
        };
        projection_work.charge(
            planned_coefficient_clone_census(self.coefficients.template(), self.base_count())?,
            self.limits,
            "aggregate explicit scalar-product accumulator terms",
        )?;
        let external_zero = self.coefficients.zero();
        let mut external_constant = external_zero;
        for ((left, right), numerator) in external_groups {
            let value =
                self.projected_rational(numerator, denominator.clone(), work, projection_work)?;
            let gram = self
                .external_gram
                .get(left)
                .and_then(|row| row.get(right))
                .ok_or(
                    SymbolicaAffineDenominatorError::InternalVerificationFailure {
                        detail: "explicit scalar-product Gram coordinate is out of range",
                    },
                )?;
            let contribution = self.projected_checked_mul(&value, gram, work, projection_work)?;
            external_constant = self.projected_checked_add(
                &external_constant,
                &contribution,
                work,
                projection_work,
            )?;
        }
        let lifted = self.lift_base_coefficient(&external_constant, projection_work)?;
        projection_work.charge(
            exact_operation_allocation_envelope(
                &residual,
                &lifted,
                BinaryOperation::Add,
                self.combined.parameter_names().len(),
            )?
            .census,
            self.limits,
            "aggregate explicit scalar-product lifted addition terms",
        )?;
        residual = self.checked_add(&residual, &lifted, work)?;
        Ok(residual)
    }

    fn lift_base_coefficient(
        &self,
        coefficient: &Coefficient,
        projection_work: &mut ProjectionAllocationBudget,
    ) -> Result<Coefficient, SymbolicaAffineDenominatorError> {
        let combined_variables = self.combined.parameter_names().len();
        projection_work.charge(
            planned_polynomial_clone_census(&coefficient.numerator, combined_variables)?,
            self.limits,
            "aggregate lifted numerator terms",
        )?;
        projection_work.charge(
            planned_polynomial_clone_census(&coefficient.denominator, combined_variables)?,
            self.limits,
            "aggregate lifted denominator terms",
        )?;
        let numerator = lift_polynomial_prefix(
            &coefficient.numerator,
            &self.combined.template().numerator,
            self.base_count(),
            self.limits.max_combined_exponent_entries,
        )?;
        let denominator = lift_polynomial_prefix(
            &coefficient.denominator,
            &self.combined.template().denominator,
            self.base_count(),
            self.limits.max_combined_exponent_entries,
        )?;
        let lifted = Coefficient::from_num_den(numerator, denominator, &Z, false);
        self.combined
            .validate_with_limits(&lifted, self.limits.exact_algebra)?;
        self.validate_retained_shape(&lifted)?;
        Ok(lifted)
    }

    fn project_affine_denominator(
        &self,
        coefficient: &Coefficient,
        work: &mut ExactWorkBudget,
        projection_work: &mut ProjectionAllocationBudget,
    ) -> Result<(AffineDenominator, ProjectionStats), SymbolicaAffineDenominatorError> {
        let base_count = self.base_count();
        let loops = self.loop_momenta.len();
        let externals = self.external_momenta.len();
        let loop_loop_count = upper_triangular_count(loops)?;
        projection_work.charge(
            planned_polynomial_clone_census(&coefficient.denominator, base_count)?,
            self.limits,
            "aggregate projected denominator terms",
        )?;
        let denominator = project_polynomial_prefix(
            &coefficient.denominator,
            &self.coefficients.template().denominator,
            base_count,
            self.limits.max_combined_exponent_entries,
        )?;
        let mut group_counts = BTreeMap::<ProjectionGroup, usize>::new();
        for (term, exponents) in coefficient.numerator.exponents_iter().enumerate() {
            let group = classify_numerator_term(
                exponents,
                self.combined.parameter_names().len(),
                base_count,
                loops,
                externals,
                loop_loop_count,
                term,
            )?;
            if let Some(count) = group_counts.get_mut(&group) {
                *count = count.checked_add(1).ok_or(
                    SymbolicaAffineDenominatorError::ResourceCountOverflow {
                        resource: "projected group terms",
                    },
                )?;
            } else {
                let requested = projection_work
                    .groups
                    .checked_add(group_counts.len())
                    .and_then(|value| value.checked_add(1))
                    .ok_or(SymbolicaAffineDenominatorError::ResourceCountOverflow {
                        resource: "aggregate projection groups",
                    })?;
                check_limit(
                    "aggregate projection groups",
                    requested,
                    self.limits.max_projection_groups,
                )?;
                projection_work.charge(
                    CoefficientCensus {
                        retained_bytes: std::mem::size_of::<(ProjectionGroup, usize)>()
                            .checked_add(64)
                            .ok_or(SymbolicaAffineDenominatorError::ResourceCountOverflow {
                                resource: "projection count-group metadata bytes",
                            })?,
                        ..CoefficientCensus::default()
                    },
                    self.limits,
                    "aggregate projection count-group metadata terms",
                )?;
                group_counts.insert(group, 1);
            }
        }
        let projected_numerator_terms =
            group_counts.values().try_fold(0usize, |total, count| {
                total.checked_add(*count).ok_or(
                    SymbolicaAffineDenominatorError::ResourceCountOverflow {
                        resource: "projected numerator terms",
                    },
                )
            })?;
        check_limit(
            "projected numerator terms",
            projected_numerator_terms,
            self.limits.max_projected_polynomial_terms,
        )?;
        let projected_exponent_entries = projected_numerator_terms.checked_mul(base_count).ok_or(
            SymbolicaAffineDenominatorError::ResourceCountOverflow {
                resource: "projected numerator exponent entries",
            },
        )?;
        check_limit(
            "projected numerator exponent entries",
            projected_exponent_entries,
            self.limits.max_projected_exponent_entries,
        )?;

        let projection_groups = group_counts.len();
        check_limit(
            "projection groups",
            projection_groups,
            self.limits.max_projection_groups,
        )?;
        let projection_denominator_replication_terms = projection_groups
            .checked_mul(denominator.nterms())
            .ok_or(SymbolicaAffineDenominatorError::ResourceCountOverflow {
                resource: "projection denominator replication terms",
            })?;
        check_limit(
            "projection denominator replication terms",
            projection_denominator_replication_terms,
            self.limits.max_projection_denominator_replication_terms,
        )?;
        let replication_entries = projection_denominator_replication_terms
            .checked_mul(base_count)
            .ok_or(SymbolicaAffineDenominatorError::ResourceCountOverflow {
                resource: "projection denominator replication exponent entries",
            })?;
        check_limit(
            "projection denominator replication exponent entries",
            replication_entries,
            self.limits
                .max_projection_denominator_replication_exponent_entries,
        )?;
        let projection_gram_operations = group_counts
            .keys()
            .filter(|group| matches!(group, ProjectionGroup::ExternalPair(_, _)))
            .count()
            .checked_mul(2)
            .ok_or(SymbolicaAffineDenominatorError::ResourceCountOverflow {
                resource: "projection Gram operations",
            })?;
        check_limit(
            "projection Gram operations",
            projection_gram_operations,
            self.limits.max_projection_gram_operations,
        )?;
        projection_work.charge_structure(
            projection_groups,
            projection_denominator_replication_terms,
            projection_gram_operations,
            self.limits,
        )?;

        let denominator_replication_census = multiply_census(
            polynomial_census(&denominator)?,
            projection_groups,
            "projection denominator replication census",
        )?;
        projection_work.charge(
            denominator_replication_census,
            self.limits,
            "aggregate projection denominator replication terms",
        )?;
        projection_work.charge(
            planned_polynomial_clone_census(&coefficient.numerator, base_count)?,
            self.limits,
            "aggregate projected group-polynomial terms",
        )?;
        let group_metadata_bytes = projection_groups
            .checked_mul(
                std::mem::size_of::<(ProjectionGroup, CoefficientPolynomial)>()
                    .checked_add(64)
                    .ok_or(SymbolicaAffineDenominatorError::ResourceCountOverflow {
                        resource: "projection group metadata bytes",
                    })?,
            )
            .ok_or(SymbolicaAffineDenominatorError::ResourceCountOverflow {
                resource: "projection group metadata bytes",
            })?;
        projection_work.charge(
            CoefficientCensus {
                retained_bytes: group_metadata_bytes,
                ..CoefficientCensus::default()
            },
            self.limits,
            "aggregate projection group metadata terms",
        )?;

        let mut groups = BTreeMap::<ProjectionGroup, CoefficientPolynomial>::new();
        for (group, count) in group_counts {
            groups.insert(
                group,
                self.coefficients
                    .template()
                    .numerator
                    .zero_with_capacity(count),
            );
        }
        for (term, (integer, exponents)) in coefficient
            .numerator
            .coefficients
            .iter()
            .zip(coefficient.numerator.exponents_iter())
            .enumerate()
        {
            let group = classify_numerator_term(
                exponents,
                self.combined.parameter_names().len(),
                base_count,
                loops,
                externals,
                loop_loop_count,
                term,
            )?;
            groups
                .get_mut(&group)
                .ok_or(
                    SymbolicaAffineDenominatorError::InternalVerificationFailure {
                        detail: "projected group count was not retained",
                    },
                )?
                .append_monomial(integer.clone(), &exponents[..base_count]);
        }

        let zero_slots = self.coordinates.len().checked_add(1).ok_or(
            SymbolicaAffineDenominatorError::ResourceCountOverflow {
                resource: "projected affine coordinate baseline",
            },
        )?;
        projection_work.charge(
            multiply_census(
                planned_coefficient_clone_census(self.coefficients.template(), self.base_count())?,
                zero_slots,
                "projected affine coordinate baseline",
            )?,
            self.limits,
            "aggregate projected affine coordinate baseline terms",
        )?;
        let zero = self.coefficients.zero();
        let mut constant = zero.clone();
        let mut coordinates = Vec::new();
        coordinates
            .try_reserve_exact(self.coordinates.len())
            .map_err(|_| SymbolicaAffineDenominatorError::AllocationFailure {
                resource: "affine scalar-product coefficients",
                requested: self.coordinates.len(),
            })?;
        coordinates.resize_with(self.coordinates.len(), || zero.clone());
        for (group, numerator) in groups {
            let value =
                self.projected_rational(numerator, denominator.clone(), work, projection_work)?;
            match group {
                ProjectionGroup::Constant => constant = value,
                ProjectionGroup::Coordinate(position) => {
                    let target = coordinates.get_mut(position).ok_or(
                        SymbolicaAffineDenominatorError::InternalVerificationFailure {
                            detail: "quadratic coordinate index is out of range",
                        },
                    )?;
                    *target = value;
                }
                ProjectionGroup::ExternalPair(left, right) => {
                    let gram = self
                        .external_gram
                        .get(left)
                        .and_then(|row| row.get(right))
                        .ok_or(
                            SymbolicaAffineDenominatorError::InternalVerificationFailure {
                                detail: "external Gram coordinate is out of range",
                            },
                        )?;
                    let contribution =
                        self.projected_checked_mul(&value, gram, work, projection_work)?;
                    constant = self.projected_checked_add(
                        &constant,
                        &contribution,
                        work,
                        projection_work,
                    )?;
                }
            }
        }
        self.coefficients
            .validate_with_limits(&constant, self.limits.exact_algebra)?;
        for value in &coordinates {
            self.coefficients
                .validate_with_limits(value, self.limits.exact_algebra)?;
        }
        let mut output_census = coefficient_census(&constant)?;
        for value in &coordinates {
            output_census
                .checked_add_assign(coefficient_census(value)?, "projected affine-row census")?;
        }
        check_limit(
            "projected polynomial terms",
            output_census.polynomial_terms,
            self.limits.max_projected_polynomial_terms,
        )?;
        check_limit(
            "projected exponent entries",
            output_census.exponent_entries,
            self.limits.max_projected_exponent_entries,
        )?;
        check_limit(
            "projected integer bits",
            output_census.integer_bits,
            self.limits.max_projected_integer_bits,
        )?;
        check_limit(
            "projected retained bytes",
            output_census.retained_bytes,
            self.limits.max_projected_retained_bytes,
        )?;
        Ok((
            AffineDenominator::new(constant, coordinates),
            ProjectionStats {
                projected_retained_bytes: output_census.retained_bytes,
            },
        ))
    }

    fn project_complete_coefficient(
        &self,
        coefficient: &Coefficient,
        work: &mut ExactWorkBudget,
        projection_work: &mut ProjectionAllocationBudget,
    ) -> Result<Coefficient, SymbolicaAffineDenominatorError> {
        projection_work.charge(
            planned_polynomial_clone_census(&coefficient.numerator, self.base_count())?,
            self.limits,
            "aggregate projected complete numerator terms",
        )?;
        projection_work.charge(
            planned_polynomial_clone_census(&coefficient.denominator, self.base_count())?,
            self.limits,
            "aggregate projected complete denominator terms",
        )?;
        let numerator = project_polynomial_prefix(
            &coefficient.numerator,
            &self.coefficients.template().numerator,
            self.base_count(),
            self.limits.max_combined_exponent_entries,
        )?;
        let denominator = project_polynomial_prefix(
            &coefficient.denominator,
            &self.coefficients.template().denominator,
            self.base_count(),
            self.limits.max_combined_exponent_entries,
        )?;
        self.projected_rational(numerator, denominator, work, projection_work)
    }

    fn projected_rational(
        &self,
        numerator: CoefficientPolynomial,
        denominator: CoefficientPolynomial,
        work: &mut ExactWorkBudget,
        projection_work: &mut ProjectionAllocationBudget,
    ) -> Result<Coefficient, SymbolicaAffineDenominatorError> {
        if numerator.is_zero() {
            projection_work.charge(
                planned_coefficient_clone_census(self.coefficients.template(), self.base_count())?,
                self.limits,
                "aggregate projected zero coefficient terms",
            )?;
            return Ok(self.coefficients.zero());
        }
        let numerator: Coefficient = numerator.into();
        let denominator: Coefficient = denominator.into();
        self.projected_checked_div(&numerator, &denominator, work, projection_work)
    }

    fn validate_projected_coefficient(
        &self,
        coefficient: &Coefficient,
    ) -> Result<(), SymbolicaAffineDenominatorError> {
        self.coefficients
            .validate_with_limits(coefficient, self.limits.exact_algebra)?;
        let census = coefficient_census(coefficient)?;
        check_limit(
            "one projected coefficient integer bits",
            census.integer_bits,
            self.limits.max_projected_integer_bits,
        )
    }

    fn projected_checked_add(
        &self,
        left: &Coefficient,
        right: &Coefficient,
        work: &mut ExactWorkBudget,
        projection_work: &mut ProjectionAllocationBudget,
    ) -> Result<Coefficient, SymbolicaAffineDenominatorError> {
        charge_dense_degree_box(
            left,
            right,
            BinaryOperation::Add,
            self.base_count(),
            self.limits,
            work,
        )?;
        let allocation = exact_operation_allocation_envelope(
            left,
            right,
            BinaryOperation::Add,
            self.base_count(),
        )?;
        projection_work.charge(
            allocation.census,
            self.limits,
            "aggregate projected exact-operation terms",
        )?;
        let result = self
            .coefficients
            .try_add(left, right, self.limits.exact_algebra)?;
        self.validate_projected_coefficient(&result)?;
        verify_operation_result_envelope(&result, coefficient_census(&result)?, allocation)?;
        Ok(result)
    }

    fn projected_checked_mul(
        &self,
        left: &Coefficient,
        right: &Coefficient,
        work: &mut ExactWorkBudget,
        projection_work: &mut ProjectionAllocationBudget,
    ) -> Result<Coefficient, SymbolicaAffineDenominatorError> {
        charge_dense_degree_box(
            left,
            right,
            BinaryOperation::Multiply,
            self.base_count(),
            self.limits,
            work,
        )?;
        let allocation = exact_operation_allocation_envelope(
            left,
            right,
            BinaryOperation::Multiply,
            self.base_count(),
        )?;
        projection_work.charge(
            allocation.census,
            self.limits,
            "aggregate projected exact-operation terms",
        )?;
        let result = self
            .coefficients
            .try_mul(left, right, self.limits.exact_algebra)?;
        self.validate_projected_coefficient(&result)?;
        verify_operation_result_envelope(&result, coefficient_census(&result)?, allocation)?;
        Ok(result)
    }

    fn projected_checked_div(
        &self,
        numerator: &Coefficient,
        denominator: &Coefficient,
        work: &mut ExactWorkBudget,
        projection_work: &mut ProjectionAllocationBudget,
    ) -> Result<Coefficient, SymbolicaAffineDenominatorError> {
        charge_dense_degree_box(
            numerator,
            denominator,
            BinaryOperation::Divide,
            self.base_count(),
            self.limits,
            work,
        )?;
        let allocation = exact_operation_allocation_envelope(
            numerator,
            denominator,
            BinaryOperation::Divide,
            self.base_count(),
        )?;
        projection_work.charge(
            allocation.census,
            self.limits,
            "aggregate projected exact-operation terms",
        )?;
        let result =
            self.coefficients
                .try_div(numerator, denominator, self.limits.exact_algebra)?;
        self.validate_projected_coefficient(&result)?;
        verify_operation_result_envelope(&result, coefficient_census(&result)?, allocation)?;
        Ok(result)
    }
}

#[derive(Clone, Copy)]
enum BinaryOperation {
    Add,
    Multiply,
    Divide,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
struct ExactWorkBudget {
    dense_degree_box_terms: usize,
    dense_degree_box_exponent_entries: usize,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
struct ProjectionStats {
    projected_retained_bytes: usize,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
struct ProjectionAllocationBudget {
    polynomial_terms: usize,
    exponent_entries: usize,
    integer_bits: usize,
    retained_bytes: usize,
    groups: usize,
    denominator_replication_terms: usize,
    gram_operations: usize,
}

impl ProjectionAllocationBudget {
    fn charge_structure(
        &mut self,
        groups: usize,
        denominator_replication_terms: usize,
        gram_operations: usize,
        limits: SymbolicaAffineDenominatorLimits,
    ) -> Result<(), SymbolicaAffineDenominatorError> {
        self.groups = self.groups.checked_add(groups).ok_or(
            SymbolicaAffineDenominatorError::ResourceCountOverflow {
                resource: "aggregate projection groups",
            },
        )?;
        self.denominator_replication_terms = self
            .denominator_replication_terms
            .checked_add(denominator_replication_terms)
            .ok_or(SymbolicaAffineDenominatorError::ResourceCountOverflow {
                resource: "aggregate projection denominator replication terms",
            })?;
        self.gram_operations = self.gram_operations.checked_add(gram_operations).ok_or(
            SymbolicaAffineDenominatorError::ResourceCountOverflow {
                resource: "aggregate projection Gram operations",
            },
        )?;
        check_limit(
            "aggregate projection groups",
            self.groups,
            limits.max_projection_groups,
        )?;
        check_limit(
            "aggregate projection denominator replication terms",
            self.denominator_replication_terms,
            limits.max_projection_denominator_replication_terms,
        )?;
        check_limit(
            "aggregate projection Gram operations",
            self.gram_operations,
            limits.max_projection_gram_operations,
        )
    }

    fn charge(
        &mut self,
        census: CoefficientCensus,
        limits: SymbolicaAffineDenominatorLimits,
        resource: &'static str,
    ) -> Result<(), SymbolicaAffineDenominatorError> {
        self.polynomial_terms = self
            .polynomial_terms
            .checked_add(census.polynomial_terms)
            .ok_or(SymbolicaAffineDenominatorError::ResourceCountOverflow { resource })?;
        self.exponent_entries = self
            .exponent_entries
            .checked_add(census.exponent_entries)
            .ok_or(SymbolicaAffineDenominatorError::ResourceCountOverflow { resource })?;
        self.integer_bits = self
            .integer_bits
            .checked_add(census.integer_bits)
            .ok_or(SymbolicaAffineDenominatorError::ResourceCountOverflow { resource })?;
        self.retained_bytes = self
            .retained_bytes
            .checked_add(census.retained_bytes)
            .ok_or(SymbolicaAffineDenominatorError::ResourceCountOverflow { resource })?;
        check_limit(
            resource,
            self.polynomial_terms,
            limits.max_projected_polynomial_terms,
        )?;
        check_limit(
            "aggregate projected exponent entries",
            self.exponent_entries,
            limits.max_projected_exponent_entries,
        )?;
        check_limit(
            "aggregate projected integer bits",
            self.integer_bits,
            limits.max_projected_integer_bits,
        )?;
        check_limit(
            "aggregate projected retained bytes",
            self.retained_bytes,
            limits.max_projected_retained_bytes,
        )
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
struct CoefficientCensus {
    polynomial_terms: usize,
    exponent_entries: usize,
    integer_bits: usize,
    retained_bytes: usize,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
struct NormalizedExpressionCensus {
    nodes: usize,
    integer_bits: usize,
}

impl CoefficientCensus {
    fn checked_add_assign(
        &mut self,
        other: Self,
        resource: &'static str,
    ) -> Result<(), SymbolicaAffineDenominatorError> {
        self.polynomial_terms = self
            .polynomial_terms
            .checked_add(other.polynomial_terms)
            .ok_or(SymbolicaAffineDenominatorError::ResourceCountOverflow { resource })?;
        self.exponent_entries = self
            .exponent_entries
            .checked_add(other.exponent_entries)
            .ok_or(SymbolicaAffineDenominatorError::ResourceCountOverflow { resource })?;
        self.integer_bits = self
            .integer_bits
            .checked_add(other.integer_bits)
            .ok_or(SymbolicaAffineDenominatorError::ResourceCountOverflow { resource })?;
        self.retained_bytes = self
            .retained_bytes
            .checked_add(other.retained_bytes)
            .ok_or(SymbolicaAffineDenominatorError::ResourceCountOverflow { resource })?;
        Ok(())
    }
}

fn integer_magnitude_bits(integer: &Integer) -> Result<usize, SymbolicaAffineDenominatorError> {
    let bits = match integer {
        Integer::Single(value) => u64::from(u64::BITS - value.unsigned_abs().leading_zeros()),
        Integer::Double(value) => u64::from(u128::BITS - value.unsigned_abs().leading_zeros()),
        Integer::Large(value) => u64::from(value.significant_bits()),
    };
    usize::try_from(bits).map_err(|_| SymbolicaAffineDenominatorError::ResourceCountOverflow {
        resource: "integer magnitude bits",
    })
}

fn signed_i64_magnitude_bits(value: i64) -> usize {
    (u64::BITS - value.unsigned_abs().leading_zeros()) as usize
}

fn integer_owned_heap_bytes(integer: &Integer) -> Result<usize, SymbolicaAffineDenominatorError> {
    match integer {
        Integer::Single(_) | Integer::Double(_) => Ok(0),
        Integer::Large(value) => usize::try_from(value.capacity())
            .map_err(|_| SymbolicaAffineDenominatorError::ResourceCountOverflow {
                resource: "integer owned heap bytes",
            })?
            .checked_add(7)
            .map(|bits| bits / 8)
            .ok_or(SymbolicaAffineDenominatorError::ResourceCountOverflow {
                resource: "integer owned heap bytes",
            }),
    }
}

fn polynomial_census(
    polynomial: &CoefficientPolynomial,
) -> Result<CoefficientCensus, SymbolicaAffineDenominatorError> {
    let polynomial_terms = polynomial.nterms();
    let exponent_entries = polynomial_terms
        .checked_mul(polynomial.variables.len())
        .ok_or(SymbolicaAffineDenominatorError::ResourceCountOverflow {
            resource: "polynomial census exponent entries",
        })?;
    if polynomial.exponents.len() != exponent_entries
        || polynomial.coefficients.len() != polynomial_terms
    {
        return Err(
            SymbolicaAffineDenominatorError::InternalVerificationFailure {
                detail: "polynomial census found a malformed storage layout",
            },
        );
    }
    let integer_bits = polynomial
        .coefficients
        .iter()
        .try_fold(0usize, |total, integer| {
            total.checked_add(integer_magnitude_bits(integer)?).ok_or(
                SymbolicaAffineDenominatorError::ResourceCountOverflow {
                    resource: "polynomial census integer bits",
                },
            )
        })?;
    let integer_slots = polynomial
        .coefficients
        .capacity()
        .checked_mul(std::mem::size_of::<Integer>())
        .ok_or(SymbolicaAffineDenominatorError::ResourceCountOverflow {
            resource: "polynomial retained bytes",
        })?;
    let exponent_bytes = polynomial
        .exponents
        .capacity()
        .checked_mul(std::mem::size_of::<u16>())
        .ok_or(SymbolicaAffineDenominatorError::ResourceCountOverflow {
            resource: "polynomial retained bytes",
        })?;
    let limb_bytes = polynomial
        .coefficients
        .iter()
        .try_fold(0usize, |total, integer| {
            total.checked_add(integer_owned_heap_bytes(integer)?).ok_or(
                SymbolicaAffineDenominatorError::ResourceCountOverflow {
                    resource: "polynomial retained bytes",
                },
            )
        })?;
    let retained_bytes = std::mem::size_of::<CoefficientPolynomial>()
        .checked_add(integer_slots)
        .and_then(|value| value.checked_add(exponent_bytes))
        .and_then(|value| value.checked_add(limb_bytes))
        .ok_or(SymbolicaAffineDenominatorError::ResourceCountOverflow {
            resource: "polynomial retained bytes",
        })?;
    Ok(CoefficientCensus {
        polynomial_terms,
        exponent_entries,
        integer_bits,
        retained_bytes,
    })
}

fn coefficient_census(
    coefficient: &Coefficient,
) -> Result<CoefficientCensus, SymbolicaAffineDenominatorError> {
    let mut census = polynomial_census(&coefficient.numerator)?;
    census.checked_add_assign(
        polynomial_census(&coefficient.denominator)?,
        "coefficient census",
    )?;
    Ok(census)
}

fn conservative_owned_capacity_bytes(
    payload_bytes: usize,
    resource: &'static str,
) -> Result<usize, SymbolicaAffineDenominatorError> {
    if payload_bytes == 0 {
        return Ok(0);
    }
    payload_bytes
        .checked_add(std::mem::size_of::<usize>())
        .and_then(|value| value.checked_mul(2))
        .ok_or(SymbolicaAffineDenominatorError::ResourceCountOverflow { resource })
}

fn retained_variable_map_arc_bytes<'a>(
    coefficients: impl IntoIterator<Item = &'a Coefficient>,
) -> Result<usize, SymbolicaAffineDenominatorError> {
    let mut distinct = BTreeSet::new();
    let mut bytes = 0usize;
    for coefficient in coefficients {
        for polynomial in [&coefficient.numerator, &coefficient.denominator] {
            let identity = Arc::as_ptr(&polynomial.variables) as usize;
            if distinct.insert(identity) {
                let arc_header = std::mem::size_of::<usize>().checked_mul(2).ok_or(
                    SymbolicaAffineDenominatorError::ResourceCountOverflow {
                        resource: "compiled retained variable-map Arc bytes",
                    },
                )?;
                let variable_payload = polynomial
                    .variables
                    .capacity()
                    .checked_mul(std::mem::size_of::<PolyVariable>())
                    .ok_or(SymbolicaAffineDenominatorError::ResourceCountOverflow {
                        resource: "compiled retained variable-map Arc bytes",
                    })?;
                let allocation = arc_header
                    .checked_add(std::mem::size_of::<Vec<PolyVariable>>())
                    .and_then(|value| value.checked_add(variable_payload))
                    .ok_or(SymbolicaAffineDenominatorError::ResourceCountOverflow {
                        resource: "compiled retained variable-map Arc bytes",
                    })?;
                bytes = bytes.checked_add(allocation).ok_or(
                    SymbolicaAffineDenominatorError::ResourceCountOverflow {
                        resource: "compiled retained variable-map Arc bytes",
                    },
                )?;
            }
        }
    }
    Ok(bytes)
}

fn compiled_retained_byte_bound(
    source_bytes: usize,
    normalized_expression_bytes: usize,
    projected_coefficient_bytes: usize,
    variable_map_arc_bytes: usize,
) -> Result<usize, SymbolicaAffineDenominatorError> {
    // The inline top-level structure owns both Atom handles and the affine row.
    // The additions below therefore charge only backing buffers and nested
    // coefficient allocations.
    let mut bytes = std::mem::size_of::<CompiledSymbolicaAffineDenominator>();
    let atom_payload = source_bytes
        .checked_add(normalized_expression_bytes)
        .ok_or(SymbolicaAffineDenominatorError::ResourceCountOverflow {
            resource: "compiled retained Atom bytes",
        })?;
    // Symbolica exposes an Atom's logical byte size but keeps RawAtom backing
    // capacity crate-private.  Charge a two-times growth policy plus one word;
    // this is intentionally a conservative retained-payload estimate, not an
    // exact observation of the private allocator capacity.
    bytes = bytes
        .checked_add(conservative_owned_capacity_bytes(
            atom_payload,
            "compiled retained Atom bytes",
        )?)
        .ok_or(SymbolicaAffineDenominatorError::ResourceCountOverflow {
            resource: "compiled retained bytes",
        })?;

    bytes = bytes
        .checked_add(conservative_owned_capacity_bytes(
            projected_coefficient_bytes,
            "compiled retained affine-coefficient bytes",
        )?)
        .ok_or(SymbolicaAffineDenominatorError::ResourceCountOverflow {
            resource: "compiled retained bytes",
        })?;
    bytes = bytes.checked_add(variable_map_arc_bytes).ok_or(
        SymbolicaAffineDenominatorError::ResourceCountOverflow {
            resource: "compiled retained variable-map Arc bytes",
        },
    )?;
    Ok(bytes)
}

fn multiply_census(
    census: CoefficientCensus,
    count: usize,
    resource: &'static str,
) -> Result<CoefficientCensus, SymbolicaAffineDenominatorError> {
    Ok(CoefficientCensus {
        polynomial_terms: census
            .polynomial_terms
            .checked_mul(count)
            .ok_or(SymbolicaAffineDenominatorError::ResourceCountOverflow { resource })?,
        exponent_entries: census
            .exponent_entries
            .checked_mul(count)
            .ok_or(SymbolicaAffineDenominatorError::ResourceCountOverflow { resource })?,
        integer_bits: census
            .integer_bits
            .checked_mul(count)
            .ok_or(SymbolicaAffineDenominatorError::ResourceCountOverflow { resource })?,
        retained_bytes: census
            .retained_bytes
            .checked_mul(count)
            .ok_or(SymbolicaAffineDenominatorError::ResourceCountOverflow { resource })?,
    })
}

fn planned_polynomial_clone_census(
    polynomial: &CoefficientPolynomial,
    variables: usize,
) -> Result<CoefficientCensus, SymbolicaAffineDenominatorError> {
    let terms = polynomial.nterms();
    let exponent_entries = terms.checked_mul(variables).ok_or(
        SymbolicaAffineDenominatorError::ResourceCountOverflow {
            resource: "planned polynomial clone exponent entries",
        },
    )?;
    let integer_bits = polynomial
        .coefficients
        .iter()
        .try_fold(0usize, |total, integer| {
            total.checked_add(integer_magnitude_bits(integer)?).ok_or(
                SymbolicaAffineDenominatorError::ResourceCountOverflow {
                    resource: "planned polynomial clone integer bits",
                },
            )
        })?;
    let limb_bytes = polynomial
        .coefficients
        .iter()
        .try_fold(0usize, |total, integer| {
            total.checked_add(integer_owned_heap_bytes(integer)?).ok_or(
                SymbolicaAffineDenominatorError::ResourceCountOverflow {
                    resource: "planned polynomial clone retained bytes",
                },
            )
        })?;
    let retained_bytes = std::mem::size_of::<CoefficientPolynomial>()
        .checked_add(terms.checked_mul(std::mem::size_of::<Integer>()).ok_or(
            SymbolicaAffineDenominatorError::ResourceCountOverflow {
                resource: "planned polynomial clone retained bytes",
            },
        )?)
        .and_then(|value| {
            exponent_entries
                .checked_mul(std::mem::size_of::<u16>())
                .and_then(|bytes| value.checked_add(bytes))
        })
        .and_then(|value| value.checked_add(limb_bytes))
        .ok_or(SymbolicaAffineDenominatorError::ResourceCountOverflow {
            resource: "planned polynomial clone retained bytes",
        })?;
    Ok(CoefficientCensus {
        polynomial_terms: terms,
        exponent_entries,
        integer_bits,
        retained_bytes,
    })
}

fn planned_coefficient_clone_census(
    coefficient: &Coefficient,
    variables: usize,
) -> Result<CoefficientCensus, SymbolicaAffineDenominatorError> {
    let mut census = planned_polynomial_clone_census(&coefficient.numerator, variables)?;
    census.checked_add_assign(
        planned_polynomial_clone_census(&coefficient.denominator, variables)?,
        "planned coefficient clone census",
    )?;
    Ok(census)
}

fn planned_unit_coefficient_census(
    variables: usize,
) -> Result<CoefficientCensus, SymbolicaAffineDenominatorError> {
    let mut census = planned_operation_polynomial_census(1, variables, 1)?;
    census.checked_add_assign(
        planned_operation_polynomial_census(1, variables, 1)?,
        "planned unit coefficient census",
    )?;
    Ok(census)
}

fn polynomial_max_integer_bits(
    polynomial: &CoefficientPolynomial,
) -> Result<usize, SymbolicaAffineDenominatorError> {
    polynomial
        .coefficients
        .iter()
        .try_fold(0usize, |maximum, integer| {
            Ok(maximum.max(integer_magnitude_bits(integer)?))
        })
}

fn ceil_log2_usize(value: usize) -> usize {
    if value <= 1 {
        0
    } else {
        usize::BITS as usize - (value - 1).leading_zeros() as usize
    }
}

fn product_coefficient_bit_bound(
    left_bits: usize,
    right_bits: usize,
    left_terms: usize,
    right_terms: usize,
) -> Result<usize, SymbolicaAffineDenominatorError> {
    if left_terms == 0 || right_terms == 0 {
        return Ok(0);
    }
    left_bits
        .checked_add(right_bits)
        .and_then(|value| value.checked_add(ceil_log2_usize(left_terms.min(right_terms))))
        .ok_or(SymbolicaAffineDenominatorError::ResourceCountOverflow {
            resource: "projected exact-operation integer-bit envelope",
        })
}

fn planned_operation_polynomial_census(
    terms: usize,
    variables: usize,
    maximum_integer_bits: usize,
) -> Result<CoefficientCensus, SymbolicaAffineDenominatorError> {
    let exponent_entries = terms.checked_mul(variables).ok_or(
        SymbolicaAffineDenominatorError::ResourceCountOverflow {
            resource: "projected exact-operation exponent entries",
        },
    )?;
    let integer_bits = terms.checked_mul(maximum_integer_bits).ok_or(
        SymbolicaAffineDenominatorError::ResourceCountOverflow {
            resource: "projected exact-operation integer bits",
        },
    )?;
    let rounded_bits = maximum_integer_bits
        .checked_add(usize::BITS as usize - 1)
        .map(|value| value / usize::BITS as usize * usize::BITS as usize)
        .ok_or(SymbolicaAffineDenominatorError::ResourceCountOverflow {
            resource: "projected exact-operation retained bytes",
        })?;
    // GMP operations may retain spare limbs.  Charge one extra limb and a 2x
    // allocator-growth envelope before entering the operation; the exact
    // post-operation census still uses `Integer::Large::capacity()`.
    let conservative_capacity_bits = rounded_bits
        .checked_add(usize::BITS as usize)
        .and_then(|value| value.checked_mul(CONSERVATIVE_GMP_CAPACITY_FACTOR))
        .ok_or(SymbolicaAffineDenominatorError::ResourceCountOverflow {
            resource: "projected exact-operation retained bytes",
        })?;
    let heap_bytes = terms.checked_mul(conservative_capacity_bits / 8).ok_or(
        SymbolicaAffineDenominatorError::ResourceCountOverflow {
            resource: "projected exact-operation retained bytes",
        },
    )?;
    let integer_slots = terms.checked_mul(std::mem::size_of::<Integer>()).ok_or(
        SymbolicaAffineDenominatorError::ResourceCountOverflow {
            resource: "projected exact-operation retained bytes",
        },
    )?;
    let exponent_payload = exponent_entries
        .checked_mul(std::mem::size_of::<u16>())
        .ok_or(SymbolicaAffineDenominatorError::ResourceCountOverflow {
            resource: "projected exact-operation retained bytes",
        })?;
    let integer_capacity = conservative_owned_capacity_bytes(
        integer_slots,
        "projected exact-operation retained bytes",
    )?;
    let exponent_capacity = conservative_owned_capacity_bytes(
        exponent_payload,
        "projected exact-operation retained bytes",
    )?;
    let retained_bytes = std::mem::size_of::<CoefficientPolynomial>()
        .checked_add(integer_capacity)
        .and_then(|value| value.checked_add(exponent_capacity))
        .and_then(|value| value.checked_add(heap_bytes))
        .ok_or(SymbolicaAffineDenominatorError::ResourceCountOverflow {
            resource: "projected exact-operation retained bytes",
        })?;
    Ok(CoefficientCensus {
        polynomial_terms: terms,
        exponent_entries,
        integer_bits,
        retained_bytes,
    })
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct ExactOperationAllocationEnvelope {
    census: CoefficientCensus,
    numerator_terms: usize,
    denominator_terms: usize,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct RawOperationPolynomialEnvelope {
    support_terms: usize,
    maximum_integer_bits: usize,
}

fn verify_operation_result_envelope(
    result: &Coefficient,
    actual: CoefficientCensus,
    planned: ExactOperationAllocationEnvelope,
) -> Result<(), SymbolicaAffineDenominatorError> {
    if result.numerator.nterms() > planned.numerator_terms
        || result.denominator.nterms() > planned.denominator_terms
        || actual.polynomial_terms > planned.census.polynomial_terms
        || actual.exponent_entries > planned.census.exponent_entries
        || actual.integer_bits > planned.census.integer_bits
        || actual.retained_bytes > planned.census.retained_bytes
    {
        return Err(
            SymbolicaAffineDenominatorError::InternalVerificationFailure {
                detail: "native exact-operation result exceeded its pre-operation envelope",
            },
        );
    }
    Ok(())
}

fn factor_coefficient_bit_bound(
    raw: RawOperationPolynomialEnvelope,
    dense_degree_box_terms: usize,
) -> Result<usize, SymbolicaAffineDenominatorError> {
    if raw.support_terms == 0 {
        return Ok(0);
    }
    // Apply Landau--Mignotte after the injective mixed-radix Kronecker
    // substitution induced by the componentwise degree box.  The substituted
    // polynomial has degree at most `dense_degree_box_terms - 1`, unchanged
    // coefficients, and at most `raw.support_terms` nonzero coefficients.
    // Every integral GCD quotient retained by Symbolica is an integral factor
    // of that raw polynomial, so this bounds its coefficient height before the
    // native normalization is entered.
    raw.maximum_integer_bits
        .checked_add(dense_degree_box_terms.saturating_sub(1))
        .and_then(|value| value.checked_add(ceil_log2_usize(raw.support_terms.max(1))))
        .and_then(|value| value.checked_add(2))
        .ok_or(SymbolicaAffineDenominatorError::ResourceCountOverflow {
            resource: "normalized exact-operation factor integer-bit envelope",
        })
}

fn normalization_may_divide(
    left: &Coefficient,
    right: &Coefficient,
    operation: BinaryOperation,
) -> bool {
    match operation {
        // If either denominator is one, denominator-GCD reduction and the
        // final numerator/denominator reduction in Symbolica are both units.
        BinaryOperation::Add => !left.denominator.is_one() && !right.denominator.is_one(),
        // Symbolica cross-cancels numerator/denominator pairs before the
        // product.  These sufficient unit tests prove that both GCDs are one.
        BinaryOperation::Multiply => {
            !((left.numerator.is_one() || right.denominator.is_one())
                && (left.denominator.is_one() || right.numerator.is_one()))
        }
        // Division is multiplication by the inverse.  These are the analogous
        // sufficient unit tests for its two cross-cancellation GCDs.
        BinaryOperation::Divide => {
            !((left.numerator.is_one() || right.numerator.is_one())
                && (left.denominator.is_one() || right.denominator.is_one()))
        }
    }
}

fn exact_operation_allocation_envelope(
    left: &Coefficient,
    right: &Coefficient,
    operation: BinaryOperation,
    variables: usize,
) -> Result<ExactOperationAllocationEnvelope, SymbolicaAffineDenominatorError> {
    let ln = left.numerator.nterms();
    let ld = left.denominator.nterms();
    let rn = right.numerator.nterms();
    let rd = right.denominator.nterms();
    let lnb = polynomial_max_integer_bits(&left.numerator)?;
    let ldb = polynomial_max_integer_bits(&left.denominator)?;
    let rnb = polynomial_max_integer_bits(&right.numerator)?;
    let rdb = polynomial_max_integer_bits(&right.denominator)?;
    let product_terms =
        |left: usize, right: usize| -> Result<usize, SymbolicaAffineDenominatorError> {
            left.checked_mul(right)
                .ok_or(SymbolicaAffineDenominatorError::ResourceCountOverflow {
                    resource: "projected exact-operation term envelope",
                })
        };
    let (numerator_terms, numerator_bits, denominator_terms, denominator_bits) = match operation {
        BinaryOperation::Add if left.denominator == right.denominator => (
            ln.checked_add(rn)
                .ok_or(SymbolicaAffineDenominatorError::ResourceCountOverflow {
                    resource: "projected exact-operation term envelope",
                })?,
            lnb.max(rnb).checked_add(1).ok_or(
                SymbolicaAffineDenominatorError::ResourceCountOverflow {
                    resource: "projected exact-operation integer-bit envelope",
                },
            )?,
            ld,
            ldb,
        ),
        BinaryOperation::Add => {
            let left_cross_terms = product_terms(ln, rd)?;
            let right_cross_terms = product_terms(rn, ld)?;
            let left_cross_bits = product_coefficient_bit_bound(lnb, rdb, ln, rd)?;
            let right_cross_bits = product_coefficient_bit_bound(rnb, ldb, rn, ld)?;
            (
                left_cross_terms.checked_add(right_cross_terms).ok_or(
                    SymbolicaAffineDenominatorError::ResourceCountOverflow {
                        resource: "projected exact-operation term envelope",
                    },
                )?,
                left_cross_bits.max(right_cross_bits).checked_add(1).ok_or(
                    SymbolicaAffineDenominatorError::ResourceCountOverflow {
                        resource: "projected exact-operation integer-bit envelope",
                    },
                )?,
                product_terms(ld, rd)?,
                product_coefficient_bit_bound(ldb, rdb, ld, rd)?,
            )
        }
        BinaryOperation::Multiply => (
            product_terms(ln, rn)?,
            product_coefficient_bit_bound(lnb, rnb, ln, rn)?,
            product_terms(ld, rd)?,
            product_coefficient_bit_bound(ldb, rdb, ld, rd)?,
        ),
        BinaryOperation::Divide => (
            product_terms(ln, rd)?,
            product_coefficient_bit_bound(lnb, rdb, ln, rd)?,
            product_terms(ld, rn)?,
            product_coefficient_bit_bound(ldb, rnb, ld, rn)?,
        ),
    };
    let (numerator_box, denominator_box) =
        operation_dense_degree_boxes(left, right, operation, variables)?;
    let normalize = normalization_may_divide(left, right, operation);
    let numerator_raw = RawOperationPolynomialEnvelope {
        support_terms: numerator_terms.min(numerator_box),
        maximum_integer_bits: numerator_bits,
    };
    let denominator_raw = RawOperationPolynomialEnvelope {
        support_terms: denominator_terms.min(denominator_box),
        maximum_integer_bits: denominator_bits,
    };
    let (planned_numerator_terms, planned_numerator_bits) = if normalize {
        (
            if numerator_raw.support_terms == 0 {
                0
            } else {
                numerator_box
            },
            factor_coefficient_bit_bound(numerator_raw, numerator_box)?,
        )
    } else {
        (
            numerator_raw.support_terms,
            numerator_raw.maximum_integer_bits,
        )
    };
    let (planned_denominator_terms, planned_denominator_bits) = if normalize {
        (
            if denominator_raw.support_terms == 0 {
                0
            } else {
                denominator_box
            },
            factor_coefficient_bit_bound(denominator_raw, denominator_box)?,
        )
    } else {
        (
            denominator_raw.support_terms,
            denominator_raw.maximum_integer_bits,
        )
    };
    // The native operation may retain the raw cross-products while it builds
    // normalized GCD quotients, so charge both phases rather than only the
    // larger-looking one.  This is a conservative logical-result/retained
    // allocation envelope; Symbolica's internal multivariate-GCD workspace is
    // not exposed by its API and is governed separately by the dense-box work
    // limits above.
    let mut census = planned_operation_polynomial_census(
        numerator_raw.support_terms,
        variables,
        numerator_raw.maximum_integer_bits,
    )?;
    census.checked_add_assign(
        planned_operation_polynomial_census(
            denominator_raw.support_terms,
            variables,
            denominator_raw.maximum_integer_bits,
        )?,
        "raw exact-operation allocation envelope",
    )?;
    if normalize {
        census.checked_add_assign(
            planned_operation_polynomial_census(
                planned_numerator_terms,
                variables,
                planned_numerator_bits,
            )?,
            "normalized exact-operation allocation envelope",
        )?;
        census.checked_add_assign(
            planned_operation_polynomial_census(
                planned_denominator_terms,
                variables,
                planned_denominator_bits,
            )?,
            "normalized exact-operation allocation envelope",
        )?;
    }
    Ok(ExactOperationAllocationEnvelope {
        census,
        numerator_terms: planned_numerator_terms,
        denominator_terms: planned_denominator_terms,
    })
}

fn polynomial_expression_census(
    polynomial: &CoefficientPolynomial,
) -> Result<NormalizedExpressionCensus, SymbolicaAffineDenominatorError> {
    if polynomial.is_zero() {
        return Ok(NormalizedExpressionCensus {
            nodes: 1,
            integer_bits: 0,
        });
    }
    let mut census = NormalizedExpressionCensus::default();
    for (integer, exponents) in polynomial
        .coefficients
        .iter()
        .zip(polynomial.exponents_iter())
    {
        let mut term_nodes = 1usize; // retained integer coefficient
        let mut factors = 1usize;
        census.integer_bits = census
            .integer_bits
            .checked_add(integer_magnitude_bits(integer)?)
            .ok_or(SymbolicaAffineDenominatorError::ResourceCountOverflow {
                resource: "normalized expression integer bits",
            })?;
        for exponent in exponents.iter().copied().filter(|exponent| *exponent != 0) {
            factors = factors.checked_add(1).ok_or(
                SymbolicaAffineDenominatorError::ResourceCountOverflow {
                    resource: "normalized expression nodes",
                },
            )?;
            if exponent == 1 {
                term_nodes = term_nodes.checked_add(1).ok_or(
                    SymbolicaAffineDenominatorError::ResourceCountOverflow {
                        resource: "normalized expression nodes",
                    },
                )?;
            } else {
                // Power node, variable, and exact integer exponent.
                term_nodes = term_nodes.checked_add(3).ok_or(
                    SymbolicaAffineDenominatorError::ResourceCountOverflow {
                        resource: "normalized expression nodes",
                    },
                )?;
                census.integer_bits = census
                    .integer_bits
                    .checked_add(
                        usize::try_from(u16::BITS - exponent.leading_zeros()).map_err(|_| {
                            SymbolicaAffineDenominatorError::ResourceCountOverflow {
                                resource: "normalized expression integer bits",
                            }
                        })?,
                    )
                    .ok_or(SymbolicaAffineDenominatorError::ResourceCountOverflow {
                        resource: "normalized expression integer bits",
                    })?;
            }
        }
        if factors > 1 {
            term_nodes = term_nodes.checked_add(1).ok_or(
                SymbolicaAffineDenominatorError::ResourceCountOverflow {
                    resource: "normalized expression nodes",
                },
            )?;
        }
        census.nodes = census.nodes.checked_add(term_nodes).ok_or(
            SymbolicaAffineDenominatorError::ResourceCountOverflow {
                resource: "normalized expression nodes",
            },
        )?;
    }
    if polynomial.nterms() > 1 {
        census.nodes = census.nodes.checked_add(1).ok_or(
            SymbolicaAffineDenominatorError::ResourceCountOverflow {
                resource: "normalized expression nodes",
            },
        )?;
    }
    Ok(census)
}

fn normalized_expression_census(
    coefficient: &Coefficient,
) -> Result<NormalizedExpressionCensus, SymbolicaAffineDenominatorError> {
    let mut census = polynomial_expression_census(&coefficient.numerator)?;
    if !coefficient.denominator.is_one() {
        let denominator = polynomial_expression_census(&coefficient.denominator)?;
        census.nodes = census
            .nodes
            .checked_add(denominator.nodes)
            .and_then(|value| value.checked_add(3))
            .ok_or(SymbolicaAffineDenominatorError::ResourceCountOverflow {
                resource: "normalized expression nodes",
            })?;
        census.integer_bits = census
            .integer_bits
            .checked_add(denominator.integer_bits)
            .and_then(|value| value.checked_add(1))
            .ok_or(SymbolicaAffineDenominatorError::ResourceCountOverflow {
                resource: "normalized expression integer bits",
            })?;
    }
    Ok(census)
}

fn normalized_expression_render_byte_bound(
    census: NormalizedExpressionCensus,
    maximum_symbol_bytes: usize,
) -> Result<usize, SymbolicaAffineDenominatorError> {
    let bytes_per_node = maximum_symbol_bytes.checked_add(8).ok_or(
        SymbolicaAffineDenominatorError::ResourceCountOverflow {
            resource: "normalized expression render bytes",
        },
    )?;
    census
        .nodes
        .checked_mul(bytes_per_node)
        .and_then(|value| value.checked_add(census.integer_bits))
        .and_then(|value| value.checked_add(16))
        .ok_or(SymbolicaAffineDenominatorError::ResourceCountOverflow {
            resource: "normalized expression render bytes",
        })
}

fn polynomial_degrees(
    polynomial: &CoefficientPolynomial,
    expected_variables: usize,
) -> Result<Vec<u16>, SymbolicaAffineDenominatorError> {
    if polynomial.variables.len() != expected_variables {
        return Err(
            SymbolicaAffineDenominatorError::InternalVerificationFailure {
                detail: "degree census found a polynomial on the wrong variable map",
            },
        );
    }
    let mut degrees = Vec::new();
    degrees.try_reserve_exact(expected_variables).map_err(|_| {
        SymbolicaAffineDenominatorError::AllocationFailure {
            resource: "componentwise degree census",
            requested: expected_variables,
        }
    })?;
    for variable in 0..expected_variables {
        degrees.push(polynomial.degree(variable));
    }
    Ok(degrees)
}

fn operation_dense_degree_boxes(
    left: &Coefficient,
    right: &Coefficient,
    operation: BinaryOperation,
    variables: usize,
) -> Result<(usize, usize), SymbolicaAffineDenominatorError> {
    let ln = polynomial_degrees(&left.numerator, variables)?;
    let ld = polynomial_degrees(&left.denominator, variables)?;
    let rn = polynomial_degrees(&right.numerator, variables)?;
    let rd = polynomial_degrees(&right.denominator, variables)?;
    let same_denominator = left.denominator == right.denominator;
    let mut numerator_box = 1usize;
    let mut denominator_box = 1usize;
    for variable in 0..variables {
        let sum = |left: u16,
                   right: u16,
                   resource: &'static str|
         -> Result<u32, SymbolicaAffineDenominatorError> {
            u32::from(left)
                .checked_add(u32::from(right))
                .ok_or(SymbolicaAffineDenominatorError::ResourceCountOverflow { resource })
        };
        let (numerator_degree, denominator_degree) = match operation {
            BinaryOperation::Add if same_denominator => (
                u32::from(ln[variable].max(rn[variable])),
                u32::from(ld[variable]),
            ),
            BinaryOperation::Add => (
                sum(ln[variable], rd[variable], "addition numerator degree")?.max(sum(
                    rn[variable],
                    ld[variable],
                    "addition numerator degree",
                )?),
                sum(ld[variable], rd[variable], "addition denominator degree")?,
            ),
            BinaryOperation::Multiply => (
                sum(
                    ln[variable],
                    rn[variable],
                    "multiplication numerator degree",
                )?,
                sum(
                    ld[variable],
                    rd[variable],
                    "multiplication denominator degree",
                )?,
            ),
            BinaryOperation::Divide => (
                sum(ln[variable], rd[variable], "division numerator degree")?,
                sum(ld[variable], rn[variable], "division denominator degree")?,
            ),
        };
        let numerator_width = usize::try_from(numerator_degree.checked_add(1).ok_or(
            SymbolicaAffineDenominatorError::ResourceCountOverflow {
                resource: "dense numerator degree box",
            },
        )?)
        .map_err(|_| SymbolicaAffineDenominatorError::ResourceCountOverflow {
            resource: "dense numerator degree box",
        })?;
        numerator_box = numerator_box.checked_mul(numerator_width).ok_or(
            SymbolicaAffineDenominatorError::ResourceCountOverflow {
                resource: "dense numerator degree box",
            },
        )?;
        let denominator_width = usize::try_from(denominator_degree.checked_add(1).ok_or(
            SymbolicaAffineDenominatorError::ResourceCountOverflow {
                resource: "dense denominator degree box",
            },
        )?)
        .map_err(|_| SymbolicaAffineDenominatorError::ResourceCountOverflow {
            resource: "dense denominator degree box",
        })?;
        denominator_box = denominator_box.checked_mul(denominator_width).ok_or(
            SymbolicaAffineDenominatorError::ResourceCountOverflow {
                resource: "dense denominator degree box",
            },
        )?;
    }
    Ok((numerator_box, denominator_box))
}

fn charge_dense_degree_box(
    left: &Coefficient,
    right: &Coefficient,
    operation: BinaryOperation,
    variables: usize,
    limits: SymbolicaAffineDenominatorLimits,
    work: &mut ExactWorkBudget,
) -> Result<(usize, usize), SymbolicaAffineDenominatorError> {
    let (numerator_box, denominator_box) =
        operation_dense_degree_boxes(left, right, operation, variables)?;
    check_limit(
        "dense numerator degree-box terms",
        numerator_box,
        limits.max_dense_degree_box_terms,
    )?;
    check_limit(
        "dense denominator degree-box terms",
        denominator_box,
        limits.max_dense_degree_box_terms,
    )?;
    let terms = numerator_box.checked_add(denominator_box).ok_or(
        SymbolicaAffineDenominatorError::ResourceCountOverflow {
            resource: "dense degree-box terms",
        },
    )?;
    let entries = terms.checked_mul(variables).ok_or(
        SymbolicaAffineDenominatorError::ResourceCountOverflow {
            resource: "dense degree-box exponent entries",
        },
    )?;
    check_limit(
        "dense degree-box exponent entries",
        entries,
        limits.max_dense_degree_box_exponent_entries,
    )?;
    work.dense_degree_box_terms = work.dense_degree_box_terms.checked_add(terms).ok_or(
        SymbolicaAffineDenominatorError::ResourceCountOverflow {
            resource: "aggregate dense degree-box terms",
        },
    )?;
    check_limit(
        "aggregate dense degree-box terms",
        work.dense_degree_box_terms,
        limits.max_aggregate_dense_degree_box_terms,
    )?;
    work.dense_degree_box_exponent_entries = work
        .dense_degree_box_exponent_entries
        .checked_add(entries)
        .ok_or(SymbolicaAffineDenominatorError::ResourceCountOverflow {
            resource: "aggregate dense degree-box exponent entries",
        })?;
    check_limit(
        "aggregate dense degree-box exponent entries",
        work.dense_degree_box_exponent_entries,
        limits.max_aggregate_dense_degree_box_exponent_entries,
    )?;
    Ok((numerator_box, denominator_box))
}

struct CheckedEvaluator<'a> {
    compiler: &'a SymbolicaAffineDenominatorCompiler,
    arithmetic_operations: u64,
    work: ExactWorkBudget,
    projection_work: ProjectionAllocationBudget,
}

impl<'a> CheckedEvaluator<'a> {
    fn new(compiler: &'a SymbolicaAffineDenominatorCompiler) -> Self {
        Self {
            compiler,
            arithmetic_operations: 0,
            work: ExactWorkBudget::default(),
            projection_work: ProjectionAllocationBudget::default(),
        }
    }

    fn evaluate(
        &mut self,
        atom: AtomView<'_>,
        scalar_product_allowed: bool,
    ) -> Result<Coefficient, SymbolicaAffineDenominatorError> {
        match atom {
            AtomView::Num(_) => self.compiler.numeric_coefficient(atom),
            AtomView::Var(variable) => {
                let symbol = variable.get_symbol();
                let position = self
                    .compiler
                    .symbol_positions
                    .get(&symbol)
                    .copied()
                    .ok_or_else(|| {
                        SymbolicaAffineDenominatorError::UnknownSymbol(atom.to_owned())
                    })?;
                Ok(self.compiler.combined.parameter_at(position))
            }
            AtomView::Add(sum) => {
                let mut result = self.compiler.combined.zero();
                for child in sum.iter() {
                    let child = self.evaluate(child, scalar_product_allowed)?;
                    self.charge_arithmetic()?;
                    result = self.compiler.checked_add(&result, &child, &mut self.work)?;
                }
                Ok(result)
            }
            AtomView::Mul(product) => {
                let mut result = self.compiler.combined.one();
                for child in product.iter() {
                    let child = self.evaluate(child, scalar_product_allowed)?;
                    self.charge_arithmetic()?;
                    result = self.compiler.checked_mul(&result, &child, &mut self.work)?;
                }
                Ok(result)
            }
            AtomView::Pow(power) => {
                let exponent = i64::try_from(power.get_exp()).map_err(|_| {
                    SymbolicaAffineDenominatorError::UnsupportedPower(atom.to_owned())
                })?;
                let absolute = exponent.unsigned_abs();
                if absolute > u64::from(self.compiler.limits.max_abs_power) {
                    return Err(SymbolicaAffineDenominatorError::UnsupportedPower(
                        atom.to_owned(),
                    ));
                }
                let base = self.evaluate(power.get_base(), scalar_product_allowed)?;
                if exponent < 0 && coefficient_contains_momentum(&base, self.compiler.base_count())?
                {
                    return Err(SymbolicaAffineDenominatorError::NegativeMomentumPower {
                        atom: power.get_base().to_owned(),
                        exponent,
                    });
                }
                self.checked_power(&base, exponent)
            }
            AtomView::Fun(function) => {
                if function.get_symbol() != self.compiler.scalar_product {
                    return Err(SymbolicaAffineDenominatorError::UnsupportedFunction(
                        atom.to_owned(),
                    ));
                }
                if !scalar_product_allowed {
                    return Err(SymbolicaAffineDenominatorError::NestedScalarProduct(
                        atom.to_owned(),
                    ));
                }
                if function.get_nargs() != 2 {
                    return Err(SymbolicaAffineDenominatorError::MalformedScalarProduct {
                        atom: atom.to_owned(),
                        arguments: function.get_nargs(),
                    });
                }
                let mut arguments = function.iter();
                let left_atom = arguments.next().ok_or(
                    SymbolicaAffineDenominatorError::InternalVerificationFailure {
                        detail: "binary scalar product has no first argument",
                    },
                )?;
                let right_atom = arguments.next().ok_or(
                    SymbolicaAffineDenominatorError::InternalVerificationFailure {
                        detail: "binary scalar product has no second argument",
                    },
                )?;
                if arguments.next().is_some() {
                    return Err(
                        SymbolicaAffineDenominatorError::InternalVerificationFailure {
                            detail: "binary scalar product retained an extra argument",
                        },
                    );
                }
                let left = self.evaluate(left_atom, false)?;
                let right = self.evaluate(right_atom, false)?;
                self.compiler.validate_vector_linear(&left, 0, left_atom)?;
                self.compiler
                    .validate_vector_linear(&right, 1, right_atom)?;
                self.charge_arithmetic()?;
                let product = self.compiler.checked_mul(&left, &right, &mut self.work)?;
                self.compiler.contract_explicit_scalar_product(
                    product,
                    &mut self.work,
                    &mut self.projection_work,
                )
            }
        }
    }

    fn checked_power(
        &mut self,
        base: &Coefficient,
        exponent: i64,
    ) -> Result<Coefficient, SymbolicaAffineDenominatorError> {
        let variables = self.compiler.combined.parameter_names().len();
        let unit_census = planned_unit_coefficient_census(variables)?;
        check_limit(
            "combined power-result integer bits",
            unit_census.integer_bits,
            self.compiler.limits.max_coefficient_integer_bits,
        )?;
        check_limit(
            "combined power-result retained bytes",
            unit_census.retained_bytes,
            self.compiler.limits.max_combined_retained_bytes,
        )?;
        if exponent == 0 {
            return Ok(self.compiler.combined.one());
        }

        self.compiler.combined.preflight_power_with_limits(
            base,
            exponent.unsigned_abs(),
            self.compiler.limits.exact_algebra,
        )?;

        let mut clone_census = unit_census;
        clone_census.checked_add_assign(
            planned_coefficient_clone_census(base, variables)?,
            "combined power base-clone census",
        )?;
        check_limit(
            "combined power base-clone integer bits",
            clone_census.integer_bits,
            self.compiler.limits.max_coefficient_integer_bits,
        )?;
        check_limit(
            "combined power base-clone retained bytes",
            clone_census.retained_bytes,
            self.compiler.limits.max_combined_retained_bytes,
        )?;
        let mut remaining = exponent.unsigned_abs();
        let mut result = self.compiler.combined.one();
        let mut factor = base.clone();
        while remaining != 0 {
            if remaining & 1 == 1 {
                self.charge_arithmetic()?;
                result = self
                    .compiler
                    .checked_mul(&result, &factor, &mut self.work)?;
            }
            remaining >>= 1;
            if remaining != 0 {
                self.charge_arithmetic()?;
                factor = self
                    .compiler
                    .checked_mul(&factor, &factor, &mut self.work)?;
            }
        }
        if exponent < 0 {
            self.charge_arithmetic()?;
            self.compiler
                .checked_div(&self.compiler.combined.one(), &result, &mut self.work)
        } else {
            Ok(result)
        }
    }

    fn charge_arithmetic(&mut self) -> Result<(), SymbolicaAffineDenominatorError> {
        self.arithmetic_operations = self.arithmetic_operations.checked_add(1).ok_or(
            SymbolicaAffineDenominatorError::ResourceCountOverflow {
                resource: "expression arithmetic operations",
            },
        )?;
        if self.arithmetic_operations > self.compiler.limits.max_arithmetic_operations {
            return Err(SymbolicaAffineDenominatorError::ResourceLimit {
                resource: "expression arithmetic operations",
                requested: u128::from(self.arithmetic_operations),
                limit: u128::from(self.compiler.limits.max_arithmetic_operations),
            });
        }
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
enum ProjectionGroup {
    Constant,
    Coordinate(usize),
    ExternalPair(usize, usize),
}

#[allow(clippy::too_many_arguments)]
fn classify_numerator_term(
    exponents: &[u16],
    expected_variables: usize,
    base_count: usize,
    loops: usize,
    externals: usize,
    loop_loop_count: usize,
    numerator_term: usize,
) -> Result<ProjectionGroup, SymbolicaAffineDenominatorError> {
    if exponents.len() != expected_variables {
        return Err(
            SymbolicaAffineDenominatorError::InternalVerificationFailure {
                detail: "combined numerator exponent row has the wrong length",
            },
        );
    }
    match momentum_degree(exponents, base_count)? {
        0 => Ok(ProjectionGroup::Constant),
        1 => Err(SymbolicaAffineDenominatorError::MomentumDegreeOne { numerator_term }),
        2 => classify_quadratic_group(
            &exponents[base_count..],
            loops,
            externals,
            loop_loop_count,
            numerator_term,
        ),
        degree => Err(SymbolicaAffineDenominatorError::MomentumDegreeTooHigh {
            numerator_term,
            degree,
        }),
    }
}

fn classify_quadratic_group(
    momentum_exponents: &[u16],
    loops: usize,
    externals: usize,
    loop_loop_count: usize,
    numerator_term: usize,
) -> Result<ProjectionGroup, SymbolicaAffineDenominatorError> {
    let mut first = None;
    let mut second = None;
    for (position, &exponent) in momentum_exponents.iter().enumerate() {
        if exponent == 0 {
            continue;
        }
        if first.is_none() {
            first = Some((position, exponent));
        } else if second.is_none() {
            second = Some((position, exponent));
        } else {
            return Err(
                SymbolicaAffineDenominatorError::InvalidQuadraticMomentumMonomial {
                    numerator_term,
                },
            );
        }
    }
    let (left, right) = match (first, second) {
        (Some((position, 2)), None) => (position, position),
        (Some((left, 1)), Some((right, 1))) => (left, right),
        _ => {
            return Err(
                SymbolicaAffineDenominatorError::InvalidQuadraticMomentumMonomial {
                    numerator_term,
                },
            );
        }
    };
    let momentum_count = loops.checked_add(externals).ok_or(
        SymbolicaAffineDenominatorError::ResourceCountOverflow {
            resource: "quadratic momentum positions",
        },
    )?;
    if right >= momentum_count {
        return Err(
            SymbolicaAffineDenominatorError::InternalVerificationFailure {
                detail: "quadratic momentum position exceeds the combined map",
            },
        );
    }
    match (left < loops, right < loops) {
        (true, true) => Ok(ProjectionGroup::Coordinate(upper_triangular_index(
            left, right, loops,
        )?)),
        (true, false) => {
            let external = right - loops;
            let offset = left.checked_mul(externals).and_then(|value| {
                loop_loop_count
                    .checked_add(value)
                    .and_then(|value| value.checked_add(external))
            });
            Ok(ProjectionGroup::Coordinate(offset.ok_or(
                SymbolicaAffineDenominatorError::ResourceCountOverflow {
                    resource: "loop-external coordinate index",
                },
            )?))
        }
        (false, true) => Err(
            SymbolicaAffineDenominatorError::InternalVerificationFailure {
                detail: "canonical momentum exponents reversed a loop-external pair",
            },
        ),
        (false, false) => Ok(ProjectionGroup::ExternalPair(left - loops, right - loops)),
    }
}

fn project_polynomial_prefix(
    source: &CoefficientPolynomial,
    target_template: &CoefficientPolynomial,
    retained: usize,
    max_exponent_entries: usize,
) -> Result<CoefficientPolynomial, SymbolicaAffineDenominatorError> {
    if target_template.variables.len() != retained {
        return Err(
            SymbolicaAffineDenominatorError::InternalVerificationFailure {
                detail: "base projection target has the wrong variable count",
            },
        );
    }
    let expected_source_entries = source.nterms().checked_mul(source.variables.len()).ok_or(
        SymbolicaAffineDenominatorError::ResourceCountOverflow {
            resource: "source projection exponent entries",
        },
    )?;
    if source.exponents.len() != expected_source_entries {
        return Err(
            SymbolicaAffineDenominatorError::InternalVerificationFailure {
                detail: "source projection polynomial has a malformed exponent layout",
            },
        );
    }
    let target_entries = source.nterms().checked_mul(retained).ok_or(
        SymbolicaAffineDenominatorError::ResourceCountOverflow {
            resource: "target projection exponent entries",
        },
    )?;
    check_limit(
        "target projection exponent entries",
        target_entries,
        max_exponent_entries,
    )?;
    let mut target = target_template.zero_with_capacity(source.nterms());
    for (integer, exponents) in source.coefficients.iter().zip(source.exponents_iter()) {
        if exponents.len() != source.variables.len() || exponents.len() < retained {
            return Err(
                SymbolicaAffineDenominatorError::InternalVerificationFailure {
                    detail: "combined polynomial exponent row is too short",
                },
            );
        }
        if exponents[retained..].iter().any(|exponent| *exponent != 0) {
            return Err(SymbolicaAffineDenominatorError::BaseCoefficientContainsMomentum);
        }
        target.append_monomial(integer.clone(), &exponents[..retained]);
    }
    Ok(target)
}

fn lift_polynomial_prefix(
    source: &CoefficientPolynomial,
    target_template: &CoefficientPolynomial,
    retained: usize,
    max_exponent_entries: usize,
) -> Result<CoefficientPolynomial, SymbolicaAffineDenominatorError> {
    if source.variables.len() != retained || target_template.variables.len() < retained {
        return Err(
            SymbolicaAffineDenominatorError::InternalVerificationFailure {
                detail: "base lift uses incompatible variable maps",
            },
        );
    }
    let target_variables = target_template.variables.len();
    let target_entries = source.nterms().checked_mul(target_variables).ok_or(
        SymbolicaAffineDenominatorError::ResourceCountOverflow {
            resource: "base lift exponent entries",
        },
    )?;
    check_limit(
        "base lift exponent entries",
        target_entries,
        max_exponent_entries,
    )?;
    let mut exponent_row = Vec::new();
    exponent_row
        .try_reserve_exact(target_variables)
        .map_err(|_| SymbolicaAffineDenominatorError::AllocationFailure {
            resource: "base lift exponent row",
            requested: target_variables,
        })?;
    exponent_row.resize(target_variables, 0u16);
    let mut target = target_template.zero_with_capacity(source.nterms());
    for (integer, exponents) in source.coefficients.iter().zip(source.exponents_iter()) {
        if exponents.len() != retained {
            return Err(
                SymbolicaAffineDenominatorError::InternalVerificationFailure {
                    detail: "base lift source exponent row has the wrong width",
                },
            );
        }
        exponent_row[..retained].copy_from_slice(exponents);
        target.append_monomial(integer.clone(), &exponent_row);
    }
    Ok(target)
}

fn reject_momentum_denominator(
    coefficient: &Coefficient,
    base_count: usize,
) -> Result<(), SymbolicaAffineDenominatorError> {
    if polynomial_contains_momentum(&coefficient.denominator, base_count)? {
        Err(SymbolicaAffineDenominatorError::MomentumDependentRationalDenominator)
    } else {
        Ok(())
    }
}

fn coefficient_contains_momentum(
    coefficient: &Coefficient,
    base_count: usize,
) -> Result<bool, SymbolicaAffineDenominatorError> {
    Ok(
        polynomial_contains_momentum(&coefficient.numerator, base_count)?
            || polynomial_contains_momentum(&coefficient.denominator, base_count)?,
    )
}

fn polynomial_contains_momentum(
    polynomial: &CoefficientPolynomial,
    base_count: usize,
) -> Result<bool, SymbolicaAffineDenominatorError> {
    if polynomial.variables.len() < base_count {
        return Err(
            SymbolicaAffineDenominatorError::InternalVerificationFailure {
                detail: "polynomial variable map is shorter than the base map",
            },
        );
    }
    Ok(polynomial.exponents_iter().any(|exponents| {
        exponents[base_count..]
            .iter()
            .any(|exponent| *exponent != 0)
    }))
}

fn momentum_degree(
    exponents: &[u16],
    base_count: usize,
) -> Result<u32, SymbolicaAffineDenominatorError> {
    let suffix = exponents.get(base_count..).ok_or(
        SymbolicaAffineDenominatorError::InternalVerificationFailure {
            detail: "polynomial exponent row is shorter than the base map",
        },
    )?;
    suffix.iter().try_fold(0u32, |degree, exponent| {
        degree.checked_add(u32::from(*exponent)).ok_or(
            SymbolicaAffineDenominatorError::ResourceCountOverflow {
                resource: "momentum degree",
            },
        )
    })
}

fn validate_external_gram(
    coefficients: &CoefficientContext,
    external_momenta: &[String],
    gram: &[Vec<Coefficient>],
    limits: SymbolicaAffineDenominatorLimits,
) -> Result<(), SymbolicaAffineDenominatorError> {
    let expected = external_momenta.len();
    let expected_entries = expected.checked_mul(expected).ok_or(
        SymbolicaAffineDenominatorError::ResourceCountOverflow {
            resource: "external Gram entries",
        },
    )?;
    check_limit(
        "external Gram entries",
        expected_entries,
        limits.max_external_gram_entries,
    )?;
    if gram.len() != expected {
        return Err(SymbolicaAffineDenominatorError::WrongExternalGramRowCount {
            expected,
            actual: gram.len(),
        });
    }
    for (row, entries) in gram.iter().enumerate() {
        if entries.len() != expected {
            return Err(
                SymbolicaAffineDenominatorError::WrongExternalGramColumnCount {
                    row,
                    expected,
                    actual: entries.len(),
                },
            );
        }
    }
    let mut polynomial_terms = 0usize;
    let mut exponent_entries = 0usize;
    let mut integer_bits = 0usize;
    for (row, entries) in gram.iter().enumerate() {
        for (column, coefficient) in entries.iter().enumerate() {
            coefficients
                .validate_with_limits(coefficient, limits.exact_algebra)
                .map_err(|error| {
                    SymbolicaAffineDenominatorError::InvalidExternalGramCoefficient {
                        row,
                        column,
                        error,
                    }
                })?;
            let coefficient_terms = coefficient
                .numerator
                .nterms()
                .checked_add(coefficient.denominator.nterms())
                .ok_or(SymbolicaAffineDenominatorError::ResourceCountOverflow {
                    resource: "external Gram polynomial terms",
                })?;
            polynomial_terms = polynomial_terms.checked_add(coefficient_terms).ok_or(
                SymbolicaAffineDenominatorError::ResourceCountOverflow {
                    resource: "external Gram polynomial terms",
                },
            )?;
            check_limit(
                "external Gram polynomial terms",
                polynomial_terms,
                limits.max_external_gram_polynomial_terms,
            )?;
            let coefficient_exponents = coefficient_terms
                .checked_mul(coefficients.parameter_names().len())
                .ok_or(SymbolicaAffineDenominatorError::ResourceCountOverflow {
                    resource: "external Gram exponent entries",
                })?;
            exponent_entries = exponent_entries.checked_add(coefficient_exponents).ok_or(
                SymbolicaAffineDenominatorError::ResourceCountOverflow {
                    resource: "external Gram exponent entries",
                },
            )?;
            check_limit(
                "external Gram exponent entries",
                exponent_entries,
                limits.max_external_gram_exponent_entries,
            )?;
            integer_bits = integer_bits
                .checked_add(coefficient_census(coefficient)?.integer_bits)
                .ok_or(SymbolicaAffineDenominatorError::ResourceCountOverflow {
                    resource: "external Gram integer bits",
                })?;
            check_limit(
                "external Gram integer bits",
                integer_bits,
                limits.max_external_gram_integer_bits,
            )?;
            if gram[column][row] != *coefficient {
                return Err(SymbolicaAffineDenominatorError::AsymmetricExternalGram {
                    row,
                    column,
                });
            }
        }
    }
    Ok(())
}

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
    // the public node-limit gate.  Keep the traversal-stack label solely for
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

fn checked_atom_shape(
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

fn scalar_product_coordinate_count(
    loops: usize,
    externals: usize,
) -> Result<usize, SymbolicaAffineDenominatorError> {
    upper_triangular_count(loops)?
        .checked_add(loops.checked_mul(externals).ok_or(
            SymbolicaAffineDenominatorError::ResourceCountOverflow {
                resource: "loop-external scalar products",
            },
        )?)
        .ok_or(SymbolicaAffineDenominatorError::ResourceCountOverflow {
            resource: "scalar-product coordinates",
        })
}

fn upper_triangular_count(size: usize) -> Result<usize, SymbolicaAffineDenominatorError> {
    size.checked_add(1)
        .and_then(|next| size.checked_mul(next))
        .map(|product| product / 2)
        .ok_or(SymbolicaAffineDenominatorError::ResourceCountOverflow {
            resource: "upper-triangular scalar products",
        })
}

fn upper_triangular_index(
    left: usize,
    right: usize,
    size: usize,
) -> Result<usize, SymbolicaAffineDenominatorError> {
    if left > right || right >= size {
        return Err(
            SymbolicaAffineDenominatorError::InternalVerificationFailure {
                detail: "invalid upper-triangular scalar-product coordinate",
            },
        );
    }
    let preceding = left
        .checked_mul(size)
        .and_then(|value| {
            left.checked_mul(left.saturating_sub(1))
                .map(|triangle| value - triangle / 2)
        })
        .ok_or(SymbolicaAffineDenominatorError::ResourceCountOverflow {
            resource: "upper-triangular coordinate index",
        })?;
    preceding.checked_add(right - left).ok_or(
        SymbolicaAffineDenominatorError::ResourceCountOverflow {
            resource: "upper-triangular coordinate index",
        },
    )
}

fn scalar_product_coordinates(
    loops: usize,
    externals: usize,
    capacity: usize,
) -> Result<Vec<ScalarProductCoordinate>, SymbolicaAffineDenominatorError> {
    let mut coordinates = Vec::new();
    coordinates.try_reserve_exact(capacity).map_err(|_| {
        SymbolicaAffineDenominatorError::AllocationFailure {
            resource: "scalar-product coordinates",
            requested: capacity,
        }
    })?;
    for left in 0..loops {
        for right in left..loops {
            coordinates.push(ScalarProductCoordinate::LoopLoop { left, right });
        }
    }
    for loop_index in 0..loops {
        for external_index in 0..externals {
            coordinates.push(ScalarProductCoordinate::LoopExternal {
                loop_index,
                external_index,
            });
        }
    }
    if coordinates.len() != capacity {
        return Err(
            SymbolicaAffineDenominatorError::InternalVerificationFailure {
                detail: "scalar-product coordinate census disagrees with construction",
            },
        );
    }
    Ok(coordinates)
}

fn plain_symbol(name: &str) -> Result<Symbol, SymbolicaAffineDenominatorError> {
    let namespaced = NamespacedSymbol::try_parse(name).ok_or_else(|| {
        SymbolicaAffineDenominatorError::Parse(format!(
            "could not form reserved Symbolica symbol {name:?}"
        ))
    })?;
    SymbolBuilder::new(namespaced)
        .build()
        .map_err(|error| SymbolicaAffineDenominatorError::Parse(error.to_string()))
}

fn maximum_combined_symbol_bytes(
    coefficients: &CoefficientContext,
) -> Result<usize, SymbolicaAffineDenominatorError> {
    coefficients
        .variables()
        .iter()
        .enumerate()
        .try_fold(1usize, |maximum, (position, variable)| {
            let PolyVariable::Symbol(symbol) = variable else {
                return Err(
                    SymbolicaAffineDenominatorError::UnsupportedCombinedVariable {
                        position,
                        label: coefficients.parameter_names()[position].clone(),
                    },
                );
            };
            Ok(maximum.max(symbol.get_name().len()))
        })
}

fn authenticate_plain_symbol(
    symbol: Symbol,
    label: &str,
    expected_name: &str,
) -> Result<(), SymbolicaAffineDenominatorError> {
    let reject = |violation| SymbolicaAffineDenominatorError::ImpureDeclaredSymbol {
        label: label.to_owned(),
        violation,
    };
    if symbol.get_name() != expected_name {
        return Err(reject("canonical name differs from the declaration"));
    }
    if symbol.get_wildcard_level() != 0 {
        return Err(reject("wildcard level is not zero"));
    }
    if symbol.has_attributes() {
        return Err(reject("attributes or tags are present"));
    }
    if !symbol.is_exportable() {
        return Err(reject("a callback or custom function is registered"));
    }
    if !symbol.get_aliases().is_empty() {
        return Err(reject("aliases are registered"));
    }
    if !matches!(symbol.get_data(), UserData::None) {
        return Err(reject("custom user data is registered"));
    }
    Ok(())
}

fn check_limit(
    resource: &'static str,
    requested: usize,
    limit: usize,
) -> Result<(), SymbolicaAffineDenominatorError> {
    if requested > limit {
        Err(SymbolicaAffineDenominatorError::ResourceLimit {
            resource,
            requested: requested as u128,
            limit: limit as u128,
        })
    } else {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    impl SymbolicaAffineDenominatorCompiler {
        fn compile_expression(
            &self,
            expression: &str,
        ) -> Result<CompiledSymbolicaAffineDenominator, SymbolicaAffineDenominatorError> {
            let source = try_parse!(expression, default_namespace = RUSTRED_NAMESPACE)
                .map_err(SymbolicaAffineDenominatorError::Parse)?;
            self.compile(source.as_view())
        }

        fn parse_base_expression(
            &self,
            expression: &str,
        ) -> Result<Coefficient, SymbolicaAffineDenominatorError> {
            let source = try_parse!(expression, default_namespace = RUSTRED_NAMESPACE)
                .map_err(SymbolicaAffineDenominatorError::Parse)?;
            self.parse_base_coefficient(source.as_view())
        }

        fn test_with_limits(&self, limits: SymbolicaAffineDenominatorLimits) -> Self {
            Self {
                coefficients: self.coefficients.clone(),
                loop_momenta: self.loop_momenta.clone(),
                external_momenta: self.external_momenta.clone(),
                external_gram: self.external_gram.clone(),
                combined: self.combined.clone(),
                symbol_positions: self.symbol_positions.clone(),
                scalar_product: self.scalar_product,
                coordinates: self.coordinates.clone(),
                limits,
            }
        }

        fn test_clone(&self) -> Self {
            self.test_with_limits(self.limits)
        }

        const fn test_limits(&self) -> SymbolicaAffineDenominatorLimits {
            self.limits
        }

        const fn test_coefficient_context(&self) -> &CoefficientContext {
            &self.coefficients
        }
    }

    fn compiler(
        parameters: &[&str],
        loops: &[&str],
        externals: &[&str],
        gram: &[&[&str]],
    ) -> SymbolicaAffineDenominatorCompiler {
        let coefficients = CoefficientContext::new(parameters.iter().copied());
        let gram = gram
            .iter()
            .map(|row| {
                row.iter()
                    .map(|entry| coefficients.coefficient_fixture(entry))
                    .collect::<Vec<_>>()
            })
            .collect::<Vec<_>>();
        SymbolicaAffineDenominatorCompiler::try_new(
            coefficients,
            loops.iter().map(|name| (*name).to_owned()).collect(),
            externals.iter().map(|name| (*name).to_owned()).collect(),
            gram,
            SymbolicaAffineDenominatorLimits::default(),
        )
        .unwrap()
    }

    fn assert_coefficients(
        compiler: &SymbolicaAffineDenominatorCompiler,
        compiled: &CompiledSymbolicaAffineDenominator,
        expected_constant: &str,
        expected_coefficients: &[&str],
    ) {
        let context = &compiler.coefficients;
        assert_eq!(
            compiled.affine_denominator().constant(),
            &context.coefficient_fixture(expected_constant)
        );
        assert_eq!(
            compiled.affine_denominator().coefficients().len(),
            expected_coefficients.len()
        );
        for (actual, expected) in compiled
            .affine_denominator()
            .coefficients()
            .iter()
            .zip(expected_coefficients)
        {
            assert_eq!(actual, &context.coefficient_fixture(expected));
        }
    }

    fn checked_test_operation(
        compiler: &SymbolicaAffineDenominatorCompiler,
        left: &Coefficient,
        right: &Coefficient,
        operation: BinaryOperation,
    ) -> Result<Coefficient, SymbolicaAffineDenominatorError> {
        let mut work = ExactWorkBudget::default();
        match operation {
            BinaryOperation::Add => compiler.checked_add(left, right, &mut work),
            BinaryOperation::Multiply => compiler.checked_mul(left, right, &mut work),
            BinaryOperation::Divide => compiler.checked_div(left, right, &mut work),
        }
    }

    #[test]
    fn one_loop_external_square_contracts_gram_and_preserves_cross_factor() {
        let compiler = compiler(&["d", "m2", "s"], &["k"], &["p"], &[&["s"]]);
        let compiled = compiler.compile_expression("(k+p)^2-m2").unwrap();
        assert_coefficients(&compiler, &compiled, "s-m2", &["1", "2"]);
        assert_eq!(
            compiler.coordinates,
            &[
                ScalarProductCoordinate::LoopLoop { left: 0, right: 0 },
                ScalarProductCoordinate::LoopExternal {
                    loop_index: 0,
                    external_index: 0,
                },
            ]
        );
    }

    #[test]
    fn two_loop_square_lowers_in_generic_upper_triangular_order() {
        let compiler = compiler(&["m2"], &["k1", "k2"], &[], &[]);
        let compiled = compiler.compile_expression("(k1+k2)^2-m2").unwrap();
        assert_coefficients(&compiler, &compiled, "-m2", &["1", "2", "1"]);
    }

    #[test]
    fn validated_sp_accepts_rational_parameter_vector_coefficients() {
        let compiler = compiler(&["a", "s", "g"], &["k1", "k2"], &["p"], &[&["g"]]);
        let compiled = compiler
            .compile_expression("sp(a/s*k1+p,k2-2*p)+a/s*k1^2")
            .unwrap();
        assert_coefficients(
            &compiler,
            &compiled,
            "-2*g",
            &["a/s", "a/s", "0", "-2*a/s", "1"],
        );
    }

    #[test]
    fn exact_parameter_denominators_are_retained_without_map_extension() {
        let compiler = compiler(&["a"], &["k"], &[], &[]);
        let compiled = compiler.compile_expression("k^2/(a+1)").unwrap();
        assert_coefficients(&compiler, &compiled, "0", &["1/(a+1)"]);
        assert_eq!(
            compiler.parse_base_expression("(a-1)/(a+1)").unwrap(),
            compiler
                .test_coefficient_context()
                .coefficient_fixture("(a-1)/(a+1)")
        );
    }

    #[test]
    fn unknown_symbols_and_functions_are_rejected() {
        let compiler = compiler(&["a"], &["k"], &[], &[]);
        assert!(matches!(
            compiler.compile_expression("q^2"),
            Err(SymbolicaAffineDenominatorError::UnknownSymbol(_))
        ));
        assert!(matches!(
            compiler.compile_expression("f(k)"),
            Err(SymbolicaAffineDenominatorError::UnsupportedFunction(_))
        ));
    }

    #[test]
    fn momentum_denominators_noninteger_powers_and_wrong_degrees_are_rejected() {
        let compiler = compiler(&["a"], &["k"], &[], &[]);
        assert!(matches!(
            compiler.compile_expression("1/k"),
            Err(SymbolicaAffineDenominatorError::NegativeMomentumPower { .. })
        ));
        assert!(matches!(
            compiler.compile_expression("k^(1/2)"),
            Err(SymbolicaAffineDenominatorError::UnsupportedPower(_))
        ));
        assert!(matches!(
            compiler.compile_expression("k+a"),
            Err(SymbolicaAffineDenominatorError::MomentumDegreeOne { .. })
        ));
        assert!(matches!(
            compiler.compile_expression("k^3"),
            Err(SymbolicaAffineDenominatorError::MomentumDegreeTooHigh { degree: 3, .. })
        ));
    }

    #[test]
    fn scalar_product_requires_two_homogeneous_vector_linear_arguments() {
        let compiler = compiler(&["a"], &["k"], &[], &[]);
        assert!(matches!(
            compiler.compile_expression("sp(k+1,k)"),
            Err(SymbolicaAffineDenominatorError::InvalidScalarProductArgument { argument: 0, .. })
        ));
        assert!(matches!(
            compiler.compile_expression("sp(k)"),
            Err(SymbolicaAffineDenominatorError::MalformedScalarProduct { arguments: 1, .. })
        ));
        assert!(matches!(
            compiler.compile_expression("sp(sp(k,k)*k,k)"),
            Err(SymbolicaAffineDenominatorError::NestedScalarProduct(_))
        ));
    }

    #[test]
    fn gram_and_declaration_authentication_is_strict() {
        let coefficients = CoefficientContext::new(["s", "t"]);
        let s = coefficients.coefficient_fixture("s");
        let t = coefficients.coefficient_fixture("t");
        assert!(matches!(
            SymbolicaAffineDenominatorCompiler::try_new(
                coefficients.clone(),
                vec!["k".to_owned()],
                vec!["p".to_owned(), "q".to_owned()],
                vec![vec![s.clone(), t.clone()], vec![s, t.clone()]],
                SymbolicaAffineDenominatorLimits::default(),
            ),
            Err(SymbolicaAffineDenominatorError::AsymmetricExternalGram { .. })
        ));
        assert!(matches!(
            SymbolicaAffineDenominatorCompiler::try_new(
                coefficients,
                vec!["k".to_owned()],
                vec!["k".to_owned()],
                vec![vec![t]],
                SymbolicaAffineDenominatorLimits::default(),
            ),
            Err(SymbolicaAffineDenominatorError::DuplicateLabel { .. })
        ));
    }

    #[test]
    fn exact_and_one_below_input_node_limits_are_deterministic() {
        let compiler = compiler(&["a"], &["k"], &[], &[]);
        let source = try_parse!("k^2+a", default_namespace = RUSTRED_NAMESPACE).unwrap();
        let (_, exact_nodes) = {
            let shape = checked_atom_shape(
                source.as_view(),
                SymbolicaAffineDenominatorLimits::default(),
            )
            .unwrap();
            (shape.1, shape.0)
        };
        let mut exact = compiler.test_limits();
        exact.max_input_nodes = exact_nodes;
        compiler
            .test_with_limits(exact)
            .compile(source.as_view())
            .unwrap();

        let mut below = exact;
        below.max_input_nodes = exact_nodes - 1;
        let below = compiler.test_with_limits(below);
        assert!(matches!(
            below.compile(source.as_view()),
            Err(SymbolicaAffineDenominatorError::ResourceLimit {
                resource: "input Atom nodes",
                ..
            })
        ));
    }

    #[test]
    fn two_external_momenta_use_the_complete_off_diagonal_gram_matrix() {
        let compiler = compiler(
            &["spp", "spq", "sqq"],
            &["k"],
            &["p", "q"],
            &[&["spp", "spq"], &["spq", "sqq"]],
        );
        let square = compiler.compile_expression("(k+p+q)^2").unwrap();
        assert_coefficients(&compiler, &square, "spp+2*spq+sqq", &["1", "2", "2"]);
        let explicit = compiler.compile_expression("sp(p,q)+k^2").unwrap();
        assert_coefficients(&compiler, &explicit, "spq", &["1", "0", "0"]);
    }

    #[test]
    fn zero_parameter_fields_are_supported() {
        let compiler = compiler(&[], &["k"], &[], &[]);
        let compiled = compiler.compile_expression("k^2+1").unwrap();
        assert_coefficients(&compiler, &compiled, "1", &["1"]);
    }

    #[test]
    fn projection_denominator_replication_limit_has_exact_boundary() {
        let base = compiler(&["a"], &["k1", "k2"], &[], &[]);
        let expression = "(k1^2+k1*k2+k2^2)/(a+1)";
        let mut exact = base.test_limits();
        exact.max_projection_denominator_replication_terms = 6;
        let exact_compiler = base.test_with_limits(exact);
        exact_compiler.compile_expression(expression).unwrap();

        let mut below = exact;
        below.max_projection_denominator_replication_terms = 5;
        let below = base.test_with_limits(below);
        assert!(matches!(
            below.compile_expression(expression),
            Err(SymbolicaAffineDenominatorError::ResourceLimit {
                resource: "projection denominator replication terms",
                requested: 6,
                limit: 5,
            })
        ));
    }

    #[test]
    fn projection_group_and_retained_limits_precede_group_allocation() {
        let base = compiler(&["a"], &["k"], &[], &[]);
        let mut no_groups = base.test_limits();
        no_groups.max_projection_groups = 0;
        let no_groups = base.test_with_limits(no_groups);
        assert!(matches!(
            no_groups.compile_expression("k^2"),
            Err(SymbolicaAffineDenominatorError::ResourceLimit {
                resource: "aggregate projection groups",
                requested: 1,
                limit: 0,
            })
        ));

        let mut no_projection_storage = base.test_limits();
        no_projection_storage.max_projected_retained_bytes = 0;
        let no_projection_storage = base.test_with_limits(no_projection_storage);
        assert!(matches!(
            no_projection_storage.compile_expression("k^2"),
            Err(SymbolicaAffineDenominatorError::ResourceLimit {
                resource: "aggregate projected retained bytes",
                requested,
                limit: 0,
            }) if requested > 0
        ));
    }

    #[test]
    fn componentwise_dense_degree_box_limit_has_exact_boundary() {
        let base = compiler(&["a", "b"], &["k"], &[], &[]);
        let expression = "(a+1)*(b+1)*k^2";
        let mut exact = base.test_limits();
        exact.max_dense_degree_box_terms = 12;
        let exact_compiler = base.test_with_limits(exact);
        exact_compiler.compile_expression(expression).unwrap();

        let mut below = exact;
        below.max_dense_degree_box_terms = 11;
        let below = base.test_with_limits(below);
        assert!(matches!(
            below.compile_expression(expression),
            Err(SymbolicaAffineDenominatorError::ResourceLimit {
                resource: "dense numerator degree-box terms",
                requested: 12,
                limit: 11,
            })
        ));
    }

    #[test]
    fn normalized_gmp_integer_limit_has_exact_boundary() {
        let base = compiler(&["a"], &["k"], &[], &[]);
        let normalized_expression = "(a+1)^16*sp(k,k)";
        let source =
            try_parse!(normalized_expression, default_namespace = RUSTRED_NAMESPACE).unwrap();
        let mut evaluator = CheckedEvaluator::new(&base);
        let evaluated = evaluator.evaluate(source.as_view(), true).unwrap();
        let normalized_bits = normalized_expression_census(&evaluated)
            .unwrap()
            .integer_bits;
        assert!(normalized_bits > 8);
        let mut normalized_exact = base.test_limits();
        normalized_exact.max_normalized_expression_integer_bits = normalized_bits;
        let normalized_exact_compiler = base.test_with_limits(normalized_exact);
        normalized_exact_compiler
            .compile_expression(normalized_expression)
            .unwrap();

        let mut below_normalized = normalized_exact;
        below_normalized.max_normalized_expression_integer_bits = normalized_bits - 1;
        let below_normalized = base.test_with_limits(below_normalized);
        assert!(matches!(
            below_normalized.compile_expression(normalized_expression),
            Err(SymbolicaAffineDenominatorError::ResourceLimit {
                resource: "normalized expression integer bits",
                requested,
                limit,
            }) if requested == normalized_bits as u128 && limit + 1 == requested
        ));
    }

    #[test]
    fn signed_constants_coefficients_and_parameter_powers_are_exact() {
        let compiler = compiler(&["a"], &["k"], &[], &[]);
        let signed = compiler.compile_expression("-2*k^2-3").unwrap();
        assert_coefficients(&compiler, &signed, "-3", &["-2"]);
        let inverse_parameter = compiler.compile_expression("a^-2*k^2").unwrap();
        assert_coefficients(&compiler, &inverse_parameter, "0", &["1/a^2"]);
    }

    #[test]
    fn gcd_normalization_densification_is_bounded_before_add_mul_and_div() {
        let base = compiler(&["a", "b"], &["k"], &[], &[]);
        let cases = [
            (
                BinaryOperation::Add,
                "1/(a-1)",
                "(a^8-2)/(a-1)",
                8usize,
                9usize,
            ),
            (BinaryOperation::Multiply, "a^8-1", "1/(a-1)", 8, 9),
            (BinaryOperation::Divide, "a^8-1", "a-1", 8, 9),
            (BinaryOperation::Divide, "a^2*b^2-1", "a*b-1", 2, 9),
        ];
        for (operation, left, right, actual_terms, planned_terms) in cases {
            let left = base.combined.coefficient_fixture(left);
            let right = base.combined.coefficient_fixture(right);
            let allocation = exact_operation_allocation_envelope(
                &left,
                &right,
                operation,
                base.combined.parameter_names().len(),
            )
            .unwrap();
            assert!(allocation.numerator_terms >= planned_terms);

            let actual = checked_test_operation(&base, &left, &right, operation).unwrap();
            assert_eq!(actual.numerator.nterms(), actual_terms);
            assert!(actual.denominator.is_one());
            let actual_census = coefficient_census(&actual).unwrap();
            verify_operation_result_envelope(&actual, actual_census, allocation).unwrap();

            let mut exact = base.test_clone();
            exact.limits.max_combined_polynomial_terms =
                allocation.numerator_terms.max(allocation.denominator_terms);
            exact.limits.max_combined_exponent_entries = allocation.census.exponent_entries;
            exact.limits.max_coefficient_integer_bits = allocation.census.integer_bits;
            exact.limits.max_combined_retained_bytes = allocation.census.retained_bytes;
            checked_test_operation(&exact, &left, &right, operation).unwrap();

            let mut below_support = base.test_clone();
            below_support.limits.max_combined_polynomial_terms = allocation.numerator_terms - 1;
            assert!(matches!(
                checked_test_operation(&below_support, &left, &right, operation),
                Err(SymbolicaAffineDenominatorError::ResourceLimit {
                    resource: "combined exact-operation numerator term envelope",
                    requested,
                    limit,
                }) if requested == allocation.numerator_terms as u128 && limit + 1 == requested
            ));

            let mut below_integer = base.test_clone();
            below_integer.limits.max_coefficient_integer_bits = allocation.census.integer_bits - 1;
            assert!(matches!(
                checked_test_operation(&below_integer, &left, &right, operation),
                Err(SymbolicaAffineDenominatorError::ResourceLimit {
                    resource: "combined exact-operation integer bits",
                    requested,
                    limit,
                }) if requested == allocation.census.integer_bits as u128 && limit + 1 == requested
            ));

            let mut below_storage = base.test_clone();
            below_storage.limits.max_combined_retained_bytes = allocation.census.retained_bytes - 1;
            assert!(matches!(
                checked_test_operation(&below_storage, &left, &right, operation),
                Err(SymbolicaAffineDenominatorError::ResourceLimit {
                    resource: "combined exact-operation retained bytes",
                    requested,
                    limit,
                }) if requested == allocation.census.retained_bytes as u128 && limit + 1 == requested
            ));
        }
    }

    #[test]
    fn combined_integer_and_storage_envelopes_are_preoperation() {
        let base = compiler(&["a"], &["k"], &[], &[]);
        let expanded = base.compile_expression("(a+1)^256*k^2").unwrap();
        assert_eq!(
            expanded.affine_denominator().coefficients()[0]
                .numerator
                .nterms(),
            257
        );

        let half_power = base.combined.coefficient_fixture("(a+1)^128");
        let power_step = exact_operation_allocation_envelope(
            &half_power,
            &half_power,
            BinaryOperation::Multiply,
            base.combined.parameter_names().len(),
        )
        .unwrap();
        assert_eq!(power_step.numerator_terms, 257);
        let squared =
            checked_test_operation(&base, &half_power, &half_power, BinaryOperation::Multiply)
                .unwrap();
        assert_eq!(squared.numerator.nterms(), 257);
        let mut exact_power_step = base.test_clone();
        exact_power_step.limits.max_combined_polynomial_terms = 257;
        exact_power_step.limits.max_combined_exponent_entries = power_step.census.exponent_entries;
        exact_power_step.limits.max_coefficient_integer_bits = power_step.census.integer_bits;
        exact_power_step.limits.max_combined_retained_bytes = power_step.census.retained_bytes;
        checked_test_operation(
            &exact_power_step,
            &half_power,
            &half_power,
            BinaryOperation::Multiply,
        )
        .unwrap();

        let mut integer_limits = base.test_limits();
        integer_limits.max_coefficient_integer_bits = 128;
        let integer_bounded = base.test_with_limits(integer_limits);
        assert!(matches!(
            integer_bounded.compile_expression("(a+1)^256*k^2"),
            Err(SymbolicaAffineDenominatorError::ResourceLimit {
                resource: "combined exact-operation integer bits",
                requested,
                limit: 128,
            }) if requested > 128
        ));

        let mut support_bounded = base.test_clone();
        support_bounded.limits.max_combined_polynomial_terms = 256;
        assert!(matches!(
            support_bounded.compile_expression("(a+1)^256*k^2"),
            Err(SymbolicaAffineDenominatorError::ResourceLimit {
                resource: "combined exact-operation numerator term envelope",
                requested: 257,
                limit: 256,
            })
        ));

        let left = base.combined.parameter_at(0);
        let right = base.combined.parameter_at(0);
        let allocation = exact_operation_allocation_envelope(
            &left,
            &right,
            BinaryOperation::Multiply,
            base.combined.parameter_names().len(),
        )
        .unwrap();
        let mut storage_bounded = base.test_clone();
        storage_bounded.limits.max_combined_retained_bytes = allocation.census.retained_bytes - 1;
        let mut work = ExactWorkBudget::default();
        assert!(matches!(
            storage_bounded.checked_mul(&left, &right, &mut work),
            Err(SymbolicaAffineDenominatorError::ResourceLimit {
                resource: "combined exact-operation retained bytes",
                requested,
                limit,
            }) if requested == allocation.census.retained_bytes as u128 && limit + 1 == requested
        ));

        let one = base.combined.one();
        let mut no_storage = base.test_clone();
        no_storage.limits.max_combined_retained_bytes = 0;
        assert!(matches!(
            checked_test_operation(&no_storage, &one, &one, BinaryOperation::Multiply),
            Err(SymbolicaAffineDenominatorError::ResourceLimit {
                resource: "combined exact-operation retained bytes",
                requested,
                limit: 0,
            }) if requested > 0
        ));
        let mut deterministic = no_storage;
        deterministic.limits.max_coefficient_integer_bits = 0;
        assert!(matches!(
            checked_test_operation(&deterministic, &one, &one, BinaryOperation::Multiply),
            Err(SymbolicaAffineDenominatorError::ResourceLimit {
                resource: "combined exact-operation integer bits",
                requested,
                limit: 0,
            }) if requested > 0
        ));

        let large_base = base.combined.coefficient_fixture("(a+1)^16");
        let unit = planned_unit_coefficient_census(base.combined.parameter_names().len()).unwrap();
        let mut zero_power_compiler = base.test_clone();
        zero_power_compiler.limits.max_coefficient_integer_bits = unit.integer_bits;
        zero_power_compiler.limits.max_combined_retained_bytes = unit.retained_bytes;
        let mut evaluator = CheckedEvaluator::new(&zero_power_compiler);
        assert_eq!(
            evaluator.checked_power(&large_base, 0).unwrap(),
            zero_power_compiler.combined.one()
        );
        let mut zero_power_rejected = base.test_clone();
        zero_power_rejected.limits.max_coefficient_integer_bits = 0;
        zero_power_rejected.limits.max_combined_retained_bytes = 0;
        let mut evaluator = CheckedEvaluator::new(&zero_power_rejected);
        assert!(matches!(
            evaluator.checked_power(&large_base, 0),
            Err(SymbolicaAffineDenominatorError::ResourceLimit {
                resource: "combined power-result integer bits",
                requested,
                limit: 0,
            }) if requested > 0
        ));

        let overflowing_base = base.combined.coefficient_fixture("a^40000");
        let mut evaluator = CheckedEvaluator::new(&base);
        assert!(matches!(
            evaluator.checked_power(&overflowing_base, 2),
            Err(SymbolicaAffineDenominatorError::ExactAlgebra(
                ExactAlgebraError::ExponentLimit {
                    operation: crate::algebra::ExactAlgebraOperation::Power,
                    variable: 0,
                    requested: 80_000,
                    limit: crate::algebra::SYMBOLICA_COEFFICIENT_EXPONENT_LIMIT,
                }
            ))
        ));
        assert_eq!(evaluator.arithmetic_operations, 0);
    }

    #[test]
    fn explicit_external_scalar_products_close_compositionally() {
        let compiler = compiler(
            &["spp", "spq", "sqq"],
            &["k"],
            &["p", "q"],
            &[&["spp", "spq"], &["spq", "sqq"]],
        );
        let product = compiler.compile_expression("sp(p,q)*k^2").unwrap();
        assert_coefficients(&compiler, &product, "0", &["spq", "0", "0"]);
        let square = compiler.compile_expression("sp(p,p)^2+k^2").unwrap();
        assert_coefficients(&compiler, &square, "spp^2", &["1", "0", "0"]);
        let quotient = compiler.compile_expression("k^2/sp(p,p)").unwrap();
        assert_coefficients(&compiler, &quotient, "0", &["1/spp", "0", "0"]);
    }

    #[test]
    fn loop_coordinate_scalar_products_remain_nonlinear_under_products() {
        let compiler = compiler(&["spp"], &["k"], &["p"], &[&["spp"]]);
        assert!(matches!(
            compiler.compile_expression("sp(k,k)^2"),
            Err(SymbolicaAffineDenominatorError::MomentumDegreeTooHigh { degree: 4, .. })
        ));
        assert!(matches!(
            compiler.compile_expression("sp(k,p)*sp(k,p)"),
            Err(SymbolicaAffineDenominatorError::MomentumDegreeTooHigh { degree: 4, .. })
        ));
        assert!(matches!(
            compiler.compile_expression("1/sp(k,k)"),
            Err(SymbolicaAffineDenominatorError::NegativeMomentumPower { exponent: -1, .. })
        ));
    }

    #[test]
    fn projection_coordinate_and_gmp_denominator_storage_boundaries_are_exact() {
        let compiler = compiler(&["a"], &["k"], &[], &[]);
        let zero = compiler.test_coefficient_context().zero();
        let coordinate_baseline = multiply_census(
            coefficient_census(&zero).unwrap(),
            2,
            "test coordinate baseline",
        )
        .unwrap();
        let mut exact_limits = compiler.test_limits();
        exact_limits.max_projected_retained_bytes = coordinate_baseline.retained_bytes;
        let mut exact_budget = ProjectionAllocationBudget::default();
        exact_budget
            .charge(
                coordinate_baseline,
                exact_limits,
                "test coordinate baseline terms",
            )
            .unwrap();
        let mut below_limits = exact_limits;
        below_limits.max_projected_retained_bytes = coordinate_baseline.retained_bytes - 1;
        let mut below_budget = ProjectionAllocationBudget::default();
        assert!(matches!(
            below_budget.charge(
                coordinate_baseline,
                below_limits,
                "test coordinate baseline terms"
            ),
            Err(SymbolicaAffineDenominatorError::ResourceLimit {
                resource: "aggregate projected retained bytes",
                ..
            })
        ));

        let large = compiler
            .test_coefficient_context()
            .coefficient_fixture("1/(12345678901234567890123456789012345678901234567890*a+1)");
        assert!(
            large
                .denominator
                .coefficients
                .iter()
                .any(|integer| matches!(integer, Integer::Large(_)))
        );
        let denominator_replication = multiply_census(
            polynomial_census(&large.denominator).unwrap(),
            3,
            "test denominator replication",
        )
        .unwrap();
        let mut denominator_limits = compiler.test_limits();
        denominator_limits.max_projected_retained_bytes = denominator_replication.retained_bytes;
        let mut denominator_budget = ProjectionAllocationBudget::default();
        denominator_budget
            .charge(
                denominator_replication,
                denominator_limits,
                "test denominator replication terms",
            )
            .unwrap();
        denominator_limits.max_projected_retained_bytes -= 1;
        let mut below_denominator = ProjectionAllocationBudget::default();
        assert!(matches!(
            below_denominator.charge(
                denominator_replication,
                denominator_limits,
                "test denominator replication terms"
            ),
            Err(SymbolicaAffineDenominatorError::ResourceLimit {
                resource: "aggregate projected retained bytes",
                ..
            })
        ));
    }

    #[test]
    fn normalized_render_byte_preflight_has_exact_boundary() {
        let base = compiler(&["a"], &["k"], &[], &[]);
        let source = try_parse!("(a+1)*k^2", default_namespace = RUSTRED_NAMESPACE).unwrap();
        let mut evaluator = CheckedEvaluator::new(&base);
        let evaluated = evaluator.evaluate(source.as_view(), true).unwrap();
        let census = normalized_expression_census(&evaluated).unwrap();
        let maximum_symbol_bytes = maximum_combined_symbol_bytes(&base.combined).unwrap();
        let bound = normalized_expression_render_byte_bound(census, maximum_symbol_bytes).unwrap();
        let mut exact = base.test_limits();
        exact.max_normalized_expression_bytes = bound;
        let exact_compiler = base.test_with_limits(exact);
        exact_compiler.compile_expression("(a+1)*k^2").unwrap();
        let mut below = exact;
        below.max_normalized_expression_bytes = bound - 1;
        let below = base.test_with_limits(below);
        assert!(matches!(
            below.compile_expression("(a+1)*k^2"),
            Err(SymbolicaAffineDenominatorError::NormalizedExpressionTooLarge {
                requested,
                limit,
            }) if requested == bound && limit + 1 == requested
        ));
    }

    #[test]
    fn complete_compiled_retained_bound_has_exact_boundary() {
        let base = compiler(&["a"], &["k"], &[], &[]);
        let expression = "(a+1)*k^2";
        let baseline = base.compile_expression(expression).unwrap();
        let mut projected = coefficient_census(baseline.affine_denominator().constant()).unwrap();
        for coefficient in baseline.affine_denominator().coefficients() {
            projected
                .checked_add_assign(
                    coefficient_census(coefficient).unwrap(),
                    "test affine census",
                )
                .unwrap();
        }
        let variable_maps = retained_variable_map_arc_bytes(
            std::iter::once(baseline.affine_denominator().constant())
                .chain(baseline.affine_denominator().coefficients()),
        )
        .unwrap();
        let retained = compiled_retained_byte_bound(
            baseline.source().as_view().get_byte_size(),
            baseline.normalized_expression().as_view().get_byte_size(),
            projected.retained_bytes,
            variable_maps,
        )
        .unwrap();
        assert!(retained > std::mem::size_of::<CompiledSymbolicaAffineDenominator>());

        let mut exact = base.test_limits();
        exact.max_compiled_retained_bytes = retained;
        base.test_with_limits(exact)
            .compile_expression(expression)
            .unwrap();

        let mut below = exact;
        below.max_compiled_retained_bytes = retained - 1;
        let below = base.test_with_limits(below);
        assert!(matches!(
            below.compile_expression(expression),
            Err(SymbolicaAffineDenominatorError::ResourceLimit {
                resource: "compiled retained bytes",
                requested,
                limit,
            }) if requested == retained as u128 && limit + 1 == requested
        ));

        let mut zero = base.test_limits();
        zero.max_compiled_retained_bytes = 0;
        let zero = base.test_with_limits(zero);
        assert!(matches!(
            zero.compile_expression(expression),
            Err(SymbolicaAffineDenominatorError::ResourceLimit {
                resource: "compiled fixed retained bytes",
                requested,
                limit: 0,
            }) if requested > 0
        ));
    }

    #[test]
    fn retained_variable_maps_are_charged_once_per_distinct_arc() {
        let first_context = CoefficientContext::new(["a"]);
        let second_context = CoefficientContext::new(["a"]);
        let first = first_context.coefficient_fixture("a+1");
        let second = second_context.coefficient_fixture("a+1");
        let one = retained_variable_map_arc_bytes([&first]).unwrap();
        assert!(one > 0);
        assert_eq!(
            retained_variable_map_arc_bytes([&first, &first]).unwrap(),
            one
        );
        assert_eq!(
            retained_variable_map_arc_bytes([&first, &second]).unwrap(),
            one.checked_mul(2).unwrap()
        );
    }
}
