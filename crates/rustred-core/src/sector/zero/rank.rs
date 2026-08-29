use symbolica::prelude::Integer;

use crate::algebra::matrix::{RightKernelDecision, first_primitive_right_kernel};
use crate::sector::Mask;

use super::analysis::Analyzer;
use super::error::Error;

#[derive(Debug)]
pub(super) enum EffectiveRankDecision {
    Zero {
        active_parameter_order: Box<[usize]>,
        primitive_kernel: Box<[Integer]>,
        rank: usize,
        exponent_row_count: usize,
    },
    Full {
        active_parameter_order: Box<[usize]>,
        rank: usize,
        exponent_row_count: usize,
        column_count: usize,
    },
}

pub(super) struct ExponentMatrix {
    pub(super) entries: Box<[u16]>,
    pub(super) rows: usize,
    pub(super) active_parameter_order: Box<[usize]>,
    pub(super) columns: usize,
}

impl Analyzer {
    pub(super) fn compute_effective(
        &self,
        effective: &Mask,
    ) -> Result<EffectiveRankDecision, Error> {
        let matrix = self.exponent_matrix(effective)?;
        let decision = first_primitive_right_kernel(
            &matrix.entries,
            matrix.rows,
            matrix.columns,
            self.limits.right_kernel(),
        )
        .map_err(Error::from_right_kernel)?;

        Ok(match decision {
            RightKernelDecision::Deficient {
                rank,
                primitive_kernel,
            } => EffectiveRankDecision::Zero {
                active_parameter_order: matrix.active_parameter_order,
                primitive_kernel,
                rank,
                exponent_row_count: matrix.rows,
            },
            RightKernelDecision::FullColumnRank { rank } => EffectiveRankDecision::Full {
                active_parameter_order: matrix.active_parameter_order,
                rank,
                exponent_row_count: matrix.rows,
                column_count: matrix.columns,
            },
        })
    }
}
