use crate::sector::{self, Mask};

use super::analysis::Analyzer;
use super::error::Error;
use super::limits::{check_limit, checked_add, checked_mul};
use super::rank::ExponentMatrix;

impl Analyzer {
    pub(super) fn exponent_matrix(&self, effective: &Mask) -> Result<ExponentMatrix, Error> {
        if effective.arity() != self.symanzik.context().parameter_count() {
            return Err(Error::Sector(sector::Error::WrongArity {
                expected: self.symanzik.context().parameter_count(),
                actual: effective.arity(),
            }));
        }
        let active_count = effective
            .active_bits()
            .iter()
            .filter(|&&active| active)
            .count();
        let columns = checked_add(active_count, 1, "rank matrix columns")?;
        check_limit("rank matrix columns", columns, self.limits.max_rank_columns)?;
        let mut active_parameter_order = Vec::new();
        active_parameter_order
            .try_reserve_exact(active_count)
            .map_err(|_| Error::AllocationFailure {
                resource: "active parameter order",
            })?;
        active_parameter_order.extend(
            effective
                .active_bits()
                .iter()
                .enumerate()
                .filter_map(|(parameter, &active)| active.then_some(parameter)),
        );

        let mut rows = 0usize;
        let mut entries = Vec::new();
        for (_, exponents) in self.symanzik.g().terms() {
            if exponents
                .iter()
                .zip(effective.active_bits())
                .any(|(&exponent, &active)| exponent > 0 && !active)
            {
                continue;
            }
            let requested_rows = checked_add(rows, 1, "rank matrix rows")?;
            check_limit(
                "rank matrix rows",
                requested_rows,
                self.limits.max_rank_rows,
            )?;
            let requested_entries = checked_add(entries.len(), columns, "rank matrix entries")?;
            check_limit(
                "rank matrix entries",
                requested_entries,
                self.limits.max_rank_entries,
            )?;
            entries
                .try_reserve_exact(columns)
                .map_err(|_| Error::AllocationFailure {
                    resource: "rank matrix entries",
                })?;
            entries.extend(
                active_parameter_order
                    .iter()
                    .map(|&parameter| exponents[parameter]),
            );
            entries.push(1);
            rows = requested_rows;
        }

        let expected_entries = checked_mul(rows, columns, "rank matrix entries")?;
        if entries.len() != expected_entries {
            return Err(Error::MatrixShape {
                rows,
                columns,
                entries: entries.len(),
            });
        }
        if rows > u32::MAX as usize
            || columns > u32::MAX as usize
            || expected_entries > u32::MAX as usize
        {
            return Err(Error::MatrixDimensionOverflow { rows, columns });
        }
        Ok(ExponentMatrix {
            entries: entries.into_boxed_slice(),
            rows,
            active_parameter_order: active_parameter_order.into_boxed_slice(),
            columns,
        })
    }
}
