use symbolica::prelude::*;

use crate::algebra::{Coefficient as NativeCoefficient, CoefficientContext};
use crate::family::{AffineDenominator, IntegralFamily};

use super::*;

#[test]
fn sector_configuration_installs_and_validates_coordinate_priority() {
    let system = SourceSystem::<3>::from_family(&sunset()).unwrap();
    let config = SectorConfig {
        permutation: Some([2, 0, 1]),
        ..Default::default()
    };
    let solver = SectorSolver::new(&system, [true; 3], config).unwrap();
    assert_eq!(solver.ordering().permutation(), Some(&[2, 0, 1]));
    assert!(matches!(
        SectorSolver::new(
            &system,
            [true; 3],
            SectorConfig {
                permutation: Some([2, 2, 1]),
                ..Default::default()
            }
        ),
        Err(SolverError::InvalidInput(_))
    ));
}

/// Quadratic forms for test families only; the engine never dispatches on
/// these loop counts or momentum routes.
pub(super) fn vacuum(momenta: &[Vec<i64>]) -> IntegralFamily {
    let loops = momenta[0].len();
    let context = CoefficientContext::new(["d"]);
    let denominators = momenta
        .iter()
        .map(|momentum| {
            let coefficients = (0..loops)
                .flat_map(|i| {
                    (i..loops).map(move |j| momentum[i] * momentum[j] * if i == j { 1 } else { 2 })
                })
                .map(|x| context.integer(x))
                .collect();
            AffineDenominator::new(context.integer(-1), coefficients)
        })
        .collect();
    IntegralFamily::new(
        "source-port-vacuum",
        (0..loops).map(|i| format!("k{i}")).collect(),
        Vec::new(),
        context.clone(),
        context.parameter("d").unwrap(),
        denominators,
        Vec::new(),
        vec![context.zero(); momenta.len()],
    )
    .unwrap()
}

pub(super) fn tadpole() -> IntegralFamily {
    vacuum(&[vec![1]])
}
pub(super) fn sunset() -> IntegralFamily {
    vacuum(&[vec![1, 0], vec![0, 1], vec![1, 1]])
}
pub(super) fn vac3() -> IntegralFamily {
    vacuum(&[
        vec![1, 0, 0],
        vec![0, 1, 0],
        vec![0, 0, 1],
        vec![1, 1, 0],
        vec![1, 0, 1],
        vec![0, 1, -1],
    ])
}

#[test]
fn tadpole_direct_rule_uses_a_shifted_pivot_and_exact_native_coefficients() {
    let system = SourceSystem::<1>::from_family(&tadpole()).unwrap();
    assert_eq!(system.rows().len(), 1);
    let solver = SectorSolver::new(&system, [true], SectorConfig::default()).unwrap();
    let candidate = solver
        .solve_case(CoordinateCase::generic(), SearchOptions::default())
        .unwrap();
    assert!(candidate.stats.direct_hit);
    assert_eq!(candidate.stats.seeds, 1);
    assert_eq!(candidate.stats.rows, 1);
    assert!(
        candidate.stats.discovery.is_none(),
        "generic hit must not construct GPLU"
    );
    assert_eq!(candidate.target, Integral::symbolic([0]).unwrap());
    assert_eq!(candidate.rhs.len(), 1);
    assert_eq!(candidate.rhs[0].integral, Integral::symbolic([-1]).unwrap());

    let polynomial = &system.rows()[0][0].coefficient;
    let n: NativeCoefficient = polynomial
        .variable(&polynomial.variables()[system.index_variables()[0]])
        .unwrap()
        .into();
    let d: NativeCoefficient = polynomial
        .variable(&polynomial.variables()[0])
        .unwrap()
        .into();
    let two: NativeCoefficient = polynomial.constant(Integer::from(2)).into();
    let one: NativeCoefficient = polynomial.one().into();
    // D=k²-1: I(n)=(d-2n+2)/(2(n-1)) I(n-1).
    let expected = &(&d - &(&two * &n)) + &two;
    let expected = &expected / &(&two * &(&n - &one));
    assert_eq!(candidate.rhs[0].coefficient, expected);
}

