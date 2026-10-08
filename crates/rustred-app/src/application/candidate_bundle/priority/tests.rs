use std::ops::ControlFlow;
use std::sync::atomic::AtomicBool;

use rustred::algebra::IndexedCoefficientContext;
use rustred::family::IntegralFamily;
use rustred::foundry::artifact::OriginalSourceContribution;
use rustred::foundry::cell::FixedIndexRestriction;
use rustred::identity::{IntegralShift, RowId, TranslatedSourceRequest};
use rustred::sector::{Mask, OrderingPolicy};
use rustred::solver::{CandidateOwnerPrograms, OwnerDomainMatchDisposition};

use super::super::{
    CandidateOwnerBundle, FamilyCandidatesRequest, family_candidates,
    load_generated_candidate_owners,
};
use super::*;

mod after_baseline;
mod lower_cuts;

const FAMILY: &str = r#"
schema = "rustred.project.toml.v1"
[family]
name = "checked_priority_tadpole"
loop_momenta = ["q"]
external_momenta = []
dimension = "d"
[[family.denominators]]
id = "P"
expression = "q^2-1"
[target]
powers = [1]
"#;

fn base() -> (Vec<u8>, IntegralFamily) {
    let bytes = family_candidates(FamilyCandidatesRequest::new(FAMILY))
        .unwrap()
        .bundle()
        .to_vec();
    let limits = CandidateBundleLimits::default();
    let bundle = codec::read(&bytes, limits).unwrap();
    let family = bundle
        .family
        .to_family(
            &bundle.coefficients,
            limits.family_limits(),
            limits.binary_limits(),
        )
        .unwrap();
    (bytes, family)
}

fn request(
    family: &IntegralFamily,
) -> (IndexedCoefficientContext, OriginalSourceCombinationRequest) {
    let generator = ParametricIbpGenerator::try_new(family).unwrap();
    let c = generator.context().clone();
    let batch = generator.prepare_ordinary_ibp().unwrap();
    let rows = (0..batch.len()).map(|i| batch.generate(i)).collect();
    let completed = batch.complete(rows).unwrap();
    let translated = generator
        .translate_selected_completed_source_rows(
            &completed,
            [TranslatedSourceRequest::new(
                0,
                IntegralShift::try_new([-1]).unwrap(),
            )],
            Default::default(),
        )
        .unwrap();
    let rhs_shift = translated.sources()[0]
        .terms()
        .keys()
        .find(|shift| shift.values() == [-1])
        .unwrap()
        .clone();
    let n1 = c.sub(&c.index(0).unwrap(), &c.one()).unwrap();
    let twice = c.mul(&c.integer(2), &n1).unwrap();
    let d = c.lift(&c.base().parameter("d").unwrap()).unwrap();
    let rhs = c.div(&c.sub(&d, &twice).unwrap(), &twice).unwrap();
    let weight = c.div(&c.integer(-1), &twice).unwrap();
    let proposal = OriginalSourceCombinationRequest {
        root_sector: Mask::try_new([true]).unwrap(),
        sector: Mask::try_new([true]).unwrap(),
        ordering: OrderingPolicy::SpiredUncutV1,
        lower: vec![1],
        upper: vec![None],
        fixed: Vec::new(),
        contributions: vec![OriginalSourceContribution {
            source_row: RowId::OrdinaryIbp {
                contraction_momentum: 0,
                differentiated_loop: 0,
            },
            offset: IntegralShift::try_new([-1]).unwrap(),
            weight,
        }],
        rhs: vec![(rhs_shift, rhs)],
        retained_conditions: Vec::new(),
    };
    (c, proposal)
}

fn load(bytes: &[u8]) -> CandidateOwnerPrograms<1> {
    let owner = Mask::try_new([true]).unwrap();
    load_generated_candidate_owners::<1>(
        &[CandidateOwnerBundle {
            bytes,
            owner_sector: &owner,
        }],
        Default::default(),
        Default::default(),
    )
    .unwrap()
    .1
}

