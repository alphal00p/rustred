//! Narrow, FORM-free adapter for Vakint's canonical two-loop Atom syntax.
//!
//! This module deliberately does not depend on the `vakint` crate.  It accepts
//! the compatible Symbolica representation
//!
//! ```text
//! numerator * vakint::topo(vakint::I2L(mass_squared, a1, a2, a3))
//! ```
//!
//! and connects it to RustRed's native tensor projector, scalar-product
//! lowering, and integrated two-loop reduction.  The accepted numerator
//! language is intentionally small: indexed loop vectors `k(1,index)` and
//! `k(2,index)`, metrics `g(index,index)`, and scalar products
//! `dot(k(1),k(2))`.  Other factors are preserved verbatim only when they do
//! not contain one of these reserved heads.

use std::collections::BTreeMap;
use std::error::Error;
use std::fmt;

use crate::{TwoLoopPipelineError, TwoLoopReductionConfig, TwoLoopReductionPipeline};
use rustred::legacy_oracle_support::symbolica_atom::{
    Atom, AtomCore, AtomView, FunctionBuilder, Symbol, get_symbol, try_parse, try_symbol,
};
use rustred::{
    IndexedVector, Integral, LoopVector, LorentzIndex, Metric, MetricPairing, ScalarProduct,
    ScalarProductMonomial, TensorConstructionLimits, TensorError, TensorFamilyError,
    TensorFamilyReducer, TensorMonomial, VacuumTensorProjector,
};

/// Resource bounds for parsing and reducing one Vakint expression.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct VakintAdapterLimits {
    /// Maximum number of Atom nodes inspected before controlled distribution.
    pub max_input_nodes: usize,
    /// Maximum number of monomials produced by distributing explicit sums.
    pub max_expanded_terms: usize,
    /// Maximum number of recognized tensor/scalar-product factor occurrences.
    pub max_tensor_factors: usize,
    /// Maximum number of indexed loop vectors passed to the projector.
    pub max_tensor_rank: usize,
    /// Maximum number of distinct, structurally compared Lorentz-index Atoms.
    pub max_distinct_indices: usize,
    /// Maximum degree in explicit `dot(k(_),k(_))` factors.
    pub max_scalar_product_degree: u32,
    /// Maximum number of perfect matchings allowed in the tensor projector.
    pub max_projector_pairings: usize,
    /// Maximum denominator monomials generated while lowering one tensor term.
    pub max_lowered_terms: usize,
    /// Maximum bounded polynomial-expansion operations during lowering.
    pub max_lowering_operations: u64,
    /// Maximum number of master summands emitted for one input expression.
    pub max_output_terms: usize,
}

impl Default for VakintAdapterLimits {
    fn default() -> Self {
        Self {
            max_input_nodes: 100_000,
            max_expanded_terms: 10_000,
            max_tensor_factors: 256,
            // RustRed's default projector has 105 pairings, i.e. rank eight.
            max_tensor_rank: 8,
            max_distinct_indices: 256,
            max_scalar_product_degree: 64,
            max_projector_pairings: 105,
            max_lowered_terms: 1_000_000,
            max_lowering_operations: 10_000_000,
            max_output_terms: 1_000_000,
        }
    }
}

/// Symbol heads used by the compatible Vakint syntax.
///
/// Existing symbols are reused without mutation.  When this adapter registers
/// the symbols first, `g` and `dot` receive the same symmetric/linear
/// attributes as Vakint so a later Vakint initialization sees no conflict.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct VakintAtomSyntax {
    pub topo: Symbol,
    pub i2l: Symbol,
    pub loop_momentum: Symbol,
    pub external_momentum: Symbol,
    pub metric: Symbol,
    pub dot: Symbol,
    pub vkdot: Symbol,
    pub dot_pow: Symbol,
}

impl VakintAtomSyntax {
    pub fn new() -> Result<Self, VakintAdapterError> {
        Ok(Self {
            topo: parse_symbol("vakint::topo")?,
            i2l: parse_symbol("vakint::I2L")?,
            loop_momentum: parse_symbol("vakint::k")?,
            external_momentum: parse_symbol("vakint::p")?,
            metric: vakint_metric_symbol()?,
            dot: vakint_dot_symbol()?,
            vkdot: vakint_vkdot_symbol()?,
            dot_pow: parse_symbol("vakint::dot_pow")?,
        })
    }

