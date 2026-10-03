//! Bounded exact integer exclusions for a free local lower cut.
//! The proof still checks the original chart; these are transport predicates.
use super::*;

pub(super) struct LowerCuts {
    axes: Vec<(usize, u64)>,
    count: usize,
}

impl LowerCuts {
    pub(super) fn new<const N: usize>(
        lower: &[u64],
        upper: &[Option<u64>],
        fixed: &[Option<i16>; N],
        sector: &[bool; N],
        maximum: usize,
    ) -> Result<Self, AppError> {
        if lower.len() != N || upper.len() != N {
            return Err(error("priority lower-cut arity mismatch"));
        }
        let mut count = 0usize;
        // Admit the whole count and largest physical boundary before any
        // boundary-sized allocation, enumeration or native arithmetic.
        for axis in 0..N {
            if fixed[axis].is_some() {
                continue;
            }
            if upper[axis].is_some() {
                return Err(error(
                    "priority bridge cannot exactly encode this free-axis bound",
                ));
            }
            let amount = usize::try_from(lower[axis]).map_err(error)?;
            count = count
                .checked_add(amount)
                .ok_or_else(|| AppError::limit("priority lower-cut count overflow"))?;
            if count > maximum {
                return Err(AppError::limit(
                    "priority lower-cut exclusions exceed guard/collection budget",
                ));
            }
            if lower[axis] != 0 {
                Self::physical_boundary(sector[axis], lower[axis] - 1)?;
            }
        }
        let axes = (0..N)
            .filter(|&i| fixed[i].is_none() && lower[i] != 0)
            .map(|i| (i, lower[i]))
            .collect();
        Ok(Self { axes, count })
    }

    fn physical_boundary(active: bool, local: u64) -> Result<i64, AppError> {
        i64::try_from(if active {
            1 + i128::from(local)
        } else {
            -i128::from(local)
        })
        .map_err(error)
    }

    pub(super) fn check_existing(
        &self,
        existing: &[Vec<CoefficientPolynomial>],
        maximum: usize,
    ) -> Result<(), AppError> {
        let entries = existing.iter().try_fold(self.count, |n, branch| {
            n.checked_add(branch.len())
                .ok_or_else(|| AppError::limit("priority exclusion entry count overflow"))
        })?;
        let branches = existing
            .len()
            .checked_add(self.count)
            .ok_or_else(|| AppError::limit("priority exclusion branch count overflow"))?;
        if entries > maximum || branches > maximum {
            return Err(AppError::limit(
                "priority combined exclusions exceed guard/collection budget",
            ));
        }
        Ok(())
    }

    pub(super) fn polynomials<const N: usize>(
        &self,
        context: &IndexedCoefficientContext,
        sector: &[bool; N],
        algebra: ExactAlgebraLimits,
    ) -> Result<Vec<IndexedPolynomial>, AppError> {
        let mut result = Vec::new();
        result
            .try_reserve_exact(self.count)
            .map_err(|_| AppError::limit("priority lower-cut allocation failed"))?;
        // Axis-major, local-ascending append order preserves the old lower0/1
        // byte encoding. Each singleton is an OR branch, never a conjunction.
        for &(axis, lower) in &self.axes {
            for local in 0..lower {
                let boundary = context
                    .sub(
                        &context.index(axis).map_err(error)?,
                        &context.integer(Self::physical_boundary(sector[axis], local)?),
                    )
                    .map_err(error)?;
                result.push(
                    context
                        .numerator_condition_with_limits(&boundary, algebra)
                        .map_err(error)?,
                );
            }
        }
        Ok(result)
    }
}
