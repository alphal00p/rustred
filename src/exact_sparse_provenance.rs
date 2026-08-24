//! On-demand expansion of exact sparse-elimination provenance.
//!
//! The base elimination certificate intentionally retains compact recursive
//! pivot traces.  This module expands only caller-selected pivots or dependent
//! source rows into explicit source-row weights.  Every expansion is derived
//! after the base certificate has replayed, and is then checked independently
//! by multiplying the weights back into the authenticated source matrix.

use std::collections::{BTreeMap, BTreeSet};
use std::error::Error;
use std::fmt;
use std::fmt::Write as _;

use symbolica::prelude::*;

use crate::coefficient::{
    coefficient_product_degree_bound, coefficient_sum_degree_bound,
    coefficient_variable_degrees, symbolica_coefficient_degree_is_representable,
};
use crate::exact_sparse_elimination::{
    ExactSparseElimination, ExactSparseEliminationError, ExactSparseRow,
};
use crate::{Coefficient, CoefficientContext, SYMBOLICA_COEFFICIENT_EXPONENT_LIMIT};

const EXACT_SPARSE_PROVENANCE_SCHEMA: &str = "rustred-exact-sparse-provenance-v1";
const FNV1A64_OFFSET: u64 = 0xcbf2_9ce4_8422_2325;
const FNV1A64_PRIME: u64 = 0x0000_0100_0000_01b3;

/// One selected explicit provenance artifact.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum ExactSparseProvenanceRequest {
    PivotRow { pivot_ordinal: usize },
    DependentZero { source_row_index: usize },
}

/// Independent resource limits for optional provenance expansion.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ExactSparseProvenanceConfig {
    pub max_requests: usize,
    pub max_forward_reductions: usize,
    pub max_forward_updates: usize,
    pub max_dag_node_visits: usize,
    pub max_dag_edge_visits: usize,
    pub max_pending_coefficients: usize,
    pub max_pending_updates: usize,
    pub max_coefficient_operations: usize,
    pub max_cumulative_pair_products: usize,
    pub max_matrix_term_updates: usize,
    pub max_item_source_weights: usize,
    pub max_retained_source_weights: usize,
    pub max_retained_forward_reductions: usize,
    pub max_retained_coefficient_terms: usize,
    pub max_retained_coefficient_bytes: usize,
    pub max_checksum_coefficient_bytes: usize,
    pub max_coefficient_degree: usize,
    pub max_coefficient_operation_terms: usize,
    pub max_coefficient_dense_terms: usize,
    pub max_integer_bits: usize,
}

impl Default for ExactSparseProvenanceConfig {
    fn default() -> Self {
        Self {
            max_requests: 10_000,
            max_forward_reductions: 100_000_000,
            max_forward_updates: 1_000_000_000,
            max_dag_node_visits: 100_000_000,
            max_dag_edge_visits: 500_000_000,
            max_pending_coefficients: 10_000,
            max_pending_updates: 500_000_000,
            max_coefficient_operations: 1_000_000_000,
            max_cumulative_pair_products: 1_000_000_000,
            max_matrix_term_updates: 1_000_000_000,
            max_item_source_weights: 10_000,
            max_retained_source_weights: 10_000_000,
            max_retained_forward_reductions: 100_000_000,
            max_retained_coefficient_terms: 500_000_000,
            max_retained_coefficient_bytes: 2 * 1024 * 1024 * 1024,
            max_checksum_coefficient_bytes: 2 * 1024 * 1024 * 1024,
            max_coefficient_degree: 4_096,
            max_coefficient_operation_terms: 10_000_000,
            max_coefficient_dense_terms: 100_000_000,
            max_integer_bits: 1_000_000,
        }
    }
}

/// One exact coefficient in an expanded source-row combination.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ExactSparseSourceWeight {
    source_row_index: usize,
    coefficient: Coefficient,
}

impl ExactSparseSourceWeight {
    pub const fn source_row_index(&self) -> usize {
        self.source_row_index
    }

    pub const fn coefficient(&self) -> &Coefficient {
        &self.coefficient
    }
}

/// One factor used when a dependent source row is reduced to zero.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ExactSparseForwardReduction {
    pivot_ordinal: usize,
    factor: Coefficient,
}

impl ExactSparseForwardReduction {
    pub const fn pivot_ordinal(&self) -> usize {
        self.pivot_ordinal
    }

    pub const fn factor(&self) -> &Coefficient {
        &self.factor
    }
}

/// Explicit source weights for one unit pivot row.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ExactSparseExpandedPivot {
    pivot_ordinal: usize,
    pivot_column: usize,
    source_weights: Vec<ExactSparseSourceWeight>,
    checksum: u64,
}

impl ExactSparseExpandedPivot {
    pub const fn pivot_ordinal(&self) -> usize {
        self.pivot_ordinal
    }

    pub const fn pivot_column(&self) -> usize {
        self.pivot_column
    }

    pub fn source_weights(&self) -> &[ExactSparseSourceWeight] {
        &self.source_weights
    }

    pub const fn checksum(&self) -> u64 {
        self.checksum
    }
}

/// Explicit normalized left-kernel root for one dependent source row.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ExactSparseDependentZeroRoot {
    source_row_index: usize,
    forward_reductions: Vec<ExactSparseForwardReduction>,
    source_weights: Vec<ExactSparseSourceWeight>,
    checksum: u64,
}

impl ExactSparseDependentZeroRoot {
    pub const fn source_row_index(&self) -> usize {
        self.source_row_index
    }

    pub fn forward_reductions(&self) -> &[ExactSparseForwardReduction] {
        &self.forward_reductions
    }

    pub fn source_weights(&self) -> &[ExactSparseSourceWeight] {
        &self.source_weights
    }

    pub const fn checksum(&self) -> u64 {
        self.checksum
    }
}

/// One expanded artifact in a provenance bundle.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ExactSparseProvenanceItem {
    Pivot(ExactSparseExpandedPivot),
    DependentZero(ExactSparseDependentZeroRoot),
}

impl ExactSparseProvenanceItem {
    pub fn source_weights(&self) -> &[ExactSparseSourceWeight] {
        match self {
            Self::Pivot(item) => item.source_weights(),
            Self::DependentZero(item) => item.source_weights(),
        }
    }

    pub const fn checksum(&self) -> u64 {
        match self {
            Self::Pivot(item) => item.checksum(),
            Self::DependentZero(item) => item.checksum(),
        }
    }
}

/// Exact work and retained-payload census for one expansion batch.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ExactSparseProvenanceStats {
    requests: usize,
    pivot_items: usize,
    dependent_zero_items: usize,
    forward_reductions: usize,
    forward_updates: usize,
    dag_node_visits: usize,
    dag_edge_visits: usize,
    pending_updates: usize,
    maximum_pending_coefficients: usize,
    coefficient_multiplications: usize,
    coefficient_additions: usize,
    coefficient_divisions: usize,
    cumulative_pair_products: usize,
    matrix_term_updates: usize,
    retained_source_weights: usize,
    retained_forward_reductions: usize,
    retained_coefficient_terms: usize,
    retained_coefficient_bytes: usize,
    maximum_item_source_weights: usize,
    maximum_coefficient_degree: usize,
}