    fn is_reserved(self, symbol: Symbol) -> bool {
        symbol == self.topo
            || symbol == self.i2l
            || symbol == self.loop_momentum
            || symbol == self.external_momentum
            || symbol == self.metric
            || symbol == self.dot
            || symbol == self.vkdot
            || symbol == self.dot_pow
    }
}

/// One safely decoded monomial before tensor reduction.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct VakintTwoLoopTerm {
    spectator: Atom,
    mass_squared: Atom,
    integral: Integral,
    tensor: TensorMonomial,
    index_atoms: Vec<Atom>,
}

impl VakintTwoLoopTerm {
    /// Product of all accepted non-Vakint factors, retained exactly.
    pub fn spectator(&self) -> &Atom {
        &self.spectator
    }

    pub fn mass_squared(&self) -> &Atom {
        &self.mass_squared
    }

    pub fn integral(&self) -> &Integral {
        &self.integral
    }

    pub fn tensor(&self) -> &TensorMonomial {
        &self.tensor
    }

    /// Exact input Atom assigned to an adapter-local Lorentz-index ID.
    pub fn index_atom(&self, index: LorentzIndex) -> Option<&Atom> {
        self.index_atoms.get(index.id() as usize)
    }

    pub fn index_atoms(&self) -> &[Atom] {
        &self.index_atoms
    }
}

/// Pure-Rust bridge from a restricted Vakint Atom to the two RustRed masters.
#[derive(Debug)]
pub struct VakintTwoLoopAdapter {
    pipeline: TwoLoopReductionPipeline,
    projector: VacuumTensorProjector,
    syntax: VakintAtomSyntax,
    rustred_mass_squared: Symbol,
    limits: VakintAdapterLimits,
}

impl VakintTwoLoopAdapter {
    /// Build the integrated scalar table as part of adapter construction.
    pub fn build(
        reduction: TwoLoopReductionConfig,
        limits: VakintAdapterLimits,
    ) -> Result<Self, VakintAdapterError> {
        let pipeline = TwoLoopReductionPipeline::build(reduction)?;
        Self::from_pipeline(pipeline, limits)
    }

    /// Attach the adapter to an already constructed two-loop pipeline.
    pub fn from_pipeline(
        pipeline: TwoLoopReductionPipeline,
        limits: VakintAdapterLimits,
    ) -> Result<Self, VakintAdapterError> {
        validate_limits(limits)?;
        let syntax = VakintAtomSyntax::new()?;
        let rustred_mass_squared = parse_symbol("rustred::m2")?;
        let projector = VacuumTensorProjector::with_dimension(
            pipeline.family().coefficients(),
            pipeline.family().dimension().clone(),
        )
        .with_max_pairings(limits.max_projector_pairings);
        Ok(Self {
            pipeline,
            projector,
            syntax,
            rustred_mass_squared,
            limits,
        })
    }

    pub fn pipeline(&self) -> &TwoLoopReductionPipeline {
        &self.pipeline
    }

    pub fn syntax(&self) -> VakintAtomSyntax {
        self.syntax
    }

    pub fn limits(&self) -> VakintAdapterLimits {
        self.limits
    }

    /// Parse text using Vakint as the default namespace.
    pub fn parse(&self, input: &str) -> Result<Atom, VakintAdapterError> {
        try_parse!(input, default_namespace = "vakint")
            .map_err(|message| VakintAdapterError::Parse(message.to_string()))
    }

    /// Decode an expression into a bounded list of tensor-integral monomials.
    ///
    /// Distribution is performed by a small local Cartesian-product walker;
    /// it never invokes an unbounded global `expand()`.
    pub fn decode(
        &self,
        input: AtomView<'_>,
    ) -> Result<Vec<VakintTwoLoopTerm>, VakintAdapterError> {
        let nodes = atom_node_count(input);
        check_limit("input Atom nodes", nodes, self.limits.max_input_nodes)?;
        let monomials = controlled_distribute(input, self.limits.max_expanded_terms)?;
        monomials
            .iter()
            .map(|monomial| self.decode_monomial(monomial.as_view()))
            .collect()
    }

    /// Run projection, family lowering, and integrated two-loop reduction.
    pub fn reduce_atom(&mut self, input: AtomView<'_>) -> Result<Atom, VakintAdapterError> {
        let decoded = self.decode(input)?;
        self.reduce_terms(&decoded)
    }

