//! Bounded compact-input bridge for one concrete Symbolica target.
//!
//! [`SymbolicaIntegralInputCompiler`](crate::SymbolicaIntegralInputCompiler)
//! retains a concrete target without interpreting its numerator.  This module
//! connects that authenticated compact syntax to the topology-independent
//! tensor compiler and to [`ConcreteIntegralKey`].  It performs no textual
//! rewriting and never changes the coefficient field or family fingerprint.
//!
//! This first schema deliberately recognizes only exact declared loop labels
//! in `sp(k,k)`, even powers such as `k^2`, and `vec(k,mu)`.  Momentum sums and
//! alternate dialects require a separate typed extension rather than an
//! ambiguous fallback at this boundary.

use std::collections::{BTreeMap, BTreeSet};
use std::error::Error;
use std::fmt;
use std::mem::size_of;
use std::panic::{AssertUnwindSafe, catch_unwind};

use symbolica::atom::{
    Atom, AtomCore, AtomView, FunctionBuilder, NamespacedSymbol, SymbolBuilder, UserData,
};
use symbolica::prelude::*;

use crate::{
    AuthenticatedVacuumCovariantTensorPolynomialLowering, CompiledSymbolicaTensorNumerator,
    ConcreteIntegralKey, GenericTensorFamilyLimits, GenericTensorPolynomialError,
    GenericTensorPolynomialLimits, LoweredSymbolicaProjectV1, ParametricRelationError,
    SymbolicaTensorNumeratorCompiler, SymbolicaTensorNumeratorError,
    SymbolicaTensorNumeratorLimits, SymbolicaTensorSyntax,
};

pub const SYMBOLICA_COMPILED_TARGET_V1_SCHEMA: &str = "rustred.symbolica-compiled-target.v1";

const COMPACT_SCALAR_PRODUCT: &str = "rustred::sp";
const COMPACT_VECTOR: &str = "rustred::vec";
const COMPACT_METRIC: &str = "rustred::metric";
const INTERNAL_LOOP_VECTOR: &str = "rustred_target_internal::loop_vector_v1";
const INTERNAL_SPECTATOR_VECTOR: &str = "rustred_target_internal::spectator_vector_v1";
const INTERNAL_METRIC: &str = "rustred_target_internal::metric_v1";
const INTERNAL_DOT: &str = "rustred_target_internal::dot_v1";
const INTERNAL_DUMMY_INDEX: &str = "rustred_target_internal::dummy_index_v1";

/// Aggregate resource policy for compact translation and the existing tensor
/// projection/lowering stack.
///
/// Symbolica exposes the packed byte size of retained [`Atom`] payloads, but
/// not the allocator capacity of every native workspace or Rust container.
/// The two `observed_*_bytes` fields therefore bound exactly that observable
/// payload seam. Structural ownership and work are bounded independently by
/// the node, operation, tensor, polynomial, and lowering limits.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SymbolicaTargetNumeratorLimits {
    pub tensor: SymbolicaTensorNumeratorLimits,
    pub polynomial: GenericTensorPolynomialLimits,
    pub lowering: GenericTensorFamilyLimits,
    pub max_input_nodes: usize,
    pub max_nesting_depth: usize,
    pub max_translation_operations: u64,
    pub max_momentum_power: u32,
    pub max_loop_momenta: usize,
    pub max_external_momenta: usize,
    pub max_denominators: usize,
    pub max_total_momentum_label_bytes: usize,
    pub max_family_fingerprint_bytes: usize,
    pub max_observed_loop_identity_atom_bytes: usize,
    pub max_observed_input_atom_bytes: usize,
    pub max_translated_nodes: usize,
    pub max_observed_translated_atom_bytes: usize,
    pub max_observed_retained_payload_bytes: usize,
}

impl Default for SymbolicaTargetNumeratorLimits {
    fn default() -> Self {
        Self {
            tensor: SymbolicaTensorNumeratorLimits::default(),
            polynomial: GenericTensorPolynomialLimits::default(),
            lowering: GenericTensorFamilyLimits::default(),
            max_input_nodes: 100_000,
            max_nesting_depth: 256,
            max_translation_operations: 1_000_000,
            max_momentum_power: 256,
            max_loop_momenta: 1_024,
            max_external_momenta: 1_024,
            max_denominators: 16_384,
            max_total_momentum_label_bytes: 1024 * 1024,
            max_family_fingerprint_bytes: 64 * 1024 * 1024,
            max_observed_loop_identity_atom_bytes: 1024 * 1024,
            max_observed_input_atom_bytes: 64 * 1024 * 1024,
            max_translated_nodes: 1_000_000,
            max_observed_translated_atom_bytes: 64 * 1024 * 1024,
            max_observed_retained_payload_bytes: 256 * 1024 * 1024,
        }
    }
}

/// Observable census of one successful compact-target compilation.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct SymbolicaTargetCompilationStats {
    input_nodes: usize,
    maximum_nesting_depth: usize,
    observed_input_atom_bytes: usize,
    translation_operations: u64,
    scalar_product_calls: usize,
    momentum_power_calls: usize,
    indexed_vector_calls: usize,
    metric_calls: usize,
    translated_nodes: usize,
    observed_translated_atom_bytes: usize,
    observed_retained_payload_bytes: usize,
}

impl SymbolicaTargetCompilationStats {
    pub const fn input_nodes(self) -> usize {
        self.input_nodes
    }

    pub const fn maximum_nesting_depth(self) -> usize {
        self.maximum_nesting_depth
    }

    pub const fn observed_input_atom_bytes(self) -> usize {
        self.observed_input_atom_bytes
    }

    pub const fn translation_operations(self) -> u64 {
        self.translation_operations
    }

    pub const fn scalar_product_calls(self) -> usize {
        self.scalar_product_calls
    }

    pub const fn momentum_power_calls(self) -> usize {
        self.momentum_power_calls
    }

    pub const fn indexed_vector_calls(self) -> usize {
        self.indexed_vector_calls
    }

