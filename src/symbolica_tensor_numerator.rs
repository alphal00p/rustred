//! Bounded Symbolica `Atom` boundary for generic vacuum tensor numerators.
//!
//! This module deliberately knows neither Vakint topology names nor loop counts.
//! A caller supplies the tensor heads and an exact map from the loop names of an
//! [`IntegralFamily`] to bare Symbolica vector atoms.  Compilation preserves the
//! original `Atom`, arbitrary scalar weights, decorated Lorentz indices, and
//! spectator-vector identities while translating tensor factors to
//! [`CovariantTensorMonomial`] values.  Only an explicit later conversion may
//! place scalar weights in the family's exact coefficient field.

use std::collections::{BTreeMap, BTreeSet};
use std::error::Error;
use std::fmt;

use symbolica::atom::{NamespacedSymbol, SymbolBuilder, representation::FunView};
use symbolica::prelude::*;

use crate::{
    AuthenticatedVacuumCovariantTensorPolynomialProjection, CovariantTensorMonomial,
    GenericCovariantTensorNumerator, GenericTensorPolynomialError, GenericTensorPolynomialLimits,
    GenericTensorProjectorError, GenericTensorProjectorLimits,
    GenericVacuumTensorPolynomialProjector, IndexedSpectatorVector, IndexedVector, IntegralFamily,
    LoopVector, LorentzIndex, Metric, ScalarProduct, ScalarProductCoordinate,
    ScalarProductMonomial, SpectatorScalarProduct, SpectatorScalarProductMonomial, SpectatorVector,
    TensorCovariantStructure, TensorError, WeightedCovariantTensorMonomial,
};

/// Resource policy shared by normalization, decoding, and rendering.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SymbolicaTensorNumeratorLimits {
    pub projector: GenericTensorProjectorLimits,
    pub max_input_nodes: usize,
    pub max_nesting_depth: usize,
    pub max_power: u32,
    pub max_expanded_terms: usize,
    pub max_expanded_factor_entries: usize,
    pub max_normalization_operations: u64,
    pub max_tensor_factor_occurrences: usize,
    pub max_distinct_indices: usize,
    pub max_distinct_spectators: usize,
    pub max_fresh_dummy_attempts: usize,
    pub max_render_terms: usize,
    pub max_render_factor_entries: usize,
}

impl Default for SymbolicaTensorNumeratorLimits {
    fn default() -> Self {
        Self {
            projector: GenericTensorProjectorLimits::default(),
            max_input_nodes: 100_000,
            max_nesting_depth: 256,
            max_power: 256,
            max_expanded_terms: 100_000,
            max_expanded_factor_entries: 16_000_000,
            max_normalization_operations: 100_000_000,
            max_tensor_factor_occurrences: 16_384,
            max_distinct_indices: 16_384,
            max_distinct_spectators: 16_384,
            max_fresh_dummy_attempts: 16_384,
            max_render_terms: 1_000_000,
            max_render_factor_entries: 64_000_000,
        }
    }
}

/// Symbol heads used at the Atom boundary.
///
/// `loop_vector` and `spectator_vector` use the convention
/// `head(identity arguments..., Lorentz index)` for indexed vectors and
/// `head(identity arguments...)` inside `dot`.  The identity arity is arbitrary
/// but must be nonzero.  `private_dummy_index` is used only to type a
/// loop--spectator dot product; generated atoms are collision-checked against
/// every user index atom.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SymbolicaTensorSyntax {
    pub loop_vector: Symbol,
    pub spectator_vector: Symbol,
    pub metric: Symbol,
    pub dot: Symbol,
    pub private_dummy_index: Symbol,
}

impl SymbolicaTensorSyntax {
    pub const fn new(
        loop_vector: Symbol,
        spectator_vector: Symbol,
        metric: Symbol,
        dot: Symbol,
        private_dummy_index: Symbol,
    ) -> Self {
        Self {
            loop_vector,
            spectator_vector,
            metric,
            dot,
            private_dummy_index,
        }
    }

    /// Register or reuse the conventional Vakint tensor heads.  This is only a
    /// syntax convenience; it does not recognize a topology or dispatch on a
    /// loop count.
    pub fn vakint() -> Result<Self, SymbolicaTensorNumeratorError> {
        let loop_vector = existing_or_plain_symbol("vakint::k")?;
        let spectator_vector = existing_or_plain_symbol("vakint::p")?;
        let metric = if let Some(symbol) = get_symbol!("vakint::g") {
            symbol
        } else {
            try_symbol!("vakint::g"; Symmetric)
                .map_err(|error| SymbolicaTensorNumeratorError::Symbol(error.to_string()))?
        };
        let dot = if let Some(symbol) = get_symbol!("vakint::dot") {
            symbol
        } else {
            try_symbol!("vakint::dot"; Symmetric, Linear)
                .map_err(|error| SymbolicaTensorNumeratorError::Symbol(error.to_string()))?
        };
        let private_dummy_index = existing_or_plain_symbol("rustred::tensor_dummy_index")?;
        Self::validate(Self::new(
            loop_vector,
            spectator_vector,
            metric,
            dot,
            private_dummy_index,
        ))
    }

    fn validate(value: Self) -> Result<Self, SymbolicaTensorNumeratorError> {
        let heads = [
            value.loop_vector,
            value.spectator_vector,
            value.metric,
            value.dot,
            value.private_dummy_index,
        ];
        let distinct = heads.into_iter().collect::<BTreeSet<_>>();
        if distinct.len() != heads.len() {
            return Err(SymbolicaTensorNumeratorError::AliasedSyntaxHeads);
        }
        Ok(value)
    }

    fn is_reserved(self, symbol: Symbol) -> bool {
        symbol == self.loop_vector
            || symbol == self.spectator_vector
            || symbol == self.metric
            || symbol == self.dot
            || symbol == self.private_dummy_index
    }
}

/// Why a stable Lorentz-index ID was allocated.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SymbolicaIndexAllocationOrigin {
    Input,
    LoopSpectatorDot { source_term: usize, factor: usize },
}

/// Replayable assignment of an exact index atom to one typed ID.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SymbolicaIndexAllocation {
    index: LorentzIndex,
    atom: Atom,
    origin: SymbolicaIndexAllocationOrigin,
}

impl SymbolicaIndexAllocation {
    pub const fn index(&self) -> LorentzIndex {
        self.index
    }

    pub const fn atom(&self) -> &Atom {
        &self.atom
    }

    pub const fn origin(&self) -> &SymbolicaIndexAllocationOrigin {
        &self.origin
    }
}

/// Replayable assignment of an exact bare spectator atom to one typed ID.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SymbolicaSpectatorAllocation {
    vector: SpectatorVector,
    atom: Atom,
}

