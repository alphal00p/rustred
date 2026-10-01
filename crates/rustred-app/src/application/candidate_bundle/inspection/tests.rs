use super::super::model::{CaseRecord, TermRecord};
use super::*;

#[test]
fn inspection_keeps_fixed_replacements_separate_from_symbolic_shifts() {
    let rule = RuleRecord {
        case: CaseRecord {
            kind: "affine".into(),
            fixed_axes: vec![1],
            fixed_values: vec![2],
            equations: vec![7, 8],
        },
        target: IntegralRecord {
            symbolic: vec![true, false, true],
            values: vec![0, 2, 1],
        },
        rhs: vec![TermRecord {
            integral: IntegralRecord {
                symbolic: vec![true, false, true],
                values: vec![-1, 1, 3],
            },
            coefficient: 9,
        }],
        sources: vec![],
        exclusions: vec![vec![3, 4], vec![5]],
    };
    let view = rule_view(13, &rule);
    assert_eq!(view.ordinal, 13);
    assert_eq!(
        view.case.fixed,
        vec![CandidateFixedAxisInspection { axis: 1, value: 2 }]
    );
    assert_eq!(view.case.affine_zero_equations, vec![7, 8]);
    assert_eq!(
        view.excluded_all_zero_conjunctions,
        vec![vec![3, 4], vec![5]]
    );
    assert_eq!(view.rhs[0].symbolic_shifts, vec![Some(-1), None, Some(2)]);
    assert_eq!(view.rhs[0].fixed_replacements, vec![None, Some(1), None]);
    assert_eq!(view.rhs[0].integral.values, vec![-1, 1, 3]);
    assert!(codec::rules::validate_rule(&rule, 3).is_ok());
    let mut wrong = rule.clone();
    wrong.case.fixed_axes = vec![3];
    assert!(codec::rules::validate_rule(&wrong, 3).is_err());
    let mut wrong = rule;
    wrong.rhs[0].integral.symbolic[1] = true;
    assert!(codec::rules::validate_rule(&wrong, 3).is_err());
}

#[test]
fn inspection_filters_reject_empty_duplicate_and_absent_entries() {
    assert!(selection(Some(&[2, 2]), 0..4).is_err());
    assert!(selection(Some(&[] as &[usize]), 0..4).is_err());
    assert!(selection(Some(&[4]), 0..4).is_err());
    assert_eq!(
        selection(Some(&[2, 0]), 0..4).unwrap().unwrap(),
        BTreeSet::from([0, 2])
    );
}

#[test]
fn inspection_json_writer_enforces_budget_without_truncating_success() {
    let mut bytes = Vec::new();
    let mut writer = LimitedWriter {
        writer: &mut bytes,
        bytes: 0,
        limit: 4,
        exceeded: false,
    };
    writer.write_all(b"1234").unwrap();
    assert!(writer.write_all(b"5").is_err());
    assert!(writer.exceeded);
    assert_eq!(bytes, b"1234");
}
