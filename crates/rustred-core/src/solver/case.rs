use super::{Integral, Power, PowerError};

mod affine;
pub use affine::{AffineCase, AffineGeometryError, AffineIntersection};

/// An equality case with some indices fixed and all other indices free.
///
/// This is deliberately not an approximation to an affine or nonlinear case.
/// Coupled equalities need their own exact representation before they can be
/// passed to the sector solver.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct CoordinateCase<const N: usize> {
    fixed: [Option<i16>; N],
}

impl<const N: usize> CoordinateCase<N> {
    pub fn new(fixed: [Option<i16>; N]) -> Result<Self, PowerError> {
        for value in fixed.iter().flatten() {
            Power::new(false, *value)?;
        }
        Ok(Self { fixed })
    }

    pub const fn generic() -> Self {
        Self { fixed: [None; N] }
    }

    pub fn fixed(&self) -> &[Option<i16>; N] {
        &self.fixed
    }

    pub fn is_numerical(&self) -> bool {
        self.fixed.iter().all(Option::is_some)
    }

    pub fn is_in_sector(&self, sector: &[bool; N]) -> bool {
        self.fixed
            .iter()
            .zip(sector)
            .all(|(value, active)| value.is_none_or(|value| (value > 0) == *active))
    }

    pub fn integral(&self) -> Integral<N> {
        Integral::new(std::array::from_fn(|i| {
            Power::new(self.fixed[i].is_none(), self.fixed[i].unwrap_or(0))
                .expect("case construction checked compact powers")
        }))
    }

    /// SpIRed's `matchWithShift` for coordinate cases: fixed coordinates must
    /// agree; every shift tangent to the unfixed coordinates is allowed.
    pub fn matches(&self, integral: &Integral<N>) -> bool {
        self.fixed
            .iter()
            .zip(integral.powers())
            .all(|(fixed, power)| match fixed {
                Some(value) => !power.is_symbolic() && power.value() == *value,
                None => power.is_symbolic(),
            })
    }
}