impl ExactSparseProvenanceStats {
    pub const fn requests(self) -> usize { self.requests }
    pub const fn pivot_items(self) -> usize { self.pivot_items }
    pub const fn dependent_zero_items(self) -> usize { self.dependent_zero_items }
    pub const fn forward_reductions(self) -> usize { self.forward_reductions }
    pub const fn forward_updates(self) -> usize { self.forward_updates }
    pub const fn dag_node_visits(self) -> usize { self.dag_node_visits }
    pub const fn dag_edge_visits(self) -> usize { self.dag_edge_visits }
    pub const fn pending_updates(self) -> usize { self.pending_updates }
    pub const fn maximum_pending_coefficients(self) -> usize { self.maximum_pending_coefficients }
    pub const fn coefficient_multiplications(self) -> usize { self.coefficient_multiplications }
    pub const fn coefficient_additions(self) -> usize { self.coefficient_additions }
    pub const fn coefficient_divisions(self) -> usize { self.coefficient_divisions }
    pub const fn cumulative_pair_products(self) -> usize { self.cumulative_pair_products }
    pub const fn matrix_term_updates(self) -> usize { self.matrix_term_updates }
    pub const fn retained_source_weights(self) -> usize { self.retained_source_weights }
    pub const fn retained_forward_reductions(self) -> usize { self.retained_forward_reductions }
    pub const fn retained_coefficient_terms(self) -> usize { self.retained_coefficient_terms }
    pub const fn retained_coefficient_bytes(self) -> usize { self.retained_coefficient_bytes }
    pub const fn maximum_item_source_weights(self) -> usize { self.maximum_item_source_weights }
    pub const fn maximum_coefficient_degree(self) -> usize { self.maximum_coefficient_degree }
}

/// A deterministic collection of independently replayed explicit provenance.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ExactSparseProvenanceBundle {
    config: ExactSparseProvenanceConfig,
    source_checksum: u64,
    certificate_checksum: u64,
    items: BTreeMap<ExactSparseProvenanceRequest, ExactSparseProvenanceItem>,
    stats: ExactSparseProvenanceStats,
    checksum: u64,
}

impl ExactSparseProvenanceBundle {
    pub const SCHEMA: &'static str = EXACT_SPARSE_PROVENANCE_SCHEMA;

    pub fn build_authenticated(
        certificate: &ExactSparseElimination,
        context: &CoefficientContext,
        source_rows: &[ExactSparseRow],
        requests: &[ExactSparseProvenanceRequest],
        config: ExactSparseProvenanceConfig,
    ) -> Result<Self, ExactSparseProvenanceError> {
        validate_config(config)?;
        certificate.replay(context, source_rows)?;
        build_from_replayed(certificate, context, source_rows, requests, config)
    }

    pub fn build_one_authenticated(
        certificate: &ExactSparseElimination,
        context: &CoefficientContext,
        source_rows: &[ExactSparseRow],
        request: ExactSparseProvenanceRequest,
        config: ExactSparseProvenanceConfig,
    ) -> Result<Self, ExactSparseProvenanceError> {
        Self::build_authenticated(certificate, context, source_rows, &[request], config)
    }

    pub const fn config(&self) -> ExactSparseProvenanceConfig { self.config }
    pub const fn source_checksum(&self) -> u64 { self.source_checksum }
    pub const fn certificate_checksum(&self) -> u64 { self.certificate_checksum }
    pub fn items(&self) -> &BTreeMap<ExactSparseProvenanceRequest, ExactSparseProvenanceItem> {
        &self.items
    }
    pub fn item(&self, request: ExactSparseProvenanceRequest) -> Option<&ExactSparseProvenanceItem> {
        self.items.get(&request)
    }
    pub const fn stats(&self) -> ExactSparseProvenanceStats { self.stats }
    pub const fn checksum(&self) -> u64 { self.checksum }

    /// Replays the base proof, re-expands every requested artifact, and compares
    /// all exact payload, statistics, and checksums.
    pub fn replay_authenticated(
        &self,
        certificate: &ExactSparseElimination,
        context: &CoefficientContext,
        source_rows: &[ExactSparseRow],
    ) -> Result<(), ExactSparseProvenanceError> {
        if certificate.checksum() != self.certificate_checksum {
            return Err(ExactSparseProvenanceError::CertificateChecksumMismatch {
                expected: self.certificate_checksum,
                actual: certificate.checksum(),
            });
        }
        let requests = self.items.keys().copied().collect::<Vec<_>>();
        let replayed = Self::build_authenticated(
            certificate,
            context,
            source_rows,
            &requests,
            self.config,
        )?;
        if &replayed != self {
            return Err(ExactSparseProvenanceError::ReplayMismatch);
        }
        Ok(())
    }
}

impl ExactSparseElimination {
    /// Canonical source-row complement of the distinct pivot base rows.
    pub fn dependent_source_rows(&self) -> Vec<usize> {
        let pivot_sources = self
            .pivot_rules()
            .iter()
            .map(|rule| rule.source_row_index())
            .collect::<BTreeSet<_>>();
        (0..self.source_row_count())
            .filter(|row| !pivot_sources.contains(row))
            .collect()
    }

    pub fn expand_provenance_authenticated(
        &self,
        context: &CoefficientContext,
        source_rows: &[ExactSparseRow],
        requests: &[ExactSparseProvenanceRequest],
        config: ExactSparseProvenanceConfig,
    ) -> Result<ExactSparseProvenanceBundle, ExactSparseProvenanceError> {
        ExactSparseProvenanceBundle::build_authenticated(
            self,
            context,
            source_rows,
            requests,
            config,
        )
    }
}

