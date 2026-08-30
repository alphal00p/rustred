use crate::algebra::CoefficientContext;
use crate::family::{AffineDenominator, IntegralFamily, IntegralFamilyLimits};

use super::super::ArtifactError;

/// Canonical equal-mass three-loop vacuum family on the six edges of `K4`.
///
/// Scalar coordinates are the generic family order
/// `(k1^2, k1.k2, k1.k3, k2^2, k2.k3, k3^2)` and denominators are
/// `k1^2-1`, `k2^2-1`, `k3^2-1`, `(k3-k1)^2-1`,
/// `(k1-k2)^2-1`, `(k2-k3)^2-1`.
pub(crate) fn canonical_family() -> Result<IntegralFamily, ArtifactError> {
    let context = CoefficientContext::try_new(["d"])?;
    let dimension = context
        .parameter("d")
        .expect("the authenticated pressure-fixture context contains d");
    let zero = context.zero();
    let one = context.one();
    let minus_one = context.integer(-1);
    let minus_two = context.integer(-2);
    Ok(IntegralFamily::new_with_limits(
        "rustred-three-loop-unit-mass-k4-pressure-v1",
        vec!["k1".to_owned(), "k2".to_owned(), "k3".to_owned()],
        Vec::new(),
        context,
        dimension,
        vec![
            AffineDenominator::new(
                minus_one.clone(),
                vec![
                    one.clone(),
                    zero.clone(),
                    zero.clone(),
                    zero.clone(),
                    zero.clone(),
                    zero.clone(),
                ],
            ),
            AffineDenominator::new(
                minus_one.clone(),
                vec![
                    zero.clone(),
                    zero.clone(),
                    zero.clone(),
                    one.clone(),
                    zero.clone(),
                    zero.clone(),
                ],
            ),
            AffineDenominator::new(
                minus_one.clone(),
                vec![
                    zero.clone(),
                    zero.clone(),
                    zero.clone(),
                    zero.clone(),
                    zero.clone(),
                    one.clone(),
                ],
            ),
            AffineDenominator::new(
                minus_one.clone(),
                vec![
                    one.clone(),
                    zero.clone(),
                    minus_two.clone(),
                    zero.clone(),
                    zero.clone(),
                    one.clone(),
                ],
            ),
            AffineDenominator::new(
                minus_one.clone(),
                vec![
                    one.clone(),
                    minus_two.clone(),
                    zero.clone(),
                    one.clone(),
                    zero.clone(),
                    zero.clone(),
                ],
            ),
            AffineDenominator::new(
                minus_one,
                vec![
                    zero.clone(),
                    zero.clone(),
                    zero.clone(),
                    one.clone(),
                    minus_two,
                    one,
                ],
            ),
        ],
        Vec::new(),
        vec![
            zero.clone(),
            zero.clone(),
            zero.clone(),
            zero.clone(),
            zero.clone(),
            zero,
        ],
        IntegralFamilyLimits::default(),
    )?)
}
