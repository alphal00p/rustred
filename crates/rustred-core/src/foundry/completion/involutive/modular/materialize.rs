use std::collections::HashMap;
use std::mem::size_of;
use std::sync::Arc;

use crate::algebra::{IndexedCoefficient, IndexedCoefficientContext};

use super::ModularGuideError;
use super::arena::ModularCoefficientDag;
use super::error::{check_limit, checked_add, checked_mul, reserve_map, reserve_vec};
use super::limits::ExactMaterializerLimits;
use super::model::{
    AccumulatedDeltaId, CoeffNode, CoeffNodeId, CoeffRef, DagOwner, EvaluationKey, PhysicalDeltaId,
    RawCoeffRef,
};
use super::payload::try_coefficient_payload_weight;
use super::postorder::{BinaryOperation, PostorderFrame, UnaryOperation};

const ACCUMULATED_DELTAS: &str = "exact materializer accumulated deltas";
const ATTEMPTS: &str = "exact materializer attempts";
const BATCH_ROOTS: &str = "exact materializer batch roots";
const ACCUMULATED_DELTA_CELLS: &str = "exact materializer accumulated-delta coordinate cells";
const DELTA_COMPOSITIONS: &str = "exact materializer delta compositions";
const DELTA_COORDINATE_OPERATIONS: &str = "exact materializer delta-coordinate operations";
const TRAVERSAL_STEPS: &str = "exact materializer traversal steps";
const FRAME_PUSHES: &str = "exact materializer frame pushes";
const LIVE_FRAMES: &str = "exact materializer live frames";
const LIVE_VALUES: &str = "exact materializer live values";
const CACHED_VALUES: &str = "exact materializer cached values";
const EXACT_OPERATIONS: &str = "exact materializer Symbolica operations";
const RETAINED_TERMS: &str = "exact materializer retained coefficient terms";
const RETAINED_EXPONENT_CELLS: &str = "exact materializer retained coefficient exponent cells";
const RETAINED_BYTES: &str = "exact materializer retained coefficient bytes";
const OUTPUT_TERMS: &str = "exact materializer output coefficient terms";
const OUTPUT_EXPONENT_CELLS: &str = "exact materializer output coefficient exponent cells";
const OUTPUT_BYTES: &str = "exact materializer output coefficient bytes";
const OUTPUT_VALUES: &str = "exact materializer output values";

/// Deterministic cumulative accounting for one caller-owned sequence of cold
/// exact fallbacks.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(super) struct ExactMaterializationCensus {
    traversal_steps: usize,
    frame_pushes: usize,
    peak_live_frames: usize,
    peak_live_values: usize,
    cache_hits: usize,
    exact_operations: usize,
    delta_compositions: usize,
    delta_coordinate_operations: usize,
    cached_values: usize,
    retained_terms: usize,
    retained_exponent_cells: usize,
    retained_bytes: usize,
    output_terms: usize,
    output_exponent_cells: usize,
    output_bytes: usize,
    output_values: usize,
}

impl ExactMaterializationCensus {
    pub(super) const fn traversal_steps(self) -> usize {
        self.traversal_steps
    }

    pub(super) const fn frame_pushes(self) -> usize {
        self.frame_pushes
    }

    pub(super) const fn peak_live_frames(self) -> usize {
        self.peak_live_frames
    }

    pub(super) const fn peak_live_values(self) -> usize {
        self.peak_live_values
    }

    pub(super) const fn cache_hits(self) -> usize {
        self.cache_hits
    }

    pub(super) const fn exact_operations(self) -> usize {
        self.exact_operations
    }

    pub(super) const fn delta_compositions(self) -> usize {
        self.delta_compositions
    }

    pub(super) const fn delta_coordinate_operations(self) -> usize {
        self.delta_coordinate_operations
    }

    pub(super) const fn cached_values(self) -> usize {
        self.cached_values
    }

    pub(super) const fn retained_terms(self) -> usize {
        self.retained_terms
    }

    pub(super) const fn retained_exponent_cells(self) -> usize {
        self.retained_exponent_cells
    }

    pub(super) const fn retained_bytes(self) -> usize {
        self.retained_bytes
    }

    pub(super) const fn output_terms(self) -> usize {
        self.output_terms
    }

    pub(super) const fn output_exponent_cells(self) -> usize {
        self.output_exponent_cells
    }