/// Typed failures from optional exact provenance expansion.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ExactSparseProvenanceError {
    Base(ExactSparseEliminationError),
    EmptyRequestBatch,
    DuplicateRequest(ExactSparseProvenanceRequest),
    PivotOrdinalOutOfRange { pivot_ordinal: usize, rank: usize },
    SourceRowOutOfRange { source_row_index: usize, source_row_count: usize },
    SourceRowIsPivotBase { source_row_index: usize, pivot_ordinal: usize },
    InvalidTrace { pivot_ordinal: usize, reason: &'static str },
    ForwardReductionDidNotVanish { source_row_index: usize },
    ExpandedMatrixIdentityMismatch { request: ExactSparseProvenanceRequest },
    NormalizedRootMismatch { source_row_index: usize },
    CertificateChecksumMismatch { expected: u64, actual: u64 },
    ReplayMismatch,
    ResourceLimit { resource: &'static str, requested: u128, limit: u128 },
    ArithmeticOverflow { resource: &'static str },
    CoefficientContextMismatch,
    DivisionByZero,
}

impl fmt::Display for ExactSparseProvenanceError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Base(error) => write!(formatter, "base exact replay failed: {error}"),
            Self::EmptyRequestBatch => formatter.write_str("provenance request batch is empty"),
            Self::DuplicateRequest(request) => write!(formatter, "duplicate provenance request {request:?}"),
            Self::PivotOrdinalOutOfRange { pivot_ordinal, rank } => write!(formatter, "pivot ordinal {pivot_ordinal} is outside 0..{rank}"),
            Self::SourceRowOutOfRange { source_row_index, source_row_count } => write!(formatter, "source row {source_row_index} is outside 0..{source_row_count}"),
            Self::SourceRowIsPivotBase { source_row_index, pivot_ordinal } => write!(formatter, "source row {source_row_index} is pivot {pivot_ordinal}'s base row, not a dependent zero"),
            Self::InvalidTrace { pivot_ordinal, reason } => write!(formatter, "invalid provenance trace at pivot {pivot_ordinal}: {reason}"),
            Self::ForwardReductionDidNotVanish { source_row_index } => write!(formatter, "source row {source_row_index} did not reduce exactly to zero"),
            Self::ExpandedMatrixIdentityMismatch { request } => write!(formatter, "expanded source weights do not replay request {request:?}"),
            Self::NormalizedRootMismatch { source_row_index } => write!(formatter, "dependent root {source_row_index} is not normalized on its own source coordinate"),
            Self::CertificateChecksumMismatch { expected, actual } => write!(formatter, "base certificate checksum mismatch: expected 0x{expected:016x}, found 0x{actual:016x}"),
            Self::ReplayMismatch => formatter.write_str("exact provenance replay differs from retained payload"),
            Self::ResourceLimit { resource, requested, limit } => write!(formatter, "exact provenance {resource} requested {requested}, limit is {limit}"),
            Self::ArithmeticOverflow { resource } => write!(formatter, "arithmetic overflow while counting {resource}"),
            Self::CoefficientContextMismatch => formatter.write_str("provenance coefficient variable map differs from the authenticated context"),
            Self::DivisionByZero => formatter.write_str("provenance expansion attempted division by zero"),
        }
    }
}

impl Error for ExactSparseProvenanceError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self { Self::Base(error) => Some(error), _ => None }
    }
}

impl From<ExactSparseEliminationError> for ExactSparseProvenanceError {
    fn from(error: ExactSparseEliminationError) -> Self { Self::Base(error) }
}

#[derive(Default)]
struct WorkCensus {
    stats: ExactSparseProvenanceStats,
    retained_bytes: usize,
}

struct CheckedArithmetic {
    context_zero: Coefficient,
    one: Coefficient,
    config: ExactSparseProvenanceConfig,
    operations: usize,
    census: WorkCensus,
}

impl CheckedArithmetic {
    fn new(context: &CoefficientContext, config: ExactSparseProvenanceConfig) -> Result<Self, ExactSparseProvenanceError> {
        let mut value = Self {
            context_zero: context.zero(),
            one: context.one(),
            config,
            operations: 0,
            census: WorkCensus::default(),
        };
        let zero = value.context_zero.clone();
        value.check_existing(&zero)?;
        Ok(value)
    }

    fn charge_operation(&mut self) -> Result<(), ExactSparseProvenanceError> {
        self.operations = checked_add(self.operations, 1, "coefficient operations")?;
        check_resource("coefficient operations", self.operations, self.config.max_coefficient_operations)
    }

    fn charge_pairs(&mut self, pairs: u128) -> Result<(), ExactSparseProvenanceError> {
        let pairs = usize::try_from(pairs).map_err(|_| ExactSparseProvenanceError::ArithmeticOverflow { resource: "cumulative coefficient term-pair products" })?;
        let requested = checked_add(self.census.stats.cumulative_pair_products, pairs, "cumulative coefficient term-pair products")?;
        check_resource("cumulative coefficient term-pair products", requested, self.config.max_cumulative_pair_products)?;
        self.census.stats.cumulative_pair_products = requested;
        Ok(())
    }

    fn check_existing(&mut self, value: &Coefficient) -> Result<(), ExactSparseProvenanceError> {
        if value.get_variables() != self.context_zero.get_variables() || value.denominator.is_zero() {
            return Err(ExactSparseProvenanceError::CoefficientContextMismatch);
        }
        let degree = coefficient_maximum_degree(value);
        if degree > self.config.max_coefficient_degree as u128 || !symbolica_coefficient_degree_is_representable(degree) {
            return Err(ExactSparseProvenanceError::ResourceLimit {
                resource: "coefficient degree",
                requested: degree,
                limit: (self.config.max_coefficient_degree as u128).min(SYMBOLICA_COEFFICIENT_EXPONENT_LIMIT),
            });
        }
        self.census.stats.maximum_coefficient_degree = self.census.stats.maximum_coefficient_degree.max(usize::try_from(degree).unwrap_or(usize::MAX));
        let terms = value.numerator.nterms().max(value.denominator.nterms());
        check_resource("coefficient operand/result terms", terms, self.config.max_coefficient_operation_terms)?;
        let dense = existing_dense_bound(value);
        if dense > self.config.max_coefficient_dense_terms as u128 {
            return Err(ExactSparseProvenanceError::ResourceLimit { resource: "coefficient dense operand/result universe", requested: dense, limit: self.config.max_coefficient_dense_terms as u128 });
        }
        let bits = value.numerator.coefficients.iter().chain(&value.denominator.coefficients).map(integer_bit_length).max().unwrap_or(0);
        if bits > self.config.max_integer_bits as u128 {
            return Err(ExactSparseProvenanceError::ResourceLimit { resource: "coefficient integer bits", requested: bits, limit: self.config.max_integer_bits as u128 });
        }
        Ok(())
    }

    fn multiply(&mut self, left: &Coefficient, right: &Coefficient) -> Result<Coefficient, ExactSparseProvenanceError> {
        self.check_existing(left)?;
        self.check_existing(right)?;
        self.charge_operation()?;
        self.charge_pairs(product_pair_work(left, right))?;
        check_degree_bound(coefficient_product_degree_bound(left, right), self.config)?;
        check_dense_bound(product_dense_bound(left, right), self.config)?;
        self.census.stats.coefficient_multiplications = checked_add(self.census.stats.coefficient_multiplications, 1, "coefficient multiplications")?;
        let output = if left.is_zero() || right.is_zero() { self.context_zero.clone() } else if left == &self.one { right.clone() } else if right == &self.one { left.clone() } else { left * right };
        self.check_existing(&output)?;
        Ok(output)
    }

    fn add(&mut self, left: &Coefficient, right: &Coefficient) -> Result<Coefficient, ExactSparseProvenanceError> {
        self.check_existing(left)?;
        self.check_existing(right)?;
        self.charge_operation()?;
        self.charge_pairs(sum_pair_work(left, right))?;
        check_degree_bound(coefficient_sum_degree_bound(left, right), self.config)?;
        check_dense_bound(sum_dense_bound(left, right), self.config)?;
        self.census.stats.coefficient_additions = checked_add(self.census.stats.coefficient_additions, 1, "coefficient additions")?;
        let output = if left.is_zero() { right.clone() } else if right.is_zero() { left.clone() } else { left + right };
        self.check_existing(&output)?;
        Ok(output)
    }

