//! Single-scale kinematics of the unit-scale publication lanes.
//!
//! A unit-scale family has exactly one dimensionful scale, fixed to a unit
//! value so that every IBP coefficient stays in `Q(d)`. Dimensional
//! homogeneity restores the scale afterwards: with every denominator
//! homogeneous of mass dimension two, an integral scales as
//! `scale^(L*d/2 - sum powers)`, so a reduction coefficient from target `a`
//! to terminal `b` carries `scale^(sum b - sum a)`. The same law serves the
//! common mass of a vacuum family and the external momentum of a
//! propagator-type ("p-integral") family; only the meaning of the scale
//! differs, and that meaning is recorded here.

use super::{IntegralFamily, IntegralFamilyError};

/// The kinematics of a family admitted by the unit-scale lanes.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum UnitScaleKinematics {
    /// No external momenta and one common unit mass: every denominator
    /// constant is `-1` (`k^2 - m^2` at `m^2 = 1`). The scale is `m^2`.
    UnitMassVacuum,
    /// Exactly one external momentum `q` with the numeric Gram entry
    /// `q.q = external_square`, either `1` or `-1`, and rational denominator
    /// constants. A constant is then a rational multiple of `q.q`, so every
    /// denominator is a homogeneous quadratic form in the loop momenta and
    /// `q` (massless lines, and ISPs such as `k.q` or `(k1-k2)^2`). The scale
    /// is `q^2 / external_square`: one at the family's own kinematic point.
    UnitScalePropagator { external_square: i8 },
}

impl IntegralFamily {
    /// Classify this family as a unit-scale family, or `None` when it has
    /// another shape (masses other than one, extra parameters, a numeric or
    /// shifted dimension, power shifts, several external momenta, or a
    /// symbolic or non-unit external Gram entry). Both kinds require `d` to
    /// be the sole parameter and the dimension.
    pub fn unit_scale_kinematics(
        &self,
    ) -> Result<Option<UnitScaleKinematics>, IntegralFamilyError> {
        let base = self.coefficient_context();
        if base.parameter_names() != ["d"]
            || base.parameter("d").as_ref() != Some(self.dimension())
            || self.power_shifts().iter().any(|shift| !shift.is_zero())
        {
            return Ok(None);
        }
        let minus_one = base.try_neg(&base.one(), Default::default())?;
        match self.external_count() {
            0 => Ok(self
                .denominators()
                .iter()
                .all(|denominator| denominator.constant() == &minus_one)
                .then_some(UnitScaleKinematics::UnitMassVacuum)),
            1 => {
                let square = &self.external_gram()[0][0];
                let external_square = if square == &base.one() {
                    1
                } else if square == &minus_one {
                    -1
                } else {
                    return Ok(None);
                };
                Ok(self
                    .denominators()
                    .iter()
                    .all(|denominator| {
                        let constant = denominator.constant();
                        constant.numerator.is_constant() && constant.denominator.is_constant()
                    })
                    .then_some(UnitScaleKinematics::UnitScalePropagator { external_square }))
            }
            _ => Ok(None),
        }
    }
}