#[test]
fn tadpole_numeric_case_reduces_and_master_corner_remains_a_bounded_miss() {
    let system = SourceSystem::<1>::from_family(&tadpole()).unwrap();
    let config = SectorConfig {
        zero_sectors: vec![[false]].into(),
        ..Default::default()
    };
    let solver = SectorSolver::new(&system, [true], config).unwrap();
    let options = SearchOptions {
        max_depth: Some(2),
        ..Default::default()
    };
    let candidate = solver
        .solve_case(CoordinateCase::new([Some(2)]).unwrap(), options)
        .unwrap();
    assert_eq!(candidate.target, Integral::numeric([2]).unwrap());
    assert_eq!(candidate.rhs[0].integral, Integral::numeric([1]).unwrap());
    assert!(matches!(
        solver.solve_case(CoordinateCase::new([Some(1)]).unwrap(), options),
        Err(SolverError::SearchExhausted { .. })
    ));
}

#[test]
fn actual_k3_and_k6_generic_cases_take_the_reference_direct_path() {
    fn check<const N: usize>(family: IntegralFamily, count: usize) {
        let sources = SourceSystem::<N>::from_family(&family).unwrap();
        assert_eq!(sources.rows().len(), count);
        let solver = SectorSolver::new(&sources, [true; N], SectorConfig::default()).unwrap();
        let candidate = solver
            .solve_case(CoordinateCase::generic(), SearchOptions::default())
            .unwrap();
        assert!(candidate.stats.direct_hit);
        assert_eq!(candidate.stats.seeds, 1);
        assert_eq!(candidate.stats.rows, 1);
        assert!(candidate.stats.discovery.is_none());
        assert_eq!(candidate.target, Integral::symbolic([0; N]).unwrap());
        assert!(!candidate.rhs.is_empty());
        for term in &candidate.rhs {
            assert_eq!(
                solver.ordering().compare(&candidate.target, &term.integral),
                std::cmp::Ordering::Less
            );
        }
    }
    check::<3>(sunset(), 4);
    check::<6>(vac3(), 9);
}

#[test]
fn coordinate_matching_accepts_tangent_shifts_but_not_fixed_displacements() {
    let case = CoordinateCase::new([Some(1), None]).unwrap();
    let matching = Integral::new([Power::new(false, 1).unwrap(), Power::new(true, 3).unwrap()]);
    assert!(case.matches(&matching));
    let displaced = Integral::new([Power::new(false, 2).unwrap(), Power::new(true, 3).unwrap()]);
    assert!(!case.matches(&displaced));
    assert!(!case.is_in_sector(&[false, true]));
}

#[test]
fn numeric_sector_classification_tracks_activation_as_well_as_pinches() {
    let context = CoefficientContext::new(["a", "b"]);
    let source = vec![Term {
        integral: Integral::symbolic([-1, 1]).unwrap(),
        coefficient: context.coefficient_fixture("1").numerator,
    }];
    let seed = Seed {
        integral: Integral::numeric([1, 0]).unwrap(),
        shifts: [0, 0],
    };
    let order = IntegralOrder::new([true, false], [false; 2]);
    let row = super::instantiate::instantiate(
        &source,
        &seed,
        &[0, 1],
        &[None; 2],
        &order,
        &[[false, false]],
        None,
    )
    .unwrap();
    assert_eq!(
        row.len(),
        1,
        "the actual [false,true] sector is not the listed zero sector"
    );
    assert_eq!(row[0].integral, Integral::numeric([0, 1]).unwrap());
    let vanished = super::instantiate::instantiate(
        &source,
        &seed,
        &[0, 1],
        &[None; 2],
        &order,
        &[[false, true]],
        None,
    )
    .unwrap();
    assert!(vanished.is_empty());
}