impl SymbolicaSpectatorAllocation {
    pub const fn vector(&self) -> SpectatorVector {
        self.vector
    }

    pub const fn atom(&self) -> &Atom {
        &self.atom
    }
}

/// One source monomial with its scalar Atom weight retained losslessly.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SymbolicaWeightedCovariantTensorMonomial {
    weight: Atom,
    monomial: CovariantTensorMonomial,
}

impl SymbolicaWeightedCovariantTensorMonomial {
    pub const fn weight(&self) -> &Atom {
        &self.weight
    }

    pub const fn monomial(&self) -> &CovariantTensorMonomial {
        &self.monomial
    }
}

/// Auditable census for one compile call.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct SymbolicaTensorCompilationStats {
    pub input_nodes: usize,
    pub expanded_terms: usize,
    pub expanded_factor_entries: usize,
    pub normalization_operations: u64,
    pub tensor_factor_occurrences: usize,
    pub index_allocations: usize,
    pub spectator_allocations: usize,
    pub fresh_dummy_attempts: usize,
}

/// Lossless compilation result and complete identity-allocation transcript.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CompiledSymbolicaTensorNumerator {
    family_fingerprint: String,
    syntax: SymbolicaTensorSyntax,
    source: Atom,
    loop_atoms: Vec<Atom>,
    terms: Vec<SymbolicaWeightedCovariantTensorMonomial>,
    index_allocations: Vec<SymbolicaIndexAllocation>,
    spectator_allocations: Vec<SymbolicaSpectatorAllocation>,
    stats: SymbolicaTensorCompilationStats,
    limits: SymbolicaTensorNumeratorLimits,
}

impl CompiledSymbolicaTensorNumerator {
    pub fn family_fingerprint(&self) -> &str {
        &self.family_fingerprint
    }

    pub const fn syntax(&self) -> SymbolicaTensorSyntax {
        self.syntax
    }

    pub const fn source(&self) -> &Atom {
        &self.source
    }

    pub fn terms(&self) -> &[SymbolicaWeightedCovariantTensorMonomial] {
        &self.terms
    }

    pub fn index_allocations(&self) -> &[SymbolicaIndexAllocation] {
        &self.index_allocations
    }

    pub fn spectator_allocations(&self) -> &[SymbolicaSpectatorAllocation] {
        &self.spectator_allocations
    }

    pub const fn stats(&self) -> SymbolicaTensorCompilationStats {
        self.stats
    }

    pub const fn limits(&self) -> SymbolicaTensorNumeratorLimits {
        self.limits
    }

    pub fn loop_atom(&self, vector: LoopVector) -> Option<&Atom> {
        self.loop_atoms.get(usize::from(vector.id()))
    }

    pub fn index_atom(&self, index: LorentzIndex) -> Option<&Atom> {
        self.index_allocations
            .get(index.id() as usize)
            .filter(|allocation| allocation.index == index)
            .map(SymbolicaIndexAllocation::atom)
    }

    pub fn spectator_atom(&self, vector: SpectatorVector) -> Option<&Atom> {
        self.spectator_allocations
            .get(vector.id() as usize)
            .filter(|allocation| allocation.vector == vector)
            .map(SymbolicaSpectatorAllocation::atom)
    }

    /// Recompile the retained original Atom and compare the complete allocation
    /// transcript, typed sources, and work census.
    pub fn verify_replay(
        &self,
        compiler: &SymbolicaTensorNumeratorCompiler,
    ) -> Result<(), SymbolicaTensorNumeratorError> {
        let replay = compiler.compile(self.source.as_view())?;
        if replay == *self {
            Ok(())
        } else {
            Err(SymbolicaTensorNumeratorError::CompilationReplayMismatch)
        }
    }

    /// Convert weights to the already-declared exact family field.  An opaque
    /// Atom is retained in this object and reported as deferred; this method
    /// never adds a variable to the family coefficient map.
    pub fn try_weighted_sources(
        &self,
        family: &IntegralFamily,
    ) -> Result<Vec<WeightedCovariantTensorMonomial>, SymbolicaTensorNumeratorError> {
        self.check_family(family)?;
        let context = family.coefficient_context();
        let mut output = Vec::with_capacity(self.terms.len());
        for (source_term, source) in self.terms.iter().enumerate() {
            let coefficient = source
                .weight
                .as_view()
                .try_to_rational_polynomial(&Q, &Z, Some(context.variables().clone()))
                .map_err(|_| SymbolicaTensorNumeratorError::DeferredWeight {
                    source_term,
                    weight: source.weight.clone(),
                })?;
            if context.validate(&coefficient).is_err() {
                return Err(SymbolicaTensorNumeratorError::DeferredWeight {
                    source_term,
                    weight: source.weight.clone(),
                });
            }
            output.push(WeightedCovariantTensorMonomial::new(
                coefficient,
                source.monomial.clone(),
            ));
        }
        Ok(output)
    }

    /// Enter the existing authenticated covariant tensor-polynomial stack.
    pub fn project(
        &self,
        family: &IntegralFamily,
        limits: GenericTensorPolynomialLimits,
    ) -> Result<AuthenticatedVacuumCovariantTensorPolynomialProjection, SymbolicaTensorNumeratorError>
    {
        let sources = self.try_weighted_sources(family)?;
        Ok(GenericVacuumTensorPolynomialProjector::with_limits(limits).project(family, sources)?)
    }

    /// Render a projected numerator using the original loop, spectator, and
    /// decorated-index atoms.  Coefficients remain exact Symbolica expressions.
    pub fn render_projected(
        &self,
        numerator: &GenericCovariantTensorNumerator,
    ) -> Result<Atom, SymbolicaTensorNumeratorError> {
        check_limit(
            "rendered tensor terms",
            numerator.terms().len(),
            self.limits.max_render_terms,
        )?;
        let mut factor_entries = 0usize;
        let mut sum = Atom::num(0);
        for term in numerator.terms() {
            let structure_entries = term
                .covariant()
                .metrics()
                .metrics()
                .len()
                .checked_add(term.covariant().spectator_vectors().len())
                .and_then(|value| {
                    value.checked_add(term.covariant().spectator_scalar_products().factors().len())
                })
                .and_then(|value| value.checked_add(term.loop_scalar_products().factors().len()))
                .ok_or(SymbolicaTensorNumeratorError::ResourceCountOverflow {
                    resource: "rendered tensor factor entries",
                })?;
            factor_entries = factor_entries.checked_add(structure_entries).ok_or(
                SymbolicaTensorNumeratorError::ResourceCountOverflow {
                    resource: "rendered tensor factor entries",
                },
            )?;
            check_limit(
                "rendered tensor factor entries",
                factor_entries,
                self.limits.max_render_factor_entries,
            )?;
            let mut product = term.coefficient().to_expression();
            product *= self.render_covariant(term.covariant())?;
            for (coordinate, &exponent) in term.loop_scalar_products().factors() {
                let ScalarProductCoordinate::LoopLoop { left, right } = coordinate else {
                    return Err(
                        SymbolicaTensorNumeratorError::UnsupportedRenderedScalarProduct {
                            coordinate: *coordinate,
                        },
                    );
                };
                let left = self.loop_atoms.get(*left).ok_or(
                    SymbolicaTensorNumeratorError::MissingLoopIdentity { position: *left },
                )?;
                let right = self.loop_atoms.get(*right).ok_or(
                    SymbolicaTensorNumeratorError::MissingLoopIdentity { position: *right },
                )?;
                product *= atom_power(
                    symmetric_binary(self.syntax.dot, left.clone(), right.clone()),
                    exponent,
                );
            }
            sum += product;
        }
        Ok(sum)
    }