    /// Reduce a single already-decoded monomial.
    ///
    /// This is useful for callers that need to inspect or transform the exact
    /// spectator/index Atom map between parsing and integration.
    pub fn reduce_term(&mut self, term: &VakintTwoLoopTerm) -> Result<Atom, VakintAdapterError> {
        self.reduce_terms(std::slice::from_ref(term))
    }

    fn reduce_terms(&mut self, decoded: &[VakintTwoLoopTerm]) -> Result<Atom, VakintAdapterError> {
        let mut output = Atom::num(0);
        let mut output_terms = 0usize;
        for term in decoded {
            let projected = self.projector.reduce(term.tensor())?;
            let lowered = TensorFamilyReducer::new(self.pipeline.family())
                .with_max_expansion_terms(self.limits.max_lowered_terms)
                .with_max_expansion_operations(self.limits.max_lowering_operations)
                .lower(term.integral(), &projected)?;

            for (metrics, scalar_combination) in lowered.structures() {
                let reduced = self.pipeline.reduce_combination(scalar_combination)?;
                for (master, coefficient) in reduced.terms() {
                    output_terms =
                        output_terms
                            .checked_add(1)
                            .ok_or(VakintAdapterError::ResourceLimit {
                                resource: "output master summands",
                                requested: u128::MAX,
                                limit: self.limits.max_output_terms as u128,
                            })?;
                    check_limit(
                        "output master summands",
                        output_terms,
                        self.limits.max_output_terms,
                    )?;

                    let coefficient = self
                        .substitute_mass(coefficient.to_expression(), term.mass_squared.as_view());
                    let metrics = self.render_metrics(metrics, term)?;
                    let topology = self.render_master(term.mass_squared(), master);
                    output += term.spectator.clone() * coefficient * metrics * topology;
                }
            }
        }
        Ok(output)
    }

    /// Parse Vakint-compatible text and reduce it in one call.
    pub fn reduce_str(&mut self, input: &str) -> Result<Atom, VakintAdapterError> {
        let parsed = self.parse(input)?;
        self.reduce_atom(parsed.as_view())
    }

