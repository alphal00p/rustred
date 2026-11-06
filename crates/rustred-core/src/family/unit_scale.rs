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
//!
//! A propagator-type family must be massless: every slot whose quadratic
//! form is a multiple of one square `(a.k + b q)^2` (a line) must carry
//! exactly the constant of that square, and every other slot (an irreducible
//! scalar product) the constant zero. Otherwise a genuine mass would be
//! silently tied to `q^2` when the scale is restored.
//!
//! Eps families (lines with powers `n + j eps`, `d = 4 - 2 eps`) are admitted
//! only through [`IntegralFamily::shifted_unit_scale_kinematics`], the p-only
//! admission; [`IntegralFamily::unit_scale_kinematics`] and every consumer
//! built on it keep refusing every power shift.

use symbolica::prelude::{Integer, Rational};

use crate::algebra::{Coefficient, CoefficientContext, ExactAlgebraLimits};

use super::{IntegralFamily, IntegralFamilyError, ScalarProductCoordinate};

/// The kinematics of a family admitted by the unit-scale lanes.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum UnitScaleKinematics {
    /// No external momenta and one common unit mass: every denominator
    /// constant is `-1` (`k^2 - m^2` at `m^2 = 1`). The scale is `m^2`.
    UnitMassVacuum,
    /// Exactly one external momentum `q` with the numeric Gram entry
    /// `q.q = external_square`, either `1` or `-1`, and massless slots: a
    /// line `(a.k + b q)^2` carries the constant `b^2 q.q`, every other slot
    /// (an ISP such as `k.q` or `k1.k2`) the constant zero. Every denominator
    /// is then a homogeneous quadratic form in the loop momenta and `q`. The
    /// scale is `q^2 / external_square`: one at the family's own kinematic
    /// point.
    UnitScalePropagator { external_square: i8 },
}

/// The kinematics of a massless propagator-type family on the p-only
/// admission path, which also admits eps lines.
///
/// Exactly the families of [`UnitScaleKinematics::UnitScalePropagator`]
/// shape (one external momentum, `q.q = +-1`, massless slots, `d` the sole
/// parameter and the dimension) whose nonzero power shifts are
/// `j (4-d)/2` with an integer `j != 0` on line slots: a line of power
/// `n + j eps` at `d = 4 - 2 eps`. Unshifted families are admitted with no
/// forced axis.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct ShiftedUnitScale {
    /// The numeric Gram entry `q.q`, `1` or `-1`.
    pub external_square: i8,
    /// The forced (eps) axes in increasing slot order: `(slot, j)` for the
    /// power shift `j (4-d)/2`. An eps line is present at every integer
    /// power `n`, so a forced axis belongs to the support of every integral
    /// of the family.
    pub forced: Vec<(usize, i64)>,
}

impl ShiftedUnitScale {
    /// Whether slot `slot` is a forced (eps) axis.
    pub fn is_forced(&self, slot: usize) -> bool {
        self.forced.iter().any(|&(forced, _)| forced == slot)
    }

    /// The eps units `j` of slot `slot`, zero for an unshifted slot.
    pub fn eps_units(&self, slot: usize) -> i64 {
        self.forced
            .iter()
            .find_map(|&(forced, units)| (forced == slot).then_some(units))
            .unwrap_or(0)
    }
}

/// The quadratic shape of one slot of a massless propagator-type family.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum SlotShape {
    /// A multiple of one square `(a.k + b q)^2` with `a != 0`.
    Line,
    /// Any other homogeneous quadratic form, for example `k.q` or `k1.k2`.
    Isp,
}

