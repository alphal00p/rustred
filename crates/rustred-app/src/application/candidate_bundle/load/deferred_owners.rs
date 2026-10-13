//! Frozen native inputs with one-time, per-owner formula preparation.
use std::collections::{BTreeMap, BTreeSet};
use std::sync::{Arc, Mutex};

use rustred::family::{IntegralFamily, IntegralKey};
use rustred::persistence::{LazyDecodedCoefficientTable, SectionTag};
use rustred::reduction::ReductionLimits;
use rustred::solver::{
    CandidateOwnerContext, CandidateOwnerInput, CandidateOwnerPrograms, CandidateReductionError,
    DeferredCandidateOwnerDescriptor, DeferredCandidateOwnerLoader, DeferredCandidateOwnerPrograms,
};

use super::super::{codec, model::ProgramRecord};
use super::owners::{self, CandidateOwnerBundle, CandidateOwnerLoadLimits};
use crate::AppError;
use crate::application::candidate_bundle::{
    CandidateDomainOverlayLoadLimits, load_generated_domain_overlay,
    validate_domain_overlay_ingress,
};

pub(crate) struct DeferredOwnerOverlay<'a, const N: usize> {
    pub bytes: &'a [u8],
    pub owner: [bool; N],
    pub baseline_digest: [u8; 32],
}

struct NativeOwner {
    ordering: rustred::sector::OrderingPolicy,
    records: ProgramRecord,
    coefficients: LazyDecodedCoefficientTable,
}
impl std::fmt::Debug for NativeOwner {
    fn fmt(&self, out: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        out.debug_struct("NativeOwner")
            .field("sectors", &self.records.sectors.len())
            .field("coefficients", &self.coefficients.len())
            .finish()
    }
}

#[derive(Debug)]
struct Overlay {
    bytes: Arc<[u8]>,
    baseline_digest: [u8; 32],
}
#[derive(Debug)]
struct Source {
    baseline: NativeOwner,
    preferred: Option<NativeOwner>,
    subset: Option<Vec<usize>>,
    overlays: Vec<Overlay>,
}
#[derive(Debug)]
struct Loader<const N: usize> {
    sources: BTreeMap<[bool; N], Mutex<Option<Source>>>,
    limits: CandidateOwnerLoadLimits,
}

impl<const N: usize> DeferredCandidateOwnerLoader<N> for Loader<N> {
    fn prepare(
        &self,
        sector: [bool; N],
        context: Arc<CandidateOwnerContext<N>>,
    ) -> Result<CandidateOwnerPrograms<N>, CandidateReductionError> {
        let result = (|| -> Result<CandidateOwnerPrograms<N>, AppError> {
            let source = self
                .sources
                .get(&sector)
                .ok_or_else(|| AppError::input("deferred owner source is missing"))?
                .lock()
                .map_err(|_| AppError::execution("deferred owner source lock poisoned"))?
                .take()
                .ok_or_else(|| AppError::execution("deferred owner source was already consumed"))?;
            let baseline = source.baseline.decode(&context, self.limits)?;
            let preferred = source
                .preferred
                .as_ref()
                .map(|owner| owner.decode(&context, self.limits))
                .transpose()?;
            let subsets = source
                .subset
                .as_ref()
                .map(|ordinals| BTreeMap::from([(sector, ordinals.as_slice())]))
                .unwrap_or_default();
            let programs = CandidateOwnerPrograms::try_new_with_preference_rule_subsets(
                context,
                [baseline],
                preferred.into_iter(),
                &subsets,
            )
            .map_err(|error| AppError::execution(error.to_string()))?;
            let mut programs = Arc::new(programs);
            for overlay in source.overlays {
                let (overlay, _) = load_generated_domain_overlay(
                    &programs,
                    &overlay.bytes,
                    sector,
                    overlay.baseline_digest,
                    CandidateDomainOverlayLoadLimits {
                        bundle: self.limits.bundle,
                        ..Default::default()
                    },
                )?;
                programs = programs
                    .append_residual_free_domain_overlays(vec![overlay], Default::default())
                    .map_err(|error| AppError::execution(error.to_string()))?;
            }
            Arc::try_unwrap(programs)
                .map_err(|_| AppError::internal_invariant("fresh deferred owner remained shared"))
        })();
        result.map_err(|error| {
            CandidateReductionError::InvalidInput(format!(
                "deferred native owner admission: {error}"
            ))
        })
    }
}