    pub const fn metric_calls(self) -> usize {
        self.metric_calls
    }

    pub const fn translated_nodes(self) -> usize {
        self.translated_nodes
    }

    pub const fn observed_translated_atom_bytes(self) -> usize {
        self.observed_translated_atom_bytes
    }

    pub const fn observed_retained_payload_bytes(self) -> usize {
        self.observed_retained_payload_bytes
    }
}

/// One family-bound compiler for authenticated compact target numerators.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SymbolicaTargetNumeratorCompiler {
    family_fingerprint: String,
    family_loop_names: Vec<String>,
    denominator_count: usize,
    scalar_product: Symbol,
    vector: Symbol,
    metric: Symbol,
    loop_positions: BTreeMap<Symbol, usize>,
    external_symbols: BTreeSet<Symbol>,
    loop_atoms: Vec<Atom>,
    tensor: SymbolicaTensorNumeratorCompiler,
    limits: SymbolicaTargetNumeratorLimits,
}

impl SymbolicaTargetNumeratorCompiler {
    /// Construct the bridge from the same lowered declaration that owns the
    /// normalized target.  Loop identities are allocated in the family's
    /// authenticated order; no label or loop count is inferred from the
    /// numerator.
    pub fn try_new(
        project: &LoweredSymbolicaProjectV1,
        limits: SymbolicaTargetNumeratorLimits,
    ) -> Result<Self, SymbolicaTargetNumeratorError> {
        catch_unwind(AssertUnwindSafe(|| Self::try_new_inner(project, limits))).map_err(|_| {
            SymbolicaTargetNumeratorError::SymbolicaPanic {
                operation: "compact target compiler construction",
            }
        })?
    }

    fn try_new_inner(
        project: &LoweredSymbolicaProjectV1,
        limits: SymbolicaTargetNumeratorLimits,
    ) -> Result<Self, SymbolicaTargetNumeratorError> {
        let family = project.family();
        if family.loop_momenta() != project.normalized().loop_momenta() {
            return Err(SymbolicaTargetNumeratorError::LoopDeclarationMismatch);
        }
        if family.denominator_count() != project.normalized().propagators().len() {
            return Err(
                SymbolicaTargetNumeratorError::DenominatorDeclarationMismatch {
                    family: family.denominator_count(),
                    normalized: project.normalized().propagators().len(),
                },
            );
        }
        check_usize_limit(
            "compact target loop momenta",
            family.loop_count(),
            limits.max_loop_momenta,
        )?;
        check_usize_limit(
            "compact target external momenta",
            family.external_count(),
            limits.max_external_momenta,
        )?;
        check_usize_limit(
            "compact target denominators",
            family.denominator_count(),
            limits.max_denominators,
        )?;
        check_usize_limit(
            "compact family fingerprint bytes",
            family.fingerprint_ref().len(),
            limits.max_family_fingerprint_bytes,
        )?;
        let total_label_bytes = family
            .loop_momenta()
            .iter()
            .chain(family.external_momenta())
            .try_fold(0usize, |total, label| {
                total.checked_add(label.len()).ok_or(
                    SymbolicaTargetNumeratorError::ResourceCountOverflow {
                        resource: "compact momentum label bytes",
                    },
                )
            })?;
        check_usize_limit(
            "compact momentum label bytes",
            total_label_bytes,
            limits.max_total_momentum_label_bytes,
        )?;

        let scalar_product = authenticated_plain_symbol(COMPACT_SCALAR_PRODUCT)?;
        let vector = authenticated_plain_symbol(COMPACT_VECTOR)?;
        let metric = authenticated_plain_symbol(COMPACT_METRIC)?;
        let syntax = SymbolicaTensorSyntax::new(
            authenticated_plain_symbol(INTERNAL_LOOP_VECTOR)?,
            authenticated_plain_symbol(INTERNAL_SPECTATOR_VECTOR)?,
            authenticated_plain_symbol(INTERNAL_METRIC)?,
            authenticated_plain_symbol(INTERNAL_DOT)?,
            authenticated_plain_symbol(INTERNAL_DUMMY_INDEX)?,
        );

        let mut loop_positions = BTreeMap::new();
        let mut loop_atoms = Vec::new();
        let mut loop_name_map = Vec::new();
        try_reserve(
            "compact loop identities",
            &mut loop_atoms,
            family.loop_count(),
        )?;
        try_reserve(
            "compact tensor loop map",
            &mut loop_name_map,
            family.loop_count(),
        )?;
        for (position, name) in family.loop_momenta().iter().enumerate() {
            let symbol = authenticated_plain_symbol(&format!("rustred::{name}"))?;
            if loop_positions.insert(symbol, position).is_some() {
                return Err(SymbolicaTargetNumeratorError::DuplicateLoopSymbol {
                    name: name.clone(),
                });
            }
            let ordinal = i64::try_from(position).map_err(|_| {
                SymbolicaTargetNumeratorError::ResourceLimit {
                    resource: "compact loop ordinal",
                    requested: position as u128,
                    limit: i64::MAX as u128,
                }
            })?;
            let atom = FunctionBuilder::new(syntax.loop_vector)
                .add_arg(Atom::num(ordinal))
                .finish();
            loop_atoms.push(atom.clone());
            loop_name_map.push((name.clone(), atom));
        }
        let loop_identity_atom_bytes = loop_atoms.iter().try_fold(0usize, |total, atom| {
            total.checked_add(atom.as_view().get_byte_size()).ok_or(
                SymbolicaTargetNumeratorError::ResourceCountOverflow {
                    resource: "compact loop identity Atom bytes",
                },
            )
        })?;
        check_usize_limit(
            "compact loop identity Atom bytes",
            loop_identity_atom_bytes,
            limits.max_observed_loop_identity_atom_bytes,
        )?;

        let mut external_symbols = BTreeSet::new();
        for name in project.normalized().external_momenta() {
            external_symbols.insert(authenticated_plain_symbol(&format!("rustred::{name}"))?);
        }
        let tensor = SymbolicaTensorNumeratorCompiler::try_new(
            family,
            syntax,
            loop_name_map,
            limits.tensor,
        )?;
        Ok(Self {
            family_fingerprint: family.fingerprint(),
            family_loop_names: family.loop_momenta().to_vec(),
            denominator_count: family.denominator_count(),
            scalar_product,
            vector,
            metric,
            loop_positions,
            external_symbols,
            loop_atoms,
            tensor,
            limits,
        })
    }

