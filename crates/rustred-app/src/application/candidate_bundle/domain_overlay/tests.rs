use std::sync::Arc;

use rustred::family::IntegralKey;
use rustred::persistence::{
    BinaryProgramKind, BinarySection, SectionTag, encode_program, inspect_program,
};
use rustred::sector::Mask;
use rustred::solver::{
    BoundOwnerOverlay, CandidateOwnerPrograms, CoordinateCase, FiniteCasePolicy, OwnerDomainScope,
    OwnerFeedbackPolicy, RoutedCandidateReducer,
};

use super::super::{
    CandidateOwnerBundle, CandidateOwnerLoadLimits, FamilyCandidatesRequest, codec as sector_codec,
    family_candidates, load_generated_candidate_bundle, load_generated_candidate_owners,
};
use super::*;

const FAMILY: &str = r#"
schema = "rustred.project.toml.v1"
[family]
name = "partial_overlay_tadpole"
loop_momenta = ["q"]
external_momenta = []
dimension = "d"
[[family.denominators]]
id = "P"
expression = "q^2-1"
[target]
powers = [1]
"#;

fn base() -> (Vec<u8>, Arc<CandidateOwnerPrograms<1>>) {
    let output = family_candidates(FamilyCandidatesRequest::new(FAMILY)).unwrap();
    let mut bundle = sector_codec::read(output.bundle(), Default::default()).unwrap();
    // A partial input for this dispatch test: keep the existing I(1) terminal
    // but omit its old recurrence. This is not a sector-completeness claim.
    assert_eq!(bundle.sectors.len(), 1);
    bundle.sectors[0].rules.clear();
    let bytes = sector_codec::write(&bundle, Default::default()).unwrap();
    let programs = cold_base(&bytes);
    (bytes, programs)
}

fn cold_base(bytes: &[u8]) -> Arc<CandidateOwnerPrograms<1>> {
    let mask = Mask::try_new([true]).unwrap();
    let (_, programs) = load_generated_candidate_owners::<1>(
        &[CandidateOwnerBundle {
            bytes,
            owner_sector: &mask,
        }],
        Default::default(),
        Default::default(),
    )
    .unwrap();
    Arc::new(programs)
}

fn patch(programs: &Arc<CandidateOwnerPrograms<1>>) -> BoundOwnerOverlay<1> {
    programs
        .bind_owner_search(
            [true],
            OwnerFeedbackPolicy {
                numerical_depth: 1,
                ..Default::default()
            },
        )
        .unwrap()
        .solve_domains_with_observer(
            vec![CoordinateCase::new([Some(2)]).unwrap().into()],
            OwnerDomainScope {
                max_numerator_rank: Some(0),
                finite_case_policy: FiniteCasePolicy::SearchFinite,
            },
            Default::default(),
            |_| {},
        )
        .unwrap()
}

fn saved() -> (Vec<u8>, Arc<CandidateOwnerPrograms<1>>, Vec<u8>) {
    let (base, programs) = base();
    let overlay = patch(&programs);
    assert_eq!(overlay.terminal_count(), 0);
    let bytes = encode_generated_domain_overlay(
        &programs,
        &overlay,
        *blake3::hash(&base).as_bytes(),
        Default::default(),
    )
    .unwrap();
    (base, programs, bytes)
}

fn mutate(bytes: &[u8], edit: impl FnOnce(&mut model::DomainRecord)) -> Vec<u8> {
    let (envelope, mut record, _) = codec::read_structure(bytes, Default::default()).unwrap();
    edit(&mut record);
    let program = bincode::encode_to_vec(&record, bincode::config::standard()).unwrap();
    let sections = envelope
        .sections()
        .iter()
        .map(|part| BinarySection {
            tag: part.tag,
            bytes: if part.tag == SectionTag::PROGRAM {
                &program
            } else {
                part.bytes
            },
        })
        .collect::<Vec<_>>();
    encode_program(
        BinaryProgramKind::DomainRules,
        &sections,
        CandidateBundleLimits::default().binary_limits(),
    )
    .unwrap()
}

#[test]
fn domain_overlay_cold_roundtrip_replays_and_preserves_existing_terminals() {
    let (base, old, bytes) = saved();
    assert_eq!(
        inspect_program(&bytes, Default::default()).unwrap().kind(),
        BinaryProgramKind::DomainRules
    );
    let programs = cold_base(&base);
    let (overlay, replay) = load_generated_domain_overlay(
        &programs,
        &bytes,
        [true],
        *blake3::hash(&base).as_bytes(),
        Default::default(),
    )
    .unwrap();
    assert!(!replay.rules.is_empty());
    assert_eq!(replay.rules.len(), overlay.rule_count());
    assert_eq!(overlay.scope().max_numerator_rank, Some(0));
    assert_eq!(programs.context().scope().max_numerator_rank, None);
    assert!(programs.owns_domain_overlay(&overlay));
    assert!(!old.owns_domain_overlay(&overlay));
    let terminal_count = programs.terminal_count();
    let patched = programs
        .append_residual_free_domain_overlays(vec![overlay], Default::default())
        .unwrap();
    assert_eq!(patched.terminal_count(), terminal_count);
    let trace = RoutedCandidateReducer::try_new(patched, [], Default::default())
        .unwrap()
        .trace_targets([IntegralKey::try_new([2]).unwrap()])
        .unwrap();
    assert!(trace.frontier().is_empty());
    assert_eq!(trace.rule_applications(), 1);
    assert_eq!(programs.overlays(&[true]).count(), 0);
}

