use std::collections::HashMap;
use std::sync::Arc;

use symbolica::domains::finite_field::{FiniteFieldCore, FiniteFieldElement, ToFiniteField, Zp64};
use symbolica::domains::{Field, Ring, RingOps};
use symbolica::prelude::Integer;

use crate::algebra::IndexedCoefficientContext;

use super::ModularGuideError;
use super::arena::ModularCoefficientDag;
use super::error::{check_limit, checked_add, checked_mul, reserve_map, reserve_vec};
use super::limits::ModularGuideLimits;
use super::model::{
    AccumulatedDeltaId, CoeffNode, CoeffNodeId, CoeffRef, DagOwner, EvaluationKey,
    ModularEvaluationBatch, ModularEvaluationQuery, ModularImage, ModularProbeCensus,
    ModularProbeIdentity, ModularQueryRole, ModularZeroEvidence, PhysicalDeltaId, RawCoeffRef,
};
use super::postorder::{BinaryOperation, PostorderFrame, UnaryOperation};

const PROBE_POINT_COORDINATES: &str = "modular guide probe point coordinates";
const PROBE_DELTAS: &str = "modular guide accumulated deltas";
const PROBE_DELTA_CELLS: &str = "modular guide accumulated-delta coordinate cells";
const TRANSLATED_POINTS: &str = "modular guide translated points";
const TRANSLATED_POINT_CELLS: &str = "modular guide translated-point coordinate cells";
const CACHED_VALUES: &str = "modular guide cached values";
const BATCH_IMAGES: &str = "modular guide completed batch images";
const PROBE_QUERIES: &str = "modular guide probe queries";
const DELTA_COMPOSITIONS: &str = "modular guide delta compositions";
const DELTA_COORDINATE_OPERATIONS: &str = "modular guide delta-coordinate operations";
const EVALUATION_STEPS: &str = "modular guide evaluation steps";
const EVALUATION_FRAME_PUSHES: &str = "modular guide evaluation frame pushes";
const LIVE_EVALUATION_FRAMES: &str = "modular guide live evaluation frames";
const LIVE_EVALUATION_VALUES: &str = "modular guide live evaluation values";
const EXACT_LEAF_EVALUATIONS: &str = "modular guide exact-leaf evaluations";
const EXACT_LEAF_TERMS: &str = "modular guide exact-leaf terms evaluated";
const EXACT_LEAF_EXPONENT_CELLS: &str = "modular guide exact-leaf exponent cells evaluated";

/// A rejected consumed probe.  It deliberately carries no scalar image, but
/// retains all work charged before the rejection so a higher-level cumulative
/// budget cannot erase singular or exhausted attempts.
#[derive(Debug)]
pub(super) struct RejectedProbeReport {
    error: ModularGuideError,
    census: ModularProbeCensus,
}

impl RejectedProbeReport {
    pub(super) const fn new(error: ModularGuideError, census: ModularProbeCensus) -> Self {
        Self { error, census }
    }

    pub(super) const fn error(&self) -> &ModularGuideError {
        &self.error
    }

    pub(super) const fn census(&self) -> ModularProbeCensus {
        self.census
    }

    pub(super) fn into_error(self) -> ModularGuideError {
        self.error
    }
}

/// One independent finite-field lane.
///
/// A failure poisons the complete lane and clears all derived caches.  This
/// prevents a partially evaluated row from being mistaken for a valid trace;
/// callers must construct a fresh probe at another `(prime, point)` pair.
#[derive(Debug)]
pub(super) struct ModularProbe {
    identity: Arc<ModularProbeIdentity>,
    dag_owner: DagOwner,
    context_fingerprint: Arc<String>,
    base_parameter_count: usize,
    index_count: usize,
    limits: ModularGuideLimits,
    field: Zp64,
    base_point: Arc<Vec<FiniteFieldElement<u64>>>,
    accumulated_deltas: Vec<Arc<Vec<i64>>>,
    accumulated_delta_lookup: HashMap<Arc<Vec<i64>>, AccumulatedDeltaId>,
    accumulated_delta_cells: usize,
    translated_points: HashMap<AccumulatedDeltaId, Arc<Vec<FiniteFieldElement<u64>>>>,
    translated_point_cells: usize,
    value_cache: HashMap<EvaluationKey, FiniteFieldElement<u64>>,
    census: ModularProbeCensus,
    rejected: bool,
}

