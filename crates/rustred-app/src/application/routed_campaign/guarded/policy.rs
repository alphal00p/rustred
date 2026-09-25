use crate::AppError;
use rustred::solver::OwnerGuardedLimits;
use serde::{Deserialize, Serialize};

#[derive(Deserialize, Serialize)]
#[serde(default, deny_unknown_fields)]
struct Policy {
    max_predicates: usize,
    max_predicate_terms: usize,
    max_events: usize,
    applied: Applied,
}
#[derive(Deserialize, Serialize)]
#[serde(default, deny_unknown_fields)]
struct Applied {
    cell_refinement_max_cardinality: Option<std::num::NonZeroUsize>,
    max_term_visits: usize,
    max_shift_groups: usize,
    max_boundary_cells: usize,
    max_sign_splits: usize,
    max_native_operations: usize,
    max_events: usize,
    max_scratch_terms: usize,
    max_scratch_boxes: usize,
    max_scratch_coordinate_cells: usize,
    max_guard_univariate_degree: usize,
}
impl Default for Policy {
    fn default() -> Self {
        Self::from_limits(OwnerGuardedLimits::default())
    }
}
impl Default for Applied {
    fn default() -> Self {
        Policy::default().applied
    }
}
impl Policy {
    fn from_limits(l: OwnerGuardedLimits) -> Self {
        let a = l.applied;
        Self {
            max_predicates: l.max_predicates,
            max_predicate_terms: l.max_predicate_terms,
            max_events: l.max_events,
            applied: Applied {
                cell_refinement_max_cardinality: match a.cell_refinement {
                    rustred::solver::OwnerAppliedCellRefinement::Off => None,
                    rustred::solver::OwnerAppliedCellRefinement::SingleFiniteAxis {
                        max_cardinality,
                    } => Some(max_cardinality),
                },
                max_term_visits: a.max_term_visits,
                max_shift_groups: a.max_shift_groups,
                max_boundary_cells: a.max_boundary_cells,
                max_sign_splits: a.max_sign_splits,
                max_native_operations: a.max_native_operations,
                max_events: a.max_events,
                max_scratch_terms: a.max_scratch_terms,
                max_scratch_boxes: a.max_scratch_boxes,
                max_scratch_coordinate_cells: a.max_scratch_coordinate_cells,
                max_guard_univariate_degree: a.matching.guard_algebra.max_univariate_degree,
            },
        }
    }
    fn limits(self) -> Result<OwnerGuardedLimits, AppError> {
        let a = self.applied;
        if [
            self.max_predicates,
            self.max_predicate_terms,
            self.max_events,
            a.max_term_visits,
            a.max_shift_groups,
            a.max_boundary_cells,
            a.max_sign_splits,
            a.max_native_operations,
            a.max_events,
            a.max_scratch_terms,
            a.max_scratch_boxes,
            a.max_scratch_coordinate_cells,
            a.max_guard_univariate_degree,
        ]
        .contains(&0)
        {
            return Err(AppError::input(
                "guarded work limits must be positive integers",
            ));
        }
        let mut l = OwnerGuardedLimits::default();
        l.max_predicates = self.max_predicates;
        l.max_predicate_terms = self.max_predicate_terms;
        l.max_events = self.max_events;
        l.applied.max_term_visits = a.max_term_visits;
        l.applied.cell_refinement = a.cell_refinement_max_cardinality.map_or(
            rustred::solver::OwnerAppliedCellRefinement::Off,
            |max_cardinality| rustred::solver::OwnerAppliedCellRefinement::SingleFiniteAxis {
                max_cardinality,
            },
        );
        l.applied.max_shift_groups = a.max_shift_groups;
        l.applied.max_boundary_cells = a.max_boundary_cells;
        l.applied.max_sign_splits = a.max_sign_splits;
        l.applied.max_native_operations = a.max_native_operations;
        l.applied.max_events = a.max_events;
        l.applied.max_scratch_terms = a.max_scratch_terms;
        l.applied.max_scratch_boxes = a.max_scratch_boxes;
        l.applied.max_scratch_coordinate_cells = a.max_scratch_coordinate_cells;
        l.applied.matching.guard_algebra.max_univariate_degree = a.max_guard_univariate_degree;
        Ok(l)
    }
}
pub(crate) fn parse(text: &str) -> Result<OwnerGuardedLimits, AppError> {
    if text.len() > 16 * 1024 {
        return Err(AppError::input("guarded work limits exceed 16 KiB"));
    }
    let shape: serde_json::Value = serde_json::from_str(text)
        .map_err(|e| AppError::input(format!("guarded work limits: {e}")))?;
    if !shape.is_object() || shape.get("applied").is_some_and(|v| !v.is_object()) {
        return Err(AppError::input(
            "guarded work limits and applied limits must be JSON objects",
        ));
    }
    // Deserialize original text, not the Value: duplicate fields remain errors.
    serde_json::from_str::<Policy>(text)
        .map_err(|e| AppError::input(format!("guarded work limits: {e}")))?
        .limits()
}
pub(super) fn json(l: OwnerGuardedLimits) -> serde_json::Value {
    let mut v = serde_json::to_value(Policy::from_limits(l)).expect("integer limits");
    v["effective_matching_limits"] =
        serde_json::json!(super::render::bounded_debug(&l.applied.matching));
    v
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn optional_application_refinement_policy_round_trips_and_rejects_invalid_values() {
        use rustred::solver::OwnerAppliedCellRefinement;
        assert_eq!(
            parse("{}").unwrap().applied.cell_refinement,
            OwnerAppliedCellRefinement::Off
        );
        let limits = parse(r#"{"applied":{"cell_refinement_max_cardinality":2}}"#).unwrap();
        assert_eq!(
            limits.applied.cell_refinement,
            OwnerAppliedCellRefinement::SingleFiniteAxis {
                max_cardinality: std::num::NonZeroUsize::new(2).unwrap(),
            }
        );
        let policy = serde_json::to_string(&Policy::from_limits(limits)).unwrap();
        assert_eq!(
            parse(&policy).unwrap().applied.cell_refinement,
            limits.applied.cell_refinement
        );
        assert_eq!(
            json(limits)["applied"]["cell_refinement_max_cardinality"],
            2
        );
        for value in ["0", "-1", "true", "2.5", "\"2\"", "18446744073709551616"] {
            assert!(
                parse(&format!(
                    "{{\"applied\":{{\"cell_refinement_max_cardinality\":{value}}}}}"
                ))
                .is_err()
            );
        }
        assert!(parse(r#"{"applied":{"cell_refinement_max_cardinality":2,"cell_refinement_max_cardinality":3}}"#).is_err());
    }
    #[test]
    fn guarded_policy_is_strict_and_preserves_native_defaults() {
        assert_eq!(
            json(parse("{}").unwrap()),
            json(OwnerGuardedLimits::default())
        );
        let l = parse(
            r#"{"max_events":123,"applied":{"max_events":456,"max_guard_univariate_degree":64}}"#,
        )
        .unwrap();
        assert_eq!(
            (
                l.max_events,
                l.applied.max_events,
                l.applied.matching.guard_algebra.max_univariate_degree
            ),
            (123, 456, 64)
        );
        for s in [
            "[]",
            "null",
            r#"{"applied":[]}"#,
            r#"{"max_events":null}"#,
            r#"{"max_events":0}"#,
            r#"{"max_events":1,"max_events":2}"#,
            r#"{"applied":{"max_events":1,"max_events":2}}"#,
            r#"{"unknown":1}"#,
        ] {
            assert!(parse(s).is_err(), "{s}");
        }
    }
}