    pub const fn limits(&self) -> SymbolicaTargetNumeratorLimits {
        self.limits
    }

    pub fn family_fingerprint(&self) -> &str {
        &self.family_fingerprint
    }

    /// Compile the normalized target retained by `project`.
    pub fn compile(
        &self,
        project: &LoweredSymbolicaProjectV1,
    ) -> Result<CompiledSymbolicaTargetV1, SymbolicaTargetNumeratorError> {
        catch_unwind(AssertUnwindSafe(|| self.compile_inner(project))).map_err(|_| {
            SymbolicaTargetNumeratorError::SymbolicaPanic {
                operation: "compact target translation",
            }
        })?
    }

    fn compile_inner(
        &self,
        project: &LoweredSymbolicaProjectV1,
    ) -> Result<CompiledSymbolicaTargetV1, SymbolicaTargetNumeratorError> {
        self.check_project(project)?;
        let target = project.normalized().target();
        if target.powers().len() != self.denominator_count {
            return Err(SymbolicaTargetNumeratorError::WrongTargetArity {
                expected: self.denominator_count,
                actual: target.powers().len(),
            });
        }
        let integral = ConcreteIntegralKey::try_new(target.powers().iter().copied())?;
        let source = target.numerator();
        let observed_input_atom_bytes = source.as_view().get_byte_size();
        check_usize_limit(
            "observed compact target input Atom bytes",
            observed_input_atom_bytes,
            self.limits.max_observed_input_atom_bytes,
        )?;
        let input_shape = atom_shape(
            source.as_view(),
            self.limits.max_input_nodes,
            self.limits.max_nesting_depth,
            "compact target input",
        )?;
        let mut translator = Translator::new(self);
        let translated = translator.translate(source.as_view(), 0)?.atom;
        let translated_shape = atom_shape(
            translated.as_view(),
            self.limits.max_translated_nodes,
            self.limits.max_nesting_depth,
            "translated target",
        )?;
        let observed_translated_atom_bytes = translated.as_view().get_byte_size();
        check_usize_limit(
            "observed translated target Atom bytes",
            observed_translated_atom_bytes,
            self.limits.max_observed_translated_atom_bytes,
        )?;
        let tensor = self.tensor.compile(translated.as_view())?;
        let observed_retained_payload_bytes = observed_retained_payload_bytes(
            &self.family_fingerprint,
            &integral,
            source,
            &translated,
            &tensor,
            &self.loop_atoms,
        )?;
        check_usize_limit(
            "compiled compact target observed retained payload bytes",
            observed_retained_payload_bytes,
            self.limits.max_observed_retained_payload_bytes,
        )?;
        let stats = SymbolicaTargetCompilationStats {
            input_nodes: input_shape.nodes,
            maximum_nesting_depth: input_shape.maximum_depth,
            observed_input_atom_bytes,
            translation_operations: translator.operations,
            scalar_product_calls: translator.scalar_product_calls,
            momentum_power_calls: translator.momentum_power_calls,
            indexed_vector_calls: translator.indexed_vector_calls,
            metric_calls: translator.metric_calls,
            translated_nodes: translated_shape.nodes,
            observed_translated_atom_bytes,
            observed_retained_payload_bytes,
        };
        Ok(CompiledSymbolicaTargetV1 {
            schema: SYMBOLICA_COMPILED_TARGET_V1_SCHEMA,
            family_fingerprint: self.family_fingerprint.clone(),
            source_numerator: source.clone(),
            translated_numerator: translated,
            integral,
            tensor,
            stats,
            limits: self.limits,
        })
    }

    fn check_project(
        &self,
        project: &LoweredSymbolicaProjectV1,
    ) -> Result<(), SymbolicaTargetNumeratorError> {
        let actual = project.family().fingerprint_ref();
        if actual != self.family_fingerprint {
            return Err(SymbolicaTargetNumeratorError::WrongFamilyFingerprint {
                expected: self.family_fingerprint.clone(),
                actual: actual.to_owned(),
            });
        }
        if project.family().loop_momenta() != self.family_loop_names {
            return Err(SymbolicaTargetNumeratorError::LoopDeclarationMismatch);
        }
        if project.family().denominator_count() != self.denominator_count {
            return Err(SymbolicaTargetNumeratorError::WrongTargetArity {
                expected: self.denominator_count,
                actual: project.family().denominator_count(),
            });
        }
        Ok(())
    }
}

/// Lossless compact-target compilation and tensor identity transcript.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CompiledSymbolicaTargetV1 {
    schema: &'static str,
    family_fingerprint: String,
    source_numerator: Atom,
    translated_numerator: Atom,
    integral: ConcreteIntegralKey,
    tensor: CompiledSymbolicaTensorNumerator,
    stats: SymbolicaTargetCompilationStats,
    limits: SymbolicaTargetNumeratorLimits,
}