impl ModularProbe {
    pub(super) fn try_new(
        dag: &ModularCoefficientDag,
        context: &IndexedCoefficientContext,
        ordinal: usize,
        modulus: u64,
        full_integer_point: &[i64],
        limits: ModularGuideLimits,
    ) -> Result<Self, ModularGuideError> {
        if !dag.owns_context(context) {
            return Err(ModularGuideError::WrongIndexedContext);
        }
        validate_modulus(modulus)?;
        let point_count = context
            .base()
            .parameter_names()
            .len()
            .checked_add(context.index_count())
            .ok_or(ModularGuideError::ResourceCountOverflow {
                resource: PROBE_POINT_COORDINATES,
            })?;
        if full_integer_point.len() != point_count {
            return Err(ModularGuideError::WrongPointArity {
                expected: point_count,
                actual: full_integer_point.len(),
            });
        }
        check_limit(
            PROBE_POINT_COORDINATES,
            point_count,
            limits.max_probe_point_coordinates,
        )?;
        check_limit(
            "modular guide retained point coordinate cells",
            checked_mul(
                "modular guide retained point coordinate cells",
                point_count,
                3,
            )?,
            limits.max_probe_retained_point_coordinate_cells,
        )?;
        check_limit(PROBE_DELTAS, 1, limits.max_probe_accumulated_deltas)?;
        check_limit(
            PROBE_DELTA_CELLS,
            context.index_count(),
            limits.max_probe_accumulated_delta_coordinate_cells,
        )?;
        check_limit(TRANSLATED_POINTS, 1, limits.max_probe_translated_points)?;
        check_limit(
            TRANSLATED_POINT_CELLS,
            point_count,
            limits.max_probe_translated_point_coordinate_cells,
        )?;

        let field = Zp64::new(modulus);
        let mut point = Vec::new();
        reserve_vec(&mut point, point_count, PROBE_POINT_COORDINATES)?;
        point.extend(
            full_integer_point
                .iter()
                .map(|&value| Integer::from(value).to_finite_field(&field)),
        );
        let base_point = Arc::new(point);
        let mut residues = Vec::new();
        reserve_vec(&mut residues, point_count, PROBE_POINT_COORDINATES)?;
        residues.extend(base_point.iter().map(|value| field.from_element(value)));
        let mut retained_integer_point = Vec::new();
        reserve_vec(
            &mut retained_integer_point,
            point_count,
            PROBE_POINT_COORDINATES,
        )?;
        retained_integer_point.extend_from_slice(full_integer_point);

        let mut zero_delta = Vec::new();
        reserve_vec(&mut zero_delta, context.index_count(), PROBE_DELTA_CELLS)?;
        zero_delta.resize(context.index_count(), 0);
        let zero_delta = Arc::new(zero_delta);
        let zero_id = AccumulatedDeltaId::try_new(0).expect("zero accumulated delta ID is fixed");
        let mut accumulated_deltas = Vec::new();
        reserve_vec(&mut accumulated_deltas, 1, PROBE_DELTAS)?;
        accumulated_deltas.push(Arc::clone(&zero_delta));
        let mut accumulated_delta_lookup = HashMap::new();
        reserve_map(&mut accumulated_delta_lookup, 1, PROBE_DELTAS)?;
        accumulated_delta_lookup.insert(zero_delta, zero_id);
        let mut translated_points = HashMap::new();
        reserve_map(&mut translated_points, 1, TRANSLATED_POINTS)?;
        translated_points.insert(zero_id, Arc::clone(&base_point));

        Ok(Self {
            identity: Arc::new(ModularProbeIdentity::new(
                ordinal,
                modulus,
                Arc::new(retained_integer_point),
                Arc::new(residues),
            )),
            dag_owner: dag.owner().clone(),
            context_fingerprint: context.fingerprint_owner(),
            base_parameter_count: context.base().parameter_names().len(),
            index_count: context.index_count(),
            limits,
            field,
            base_point,
            accumulated_deltas,
            accumulated_delta_lookup,
            accumulated_delta_cells: context.index_count(),
            translated_points,
            translated_point_cells: point_count,
            value_cache: HashMap::new(),
            census: ModularProbeCensus::default(),
            rejected: false,
        })
    }