    /// Render one remaining Lorentz covariant with the exact user atoms that
    /// were interned during compilation. This is also the comparison boundary
    /// for scalar-reduced terms whose integral key is tracked separately.
    /// Render one covariant independently of its scalar-integral coefficient.
    /// This is the boundary used after a tensor-plus-IBP reduction has grouped
    /// scalar masters by covariant structure.
    pub fn render_covariant(
        &self,
        covariant: &TensorCovariantStructure,
    ) -> Result<Atom, SymbolicaTensorNumeratorError> {
        let factor_entries = covariant
            .metrics()
            .metrics()
            .len()
            .checked_add(covariant.spectator_vectors().len())
            .and_then(|value| {
                value.checked_add(covariant.spectator_scalar_products().factors().len())
            })
            .ok_or(SymbolicaTensorNumeratorError::ResourceCountOverflow {
                resource: "rendered covariant factor entries",
            })?;
        check_limit(
            "rendered covariant factor entries",
            factor_entries,
            self.limits.max_render_factor_entries,
        )?;
        let mut product = Atom::num(1);
        for metric in covariant.metrics().metrics() {
            let left = self.required_index_atom(metric.left())?.clone();
            let right = self.required_index_atom(metric.right())?.clone();
            product *= symmetric_binary(self.syntax.metric, left, right);
        }
        for vector in covariant.spectator_vectors() {
            product *= append_index(
                self.required_spectator_atom(vector.vector())?,
                self.required_index_atom(vector.index())?.clone(),
            )?;
        }
        for (scalar_product, &exponent) in covariant.spectator_scalar_products().factors() {
            let left = self.required_spectator_atom(scalar_product.left())?.clone();
            let right = self
                .required_spectator_atom(scalar_product.right())?
                .clone();
            product *= atom_power(symmetric_binary(self.syntax.dot, left, right), exponent);
        }
        Ok(product)
    }

    fn check_family(&self, family: &IntegralFamily) -> Result<(), SymbolicaTensorNumeratorError> {
        let actual = family.fingerprint();
        if actual == self.family_fingerprint {
            Ok(())
        } else {
            Err(SymbolicaTensorNumeratorError::WrongFamilyFingerprint {
                expected: self.family_fingerprint.clone(),
                actual,
            })
        }
    }

    fn required_index_atom(
        &self,
        index: LorentzIndex,
    ) -> Result<&Atom, SymbolicaTensorNumeratorError> {
        self.index_atom(index)
            .ok_or(SymbolicaTensorNumeratorError::MissingIndexIdentity { index })
    }

    fn required_spectator_atom(
        &self,
        vector: SpectatorVector,
    ) -> Result<&Atom, SymbolicaTensorNumeratorError> {
        self.spectator_atom(vector)
            .ok_or(SymbolicaTensorNumeratorError::MissingSpectatorIdentity { vector })
    }
}

/// Topology-independent compiler configured by one family and exact bare loop
/// vector identities.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SymbolicaTensorNumeratorCompiler {
    family_fingerprint: String,
    family_loop_names: Vec<String>,
    syntax: SymbolicaTensorSyntax,
    loop_atoms: Vec<Atom>,
    loop_by_atom: BTreeMap<Atom, LoopVector>,
    limits: SymbolicaTensorNumeratorLimits,
}

impl SymbolicaTensorNumeratorCompiler {
    /// Build from `(family loop name, bare vector Atom)` entries.  Entries may
    /// appear in any order; matching is exact and simultaneous rather than a
    /// rewrite sequence.
    pub fn try_new(
        family: &IntegralFamily,
        syntax: SymbolicaTensorSyntax,
        loop_name_map: impl IntoIterator<Item = (String, Atom)>,
        limits: SymbolicaTensorNumeratorLimits,
    ) -> Result<Self, SymbolicaTensorNumeratorError> {
        let syntax = SymbolicaTensorSyntax::validate(syntax)?;
        let mut supplied = BTreeMap::<String, Atom>::new();
        for (name, atom) in loop_name_map {
            if supplied.insert(name.clone(), atom).is_some() {
                return Err(SymbolicaTensorNumeratorError::DuplicateLoopName { name });
            }
        }
        if supplied.len() != family.loop_momenta().len() {
            return Err(SymbolicaTensorNumeratorError::LoopMapCardinality {
                expected: family.loop_momenta().len(),
                actual: supplied.len(),
            });
        }
        let mut loop_atoms = Vec::with_capacity(family.loop_momenta().len());
        let mut loop_by_atom = BTreeMap::new();
        for (position, name) in family.loop_momenta().iter().enumerate() {
            let atom = supplied.remove(name).ok_or_else(|| {
                SymbolicaTensorNumeratorError::MissingLoopName { name: name.clone() }
            })?;
            validate_bare_vector(atom.as_view(), syntax.loop_vector, "loop")?;
            let vector = LoopVector::new(u16::try_from(position).map_err(|_| {
                SymbolicaTensorNumeratorError::ResourceLimit {
                    resource: "loop-vector identifier",
                    requested: position as u128,
                    limit: u16::MAX as u128,
                }
            })?);
            if let Some(previous) = loop_by_atom.insert(atom.clone(), vector) {
                return Err(SymbolicaTensorNumeratorError::DuplicateLoopIdentity {
                    first: previous,
                    second: vector,
                    atom,
                });
            }
            loop_atoms.push(atom);
        }
        if let Some((name, _)) = supplied.into_iter().next() {
            return Err(SymbolicaTensorNumeratorError::UnknownLoopName { name });
        }
        Ok(Self {
            family_fingerprint: family.fingerprint(),
            family_loop_names: family.loop_momenta().to_vec(),
            syntax,
            loop_atoms,
            loop_by_atom,
            limits,
        })
    }