    fn decode_monomial(
        &self,
        monomial: AtomView<'_>,
    ) -> Result<VakintTwoLoopTerm, VakintAdapterError> {
        let mut topology = None;
        let mut spectator = Atom::num(1);
        let mut interner = IndexInterner::new(self.limits.max_distinct_indices);
        let mut vectors = Vec::new();
        let mut metrics = Vec::new();
        let mut scalar_products = ScalarProductMonomial::one();
        let mut tensor_factors = 0usize;
        let mut scalar_product_degree = 0u32;

        let factors: Vec<_> = match monomial {
            AtomView::Mul(product) => product.iter().collect(),
            _ => vec![monomial],
        };
        for factor in factors {
            if let Some(decoded_topology) = self.decode_topology(factor)? {
                if topology.replace(decoded_topology).is_some() {
                    return Err(VakintAdapterError::MultipleTopologies);
                }
                continue;
            }

            // Inspect the base first.  Powers of ordinary spectator factors
            // (for example a coupling inverse) are opaque and must remain
            // untouched; only recognized tensor factors require a bounded,
            // nonnegative integer exponent.
            let base = match factor {
                AtomView::Pow(power) => power.get_base(),
                _ => factor,
            };
            if let Some((loop_vector, index)) = self.decode_indexed_vector(base, &mut interner)? {
                let exponent = recognized_nonnegative_power(factor)?;
                let exponent =
                    usize::try_from(exponent).map_err(|_| VakintAdapterError::ResourceLimit {
                        resource: "indexed-vector power",
                        requested: exponent as u128,
                        limit: usize::MAX as u128,
                    })?;
                tensor_factors = checked_resource_add(
                    "tensor factor occurrences",
                    tensor_factors,
                    exponent,
                    self.limits.max_tensor_factors,
                )?;
                let rank = vectors.len().checked_add(exponent).ok_or(
                    VakintAdapterError::ResourceLimit {
                        resource: "tensor rank",
                        requested: u128::MAX,
                        limit: self.limits.max_tensor_rank as u128,
                    },
                )?;
                check_limit("tensor rank", rank, self.limits.max_tensor_rank)?;
                vectors.extend(std::iter::repeat_n(
                    IndexedVector::new(loop_vector, index),
                    exponent,
                ));
                continue;
            }
            if let Some(metric) = self.decode_metric(base, &mut interner)? {
                let exponent = recognized_nonnegative_power(factor)?;
                let exponent =
                    usize::try_from(exponent).map_err(|_| VakintAdapterError::ResourceLimit {
                        resource: "metric power",
                        requested: exponent as u128,
                        limit: usize::MAX as u128,
                    })?;
                tensor_factors = checked_resource_add(
                    "tensor factor occurrences",
                    tensor_factors,
                    exponent,
                    self.limits.max_tensor_factors,
                )?;
                metrics.extend(std::iter::repeat_n(metric, exponent));
                continue;
            }
            if let Some(scalar_product) = self.decode_scalar_product(base)? {
                let exponent = recognized_nonnegative_power(factor)?;
                tensor_factors = checked_resource_add(
                    "tensor factor occurrences",
                    tensor_factors,
                    usize::try_from(exponent).unwrap_or(usize::MAX),
                    self.limits.max_tensor_factors,
                )?;
                scalar_product_degree = scalar_product_degree.checked_add(exponent).ok_or(
                    VakintAdapterError::ResourceLimit {
                        resource: "scalar-product degree",
                        requested: u128::MAX,
                        limit: self.limits.max_scalar_product_degree as u128,
                    },
                )?;
                if scalar_product_degree > self.limits.max_scalar_product_degree {
                    return Err(VakintAdapterError::ResourceLimit {
                        resource: "scalar-product degree",
                        requested: scalar_product_degree as u128,
                        limit: self.limits.max_scalar_product_degree as u128,
                    });
                }
                scalar_products.try_multiply_power(scalar_product, exponent)?;
                continue;
            }

            if contains_reserved_head(factor, self.syntax) {
                return Err(VakintAdapterError::UnsupportedReservedFactor(
                    factor.to_canonical_string(),
                ));
            }
            spectator *= factor.to_owned();
        }

        let (mass_squared, integral) = topology.ok_or(VakintAdapterError::MissingTopology)?;
        if contains_reserved_head(mass_squared.as_view(), self.syntax) {
            return Err(VakintAdapterError::UnsupportedMass(
                mass_squared.to_canonical_string(),
            ));
        }
        let construction_limits = TensorConstructionLimits {
            max_vectors: self.limits.max_tensor_factors,
            max_metrics: self.limits.max_tensor_factors,
            max_scalar_product_factor_entries: self.limits.max_tensor_factors,
            max_distinct_scalar_products: self.limits.max_tensor_factors,
            max_scalar_product_degree: u64::from(self.limits.max_scalar_product_degree),
            max_index_endpoints: self.limits.max_tensor_factors.saturating_mul(2),
        };
        let tensor = TensorMonomial::try_from_parts_with_limits(
            vectors,
            metrics,
            scalar_products,
            construction_limits,
        )?;
        Ok(VakintTwoLoopTerm {
            spectator,
            mass_squared,
            integral,
            tensor,
            index_atoms: interner.into_atoms(),
        })
    }

    fn decode_topology(
        &self,
        factor: AtomView<'_>,
    ) -> Result<Option<(Atom, Integral)>, VakintAdapterError> {
        let AtomView::Fun(topo) = factor else {
            return Ok(None);
        };
        if topo.get_symbol() != self.syntax.topo {
            return Ok(None);
        }
        if topo.get_nargs() != 1 {
            return Err(VakintAdapterError::MalformedTopology(
                factor.to_canonical_string(),
            ));
        }
        let AtomView::Fun(i2l) = topo.get(0) else {
            return Err(VakintAdapterError::MalformedTopology(
                factor.to_canonical_string(),
            ));
        };
        if i2l.get_symbol() != self.syntax.i2l || i2l.get_nargs() != 4 {
            return Err(VakintAdapterError::MalformedTopology(
                factor.to_canonical_string(),
            ));
        }
        let mass_squared = i2l.get(0).to_owned();
        let mut powers = Vec::with_capacity(3);
        for position in 1..4 {
            let power = i64::try_from(i2l.get(position)).map_err(|_| {
                VakintAdapterError::NonIntegerPropagatorPower {
                    position,
                    value: i2l.get(position).to_canonical_string(),
                }
            })?;
            powers.push(
                i32::try_from(power).map_err(|_| {
                    VakintAdapterError::PropagatorPowerOutOfRange { position, power }
                })?,
            );
        }
        Ok(Some((mass_squared, Integral::new(powers))))
    }