    /// Consume this lane and release images only after every requested
    /// coefficient succeeds. On any singularity or resource stop the partial
    /// buffer is dropped together with the rejected probe.
    pub(super) fn try_evaluate_batch(
        mut self,
        dag: &ModularCoefficientDag,
        coefficients: &[CoeffRef],
    ) -> Result<ModularEvaluationBatch, ModularGuideError> {
        if let Err(error) = check_limit(
            BATCH_IMAGES,
            coefficients.len(),
            self.limits.max_probe_batch_images,
        ) {
            self.reject();
            return Err(error);
        }
        if let Err(error) = check_limit(
            PROBE_QUERIES,
            coefficients.len(),
            self.limits.max_probe_queries,
        ) {
            self.reject();
            return Err(error);
        }
        let mut queries = Vec::new();
        if let Err(error) = reserve_vec(&mut queries, coefficients.len(), BATCH_IMAGES) {
            self.reject();
            return Err(error);
        }
        queries.extend(
            coefficients
                .iter()
                .cloned()
                .map(|root| ModularEvaluationQuery {
                    role: ModularQueryRole::Coefficient,
                    root,
                }),
        );
        let images = self.try_evaluate_retained_batch(dag, coefficients)?;
        Ok(ModularEvaluationBatch {
            identity: Arc::clone(&self.identity),
            dag_owner: self.dag_owner.clone(),
            context_fingerprint: Arc::clone(&self.context_fingerprint),
            queries: queries.into_boxed_slice(),
            guard_count: 0,
            images,
            census: self.census,
        })
    }