#[test]
fn domain_overlay_cannot_be_loaded_as_complete_sector_candidates() {
    let (_, _, bytes) = saved();
    assert!(sector_codec::read(&bytes, Default::default()).is_err());
    assert!(
        load_generated_candidate_bundle::<1>(&bytes, Default::default(), Default::default())
            .is_err()
    );
}

#[test]
fn domain_overlay_binding_and_identity_mutations_fail_closed() {
    let (base, programs, bytes) = saved();
    let digest = *blake3::hash(&base).as_bytes();
    assert!(
        load_generated_domain_overlay(&programs, &bytes, [false], digest, Default::default())
            .is_err()
    );
    assert!(
        load_generated_domain_overlay(&programs, &bytes, [true], [0; 32], Default::default())
            .is_err()
    );
    for altered in [
        mutate(&bytes, |r| r.family_fingerprint.push_str("wrong")),
        mutate(&bytes, |r| r.root_sector[0] = false),
        mutate(&bytes, |r| r.integral_order.push_str("wrong")),
        mutate(&bytes, |r| r.rules[0].sources[0].basis_row = usize::MAX),
        mutate(&bytes, |r| r.rules[0].rhs.clear()),
        mutate(&bytes, |r| r.rules[0].rhs[0].coefficient = u32::MAX),
        mutate(&bytes, |r| {
            r.finite_residuals.push(r.rules[0].target.clone())
        }),
        mutate(&bytes, |r| r.requested_cases.clear()),
    ] {
        assert!(
            load_generated_domain_overlay(&programs, &altered, [true], digest, Default::default())
                .is_err()
        );
        assert_eq!(programs.overlays(&[true]).count(), 0);
    }
    let mut trailing = bytes.clone();
    trailing.push(0);
    for altered in [&bytes[..bytes.len() - 1], trailing.as_slice()] {
        assert!(
            load_generated_domain_overlay(&programs, altered, [true], digest, Default::default())
                .is_err()
        );
    }
}

#[test]
fn domain_overlay_export_rejects_residuals_and_foreign_lineages() {
    let (base, programs) = base();
    let residual = programs
        .bind_owner_search(
            [true],
            OwnerFeedbackPolicy {
                numerical_depth: 0,
                ..Default::default()
            },
        )
        .unwrap()
        .solve_domains_with_observer(
            vec![CoordinateCase::new([Some(1)]).unwrap().into()],
            OwnerDomainScope {
                max_numerator_rank: Some(0),
                finite_case_policy: FiniteCasePolicy::SearchFinite,
            },
            Default::default(),
            |_| {},
        )
        .unwrap();
    assert!(residual.terminal_count() > 0);
    assert!(
        encode_generated_domain_overlay(
            &programs,
            &residual,
            *blake3::hash(&base).as_bytes(),
            Default::default()
        )
        .is_err()
    );
    let other = cold_base(&base);
    assert!(
        encode_generated_domain_overlay(
            &other,
            &patch(&programs),
            *blake3::hash(&base).as_bytes(),
            Default::default()
        )
        .is_err()
    );
}

#[test]
fn domain_overlay_combined_ingress_counts_base_and_patch_before_import() {
    let (base, _, bytes) = saved();
    let mask = Mask::try_new([true]).unwrap();
    let inputs = [CandidateOwnerBundle {
        bytes: &base,
        owner_sector: &mask,
    }];
    let patches = [bytes.as_slice()];
    validate_domain_overlay_ingress(&inputs, &patches, Default::default()).unwrap();
    let mut limits = CandidateOwnerLoadLimits::default();
    limits.max_total_input_bytes = base.len() + bytes.len() - 1;
    assert_eq!(
        validate_domain_overlay_ingress(&inputs, &patches, limits)
            .unwrap_err()
            .kind(),
        crate::AppErrorKind::Limit
    );
    let base_view = inspect_program(&base, Default::default()).unwrap();
    let patch_view = inspect_program(&bytes, Default::default()).unwrap();
    limits = Default::default();
    limits.max_total_symbolica_state_bytes = base_view
        .section(SectionTag::SYMBOLICA_STATE)
        .unwrap()
        .len()
        + patch_view
            .section(SectionTag::SYMBOLICA_STATE)
            .unwrap()
            .len()
        - 1;
    assert_eq!(
        validate_domain_overlay_ingress(&inputs, &patches, limits)
            .unwrap_err()
            .kind(),
        crate::AppErrorKind::Limit
    );
    limits = Default::default();
    limits.bundle.max_total_coefficient_bytes = base_view
        .section(SectionTag::COEFFICIENTS)
        .unwrap()
        .len()
        .max(patch_view.section(SectionTag::COEFFICIENTS).unwrap().len());
    assert_eq!(
        validate_domain_overlay_ingress(&inputs, &patches, limits)
            .unwrap_err()
            .kind(),
        crate::AppErrorKind::Limit
    );
}