    pub(super) const fn output_bytes(self) -> usize {
        self.output_bytes
    }

    pub(super) const fn output_values(self) -> usize {
        self.output_values
    }
}

/// One root-bound exact coefficient and the cumulative budget snapshot at the
/// point it was produced.
#[derive(Clone, Debug)]
pub(super) struct ExactMaterialization {
    dag_owner: DagOwner,
    context_fingerprint: Arc<String>,
    root: CoeffRef,
    value: Arc<IndexedCoefficient>,
    census: ExactMaterializationCensus,
}

/// An all-or-nothing ordered exact fallback batch.  Every contained value is
/// independently root-bound, while construction uses one iterative cache for
/// the complete input sequence. No entry exists unless every root completed.
#[derive(Debug)]
pub(super) struct ExactMaterializationBatch {
    dag_owner: DagOwner,
    context_fingerprint: Arc<String>,
    roots: Box<[CoeffRef]>,
    materializations: Box<[ExactMaterialization]>,
    census: ExactMaterializationCensus,
}

/// Caller-owned cumulative budget for exact-lazy fallback.
///
/// Limits are fixed at construction and every attempt is charged before DAG
/// or context ingress. Reusing this value is therefore mandatory for a
/// completion campaign that resolves more than one root; failures cannot
/// reset work already spent.
#[derive(Debug)]
pub(super) struct ExactMaterializationBudget {
    limits: ExactMaterializerLimits,
    attempts: usize,
    census: ExactMaterializationCensus,
}

impl ExactMaterializationBudget {
    pub(super) const fn new(limits: ExactMaterializerLimits) -> Self {
        Self {
            limits,
            attempts: 0,
            census: ExactMaterializationCensus {
                traversal_steps: 0,
                frame_pushes: 0,
                peak_live_frames: 0,
                peak_live_values: 0,
                cache_hits: 0,
                exact_operations: 0,
                delta_compositions: 0,
                delta_coordinate_operations: 0,
                cached_values: 0,
                retained_terms: 0,
                retained_exponent_cells: 0,
                retained_bytes: 0,
                output_terms: 0,
                output_exponent_cells: 0,
                output_bytes: 0,
                output_values: 0,
            },
        }
    }

    pub(super) const fn attempts(&self) -> usize {
        self.attempts
    }

    pub(super) const fn census(&self) -> ExactMaterializationCensus {
        self.census
    }

    fn try_start_attempt(&mut self) -> Result<(), ModularGuideError> {
        charge(ATTEMPTS, &mut self.attempts, 1, self.limits.max_attempts)
    }
}

impl ExactMaterialization {
    pub(super) fn value(&self) -> &IndexedCoefficient {
        &self.value
    }

    pub(super) const fn census(&self) -> ExactMaterializationCensus {
        self.census
    }

    pub(super) fn owns(
        &self,
        dag: &ModularCoefficientDag,
        context: &IndexedCoefficientContext,
        root: &CoeffRef,
    ) -> bool {
        self.dag_owner.belongs_to(dag.owner())
            && context.owns_fingerprint(&self.context_fingerprint)
            && self.root == *root
            && dag.raw(&self.root).is_ok()
            && dag.raw(root).is_ok()
    }
}

impl ExactMaterializationBatch {
    pub(super) fn roots(&self) -> &[CoeffRef] {
        &self.roots
    }

    pub(super) fn materializations(&self) -> &[ExactMaterialization] {
        &self.materializations
    }

    pub(super) fn into_materializations(self) -> Box<[ExactMaterialization]> {
        self.materializations
    }

    pub(super) const fn census(&self) -> ExactMaterializationCensus {
        self.census
    }

    pub(super) fn owns(
        &self,
        dag: &ModularCoefficientDag,
        context: &IndexedCoefficientContext,
        roots: &[CoeffRef],
    ) -> bool {
        self.dag_owner.belongs_to(dag.owner())
            && context.owns_fingerprint(&self.context_fingerprint)
            && self.roots.as_ref() == roots
            && self.materializations.len() == roots.len()
            && self
                .materializations
                .iter()
                .zip(roots)
                .all(|(materialization, root)| materialization.owns(dag, context, root))
    }
}