    /// Consume one probe in the theorem-bearing ELC1 layout.
    ///
    /// Guards are evaluated first and must all have nonzero images.  Only then
    /// is the complete coefficient batch evaluated.  Any zero guard,
    /// singularity, stale/foreign root, allocation failure, or resource stop
    /// rejects the lane and returns census only; no prefix image escapes.
    pub(super) fn try_evaluate_guarded_batch(
        mut self,
        dag: &ModularCoefficientDag,
        guards: &[CoeffRef],
        coefficients: &[CoeffRef],
    ) -> Result<ModularEvaluationBatch, RejectedProbeReport> {
        let total = match checked_add(BATCH_IMAGES, guards.len(), coefficients.len()) {
            Ok(total) => total,
            Err(error) => return Err(self.into_rejected_report(error)),
        };
        if let Err(error) = check_limit(BATCH_IMAGES, total, self.limits.max_probe_batch_images) {
            return Err(self.into_rejected_report(error));
        }
        let requested_queries = match checked_add(PROBE_QUERIES, self.census.queries, total) {
            Ok(requested) => requested,
            Err(error) => return Err(self.into_rejected_report(error)),
        };
        if let Err(error) = check_limit(
            PROBE_QUERIES,
            requested_queries,
            self.limits.max_probe_queries,
        ) {
            return Err(self.into_rejected_report(error));
        }

        let mut queries = Vec::new();
        if let Err(error) = reserve_vec(&mut queries, total, BATCH_IMAGES) {
            return Err(self.into_rejected_report(error));
        }
        queries.extend(guards.iter().cloned().map(|root| ModularEvaluationQuery {
            role: ModularQueryRole::Guard,
            root,
        }));
        queries.extend(
            coefficients
                .iter()
                .cloned()
                .map(|root| ModularEvaluationQuery {
                    role: ModularQueryRole::Coefficient,
                    root,
                }),
        );

        let guard_images = match self.try_evaluate_retained_batch(dag, guards) {
            Ok(images) => images,
            Err(error) => return Err(self.into_rejected_report(error)),
        };
        if guard_images
            .iter()
            .any(|image| image.zero_evidence() != ModularZeroEvidence::Nonzero)
        {
            return Err(self.into_rejected_report(ModularGuideError::SampledZeroLocalizationGuard));
        }
        let coefficient_images = match self.try_evaluate_retained_batch(dag, coefficients) {
            Ok(images) => images,
            Err(error) => return Err(self.into_rejected_report(error)),
        };

        let mut images = Vec::new();
        if let Err(error) = reserve_vec(&mut images, total, BATCH_IMAGES) {
            return Err(self.into_rejected_report(error));
        }
        images.extend(guard_images);
        images.extend(coefficient_images);
        Ok(ModularEvaluationBatch {
            identity: Arc::clone(&self.identity),
            dag_owner: self.dag_owner.clone(),
            context_fingerprint: Arc::clone(&self.context_fingerprint),
            queries: queries.into_boxed_slice(),
            guard_count: guards.len(),
            images: images.into_boxed_slice(),
            census: self.census,
        })
    }

    /// Evaluate one complete batch while retaining this lane's cache for a
    /// later modular Ore operation.
    ///
    /// This boundary is private to proposal-only modular guidance. Any
    /// failure poisons the whole probe and drops its caches; callers must not
    /// release an earlier partial trace if a later batch fails.
    pub(super) fn try_evaluate_retained_batch(
        &mut self,
        dag: &ModularCoefficientDag,
        coefficients: &[CoeffRef],
    ) -> Result<Box<[ModularImage]>, ModularGuideError> {
        let result = self.try_evaluate_retained_batch_inner(dag, coefficients);
        if result.is_err() {
            self.reject();
        }
        result
    }

    fn try_evaluate_retained_batch_inner(
        &mut self,
        dag: &ModularCoefficientDag,
        coefficients: &[CoeffRef],
    ) -> Result<Box<[ModularImage]>, ModularGuideError> {
        if self.rejected {
            return Err(ModularGuideError::RejectedProbe);
        }
        check_limit(
            BATCH_IMAGES,
            coefficients.len(),
            self.limits.max_probe_batch_images,
        )?;
        if !self.dag_owner.belongs_to(dag.owner()) {
            return Err(ModularGuideError::WrongDagOwner);
        }
        let requested_queries =
            checked_add(PROBE_QUERIES, self.census.queries, coefficients.len())?;
        check_limit(
            PROBE_QUERIES,
            requested_queries,
            self.limits.max_probe_queries,
        )?;
        let mut images = Vec::new();
        reserve_vec(&mut images, coefficients.len(), BATCH_IMAGES)?;
        for coefficient in coefficients {
            images.push(self.try_evaluate(dag, coefficient)?);
        }
        Ok(images.into_boxed_slice())
    }

    fn try_evaluate(
        &mut self,
        dag: &ModularCoefficientDag,
        coefficient: &CoeffRef,
    ) -> Result<ModularImage, ModularGuideError> {
        if self.rejected {
            return Err(ModularGuideError::RejectedProbe);
        }
        if let Err(error) = charge(
            PROBE_QUERIES,
            &mut self.census.queries,
            1,
            self.limits.max_probe_queries,
        ) {
            self.reject();
            return Err(error);
        }
        let result = self.try_evaluate_inner(dag, coefficient);
        if result.is_err() {
            self.reject();
        }
        result
    }

