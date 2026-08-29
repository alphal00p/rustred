use crate::algebra::{
    CoefficientContext, ExactAlgebraError, ExactAlgebraOperation, IndexedAlgebraError,
    SYMBOLICA_COEFFICIENT_EXPONENT_LIMIT,
};
use crate::family::{AffineDenominator, IntegralFamily};

use super::super::{ParametricIbpError, ParametricIbpGenerator};

#[test]
fn maximal_power_shift_times_parameter_is_a_typed_error_not_a_symbolica_panic() {
    let base = CoefficientContext::new(["x"]);
    let x = base.parameter("x").unwrap();
    let maximal_power = base.coefficient_fixture("x^65535");
    let family = IntegralFamily::new(
        "maximal-power-shift",
        vec!["k".into()],
        Vec::new(),
        base.clone(),
        base.integer(4),
        vec![AffineDenominator::new(x, vec![base.one()])],
        Vec::new(),
        vec![maximal_power],
    )
    .unwrap();

    let generator = ParametricIbpGenerator::try_new(&family).unwrap();
    let batch = generator.prepare_ordinary_ibp().unwrap();
    let error = batch.generate(0).unwrap_err();
    assert!(matches!(
        error,
        ParametricIbpError::Coefficient(IndexedAlgebraError::ExactAlgebra(
            ExactAlgebraError::ExponentLimit {
                operation: ExactAlgebraOperation::Multiply,
                requested: 65_536,
                limit: SYMBOLICA_COEFFICIENT_EXPONENT_LIMIT,
                ..
            }
        ))
    ));
}
