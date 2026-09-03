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
    CoeffNode, CoeffNodeId, CoeffRef, DagOwner, EvaluationKey, ModularEvaluationBatch,
    ModularImage, ModularProbeCensus, ModularProbeIdentity, ModularZeroEvidence, PhysicalDeltaId,
    ProbeDeltaId, RawCoeffRef,
};

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
const EXACT_LEAF_EVALUATIONS: &str = "modular guide exact-leaf evaluations";
const EXACT_LEAF_TERMS: &str = "modular guide exact-leaf terms evaluated";
const EXACT_LEAF_EXPONENT_CELLS: &str = "modular guide exact-leaf exponent cells evaluated";

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
    accumulated_delta_lookup: HashMap<Arc<Vec<i64>>, ProbeDeltaId>,
    accumulated_delta_cells: usize,
    translated_points: HashMap<ProbeDeltaId, Arc<Vec<FiniteFieldElement<u64>>>>,
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
        let zero_id = ProbeDeltaId::try_new(0).expect("zero probe delta ID is fixed");
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
        check_limit(
            BATCH_IMAGES,
            coefficients.len(),
            self.limits.max_probe_batch_images,
        )?;
        if !self.dag_owner.belongs_to(dag.owner()) {
            return Err(ModularGuideError::WrongDagOwner);
        }
        let mut queries = Vec::new();
        reserve_vec(&mut queries, coefficients.len(), BATCH_IMAGES)?;
        queries.extend(coefficients.iter().cloned());
        let mut images = Vec::new();
        reserve_vec(&mut images, coefficients.len(), BATCH_IMAGES)?;
        for coefficient in coefficients {
            images.push(self.try_evaluate(dag, coefficient)?);
        }
        Ok(ModularEvaluationBatch {
            identity: Arc::clone(&self.identity),
            dag_owner: self.dag_owner.clone(),
            queries: queries.into_boxed_slice(),
            images: images.into_boxed_slice(),
            census: self.census,
        })
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
        let value = self.try_evaluate_raw(dag, raw, self.zero_delta_id(), 0)?;
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
        inherited: ProbeDeltaId,
        depth: usize,
    ) -> Result<FiniteFieldElement<u64>, ModularGuideError> {
        if depth > self.limits.max_probe_evaluation_depth {
            return Err(ModularGuideError::ResourceLimit {
                resource: "modular guide evaluation depth",
                requested: depth,
                limit: self.limits.max_probe_evaluation_depth,
            });
        }
        let translation = self.try_compose_delta(dag, inherited, reference.translation)?;
        self.try_evaluate_node(dag, reference.node, translation, depth)
    }

    fn try_evaluate_node(
        &mut self,
        dag: &ModularCoefficientDag,
        node_id: CoeffNodeId,
        translation: ProbeDeltaId,
        depth: usize,
    ) -> Result<FiniteFieldElement<u64>, ModularGuideError> {
        let key = EvaluationKey {
            node: node_id,
            translation,
        };
        if let Some(&value) = self.value_cache.get(&key) {
            self.census.cache_hits = self.census.cache_hits.checked_add(1).ok_or(
                ModularGuideError::ResourceCountOverflow {
                    resource: "modular guide cache hits",
                },
            )?;
            return Ok(value);
        }
        charge(
            EVALUATION_STEPS,
            &mut self.census.evaluation_steps,
            1,
            self.limits.max_probe_evaluation_steps,
        )?;
        let node = dag.node(node_id)?.clone();
        let next_depth = depth
            .checked_add(1)
            .ok_or(ModularGuideError::ResourceCountOverflow {
                resource: "modular guide evaluation depth",
            })?;
        let value = match node {
            CoeffNode::Zero => self.field.zero(),
            CoeffNode::One => self.field.one(),
            CoeffNode::ExactLeaf(leaf_id) => {
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
                self.field.div(&numerator, &denominator)
            }
            CoeffNode::Neg(child) => {
                let child = self.try_evaluate_raw(dag, child, translation, next_depth)?;
                self.field.neg(&child)
            }
            CoeffNode::Add(left, right) => {
                let left = self.try_evaluate_raw(dag, left, translation, next_depth)?;
                let right = self.try_evaluate_raw(dag, right, translation, next_depth)?;
                self.field.add(&left, &right)
            }
            CoeffNode::Mul(left, right) => {
                let left = self.try_evaluate_raw(dag, left, translation, next_depth)?;
                let right = self.try_evaluate_raw(dag, right, translation, next_depth)?;
                self.field.mul(&left, &right)
            }
            CoeffNode::Inv(child) => {
                let child = self.try_evaluate_raw(dag, child, translation, next_depth)?;
                if self.field.is_zero(&child) {
                    return Err(ModularGuideError::SingularInverse { node: node_id });
                }
                self.field.inv(&child)
            }
        };
        self.try_cache_value(key, value)?;
        Ok(value)
    }

    fn try_compose_delta(
        &mut self,
        dag: &ModularCoefficientDag,
        inherited: ProbeDeltaId,
        added: PhysicalDeltaId,
    ) -> Result<ProbeDeltaId, ModularGuideError> {
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
        let id = ProbeDeltaId::try_new(self.accumulated_deltas.len()).ok_or(
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
        translation: ProbeDeltaId,
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

    fn zero_delta_id(&self) -> ProbeDeltaId {
        ProbeDeltaId::try_new(0).expect("zero probe delta ID is fixed")
    }

    fn reject(&mut self) {
        self.rejected = true;
        drop(std::mem::take(&mut self.value_cache));
        drop(std::mem::take(&mut self.translated_points));
        drop(std::mem::take(&mut self.accumulated_delta_lookup));
        drop(std::mem::take(&mut self.accumulated_deltas));
        self.accumulated_delta_cells = 0;
        self.translated_point_cells = 0;
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