    pub(super) fn identity(&self) -> &ModularProbeIdentity {
        &self.identity
    }

    pub(super) fn identity_owner(&self) -> Arc<ModularProbeIdentity> {
        Arc::clone(&self.identity)
    }

    pub(super) fn census(&self) -> ModularProbeCensus {
        self.census
    }

    pub(super) fn is_rejected(&self) -> bool {
        self.rejected
    }

    pub(super) fn field(&self) -> &Zp64 {
        &self.field
    }

    fn try_evaluate_inner(
        &mut self,
        dag: &ModularCoefficientDag,
        coefficient: &CoeffRef,
    ) -> Result<ModularImage, ModularGuideError> {
        if !self.dag_owner.belongs_to(dag.owner()) || !dag.owns_reference(coefficient) {
            return Err(ModularGuideError::WrongDagOwner);
        }
        if !Arc::ptr_eq(dag.context_fingerprint(), &self.context_fingerprint)
            && dag.context_fingerprint().as_str() != self.context_fingerprint.as_str()
        {
            return Err(ModularGuideError::WrongIndexedContext);
        }
        if dag.index_count() != self.index_count {
            return Err(ModularGuideError::Invariant {
                detail: "matching modular context fingerprints have different index arity",
            });
        }
        let raw = dag.raw(coefficient)?;
        let value = self.try_evaluate_raw(dag, raw, self.zero_delta_id())?;
        let zero_evidence = if dag.is_known_zero(coefficient)? {
            ModularZeroEvidence::KnownZero
        } else if self.field.is_zero(&value) {
            ModularZeroEvidence::SampledZero
        } else {
            ModularZeroEvidence::Nonzero
        };
        Ok(ModularImage::new(value, zero_evidence))
    }

