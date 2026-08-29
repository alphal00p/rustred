use crate::algebra::CoefficientContext;
use crate::family::{AffineDenominator, CoefficientLocation, IntegralFamily};
use crate::sector::{CutConstraint, Pattern, PatternSlot, Restrictions};

use super::super::{CoefficientMatrix, ConditionSource, Limits, MomentumMap, verify};
use super::{Error, TransportError, compile};

fn equal_mass_sunset(asymmetric_shift: bool) -> IntegralFamily {
    let coefficients = CoefficientContext::new(["d", "m2"]);
    let zero = coefficients.zero();
    let one = coefficients.one();
    let minus_m2 = coefficients.coefficient_fixture("-m2");
    let second_shift = if asymmetric_shift {
        one.clone()
    } else {
        zero.clone()
    };
    IntegralFamily::new(
        "permutation-test-family",
        vec!["k1".into(), "k2".into()],
        Vec::new(),
        coefficients.clone(),
        coefficients.parameter("d").unwrap(),
        vec![
            AffineDenominator::new(
                minus_m2.clone(),
                vec![one.clone(), zero.clone(), zero.clone()],
            ),
            AffineDenominator::new(
                minus_m2.clone(),
                vec![zero.clone(), zero.clone(), one.clone()],
            ),
            AffineDenominator::new(minus_m2, vec![one.clone(), coefficients.integer(2), one]),
        ],
        Vec::new(),
        vec![zero.clone(), second_shift, zero],
    )
    .unwrap()
}

fn swap_loop_momenta(coefficients: &CoefficientContext) -> MomentumMap {
    let zero = coefficients.zero();
    let one = coefficients.one();
    MomentumMap::new(
        CoefficientMatrix::try_new(2, 2, [zero.clone(), one.clone(), one, zero]).unwrap(),
        CoefficientMatrix::try_new(2, 0, []).unwrap(),
        CoefficientMatrix::try_new(0, 0, []).unwrap(),
    )
}

fn verified_swap(family: &IntegralFamily) -> super::super::VerifiedMap {
    verify(
        family,
        family,
        swap_loop_momenta(family.coefficient_context()),
        Limits::default(),
    )
    .unwrap()
}

#[test]
fn compile_retains_exact_proof_and_intrinsic_bijection() {
    let family = equal_mass_sunset(false);
    let affine = verified_swap(&family);
    let determinant_condition = affine
        .nonzero_conditions()
        .iter()
        .find(|condition| {
            condition.polynomial() == family.domain().determinant_nonzero().polynomial()
        })
        .unwrap();
    assert!(
        determinant_condition
            .sources()
            .contains(&ConditionSource::SourceFamily(
                CoefficientLocation::BasisDeterminantNumerator,
            ))
    );
    assert!(
        determinant_condition
            .sources()
            .contains(&ConditionSource::TargetFamily(
                CoefficientLocation::BasisDeterminantNumerator,
            ))
    );
    assert_eq!(
        affine.stats().condition_sources(),
        affine
            .nonzero_conditions()
            .iter()
            .map(|condition| condition.sources().len())
            .sum::<usize>()
    );

    let permutation = compile(&family, affine).unwrap();
    assert_eq!(permutation.denominator_count(), 3);
    assert_eq!(permutation.source_for_target(), &[1, 0, 2]);
    assert_eq!(
        permutation.affine().source_family_fingerprint(),
        family.fingerprint()
    );
}

#[test]
fn compile_rejects_power_shift_transport() {
    let family = equal_mass_sunset(true);
    assert!(matches!(
        compile(&family, verified_swap(&family)),
        Err(Error::PowerShiftMismatch {
            source: 1,
            target: 0,
        })
    ));
}

#[test]
fn restrictions_are_checked_only_when_selected_for_application() {
    let family = equal_mass_sunset(false);
    let permutation = compile(&family, verified_swap(&family)).unwrap();
    permutation
        .check_restrictions(&Restrictions::unrestricted(3).unwrap())
        .unwrap();

    let cut = Restrictions::try_new(
        CutConstraint::try_from_positions(3, [0]).unwrap(),
        Pattern::any(3).unwrap(),
    )
    .unwrap();
    assert_eq!(
        permutation.check_restrictions(&cut),
        Err(Error::CutMismatch {
            source: 1,
            target: 0,
        })
    );

    let pattern = Restrictions::try_new(
        CutConstraint::none(3).unwrap(),
        Pattern::try_new([PatternSlot::Active, PatternSlot::Any, PatternSlot::Any]).unwrap(),
    )
    .unwrap();
    assert_eq!(
        permutation.check_restrictions(&pattern),
        Err(Error::PatternMismatch {
            source: 1,
            target: 0,
        })
    );
}

#[test]
fn restriction_and_transport_arities_fail_closed() {
    let family = equal_mass_sunset(false);
    let permutation = compile(&family, verified_swap(&family)).unwrap();
    assert_eq!(
        permutation.check_restrictions(&Restrictions::unrestricted(2).unwrap()),
        Err(Error::WrongRestrictionArity {
            expected: 3,
            actual: 2,
        })
    );

    let short_source = [1, 2];
    let source = [1, 2, 3];
    let mut target = [0; 3];
    assert_eq!(
        permutation.transport_into(&short_source, &mut target),
        Err(TransportError::WrongSourceArity {
            expected: 3,
            actual: 2,
        })
    );
    assert_eq!(
        permutation.transport_into(&source, &mut target[..2]),
        Err(TransportError::WrongTargetArity {
            expected: 3,
            actual: 2,
        })
    );
}

#[test]
fn transport_reuses_caller_owned_storage() {
    let family = equal_mass_sunset(false);
    let permutation = compile(&family, verified_swap(&family)).unwrap();
    let mut target = [i64::MIN; 3];

    permutation
        .transport_into(&[2, 3, -1], &mut target)
        .unwrap();
    assert_eq!(target, [3, 2, -1]);

    permutation
        .transport_into(&[-4, 7, 5], &mut target)
        .unwrap();
    assert_eq!(target, [7, -4, 5]);
}
