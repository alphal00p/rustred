//! Direct exceptional-geometry diagnostics from the thirteen captured failures.
//!
//! These inputs contain the complete original fixed face and any incoming
//! affine parent. A successful case union is not a reduction artifact. Explicit
//! unsupported/budget outcomes remain diagnostic failures to admit that case,
//! not empty domains or successful family closure.

use symbolica::prelude::Integer;

use crate::algebra::{CoefficientContext, CoefficientPolynomial};

use super::super::{Case, CoordinateCase};
use super::{CaseIntersectionFailure, CaseIntersectionLimits};

#[path = "captured_inputs.rs"]
mod inputs;
use inputs::CAPTURED;

fn equations(context: &CoefficientContext, values: &[&str]) -> Vec<CoefficientPolynomial> {
    values
        .iter()
        .map(|value| context.coefficient_fixture(value).numerator)
        .collect()
}

fn restrict(
    case: &Case<15>,
    equation: &CoefficientPolynomial,
    indices: &[usize; 15],
) -> CoefficientPolynomial {
    if let Some(affine) = case.affine() {
        affine.restrict_equation(equation).unwrap()
    } else {
        let mut restricted = equation.clone();
        for (axis, fixed) in case.fixed().iter().enumerate() {
            if let Some(fixed) = fixed {
                restricted = restricted.replace(indices[axis], &Integer::from(*fixed));
            }
        }
        restricted
    }
}

#[test]
fn captured_thirteen_nonlinear_conjunctions_report_exact_admission_outcomes() {
    assert_eq!(CAPTURED.len(), 13);
    let names: Vec<_> = ["d".to_owned(), "x".to_owned()]
        .into_iter()
        .chain((0..15).map(|axis| format!("n{axis}")))
        .collect();
    let context = CoefficientContext::new(names.iter().map(String::as_str));
    let indices = std::array::from_fn(|axis| axis + 2);
    let mut admitted = 0;
    let mut empty = 0;
    let mut unresolved = 0;
    let mut unexpected = Vec::new();
    for fixture in CAPTURED {
        assert_eq!(fixture.sector.len(), 15);
        let sector = std::array::from_fn(|axis| fixture.sector.as_bytes()[axis] == b'1');
        let coordinate: Case<15> = CoordinateCase::new(fixture.fixed).unwrap().into();
        let parent = coordinate
            .intersect(
                &equations(&context, fixture.parent_equalities),
                &indices,
                &sector,
            )
            .unwrap()
            .expect("captured incoming parent was nonempty at admission");
        assert_eq!(parent.fixed(), &fixture.fixed);
        assert_eq!(
            parent.affine().is_some(),
            !fixture.parent_equalities.is_empty()
        );
        let conjunction = equations(&context, fixture.equations);
        assert!(
            conjunction
                .iter()
                .all(|polynomial| polynomial.nvars() == 17)
        );
        let result = parent.intersect_many(
            &conjunction,
            &indices,
            &sector,
            CaseIntersectionLimits::default(),
        );
        match result {
            Ok(result) => {
                admitted += 1;
                empty += usize::from(result.cases.is_empty());
                println!(
                    "CAPTURED_GEOMETRY sector={} status=admitted branches={} stats={:?}",
                    fixture.sector,
                    result.cases.len(),
                    result.stats
                );
                // An exact necessary soundness check, not a sampled coverage
                // test: every emitted affine domain implies the full original
                // conjunction and retains the original integer parent.
                for (ordinal, case) in result.cases.iter().enumerate() {
                    assert!(parent.contains(case).unwrap());
                    assert!(case.is_in_sector(&sector));
                    for equation in &conjunction {
                        assert!(restrict(case, equation, &indices).is_zero());
                    }
                    let coupled = case.affine().map_or_else(Vec::new, |affine| {
                        affine.equations().iter().map(ToString::to_string).collect()
                    });
                    println!(
                        "CAPTURED_BRANCH sector={} ordinal={} fixed={:?} coupled={:?}",
                        fixture.sector,
                        ordinal,
                        case.fixed(),
                        coupled
                    );
                }
            }
            Err(error) => {
                unresolved += 1;
                assert_eq!(error.original_parent, parent);
                assert_eq!(error.original_conjunction.as_ref(), conjunction);
                assert!(parent.contains(&error.unresolved_parent).unwrap());
                println!(
                    "CAPTURED_GEOMETRY sector={} status=incomplete failure={:?} fixed={:?} equations={:?} stats={:?}",
                    fixture.sector,
                    error.failure,
                    error.unresolved_parent.fixed(),
                    error
                        .unresolved_conjunction
                        .iter()
                        .map(ToString::to_string)
                        .collect::<Vec<_>>(),
                    error.stats,
                );
                if !matches!(
                    error.failure,
                    CaseIntersectionFailure::UnsupportedGeometry
                        | CaseIntersectionFailure::Budget { .. }
                        | CaseIntersectionFailure::RepeatedState
                        | CaseIntersectionFailure::Admission(
                            crate::solver::AffineGeometryError::Coordinate(
                                crate::solver::GeometryError::CompactOverflow { .. }
                            )
                        )
                ) {
                    unexpected.push((fixture.sector, error.failure));
                }
            }
        }
    }
    println!(
        "CAPTURED_GEOMETRY_SUMMARY inputs=13 admitted={admitted} empty={empty} incomplete={unresolved}; geometry diagnostic only, not family closure"
    );
    assert!(
        unexpected.is_empty(),
        "unexpected admission/native failures: {unexpected:?}"
    );
    assert_eq!(admitted + unresolved, 13);
}