    fn subtract(&mut self, left: &Coefficient, right: &Coefficient) -> Result<Coefficient, ExactSparseProvenanceError> {
        self.add(left, &(-right.clone()))
    }

    fn divide(&mut self, left: &Coefficient, right: &Coefficient) -> Result<Coefficient, ExactSparseProvenanceError> {
        if right.is_zero() { return Err(ExactSparseProvenanceError::DivisionByZero); }
        self.check_existing(left)?;
        self.check_existing(right)?;
        self.charge_operation()?;
        self.charge_pairs(product_pair_work(left, right))?;
        check_degree_bound(quotient_degree_bound(left, right), self.config)?;
        check_dense_bound(quotient_dense_bound(left, right), self.config)?;
        self.census.stats.coefficient_divisions = checked_add(self.census.stats.coefficient_divisions, 1, "coefficient divisions")?;
        let output = if right == &self.one { left.clone() } else { left / right };
        self.check_existing(&output)?;
        Ok(output)
    }

    fn charge_retained_coefficient(&mut self, value: &Coefficient) -> Result<(), ExactSparseProvenanceError> {
        self.check_existing(value)?;
        let terms = value.numerator.nterms().checked_add(value.denominator.nterms()).ok_or(ExactSparseProvenanceError::ArithmeticOverflow { resource: "retained coefficient terms" })?;
        let requested_terms = checked_add(self.census.stats.retained_coefficient_terms, terms, "retained coefficient terms")?;
        check_resource("retained coefficient terms", requested_terms, self.config.max_retained_coefficient_terms)?;
        let bytes = bounded_display_len(value, self.retained_bytes, self.config.max_retained_coefficient_bytes, "retained coefficient bytes")?;
        let requested_bytes = checked_add(self.retained_bytes, bytes, "retained coefficient bytes")?;
        check_resource("retained coefficient bytes", requested_bytes, self.config.max_retained_coefficient_bytes)?;
        self.census.stats.retained_coefficient_terms = requested_terms;
        self.census.stats.retained_coefficient_bytes = requested_bytes;
        self.retained_bytes = requested_bytes;
        Ok(())
    }
}

fn build_from_replayed(
    certificate: &ExactSparseElimination,
    context: &CoefficientContext,
    source_rows: &[ExactSparseRow],
    requests: &[ExactSparseProvenanceRequest],
    config: ExactSparseProvenanceConfig,
) -> Result<ExactSparseProvenanceBundle, ExactSparseProvenanceError> {
    if requests.is_empty() { return Err(ExactSparseProvenanceError::EmptyRequestBatch); }
    check_resource("requests", requests.len(), config.max_requests)?;
    let mut canonical = BTreeSet::new();
    for &request in requests {
        validate_request(certificate, request)?;
        if !canonical.insert(request) { return Err(ExactSparseProvenanceError::DuplicateRequest(request)); }
    }
    let mut arithmetic = CheckedArithmetic::new(context, config)?;
    arithmetic.census.stats.requests = canonical.len();
    let pivot_source_to_ordinal = certificate.pivot_rules().iter().map(|rule| (rule.source_row_index(), rule.ordinal())).collect::<BTreeMap<_, _>>();
    let mut items = BTreeMap::new();
    items.try_reserve(canonical.len()).map_err(|_| ExactSparseProvenanceError::ResourceLimit { resource: "provenance item storage", requested: canonical.len() as u128, limit: config.max_requests as u128 })?;

    for request in canonical {
        let item = match request {
            ExactSparseProvenanceRequest::PivotRow { pivot_ordinal } => {
                arithmetic.census.stats.pivot_items = checked_add(arithmetic.census.stats.pivot_items, 1, "pivot items")?;
                let rule = &certificate.pivot_rules()[pivot_ordinal];
                let mut seeds = BTreeMap::new();
                seeds.insert(pivot_ordinal, context.one());
                let weights = flatten_pending(certificate, context, seeds, BTreeMap::new(), &mut arithmetic)?;
                verify_expanded_identity(request, &weights, rule.row(), source_rows, &mut arithmetic)?;
                retain_weights(&weights, &mut arithmetic)?;
                let source_weights = source_weight_vec(weights);
                let checksum = item_checksum(certificate, request, &[], &source_weights, config)?;
                ExactSparseProvenanceItem::Pivot(ExactSparseExpandedPivot { pivot_ordinal, pivot_column: rule.pivot_column(), source_weights, checksum })
            }
            ExactSparseProvenanceRequest::DependentZero { source_row_index } => {
                if let Some(&pivot_ordinal) = pivot_source_to_ordinal.get(&source_row_index) {
                    return Err(ExactSparseProvenanceError::SourceRowIsPivotBase { source_row_index, pivot_ordinal });
                }
                arithmetic.census.stats.dependent_zero_items = checked_add(arithmetic.census.stats.dependent_zero_items, 1, "dependent-zero items")?;
                let reductions = reduce_source_to_zero(source_row_index, source_rows, certificate, &mut arithmetic)?;
                let mut seeds = BTreeMap::new();
                for reduction in &reductions {
                    seeds.insert(reduction.pivot_ordinal, -reduction.factor.clone());
                }
                let mut initial = BTreeMap::new();
                initial.insert(source_row_index, context.one());
                let weights = flatten_pending(certificate, context, seeds, initial, &mut arithmetic)?;
                if weights.get(&source_row_index) != Some(&context.one()) {
                    return Err(ExactSparseProvenanceError::NormalizedRootMismatch { source_row_index });
                }
                for (&row, coefficient) in &weights {
                    if row != source_row_index && !pivot_source_to_ordinal.contains_key(&row) && !coefficient.is_zero() {
                        return Err(ExactSparseProvenanceError::NormalizedRootMismatch { source_row_index });
                    }
                }
                verify_expanded_identity(request, &weights, &ExactSparseRow::new(), source_rows, &mut arithmetic)?;
                retain_forward_reductions(&reductions, &mut arithmetic)?;
                retain_weights(&weights, &mut arithmetic)?;
                let source_weights = source_weight_vec(weights);
                let checksum = item_checksum(certificate, request, &reductions, &source_weights, config)?;
                ExactSparseProvenanceItem::DependentZero(ExactSparseDependentZeroRoot { source_row_index, forward_reductions: reductions, source_weights, checksum })
            }
        };
        items.insert(request, item);
    }
    let stats = arithmetic.census.stats;
    let checksum = bundle_checksum(certificate, config, &items, stats);
    Ok(ExactSparseProvenanceBundle {
        config,
        source_checksum: certificate.source_checksum(),
        certificate_checksum: certificate.checksum(),
        items,
        stats,
        checksum,
    })
}

