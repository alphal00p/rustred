use crate::algebra::CoefficientContext;
use crate::solver::{Integral, Term};

use super::*;

fn row(context: &CoefficientContext, inputs: &[&str]) -> ExactRow<1> {
    inputs
        .iter()
        .enumerate()
        .map(|(ordinal, input)| Term {
            integral: Integral::symbolic([ordinal as i16]).unwrap(),
            coefficient: context.coefficient_fixture(input),
        })
        .collect()
}

#[test]
fn denominator_only_variables_survive_and_original_map_is_restored() {
    let context = CoefficientContext::new(["unused0", "a", "unused1", "b", "unused2"]);
    let rows = vec![row(&context, &["(a+1)/(b-2)", "3/(b-2)", "0"])];
    let variables = FrameVariables::try_new(&rows).unwrap();
    assert_eq!(variables.original_len(), 5);
    assert_eq!(variables.active_len(), 2);
    assert_eq!(variables.active[0], variables.original[1]);
    assert_eq!(variables.active[1], variables.original[3]);
    for term in rows.iter().flatten() {
        let compact = variables.map_coefficient(&term.coefficient).unwrap();
        assert_eq!(compact.get_variables().len(), 2);
        assert_eq!(
            variables.restore_coefficient(&compact).unwrap(),
            term.coefficient
        );
    }
    let left = variables.map_coefficient(&rows[0][0].coefficient).unwrap();
    let right = variables.map_coefficient(&rows[0][1].coefficient).unwrap();
    assert_eq!(
        variables.restore_coefficient(&(&left + &right)).unwrap(),
        context.coefficient_fixture("(a+4)/(b-2)")
    );
}

#[test]
fn constant_and_empty_frames_use_valid_zero_variable_contexts() {
    let context = CoefficientContext::new(["a", "b"]);
    let rows = vec![row(&context, &["0", "1", "-3/7"])];
    let variables = FrameVariables::try_new(&rows).unwrap();
    assert_eq!(variables.active_len(), 0);
    for term in rows.iter().flatten() {
        let compact = variables.map_coefficient(&term.coefficient).unwrap();
        assert!(compact.get_variables().is_empty());
        assert_eq!(
            variables.restore_coefficient(&compact).unwrap(),
            term.coefficient
        );
    }
    let empty = FrameVariables::try_new::<1>(&[Vec::new()]).unwrap();
    assert_eq!((empty.original_len(), empty.active_len()), (0, 0));
}

#[test]
fn fully_active_contexts_share_the_map_and_preserve_values() {
    let context = CoefficientContext::new(["a", "b"]);
    let rows = vec![row(&context, &["a", "1/b"])];
    let variables = FrameVariables::try_new(&rows).unwrap();
    assert!(Arc::ptr_eq(&variables.active, &variables.original));
    for term in rows.iter().flatten() {
        let mapped = variables.map_coefficient(&term.coefficient).unwrap();
        assert_eq!(mapped, term.coefficient);
        assert!(Arc::ptr_eq(
            mapped.get_variables(),
            term.coefficient.get_variables()
        ));
    }
}

#[test]
fn mixed_coefficient_or_numerator_denominator_maps_are_rejected() {
    let context = CoefficientContext::new(["a", "b"]);
    let other = CoefficientContext::new(["b", "a"]);
    let mut rows = vec![row(&context, &["a"]), row(&other, &["b"])];
    assert!(matches!(
        FrameVariables::try_new(&rows),
        Err(MaterializationError::CoefficientVariableMapMismatch)
    ));
    rows.pop();
    rows[0][0].coefficient.denominator = other.coefficient_fixture("1").denominator;
    assert!(matches!(
        FrameVariables::try_new(&rows),
        Err(MaterializationError::CoefficientVariableMapMismatch)
    ));
}

#[test]
fn mapping_rejects_new_active_variables_and_wrong_restore_context() {
    let context = CoefficientContext::new(["unused", "a", "b"]);
    let rows = vec![row(&context, &["a/b"])];
    let variables = FrameVariables::try_new(&rows).unwrap();
    assert!(matches!(
        variables.map_coefficient(&context.coefficient_fixture("unused+a/b")),
        Err(MaterializationError::CoefficientVariableRemap(_))
    ));
    assert!(matches!(
        variables.restore_coefficient(&rows[0][0].coefficient),
        Err(MaterializationError::CoefficientVariableMapMismatch)
    ));
}