    fn try_evaluate_raw(
        &mut self,
        dag: &ModularCoefficientDag,
        reference: RawCoeffRef,
        inherited: AccumulatedDeltaId,
    ) -> Result<FiniteFieldElement<u64>, ModularGuideError> {
        let mut frames = Vec::new();
        let mut values = Vec::new();
        self.try_push_frame(
            &mut frames,
            PostorderFrame::Enter {
                reference,
                inherited,
            },
        )?;

        while let Some(frame) = frames.pop() {
            match frame {
                PostorderFrame::Enter {
                    reference,
                    inherited,
                } => {
                    let translation =
                        self.try_compose_delta(dag, inherited, reference.translation)?;
                    let key = EvaluationKey {
                        node: reference.node,
                        translation,
                    };
                    if let Some(&value) = self.value_cache.get(&key) {
                        self.census.cache_hits = self.census.cache_hits.checked_add(1).ok_or(
                            ModularGuideError::ResourceCountOverflow {
                                resource: "modular guide cache hits",
                            },
                        )?;
                        self.try_push_value(&mut values, value)?;
                        continue;
                    }
                    charge(
                        EVALUATION_STEPS,
                        &mut self.census.evaluation_steps,
                        1,
                        self.limits.max_probe_evaluation_steps,
                    )?;
                    match dag.node(reference.node)?.clone() {
                        CoeffNode::Zero => {
                            let value = self.field.zero();
                            self.try_finish_value(key, value, &mut values)?;
                        }
                        CoeffNode::One => {
                            let value = self.field.one();
                            self.try_finish_value(key, value, &mut values)?;
                        }
                        CoeffNode::ExactLeaf(leaf_id) => {
                            let value =
                                self.try_evaluate_leaf(dag, reference.node, leaf_id, translation)?;
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
                        detail: "iterative modular unary operation has no child value",
                    })?;
                    let value = match operation {
                        UnaryOperation::Neg => self.field.neg(&child),
                        UnaryOperation::Inv => {
                            if self.field.is_zero(&child) {
                                return Err(ModularGuideError::SingularInverse { node: key.node });
                            }
                            self.field.inv(&child)
                        }
                    };
                    self.try_finish_value(key, value, &mut values)?;
                }
                PostorderFrame::AfterLeft {
                    key,
                    right,
                    inherited,
                    operation,
                } => {
                    if values.is_empty() {
                        return Err(ModularGuideError::Invariant {
                            detail: "iterative modular binary operation has no left value",
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
                        detail: "iterative modular binary operation has no right value",
                    })?;
                    let left = values.pop().ok_or(ModularGuideError::Invariant {
                        detail: "iterative modular binary operation lost its left value",
                    })?;
                    let value = match operation {
                        BinaryOperation::Add => self.field.add(&left, &right),
                        BinaryOperation::Mul => self.field.mul(&left, &right),
                    };
                    self.try_finish_value(key, value, &mut values)?;
                }
            }
        }

        if values.len() != 1 {
            return Err(ModularGuideError::Invariant {
                detail: "iterative modular evaluation did not produce exactly one root value",
            });
        }
        values.pop().ok_or(ModularGuideError::Invariant {
            detail: "iterative modular evaluation lost its root value",
        })
    }

    fn try_evaluate_leaf(
        &mut self,
        dag: &ModularCoefficientDag,
        node_id: CoeffNodeId,
        leaf_id: super::model::ExactLeafId,
        translation: AccumulatedDeltaId,
    ) -> Result<FiniteFieldElement<u64>, ModularGuideError> {
        charge(
            EXACT_LEAF_EVALUATIONS,
            &mut self.census.exact_leaf_evaluations,
            1,
            self.limits.max_probe_exact_leaf_evaluations,
        )?;
        let leaf = dag.exact_leaf(leaf_id)?;
        let leaf_terms = leaf
            .raw()
            .numerator
            .coefficients
            .len()
            .checked_add(leaf.raw().denominator.coefficients.len())
            .ok_or(ModularGuideError::ResourceCountOverflow {
                resource: EXACT_LEAF_TERMS,
            })?;
        charge(
            EXACT_LEAF_TERMS,
            &mut self.census.exact_leaf_terms_evaluated,
            leaf_terms,
            self.limits.max_probe_exact_leaf_terms_evaluated,
        )?;
        let exponent_cells = leaf
            .raw()
            .numerator
            .exponents
            .len()
            .checked_add(leaf.raw().denominator.exponents.len())
            .ok_or(ModularGuideError::ResourceCountOverflow {
                resource: EXACT_LEAF_EXPONENT_CELLS,
            })?;
        charge(
            EXACT_LEAF_EXPONENT_CELLS,
            &mut self.census.exact_leaf_exponent_cells_evaluated,
            exponent_cells,
            self.limits.max_probe_exact_leaf_exponent_cells_evaluated,
        )?;
        let point = self.try_translated_point(translation)?;
        let numerator = leaf.raw().numerator.evaluate_with_coeff_map(
            |value| value.to_finite_field(&self.field),
            point.as_slice(),
            &self.field,
        );
        let denominator = leaf.raw().denominator.evaluate_with_coeff_map(
            |value| value.to_finite_field(&self.field),
            point.as_slice(),
            &self.field,
        );
        if self.field.is_zero(&denominator) {
            return Err(ModularGuideError::SingularExactLeaf { node: node_id });
        }
        Ok(self.field.div(&numerator, &denominator))
    }

    fn try_finish_value(
        &mut self,
        key: EvaluationKey,
        value: FiniteFieldElement<u64>,
        values: &mut Vec<FiniteFieldElement<u64>>,
    ) -> Result<(), ModularGuideError> {
        self.try_cache_value(key, value)?;
        self.try_push_value(values, value)
    }

    fn try_push_frame(
        &mut self,
        frames: &mut Vec<PostorderFrame>,
        frame: PostorderFrame,
    ) -> Result<(), ModularGuideError> {
        charge(
            EVALUATION_FRAME_PUSHES,
            &mut self.census.evaluation_frame_pushes,
            1,
            self.limits.max_probe_evaluation_frame_pushes,
        )?;
        let requested = checked_add(LIVE_EVALUATION_FRAMES, frames.len(), 1)?;
        check_limit(
            LIVE_EVALUATION_FRAMES,
            requested,
            self.limits.max_probe_live_evaluation_frames,
        )?;
        reserve_vec(frames, 1, LIVE_EVALUATION_FRAMES)?;
        frames.push(frame);
        self.census.peak_live_evaluation_frames =
            self.census.peak_live_evaluation_frames.max(requested);
        Ok(())
    }

    fn try_push_value(
        &mut self,
        values: &mut Vec<FiniteFieldElement<u64>>,
        value: FiniteFieldElement<u64>,
    ) -> Result<(), ModularGuideError> {
        let requested = checked_add(LIVE_EVALUATION_VALUES, values.len(), 1)?;
        check_limit(
            LIVE_EVALUATION_VALUES,
            requested,
            self.limits.max_probe_live_evaluation_values,
        )?;
        reserve_vec(values, 1, LIVE_EVALUATION_VALUES)?;
        values.push(value);
        self.census.peak_live_evaluation_values =
            self.census.peak_live_evaluation_values.max(requested);
        Ok(())
    }

    fn try_compose_delta(
        &mut self,
        dag: &ModularCoefficientDag,
        inherited: AccumulatedDeltaId,
        added: PhysicalDeltaId,
    ) -> Result<AccumulatedDeltaId, ModularGuideError> {
        let added = dag.delta(added)?;
        if added.iter().all(|&value| value == 0) {
            return Ok(inherited);
        }
        charge(
            DELTA_COMPOSITIONS,
            &mut self.census.delta_compositions,
            1,
            self.limits.max_probe_delta_compositions,
        )?;
        charge(
            DELTA_COORDINATE_OPERATIONS,
            &mut self.census.delta_coordinate_operations,
            self.index_count,
            self.limits.max_probe_delta_coordinate_operations,
        )?;
        let inherited_values = self.accumulated_deltas.get(inherited.as_usize()).ok_or(
            ModularGuideError::Invariant {
                detail: "evaluation names an absent accumulated probe translation",
            },
        )?;
        let mut result = Vec::new();
        reserve_vec(&mut result, self.index_count, PROBE_DELTA_CELLS)?;
        for (&left, &right) in inherited_values.iter().zip(added) {
            let value =
                left.checked_add(right)
                    .ok_or(ModularGuideError::ResourceCountOverflow {
                        resource: "modular guide accumulated translation coordinate",
                    })?;
            if value.unsigned_abs() > self.limits.max_absolute_physical_translation {
                return Err(ModularGuideError::ResourceLimit {
                    resource: "modular guide absolute accumulated translation",
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
        let requested_deltas = checked_add(PROBE_DELTAS, self.accumulated_deltas.len(), 1)?;
        check_limit(
            PROBE_DELTAS,
            requested_deltas,
            self.limits.max_probe_accumulated_deltas,
        )?;
        let requested_cells = checked_add(
            PROBE_DELTA_CELLS,
            self.accumulated_delta_cells,
            self.index_count,
        )?;
        check_limit(
            PROBE_DELTA_CELLS,
            requested_cells,
            self.limits.max_probe_accumulated_delta_coordinate_cells,
        )?;
        let id = AccumulatedDeltaId::try_new(self.accumulated_deltas.len()).ok_or(
            ModularGuideError::IdentifierNotRepresentable {
                resource: "modular guide accumulated-delta identifier",
                value: self.accumulated_deltas.len(),
            },
        )?;
        reserve_vec(&mut self.accumulated_deltas, 1, PROBE_DELTAS)?;
        reserve_map(&mut self.accumulated_delta_lookup, 1, PROBE_DELTAS)?;
        let result = Arc::new(result);
        self.accumulated_delta_lookup
            .insert(Arc::clone(&result), id);
        self.accumulated_deltas.push(result);
        self.accumulated_delta_cells = requested_cells;
        Ok(id)
    }

    fn try_translated_point(
        &mut self,
        translation: AccumulatedDeltaId,
    ) -> Result<Arc<Vec<FiniteFieldElement<u64>>>, ModularGuideError> {
        if let Some(point) = self.translated_points.get(&translation) {
            return Ok(Arc::clone(point));
        }
        let delta = self.accumulated_deltas.get(translation.as_usize()).ok_or(
            ModularGuideError::Invariant {
                detail: "translated point names an absent accumulated delta",
            },
        )?;
        let requested_points = checked_add(TRANSLATED_POINTS, self.translated_points.len(), 1)?;
        check_limit(
            TRANSLATED_POINTS,
            requested_points,
            self.limits.max_probe_translated_points,
        )?;
        let requested_cells = checked_add(
            TRANSLATED_POINT_CELLS,
            self.translated_point_cells,
            self.base_point.len(),
        )?;
        check_limit(
            TRANSLATED_POINT_CELLS,
            requested_cells,
            self.limits.max_probe_translated_point_coordinate_cells,
        )?;
        let mut point = Vec::new();
        reserve_vec(&mut point, self.base_point.len(), TRANSLATED_POINT_CELLS)?;
        point.extend(self.base_point.iter().copied());
        for (position, &offset) in delta.iter().enumerate() {
            let coordinate = self.base_parameter_count.checked_add(position).ok_or(
                ModularGuideError::ResourceCountOverflow {
                    resource: PROBE_POINT_COORDINATES,
                },
            )?;
            let current = point
                .get(coordinate)
                .copied()
                .ok_or(ModularGuideError::Invariant {
                    detail: "translated index coordinate is absent from the modular point",
                })?;
            point[coordinate] = self.field.add(
                &current,
                &Integer::from(offset).to_finite_field(&self.field),
            );
        }
        reserve_map(&mut self.translated_points, 1, TRANSLATED_POINTS)?;
        let point = Arc::new(point);
        self.translated_points
            .insert(translation, Arc::clone(&point));
        self.translated_point_cells = requested_cells;
        Ok(point)
    }

    fn try_cache_value(
        &mut self,
        key: EvaluationKey,
        value: FiniteFieldElement<u64>,
    ) -> Result<(), ModularGuideError> {
        let requested = checked_add(CACHED_VALUES, self.value_cache.len(), 1)?;
        check_limit(
            CACHED_VALUES,
            requested,
            self.limits.max_probe_cached_values,
        )?;
        reserve_map(&mut self.value_cache, 1, CACHED_VALUES)?;
        self.value_cache.insert(key, value);
        Ok(())
    }

    fn zero_delta_id(&self) -> AccumulatedDeltaId {
        AccumulatedDeltaId::try_new(0).expect("zero accumulated delta ID is fixed")
    }

    pub(super) fn reject(&mut self) {
        self.rejected = true;
        drop(std::mem::take(&mut self.value_cache));
        drop(std::mem::take(&mut self.translated_points));
        drop(std::mem::take(&mut self.accumulated_delta_lookup));
        drop(std::mem::take(&mut self.accumulated_deltas));
        self.accumulated_delta_cells = 0;
        self.translated_point_cells = 0;
    }

    fn into_rejected_report(mut self, error: ModularGuideError) -> RejectedProbeReport {
        let census = self.census;
        self.reject();
        RejectedProbeReport::new(error, census)
    }
}

fn validate_modulus(modulus: u64) -> Result<(), ModularGuideError> {
    if modulus.is_multiple_of(2) || modulus == u64::MAX || !Integer::from(modulus).is_prime(0) {
        Err(ModularGuideError::UnsupportedModulus { modulus })
    } else {
        Ok(())
    }
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