    fn decode_indexed_vector(
        &self,
        atom: AtomView<'_>,
        interner: &mut IndexInterner,
    ) -> Result<Option<(LoopVector, LorentzIndex)>, VakintAdapterError> {
        let AtomView::Fun(function) = atom else {
            return Ok(None);
        };
        if function.get_symbol() != self.syntax.loop_momentum || function.get_nargs() != 2 {
            return Ok(None);
        }
        let loop_vector = decode_loop_vector(function.get(0), atom)?;
        let index_atom = function.get(1);
        if contains_reserved_head(index_atom, self.syntax) {
            return Err(VakintAdapterError::UnsupportedIndex(
                index_atom.to_canonical_string(),
            ));
        }
        let index = interner.intern(index_atom)?;
        Ok(Some((loop_vector, index)))
    }

    fn decode_metric(
        &self,
        atom: AtomView<'_>,
        interner: &mut IndexInterner,
    ) -> Result<Option<Metric>, VakintAdapterError> {
        let AtomView::Fun(function) = atom else {
            return Ok(None);
        };
        if function.get_symbol() != self.syntax.metric || function.get_nargs() != 2 {
            return Ok(None);
        }
        let left_atom = function.get(0);
        let right_atom = function.get(1);
        for index_atom in [left_atom, right_atom] {
            if contains_reserved_head(index_atom, self.syntax) {
                return Err(VakintAdapterError::UnsupportedIndex(
                    index_atom.to_canonical_string(),
                ));
            }
        }
        let left = interner.intern(left_atom)?;
        let right = interner.intern(right_atom)?;
        Ok(Some(Metric::new(left, right)))
    }

    fn decode_scalar_product(
        &self,
        atom: AtomView<'_>,
    ) -> Result<Option<ScalarProduct>, VakintAdapterError> {
        let AtomView::Fun(function) = atom else {
            return Ok(None);
        };
        if function.get_symbol() != self.syntax.dot || function.get_nargs() != 2 {
            return Ok(None);
        }
        let left = decode_bare_loop_vector(function.get(0), atom, self.syntax)?;
        let right = decode_bare_loop_vector(function.get(1), atom, self.syntax)?;
        Ok(Some(ScalarProduct::new(left, right)))
    }

    fn substitute_mass(&self, coefficient: Atom, mass_squared: AtomView<'_>) -> Atom {
        coefficient
            .replace(Atom::var(self.rustred_mass_squared).to_pattern())
            .with(mass_squared.to_pattern())
    }

    fn render_metrics(
        &self,
        metrics: &MetricPairing,
        term: &VakintTwoLoopTerm,
    ) -> Result<Atom, VakintAdapterError> {
        let mut product = Atom::num(1);
        for metric in metrics.metrics() {
            let mut left = term
                .index_atom(metric.left())
                .ok_or(VakintAdapterError::InternalMissingIndex(metric.left().id()))?
                .clone();
            let mut right = term
                .index_atom(metric.right())
                .ok_or(VakintAdapterError::InternalMissingIndex(
                    metric.right().id(),
                ))?
                .clone();
            // Keep stable metric argument ordering even when the ambient
            // Symbolica registry did not declare `vakint::g` symmetric.
            if right < left {
                std::mem::swap(&mut left, &mut right);
            }
            product *= FunctionBuilder::new(self.syntax.metric)
                .add_args([left, right])
                .finish();
        }
        Ok(product)
    }

    fn render_master(&self, mass_squared: &Atom, master: &Integral) -> Atom {
        let i2l = FunctionBuilder::new(self.syntax.i2l)
            .add_arg(mass_squared.clone())
            .add_args(master.powers().iter().copied().map(Atom::num))
            .finish();
        FunctionBuilder::new(self.syntax.topo).add_arg(i2l).finish()
    }
}