/// Materialize one owned coefficient root through the authenticated indexed
/// context and its Symbolica-backed arithmetic.  This is deliberately private
/// to exact-lazy support fallback; it does not create an executable rule or an
/// artifact value.
pub(super) fn try_materialize_exact(
    dag: &ModularCoefficientDag,
    context: &IndexedCoefficientContext,
    root: &CoeffRef,
    budget: &mut ExactMaterializationBudget,
) -> Result<ExactMaterialization, ModularGuideError> {
    budget.try_start_attempt()?;
    let requested_outputs = checked_add(OUTPUT_VALUES, budget.census.output_values, 1)?;
    check_limit(
        OUTPUT_VALUES,
        requested_outputs,
        budget.limits.max_output_values,
    )?;
    if !dag.owns_context(context) {
        return Err(ModularGuideError::WrongIndexedContext);
    }
    let raw_root = dag.raw(root)?;
    let binding = ExactMaterializationBinding {
        dag_owner: dag.owner().clone(),
        context_fingerprint: context.fingerprint_owner(),
        root: root.clone(),
    };
    let mut materializer = ExactMaterializer::try_new(dag, context, budget)?;
    let value = materializer.try_materialize(raw_root)?;
    materializer.finish(value, binding)
}

/// Materialize a complete ordered root batch through one Symbolica-backed
/// iterative memo table. The attempt is charged before ownership ingress and
/// no root-bound value is returned unless every root and every output-payload
/// check succeeds.
pub(super) fn try_materialize_exact_batch(
    dag: &ModularCoefficientDag,
    context: &IndexedCoefficientContext,
    roots: &[CoeffRef],
    budget: &mut ExactMaterializationBudget,
) -> Result<ExactMaterializationBatch, ModularGuideError> {
    budget.try_start_attempt()?;
    check_limit(BATCH_ROOTS, roots.len(), budget.limits.max_batch_roots)?;
    let requested_outputs = checked_add(OUTPUT_VALUES, budget.census.output_values, roots.len())?;
    check_limit(
        OUTPUT_VALUES,
        requested_outputs,
        budget.limits.max_output_values,
    )?;
    if !dag.owns_context(context) {
        return Err(ModularGuideError::WrongIndexedContext);
    }

    let mut owned_roots = Vec::new();
    reserve_vec(&mut owned_roots, roots.len(), BATCH_ROOTS)?;
    let mut raw_roots = Vec::new();
    reserve_vec(&mut raw_roots, roots.len(), BATCH_ROOTS)?;
    for root in roots {
        raw_roots.push(dag.raw(root)?);
        owned_roots.push(root.clone());
    }

    let binding = ExactMaterializationBatchBinding {
        dag_owner: dag.owner().clone(),
        context_fingerprint: context.fingerprint_owner(),
        roots: owned_roots.into_boxed_slice(),
    };
    let mut materializer = ExactMaterializer::try_new(dag, context, budget)?;
    let mut values = Vec::new();
    reserve_vec(&mut values, raw_roots.len(), OUTPUT_VALUES)?;
    for raw_root in raw_roots {
        values.push(materializer.try_materialize(raw_root)?);
    }
    materializer.finish_batch(values, binding)
}

struct ExactMaterializationBinding {
    dag_owner: DagOwner,
    context_fingerprint: Arc<String>,
    root: CoeffRef,
}

struct ExactMaterializationBatchBinding {
    dag_owner: DagOwner,
    context_fingerprint: Arc<String>,
    roots: Box<[CoeffRef]>,
}

struct ExactMaterializer<'dag, 'context, 'budget> {
    dag: &'dag ModularCoefficientDag,
    context: &'context IndexedCoefficientContext,
    limits: ExactMaterializerLimits,
    budget: &'budget mut ExactMaterializationBudget,
    accumulated_deltas: Vec<Arc<Vec<i64>>>,
    accumulated_delta_lookup: HashMap<Arc<Vec<i64>>, AccumulatedDeltaId>,
    accumulated_delta_cells: usize,
    value_cache: HashMap<EvaluationKey, Arc<IndexedCoefficient>>,
}

