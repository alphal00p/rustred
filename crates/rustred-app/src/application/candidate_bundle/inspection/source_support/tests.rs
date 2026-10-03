use super::super::super::model::{CaseRecord, IntegralRecord, SeedRecord};
use super::*;

fn options() -> CandidateSourceSupportOptions {
    CandidateSourceSupportOptions {
        sector: vec![true],
        rule_ordinal: 0,
        source_recenter_nomination: None,
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
            values: vec![0, 2],
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
fn unknown_recenter_preserves_raw_seeds_and_explicit_nominee_preserves_fixed_displacements() {
    let rule = shifted_fixed_rule();
    let raw = offsets(&rule, 2, &options()).unwrap();
    assert_eq!(raw, vec![vec![2, -1], vec![5, 5]]);
    let mut nominated = options();
    nominated.source_recenter_nomination = Some(vec![-3, 0]);
    let support = offsets(&rule, 2, &nominated).unwrap();
    assert_eq!(support, vec![vec![-1, -1], vec![2, 5]]);
    // Different preconditioned basis ordinals at one physical seed add no new
    // offset. The inspector neither interprets nor claims to validate them.
    let mut reordered = rule;
    reordered.sources.reverse();
    assert_eq!(offsets(&reordered, 2, &nominated).unwrap(), support);
}

#[test]
fn recenter_nomination_refuses_wrong_arity_fixed_translation_and_overflow() {
    let rule = shifted_fixed_rule();
    for nominee in [vec![-1], vec![-1, 1], vec![i64::MAX, 0]] {
        let mut opts = options();
        opts.source_recenter_nomination = Some(nominee);
        assert!(offsets(&rule, 2, &opts).is_err());
    }
    let mut noncanonical = rule;
    noncanonical.target.values[0] = 3;
    assert!(offsets(&noncanonical, 2, &options()).is_err());
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
    assert_eq!(report.source_recenter_nomination, None);
    assert_eq!(report.source_recenter_status, "unknown");
    assert_eq!(report.source_offset_frame, "unrecentered_stored_seeds");
    assert!(report.original_inventory_completed);
    assert!(!report.saved_basis_ordinals_validated);
    assert!(!report.source_replay_claim && !report.dispatch_claim && !report.closure_claim);
    let json: serde_json::Value = serde_json::from_str(&report.to_json().unwrap()).unwrap();
    assert_eq!(json["case"]["kind"], "coordinate");
    assert!(json.get("weights").is_none());
    assert!(json.get("basis_row").is_none());
    assert!(json.get("canonical_translation").is_none());

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

#[test]
fn generated_canonical_tadpole_requires_unknown_raw_pivot_recenter() {
    use rustred::identity::{IntegralShift, TranslatedSourceRequest};
    use rustred::solver::SourceSystem;

    let generated = crate::family_candidates(crate::FamilyCandidatesRequest::new(TADPOLE)).unwrap();
    let limits = CandidateBundleLimits::default();
    let bundle = codec::read(generated.bundle(), limits).unwrap();
    let record = &bundle
        .sectors
        .iter()
        .find(|s| s.sector == [true])
        .unwrap()
        .rules[0];
    assert_eq!(record.target.symbolic, [true]);
    assert_eq!(record.target.values, [0]);
    assert_eq!(record.sources.len(), 1);
    assert_eq!(record.sources[0].integral.values, [0]);
    assert_eq!(record.sources[0].shifts, [0]);
    assert_eq!(offsets(record, 1, &options()).unwrap(), [vec![0]]);

    let family = bundle
        .family
        .to_family(
            &bundle.coefficients,
            limits.family_limits(),
            limits.binary_limits(),
        )
        .unwrap();
    let generator = ParametricIbpGenerator::try_new(&family).unwrap();
    let c = generator.context();
    let system = SourceSystem::<1>::from_family(&family).unwrap();
    let solutions = codec::solutions::<1>(&bundle, c, system.index_variables(), limits).unwrap();
    let saved = &solutions
        .iter()
        .find(|(s, _)| s == &[true])
        .unwrap()
        .1
        .rules[0]
        .candidate;
    let expected = saved
        .rhs
        .iter()
        .map(|term| {
            (
                vec![i64::from(term.integral[0].value())],
                term.coefficient.clone(),
            )
        })
        .collect::<std::collections::BTreeMap<_, _>>();
    let prepared = generator.prepare_ordinary_ibp().unwrap();
    let rows = (0..prepared.len()).map(|i| prepared.generate(i)).collect();
    let complete = prepared.complete(rows).unwrap();
    let normalized_rhs = |offset: i64| {
        let translated = generator
            .translate_selected_completed_source_rows(
                &complete,
                [TranslatedSourceRequest::new(
                    0,
                    IntegralShift::try_new([offset]).unwrap(),
                )],
                Default::default(),
            )
            .unwrap();
        let terms = translated.sources()[0].terms();
        let pivot = terms
            .iter()
            .find(|(shift, _)| shift.values() == [0])
            .unwrap()
            .1;
        terms
            .iter()
            .filter(|(shift, _)| shift.values() != [0])
            .map(|(shift, coefficient)| {
                let rhs = c
                    .div(
                        &c.neg_with_limits(coefficient, Default::default()).unwrap(),
                        pivot,
                    )
                    .unwrap();
                (shift.values().to_vec(), rhs.raw().clone())
            })
            .collect::<std::collections::BTreeMap<_, _>>()
    };
    assert_ne!(normalized_rhs(0), expected);
    assert_eq!(normalized_rhs(-1), expected);
    let mut nominated = options();
    nominated.source_recenter_nomination = Some(vec![-1]);
    let report =
        inspect_generated_candidate_source_support(generated.bundle(), limits, nominated).unwrap();
    assert_eq!(report.source_offsets, [vec![-1]]);
    assert_eq!(report.source_recenter_nomination, Some(vec![-1]));
    assert_eq!(
        report.source_recenter_status,
        "caller_nomination_unverified"
    );
    assert_eq!(report.source_offset_frame, "caller_recentered_stored_seeds");
    assert!(!report.source_replay_claim);
}
