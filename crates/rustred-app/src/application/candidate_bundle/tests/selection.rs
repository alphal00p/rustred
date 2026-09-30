//! Runtime work selection reuses the ordinary solver and never grants closure.

use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

use rustred::family::IntegralKey;
use rustred::solver::CandidateReductionError;

use super::*;

fn select(request: &mut FamilyCandidatesRequest, masks: &[&str]) {
    request.selected_sectors = Some(
        FamilyCandidatesRequest::parse_selected_sectors(
            &masks
                .iter()
                .map(|mask| (*mask).to_owned())
                .collect::<Vec<_>>(),
        )
        .unwrap(),
    );
}

fn saved(directory: &Path) -> BTreeMap<String, Vec<u8>> {
    fs::read_dir(directory)
        .unwrap()
        .map(|entry| {
            let entry = entry.unwrap();
            (
                entry.file_name().into_string().unwrap(),
                fs::read(entry.path()).unwrap(),
            )
        })
        .collect()
}

fn decode(bytes: &[u8]) -> Vec<([bool; 3], rustred::solver::SectorSolution<3>)> {
    let bundle = codec::read(bytes, Default::default()).unwrap();
    let family = preparation::family(&bundle.family_source, InputFormat::Toml).unwrap();
    let context = ParametricIbpGenerator::try_new(&family)
        .unwrap()
        .context()
        .clone();
    let sources = rustred::solver::SourceSystem::<3>::from_family(&family).unwrap();
    codec::solutions::<3>(
        &bundle,
        &context,
        sources.index_variables(),
        Default::default(),
    )
    .unwrap()
}

#[test]
fn selected_default_and_complete_set_preserve_the_ordinary_program() {
    let request = FamilyCandidatesRequest::new(K3);
    assert!(request.selected_sectors.is_none());
    let ordinary = family_candidates(request.clone()).unwrap();
    let info = inspect_generated_candidate_bundle(ordinary.bundle(), Default::default()).unwrap();
    assert_eq!(info.root_sector, vec![true; 3]);
    let mut explicit = request;
    // User order is irrelevant to set identity or generated canonical ordinals.
    explicit.selected_sectors = Some(info.sectors.iter().rev().cloned().collect());
    let generated = family_candidates(explicit.clone()).unwrap();
    assert_same_program(ordinary.bundle(), generated.bundle());
    assert!(info.sectors.len() > 1);
    explicit.n_cores = 2;
    let parallel = family_candidates(explicit).unwrap();
    assert_same_program(generated.bundle(), parallel.bundle());
    let report: toml::Value = toml::from_str(generated.to_toml()).unwrap();
    assert_eq!(
        report["generation_scope"].as_str(),
        Some("selected-sectors")
    );
    assert_eq!(
        report["selected_sectors"].as_array().unwrap().len(),
        info.solved_sectors
    );
}

#[test]
fn selected_subset_keeps_exact_rules_original_axes_and_missing_successors() {
    let mut request = FamilyCandidatesRequest::new(K3);
    request.permutation = Some(vec![2, 0, 1]);
    let ordinary = family_candidates(request.clone()).unwrap();
    select(&mut request, &["111"]);
    let selected = family_candidates(request.clone()).unwrap();
    let info = inspect_generated_candidate_bundle(selected.bundle(), Default::default()).unwrap();
    assert_eq!(info.sectors, vec![vec![true; 3]]);
    assert_eq!(info.solved_sectors, 1);
    let mut all = decode(ordinary.bundle());
    let expected = all
        .remove(
            all.iter()
                .position(|(sector, _)| *sector == [true; 3])
                .unwrap(),
        )
        .1;
    let actual = decode(selected.bundle()).pop().unwrap().1;
    assert_eq!(
        actual.order.persisted_policy().unwrap(),
        expected.order.persisted_policy().unwrap()
    );
    assert_eq!(actual.finite_residuals, expected.finite_residuals);
    assert_eq!(actual.rules.len(), expected.rules.len());
    for (left, right) in actual.rules.iter().zip(&expected.rules) {
        assert_eq!(left.candidate.case, right.candidate.case);
        assert_eq!(left.candidate.target, right.candidate.target);
        assert_eq!(left.candidate.sources, right.candidate.sources);
        assert_eq!(left.exceptions.branches, right.exceptions.branches);
        assert_eq!(left.candidate.rhs.len(), right.candidate.rhs.len());
        for (a, b) in left.candidate.rhs.iter().zip(&right.candidate.rhs) {
            assert_eq!(a.integral, b.integral);
            assert_eq!(a.coefficient, b.coefficient);
        }
    }
    request.n_cores = 2;
    assert_same_program(
        selected.bundle(),
        family_candidates(request).unwrap().bundle(),
    );
    let (_, mut reducer) = load_generated_candidate_bundle::<3>(
        selected.bundle(),
        Default::default(),
        Default::default(),
    )
    .unwrap();
    let missing = IntegralKey::try_new([0, 1, 1]).unwrap();
    assert!(
        matches!(reducer.reduce_unit_mass(&missing), Err(CandidateReductionError::Uncovered { target }) if target == missing)
    );
    // A proved scaleless sector is still zero; it was not reclassified by selection.
    let zero = IntegralKey::try_new([0, 0, 1]).unwrap();
    assert!(reducer.reduce_unit_mass(&zero).unwrap().terms().is_empty());
    assert!(certify_candidates(CandidateCertificationRequest::new(selected.bundle())).is_err());
}

