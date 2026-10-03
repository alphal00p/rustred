use super::super::super::model::{CaseRecord, IntegralRecord, SeedRecord};
use super::*;

fn options() -> CandidateSourceSupportOptions {
    CandidateSourceSupportOptions {
        sector: vec![true],
        rule_ordinal: 0,
        max_retained_seeds: 10000,
        max_unique_offsets: 1000,
        max_original_rows: 100,
        max_nominated_pairs: 10000,
        max_generated_terms: 10000,
        max_generated_conditions: 10000,
        max_output_bytes: 1 << 20,
        source_generation: Default::default(),
    }
}

fn shifted_fixed_rule() -> RuleRecord {
    RuleRecord {
        case: CaseRecord {
            kind: "coordinate".into(),
            fixed_axes: vec![1],
            fixed_values: vec![2],
            equations: vec![],
        },
        target: IntegralRecord {
            symbolic: vec![true, false],
            values: vec![3, 2],
        },
        rhs: vec![],
        exclusions: vec![vec![4, 5]],
        sources: vec![
            SeedRecord {
                basis_row: usize::MAX,
                integral: IntegralRecord {
                    symbolic: vec![true, false],
                    values: vec![5, 7],
                },
                shifts: vec![5, 0],
            },
            SeedRecord {
                basis_row: 0,
                integral: IntegralRecord {
                    symbolic: vec![true, false],
                    values: vec![5, 7],
                },
                shifts: vec![5, 0],
            },
            SeedRecord {
                basis_row: 3,
                integral: IntegralRecord {
                    symbolic: vec![true, false],
                    values: vec![2, 1],
                },
                shifts: vec![2, 0],
            },
        ],
    }
}

#[test]
fn physical_offsets_recenter_symbolic_targets_and_preserve_fixed_seed_displacements() {
    let rule = shifted_fixed_rule();
    let (canonical, support) = offsets(&rule, 2, &options()).unwrap();
    assert_eq!(canonical, vec![-3, 0]);
    assert_eq!(support, vec![vec![-1, -1], vec![2, 5]]);
    // Different preconditioned basis ordinals at one physical seed add no new
    // offset. The inspector neither interprets nor claims to validate them.
    let mut reordered = rule;
    reordered.sources.reverse();
    assert_eq!(
        offsets(&reordered, 2, &options()).unwrap(),
        (canonical, support)
    );
}

#[test]
fn inconsistent_seed_transport_and_affine_cases_fail_closed() {
    let rule = shifted_fixed_rule();
    let mut wrong = rule.clone();
    wrong.sources[0].integral.symbolic[0] = false;
    assert!(offsets(&wrong, 2, &options()).is_err());
    let mut wrong = rule.clone();
    wrong.sources[0].shifts[0] += 1;
    assert!(offsets(&wrong, 2, &options()).is_err());
    let mut wrong = rule.clone();
    wrong.sources[0].shifts[1] = 1;
    assert!(offsets(&wrong, 2, &options()).is_err());
    let mut wrong = rule.clone();
    wrong.target.values[1] += 1;
    assert!(offsets(&wrong, 2, &options()).is_err());
    let mut wrong = rule;
    wrong.case.kind = "affine".into();
    wrong.case.equations = vec![7];
    assert!(
        offsets(&wrong, 2, &options())
            .unwrap_err()
            .to_string()
            .contains("affine")
    );
}

#[test]
fn seed_scan_unique_offset_and_empty_support_caps_are_explicit() {
    let mut rule = shifted_fixed_rule();
    let mut limits = options();
    limits.max_retained_seeds = 2;
    assert!(offsets(&rule, 2, &limits).is_err());
    limits = options();
    limits.max_unique_offsets = 1;
    assert!(offsets(&rule, 2, &limits).is_err());
    limits.max_unique_offsets = 0;
    assert!(offsets(&rule, 2, &limits).is_err());
    rule.sources.clear();
    assert!(offsets(&rule, 2, &options()).is_err());
}

const TADPOLE: &str = r#"
schema="rustred.project.toml.v1"
[family]
name="saved_source_support_tadpole"
loop_momenta=["q"]
external_momenta=[]
dimension="d"
[[family.denominators]]
id="P"
expression="q^2-1"
[target]
powers=[1]
"#;

#[test]
fn completed_tadpole_inventory_nominates_without_source_or_dispatch_authority() {
    let generated = crate::family_candidates(crate::FamilyCandidatesRequest::new(TADPOLE)).unwrap();
    let report = inspect_generated_candidate_source_support(
        generated.bundle(),
        Default::default(),
        options(),
    )
    .unwrap();
    assert_eq!(report.ordinary_source_rows, vec!["ordinary-ibp:0:0"]);
    assert!(!report.source_offsets.is_empty());
    assert_eq!(report.nominated_pair_count, report.source_offsets.len());
    assert_eq!(report.canonical_translation, vec![0]);
    assert!(report.original_inventory_completed);
    assert!(!report.saved_basis_ordinals_validated);
    assert!(!report.source_replay_claim && !report.dispatch_claim && !report.closure_claim);
    let json: serde_json::Value = serde_json::from_str(&report.to_json().unwrap()).unwrap();
    assert_eq!(json["case"]["kind"], "coordinate");
    assert!(json.get("weights").is_none());
    assert!(json.get("basis_row").is_none());

    let mut tiny = options();
    tiny.max_output_bytes = 1;
    assert!(
        inspect_generated_candidate_source_support(generated.bundle(), Default::default(), tiny)
            .is_err()
    );
    let mut tiny = options();
    tiny.max_original_rows = 0;
    assert!(
        inspect_generated_candidate_source_support(generated.bundle(), Default::default(), tiny)
            .is_err()
    );
    assert!(report.generated_term_count > 1);
    let mut tiny = options();
    tiny.max_generated_terms = 1;
    assert!(
        inspect_generated_candidate_source_support(generated.bundle(), Default::default(), tiny)
            .is_err()
    );
    let mut absent = options();
    absent.rule_ordinal = usize::MAX;
    assert!(
        inspect_generated_candidate_source_support(generated.bundle(), Default::default(), absent)
            .is_err()
    );

    let mut bundle = codec::read(generated.bundle(), Default::default()).unwrap();
    let rule = &mut bundle
        .sectors
        .iter_mut()
        .find(|s| s.sector == [true])
        .unwrap()
        .rules[0];
    let mut extra = rule.sources[0].clone();
    extra.integral.values[0] -= 1;
    extra.shifts[0] -= 1;
    rule.sources.push(extra);
    let bytes = codec::write(&bundle, Default::default()).unwrap();
    let mut tiny = options();
    tiny.max_nominated_pairs = 1;
    assert!(inspect_generated_candidate_source_support(&bytes, Default::default(), tiny).is_err());
}
