use crate::algebra::{Coefficient, CoefficientContext};

pub(super) fn identity(context: &CoefficientContext, size: usize) -> Vec<Vec<Coefficient>> {
    (0..size)
        .map(|row| {
            (0..size)
                .map(|column| {
                    if row == column {
                        context.one()
                    } else {
                        context.zero()
                    }
                })
                .collect()
        })
        .collect()
}
