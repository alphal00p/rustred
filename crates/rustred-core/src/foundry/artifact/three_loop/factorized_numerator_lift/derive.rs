//! K6-only adapters for the angular-moment validation oracle.

use crate::algebra::ExactAlgebraError;
use crate::family::{IntegralFamily, ScalarProductCoordinate};
use crate::foundry::artifact::FactorizationRule;
use crate::foundry::artifact::factorized_numerator_lift::RoutedAffineDenominator;

use super::ARITY;
use super::exact_limits;
use super::model::CornerAngularForm;

pub(super) fn factorization_for_sector<'a>(
    rules: &'a [FactorizationRule],
    sector: &[i64; ARITY],
) -> &'a FactorizationRule {
    rules
        .iter()
        .find(|rule| {
            rule.application_domain()
                .sector()
                .active_bits()
                .iter()
                .zip(sector)
                .all(|(&active, &power)| active == (power >= 1))
        })
        .unwrap()
}

pub(super) fn corner_angular_form(
    family: &IntegralFamily,
    form: &RoutedAffineDenominator,
) -> Result<CornerAngularForm, ExactAlgebraError> {
    let context = family.coefficient_context();
    let mut constant = form.constant().clone();
    let mut cross_coefficients = std::array::from_fn(|_| context.zero());
    for (slot, coordinate) in family.coordinates().iter().enumerate() {
        match *coordinate {
            // At a one-loop tadpole corner, every positive radial moment
            // q_i^(2r) equals the corner after scaleless polynomial pieces
            // are discarded. The production routing action does not make this
            // undotted-corner simplification.
            ScalarProductCoordinate::LoopLoop { left, right } if left == right => {
                constant = context.try_add(
                    &constant,
                    &form.scalar_coefficients()[slot],
                    exact_limits(),
                )?;
            }
            ScalarProductCoordinate::LoopLoop { left: 0, right: 1 } => {
                cross_coefficients[0] = form.scalar_coefficients()[slot].clone();
            }
            ScalarProductCoordinate::LoopLoop { left: 0, right: 2 } => {
                cross_coefficients[1] = form.scalar_coefficients()[slot].clone();
            }
            ScalarProductCoordinate::LoopLoop { left: 1, right: 2 } => {
                cross_coefficients[2] = form.scalar_coefficients()[slot].clone();
            }
            ScalarProductCoordinate::LoopLoop { .. } => unreachable!(),
            ScalarProductCoordinate::LoopExternal { .. } => {
                panic!("the K=6 angular-moment oracle is a vacuum fixture")
            }
        }
    }
    Ok(CornerAngularForm {
        constant,
        cross_coefficients,
    })
}