fn disposition(programs: &CandidateOwnerPrograms<1>, power: u64) -> OwnerDomainMatchDisposition {
    let mut found = Vec::new();
    programs
        .visit_owner_domain_matches(
            [true],
            &[power - 1],
            &[Some(power - 1)],
            Some(0),
            Default::default(),
            &AtomicBool::new(false),
            |piece| {
                found.push(piece.disposition());
                ControlFlow::Continue(())
            },
        )
        .unwrap();
    assert_eq!(found.len(), 1);
    found[0]
}

#[test]
fn checked_priority_roundtrip_preserves_every_old_record_and_coefficient() {
    let (bytes, family) = base();
    let (_, proposal) = request(&family);
    let export = encode_checked_priority_owner::<1>(
        &bytes,
        proposal,
        Default::default(),
        Default::default(),
    )
    .unwrap();
    let old = codec::read(&bytes, Default::default()).unwrap();
    let new = codec::read(export.bytes(), Default::default()).unwrap();
    assert_eq!(&new.sectors[0].rules[1..], old.sectors[0].rules.as_slice());
    assert!(new.sectors[0].rules[0].sources.is_empty());
    assert_eq!(
        new.sectors[0].finite_residuals,
        old.sectors[0].finite_residuals
    );
    let mut restored_metadata = new.records.clone();
    restored_metadata.sectors[0].rules.remove(0);
    assert_eq!(restored_metadata, old.records);
    assert_eq!(new.family, old.family);
    for i in 0..old.coefficients.len() {
        let id = CoefficientId::try_from_index(i).unwrap();
        assert_eq!(
            old.coefficients.coefficient(id).unwrap(),
            new.coefficients.coefficient(id).unwrap()
        );
    }
    assert_eq!(
        export.base_owner_blake3(),
        blake3::hash(&bytes).to_hex().as_str()
    );
    assert_eq!(
        export.owner_blake3(),
        blake3::hash(export.bytes()).to_hex().as_str()
    );
    assert_eq!(export.proof().requested_bounds(), (&[1][..], &[None][..]));
    assert!(!export.proof().cells().next().unwrap().sources().is_empty());
    // The actual normalized RHS pole is retained in the immutable proof;
    // weight-unit discharge must not reclassify it as an optional condition.
    assert!(
        export
            .proof()
            .cells()
            .any(|cell| cell.rule().nonzero_guards().iter().any(|guard| guard
                .origins()
                .iter()
                .any(|origin| matches!(
                    origin,
                    ParametricGuardOrigin::RuleCoefficientDenominator { .. }
                ))))
    );
    let programs = load(export.bytes());
    assert_eq!(
        disposition(&programs, 2),
        OwnerDomainMatchDisposition::SelectedRule { batch: 0, rule: 0 }
    );
    assert_eq!(
        disposition(&programs, 30),
        OwnerDomainMatchDisposition::SelectedRule { batch: 0, rule: 0 }
    );
    assert_eq!(disposition(&programs, 1), disposition(&load(&bytes), 1));
    assert_eq!(
        disposition(&programs, 1),
        OwnerDomainMatchDisposition::Terminal { batch: 0 }
    );
}