impl CompiledSymbolicaTargetV1 {
    pub const fn schema(&self) -> &'static str {
        self.schema
    }

    pub fn family_fingerprint(&self) -> &str {
        &self.family_fingerprint
    }

    pub const fn source_numerator(&self) -> &Atom {
        &self.source_numerator
    }

    pub const fn translated_numerator(&self) -> &Atom {
        &self.translated_numerator
    }

    pub const fn integral(&self) -> &ConcreteIntegralKey {
        &self.integral
    }

    pub const fn tensor(&self) -> &CompiledSymbolicaTensorNumerator {
        &self.tensor
    }

    pub const fn stats(&self) -> SymbolicaTargetCompilationStats {
        self.stats
    }

    pub const fn limits(&self) -> SymbolicaTargetNumeratorLimits {
        self.limits
    }

    /// Recompile the retained normalized target and compare the complete
    /// translation, tensor allocation transcript, and work census.
    pub fn verify_replay(
        &self,
        compiler: &SymbolicaTargetNumeratorCompiler,
        project: &LoweredSymbolicaProjectV1,
    ) -> Result<(), SymbolicaTargetNumeratorError> {
        let replay = compiler.compile(project)?;
        if replay == *self {
            Ok(())
        } else {
            Err(SymbolicaTargetNumeratorError::CompilationReplayMismatch)
        }
    }

    /// Enter the existing FORM-free vacuum tensor projection and exact family
    /// lowering.  The concrete powers are used only here, never during generic
    /// parametric IBP derivation.
    pub fn project_and_lower(
        &self,
        project: &LoweredSymbolicaProjectV1,
    ) -> Result<AuthenticatedVacuumCovariantTensorPolynomialLowering, SymbolicaTargetNumeratorError>
    {
        let actual = project.family().fingerprint_ref();
        if actual != self.family_fingerprint {
            return Err(SymbolicaTargetNumeratorError::WrongFamilyFingerprint {
                expected: self.family_fingerprint.clone(),
                actual: actual.to_owned(),
            });
        }
        let target = project.normalized().target();
        if target.powers().len() != self.integral.powers().len() {
            return Err(SymbolicaTargetNumeratorError::WrongTargetArity {
                expected: self.integral.powers().len(),
                actual: target.powers().len(),
            });
        }
        for (position, (&expected, &actual)) in self
            .integral
            .powers()
            .iter()
            .zip(target.powers())
            .enumerate()
        {
            if actual != expected {
                return Err(SymbolicaTargetNumeratorError::ConcreteTargetPowerMismatch {
                    position,
                    expected,
                    actual,
                });
            }
        }
        if target.numerator() != &self.source_numerator {
            return Err(SymbolicaTargetNumeratorError::ConcreteTargetNumeratorMismatch);
        }
        let projection = self
            .tensor
            .project(project.family(), self.limits.polynomial)?;
        let lowering =
            projection.lower_with_limits(project.family(), &self.integral, self.limits.lowering)?;
        lowering.verify(project.family())?;
        Ok(lowering)
    }
}

#[derive(Clone, Debug)]
struct TranslatedAtom {
    atom: Atom,
    contains_tensor: bool,
}

struct Translator<'a> {
    compiler: &'a SymbolicaTargetNumeratorCompiler,
    operations: u64,
    scalar_product_calls: usize,
    momentum_power_calls: usize,
    indexed_vector_calls: usize,
    metric_calls: usize,
}

impl<'a> Translator<'a> {
    fn new(compiler: &'a SymbolicaTargetNumeratorCompiler) -> Self {
        Self {
            compiler,
            operations: 0,
            scalar_product_calls: 0,
            momentum_power_calls: 0,
            indexed_vector_calls: 0,
            metric_calls: 0,
        }
    }

    fn translate(
        &mut self,
        source: AtomView<'_>,
        depth: usize,
    ) -> Result<TranslatedAtom, SymbolicaTargetNumeratorError> {
        check_usize_limit(
            "compact target translation depth",
            depth,
            self.compiler.limits.max_nesting_depth,
        )?;
        self.charge(1)?;
        match source {
            AtomView::Num(_) => Ok(TranslatedAtom {
                atom: source.to_owned(),
                contains_tensor: false,
            }),
            AtomView::Var(variable) => {
                let symbol = variable.get_symbol();
                if self.compiler.loop_positions.contains_key(&symbol) {
                    return Err(SymbolicaTargetNumeratorError::BareMomentum {
                        atom: source.to_owned(),
                    });
                }
                if self.compiler.external_symbols.contains(&symbol) {
                    return Err(SymbolicaTargetNumeratorError::ExternalMomentumUnsupported {
                        atom: source.to_owned(),
                    });
                }
                Ok(TranslatedAtom {
                    atom: source.to_owned(),
                    contains_tensor: false,
                })
            }
            AtomView::Add(sum) => {
                let mut atom = Atom::num(0);
                let mut contains_tensor = false;
                for child in sum.iter() {
                    let child = self.translate(child, checked_depth(depth)?)?;
                    self.charge(1)?;
                    atom += child.atom;
                    contains_tensor |= child.contains_tensor;
                }
                Ok(TranslatedAtom {
                    atom,
                    contains_tensor,
                })
            }
            AtomView::Mul(product) => {
                let mut atom = Atom::num(1);
                let mut contains_tensor = false;
                for child in product.iter() {
                    let child = self.translate(child, checked_depth(depth)?)?;
                    self.charge(1)?;
                    atom *= child.atom;
                    contains_tensor |= child.contains_tensor;
                }
                Ok(TranslatedAtom {
                    atom,
                    contains_tensor,
                })
            }
            AtomView::Pow(power) => {
                let exponent = i64::try_from(power.get_exp()).map_err(|_| {
                    SymbolicaTargetNumeratorError::UnsupportedPower {
                        atom: source.to_owned(),
                    }
                })?;
                if let Some(position) = self.exact_loop_position(power.get_base())? {
                    return self.translate_momentum_power(source, position, exponent);
                }
                let base = self.translate(power.get_base(), checked_depth(depth)?)?;
                if exponent < 0 && base.contains_tensor {
                    return Err(SymbolicaTargetNumeratorError::NegativeTensorPower {
                        atom: source.to_owned(),
                        exponent,
                    });
                }
                self.charge(1)?;
                Ok(TranslatedAtom {
                    atom: base.atom.pow(power.get_exp().to_owned()),
                    contains_tensor: base.contains_tensor,
                })
            }
            AtomView::Fun(function) if function.get_symbol() == self.compiler.scalar_product => {
                if function.get_nargs() != 2 {
                    return Err(SymbolicaTargetNumeratorError::MalformedScalarProduct {
                        atom: source.to_owned(),
                        arguments: function.get_nargs(),
                    });
                }
                let left = self.exact_required_loop(function.get(0), 0, source)?;
                let right = self.exact_required_loop(function.get(1), 1, source)?;
                self.scalar_product_calls =
                    checked_increment("compact scalar-product calls", self.scalar_product_calls)?;
                self.charge(1)?;
                Ok(TranslatedAtom {
                    atom: symmetric_binary(
                        self.compiler.tensor.syntax().dot,
                        self.compiler.loop_atoms[left].clone(),
                        self.compiler.loop_atoms[right].clone(),
                    ),
                    contains_tensor: true,
                })
            }
            AtomView::Fun(function) if function.get_symbol() == self.compiler.vector => {
                if function.get_nargs() != 2 {
                    return Err(SymbolicaTargetNumeratorError::MalformedVector {
                        atom: source.to_owned(),
                        arguments: function.get_nargs(),
                    });
                }
                let position = self.exact_required_loop(function.get(0), 0, source)?;
                self.validate_index(function.get(1), checked_depth(depth)?)?;
                self.indexed_vector_calls =
                    checked_increment("compact indexed-vector calls", self.indexed_vector_calls)?;
                self.charge(1)?;
                Ok(TranslatedAtom {
                    atom: FunctionBuilder::new(self.compiler.tensor.syntax().loop_vector)
                        .add_arg(Atom::num(i64::try_from(position).map_err(|_| {
                            SymbolicaTargetNumeratorError::ResourceLimit {
                                resource: "compact loop ordinal",
                                requested: position as u128,
                                limit: i64::MAX as u128,
                            }
                        })?))
                        .add_arg(function.get(1).to_owned())
                        .finish(),
                    contains_tensor: true,
                })
            }
            AtomView::Fun(function) if function.get_symbol() == self.compiler.metric => {
                if function.get_nargs() != 2 {
                    return Err(SymbolicaTargetNumeratorError::MalformedMetric {
                        atom: source.to_owned(),
                        arguments: function.get_nargs(),
                    });
                }
                self.validate_index(function.get(0), checked_depth(depth)?)?;
                self.validate_index(function.get(1), checked_depth(depth)?)?;
                self.metric_calls = checked_increment("compact metric calls", self.metric_calls)?;
                self.charge(1)?;
                Ok(TranslatedAtom {
                    atom: symmetric_binary(
                        self.compiler.tensor.syntax().metric,
                        function.get(0).to_owned(),
                        function.get(1).to_owned(),
                    ),
                    contains_tensor: true,
                })
            }
            AtomView::Fun(_) => Err(SymbolicaTargetNumeratorError::UnsupportedFunction {
                atom: source.to_owned(),
            }),
        }
    }