fn validate_request(certificate: &ExactSparseElimination, request: ExactSparseProvenanceRequest) -> Result<(), ExactSparseProvenanceError> {
    match request {
        ExactSparseProvenanceRequest::PivotRow { pivot_ordinal } if pivot_ordinal >= certificate.rank() => Err(ExactSparseProvenanceError::PivotOrdinalOutOfRange { pivot_ordinal, rank: certificate.rank() }),
        ExactSparseProvenanceRequest::DependentZero { source_row_index } if source_row_index >= certificate.source_row_count() => Err(ExactSparseProvenanceError::SourceRowOutOfRange { source_row_index, source_row_count: certificate.source_row_count() }),
        _ => Ok(()),
    }
}

fn reduce_source_to_zero(
    source_row_index: usize,
    source_rows: &[ExactSparseRow],
    certificate: &ExactSparseElimination,
    arithmetic: &mut CheckedArithmetic,
) -> Result<Vec<ExactSparseForwardReduction>, ExactSparseProvenanceError> {
    let mut row = source_rows[source_row_index].clone();
    let mut reductions = Vec::new();
    for rule in certificate.pivot_rules() {
        let Some(factor) = row.get(&rule.pivot_column()).cloned() else { continue; };
        if factor.is_zero() { return Err(ExactSparseProvenanceError::InvalidTrace { pivot_ordinal: rule.ordinal(), reason: "forward reduction encountered an explicit zero" }); }
        arithmetic.census.stats.forward_reductions = checked_add(arithmetic.census.stats.forward_reductions, 1, "forward reductions")?;
        check_resource("forward reductions", arithmetic.census.stats.forward_reductions, arithmetic.config.max_forward_reductions)?;
        arithmetic.census.stats.forward_updates = checked_add(arithmetic.census.stats.forward_updates, 1, "forward updates")?;
        check_resource("forward updates", arithmetic.census.stats.forward_updates, arithmetic.config.max_forward_updates)?;
        row.remove(&rule.pivot_column());
        for (&column, pivot_coefficient) in rule.row() {
            if column == rule.pivot_column() { continue; }
            arithmetic.census.stats.forward_updates = checked_add(arithmetic.census.stats.forward_updates, 1, "forward updates")?;
            check_resource("forward updates", arithmetic.census.stats.forward_updates, arithmetic.config.max_forward_updates)?;
            let delta = arithmetic.multiply(&factor, pivot_coefficient)?;
            let updated = match row.remove(&column) {
                Some(current) => arithmetic.subtract(&current, &delta)?,
                None => -delta,
            };
            if !updated.is_zero() { row.insert(column, updated); }
        }
        reductions.push(ExactSparseForwardReduction { pivot_ordinal: rule.ordinal(), factor });
    }
    if !row.is_empty() { return Err(ExactSparseProvenanceError::ForwardReductionDidNotVanish { source_row_index }); }
    Ok(reductions)
}

fn flatten_pending(
    certificate: &ExactSparseElimination,
    context: &CoefficientContext,
    seeds: BTreeMap<usize, Coefficient>,
    mut weights: BTreeMap<usize, Coefficient>,
    arithmetic: &mut CheckedArithmetic,
) -> Result<BTreeMap<usize, Coefficient>, ExactSparseProvenanceError> {
    let rank = certificate.rank();
    check_resource("pending coefficient slots", rank, arithmetic.config.max_pending_coefficients)?;
    let mut pending = Vec::new();
    pending.try_reserve_exact(rank).map_err(|_| ExactSparseProvenanceError::ResourceLimit { resource: "pending coefficient storage", requested: rank as u128, limit: arithmetic.config.max_pending_coefficients as u128 })?;
    pending.resize_with(rank, || None);
    let mut live = 0_usize;
    for (ordinal, coefficient) in seeds {
        if ordinal >= rank { return Err(ExactSparseProvenanceError::PivotOrdinalOutOfRange { pivot_ordinal: ordinal, rank }); }
        add_pending(&mut pending, ordinal, coefficient, &mut live, arithmetic)?;
    }
    for ordinal in (0..rank).rev() {
        let Some(alpha) = pending[ordinal].take() else { continue; };
        live = live.checked_sub(1).ok_or(ExactSparseProvenanceError::ArithmeticOverflow { resource: "live pending coefficients" })?;
        arithmetic.census.stats.dag_node_visits = checked_add(arithmetic.census.stats.dag_node_visits, 1, "DAG node visits")?;
        check_resource("DAG node visits", arithmetic.census.stats.dag_node_visits, arithmetic.config.max_dag_node_visits)?;
        let rule = &certificate.pivot_rules()[ordinal];
        if rule.ordinal() != ordinal || rule.trace().base_source_row_index() != rule.source_row_index() || rule.trace().divisor().is_zero() {
            return Err(ExactSparseProvenanceError::InvalidTrace { pivot_ordinal: ordinal, reason: "base, ordinal, or divisor invariant failed" });
        }
        let quotient = arithmetic.divide(&alpha, rule.trace().divisor())?;
        add_map_coefficient(&mut weights, rule.source_row_index(), quotient.clone(), arithmetic)?;
        for reduction in rule.trace().reductions() {
            let prior = reduction.prior_pivot_ordinal();
            if prior >= ordinal || reduction.factor().is_zero() {
                return Err(ExactSparseProvenanceError::InvalidTrace { pivot_ordinal: ordinal, reason: "edge does not reference a strict nonzero prior pivot" });
            }
            arithmetic.census.stats.dag_edge_visits = checked_add(arithmetic.census.stats.dag_edge_visits, 1, "DAG edge visits")?;
            check_resource("DAG edge visits", arithmetic.census.stats.dag_edge_visits, arithmetic.config.max_dag_edge_visits)?;
            let contribution = -arithmetic.multiply(&quotient, reduction.factor())?;
            add_pending(&mut pending, prior, contribution, &mut live, arithmetic)?;
        }
    }
    arithmetic.census.stats.maximum_item_source_weights = arithmetic.census.stats.maximum_item_source_weights.max(weights.len());
    check_resource("source weights in one item", weights.len(), arithmetic.config.max_item_source_weights)?;
    Ok(weights)
}

fn add_pending(
    pending: &mut [Option<Coefficient>],
    ordinal: usize,
    contribution: Coefficient,
    live: &mut usize,
    arithmetic: &mut CheckedArithmetic,
) -> Result<(), ExactSparseProvenanceError> {
    arithmetic.census.stats.pending_updates = checked_add(arithmetic.census.stats.pending_updates, 1, "pending coefficient updates")?;
    check_resource("pending coefficient updates", arithmetic.census.stats.pending_updates, arithmetic.config.max_pending_updates)?;
    let prior = pending[ordinal].take();
    let updated = match prior {
        Some(current) => arithmetic.add(&current, &contribution)?,
        None => contribution,
    };
    match (updated.is_zero(), prior.is_some()) {
        (true, true) => *live = live.checked_sub(1).ok_or(ExactSparseProvenanceError::ArithmeticOverflow { resource: "live pending coefficients" })?,
        (false, false) => *live = checked_add(*live, 1, "live pending coefficients")?,
        _ => {}
    }
    if !updated.is_zero() { pending[ordinal] = Some(updated); }
    check_resource("live pending coefficients", *live, arithmetic.config.max_pending_coefficients)?;
    arithmetic.census.stats.maximum_pending_coefficients = arithmetic.census.stats.maximum_pending_coefficients.max(*live);
    Ok(())
}