fn validate_limits(limits: VakintAdapterLimits) -> Result<(), VakintAdapterError> {
    for (resource, limit) in [
        ("input Atom nodes", limits.max_input_nodes),
        ("expanded terms", limits.max_expanded_terms),
        ("tensor factor occurrences", limits.max_tensor_factors),
        ("distinct Lorentz indices", limits.max_distinct_indices),
        ("projector perfect matchings", limits.max_projector_pairings),
        ("lowered denominator monomials", limits.max_lowered_terms),
        ("output master summands", limits.max_output_terms),
    ] {
        if limit == 0 {
            return Err(VakintAdapterError::InvalidLimit { resource });
        }
    }
    if limits.max_lowering_operations == 0 {
        return Err(VakintAdapterError::InvalidLimit {
            resource: "lowering expansion operations",
        });
    }
    Ok(())
}

fn parse_symbol(name: &str) -> Result<Symbol, VakintAdapterError> {
    let atom = try_parse!(name, default_namespace = "rustred")
        .map_err(|message| VakintAdapterError::Symbol(message.to_string()))?;
    match atom {
        Atom::Var(variable) => Ok(variable.get_symbol()),
        _ => Err(VakintAdapterError::Symbol(format!(
            "`{name}` did not parse as a symbol"
        ))),
    }
}

fn vakint_metric_symbol() -> Result<Symbol, VakintAdapterError> {
    if let Some(symbol) = get_symbol!("vakint::g") {
        return Ok(symbol);
    }
    try_symbol!("vakint::g"; Symmetric)
        .map_err(|message| VakintAdapterError::Symbol(message.to_string()))
}

fn vakint_dot_symbol() -> Result<Symbol, VakintAdapterError> {
    if let Some(symbol) = get_symbol!("vakint::dot") {
        return Ok(symbol);
    }
    try_symbol!("vakint::dot"; Symmetric, Linear)
        .map_err(|message| VakintAdapterError::Symbol(message.to_string()))
}

fn vakint_vkdot_symbol() -> Result<Symbol, VakintAdapterError> {
    if let Some(symbol) = get_symbol!("vakint::vkdot") {
        return Ok(symbol);
    }
    try_symbol!("vakint::vkdot"; Symmetric, Linear)
        .map_err(|message| VakintAdapterError::Symbol(message.to_string()))
}

fn atom_node_count(atom: AtomView<'_>) -> usize {
    let mut count = 0usize;
    let mut pending = vec![atom];
    while let Some(current) = pending.pop() {
        count = count.saturating_add(1);
        match current {
            AtomView::Fun(function) => pending.extend(function.iter()),
            AtomView::Pow(power) => pending.extend(power.iter()),
            AtomView::Mul(product) => pending.extend(product.iter()),
            AtomView::Add(sum) => pending.extend(sum.iter()),
            AtomView::Num(_) | AtomView::Var(_) => {}
        }
    }
    count
}

fn controlled_distribute(
    atom: AtomView<'_>,
    max_terms: usize,
) -> Result<Vec<Atom>, VakintAdapterError> {
    enum Task<'a> {
        Visit(AtomView<'a>),
        Add(usize),
        Multiply(usize),
    }

    let mut tasks = vec![Task::Visit(atom)];
    let mut values = Vec::<Vec<Atom>>::new();
    while let Some(task) = tasks.pop() {
        match task {
            Task::Visit(AtomView::Add(sum)) => {
                let children: Vec<_> = sum.iter().collect();
                tasks.push(Task::Add(children.len()));
                tasks.extend(children.into_iter().rev().map(Task::Visit));
            }
            Task::Visit(AtomView::Mul(product)) => {
                let children: Vec<_> = product.iter().collect();
                tasks.push(Task::Multiply(children.len()));
                tasks.extend(children.into_iter().rev().map(Task::Visit));
            }
            Task::Visit(leaf) => values.push(vec![leaf.to_owned()]),
            Task::Add(child_count) => {
                let first_child = values
                    .len()
                    .checked_sub(child_count)
                    .expect("distribution value stack matches the Atom tree");
                let children: Vec<_> = values.drain(first_child..).collect();
                let attempted = children
                    .iter()
                    .fold(0usize, |total, child| total.saturating_add(child.len()));
                check_limit("expanded terms", attempted, max_terms)?;
                values.push(children.into_iter().flatten().collect());
            }
            Task::Multiply(child_count) => {
                let first_child = values
                    .len()
                    .checked_sub(child_count)
                    .expect("distribution value stack matches the Atom tree");
                let children: Vec<_> = values.drain(first_child..).collect();
                let mut output = vec![Atom::num(1)];
                for child_terms in children {
                    let attempted = output.len().saturating_mul(child_terms.len());
                    check_limit("expanded terms", attempted, max_terms)?;
                    let mut next = Vec::with_capacity(attempted);
                    for prefix in &output {
                        for suffix in &child_terms {
                            next.push(prefix.clone() * suffix.clone());
                        }
                    }
                    output = next;
                }
                values.push(output);
            }
        }
    }
    debug_assert_eq!(values.len(), 1);
    Ok(values.pop().unwrap_or_default())
}