    fn translate_momentum_power(
        &mut self,
        source: AtomView<'_>,
        position: usize,
        exponent: i64,
    ) -> Result<TranslatedAtom, SymbolicaTargetNumeratorError> {
        if exponent < 0 {
            return Err(SymbolicaTargetNumeratorError::NegativeMomentumPower {
                atom: source.to_owned(),
                exponent,
            });
        }
        if exponent % 2 != 0 {
            return Err(SymbolicaTargetNumeratorError::OddMomentumPower {
                atom: source.to_owned(),
                exponent,
            });
        }
        let exponent_u32 = u32::try_from(exponent).map_err(|_| {
            SymbolicaTargetNumeratorError::UnsupportedPower {
                atom: source.to_owned(),
            }
        })?;
        if exponent_u32 > self.compiler.limits.max_momentum_power {
            return Err(SymbolicaTargetNumeratorError::ResourceLimit {
                resource: "compact momentum power",
                requested: u128::from(exponent_u32),
                limit: u128::from(self.compiler.limits.max_momentum_power),
            });
        }
        self.momentum_power_calls =
            checked_increment("compact momentum-power calls", self.momentum_power_calls)?;
        self.charge(1)?;
        let dot = symmetric_binary(
            self.compiler.tensor.syntax().dot,
            self.compiler.loop_atoms[position].clone(),
            self.compiler.loop_atoms[position].clone(),
        );
        let half = exponent_u32 / 2;
        Ok(TranslatedAtom {
            atom: match half {
                0 => Atom::num(1),
                1 => dot,
                _ => dot.pow(Atom::num(i64::from(half))),
            },
            contains_tensor: half != 0,
        })
    }

    fn exact_loop_position(
        &self,
        atom: AtomView<'_>,
    ) -> Result<Option<usize>, SymbolicaTargetNumeratorError> {
        let AtomView::Var(variable) = atom else {
            return Ok(None);
        };
        let symbol = variable.get_symbol();
        if let Some(&position) = self.compiler.loop_positions.get(&symbol) {
            Ok(Some(position))
        } else if self.compiler.external_symbols.contains(&symbol) {
            Err(SymbolicaTargetNumeratorError::ExternalMomentumUnsupported {
                atom: atom.to_owned(),
            })
        } else {
            Ok(None)
        }
    }

    fn exact_required_loop(
        &self,
        atom: AtomView<'_>,
        argument: usize,
        parent: AtomView<'_>,
    ) -> Result<usize, SymbolicaTargetNumeratorError> {
        self.exact_loop_position(atom)?.ok_or_else(|| {
            SymbolicaTargetNumeratorError::UnsupportedMomentumArgument {
                parent: parent.to_owned(),
                argument,
                atom: atom.to_owned(),
            }
        })
    }