fn add_map_coefficient(
    map: &mut BTreeMap<usize, Coefficient>,
    key: usize,
    contribution: Coefficient,
    arithmetic: &mut CheckedArithmetic,
) -> Result<(), ExactSparseProvenanceError> {
    let updated = match map.remove(&key) {
        Some(current) => arithmetic.add(&current, &contribution)?,
        None => contribution,
    };
    if !updated.is_zero() { map.insert(key, updated); }
    Ok(())
}

fn verify_expanded_identity(
    request: ExactSparseProvenanceRequest,
    weights: &BTreeMap<usize, Coefficient>,
    expected: &ExactSparseRow,
    source_rows: &[ExactSparseRow],
    arithmetic: &mut CheckedArithmetic,
) -> Result<(), ExactSparseProvenanceError> {
    let mut actual = ExactSparseRow::new();
    for (&source_row_index, weight) in weights {
        let source = source_rows.get(source_row_index).ok_or(ExactSparseProvenanceError::SourceRowOutOfRange { source_row_index, source_row_count: source_rows.len() })?;
        for (&column, coefficient) in source {
            arithmetic.census.stats.matrix_term_updates = checked_add(arithmetic.census.stats.matrix_term_updates, 1, "matrix term updates")?;
            check_resource("matrix term updates", arithmetic.census.stats.matrix_term_updates, arithmetic.config.max_matrix_term_updates)?;
            let contribution = arithmetic.multiply(weight, coefficient)?;
            let updated = match actual.remove(&column) {
                Some(current) => arithmetic.add(&current, &contribution)?,
                None => contribution,
            };
            if !updated.is_zero() { actual.insert(column, updated); }
        }
    }
    if &actual != expected { return Err(ExactSparseProvenanceError::ExpandedMatrixIdentityMismatch { request }); }
    Ok(())
}

fn retain_weights(weights: &BTreeMap<usize, Coefficient>, arithmetic: &mut CheckedArithmetic) -> Result<(), ExactSparseProvenanceError> {
    let requested = checked_add(arithmetic.census.stats.retained_source_weights, weights.len(), "retained source weights")?;
    check_resource("retained source weights", requested, arithmetic.config.max_retained_source_weights)?;
    for coefficient in weights.values() { arithmetic.charge_retained_coefficient(coefficient)?; }
    arithmetic.census.stats.retained_source_weights = requested;
    Ok(())
}

fn retain_forward_reductions(reductions: &[ExactSparseForwardReduction], arithmetic: &mut CheckedArithmetic) -> Result<(), ExactSparseProvenanceError> {
    let requested = checked_add(arithmetic.census.stats.retained_forward_reductions, reductions.len(), "retained forward reductions")?;
    check_resource("retained forward reductions", requested, arithmetic.config.max_retained_forward_reductions)?;
    for reduction in reductions { arithmetic.charge_retained_coefficient(&reduction.factor)?; }
    arithmetic.census.stats.retained_forward_reductions = requested;
    Ok(())
}

fn source_weight_vec(weights: BTreeMap<usize, Coefficient>) -> Vec<ExactSparseSourceWeight> {
    weights.into_iter().map(|(source_row_index, coefficient)| ExactSparseSourceWeight { source_row_index, coefficient }).collect()
}

fn validate_config(config: ExactSparseProvenanceConfig) -> Result<(), ExactSparseProvenanceError> {
    if config.max_coefficient_degree as u128 > SYMBOLICA_COEFFICIENT_EXPONENT_LIMIT {
        return Err(ExactSparseProvenanceError::ResourceLimit { resource: "configured coefficient degree", requested: config.max_coefficient_degree as u128, limit: SYMBOLICA_COEFFICIENT_EXPONENT_LIMIT });
    }
    Ok(())
}

fn coefficient_maximum_degree(value: &Coefficient) -> u128 {
    coefficient_variable_degrees(value).into_iter().map(|(n, d)| n.max(d)).max().unwrap_or(0)
}

fn quotient_degree_bound(left: &Coefficient, right: &Coefficient) -> u128 {
    if left.get_variables() != right.get_variables() { return u128::MAX; }
    coefficient_variable_degrees(left).into_iter().zip(coefficient_variable_degrees(right)).map(|((ln, ld), (rn, rd))| ln.saturating_add(rd).max(ld.saturating_add(rn))).max().unwrap_or(0)
}

fn dense_monomial_bound(degrees: impl IntoIterator<Item = u128>) -> u128 {
    degrees.into_iter().fold(1_u128, |count, degree| count.saturating_mul(degree.saturating_add(1)))
}

fn existing_dense_bound(value: &Coefficient) -> u128 {
    let degrees = coefficient_variable_degrees(value);
    dense_monomial_bound(degrees.iter().map(|&(n, _)| n)).max(dense_monomial_bound(degrees.iter().map(|&(_, d)| d)))
}

fn product_dense_bound(left: &Coefficient, right: &Coefficient) -> u128 {
    let left = coefficient_variable_degrees(left);
    let right = coefficient_variable_degrees(right);
    dense_monomial_bound(left.iter().zip(&right).map(|(&(ln, _), &(rn, _))| ln.saturating_add(rn))).max(dense_monomial_bound(left.iter().zip(&right).map(|(&(_, ld), &(_, rd))| ld.saturating_add(rd))))
}

fn sum_dense_bound(left: &Coefficient, right: &Coefficient) -> u128 {
    let left = coefficient_variable_degrees(left);
    let right = coefficient_variable_degrees(right);
    dense_monomial_bound(left.iter().zip(&right).map(|(&(ln, ld), &(rn, rd))| ln.saturating_add(rd).max(rn.saturating_add(ld)))).max(dense_monomial_bound(left.iter().zip(&right).map(|(&(_, ld), &(_, rd))| ld.saturating_add(rd))))
}

fn quotient_dense_bound(left: &Coefficient, right: &Coefficient) -> u128 {
    let left = coefficient_variable_degrees(left);
    let right = coefficient_variable_degrees(right);
    dense_monomial_bound(left.iter().zip(&right).map(|(&(ln, _), &(_, rd))| ln.saturating_add(rd))).max(dense_monomial_bound(left.iter().zip(&right).map(|(&(_, ld), &(rn, _))| ld.saturating_add(rn))))
}

fn check_degree_bound(requested: u128, config: ExactSparseProvenanceConfig) -> Result<(), ExactSparseProvenanceError> {
    let limit = (config.max_coefficient_degree as u128).min(SYMBOLICA_COEFFICIENT_EXPONENT_LIMIT);
    if requested > limit || !symbolica_coefficient_degree_is_representable(requested) { return Err(ExactSparseProvenanceError::ResourceLimit { resource: "coefficient degree", requested, limit }); }
    Ok(())
}