#[test]
fn generic_field_weight_poles_require_exclusive_native_weight_origins() {
    let (_, family) = base();
    let (c, proposal) = request(&family);
    let d = c.lift(&c.base().parameter("d").unwrap()).unwrap();
    let pole = c
        .numerator_condition_with_limits(&c.sub(&d, &c.integer(2)).unwrap(), Default::default())
        .unwrap();
    let weight_origin = ParametricGuardOrigin::SourceCombinationDenominator {
        source_ordinal: 0,
        row_id: proposal.contributions[0].source_row.clone(),
    };
    let eligible = |p: &IndexedPolynomial, origins: &[ParametricGuardOrigin]| {
        is_generic_field_weight_pole(&c, p, origins, Default::default()).unwrap()
    };
    // Classifier-only regression: no sealed proof is fabricated here. The
    // public exporter always checks the full original weighted product first.
    assert!(eligible(&pole, std::slice::from_ref(&weight_origin)));
    assert!(!eligible(&pole, &[]));
    let zero = c
        .numerator_condition_with_limits(&c.integer(0), Default::default())
        .unwrap();
    assert!(!eligible(&zero, std::slice::from_ref(&weight_origin)));
    for required_origin in [
        ParametricGuardOrigin::SourceCondition {
            source_ordinal: 0,
            row_id: proposal.contributions[0].source_row.clone(),
            condition_ordinal: 0,
            condition_sources: Box::new([]),
        },
        ParametricGuardOrigin::SourceCoefficientDenominator {
            source_ordinal: 0,
            row_id: proposal.contributions[0].source_row.clone(),
            shift: proposal.rhs[0].0.clone(),
        },
        ParametricGuardOrigin::OriginalDomainCondition {
            condition_ordinal: 0,
        },
        ParametricGuardOrigin::RuleCoefficientDenominator {
            shift: proposal.rhs[0].0.clone(),
        },
        ParametricGuardOrigin::FinalTargetCoefficient,
    ] {
        assert!(!eligible(&pole, std::slice::from_ref(&required_origin)));
        // The same polynomial with both origins must NOT pass an ANY-origin
        // test. Genuine source/retained/final conditions still require runtime
        // denominator coverage, even when a weight has the identical pole.
        assert!(!eligible(&pole, &[weight_origin.clone(), required_origin]));
    }
}

#[test]
fn generic_field_weight_poles_do_not_discharge_free_index_guards() {
    let (_, family) = base();
    let (c, proposal) = request(&family);
    let d = c.lift(&c.base().parameter("d").unwrap()).unwrap();
    let polynomial = c
        .numerator_condition_with_limits(
            &c.sub(&d, &c.index(0).unwrap()).unwrap(),
            Default::default(),
        )
        .unwrap();
    let origins = [ParametricGuardOrigin::SourceCombinationDenominator {
        source_ordinal: 0,
        row_id: proposal.contributions[0].source_row.clone(),
    }];
    assert!(!is_generic_field_weight_pole(&c, &polynomial, &origins, Default::default()).unwrap());
    // A true exact singleton is different: after n=2 is imposed by the
    // checked cell, d-n is the nonzero base-field element d-2.
    let specialized = c
        .specialize_fixed_polynomial(&polynomial, &[(0, 2)], Default::default())
        .unwrap();
    assert!(is_generic_field_weight_pole(&c, &specialized, &origins, Default::default()).unwrap());
}

#[test]
fn checked_priority_fixed_face_uses_old_fallback_outside_exact_scope() {
    let (bytes, family) = base();
    let (_, mut proposal) = request(&family);
    proposal.fixed = vec![FixedIndexRestriction::new(0, 3)];
    proposal.lower = vec![2];
    proposal.upper = vec![Some(2)];
    let export = encode_checked_priority_owner::<1>(
        &bytes,
        proposal,
        Default::default(),
        Default::default(),
    )
    .unwrap();
    let new = load(export.bytes());
    let old = load(&bytes);
    assert_eq!(
        disposition(&new, 3),
        OwnerDomainMatchDisposition::SelectedRule { batch: 0, rule: 0 }
    );
    for power in [2, 4, 30] {
        let original = disposition(&old, power);
        let expected = match original {
            OwnerDomainMatchDisposition::SelectedRule { batch, rule } => {
                OwnerDomainMatchDisposition::SelectedRule {
                    batch,
                    rule: rule + 1,
                }
            }
            other => other,
        };
        assert_eq!(disposition(&new, power), expected);
    }
    assert_eq!(disposition(&new, 1), disposition(&old, 1));
}