    fn validate_index(
        &mut self,
        atom: AtomView<'_>,
        depth: usize,
    ) -> Result<(), SymbolicaTargetNumeratorError> {
        let mut pending = Vec::new();
        push_pending_atom(
            &mut pending,
            atom,
            depth,
            self.compiler.limits.max_input_nodes,
            "compact Lorentz-index traversal stack",
        )?;
        let mut inspected = 0usize;
        while let Some((current, current_depth)) = pending.pop() {
            inspected = checked_increment("compact Lorentz-index nodes", inspected)?;
            check_usize_limit(
                "compact Lorentz-index nodes",
                inspected,
                self.compiler.limits.max_input_nodes,
            )?;
            check_usize_limit(
                "compact Lorentz-index depth",
                current_depth,
                self.compiler.limits.max_nesting_depth,
            )?;
            self.charge(1)?;
            match current {
                AtomView::Var(variable) => {
                    let symbol = variable.get_symbol();
                    if self.compiler.loop_positions.contains_key(&symbol)
                        || self.compiler.external_symbols.contains(&symbol)
                    {
                        return Err(SymbolicaTargetNumeratorError::MomentumInLorentzIndex {
                            atom: atom.to_owned(),
                        });
                    }
                }
                AtomView::Fun(function)
                    if function.get_symbol() == self.compiler.scalar_product
                        || function.get_symbol() == self.compiler.vector
                        || function.get_symbol() == self.compiler.metric =>
                {
                    return Err(SymbolicaTargetNumeratorError::TensorSyntaxInLorentzIndex {
                        atom: atom.to_owned(),
                    });
                }
                AtomView::Fun(function) => {
                    let next_depth = checked_depth(current_depth)?;
                    for child in function.iter() {
                        push_pending_atom(
                            &mut pending,
                            child,
                            next_depth,
                            self.compiler.limits.max_input_nodes,
                            "compact Lorentz-index traversal stack",
                        )?;
                    }
                }
                AtomView::Pow(power) => {
                    let next_depth = checked_depth(current_depth)?;
                    for child in power.iter() {
                        push_pending_atom(
                            &mut pending,
                            child,
                            next_depth,
                            self.compiler.limits.max_input_nodes,
                            "compact Lorentz-index traversal stack",
                        )?;
                    }
                }
                AtomView::Mul(product) => {
                    let next_depth = checked_depth(current_depth)?;
                    for child in product.iter() {
                        push_pending_atom(
                            &mut pending,
                            child,
                            next_depth,
                            self.compiler.limits.max_input_nodes,
                            "compact Lorentz-index traversal stack",
                        )?;
                    }
                }
                AtomView::Add(sum) => {
                    let next_depth = checked_depth(current_depth)?;
                    for child in sum.iter() {
                        push_pending_atom(
                            &mut pending,
                            child,
                            next_depth,
                            self.compiler.limits.max_input_nodes,
                            "compact Lorentz-index traversal stack",
                        )?;
                    }
                }
                AtomView::Num(_) => {}
            }
        }
        Ok(())
    }

    fn charge(&mut self, amount: u64) -> Result<(), SymbolicaTargetNumeratorError> {
        let requested = self.operations.checked_add(amount).ok_or(
            SymbolicaTargetNumeratorError::ResourceCountOverflow {
                resource: "compact target translation operations",
            },
        )?;
        if requested > self.compiler.limits.max_translation_operations {
            return Err(SymbolicaTargetNumeratorError::WorkLimit {
                resource: "compact target translation operations",
                requested,
                limit: self.compiler.limits.max_translation_operations,
            });
        }
        self.operations = requested;
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, Default)]
struct AtomShape {
    nodes: usize,
    maximum_depth: usize,
}

fn atom_shape(
    atom: AtomView<'_>,
    max_nodes: usize,
    max_depth: usize,
    resource: &'static str,
) -> Result<AtomShape, SymbolicaTargetNumeratorError> {
    let mut shape = AtomShape::default();
    let mut pending = Vec::new();
    push_pending_atom(&mut pending, atom, 0, max_nodes, resource)?;
    while let Some((current, depth)) = pending.pop() {
        shape.nodes = shape
            .nodes
            .checked_add(1)
            .ok_or(SymbolicaTargetNumeratorError::ResourceCountOverflow { resource })?;
        check_usize_limit(resource, shape.nodes, max_nodes)?;
        check_usize_limit(resource, depth, max_depth)?;
        shape.maximum_depth = shape.maximum_depth.max(depth);
        let next_depth = checked_depth(depth)?;
        match current {
            AtomView::Fun(function) => {
                for child in function.iter() {
                    push_pending_atom(&mut pending, child, next_depth, max_nodes, resource)?;
                }
            }
            AtomView::Pow(power) => {
                for child in power.iter() {
                    push_pending_atom(&mut pending, child, next_depth, max_nodes, resource)?;
                }
            }
            AtomView::Mul(product) => {
                for child in product.iter() {
                    push_pending_atom(&mut pending, child, next_depth, max_nodes, resource)?;
                }
            }
            AtomView::Add(sum) => {
                for child in sum.iter() {
                    push_pending_atom(&mut pending, child, next_depth, max_nodes, resource)?;
                }
            }
            AtomView::Num(_) | AtomView::Var(_) => {}
        }
    }
    Ok(shape)
}

fn push_pending_atom<'a>(
    pending: &mut Vec<(AtomView<'a>, usize)>,
    atom: AtomView<'a>,
    depth: usize,
    limit: usize,
    resource: &'static str,
) -> Result<(), SymbolicaTargetNumeratorError> {
    let requested = pending
        .len()
        .checked_add(1)
        .ok_or(SymbolicaTargetNumeratorError::ResourceCountOverflow { resource })?;
    check_usize_limit(resource, requested, limit)?;
    pending
        .try_reserve(1)
        .map_err(|_| SymbolicaTargetNumeratorError::AllocationFailure {
            resource,
            requested,
        })?;
    pending.push((atom, depth));
    Ok(())
}