#[test]
fn rational_affine_source_restriction_keeps_scale_and_original_columns() {
    let context = CoefficientContext::new(["a", "b"]);
    let equation = context.coefficient_fixture("2*a-b-4").numerator;
    let AffineIntersection::Affine(case) =
        AffineCase::from_coordinate(&CoordinateCase::generic(), &[equation], &[0, 1], &[true; 2])
            .unwrap()
    else {
        panic!("expected an exact affine case")
    };
    let source = vec![
        Term {
            integral: Integral::symbolic([0, 0]).unwrap(),
            coefficient: context.coefficient_fixture("a").numerator,
        },
        Term {
            integral: Integral::symbolic([-1, 0]).unwrap(),
            coefficient: context.one().numerator,
        },
    ];
    let order = IntegralOrder::new([true; 2], [false; 2]);
    let row = super::instantiate::instantiate(
        &source,
        &Seed {
            integral: Integral::symbolic([0, 0]).unwrap(),
            shifts: [0, 0],
        },
        &[0, 1],
        &[None; 2],
        &order,
        &[],
        Some(&case),
    )
    .unwrap();
    assert_eq!(row.len(), 2);
    assert_eq!(row[0].coefficient, context.coefficient_fixture("(b+4)/2"));
    assert_ne!(row[0].integral, row[1].integral);
    let (_, rhs) = super::instantiate::canonicalize(row, &[0, 1]).unwrap();
    assert_eq!(rhs[0].coefficient, context.coefficient_fixture("-2/(b+4)"));

    // A source displacement need not be tangent. Translation must precede
    // restriction: (2a-b-4)(a+1,b) restricts to 2, not 0.
    let source = vec![Term {
        integral: Integral::symbolic([0, 0]).unwrap(),
        coefficient: context.coefficient_fixture("2*a-b-4").numerator,
    }];
    let row = super::instantiate::instantiate(
        &source,
        &Seed {
            integral: Integral::symbolic([1, 0]).unwrap(),
            shifts: [1, 0],
        },
        &[0, 1],
        &[None; 2],
        &order,
        &[],
        Some(&case),
    )
    .unwrap();
    assert_eq!(row.len(), 1);
    assert_eq!(row[0].integral, Integral::symbolic([1, 0]).unwrap());
    assert_eq!(row[0].coefficient, context.integer(2));
}

#[test]
fn invalid_field_and_coordinate_case_are_rejected_at_entry() {
    let system = SourceSystem::<1>::from_family(&tadpole()).unwrap();
    let solver = SectorSolver::new(&system, [true], SectorConfig::default()).unwrap();
    let options = SearchOptions {
        prime: 15,
        ..Default::default()
    };
    assert!(matches!(
        solver.solve_case(CoordinateCase::generic(), options),
        Err(SolverError::InvalidInput(_))
    ));
    assert!(matches!(
        solver.solve_case(
            CoordinateCase::new([Some(0)]).unwrap(),
            SearchOptions::default()
        ),
        Err(SolverError::InvalidInput(_))
    ));
}

#[test]
fn vanished_cuts_are_zero_even_without_an_explicit_zero_sector_list() {
    let mut system = SourceSystem::<1>::from_family(&tadpole()).unwrap();
    let one = system.rows[0][0].coefficient.one();
    system.rows = vec![vec![
        Term {
            integral: Integral::symbolic([0]).unwrap(),
            coefficient: one.clone(),
        },
        Term {
            integral: Integral::symbolic([-1]).unwrap(),
            coefficient: -one,
        },
    ]];
    let config = SectorConfig {
        deltas: [true],
        removed_deltas: [true],
        permutation: None,
        zero_sectors: Vec::new().into(),
    };
    let solver = SectorSolver::new(&system, [true], config).unwrap();
    let candidate = solver
        .solve_case(
            CoordinateCase::new([Some(1)]).unwrap(),
            SearchOptions::default(),
        )
        .unwrap();
    assert!(candidate.rhs.is_empty());
}

#[test]
fn depth_limit_is_checked_before_overflow_in_an_excluded_shell() {
    let mut system = SourceSystem::<1>::from_family(&tadpole()).unwrap();
    let template = &system.rows[0][0].coefficient;
    let index = template
        .variable(&template.variables()[system.indices[0]])
        .unwrap();
    let coefficient = &index + &template.constant(Integer::from(64));
    system.rows = vec![vec![Term {
        integral: Integral::symbolic([0]).unwrap(),
        coefficient,
    }]];
    let solver = SectorSolver::new(&system, [false], SectorConfig::default()).unwrap();
    let options = SearchOptions {
        max_depth: Some(0),
        ..Default::default()
    };
    assert!(matches!(
        solver.solve_case(CoordinateCase::new([Some(-64)]).unwrap(), options),
        Err(SolverError::SearchExhausted { depth: 0, rows: 1 })
    ));
}

#[test]
fn dimension_precedes_scalars_in_the_reference_coefficient_priority() {
    let context = CoefficientContext::new(["m2", "d"]);
    let family = IntegralFamily::new(
        "dimension-priority",
        vec!["k".into()],
        Vec::new(),
        context.clone(),
        context.parameter("d").unwrap(),
        vec![AffineDenominator::new(
            context.parameter("m2").unwrap(),
            vec![context.one()],
        )],
        Vec::new(),
        vec![context.zero()],
    )
    .unwrap();
    let sources = SourceSystem::<1>::from_family(&family).unwrap();
    assert_eq!(sources.coefficient_order(), &[2, 1, 0]);
}
