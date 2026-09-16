use super::super::{PredicateCoverError, PredicateCoverLimits, TraversalWork, check_valuations};
use super::*;
use crate::algebra::CoefficientContext;

#[test]
fn first_failure_preserves_exact_box_sector_maps_and_partial_truth_assignment() {
    let context = CoefficientContext::new(["d", "a", "b", "c"]);
    let equations = ["a-b", "b-c", "a-c"].map(|e| context.coefficient_fixture(e).numerator);
    let atoms = equations
        .iter()
        .map(|equation| Atom {
            indices: &[1, 2, 3],
            equation,
        })
        .collect::<Vec<_>>();
    let mut assignments = [Some(true), Some(false), None];
    let before = assignments;
    let mut work = TraversalWork::new(&[false, true, false], &atoms);
    let error = check_valuations(
        &[false, true, false],
        &atoms,
        &[],
        &[],
        &mut assignments,
        &mut work,
        PredicateCoverLimits::default(),
    )
    .unwrap_err();
    let PredicateCoverError::Uncovered {
        boxes,
        unbounded_boxes,
        witness,
    } = &error
    else {
        panic!("expected missing cover");
    };
    assert_eq!((*boxes, *unbounded_boxes), (1, 1));
    assert_eq!(assignments, before);
    assert_eq!(work.nodes, 1);
    assert_eq!(&*witness.sector, &[false, true, false]);
    assert_eq!(&*witness.lower, &[0; 3]);
    assert_eq!(&*witness.upper, &[None; 3]);
    for ((actual, equation), assignment) in witness.atoms.iter().zip(&equations).zip(before) {
        assert_eq!(actual.assignment, assignment);
        assert_eq!(&*actual.indices, &[1, 2, 3]);
        assert_eq!(&actual.equation, equation);
    }
    let text = error.to_string();
    assert!(text.contains("not an exhaustive complement census"));
    assert!(text.contains("not a concrete uncovered integral"));
    assert!(text.contains("sector=[false, true, false]"));
    assert!(text.contains("upper=[None, None, None]"));
    assert!(text.contains("x=n-1 if active, x=-n otherwise"));
    assert!(text.contains("atom 0: equal_zero; index_variables=[1, 2, 3]"));
    assert!(text.contains("atom 1: not_equal_zero"));
    assert!(text.contains("atom 2: unassigned"));
    assert!(text.contains(&equations[2].to_string()));
}

#[test]
fn exact_finite_bounds_and_native_equations_survive_display_truncation() {
    let context = CoefficientContext::new(["a", "b"]);
    let equation = context.coefficient_fixture("3*a-2*b-1").numerator;
    let first = LatticeBox::try_new([u64::MAX, 0], [Some(u64::MAX), Some(2)]).unwrap();
    let atoms = (0..4096)
        .map(|_| Atom {
            indices: &[0, 1],
            equation: &equation,
        })
        .collect::<Vec<_>>();
    let witness =
        PredicateCoverWitness::capture(&[true, false], &first, &atoms, &vec![None; atoms.len()]);
    let before = witness.clone();
    let text = witness.to_string();
    assert!(text.contains("18446744073709551615"));
    assert!(text.contains("predicate-cover diagnostic truncated at 16384 bytes"));
    assert!(text.len() <= MAX_DISPLAY_BYTES + 128);
    assert_eq!(witness, before);
    assert_eq!(witness.atoms.len(), 4096);
    assert_eq!(witness.atoms.last().unwrap().equation, equation);
    assert_eq!(format!("{witness:?}"), text);
}

#[test]
fn bounded_native_output_stops_without_splitting_utf8() {
    let mut output = BoundedText::new(3);
    assert!(output.write_str("éé").is_err());
    assert_eq!(output.text, "é");
    assert!(output.truncated);
    assert!(output.write_str("more").is_err());
    assert_eq!(output.text, "é");
}

#[test]
fn audit_report_preserves_failure_context_for_stored_and_checked_covers() {
    let (audit, mut solution) = crate::foundry::artifact::source_port::tests::solved_tadpole();
    let complete = audit.audit_sector([true], None, &solution).unwrap();
    assert!(complete.issues.is_empty());
    let terminals = std::mem::take(&mut solution.finite_residuals);
    let finite = audit.audit_sector([true], None, &solution).unwrap();
    assert_eq!(finite.checked_rule_uncovered_boxes, 1);
    assert_eq!(finite.checked_rule_unbounded_boxes, 0);
    for label in [
        "stored guard predicate coverage:",
        "checked rule predicate coverage:",
    ] {
        let issue = finite
            .issues
            .iter()
            .find(|issue| issue.starts_with(label))
            .unwrap();
        assert!(issue.contains("first failed abstract Boolean branch"));
        assert!(issue.contains("lower=[0], upper=[Some(0)]"));
        assert!(issue.contains("sector=[true]"));
    }
    solution.finite_residuals = terminals;
    let restored = audit.audit_sector([true], None, &solution).unwrap();
    assert_eq!(restored.exact_replayed_rules, complete.exact_replayed_rules);
    assert_eq!(
        restored.uniformly_descending_rules,
        complete.uniformly_descending_rules
    );
    assert_eq!(restored.checked_rule_uncovered_boxes, 0);
    assert!(restored.issues.is_empty());
}
