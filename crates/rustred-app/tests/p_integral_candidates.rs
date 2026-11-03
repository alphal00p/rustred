//! `family-candidates` on massless propagator-type families (one external
//! momentum `q`, `q.q = 1`): generation, the default unit-scale admission of
//! the saved bundle, exact application, and the per-sector failure report.
//!
//! The four-loop crossed-rung family is slow and ignored by default:
//!
//! ```text
//! cargo test --release -p rustred-app --test p_integral_candidates -- --ignored
//! ```
use rustred::family::IntegralKey;
use rustred::reduction::ReductionLimits;
use rustred::solver::CandidateFamilyAdmission;
use rustred_app::{
    CandidateBundleLimits, FamilyCandidatesRequest, SectorFailurePolicy, family_candidates,
    load_generated_candidate_bundle,
};

const KITE: &str = "I(name(p2l_kite),loops(k1,k2),externals(q),dimension(d),\
    prop(D1,k1^2,1),prop(D2,k2^2,1),prop(D3,(k1-q)^2,1),prop(D4,(k2-q)^2,1),\
    prop(D5,(k1-k2)^2,1),gram(q,q,1))";

const LADDER: &str = "I(name(p3l_la),loops(k1,k2,k3),externals(q),dimension(d),\
    prop(D1,k1^2,1),prop(D2,k2^2,1),prop(D3,k3^2,1),prop(D4,(k1-q)^2,1),\
    prop(D5,(k2-q)^2,1),prop(D6,(k3-q)^2,1),prop(D7,(k1-k2)^2,1),prop(D8,(k2-k3)^2,1),\
    prop(D9,(k1-k3)^2,0),gram(q,q,1))";

const CROSSED: &str = "I(name(p4l_np),loops(k1,k2,k3,k4),externals(q),dimension(d),\
    prop(D1,(-k1+k3+q)^2,1),prop(D2,(-k2+k3+k4+q)^2,1),prop(D3,(-k2+k4+q)^2,1),\
    prop(D4,(-k2+q)^2,1),prop(D5,(k1-k3)^2,1),prop(D6,(k1)^2,1),prop(D7,(k2-k4)^2,1),\
    prop(D8,(k2)^2,1),prop(D9,(-k1+k2-k4)^2,1),prop(D10,(k3)^2,1),prop(D11,(k4)^2,1),\
    prop(D12,(k1-k2)^2,0),prop(D13,(k2-k3)^2,0),prop(D14,(k3-k4)^2,0),gram(q,q,1))";

fn report(toml: &str) -> toml::Table {
    toml.parse().unwrap()
}

fn key(powers: &[i64]) -> IntegralKey {
    IntegralKey::try_new(powers.to_vec()).unwrap()
}

#[test]
fn kite_bundle_applies_under_the_unit_scale_propagator_admission() {
    let generated = family_candidates(FamilyCandidatesRequest::new(KITE)).unwrap();
    let summary = report(generated.to_toml());
    assert_eq!(summary["solved_sectors"].as_integer(), Some(8));
    assert_eq!(summary["finite_residuals"].as_integer(), Some(3));
    assert!(!summary.contains_key("failed_sectors"));
    let (_, mut reducer) = load_generated_candidate_bundle::<5>(
        generated.bundle(),
        CandidateBundleLimits::default(),
        ReductionLimits::default(),
    )
    .unwrap();
    assert_eq!(
        reducer.admission(),
        CandidateFamilyAdmission::UnitScalePropagator
    );
    let target = key(&[2, 1, 1, 1, -1]);
    let decomposition = reducer.reduce_unit_mass(&target).unwrap();
    assert!(decomposition.unresolved().is_empty());
    assert!(!decomposition.terms().is_empty());
    for terminal in decomposition.terms().keys() {
        assert!(reducer.terminals().contains(terminal));
        // Dimension five minus the terminal's: each lost unit is one q^2.
        let scale: i64 = terminal.powers().iter().sum::<i64>() - 4;
        assert_eq!(
            decomposition.common_mass_squared_power(terminal).unwrap(),
            i128::from(scale)
        );
    }
}

#[test]
fn three_loop_ladder_generates_its_physical_downset() {
    let mut request = FamilyCandidatesRequest::new(LADDER);
    request.nonpositive_indices = vec![8];
    request.n_cores = 2;
    let generated = family_candidates(request).unwrap();
    let summary = report(generated.to_toml());
    assert_eq!(summary["solved_sectors"].as_integer(), Some(47));
    assert_eq!(summary["finite_residuals"].as_integer(), Some(10));
}

#[test]
fn reported_sector_failures_require_an_uncheckpointed_run() {
    let mut request = FamilyCandidatesRequest::new(KITE);
    request.sector_failures = SectorFailurePolicy::Report;
    request.checkpoint = Some(rustred_app::CandidateCheckpointOptions::new(
        std::env::temp_dir().join("p_integral_candidates_unused_checkpoint"),
    ));
    let error = family_candidates(request).unwrap_err().to_string();
    assert!(
        error.contains("cannot be combined with checkpoints"),
        "{error}"
    );
}

/// 363 of 366 sectors generate; three fail on exceptional loci with
/// unsupported nonlinear equalities (as in the pint-study). They are
/// reported, not worked around. About 550 s on 8 workers, 7 GB.
#[test]
#[ignore = "slow: four-loop crossed-rung generation (run with --ignored, release)"]
fn crossed_rung_reports_its_three_exceptional_sectors() {
    let mut request = FamilyCandidatesRequest::new(CROSSED);
    request.nonpositive_indices = vec![11, 12, 13];
    request.n_cores = std::thread::available_parallelism().map_or(1, |n| n.get().min(8));
    request.sector_failures = SectorFailurePolicy::Report;
    let generated = family_candidates(request).unwrap();
    let summary = report(generated.to_toml());
    assert_eq!(summary["sector_failures"].as_str(), Some("report"));
    assert_eq!(summary["solved_sectors"].as_integer(), Some(363));
    let failed = summary["failed_sectors"].as_array().unwrap();
    let mut masks: Vec<String> = failed
        .iter()
        .map(|sector| {
            assert_eq!(
                sector["kind"].as_str(),
                Some("unsupported-nonlinear-equalities")
            );
            sector["sector"]
                .as_array()
                .unwrap()
                .iter()
                .map(|active| if active.as_bool().unwrap() { '1' } else { '0' })
                .collect()
        })
        .collect();
    masks.sort();
    assert_eq!(
        masks,
        ["11010101011000", "11011001011000", "11110101000000"]
    );
    // The bundle loads; integrals of the failed sectors stay uncovered.
    let (_, mut reducer) = load_generated_candidate_bundle::<14>(
        generated.bundle(),
        CandidateBundleLimits::default(),
        ReductionLimits::default(),
    )
    .unwrap();
    let mask = failed[0]["sector"].as_array().unwrap();
    let powers: Vec<i64> = mask
        .iter()
        .map(|active| i64::from(active.as_bool().unwrap()))
        .collect();
    assert!(reducer.reduce_unit_mass(&key(&powers)).is_err());
}