fn check_dense_bound(requested: u128, config: ExactSparseProvenanceConfig) -> Result<(), ExactSparseProvenanceError> {
    if requested > config.max_coefficient_dense_terms as u128 { return Err(ExactSparseProvenanceError::ResourceLimit { resource: "coefficient dense operand/result universe", requested, limit: config.max_coefficient_dense_terms as u128 }); }
    Ok(())
}

fn term_pair_product(left: usize, right: usize) -> u128 { (left as u128).saturating_mul(right as u128) }
fn product_pair_work(left: &Coefficient, right: &Coefficient) -> u128 {
    term_pair_product(left.numerator.nterms(), right.numerator.nterms())
        .saturating_add(term_pair_product(left.denominator.nterms(), right.denominator.nterms()))
        .saturating_add(term_pair_product(left.numerator.nterms(), right.denominator.nterms()))
        .saturating_add(term_pair_product(left.denominator.nterms(), right.numerator.nterms()))
}
fn sum_pair_work(left: &Coefficient, right: &Coefficient) -> u128 {
    term_pair_product(left.denominator.nterms(), right.denominator.nterms())
        .saturating_add(term_pair_product(left.numerator.nterms(), right.denominator.nterms()))
        .saturating_add(term_pair_product(right.numerator.nterms(), left.denominator.nterms()))
}

fn integer_bit_length(value: &Integer) -> u128 {
    match value {
        Integer::Single(0) => 0,
        Integer::Single(number) => u128::from(number.unsigned_abs().ilog2() + 1),
        Integer::Double(0) => 0,
        Integer::Double(number) => u128::from(number.unsigned_abs().ilog2() + 1),
        Integer::Large(number) => u128::from(number.significant_bits()),
    }
}

fn checked_add(current: usize, addend: usize, resource: &'static str) -> Result<usize, ExactSparseProvenanceError> {
    current.checked_add(addend).ok_or(ExactSparseProvenanceError::ArithmeticOverflow { resource })
}

fn check_resource(resource: &'static str, requested: usize, limit: usize) -> Result<(), ExactSparseProvenanceError> {
    if requested > limit { return Err(ExactSparseProvenanceError::ResourceLimit { resource, requested: requested as u128, limit: limit as u128 }); }
    Ok(())
}

fn item_checksum(
    certificate: &ExactSparseElimination,
    request: ExactSparseProvenanceRequest,
    reductions: &[ExactSparseForwardReduction],
    weights: &[ExactSparseSourceWeight],
    config: ExactSparseProvenanceConfig,
) -> Result<u64, ExactSparseProvenanceError> {
    let mut hash = FNV1A64_OFFSET;
    hash_length_prefixed(&mut hash, EXACT_SPARSE_PROVENANCE_SCHEMA.as_bytes());
    hash_u64(&mut hash, certificate.checksum());
    hash_request(&mut hash, request);
    let mut bytes = 0_usize;
    hash_usize(&mut hash, reductions.len());
    for reduction in reductions {
        hash_usize(&mut hash, reduction.pivot_ordinal);
        hash_display_bounded(&mut hash, &reduction.factor, &mut bytes, config.max_checksum_coefficient_bytes)?;
    }
    hash_usize(&mut hash, weights.len());
    for weight in weights {
        hash_usize(&mut hash, weight.source_row_index);
        hash_display_bounded(&mut hash, &weight.coefficient, &mut bytes, config.max_checksum_coefficient_bytes)?;
    }
    Ok(hash)
}

fn bundle_checksum(certificate: &ExactSparseElimination, config: ExactSparseProvenanceConfig, items: &BTreeMap<ExactSparseProvenanceRequest, ExactSparseProvenanceItem>, stats: ExactSparseProvenanceStats) -> u64 {
    let mut hash = FNV1A64_OFFSET;
    hash_length_prefixed(&mut hash, EXACT_SPARSE_PROVENANCE_SCHEMA.as_bytes());
    hash_u64(&mut hash, certificate.source_checksum());
    hash_u64(&mut hash, certificate.checksum());
    hash_config(&mut hash, config);
    hash_usize(&mut hash, items.len());
    for (&request, item) in items { hash_request(&mut hash, request); hash_u64(&mut hash, item.checksum()); }
    hash_stats(&mut hash, stats);
    hash
}

fn hash_request(hash: &mut u64, request: ExactSparseProvenanceRequest) {
    match request {
        ExactSparseProvenanceRequest::PivotRow { pivot_ordinal } => { hash_u64(hash, 0); hash_usize(hash, pivot_ordinal); }
        ExactSparseProvenanceRequest::DependentZero { source_row_index } => { hash_u64(hash, 1); hash_usize(hash, source_row_index); }
    }
}

fn hash_config(hash: &mut u64, config: ExactSparseProvenanceConfig) {
    for value in [config.max_requests, config.max_forward_reductions, config.max_forward_updates, config.max_dag_node_visits, config.max_dag_edge_visits, config.max_pending_coefficients, config.max_pending_updates, config.max_coefficient_operations, config.max_cumulative_pair_products, config.max_matrix_term_updates, config.max_item_source_weights, config.max_retained_source_weights, config.max_retained_forward_reductions, config.max_retained_coefficient_terms, config.max_retained_coefficient_bytes, config.max_checksum_coefficient_bytes, config.max_coefficient_degree, config.max_coefficient_operation_terms, config.max_coefficient_dense_terms, config.max_integer_bits] { hash_usize(hash, value); }
}

fn hash_stats(hash: &mut u64, stats: ExactSparseProvenanceStats) {
    for value in [stats.requests, stats.pivot_items, stats.dependent_zero_items, stats.forward_reductions, stats.forward_updates, stats.dag_node_visits, stats.dag_edge_visits, stats.pending_updates, stats.maximum_pending_coefficients, stats.coefficient_multiplications, stats.coefficient_additions, stats.coefficient_divisions, stats.cumulative_pair_products, stats.matrix_term_updates, stats.retained_source_weights, stats.retained_forward_reductions, stats.retained_coefficient_terms, stats.retained_coefficient_bytes, stats.maximum_item_source_weights, stats.maximum_coefficient_degree] { hash_usize(hash, value); }
}

fn hash_u64(hash: &mut u64, value: u64) { hash_bytes(hash, &value.to_le_bytes()); }
fn hash_usize(hash: &mut u64, value: usize) { hash_u64(hash, value as u64); }
fn hash_length_prefixed(hash: &mut u64, bytes: &[u8]) { hash_usize(hash, bytes.len()); hash_bytes(hash, bytes); }
fn hash_bytes(hash: &mut u64, bytes: &[u8]) { for &byte in bytes { *hash ^= u64::from(byte); *hash = hash.wrapping_mul(FNV1A64_PRIME); } }

