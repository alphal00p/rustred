use crate::sector::{self, Mask};

use super::analysis::ZeroSectorAnalyzer;
use super::error::ZeroSectorError;
use super::limits::{check_limit, checked_add, checked_mul};
use super::rank::ExponentMatrix;

impl ZeroSectorAnalyzer {
    pub(super) fn exponent_matrix(
        &self,
        effective: &Mask,
    ) -> Result<ExponentMatrix, ZeroSectorError> {
        if effective.arity() != self.symanzik.context().parameter_count() {
            return Err(ZeroSectorError::Sector(sector::Error::WrongArity {
                expected: self.symanzik.context().parameter_count(),
                actual: effective.arity(),
            }));
        }
        let active_parameter_order = effective
            .active_bits()
            .iter()
            .enumerate()
            .filter_map(|(parameter, &active)| active.then_some(parameter))
            .collect::<Vec<_>>();
        let columns = checked_add(active_parameter_order.len(), 1, "rank matrix columns")?;
        check_limit("rank matrix columns", columns, self.limits.max_rank_columns)?;
        let mut rows = Vec::new();
        for (_, exponents) in self.symanzik.g().terms() {
            if exponents
                .iter()
                .zip(effective.active_bits())
                .any(|(&exponent, &active)| exponent > 0 && !active)
            {
                continue;
            }
            let requested = checked_add(rows.len(), 1, "rank matrix rows")?;
            check_limit("rank matrix rows", requested, self.limits.max_rank_rows)?;
            let mut row = Vec::with_capacity(columns);
            row.extend(
                active_parameter_order
                    .iter()
                    .map(|&parameter| exponents[parameter]),
            );
            row.push(1);
            rows.push(row);
        }
        let entries = checked_mul(rows.len(), columns, "rank matrix entries")?;
        check_limit("rank matrix entries", entries, self.limits.max_rank_entries)?;
        if rows.len() > u32::MAX as usize
            || columns > u32::MAX as usize
            || entries > u32::MAX as usize
        {
            return Err(ZeroSectorError::MatrixDimensionOverflow {
                rows: rows.len(),
                columns,
            });
        }
        Ok(ExponentMatrix {
            rows,
            active_parameter_order: active_parameter_order.into_boxed_slice(),
            columns,
        })
    }
}