fn recognized_nonnegative_power(atom: AtomView<'_>) -> Result<u32, VakintAdapterError> {
    let AtomView::Pow(power) = atom else {
        return Ok(1);
    };
    let exponent = power.get_exp();
    let exponent = i64::try_from(exponent)
        .map_err(|_| VakintAdapterError::UnsupportedPower(atom.to_canonical_string()))?;
    let exponent = u32::try_from(exponent)
        .map_err(|_| VakintAdapterError::UnsupportedPower(atom.to_canonical_string()))?;
    Ok(exponent)
}

fn decode_loop_vector(
    atom: AtomView<'_>,
    full_factor: AtomView<'_>,
) -> Result<LoopVector, VakintAdapterError> {
    let id = i64::try_from(atom).map_err(|_| {
        VakintAdapterError::UnsupportedLoopMomentum(full_factor.to_canonical_string())
    })?;
    match id {
        1 => Ok(LoopVector::new(0)),
        2 => Ok(LoopVector::new(1)),
        _ => Err(VakintAdapterError::UnsupportedLoopMomentum(
            full_factor.to_canonical_string(),
        )),
    }
}

fn decode_bare_loop_vector(
    atom: AtomView<'_>,
    full_factor: AtomView<'_>,
    syntax: VakintAtomSyntax,
) -> Result<LoopVector, VakintAdapterError> {
    let AtomView::Fun(function) = atom else {
        return Err(VakintAdapterError::UnsupportedScalarProduct(
            full_factor.to_canonical_string(),
        ));
    };
    if function.get_symbol() != syntax.loop_momentum || function.get_nargs() != 1 {
        return Err(VakintAdapterError::UnsupportedScalarProduct(
            full_factor.to_canonical_string(),
        ));
    }
    decode_loop_vector(function.get(0), full_factor)
}

fn contains_reserved_head(atom: AtomView<'_>, syntax: VakintAtomSyntax) -> bool {
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

fn check_limit(
    resource: &'static str,
    requested: usize,
    limit: usize,
) -> Result<(), VakintAdapterError> {
    if requested > limit {
        Err(VakintAdapterError::ResourceLimit {
            resource,
            requested: requested as u128,
            limit: limit as u128,
        })
    } else {
        Ok(())
    }
}

fn checked_resource_add(
    resource: &'static str,
    left: usize,
    right: usize,
    limit: usize,
) -> Result<usize, VakintAdapterError> {
    let Some(total) = left.checked_add(right) else {
        return Err(VakintAdapterError::ResourceLimit {
            resource,
            requested: u128::MAX,
            limit: limit as u128,
        });
    };
    check_limit(resource, total, limit)?;
    Ok(total)
}

#[derive(Debug)]
struct IndexInterner {
    ids: BTreeMap<Atom, LorentzIndex>,
    atoms: Vec<Atom>,
    limit: usize,
}

impl IndexInterner {
    fn new(limit: usize) -> Self {
        Self {
            ids: BTreeMap::new(),
            atoms: Vec::new(),
            limit,
        }
    }

    fn intern(&mut self, atom: AtomView<'_>) -> Result<LorentzIndex, VakintAdapterError> {
        if let Some(&index) = self.ids.get(&atom.to_owned()) {
            return Ok(index);
        }
        let requested = self.atoms.len().saturating_add(1);
        check_limit("distinct Lorentz indices", requested, self.limit)?;
        let id =
            u32::try_from(self.atoms.len()).map_err(|_| VakintAdapterError::ResourceLimit {
                resource: "Lorentz-index identifier",
                requested: self.atoms.len() as u128,
                limit: u32::MAX as u128,
            })?;
        let atom = atom.to_owned();
        let index = LorentzIndex::new(id);
        self.ids.insert(atom.clone(), index);
        self.atoms.push(atom);
        Ok(index)
    }

    fn into_atoms(self) -> Vec<Atom> {
        self.atoms
    }
}

/// Typed failures from the intentionally narrow Vakint bridge.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum VakintAdapterError {
    Parse(String),
    Symbol(String),
    Pipeline(TwoLoopPipelineError),
    Tensor(TensorError),
    TensorFamily(TensorFamilyError),
    InvalidLimit {
        resource: &'static str,
    },
    ResourceLimit {
        resource: &'static str,
        requested: u128,
        limit: u128,
    },
    MissingTopology,
    MultipleTopologies,
    MalformedTopology(String),
    NonIntegerPropagatorPower {
        position: usize,
        value: String,
    },
    PropagatorPowerOutOfRange {
        position: usize,
        power: i64,
    },
    UnsupportedPower(String),
    UnsupportedLoopMomentum(String),
    UnsupportedScalarProduct(String),
    UnsupportedIndex(String),
    UnsupportedReservedFactor(String),
    UnsupportedMass(String),
    InternalMissingIndex(u32),
}