impl<'dag, 'context, 'budget> ExactMaterializer<'dag, 'context, 'budget> {
    fn try_new(
        dag: &'dag ModularCoefficientDag,
        context: &'context IndexedCoefficientContext,
        budget: &'budget mut ExactMaterializationBudget,
    ) -> Result<Self, ModularGuideError> {
        let limits = budget.limits;
        check_limit(ACCUMULATED_DELTAS, 1, limits.max_accumulated_deltas)?;
        check_limit(
            ACCUMULATED_DELTA_CELLS,
            dag.index_count(),
            limits.max_accumulated_delta_coordinate_cells,
        )?;
        let mut zero = Vec::new();
        reserve_vec(&mut zero, dag.index_count(), ACCUMULATED_DELTA_CELLS)?;
        zero.resize(dag.index_count(), 0);
        let zero = Arc::new(zero);
        let zero_id = AccumulatedDeltaId::try_new(0).expect("zero accumulated delta ID is fixed");
        let mut accumulated_deltas = Vec::new();
        reserve_vec(&mut accumulated_deltas, 1, ACCUMULATED_DELTAS)?;
        accumulated_deltas.push(Arc::clone(&zero));
        let mut accumulated_delta_lookup = HashMap::new();
        reserve_map(&mut accumulated_delta_lookup, 1, ACCUMULATED_DELTAS)?;
        accumulated_delta_lookup.insert(zero, zero_id);
        Ok(Self {
            dag,
            context,
            limits,
            budget,
            accumulated_deltas,
            accumulated_delta_lookup,
            accumulated_delta_cells: dag.index_count(),
            value_cache: HashMap::new(),
        })
    }