/// Sum every retained payload exposed by the public Atom/transcript APIs. This
/// is intentionally not named an ownership bound: Symbolica does not expose
/// native workspace capacity, and Rust container capacities are private in the
/// delegated compiler.
fn observed_retained_payload_bytes(
    fingerprint: &str,
    integral: &ConcreteIntegralKey,
    source: &Atom,
    translated: &Atom,
    tensor: &CompiledSymbolicaTensorNumerator,
    loop_atoms: &[Atom],
) -> Result<usize, SymbolicaTargetNumeratorError> {
    let mut bytes = fingerprint.len();
    // The outer compiled-target certificate and the delegated compiled tensor
    // transcript own independent fingerprint Strings.
    add_bytes(&mut bytes, tensor.family_fingerprint().len())?;
    add_bytes(
        &mut bytes,
        integral
            .powers()
            .len()
            .checked_mul(size_of::<i64>())
            .ok_or(SymbolicaTargetNumeratorError::ResourceCountOverflow {
                resource: "compiled compact target observed retained payload bytes",
            })?,
    )?;
    add_bytes(&mut bytes, source.as_view().get_byte_size())?;
    // `translated_numerator` and the inner compiler's retained source are
    // separate owned Atoms.
    add_bytes(&mut bytes, translated.as_view().get_byte_size())?;
    add_bytes(&mut bytes, tensor.source().as_view().get_byte_size())?;
    for atom in loop_atoms {
        add_bytes(&mut bytes, atom.as_view().get_byte_size())?;
    }
    for term in tensor.terms() {
        add_bytes(&mut bytes, term.weight().as_view().get_byte_size())?;
    }
    for allocation in tensor.index_allocations() {
        add_bytes(&mut bytes, allocation.atom().as_view().get_byte_size())?;
    }
    for allocation in tensor.spectator_allocations() {
        add_bytes(&mut bytes, allocation.atom().as_view().get_byte_size())?;
    }
    Ok(bytes)
}

fn add_bytes(total: &mut usize, amount: usize) -> Result<(), SymbolicaTargetNumeratorError> {
    *total =
        total
            .checked_add(amount)
            .ok_or(SymbolicaTargetNumeratorError::ResourceCountOverflow {
                resource: "compiled compact target observed retained payload bytes",
            })?;
    Ok(())
}

fn checked_increment(
    resource: &'static str,
    current: usize,
) -> Result<usize, SymbolicaTargetNumeratorError> {
    current
        .checked_add(1)
        .ok_or(SymbolicaTargetNumeratorError::ResourceCountOverflow { resource })
}

fn checked_depth(depth: usize) -> Result<usize, SymbolicaTargetNumeratorError> {
    depth
        .checked_add(1)
        .ok_or(SymbolicaTargetNumeratorError::ResourceCountOverflow {
            resource: "compact target nesting depth",
        })
}

fn symmetric_binary(symbol: Symbol, mut left: Atom, mut right: Atom) -> Atom {
    if right < left {
        std::mem::swap(&mut left, &mut right);
    }
    FunctionBuilder::new(symbol)
        .add_args([left, right])
        .finish()
}

fn authenticated_plain_symbol(name: &str) -> Result<Symbol, SymbolicaTargetNumeratorError> {
    let namespaced = NamespacedSymbol::try_parse(name)
        .ok_or_else(|| SymbolicaTargetNumeratorError::Symbol(name.to_owned()))?;
    let symbol = if let Some(symbol) = Symbol::get_symbol(namespaced.clone()) {
        symbol
    } else {
        SymbolBuilder::new(namespaced)
            .build()
            .map_err(|error| SymbolicaTargetNumeratorError::Symbol(error.to_string()))?
    };
    if symbol.get_name() != name
        || symbol.get_wildcard_level() != 0
        || symbol.has_attributes()
        || !symbol.is_exportable()
        || !symbol.get_aliases().is_empty()
        || !matches!(symbol.get_data(), UserData::None)
    {
        return Err(SymbolicaTargetNumeratorError::UnsafeRegisteredSymbol {
            symbol: name.to_owned(),
        });
    }
    Ok(symbol)
}

fn try_reserve<T>(
    resource: &'static str,
    target: &mut Vec<T>,
    additional: usize,
) -> Result<(), SymbolicaTargetNumeratorError> {
    target.try_reserve_exact(additional).map_err(|_| {
        SymbolicaTargetNumeratorError::AllocationFailure {
            resource,
            requested: additional,
        }
    })
}

fn check_usize_limit(
    resource: &'static str,
    requested: usize,
    limit: usize,
) -> Result<(), SymbolicaTargetNumeratorError> {
    if requested > limit {
        Err(SymbolicaTargetNumeratorError::ResourceLimit {
            resource,
            requested: requested as u128,
            limit: limit as u128,
        })
    } else {
        Ok(())
    }
}

/// Typed failures for authenticated compact target translation.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SymbolicaTargetNumeratorError {
    Symbol(String),
    UnsafeRegisteredSymbol {
        symbol: String,
    },
    DuplicateLoopSymbol {
        name: String,
    },
    LoopDeclarationMismatch,
    DenominatorDeclarationMismatch {
        family: usize,
        normalized: usize,
    },
    WrongFamilyFingerprint {
        expected: String,
        actual: String,
    },
    WrongTargetArity {
        expected: usize,
        actual: usize,
    },
    ConcreteTargetPowerMismatch {
        position: usize,
        expected: i64,
        actual: i64,
    },
    ConcreteTargetNumeratorMismatch,
    BareMomentum {
        atom: Atom,
    },
    ExternalMomentumUnsupported {
        atom: Atom,
    },
    UnsupportedMomentumArgument {
        parent: Atom,
        argument: usize,
        atom: Atom,
    },
    MalformedScalarProduct {
        atom: Atom,
        arguments: usize,
    },
    MalformedVector {
        atom: Atom,
        arguments: usize,
    },
    MalformedMetric {
        atom: Atom,
        arguments: usize,
    },
    UnsupportedFunction {
        atom: Atom,
    },
    MomentumInLorentzIndex {
        atom: Atom,
    },
    TensorSyntaxInLorentzIndex {
        atom: Atom,
    },
    NegativeMomentumPower {
        atom: Atom,
        exponent: i64,
    },
    OddMomentumPower {
        atom: Atom,
        exponent: i64,
    },
    NegativeTensorPower {
        atom: Atom,
        exponent: i64,
    },
    UnsupportedPower {
        atom: Atom,
    },
    CompilationReplayMismatch,
    ResourceLimit {
        resource: &'static str,
        requested: u128,
        limit: u128,
    },
    WorkLimit {
        resource: &'static str,
        requested: u64,
        limit: u64,
    },
    ResourceCountOverflow {
        resource: &'static str,
    },
    AllocationFailure {
        resource: &'static str,
        requested: usize,
    },
    SymbolicaPanic {
        operation: &'static str,
    },
    ConcreteIntegral(ParametricRelationError),
    Tensor(SymbolicaTensorNumeratorError),
    Polynomial(GenericTensorPolynomialError),
}