impl IntegralFamily {
    /// Classify this family as a unit-scale family, or `None` when it has
    /// another shape (masses other than one, extra parameters, a numeric or
    /// shifted dimension, power shifts, several external momenta, a
    /// symbolic or non-unit external Gram entry, or, with one external
    /// momentum, a massive slot). Both kinds require `d` to be the sole
    /// parameter and the dimension.
    pub fn unit_scale_kinematics(
        &self,
    ) -> Result<Option<UnitScaleKinematics>, IntegralFamilyError> {
        let base = self.coefficient_context();
        if !self.has_sole_dimension_parameter()
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
            1 => Ok(self
                .massless_propagator_slots()?
                .map(
                    |(external_square, _)| UnitScaleKinematics::UnitScalePropagator {
                        external_square,
                    },
                )),
            _ => Ok(None),
        }
    }

    /// The p-only admission of massless propagator-type families with eps
    /// lines, see [`ShiftedUnitScale`]. `None` for every other shape: a
    /// vacuum family, several external momenta, a non-unit or symbolic Gram
    /// entry, a massive slot, extra parameters, a shift on an ISP slot, or a
    /// shift other than `j (4-d)/2` with integer `j != 0`.
    ///
    /// Only the p-integral path calls this; the unit-scale consumers use
    /// [`Self::unit_scale_kinematics`], which refuses every power shift.
    pub fn shifted_unit_scale_kinematics(
        &self,
    ) -> Result<Option<ShiftedUnitScale>, IntegralFamilyError> {
        if !self.has_sole_dimension_parameter() || self.external_count() != 1 {
            return Ok(None);
        }
        let Some((external_square, shapes)) = self.massless_propagator_slots()? else {
            return Ok(None);
        };
        let context = self.coefficient_context();
        let limits = ExactAlgebraLimits::default();
        let mut forced = Vec::new();
        for (slot, shift) in self.power_shifts().iter().enumerate() {
            if shift.is_zero() {
                continue;
            }
            if shapes[slot] != SlotShape::Line {
                return Ok(None);
            }
            match eps_units(context, self.dimension(), shift, limits)? {
                Some(units) => forced.push((slot, units)),
                None => return Ok(None),
            }
        }
        Ok(Some(ShiftedUnitScale {
            external_square,
            forced,
        }))
    }

    /// `d` is the sole coefficient parameter and the dimension.
    fn has_sole_dimension_parameter(&self) -> bool {
        let base = self.coefficient_context();
        base.parameter_names() == ["d"] && base.parameter("d").as_ref() == Some(self.dimension())
    }

    /// With one external momentum of numeric square `+-1`: the square and
    /// the shape of every slot when every slot is massless, else `None`.
    ///
    /// A slot `D = c + sum K_ij k_i.k_j + sum 2 B_i k_i.q` (numeric rational
    /// coefficients) is a line when its loop block `K` has rank one and `B`
    /// lies in its column space, so that `D - c + c'` is a multiple of
    /// `(a.k + b q)^2` for exactly one constant `c'`; it is massless when
    /// `c = c'`, i.e. `c K_pp = q.q B_p^2` at any pivot `K_pp != 0`. Every
    /// other slot is an ISP and must have `c = 0`.
    fn massless_propagator_slots(
        &self,
    ) -> Result<Option<(i8, Vec<SlotShape>)>, IntegralFamilyError> {
        let base = self.coefficient_context();
        let minus_one = base.try_neg(&base.one(), Default::default())?;
        let [row] = self.external_gram() else {
            return Ok(None);
        };
        let [square] = row.as_slice() else {
            return Ok(None);
        };
        let external_square: i8 = if square == &base.one() {
            1
        } else if square == &minus_one {
            -1
        } else {
            return Ok(None);
        };
        let gram = Rational::from(i64::from(external_square));
        let loops = self.loop_count();
        let half = Rational::from((Integer::from(1), Integer::from(2)));
        let mut shapes = Vec::with_capacity(self.denominator_count());
        for denominator in self.denominators() {
            let Some(constant) = numeric(denominator.constant()) else {
                return Ok(None);
            };
            let mut quadratic = vec![vec![Rational::zero(); loops]; loops];
            let mut mixed = vec![Rational::zero(); loops];
            for (coordinate, value) in self.coordinates().iter().zip(denominator.coefficients()) {
                let Some(value) = numeric(value) else {
                    return Ok(None);
                };
                match *coordinate {
                    ScalarProductCoordinate::LoopLoop { left, right } if left == right => {
                        quadratic[left][left] = value;
                    }
                    ScalarProductCoordinate::LoopLoop { left, right } => {
                        let value = &value * &half;
                        quadratic[left][right] = value.clone();
                        quadratic[right][left] = value;
                    }
                    ScalarProductCoordinate::LoopExternal { loop_index, .. } => {
                        mixed[loop_index] = &value * &half;
                    }
                }
            }
            let shape = match line_pivot(&quadratic, &mixed) {
                Some(pivot) => {
                    if &constant * &quadratic[pivot][pivot]
                        != &gram * &(&mixed[pivot] * &mixed[pivot])
                    {
                        return Ok(None);
                    }
                    SlotShape::Line
                }
                None => {
                    if !constant.is_zero() {
                        return Ok(None);
                    }
                    SlotShape::Isp
                }
            };
            shapes.push(shape);
        }
        Ok(Some((external_square, shapes)))
    }
}

/// The exact rational value of a numeric coefficient.
fn numeric(value: &Coefficient) -> Option<Rational> {
    (value.numerator.is_constant() && value.denominator.is_constant()).then(|| {
        Rational::from((
            value.numerator.get_constant(),
            value.denominator.get_constant(),
        ))
    })
}

/// A pivot `p` with `K_pp != 0` when the symmetric loop block `K` has rank
/// one (`K_ij K_pp = K_ip K_jp`) and the loop-external vector `B` lies in its
/// column space (`B_i K_pp = B_p K_ip`), i.e. the form is a multiple of a
/// square `(a.k + b q)^2`; `None` otherwise.
fn line_pivot(quadratic: &[Vec<Rational>], mixed: &[Rational]) -> Option<usize> {
    let pivot = (0..quadratic.len()).find(|&p| !quadratic[p][p].is_zero())?;
    let k_pp = &quadratic[pivot][pivot];
    let rank_one = (0..quadratic.len()).all(|i| {
        (0..quadratic.len())
            .all(|j| &quadratic[i][j] * k_pp == &quadratic[i][pivot] * &quadratic[j][pivot])
    });
    let in_span =
        (0..mixed.len()).all(|i| &mixed[i] * k_pp == &mixed[pivot] * &quadratic[i][pivot]);
    (rank_one && in_span).then_some(pivot)
}

/// The integer `j != 0` of a power shift `j (4-d)/2`, else `None`.
fn eps_units(
    context: &CoefficientContext,
    dimension: &Coefficient,
    shift: &Coefficient,
    limits: ExactAlgebraLimits,
) -> Result<Option<i64>, IntegralFamilyError> {
    let unit = context.try_div(
        &context.try_sub(&context.integer(4), dimension, limits)?,
        &context.integer(2),
        limits,
    )?;
    let units = context.try_div(shift, &unit, limits)?;
    if !units.denominator.is_one() || !units.numerator.is_constant() {
        return Ok(None);
    }
    let Ok(units) = i64::try_from(units.numerator.get_constant()) else {
        return Ok(None);
    };
    Ok((units != 0).then_some(units))
}