    fn try_materialize(
        &mut self,
        root: RawCoeffRef,
    ) -> Result<Arc<IndexedCoefficient>, ModularGuideError> {
        let mut frames = Vec::new();
        let mut values = Vec::new();
        self.try_push_frame(
            &mut frames,
            PostorderFrame::Enter {
                reference: root,
                inherited: self.zero_delta_id(),
            },
        )?;

        while let Some(frame) = frames.pop() {
            match frame {
                PostorderFrame::Enter {
                    reference,
                    inherited,
                } => {
                    let translation = self.try_compose_delta(inherited, reference.translation)?;
                    let key = EvaluationKey {
                        node: reference.node,
                        translation,
                    };
                    if let Some(value) = self.value_cache.get(&key).cloned() {
                        self.budget.census.cache_hits = checked_add(
                            "exact materializer cache hits",
                            self.budget.census.cache_hits,
                            1,
                        )?;
                        self.try_push_value(&mut values, value)?;
                        continue;
                    }
                    charge(
                        TRAVERSAL_STEPS,
                        &mut self.budget.census.traversal_steps,
                        1,
                        self.limits.max_traversal_steps,
                    )?;
                    match self.dag.node(reference.node)?.clone() {
                        CoeffNode::Zero => {
                            let value = Arc::new(self.context.zero());
                            self.try_finish_value(key, value, &mut values)?;
                        }
                        CoeffNode::One => {
                            let value = Arc::new(self.context.one());
                            self.try_finish_value(key, value, &mut values)?;
                        }
                        CoeffNode::ExactLeaf(leaf_id) => {
                            let leaf = self.dag.exact_leaf(leaf_id)?;
                            let value = if translation == self.zero_delta_id() {
                                Arc::clone(leaf)
                            } else {
                                self.charge_exact_operation()?;
                                Arc::new(self.context.translate_sealed(
                                    leaf,
                                    self.delta(translation)?,
                                    self.limits.indexed_algebra,
                                )?)
                            };
                            self.try_finish_value(key, value, &mut values)?;
                        }
                        CoeffNode::Neg(child) => {
                            self.try_push_frame(
                                &mut frames,
                                PostorderFrame::FinishUnary {
                                    key,
                                    operation: UnaryOperation::Neg,
                                },
                            )?;
                            self.try_push_frame(
                                &mut frames,
                                PostorderFrame::Enter {
                                    reference: child,
                                    inherited: translation,
                                },
                            )?;
                        }
                        CoeffNode::Inv(child) => {
                            self.try_push_frame(
                                &mut frames,
                                PostorderFrame::FinishUnary {
                                    key,
                                    operation: UnaryOperation::Inv,
                                },
                            )?;
                            self.try_push_frame(
                                &mut frames,
                                PostorderFrame::Enter {
                                    reference: child,
                                    inherited: translation,
                                },
                            )?;
                        }
                        CoeffNode::Add(left, right) => {
                            self.try_push_frame(
                                &mut frames,
                                PostorderFrame::AfterLeft {
                                    key,
                                    right,
                                    inherited: translation,
                                    operation: BinaryOperation::Add,
                                },
                            )?;
                            self.try_push_frame(
                                &mut frames,
                                PostorderFrame::Enter {
                                    reference: left,
                                    inherited: translation,
                                },
                            )?;
                        }
                        CoeffNode::Mul(left, right) => {
                            self.try_push_frame(
                                &mut frames,
                                PostorderFrame::AfterLeft {
                                    key,
                                    right,
                                    inherited: translation,
                                    operation: BinaryOperation::Mul,
                                },
                            )?;
                            self.try_push_frame(
                                &mut frames,
                                PostorderFrame::Enter {
                                    reference: left,
                                    inherited: translation,
                                },
                            )?;
                        }
                    }
                }
                PostorderFrame::FinishUnary { key, operation } => {
                    let child = values.pop().ok_or(ModularGuideError::Invariant {
                        detail: "iterative exact unary operation has no child value",
                    })?;
                    self.charge_exact_operation()?;
                    let value = match operation {
                        UnaryOperation::Neg => {
                            let child = self.context.bind_sealed(&child)?;
                            self.context.neg_bound_with_limits(
                                child,
                                self.limits.indexed_algebra.exact_algebra,
                            )?
                        }
                        UnaryOperation::Inv => {
                            if child.is_zero() {
                                return Err(ModularGuideError::ExactZeroInverse { node: key.node });
                            }
                            let one = self.context.one();
                            let one = self.context.bind_sealed(&one)?;
                            let child = self.context.bind_sealed(&child)?;
                            self.context.div_bound_with_limits(
                                one,
                                child,
                                self.limits.indexed_algebra.exact_algebra,
                            )?
                        }
                    };
                    self.try_finish_value(key, Arc::new(value), &mut values)?;
                }
                PostorderFrame::AfterLeft {
                    key,
                    right,
                    inherited,
                    operation,
                } => {
                    if values.is_empty() {
                        return Err(ModularGuideError::Invariant {
                            detail: "iterative exact binary operation has no left value",
                        });
                    }
                    self.try_push_frame(
                        &mut frames,
                        PostorderFrame::FinishBinary { key, operation },
                    )?;
                    self.try_push_frame(
                        &mut frames,
                        PostorderFrame::Enter {
                            reference: right,
                            inherited,
                        },
                    )?;
                }
                PostorderFrame::FinishBinary { key, operation } => {
                    let right = values.pop().ok_or(ModularGuideError::Invariant {
                        detail: "iterative exact binary operation has no right value",
                    })?;
                    let left = values.pop().ok_or(ModularGuideError::Invariant {
                        detail: "iterative exact binary operation lost its left value",
                    })?;
                    self.charge_exact_operation()?;
                    let left = self.context.bind_sealed(&left)?;
                    let right = self.context.bind_sealed(&right)?;
                    let value = match operation {
                        BinaryOperation::Add => self.context.add_bound_with_limits(
                            left,
                            right,
                            self.limits.indexed_algebra.exact_algebra,
                        )?,
                        BinaryOperation::Mul => self.context.mul_bound_with_limits(
                            left,
                            right,
                            self.limits.indexed_algebra.exact_algebra,
                        )?,
                    };
                    self.try_finish_value(key, Arc::new(value), &mut values)?;
                }
            }
        }

        if values.len() != 1 {
            return Err(ModularGuideError::Invariant {
                detail: "iterative exact materialization did not produce exactly one root value",
            });
        }
        values.pop().ok_or(ModularGuideError::Invariant {
            detail: "iterative exact materialization lost its root value",
        })
    }