    pub const fn syntax(&self) -> SymbolicaTensorSyntax {
        self.syntax
    }

    pub const fn limits(&self) -> SymbolicaTensorNumeratorLimits {
        self.limits
    }

    pub fn loop_atoms(&self) -> &[Atom] {
        &self.loop_atoms
    }

    pub fn compile(
        &self,
        source: AtomView<'_>,
    ) -> Result<CompiledSymbolicaTensorNumerator, SymbolicaTensorNumeratorError> {
        let input_nodes = checked_atom_node_count(source, self.limits.max_input_nodes)?;
        let mut normalization_operations = 0u64;
        let normalized = normalize_polynomial(
            source,
            0,
            self.syntax,
            self.limits,
            &mut normalization_operations,
        )?;
        let expanded_factor_entries = polynomial_factor_entries(&normalized)?;
        check_limit(
            "expanded tensor terms",
            normalized.len(),
            self.limits.max_expanded_terms,
        )?;
        check_limit(
            "expanded tensor factor entries",
            expanded_factor_entries,
            self.limits.max_expanded_factor_entries,
        )?;

        let mut state = DecodeState::new(self);
        // Reserve every explicit user index before allocating private dummy
        // indices for loop--spectator dots.  A one-pass decoder could otherwise
        // pick dummy(0) and only later discover that the same Atom occurs as an
        // explicit index in another factor or summand.
        state.preintern_user_indices(&normalized)?;
        let mut terms = Vec::with_capacity(normalized.len());
        for (source_term, factors) in normalized.iter().enumerate() {
            terms.push(state.decode_term(source_term, factors)?);
        }
        let stats = SymbolicaTensorCompilationStats {
            input_nodes,
            expanded_terms: normalized.len(),
            expanded_factor_entries,
            normalization_operations,
            tensor_factor_occurrences: state.tensor_factor_occurrences,
            index_allocations: state.indices.allocations.len(),
            spectator_allocations: state.spectators.allocations.len(),
            fresh_dummy_attempts: state.indices.fresh_dummy_attempts,
        };
        Ok(CompiledSymbolicaTensorNumerator {
            family_fingerprint: self.family_fingerprint.clone(),
            syntax: self.syntax,
            source: source.to_owned(),
            loop_atoms: self.loop_atoms.clone(),
            terms,
            index_allocations: state.indices.allocations,
            spectator_allocations: state.spectators.allocations,
            stats,
            limits: self.limits,
        })
    }
}

struct DecodeState<'a> {
    compiler: &'a SymbolicaTensorNumeratorCompiler,
    indices: IndexInterner,
    spectators: SpectatorInterner,
    tensor_factor_occurrences: usize,
}

impl<'a> DecodeState<'a> {
    fn new(compiler: &'a SymbolicaTensorNumeratorCompiler) -> Self {
        Self {
            compiler,
            indices: IndexInterner::new(
                compiler.syntax.private_dummy_index,
                compiler.limits.max_distinct_indices,
                compiler.limits.max_fresh_dummy_attempts,
            ),
            spectators: SpectatorInterner::new(compiler.limits.max_distinct_spectators),
            tensor_factor_occurrences: 0,
        }
    }

    fn preintern_user_indices(
        &mut self,
        polynomial: &FactorPolynomial,
    ) -> Result<(), SymbolicaTensorNumeratorError> {
        for factors in polynomial {
            for factor in factors {
                let AtomView::Fun(function) = factor.as_view() else {
                    continue;
                };
                let head = function.get_symbol();
                if (head == self.compiler.syntax.loop_vector
                    || head == self.compiler.syntax.spectator_vector)
                    && function.get_nargs() >= 2
                {
                    self.indices
                        .intern_input(function.get(function.get_nargs() - 1))?;
                } else if head == self.compiler.syntax.metric && function.get_nargs() == 2 {
                    self.indices.intern_input(function.get(0))?;
                    self.indices.intern_input(function.get(1))?;
                }
            }
        }
        Ok(())
    }

    fn decode_term(
        &mut self,
        source_term: usize,
        factors: &[Atom],
    ) -> Result<SymbolicaWeightedCovariantTensorMonomial, SymbolicaTensorNumeratorError> {
        let mut weight = Atom::num(1);
        let mut loop_vectors = Vec::new();
        let mut spectator_vectors = Vec::new();
        let mut metrics = Vec::new();
        let mut loop_scalar_products = ScalarProductMonomial::one();
        let mut spectator_scalar_products = SpectatorScalarProductMonomial::one();
        let mut loop_scalar_degree = 0u64;
        let mut spectator_scalar_degree = 0u64;

        for (factor_position, factor) in factors.iter().enumerate() {
            if let Some((vector, index_atom)) = self.decode_indexed_loop(factor.as_view())? {
                self.charge_tensor_factor()?;
                let index = self.indices.intern_input(index_atom)?;
                loop_vectors.push(IndexedVector::new(vector, index));
                continue;
            }
            if let Some((bare, index_atom)) = self.decode_indexed_spectator(factor.as_view())? {
                self.charge_tensor_factor()?;
                let vector = self.spectators.intern(bare)?;
                let index = self.indices.intern_input(index_atom)?;
                spectator_vectors.push(IndexedSpectatorVector::new(vector, index));
                continue;
            }
            if let Some((left, right)) = self.decode_metric(factor.as_view())? {
                self.charge_tensor_factor()?;
                metrics.push(Metric::new(
                    self.indices.intern_input(left)?,
                    self.indices.intern_input(right)?,
                ));
                continue;
            }
            if let Some((left, right)) = self.decode_dot(factor.as_view())? {
                self.charge_tensor_factor()?;
                match (left, right) {
                    (VectorIdentity::Loop(left), VectorIdentity::Loop(right)) => {
                        loop_scalar_degree = checked_degree(
                            loop_scalar_degree,
                            self.compiler.limits.projector.max_scalar_product_degree,
                            "loop scalar-product degree",
                        )?;
                        loop_scalar_products.try_multiply(ScalarProduct::new(left, right))?;
                    }
                    (VectorIdentity::Spectator(left), VectorIdentity::Spectator(right)) => {
                        spectator_scalar_degree = checked_degree(
                            spectator_scalar_degree,
                            self.compiler
                                .limits
                                .projector
                                .max_spectator_scalar_product_degree,
                            "spectator scalar-product degree",
                        )?;
                        spectator_scalar_products
                            .try_multiply_power(SpectatorScalarProduct::new(left, right), 1)?;
                    }
                    (VectorIdentity::Loop(loop_vector), VectorIdentity::Spectator(spectator))
                    | (VectorIdentity::Spectator(spectator), VectorIdentity::Loop(loop_vector)) => {
                        let index = self.indices.fresh_dummy(source_term, factor_position)?;
                        loop_vectors.push(IndexedVector::new(loop_vector, index));
                        spectator_vectors.push(IndexedSpectatorVector::new(spectator, index));
                    }
                }
                continue;
            }
            if contains_reserved_head(factor.as_view(), self.compiler.syntax) {
                return Err(SymbolicaTensorNumeratorError::UnsupportedReservedFactor {
                    source_term,
                    factor: factor.clone(),
                });
            }
            weight *= factor.clone();
        }

        let monomial = CovariantTensorMonomial::try_from_parts_with_limits(
            loop_vectors,
            spectator_vectors,
            metrics,
            loop_scalar_products,
            spectator_scalar_products,
            self.compiler.limits.projector,
        )?;
        Ok(SymbolicaWeightedCovariantTensorMonomial { weight, monomial })
    }

