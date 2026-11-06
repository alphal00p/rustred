//! Two-sided eps axes through the application path (owner: M2-T7b rr-twosided).
//!
//! `FamilyCandidatesRequest::two_sided_eps_axes` is the opt-in of M2-T7c's
//! `p-generate`. What the generated rules mean is tested in rustred-core's
//! `tests/p_library/twosided.rs`; this module pins what only the application
//! plumbing decides (`candidate_bundle/generate.rs`): which order program is
//! installed, persisted and checkpointed, which requests are refused, and
//! that the option changes nothing while it is off or while the family has no
//! eps line. A two-sided bundle does not load through `CandidateReducer`: that
//! needs M2-T6's p-only constructor (`eps.rs` pins the refusals in place).
use rustred::order::{CompiledOrder, Limits, OrderDescriptor};
use rustred::sector::OrderingPolicy;
use rustred_app::{
    CandidateCheckpointOptions, FamilyCandidatesRequest, equivalent_generated_programs,
    family_candidates, inspect_generated_candidate_bundle,
};

/// The 2L kite with eps on D1 and D5, LP 1 (`rules_study/targets.tsv` T001).
const T001C: &str = include_str!("../../../rustred-core/tests/support/eps_families/T001c.sym");

/// The eps axes of T001c.
const AXES: [bool; 5] = [true, false, false, false, true];

/// The four raw charts of the top support: each eps axis on both sides of zero.
fn charts() -> Vec<Vec<bool>> {
    [[true, true], [false, true], [true, false], [false, false]]
        .iter()
        .map(|&[d1, d5]| vec![d1, true, true, true, d5])
        .collect()
}

fn two_sided_request() -> FamilyCandidatesRequest {
    let mut request = FamilyCandidatesRequest::new(T001C);
    request.two_sided_eps_axes = true;
    request.selected_sectors = Some(charts());
    request
}

fn compile(descriptor: OrderDescriptor) -> CompiledOrder {
    CompiledOrder::compile(descriptor, Limits::default()).unwrap()
}

/// What a bundle or checkpoint must name as its order: the family's default
/// two-sided program, not the legacy comparator.
fn two_sided_order_id() -> String {
    OrderingPolicy::try_programmed(compile(OrderDescriptor::spired_uncut_two_sided(
        AXES.to_vec(),
    )))
    .unwrap()
    .stable_id()
    .to_string()
}

fn refusal(request: FamilyCandidatesRequest) -> String {
    match family_candidates(request) {
        Ok(_) => panic!("the request was not refused"),
        Err(error) => error.to_string(),
    }
}

/// `(reused, newly solved)` sectors of a checkpointed generation's report.
fn checkpoint_sectors(report: &str) -> (i64, i64) {
    let report: toml::Value = toml::from_str(report).unwrap();
    let count = |field: &str| report["checkpoint"][field].as_integer().unwrap();
    (count("reused_sectors"), count("newly_solved_sectors"))
}

#[test]
fn a_two_sided_generation_closes_every_chart_and_persists_its_order() {
    for root_zero_census in [false, true] {
        let mut request = two_sided_request();
        request.root_zero_census = root_zero_census;
        let generated = family_candidates(request).unwrap();
        let inspection =
            inspect_generated_candidate_bundle(generated.bundle(), Default::default()).unwrap();
        assert_eq!(inspection.solved_sectors, 4, "census {root_zero_census}");
        assert_eq!(inspection.finite_residuals, 1, "census {root_zero_census}");
        assert_eq!(inspection.integral_order, two_sided_order_id());
        generated.artifact().unwrap();
    }
}

#[test]
fn the_order_of_a_two_sided_generation_must_carry_the_family_axes() {
    let mut supplied = two_sided_request();
    supplied.integral_order = Some(compile(OrderDescriptor::spired_uncut_two_sided(
        AXES.to_vec(),
    )));
    let generated = family_candidates(supplied.clone()).unwrap();
    let inspection =
        inspect_generated_candidate_bundle(generated.bundle(), Default::default()).unwrap();
    assert_eq!(inspection.integral_order, two_sided_order_id());

    let other_axes = OrderDescriptor::spired_uncut_two_sided(vec![true, true, false, false, true]);
    for program in [OrderDescriptor::spired_uncut(5), other_axes] {
        supplied.integral_order = Some(compile(program));
        let error = refusal(supplied.clone());
        assert!(
            error.contains("does not carry the family's two-sided eps axes"),
            "{error}"
        );
    }

    let mut legacy = two_sided_request();
    legacy.permutation = Some(vec![0, 1, 2, 3, 4]);
    let error = refusal(legacy);
    assert!(error.contains("legacy --permutation"), "{error}");
}

#[test]
fn the_option_is_off_by_default_and_the_lower_chart_then_fails_closed() {
    let mut request = FamilyCandidatesRequest::new(T001C);
    assert!(!request.two_sided_eps_axes);
    request.selected_sectors = Some(vec![vec![false, true, true, true, false]]);
    let error = refusal(request);
    assert!(
        error.contains("numeric RHS index activates an inactive sector coordinate"),
        "{error}"
    );
}

#[test]
fn a_family_without_eps_lines_ignores_the_option() {
    let kite = T001C.replace("  ,power_shift(D1,2-d/2)\n  ,power_shift(D5,2-d/2)\n", "");
    assert!(!kite.contains("power_shift"));
    let plain = FamilyCandidatesRequest::new(kite);
    let mut opted_in = plain.clone();
    opted_in.two_sided_eps_axes = true;
    assert_eq!(
        family_candidates(plain).unwrap().bundle(),
        family_candidates(opted_in).unwrap().bundle()
    );
}

#[test]
fn a_two_sided_checkpoint_resumes_under_its_own_order_only() {
    let directory =
        std::env::temp_dir().join(format!("rustred-p-library-twosided-{}", std::process::id()));
    std::fs::create_dir_all(&directory).unwrap();
    let mut request = two_sided_request();
    request.checkpoint = Some(CandidateCheckpointOptions::new(directory.join("ckpt")));
    let generated = family_candidates(request.clone()).unwrap();
    assert_eq!(checkpoint_sectors(generated.to_toml()), (0, 4));

    // A resume reuses all four charts and solves nothing.
    request.checkpoint.as_mut().unwrap().resume = true;
    let resumed = family_candidates(request.clone()).unwrap();
    assert_eq!(checkpoint_sectors(resumed.to_toml()), (4, 0));
    assert!(
        equivalent_generated_programs(generated.bundle(), resumed.bundle(), Default::default())
            .unwrap()
    );

    // Off, the request names another order: the manifest no longer matches.
    request.two_sided_eps_axes = false;
    let error = refusal(request);
    assert!(error.contains("checkpoint manifest differs"), "{error}");
    std::fs::remove_dir_all(&directory).unwrap();
}