    fn finish(
        self,
        value: Arc<IndexedCoefficient>,
        binding: ExactMaterializationBinding,
    ) -> Result<ExactMaterialization, ModularGuideError> {
        let weight = coefficient_weight(&value)?;
        check_limit(OUTPUT_TERMS, weight.terms, self.limits.max_output_terms)?;
        check_limit(
            OUTPUT_EXPONENT_CELLS,
            weight.exponent_cells,
            self.limits.max_output_exponent_cells,
        )?;
        check_limit(
            OUTPUT_BYTES,
            weight.bytes,
            self.limits.max_output_retained_bytes,
        )?;
        let output_terms =
            checked_add(OUTPUT_TERMS, self.budget.census.output_terms, weight.terms)?;
        let output_exponent_cells = checked_add(
            OUTPUT_EXPONENT_CELLS,
            self.budget.census.output_exponent_cells,
            weight.exponent_cells,
        )?;
        let output_bytes =
            checked_add(OUTPUT_BYTES, self.budget.census.output_bytes, weight.bytes)?;
        let output_values = checked_add(OUTPUT_VALUES, self.budget.census.output_values, 1)?;
        check_limit(OUTPUT_TERMS, output_terms, self.limits.max_output_terms)?;
        check_limit(
            OUTPUT_EXPONENT_CELLS,
            output_exponent_cells,
            self.limits.max_output_exponent_cells,
        )?;
        check_limit(
            OUTPUT_BYTES,
            output_bytes,
            self.limits.max_output_retained_bytes,
        )?;
        check_limit(OUTPUT_VALUES, output_values, self.limits.max_output_values)?;
        self.budget.census.output_terms = output_terms;
        self.budget.census.output_exponent_cells = output_exponent_cells;
        self.budget.census.output_bytes = output_bytes;
        self.budget.census.output_values = output_values;
        Ok(ExactMaterialization {
            dag_owner: binding.dag_owner,
            context_fingerprint: binding.context_fingerprint,
            root: binding.root,
            value,
            census: self.budget.census,
        })
    }

    fn finish_batch(
        self,
        values: Vec<Arc<IndexedCoefficient>>,
        binding: ExactMaterializationBatchBinding,
    ) -> Result<ExactMaterializationBatch, ModularGuideError> {
        if values.len() != binding.roots.len() {
            return Err(ModularGuideError::Invariant {
                detail: "exact materializer batch lost ordered root/value correspondence",
            });
        }

        let mut added_terms = 0usize;
        let mut added_exponent_cells = 0usize;
        let mut added_bytes = 0usize;
        for value in &values {
            let weight = coefficient_weight(value)?;
            added_terms = checked_add(OUTPUT_TERMS, added_terms, weight.terms)?;
            added_exponent_cells = checked_add(
                OUTPUT_EXPONENT_CELLS,
                added_exponent_cells,
                weight.exponent_cells,
            )?;
            added_bytes = checked_add(OUTPUT_BYTES, added_bytes, weight.bytes)?;
        }
        check_limit(OUTPUT_TERMS, added_terms, self.limits.max_output_terms)?;
        check_limit(
            OUTPUT_EXPONENT_CELLS,
            added_exponent_cells,
            self.limits.max_output_exponent_cells,
        )?;
        check_limit(
            OUTPUT_BYTES,
            added_bytes,
            self.limits.max_output_retained_bytes,
        )?;
        check_limit(OUTPUT_VALUES, values.len(), self.limits.max_output_values)?;

        let output_terms = checked_add(OUTPUT_TERMS, self.budget.census.output_terms, added_terms)?;
        let output_exponent_cells = checked_add(
            OUTPUT_EXPONENT_CELLS,
            self.budget.census.output_exponent_cells,
            added_exponent_cells,
        )?;
        let output_bytes = checked_add(OUTPUT_BYTES, self.budget.census.output_bytes, added_bytes)?;
        let output_values = checked_add(
            OUTPUT_VALUES,
            self.budget.census.output_values,
            values.len(),
        )?;
        check_limit(OUTPUT_TERMS, output_terms, self.limits.max_output_terms)?;
        check_limit(
            OUTPUT_EXPONENT_CELLS,
            output_exponent_cells,
            self.limits.max_output_exponent_cells,
        )?;
        check_limit(
            OUTPUT_BYTES,
            output_bytes,
            self.limits.max_output_retained_bytes,
        )?;
        check_limit(OUTPUT_VALUES, output_values, self.limits.max_output_values)?;

        let mut materializations = Vec::new();
        reserve_vec(&mut materializations, values.len(), OUTPUT_VALUES)?;
        self.budget.census.output_terms = output_terms;
        self.budget.census.output_exponent_cells = output_exponent_cells;
        self.budget.census.output_bytes = output_bytes;
        self.budget.census.output_values = output_values;
        let census = self.budget.census;
        let dag_owner = binding.dag_owner;
        let context_fingerprint = binding.context_fingerprint;
        let roots = binding.roots;
        materializations.extend(roots.iter().cloned().zip(values).map(|(root, value)| {
            ExactMaterialization {
                dag_owner: dag_owner.clone(),
                context_fingerprint: Arc::clone(&context_fingerprint),
                root,
                value,
                census,
            }
        }));
        Ok(ExactMaterializationBatch {
            dag_owner,
            context_fingerprint,
            roots,
            materializations: materializations.into_boxed_slice(),
            census,
        })
    }