impl NativeOwner {
    fn decode<const N: usize>(
        &self,
        context: &Arc<CandidateOwnerContext<N>>,
        limits: CandidateOwnerLoadLimits,
    ) -> Result<CandidateOwnerInput<N>, AppError> {
        let mut solutions = codec::solutions_from_table::<N>(
            &self.records,
            &self.coefficients,
            context.coefficient_context(),
            context.index_variables(),
            limits.bundle,
        )?;
        if solutions.len() != 1 {
            return Err(AppError::input("deferred owner no longer has one sector"));
        }
        let (sector, solution) = solutions.pop().unwrap();
        let expected: [bool; N] = rustred::storage_array(&self.records.sectors[0].sector, false)
            .ok_or_else(|| AppError::input("deferred owner storage arity changed"))?;
        if sector != expected {
            return Err(AppError::input(
                "deferred owner sector changed on native import",
            ));
        }
        Ok(CandidateOwnerInput {
            sector,
            saved_root: rustred::storage_array(&self.records.root_sector, false)
                .ok_or_else(|| AppError::input("deferred root storage arity changed"))?,
            ordering: self.ordering.clone(),
            solution,
        })
    }
}

/// Whole-collection structure and native frame admission precede native state
/// import. Only native family geometry is decoded now; formulas remain frozen
/// encoded data until the corresponding owner is requested by the core.
pub(crate) fn prepare_deferred_candidate_owners<const N: usize>(
    family: Arc<IntegralFamily>,
    inputs: &[CandidateOwnerBundle<'_>],
    preferences: &[CandidateOwnerBundle<'_>],
    rule_subsets: &BTreeMap<[bool; N], &[usize]>,
    overlays: &[DeferredOwnerOverlay<'_, N>],
    limits: CandidateOwnerLoadLimits,
    reduction_limits: ReductionLimits,
) -> Result<DeferredCandidateOwnerPrograms<N>, AppError> {
    let subset_entries = rule_subsets
        .values()
        .try_fold(0usize, |sum, ordinals| sum.checked_add(ordinals.len()))
        .ok_or_else(|| AppError::limit("deferred preferred subset count overflow"))?;
    if rule_subsets.len() > preferences.len()
        || subset_entries > limits.bundle.max_collection_entries
    {
        return Err(AppError::limit(
            "deferred preferred subset metadata exceeds admission limit",
        ));
    }
    let admission = owners::admit_owner_inputs::<N>(inputs, preferences, limits)?;
    let all_inputs = inputs
        .iter()
        .chain(preferences)
        .copied()
        .collect::<Vec<_>>();
    let patches = overlays
        .iter()
        .map(|overlay| overlay.bytes)
        .collect::<Vec<_>>();
    if !patches.is_empty() {
        validate_domain_overlay_ingress(&all_inputs, &patches, limits)?;
    }
    let mut native = Vec::new();
    for (input, descriptor) in all_inputs.iter().zip(admission.descriptors) {
        let (envelope, records, native_family) = codec::read_structure(input.bytes, limits.bundle)?;
        let coefficients = LazyDecodedCoefficientTable::new(
            Arc::from(
                envelope
                    .section(SectionTag::SYMBOLICA_STATE)
                    .expect("admitted state"),
            ),
            Arc::from(
                envelope
                    .section(SectionTag::COEFFICIENTS)
                    .expect("admitted coefficients"),
            ),
            limits.bundle.binary_limits(),
        )
        .map_err(codec::binary_error)?;
        codec::validate_ids(&records, coefficients.len())?;
        if records.family_fingerprint != family.fingerprint() {
            return Err(AppError::input(
                "deferred owner family differs from terminal publication",
            ));
        }
        native.push((
            native_family,
            NativeOwner {
                ordering: descriptor.ordering,
                records,
                coefficients,
            },
        ));
    }
    // Every frame/ID has now passed boundary admission; native family binding
    // imports only the small geometry subset of each existing Symbolica table.
    for overlay in overlays {
        super::super::domain_overlay::validate_deferred_overlay(
            overlay.bytes,
            overlay.owner,
            overlay.baseline_digest,
            &family,
            limits.bundle,
        )?;
    }
    for (native_family, owner) in &native {
        native_family
            .validate_against_family(
                &family,
                &owner.coefficients,
                limits.bundle.family_limits(),
                limits.bundle.binary_limits(),
            )
            .map_err(codec::binary_error)?;
    }
    let proofs = owners::zero_proofs::<N>(&family, &admission.zero_masks)?;
    let context = Arc::new(
        CandidateOwnerContext::try_new(family, admission.scope, proofs, reduction_limits)
            .map_err(|error| AppError::execution(error.to_string()))?,
    );
    let mut descriptors = Vec::new();
    let mut sources = BTreeMap::new();
    for (ordinal, (_, owner)) in native.into_iter().enumerate() {
        let sector: [bool; N] = rustred::storage_array(&owner.records.sectors[0].sector, false)
            .expect("admitted arity");
        if ordinal < inputs.len() {
            let terminals = owner.records.sectors[0]
                .finite_residuals
                .iter()
                .map(|record| {
                    if record.symbolic.iter().any(|&symbolic| symbolic) {
                        return Err(AppError::input("deferred declared terminal is symbolic"));
                    }
                    IntegralKey::try_new(record.values.iter().map(|&value| i64::from(value)))
                        .map_err(|error| AppError::input(error.to_string()))
                })
                .collect::<Result<BTreeSet<_>, _>>()?;
            descriptors.push(DeferredCandidateOwnerDescriptor {
                sector,
                saved_root: rustred::storage_array(&owner.records.root_sector, false)
                    .expect("admitted arity"),
                ordering: owner.ordering.clone(),
                terminals,
            });
            sources.insert(
                sector,
                Source {
                    baseline: owner,
                    preferred: None,
                    subset: rule_subsets.get(&sector).map(|slice| slice.to_vec()),
                    overlays: Vec::new(),
                },
            );
        } else {
            sources
                .get_mut(&sector)
                .expect("admitted preference owner")
                .preferred = Some(owner);
        }
    }
    for overlay in overlays {
        let owner = sources
            .get_mut(&overlay.owner)
            .ok_or_else(|| AppError::input("deferred overlay owner is missing"))?;
        if owner.preferred.is_some() {
            return Err(AppError::input("deferred preference and overlay collision"));
        }
        owner.overlays.push(Overlay {
            bytes: Arc::from(overlay.bytes),
            baseline_digest: overlay.baseline_digest,
        });
    }
    for (sector, ordinals) in rule_subsets {
        let owner = sources
            .get(sector)
            .ok_or_else(|| AppError::input("deferred preferred subset has no owner"))?;
        let preferred = owner
            .preferred
            .as_ref()
            .ok_or_else(|| AppError::input("deferred preferred subset has no preferred program"))?;
        if ordinals.windows(2).any(|pair| pair[0] >= pair[1])
            || ordinals
                .iter()
                .any(|&ordinal| ordinal >= preferred.records.sectors[0].rules.len())
        {
            return Err(AppError::input(
                "deferred preferred subset has invalid saved ordinals",
            ));
        }
    }
    let loader = Arc::new(Loader {
        sources: sources
            .into_iter()
            .map(|(sector, source)| (sector, Mutex::new(Some(source))))
            .collect(),
        limits,
    });
    DeferredCandidateOwnerPrograms::try_new(context, descriptors, loader)
        .map_err(|error| AppError::execution(error.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use rustred::algebra::{Coefficient, CoefficientContext};
    use rustred::persistence::{CoefficientId, CoefficientTableBuilder};
    use rustred::sector::Mask;
    use rustred::solver::RoutedCandidateReducer;

    fn fixture() -> (Arc<IntegralFamily>, Vec<u8>) {
        let mut request = crate::FamilyCandidatesRequest::new(
            r#"
schema="rustred.project.toml.v1"
[family]
name="deferred_native_owner_tests"
loop_momenta=["q"]
external_momenta=[]
dimension="d"
[[family.denominators]]
id="P"
expression="q^2-1"
[target]
powers=[1]
"#,
        );
        request.numerical_depth = 0;
        request.max_numerator_rank = Some(2);
        let bytes = crate::family_candidates(request).unwrap().bundle().to_vec();
        let decoded = codec::read(&bytes, Default::default()).unwrap();
        let family =
            Arc::new(super::super::reconstruct_family::<1>(&decoded, Default::default()).unwrap());
        (family, bytes)
    }

    fn replace_first_coefficient(
        bytes: &[u8],
        replacement: impl FnOnce(&Coefficient) -> Coefficient,
    ) -> Vec<u8> {
        let decoded = codec::read(bytes, Default::default()).unwrap();
        let mut records = decoded.records.clone();
        let old = records.sectors[0].rules[0].rhs[0].coefficient;
        let mut table = CoefficientTableBuilder::new(Default::default());
        for i in 0..decoded.coefficients.len() {
            let id = CoefficientId::try_from_index(i).unwrap();
            assert_eq!(
                table
                    .intern(decoded.coefficients.coefficient(id).unwrap())
                    .unwrap(),
                id
            );
        }
        let value = replacement(
            decoded
                .coefficients
                .coefficient(CoefficientId::try_from_index(old as usize).unwrap())
                .unwrap(),
        );
        records.sectors[0].rules[0].rhs[0].coefficient =
            table.intern(&value).unwrap().index() as u32;
        codec::write_records(
            &records,
            &decoded.family,
            &table.finish().unwrap(),
            Default::default(),
        )
        .unwrap()
    }

    #[test]
    fn malformed_native_formula_fails_once_on_first_owner_use() {
        let (family, bytes) = fixture();
        let foreign = CoefficientContext::try_new(["foreign_parameter"])
            .unwrap()
            .one();
        let bytes = replace_first_coefficient(&bytes, |_| foreign);
        let mask = Mask::try_new([true]).unwrap();
        let catalogue = Arc::new(
            prepare_deferred_candidate_owners::<1>(
                family.clone(),
                &[CandidateOwnerBundle {
                    bytes: &bytes,
                    owner_sector: &mask,
                }],
                &[],
                &BTreeMap::new(),
                &[],
                Default::default(),
                Default::default(),
            )
            .unwrap(),
        );
        assert_eq!(catalogue.statistics().prepared_owners, 0);
        let mut application = rustred::solver::RoutedCandidateApplication::try_new_deferred(
            catalogue.clone(),
            [],
            Default::default(),
        )
        .unwrap();
        let key = IntegralKey::try_new([2]).unwrap();
        let first = application
            .reduce_linear_combination([(key.clone(), family.coefficient_context().one())])
            .unwrap_err();
        let second = application
            .reduce_linear_combination([(key, family.coefficient_context().one())])
            .unwrap_err();
        assert_eq!(first, second);
        assert_eq!(catalogue.statistics().preparation_attempts, 1);
        assert_eq!(catalogue.statistics().failed_owners, 1);
        assert_eq!(application.cache_statistics().cached_integrals(), 0);
    }

    #[test]
    fn deferred_preferred_subsets_match_existing_eager_dispatch() {
        let (family, bytes) = fixture();
        let preferred = replace_first_coefficient(&bytes, |value| value + value);
        let mask = Mask::try_new([true]).unwrap();
        let inputs = [CandidateOwnerBundle {
            bytes: &bytes,
            owner_sector: &mask,
        }];
        let preferences = [CandidateOwnerBundle {
            bytes: &preferred,
            owner_sector: &mask,
        }];
        for ordinals in [None, Some(Vec::<usize>::new()), Some(vec![0])] {
            let subsets = ordinals
                .as_ref()
                .map(|v| BTreeMap::from([([true], v.as_slice())]))
                .unwrap_or_default();
            let (_, eager) =
                owners::load_generated_candidate_owners_with_preference_rule_subsets::<1>(
                    &inputs,
                    &preferences,
                    &subsets,
                    Default::default(),
                    Default::default(),
                )
                .unwrap();
            let mut eager =
                RoutedCandidateReducer::try_new(Arc::new(eager), [], Default::default())
                    .unwrap()
                    .application();
            let catalogue = Arc::new(
                prepare_deferred_candidate_owners::<1>(
                    family.clone(),
                    &inputs,
                    &preferences,
                    &subsets,
                    &[],
                    Default::default(),
                    Default::default(),
                )
                .unwrap(),
            );
            let mut lazy = rustred::solver::RoutedCandidateApplication::try_new_deferred(
                catalogue.clone(),
                [],
                Default::default(),
            )
            .unwrap();
            for power in [1, 2, 3] {
                let key = IntegralKey::try_new([power]).unwrap();
                let input = [(key, family.coefficient_context().one())];
                let expected = eager.reduce_linear_combination(input.clone()).unwrap();
                let actual = lazy.reduce_linear_combination(input).unwrap();
                assert_eq!(actual.terms(), expected.terms());
                assert_eq!(actual.nonzero_conditions(), expected.nonzero_conditions());
            }
            assert_eq!(catalogue.statistics().prepared_owners, 1);
            assert_eq!(catalogue.statistics().preparation_attempts, 1);
        }
    }
}