    fn charge_tensor_factor(&mut self) -> Result<(), SymbolicaTensorNumeratorError> {
        self.tensor_factor_occurrences = self.tensor_factor_occurrences.checked_add(1).ok_or(
            SymbolicaTensorNumeratorError::ResourceCountOverflow {
                resource: "tensor factor occurrences",
            },
        )?;
        check_limit(
            "tensor factor occurrences",
            self.tensor_factor_occurrences,
            self.compiler.limits.max_tensor_factor_occurrences,
        )
    }

    fn decode_indexed_loop<'b>(
        &self,
        atom: AtomView<'b>,
    ) -> Result<Option<(LoopVector, AtomView<'b>)>, SymbolicaTensorNumeratorError> {
        let AtomView::Fun(function) = atom else {
            return Ok(None);
        };
        if function.get_symbol() != self.compiler.syntax.loop_vector {
            return Ok(None);
        }
        if function.get_nargs() < 2 {
            return Ok(None);
        }
        let index = function.get(function.get_nargs() - 1);
        let bare = function_without_last_argument(function);
        if let Some(&vector) = self.compiler.loop_by_atom.get(&bare) {
            Ok(Some((vector, index)))
        } else {
            Err(SymbolicaTensorNumeratorError::UnknownLoopVector { atom: bare })
        }
    }

    fn decode_indexed_spectator<'b>(
        &self,
        atom: AtomView<'b>,
    ) -> Result<Option<(Atom, AtomView<'b>)>, SymbolicaTensorNumeratorError> {
        let AtomView::Fun(function) = atom else {
            return Ok(None);
        };
        if function.get_symbol() != self.compiler.syntax.spectator_vector {
            return Ok(None);
        }
        if function.get_nargs() < 2 {
            return Ok(None);
        }
        let index = function.get(function.get_nargs() - 1);
        Ok(Some((function_without_last_argument(function), index)))
    }

    fn decode_metric<'b>(
        &self,
        atom: AtomView<'b>,
    ) -> Result<Option<(AtomView<'b>, AtomView<'b>)>, SymbolicaTensorNumeratorError> {
        let AtomView::Fun(function) = atom else {
            return Ok(None);
        };
        if function.get_symbol() != self.compiler.syntax.metric {
            return Ok(None);
        }
        if function.get_nargs() != 2 {
            return Err(SymbolicaTensorNumeratorError::MalformedMetric {
                atom: atom.to_owned(),
            });
        }
        Ok(Some((function.get(0), function.get(1))))
    }

    fn decode_dot(
        &mut self,
        atom: AtomView<'_>,
    ) -> Result<Option<(VectorIdentity, VectorIdentity)>, SymbolicaTensorNumeratorError> {
        let AtomView::Fun(function) = atom else {
            return Ok(None);
        };
        if function.get_symbol() != self.compiler.syntax.dot {
            return Ok(None);
        }
        if function.get_nargs() != 2 {
            return Err(SymbolicaTensorNumeratorError::MalformedDot {
                atom: atom.to_owned(),
            });
        }
        let left = self.decode_bare_vector(function.get(0), atom)?;
        let right = self.decode_bare_vector(function.get(1), atom)?;
        Ok(Some((left, right)))
    }

    fn decode_bare_vector(
        &mut self,
        atom: AtomView<'_>,
        full_dot: AtomView<'_>,
    ) -> Result<VectorIdentity, SymbolicaTensorNumeratorError> {
        let owned = atom.to_owned();
        if let Some(&vector) = self.compiler.loop_by_atom.get(&owned) {
            return Ok(VectorIdentity::Loop(vector));
        }
        let AtomView::Fun(function) = atom else {
            return Err(SymbolicaTensorNumeratorError::UnsupportedDotArgument {
                dot: full_dot.to_owned(),
                argument: owned,
            });
        };
        if function.get_symbol() == self.compiler.syntax.loop_vector {
            return Err(SymbolicaTensorNumeratorError::UnknownLoopVector { atom: owned });
        }
        if function.get_symbol() != self.compiler.syntax.spectator_vector
            || function.get_nargs() == 0
        {
            return Err(SymbolicaTensorNumeratorError::UnsupportedDotArgument {
                dot: full_dot.to_owned(),
                argument: owned,
            });
        }
        Ok(VectorIdentity::Spectator(self.spectators.intern(owned)?))
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum VectorIdentity {
    Loop(LoopVector),
    Spectator(SpectatorVector),
}

struct IndexInterner {
    ids: BTreeMap<Atom, LorentzIndex>,
    allocations: Vec<SymbolicaIndexAllocation>,
    private_dummy_index: Symbol,
    max_indices: usize,
    max_fresh_dummy_attempts: usize,
    next_dummy_serial: u64,
    fresh_dummy_attempts: usize,
}

impl IndexInterner {
    fn new(
        private_dummy_index: Symbol,
        max_indices: usize,
        max_fresh_dummy_attempts: usize,
    ) -> Self {
        Self {
            ids: BTreeMap::new(),
            allocations: Vec::new(),
            private_dummy_index,
            max_indices,
            max_fresh_dummy_attempts,
            next_dummy_serial: 0,
            fresh_dummy_attempts: 0,
        }
    }

    fn intern_input(
        &mut self,
        atom: AtomView<'_>,
    ) -> Result<LorentzIndex, SymbolicaTensorNumeratorError> {
        self.intern(atom.to_owned(), SymbolicaIndexAllocationOrigin::Input)
    }

    fn fresh_dummy(
        &mut self,
        source_term: usize,
        factor: usize,
    ) -> Result<LorentzIndex, SymbolicaTensorNumeratorError> {
        loop {
            self.fresh_dummy_attempts = self.fresh_dummy_attempts.checked_add(1).ok_or(
                SymbolicaTensorNumeratorError::ResourceCountOverflow {
                    resource: "fresh dummy attempts",
                },
            )?;
            check_limit(
                "fresh dummy attempts",
                self.fresh_dummy_attempts,
                self.max_fresh_dummy_attempts,
            )?;
            let serial = self.next_dummy_serial;
            self.next_dummy_serial = self.next_dummy_serial.checked_add(1).ok_or(
                SymbolicaTensorNumeratorError::ResourceCountOverflow {
                    resource: "fresh dummy serial",
                },
            )?;
            let atom = FunctionBuilder::new(self.private_dummy_index)
                .add_arg(Atom::num(i64::try_from(serial).map_err(|_| {
                    SymbolicaTensorNumeratorError::ResourceLimit {
                        resource: "fresh dummy serial",
                        requested: u128::from(serial),
                        limit: i64::MAX as u128,
                    }
                })?))
                .finish();
            if self.ids.contains_key(&atom) {
                continue;
            }
            return self.intern(
                atom,
                SymbolicaIndexAllocationOrigin::LoopSpectatorDot {
                    source_term,
                    factor,
                },
            );
        }
    }

    fn intern(
        &mut self,
        atom: Atom,
        origin: SymbolicaIndexAllocationOrigin,
    ) -> Result<LorentzIndex, SymbolicaTensorNumeratorError> {
        if let Some(&index) = self.ids.get(&atom) {
            return Ok(index);
        }
        let requested = self.allocations.len().checked_add(1).ok_or(
            SymbolicaTensorNumeratorError::ResourceCountOverflow {
                resource: "distinct Lorentz indices",
            },
        )?;
        check_limit("distinct Lorentz indices", requested, self.max_indices)?;
        let index = LorentzIndex::new(u32::try_from(self.allocations.len()).map_err(|_| {
            SymbolicaTensorNumeratorError::ResourceLimit {
                resource: "Lorentz-index identifier",
                requested: self.allocations.len() as u128,
                limit: u32::MAX as u128,
            }
        })?);
        self.ids.insert(atom.clone(), index);
        self.allocations.push(SymbolicaIndexAllocation {
            index,
            atom,
            origin,
        });
        Ok(index)
    }
}

struct SpectatorInterner {
    ids: BTreeMap<Atom, SpectatorVector>,
    allocations: Vec<SymbolicaSpectatorAllocation>,
    limit: usize,
}

impl SpectatorInterner {
    fn new(limit: usize) -> Self {
        Self {
            ids: BTreeMap::new(),
            allocations: Vec::new(),
            limit,
        }
    }

    fn intern(&mut self, atom: Atom) -> Result<SpectatorVector, SymbolicaTensorNumeratorError> {
        if let Some(&vector) = self.ids.get(&atom) {
            return Ok(vector);
        }
        let requested = self.allocations.len().checked_add(1).ok_or(
            SymbolicaTensorNumeratorError::ResourceCountOverflow {
                resource: "distinct spectator vectors",
            },
        )?;
        check_limit("distinct spectator vectors", requested, self.limit)?;
        let vector = SpectatorVector::new(u32::try_from(self.allocations.len()).map_err(|_| {
            SymbolicaTensorNumeratorError::ResourceLimit {
                resource: "spectator-vector identifier",
                requested: self.allocations.len() as u128,
                limit: u32::MAX as u128,
            }
        })?);
        self.ids.insert(atom.clone(), vector);
        self.allocations
            .push(SymbolicaSpectatorAllocation { vector, atom });
        Ok(vector)
    }
}

type FactorMonomial = Vec<Atom>;
type FactorPolynomial = Vec<FactorMonomial>;

fn normalize_polynomial(
    atom: AtomView<'_>,
    depth: usize,
    syntax: SymbolicaTensorSyntax,
    limits: SymbolicaTensorNumeratorLimits,
    operations: &mut u64,
) -> Result<FactorPolynomial, SymbolicaTensorNumeratorError> {
    if depth > limits.max_nesting_depth {
        return Err(SymbolicaTensorNumeratorError::ResourceLimit {
            resource: "tensor Atom nesting depth",
            requested: depth as u128,
            limit: limits.max_nesting_depth as u128,
        });
    }
    charge_work(operations, 1, limits.max_normalization_operations)?;
    match atom {
        AtomView::Add(sum) => {
            let mut output = Vec::new();
            let mut factor_entries = 0usize;
            for child in sum.iter() {
                let child = normalize_polynomial(child, depth + 1, syntax, limits, operations)?;
                let attempted = output.len().checked_add(child.len()).ok_or(
                    SymbolicaTensorNumeratorError::ResourceCountOverflow {
                        resource: "expanded tensor terms",
                    },
                )?;
                check_limit(
                    "expanded tensor terms",
                    attempted,
                    limits.max_expanded_terms,
                )?;
                factor_entries = factor_entries
                    .checked_add(polynomial_factor_entries(&child)?)
                    .ok_or(SymbolicaTensorNumeratorError::ResourceCountOverflow {
                        resource: "expanded tensor factor entries",
                    })?;
                check_limit(
                    "expanded tensor factor entries",
                    factor_entries,
                    limits.max_expanded_factor_entries,
                )?;
                output.extend(child);
            }
            Ok(output)
        }
        AtomView::Mul(product) => {
            let mut output = vec![Vec::new()];
            for child in product.iter() {
                let child = normalize_polynomial(child, depth + 1, syntax, limits, operations)?;
                output = multiply_polynomials(output, &child, limits, operations)?;
            }
            Ok(output)
        }
        AtomView::Pow(power) if contains_reserved_head(power.get_base(), syntax) => {
            let exponent = i64::try_from(power.get_exp()).map_err(|_| {
                SymbolicaTensorNumeratorError::UnsupportedTensorPower {
                    atom: atom.to_owned(),
                }
            })?;
            let exponent = u32::try_from(exponent).map_err(|_| {
                SymbolicaTensorNumeratorError::UnsupportedTensorPower {
                    atom: atom.to_owned(),
                }
            })?;
            if exponent > limits.max_power {
                return Err(SymbolicaTensorNumeratorError::ResourceLimit {
                    resource: "tensor power",
                    requested: u128::from(exponent),
                    limit: u128::from(limits.max_power),
                });
            }
            let base =
                normalize_polynomial(power.get_base(), depth + 1, syntax, limits, operations)?;
            let mut output = vec![Vec::new()];
            for _ in 0..exponent {
                output = multiply_polynomials(output, &base, limits, operations)?;
            }
            Ok(output)
        }
        _ => Ok(vec![vec![atom.to_owned()]]),
    }
}

fn multiply_polynomials(
    left: FactorPolynomial,
    right: &FactorPolynomial,
    limits: SymbolicaTensorNumeratorLimits,
    operations: &mut u64,
) -> Result<FactorPolynomial, SymbolicaTensorNumeratorError> {
    let terms = left.len().checked_mul(right.len()).ok_or(
        SymbolicaTensorNumeratorError::ResourceCountOverflow {
            resource: "expanded tensor terms",
        },
    )?;
    check_limit("expanded tensor terms", terms, limits.max_expanded_terms)?;
    let mut factor_entries = 0usize;
    for left_term in &left {
        for right_term in right {
            let entries = left_term.len().checked_add(right_term.len()).ok_or(
                SymbolicaTensorNumeratorError::ResourceCountOverflow {
                    resource: "expanded tensor factor entries",
                },
            )?;
            factor_entries = factor_entries.checked_add(entries).ok_or(
                SymbolicaTensorNumeratorError::ResourceCountOverflow {
                    resource: "expanded tensor factor entries",
                },
            )?;
            // Check structural size before charging/copying this candidate so
            // the more specific expansion bound wins deterministically and no
            // over-limit output allocation is attempted.
            check_limit(
                "expanded tensor factor entries",
                factor_entries,
                limits.max_expanded_factor_entries,
            )?;
            let work = u64::try_from(entries)
                .map_err(|_| SymbolicaTensorNumeratorError::ResourceCountOverflow {
                    resource: "tensor normalization operations",
                })?
                .checked_add(1)
                .ok_or(SymbolicaTensorNumeratorError::ResourceCountOverflow {
                    resource: "tensor normalization operations",
                })?;
            charge_work(operations, work, limits.max_normalization_operations)?;
        }
    }
    let mut output = Vec::with_capacity(terms);
    for left_term in left {
        for right_term in right {
            let mut product = Vec::with_capacity(left_term.len() + right_term.len());
            product.extend(left_term.iter().cloned());
            product.extend(right_term.iter().cloned());
            output.push(product);
        }
    }
    Ok(output)
}

fn checked_atom_node_count(
    atom: AtomView<'_>,
    limit: usize,
) -> Result<usize, SymbolicaTensorNumeratorError> {
    let mut count = 0usize;
    let mut pending = vec![atom];
    while let Some(current) = pending.pop() {
        count =
            count
                .checked_add(1)
                .ok_or(SymbolicaTensorNumeratorError::ResourceCountOverflow {
                    resource: "input Atom nodes",
                })?;
        check_limit("input Atom nodes", count, limit)?;
        match current {
            AtomView::Fun(function) => pending.extend(function.iter()),
            AtomView::Pow(power) => pending.extend(power.iter()),
            AtomView::Mul(product) => pending.extend(product.iter()),
            AtomView::Add(sum) => pending.extend(sum.iter()),
            AtomView::Num(_) | AtomView::Var(_) => {}
        }
    }
    Ok(count)
}

fn polynomial_factor_entries(
    polynomial: &FactorPolynomial,
) -> Result<usize, SymbolicaTensorNumeratorError> {
    polynomial.iter().try_fold(0usize, |total, term| {
        total
            .checked_add(term.len())
            .ok_or(SymbolicaTensorNumeratorError::ResourceCountOverflow {
                resource: "expanded tensor factor entries",
            })
    })
}

fn contains_reserved_head(atom: AtomView<'_>, syntax: SymbolicaTensorSyntax) -> bool {
    let mut pending = vec![atom];
    while let Some(current) = pending.pop() {
        match current {
            AtomView::Var(variable) if syntax.is_reserved(variable.get_symbol()) => return true,
            AtomView::Fun(function) => {
                if syntax.is_reserved(function.get_symbol()) {
                    return true;
                }
                pending.extend(function.iter());
            }
            AtomView::Pow(power) => pending.extend(power.iter()),
            AtomView::Mul(product) => pending.extend(product.iter()),
            AtomView::Add(sum) => pending.extend(sum.iter()),
            AtomView::Num(_) | AtomView::Var(_) => {}
        }
    }
    false
}

fn validate_bare_vector(
    atom: AtomView<'_>,
    expected_head: Symbol,
    kind: &'static str,
) -> Result<(), SymbolicaTensorNumeratorError> {
    let AtomView::Fun(function) = atom else {
        return Err(SymbolicaTensorNumeratorError::MalformedBareVector {
            kind,
            atom: atom.to_owned(),
        });
    };
    if function.get_symbol() != expected_head || function.get_nargs() == 0 {
        return Err(SymbolicaTensorNumeratorError::MalformedBareVector {
            kind,
            atom: atom.to_owned(),
        });
    }
    Ok(())
}

fn function_without_last_argument(function: FunView<'_>) -> Atom {
    FunctionBuilder::new(function.get_symbol())
        .add_args(function.iter().take(function.get_nargs() - 1))
        .finish()
}

fn append_index(bare: &Atom, index: Atom) -> Result<Atom, SymbolicaTensorNumeratorError> {
    let AtomView::Fun(function) = bare.as_view() else {
        return Err(SymbolicaTensorNumeratorError::MalformedBareVector {
            kind: "spectator",
            atom: bare.clone(),
        });
    };
    Ok(FunctionBuilder::new(function.get_symbol())
        .add_args(function.iter())
        .add_arg(index)
        .finish())
}

fn symmetric_binary(symbol: Symbol, mut left: Atom, mut right: Atom) -> Atom {
    if right < left {
        std::mem::swap(&mut left, &mut right);
    }
    FunctionBuilder::new(symbol)
        .add_args([left, right])
        .finish()
}

fn atom_power(atom: Atom, exponent: u32) -> Atom {
    match exponent {
        0 => Atom::num(1),
        1 => atom,
        exponent => atom.pow(Atom::num(i64::from(exponent))),
    }
}

fn checked_degree(
    current: u64,
    limit: u64,
    resource: &'static str,
) -> Result<u64, SymbolicaTensorNumeratorError> {
    let requested = current
        .checked_add(1)
        .ok_or(SymbolicaTensorNumeratorError::ResourceCountOverflow { resource })?;
    if requested > limit {
        Err(SymbolicaTensorNumeratorError::ResourceLimit {
            resource,
            requested: u128::from(requested),
            limit: u128::from(limit),
        })
    } else {
        Ok(requested)
    }
}

fn charge_work(
    operations: &mut u64,
    amount: u64,
    limit: u64,
) -> Result<(), SymbolicaTensorNumeratorError> {
    let requested = operations.checked_add(amount).ok_or(
        SymbolicaTensorNumeratorError::ResourceCountOverflow {
            resource: "tensor normalization operations",
        },
    )?;
    if requested > limit {
        Err(SymbolicaTensorNumeratorError::WorkLimit {
            resource: "tensor normalization operations",
            requested,
            limit,
        })
    } else {
        *operations = requested;
        Ok(())
    }
}

fn check_limit(
    resource: &'static str,
    requested: usize,
    limit: usize,
) -> Result<(), SymbolicaTensorNumeratorError> {
    if requested > limit {
        Err(SymbolicaTensorNumeratorError::ResourceLimit {
            resource,
            requested: requested as u128,
            limit: limit as u128,
        })
    } else {
        Ok(())
    }
}

fn existing_or_plain_symbol(name: &str) -> Result<Symbol, SymbolicaTensorNumeratorError> {
    let namespaced = NamespacedSymbol::try_parse(name)
        .ok_or_else(|| SymbolicaTensorNumeratorError::Symbol(name.to_owned()))?;
    if let Some(symbol) = Symbol::get_symbol(namespaced.clone()) {
        return Ok(symbol);
    }
    SymbolBuilder::new(namespaced)
        .build()
        .map_err(|error| SymbolicaTensorNumeratorError::Symbol(error.to_string()))
}

/// Typed failures at the Symbolica tensor boundary.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SymbolicaTensorNumeratorError {
    Symbol(String),
    AliasedSyntaxHeads,
    LoopMapCardinality {
        expected: usize,
        actual: usize,
    },
    DuplicateLoopName {
        name: String,
    },
    MissingLoopName {
        name: String,
    },
    UnknownLoopName {
        name: String,
    },
    DuplicateLoopIdentity {
        first: LoopVector,
        second: LoopVector,
        atom: Atom,
    },
    MalformedBareVector {
        kind: &'static str,
        atom: Atom,
    },
    UnknownLoopVector {
        atom: Atom,
    },
    MalformedMetric {
        atom: Atom,
    },
    MalformedDot {
        atom: Atom,
    },
    UnsupportedDotArgument {
        dot: Atom,
        argument: Atom,
    },
    UnsupportedTensorPower {
        atom: Atom,
    },
    UnsupportedReservedFactor {
        source_term: usize,
        factor: Atom,
    },
    DeferredWeight {
        source_term: usize,
        weight: Atom,
    },
    WrongFamilyFingerprint {
        expected: String,
        actual: String,
    },
    MissingLoopIdentity {
        position: usize,
    },
    MissingIndexIdentity {
        index: LorentzIndex,
    },
    MissingSpectatorIdentity {
        vector: SpectatorVector,
    },
    UnsupportedRenderedScalarProduct {
        coordinate: ScalarProductCoordinate,
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
    Tensor(TensorError),
    Projector(GenericTensorProjectorError),
    Polynomial(GenericTensorPolynomialError),
}

impl fmt::Display for SymbolicaTensorNumeratorError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Symbol(error) => write!(formatter, "cannot register tensor symbol: {error}"),
            Self::AliasedSyntaxHeads => {
                formatter.write_str("tensor syntax heads must be pairwise distinct")
            }
            Self::LoopMapCardinality { expected, actual } => write!(
                formatter,
                "tensor loop map has {actual} entries, expected {expected}"
            ),
            Self::DuplicateLoopName { name } => {
                write!(formatter, "tensor loop map repeats family name {name:?}")
            }
            Self::MissingLoopName { name } => {
                write!(formatter, "tensor loop map is missing family name {name:?}")
            }
            Self::UnknownLoopName { name } => {
                write!(
                    formatter,
                    "tensor loop map contains unknown family name {name:?}"
                )
            }
            Self::DuplicateLoopIdentity {
                first,
                second,
                atom,
            } => write!(
                formatter,
                "bare loop Atom {atom} maps to both loop IDs {} and {}",
                first.id(),
                second.id()
            ),
            Self::MalformedBareVector { kind, atom } => {
                write!(formatter, "malformed bare {kind} vector Atom {atom}")
            }
            Self::UnknownLoopVector { atom } => {
                write!(
                    formatter,
                    "indexed or dotted loop vector {atom} is absent from the supplied map"
                )
            }
            Self::MalformedMetric { atom } => {
                write!(formatter, "metric must have exactly two indices: {atom}")
            }
            Self::MalformedDot { atom } => {
                write!(formatter, "dot must have exactly two bare vectors: {atom}")
            }
            Self::UnsupportedDotArgument { dot, argument } => write!(
                formatter,
                "dot argument {argument} in {dot} is neither a mapped loop nor a bare spectator vector"
            ),
            Self::UnsupportedTensorPower { atom } => write!(
                formatter,
                "tensor-containing power must have a bounded nonnegative integer exponent: {atom}"
            ),
            Self::UnsupportedReservedFactor {
                source_term,
                factor,
            } => write!(
                formatter,
                "source term {source_term} contains reserved tensor syntax in unsupported factor {factor}"
            ),
            Self::DeferredWeight {
                source_term,
                weight,
            } => write!(
                formatter,
                "source term {source_term} has opaque scalar weight {weight}; it is retained but is not in the family coefficient field"
            ),
            Self::WrongFamilyFingerprint { expected, actual } => write!(
                formatter,
                "compiled numerator belongs to family {expected:?}, not {actual:?}"
            ),
            Self::MissingLoopIdentity { position } => {
                write!(
                    formatter,
                    "no Atom identity is retained for loop position {position}"
                )
            }
            Self::MissingIndexIdentity { index } => {
                write!(
                    formatter,
                    "no Atom identity is retained for Lorentz index {}",
                    index.id()
                )
            }
            Self::MissingSpectatorIdentity { vector } => write!(
                formatter,
                "no Atom identity is retained for spectator vector {}",
                vector.id()
            ),
            Self::UnsupportedRenderedScalarProduct { coordinate } => write!(
                formatter,
                "vacuum covariant renderer cannot emit scalar-product coordinate {coordinate:?}"
            ),
            Self::CompilationReplayMismatch => formatter
                .write_str("recompiled tensor numerator differs from its retained transcript"),
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
            Self::Tensor(error) => error.fmt(formatter),
            Self::Projector(error) => error.fmt(formatter),
            Self::Polynomial(error) => error.fmt(formatter),
        }
    }
}

impl Error for SymbolicaTensorNumeratorError {}

impl From<TensorError> for SymbolicaTensorNumeratorError {
    fn from(value: TensorError) -> Self {
        Self::Tensor(value)
    }
}

impl From<GenericTensorProjectorError> for SymbolicaTensorNumeratorError {
    fn from(value: GenericTensorProjectorError) -> Self {
        Self::Projector(value)
    }
}

impl From<GenericTensorPolynomialError> for SymbolicaTensorNumeratorError {
    fn from(value: GenericTensorPolynomialError) -> Self {
        Self::Polynomial(value)
    }
}