#[test]
fn checked_priority_refuses_unpersisted_parameter_guard_without_changing_base() {
    let (bytes, family) = base();
    let before = bytes.clone();
    let (c, mut proposal) = request(&family);
    let d = c.lift(&c.base().parameter("d").unwrap()).unwrap();
    proposal.retained_conditions.push(
        c.numerator_condition_with_limits(&c.sub(&d, &c.integer(7)).unwrap(), Default::default())
            .unwrap(),
    );
    // A valid generic-d source identity is insufficient: saved runtime must
    // also retain its extra d != 7 assumption, not silently erase it.
    assert!(
        check_original_source_combination(&family, proposal.clone(), Default::default()).is_ok()
    );
    let error = encode_checked_priority_owner::<1>(
        &bytes,
        proposal,
        Default::default(),
        Default::default(),
    )
    .unwrap_err();
    assert!(
        error.to_string().contains("proof guard is not retained"),
        "{error}"
    );
    assert_eq!(bytes, before);
}

#[test]
fn checked_priority_refuses_unrepresentable_bounds_and_widened_fixed_face() {
    let (bytes, family) = base();
    for upper in [Some(4)] {
        let (_, mut proposal) = request(&family);
        proposal.lower[0] = 2;
        proposal.upper[0] = upper;
        let error = encode_checked_priority_owner::<1>(
            &bytes,
            proposal,
            Default::default(),
            Default::default(),
        )
        .unwrap_err();
        assert!(
            error.to_string().contains("cannot exactly encode"),
            "{error}"
        );
    }
    let (_, mut proposal) = request(&family);
    proposal.fixed = vec![FixedIndexRestriction::new(0, 3)];
    assert!(
        encode_checked_priority_owner::<1>(
            &bytes,
            proposal,
            Default::default(),
            Default::default()
        )
        .is_err()
    );
}

#[test]
fn checked_priority_rejects_mutated_source_product_root_and_resource_limits() {
    let (bytes, family) = base();
    let (c, mut proposal) = request(&family);
    proposal.contributions[0].weight = c.one();
    assert!(
        encode_checked_priority_owner::<1>(
            &bytes,
            proposal,
            Default::default(),
            Default::default()
        )
        .is_err()
    );
    let (_, mut proposal) = request(&family);
    proposal.root_sector = Mask::try_new([false]).unwrap();
    assert!(
        encode_checked_priority_owner::<1>(
            &bytes,
            proposal,
            Default::default(),
            Default::default()
        )
        .is_err()
    );
    let (_, proposal) = request(&family);
    let mut limits = OriginalSourceCombinationLimits::default();
    limits.cell.max_retained_terms = 0;
    assert!(
        encode_checked_priority_owner::<1>(&bytes, proposal, limits, Default::default()).is_err()
    );
    let (_, proposal) = request(&family);
    let mut limits = CandidateBundleLimits::default();
    limits.max_bundle_bytes = 0;
    assert!(
        encode_checked_priority_owner::<1>(&bytes, proposal, Default::default(), limits).is_err()
    );
}

#[test]
#[cfg(feature = "capacity-dispatch")]
fn capacity_priority_export_preserves_physical_proof_and_transport() {
    let (bytes, family) = base();
    let (_, proposal) = request(&family);
    let physical = encode_checked_priority_owner::<1>(
        &bytes,
        proposal.clone(),
        Default::default(),
        Default::default(),
    )
    .unwrap();
    let padded = encode_checked_priority_owner::<4>(
        &bytes,
        proposal,
        Default::default(),
        Default::default(),
    )
    .unwrap();
    let physical = codec::read(physical.bytes(), Default::default()).unwrap();
    let padded = codec::read(padded.bytes(), Default::default()).unwrap();
    assert_eq!(physical.records, padded.records);
    assert_eq!(physical.family, padded.family);
    assert_eq!(physical.coefficients.len(), padded.coefficients.len());
    for i in 0..physical.coefficients.len() {
        let id = CoefficientId::try_from_index(i).unwrap();
        assert_eq!(
            physical.coefficients.coefficient(id).unwrap(),
            padded.coefficients.coefficient(id).unwrap()
        );
    }
}