impl fmt::Display for VakintAdapterError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Parse(message) => write!(formatter, "cannot parse Vakint expression: {message}"),
            Self::Symbol(message) => write!(formatter, "cannot register Vakint symbol: {message}"),
            Self::Pipeline(error) => error.fmt(formatter),
            Self::Tensor(error) => error.fmt(formatter),
            Self::TensorFamily(error) => error.fmt(formatter),
            Self::InvalidLimit { resource } => {
                write!(
                    formatter,
                    "Vakint adapter limit `{resource}` must be nonzero"
                )
            }
            Self::ResourceLimit {
                resource,
                requested,
                limit,
            } => write!(
                formatter,
                "Vakint adapter {resource} requires {requested}, exceeding limit {limit}"
            ),
            Self::MissingTopology => formatter.write_str(
                "each expanded Vakint term must contain one topo(I2L(mass,a1,a2,a3)) factor",
            ),
            Self::MultipleTopologies => formatter
                .write_str("an expanded Vakint term contains more than one topology factor"),
            Self::MalformedTopology(value) => write!(
                formatter,
                "unsupported two-loop topology `{value}`; expected topo(I2L(mass,a1,a2,a3))"
            ),
            Self::NonIntegerPropagatorPower { position, value } => write!(
                formatter,
                "I2L propagator power {position} is not an integer: {value}"
            ),
            Self::PropagatorPowerOutOfRange { position, power } => write!(
                formatter,
                "I2L propagator power {position}={power} is outside the i32 range"
            ),
            Self::UnsupportedPower(value) => write!(
                formatter,
                "Vakint tensor factor has a noninteger or negative power: {value}"
            ),
            Self::UnsupportedLoopMomentum(value) => write!(
                formatter,
                "only loop momenta k(1,...) and k(2,...) are supported: {value}"
            ),
            Self::UnsupportedScalarProduct(value) => write!(
                formatter,
                "scalar products must have the form dot(k(1|2),k(1|2)): {value}"
            ),
            Self::UnsupportedIndex(value) => write!(
                formatter,
                "a Lorentz-index Atom contains reserved Vakint syntax: {value}"
            ),
            Self::UnsupportedReservedFactor(value) => write!(
                formatter,
                "reserved Vakint tensor/topology syntax occurs in an unsupported factor: {value}"
            ),
            Self::UnsupportedMass(value) => write!(
                formatter,
                "the I2L mass argument contains reserved Vakint syntax: {value}"
            ),
            Self::InternalMissingIndex(index) => write!(
                formatter,
                "projected metric refers to unknown adapter-local index {index}"
            ),
        }
    }
}

impl Error for VakintAdapterError {}

impl From<TwoLoopPipelineError> for VakintAdapterError {
    fn from(value: TwoLoopPipelineError) -> Self {
        Self::Pipeline(value)
    }
}

impl From<TensorError> for VakintAdapterError {
    fn from(value: TensorError) -> Self {
        Self::Tensor(value)
    }
}

impl From<TensorFamilyError> for VakintAdapterError {
    fn from(value: TensorFamilyError) -> Self {
        Self::TensorFamily(value)
    }
}