impl fmt::Display for SymbolicaTargetNumeratorError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Symbol(error) => {
                write!(formatter, "cannot register compact target symbol: {error}")
            }
            Self::UnsafeRegisteredSymbol { symbol } => write!(
                formatter,
                "compact target symbol {symbol:?} has unsafe attributes, aliases, callbacks, or user data"
            ),
            Self::DuplicateLoopSymbol { name } => {
                write!(formatter, "compact loop symbol {name:?} occurs twice")
            }
            Self::LoopDeclarationMismatch => formatter
                .write_str("normalized and lowered projects disagree on ordered loop declarations"),
            Self::DenominatorDeclarationMismatch { family, normalized } => write!(
                formatter,
                "lowered family has {family} denominators but normalized input retains {normalized}"
            ),
            Self::WrongFamilyFingerprint { expected, actual } => write!(
                formatter,
                "compact target compiler belongs to family {expected:?}, not {actual:?}"
            ),
            Self::WrongTargetArity { expected, actual } => write!(
                formatter,
                "compact target has {actual} powers, expected {expected}"
            ),
            Self::ConcreteTargetPowerMismatch {
                position,
                expected,
                actual,
            } => write!(
                formatter,
                "supplied project target power {position} is {actual}, but the compiled target retains {expected}"
            ),
            Self::ConcreteTargetNumeratorMismatch => formatter.write_str(
                "supplied project numerator differs from the numerator retained by the compiled target",
            ),
            Self::BareMomentum { atom } => write!(
                formatter,
                "declared momentum {atom} occurs bare in scalar numerator context"
            ),
            Self::ExternalMomentumUnsupported { atom } => write!(
                formatter,
                "external family momentum {atom} is unsupported by the vacuum target bridge"
            ),
            Self::UnsupportedMomentumArgument {
                parent,
                argument,
                atom,
            } => write!(
                formatter,
                "argument {argument} of {parent} is not one exact declared loop momentum: {atom}"
            ),
            Self::MalformedScalarProduct { atom, arguments } => write!(
                formatter,
                "compact scalar product {atom} has {arguments} arguments, expected 2"
            ),
            Self::MalformedVector { atom, arguments } => write!(
                formatter,
                "compact indexed vector {atom} has {arguments} arguments, expected 2"
            ),
            Self::MalformedMetric { atom, arguments } => write!(
                formatter,
                "compact metric {atom} has {arguments} arguments, expected 2"
            ),
            Self::UnsupportedFunction { atom } => {
                write!(formatter, "unsupported compact numerator function {atom}")
            }
            Self::MomentumInLorentzIndex { atom } => {
                write!(
                    formatter,
                    "Lorentz index contains a declared momentum: {atom}"
                )
            }
            Self::TensorSyntaxInLorentzIndex { atom } => {
                write!(
                    formatter,
                    "Lorentz index contains compact tensor syntax: {atom}"
                )
            }
            Self::NegativeMomentumPower { atom, exponent } => write!(
                formatter,
                "declared momentum power {atom} has negative exponent {exponent}"
            ),
            Self::OddMomentumPower { atom, exponent } => write!(
                formatter,
                "scalar momentum spelling {atom} has odd exponent {exponent}"
            ),
            Self::NegativeTensorPower { atom, exponent } => write!(
                formatter,
                "tensor-containing power {atom} has negative exponent {exponent}"
            ),
            Self::UnsupportedPower { atom } => {
                write!(formatter, "unsupported compact numerator power {atom}")
            }
            Self::CompilationReplayMismatch => formatter
                .write_str("recompiled compact target differs from its retained transcript"),
            Self::ResourceLimit {
                resource,
                requested,
                limit,
            } => write!(
                formatter,
                "{resource} needs {requested} units, exceeding configured limit {limit}"
            ),
            Self::WorkLimit {
                resource,
                requested,
                limit,
            } => write!(
                formatter,
                "{resource} needs {requested} operations, exceeding configured limit {limit}"
            ),
            Self::ResourceCountOverflow { resource } => {
                write!(formatter, "{resource} count overflowed its representation")
            }
            Self::AllocationFailure {
                resource,
                requested,
            } => write!(
                formatter,
                "could not allocate {requested} units for {resource}"
            ),
            Self::SymbolicaPanic { operation } => {
                write!(formatter, "Symbolica panicked during {operation}")
            }
            Self::ConcreteIntegral(error) => error.fmt(formatter),
            Self::Tensor(error) => error.fmt(formatter),
            Self::Polynomial(error) => error.fmt(formatter),
        }
    }
}

impl Error for SymbolicaTargetNumeratorError {}

impl From<ParametricRelationError> for SymbolicaTargetNumeratorError {
    fn from(error: ParametricRelationError) -> Self {
        Self::ConcreteIntegral(error)
    }
}

impl From<SymbolicaTensorNumeratorError> for SymbolicaTargetNumeratorError {
    fn from(error: SymbolicaTensorNumeratorError) -> Self {
        Self::Tensor(error)
    }
}

impl From<GenericTensorPolynomialError> for SymbolicaTargetNumeratorError {
    fn from(error: GenericTensorPolynomialError) -> Self {
        Self::Polynomial(error)
    }
}