#[test]
fn selected_invalid_jobs_fail_before_checkpoint_publication() {
    for (masks, nonpositive, expected) in [
        (vec![], vec![], "empty"),
        (vec!["111", "111"], vec![], "duplicate"),
        (vec!["11"], vec![], "arity"),
        (vec!["111"], vec![0], "outside"),
        (vec!["001"], vec![], "proved zero"),
    ] {
        let directory = super::checkpoint::Directory::new();
        let mut request = FamilyCandidatesRequest::new(K3);
        request.nonpositive_indices = nonpositive;
        request.selected_sectors = Some(
            masks
                .iter()
                .map(|mask| mask.bytes().map(|b| b == b'1').collect())
                .collect(),
        );
        request.checkpoint = Some(directory.options());
        let error = family_candidates(request).unwrap_err();
        assert!(error.message().contains(expected), "{expected}: {error}");
        assert!(!directory.options().directory.exists());
    }
}

#[test]
fn selected_checkpoint_resume_load_and_scope_mismatch_are_exact() {
    let directory = super::checkpoint::Directory::new();
    let mut request = FamilyCandidatesRequest::new(K3);
    select(&mut request, &["111", "011"]);
    request.checkpoint = Some(directory.options());
    let original = family_candidates(request.clone()).unwrap();
    request.checkpoint.as_mut().unwrap().resume = true;
    request.selected_sectors.as_mut().unwrap().reverse();
    let before = saved(&directory.options().directory);
    let resumed = family_candidates(request.clone()).unwrap();
    assert_same_program(original.bundle(), resumed.bundle());
    let report: toml::Value = toml::from_str(resumed.to_toml()).unwrap();
    assert_eq!(
        report["checkpoint"]["newly_solved_sectors"].as_integer(),
        Some(0)
    );
    assert_eq!(report["checkpoint"]["reused_sectors"].as_integer(), Some(2));
    // A complete selected checkpoint loads without constructing any executor.
    request.n_cores = 0;
    let (_, mut loaded) =
        load_generated_candidate_checkpoint::<3>(&request, Default::default()).unwrap();
    let missing = IntegralKey::try_new([1, 0, 1]).unwrap();
    assert!(
        matches!(loaded.reduce_unit_mass(&missing), Err(CandidateReductionError::Uncovered { target }) if target == missing)
    );
    assert_eq!(before, saved(&directory.options().directory));
    for masks in [
        None,
        Some(vec![vec![true; 3]]),
        Some(vec![vec![true, false, true], vec![true; 3]]),
    ] {
        let mut changed = request.clone();
        changed.selected_sectors = masks;
        assert!(load_generated_candidate_checkpoint::<3>(&changed, Default::default()).is_err());
        changed.n_cores = 1;
        assert!(family_candidates(changed).is_err());
        assert_eq!(before, saved(&directory.options().directory));
    }
}

#[test]
fn selected_source_plan_must_match_selected_not_full_root_inventory() {
    let directory = super::checkpoint::Directory::new();
    let mut request = FamilyCandidatesRequest::new(K3);
    select(&mut request, &["111"]);
    request.checkpoint = Some(directory.options());
    request.discovery_strategy = Some(CandidateDiscoveryStrategy {
        rows: CandidateSourcePriority::Materialized {
            sectors: vec![CandidateSourceVisitPlan {
                sector: vec![false, true, true],
                ordinals: vec![],
            }],
        },
        ..Default::default()
    });
    assert!(
        family_candidates(request)
            .unwrap_err()
            .message()
            .contains("canonical prepared sector")
    );
    assert!(!directory.options().directory.exists());
}

#[test]
fn selected_mask_parser_is_strict_and_preserves_axis_positions() {
    assert_eq!(
        FamilyCandidatesRequest::parse_selected_sectors(&["011".into(), "101".into()]).unwrap(),
        vec![vec![false, true, true], vec![true, false, true]]
    );
    for masks in [
        vec![],
        vec!["".into()],
        vec!["10a".into()],
        vec![" 01".into()],
        vec!["1".repeat(17)],
        vec!["１".into()],
    ] {
        assert!(FamilyCandidatesRequest::parse_selected_sectors(&masks).is_err());
    }
}

#[test]
fn selected_jobs_compose_with_runtime_source_and_mathematical_order() {
    let mut request = FamilyCandidatesRequest::new(K1);
    select(&mut request, &["1"]);
    request.discovery_strategy = Some(CandidateDiscoveryStrategy {
        rows: CandidateSourcePriority::Features {
            priorities: vec![CandidateRowPriority {
                feature: CandidateRowFeature::Terms,
                descending: false,
            }],
        },
        ..Default::default()
    });
    request.integral_order = Some(
        CandidateIntegralOrder::from_json(
            r#"{
        "version":1,"support_weights":[0],"support_priority":[0],
        "degree_rows":[{"active":[1],"inactive":[1]}],
        "coordinate_priority":[0],"coordinate_groups":"active-first",
        "active_direction":"descending","inactive_direction":"descending"
    }"#,
        )
        .unwrap(),
    );
    let generated = family_candidates(request.clone()).unwrap();
    let info = inspect_generated_candidate_bundle(generated.bundle(), Default::default()).unwrap();
    let expected =
        rustred::sector::OrderingPolicy::try_programmed(request.integral_order.unwrap()).unwrap();
    assert_eq!(info.integral_order, expected.stable_id().as_str());
    assert_eq!(info.sectors, vec![vec![true]]);
    let report: toml::Value = toml::from_str(generated.to_toml()).unwrap();
    assert_eq!(
        report["discovery_strategy"]["rows"]["kind"].as_str(),
        Some("features")
    );
    let (_, mut loaded) = load_generated_candidate_bundle::<1>(
        generated.bundle(),
        Default::default(),
        Default::default(),
    )
    .unwrap();
    assert_eq!(loaded.ordering(), expected);
    assert!(
        !loaded
            .reduce_unit_mass(&IntegralKey::try_new([3]).unwrap())
            .unwrap()
            .terms()
            .is_empty()
    );
}