    fn try_finish_value(
        &mut self,
        key: EvaluationKey,
        value: Arc<IndexedCoefficient>,
        values: &mut Vec<Arc<IndexedCoefficient>>,
    ) -> Result<(), ModularGuideError> {
        self.try_cache_value(key, Arc::clone(&value))?;
        self.try_push_value(values, value)
    }

    fn try_cache_value(
        &mut self,
        key: EvaluationKey,
        value: Arc<IndexedCoefficient>,
    ) -> Result<(), ModularGuideError> {
        let requested_values = checked_add(CACHED_VALUES, self.budget.census.cached_values, 1)?;
        check_limit(
            CACHED_VALUES,
            requested_values,
            self.limits.max_cached_values,
        )?;
        let weight = coefficient_weight(&value)?;
        let retained_terms = checked_add(
            RETAINED_TERMS,
            self.budget.census.retained_terms,
            weight.terms,
        )?;
        let retained_exponent_cells = checked_add(
            RETAINED_EXPONENT_CELLS,
            self.budget.census.retained_exponent_cells,
            weight.exponent_cells,
        )?;
        let retained_bytes = checked_add(
            RETAINED_BYTES,
            self.budget.census.retained_bytes,
            weight.bytes,
        )?;
        check_limit(
            RETAINED_TERMS,
            retained_terms,
            self.limits.max_retained_terms,
        )?;
        check_limit(
            RETAINED_EXPONENT_CELLS,
            retained_exponent_cells,
            self.limits.max_retained_exponent_cells,
        )?;
        check_limit(
            RETAINED_BYTES,
            retained_bytes,
            self.limits.max_retained_bytes,
        )?;
        reserve_map(&mut self.value_cache, 1, CACHED_VALUES)?;
        if self.value_cache.insert(key, value).is_some() {
            return Err(ModularGuideError::Invariant {
                detail: "exact materializer tried to replace a cached value",
            });
        }
        self.budget.census.cached_values = requested_values;
        self.budget.census.retained_terms = retained_terms;
        self.budget.census.retained_exponent_cells = retained_exponent_cells;
        self.budget.census.retained_bytes = retained_bytes;
        Ok(())
    }

    fn try_push_frame(
        &mut self,
        frames: &mut Vec<PostorderFrame>,
        frame: PostorderFrame,
    ) -> Result<(), ModularGuideError> {
        charge(
            FRAME_PUSHES,
            &mut self.budget.census.frame_pushes,
            1,
            self.limits.max_frame_pushes,
        )?;
        let requested = checked_add(LIVE_FRAMES, frames.len(), 1)?;
        check_limit(LIVE_FRAMES, requested, self.limits.max_live_frames)?;
        reserve_vec(frames, 1, LIVE_FRAMES)?;
        frames.push(frame);
        self.budget.census.peak_live_frames = self.budget.census.peak_live_frames.max(requested);
        Ok(())
    }

    fn try_push_value(
        &mut self,
        values: &mut Vec<Arc<IndexedCoefficient>>,
        value: Arc<IndexedCoefficient>,
    ) -> Result<(), ModularGuideError> {
        let requested = checked_add(LIVE_VALUES, values.len(), 1)?;
        check_limit(LIVE_VALUES, requested, self.limits.max_live_values)?;
        reserve_vec(values, 1, LIVE_VALUES)?;
        values.push(value);
        self.budget.census.peak_live_values = self.budget.census.peak_live_values.max(requested);
        Ok(())
    }