struct BoundedWriter<'a> { hash: Option<&'a mut u64>, length: usize, limit: usize }
impl fmt::Write for BoundedWriter<'_> {
    fn write_str(&mut self, value: &str) -> fmt::Result {
        let next = self.length.checked_add(value.len()).ok_or(fmt::Error)?;
        if next > self.limit { return Err(fmt::Error); }
        if let Some(hash) = self.hash.as_deref_mut() { hash_bytes(hash, value.as_bytes()); }
        self.length = next;
        Ok(())
    }
}

fn bounded_display_len(value: &Coefficient, used: usize, total_limit: usize, resource: &'static str) -> Result<usize, ExactSparseProvenanceError> {
    let mut writer = BoundedWriter { hash: None, length: 0, limit: total_limit.saturating_sub(used) };
    write!(&mut writer, "{value}").map_err(|_| ExactSparseProvenanceError::ResourceLimit { resource, requested: total_limit.saturating_add(1) as u128, limit: total_limit as u128 })?;
    Ok(writer.length)
}

fn hash_display_bounded(hash: &mut u64, value: &Coefficient, used: &mut usize, total_limit: usize) -> Result<(), ExactSparseProvenanceError> {
    let mut writer = BoundedWriter { hash: Some(hash), length: 0, limit: total_limit.saturating_sub(*used) };
    write!(&mut writer, "{value}").map_err(|_| ExactSparseProvenanceError::ResourceLimit { resource: "checksum coefficient bytes", requested: total_limit.saturating_add(1) as u128, limit: total_limit as u128 })?;
    *used = checked_add(*used, writer.length, "checksum coefficient bytes")?;
    hash_u64(writer.hash.as_deref_mut().expect("hash writer retains hash"), u64::MAX);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn add_scaled(target: &mut ExactSparseRow, source: &ExactSparseRow, scale: &Coefficient) {
        for (&column, coefficient) in source {
            let contribution = scale * coefficient;
            let updated = target.remove(&column).map_or(contribution.clone(), |current| &current + &contribution);
            if !updated.is_zero() { target.insert(column, updated); }
        }
    }

    fn fixture() -> (CoefficientContext, Vec<ExactSparseRow>, ExactSparseElimination) {
        let context = CoefficientContext::new(["d"]);
        let d = context.parameter("d").unwrap();
        let mut first = ExactSparseRow::new();
        first.insert(2, context.integer(2));
        first.insert(0, d.clone());
        let mut second = ExactSparseRow::new();
        second.insert(2, context.integer(6));
        second.insert(1, context.integer(3));
        second.insert(0, &context.integer(7) + &(&context.integer(3) * &d));
        let mut dependent = ExactSparseRow::new();
        add_scaled(&mut dependent, &first, &context.integer(2));
        add_scaled(&mut dependent, &second, &context.integer(-1));
        let rows = vec![first, second, dependent];
        let certificate = ExactSparseElimination::build(&context, &rows, 3, &[(0, 2), (1, 1)], Default::default()).unwrap();
        (context, rows, certificate)
    }

    #[test]
    fn expands_pivots_and_a_normalized_dependent_root() {
        let (context, rows, certificate) = fixture();
        let requests = [
            ExactSparseProvenanceRequest::PivotRow { pivot_ordinal: 0 },
            ExactSparseProvenanceRequest::PivotRow { pivot_ordinal: 1 },
            ExactSparseProvenanceRequest::DependentZero { source_row_index: 2 },
        ];
        let bundle = ExactSparseProvenanceBundle::build_authenticated(&certificate, &context, &rows, &requests, Default::default()).unwrap();
        let first = bundle.item(requests[0]).unwrap().source_weights();
        assert_eq!(first.len(), 1);
        assert_eq!(first[0].source_row_index(), 0);
        assert_eq!(first[0].coefficient(), &context.rational(crate::ExactRational::new(1, 2).unwrap()));
        let second = bundle.item(requests[1]).unwrap().source_weights();
        assert_eq!(second.iter().map(|weight| weight.source_row_index()).collect::<Vec<_>>(), vec![0, 1]);
        assert_eq!(second[0].coefficient(), &context.integer(-1));
        assert_eq!(second[1].coefficient(), &context.rational(crate::ExactRational::new(1, 3).unwrap()));
        let root = bundle.item(requests[2]).unwrap().source_weights();
        assert_eq!(root.iter().map(|weight| (weight.source_row_index(), weight.coefficient().clone())).collect::<Vec<_>>(), vec![(0, context.integer(-2)), (1, context.one()), (2, context.one())]);
        bundle.replay_authenticated(&certificate, &context, &rows).unwrap();
    }

    #[test]
    fn canonical_order_is_checksum_stable_and_duplicates_are_rejected() {
        let (context, rows, certificate) = fixture();
        let a = ExactSparseProvenanceRequest::PivotRow { pivot_ordinal: 1 };
        let b = ExactSparseProvenanceRequest::DependentZero { source_row_index: 2 };
        let first = ExactSparseProvenanceBundle::build_authenticated(&certificate, &context, &rows, &[a, b], Default::default()).unwrap();
        let second = ExactSparseProvenanceBundle::build_authenticated(&certificate, &context, &rows, &[b, a], Default::default()).unwrap();
        assert_eq!(first, second);
        assert!(matches!(ExactSparseProvenanceBundle::build_authenticated(&certificate, &context, &rows, &[a, a], Default::default()), Err(ExactSparseProvenanceError::DuplicateRequest(request)) if request == a));
    }

    #[test]
    fn rejects_non_dependent_root_and_enforces_tight_caps() {
        let (context, rows, certificate) = fixture();
        let root = ExactSparseProvenanceRequest::DependentZero { source_row_index: 0 };
        assert!(matches!(ExactSparseProvenanceBundle::build_one_authenticated(&certificate, &context, &rows, root, Default::default()), Err(ExactSparseProvenanceError::SourceRowIsPivotBase { .. })));
        let mut config = ExactSparseProvenanceConfig::default();
        config.max_dag_node_visits = 0;
        assert!(matches!(ExactSparseProvenanceBundle::build_one_authenticated(&certificate, &context, &rows, ExactSparseProvenanceRequest::PivotRow { pivot_ordinal: 0 }, config), Err(ExactSparseProvenanceError::ResourceLimit { resource: "DAG node visits", .. })));
    }

    #[test]
    fn replay_detects_retained_weight_tampering() {
        let (context, rows, certificate) = fixture();
        let request = ExactSparseProvenanceRequest::PivotRow { pivot_ordinal: 1 };
        let mut bundle = ExactSparseProvenanceBundle::build_one_authenticated(&certificate, &context, &rows, request, Default::default()).unwrap();
        match bundle.items.get_mut(&request).unwrap() {
            ExactSparseProvenanceItem::Pivot(item) => item.source_weights[0].coefficient = context.integer(99),
            ExactSparseProvenanceItem::DependentZero(_) => unreachable!(),
        }
        assert!(matches!(bundle.replay_authenticated(&certificate, &context, &rows), Err(ExactSparseProvenanceError::ReplayMismatch)));
    }
}