    fn try_compose_delta(
        &mut self,
        inherited: AccumulatedDeltaId,
        added: PhysicalDeltaId,
    ) -> Result<AccumulatedDeltaId, ModularGuideError> {
        let added = self.dag.delta(added)?;
        if added.iter().all(|&value| value == 0) {
            return Ok(inherited);
        }
        charge(
            DELTA_COMPOSITIONS,
            &mut self.budget.census.delta_compositions,
            1,
            self.limits.max_delta_compositions,
        )?;
        charge(
            DELTA_COORDINATE_OPERATIONS,
            &mut self.budget.census.delta_coordinate_operations,
            self.dag.index_count(),
            self.limits.max_delta_coordinate_operations,
        )?;
        let inherited_values = self.delta(inherited)?;
        let mut result = Vec::new();
        reserve_vec(&mut result, self.dag.index_count(), ACCUMULATED_DELTA_CELLS)?;
        for (&left, &right) in inherited_values.iter().zip(added) {
            let value =
                left.checked_add(right)
                    .ok_or(ModularGuideError::ResourceCountOverflow {
                        resource: "exact materializer accumulated translation coordinate",
                    })?;
            if value.unsigned_abs() > self.limits.max_absolute_physical_translation {
                return Err(ModularGuideError::ResourceLimit {
                    resource: "exact materializer absolute accumulated translation",
                    requested: usize::try_from(value.unsigned_abs()).unwrap_or(usize::MAX),
                    limit: usize::try_from(self.limits.max_absolute_physical_translation)
                        .unwrap_or(usize::MAX),
                });
            }
            result.push(value);
        }
        if let Some(&id) = self.accumulated_delta_lookup.get(&result) {
            return Ok(id);
        }
        let requested_deltas = checked_add(ACCUMULATED_DELTAS, self.accumulated_deltas.len(), 1)?;
        check_limit(
            ACCUMULATED_DELTAS,
            requested_deltas,
            self.limits.max_accumulated_deltas,
        )?;
        let requested_cells = checked_add(
            ACCUMULATED_DELTA_CELLS,
            self.accumulated_delta_cells,
            self.dag.index_count(),
        )?;
        check_limit(
            ACCUMULATED_DELTA_CELLS,
            requested_cells,
            self.limits.max_accumulated_delta_coordinate_cells,
        )?;
        let id = AccumulatedDeltaId::try_new(self.accumulated_deltas.len()).ok_or(
            ModularGuideError::IdentifierNotRepresentable {
                resource: "exact materializer accumulated-delta identifier",
                value: self.accumulated_deltas.len(),
            },
        )?;
        reserve_vec(&mut self.accumulated_deltas, 1, ACCUMULATED_DELTAS)?;
        reserve_map(&mut self.accumulated_delta_lookup, 1, ACCUMULATED_DELTAS)?;
        let result = Arc::new(result);
        self.accumulated_delta_lookup
            .insert(Arc::clone(&result), id);
        self.accumulated_deltas.push(result);
        self.accumulated_delta_cells = requested_cells;
        Ok(id)
    }

    fn delta(&self, id: AccumulatedDeltaId) -> Result<&[i64], ModularGuideError> {
        self.accumulated_deltas
            .get(id.as_usize())
            .map(|delta| delta.as_slice())
            .ok_or(ModularGuideError::Invariant {
                detail: "exact materializer names an absent accumulated translation",
            })
    }

    fn zero_delta_id(&self) -> AccumulatedDeltaId {
        AccumulatedDeltaId::try_new(0).expect("zero accumulated delta ID is fixed")
    }

    fn charge_exact_operation(&mut self) -> Result<(), ModularGuideError> {
        charge(
            EXACT_OPERATIONS,
            &mut self.budget.census.exact_operations,
            1,
            self.limits.max_exact_operations,
        )
    }
}

#[derive(Clone, Copy)]
struct CoefficientWeight {
    terms: usize,
    exponent_cells: usize,
    bytes: usize,
}

fn coefficient_weight(
    coefficient: &IndexedCoefficient,
) -> Result<CoefficientWeight, ModularGuideError> {
    let payload = try_coefficient_payload_weight(coefficient)?;
    let bytes = [
        payload.bytes,
        size_of::<Arc<IndexedCoefficient>>(),
        size_of::<EvaluationKey>(),
    ]
    .into_iter()
    .try_fold(0usize, |total, value| {
        checked_add(RETAINED_BYTES, total, value)
    })?;
    Ok(CoefficientWeight {
        terms: payload.terms,
        exponent_cells: payload.exponent_cells,
        bytes,
    })
}

fn charge(
    resource: &'static str,
    current: &mut usize,
    amount: usize,
    limit: usize,
) -> Result<(), ModularGuideError> {
    let requested = checked_add(resource, *current, amount)?;
    check_limit(resource, requested, limit)?;
    *current = requested;
    Ok(())
}
