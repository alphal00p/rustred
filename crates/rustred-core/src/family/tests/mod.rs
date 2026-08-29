//! Integral-family unit tests.

use std::borrow::Cow;
use std::collections::BTreeSet;

use symbolica::prelude::Integer;

use crate::algebra::{Coefficient, CoefficientContext, ExactAlgebraError, ExactAlgebraLimits};

use super::build::retain_family_name;
use super::exact::{invert_symbolic_matrix, verify_inverse};
use super::*;

mod construction;
mod domain;
mod exact_matrix;
mod fingerprint;
mod replay;
mod validation;

fn identity_denominators(context: &CoefficientContext, size: usize) -> Vec<AffineDenominator> {
    (0..size)
        .map(|row| {
            AffineDenominator::new(
                context.zero(),
                (0..size)
                    .map(|column| {
                        if row == column {
                            context.one()
                        } else {
                            context.zero()
                        }
                    })
                    .collect(),
            )
        })
        .collect()
}

fn one_loop_family_from_basis(
    context: &CoefficientContext,
    name: &str,
    basis: Vec<Vec<Coefficient>>,
) -> Result<IntegralFamily, IntegralFamilyError> {
    let size = basis.len();
    assert!(size > 0);
    assert!(basis.iter().all(|row| row.len() == size));
    let external_count = size - 1;
    let external_momenta = (0..external_count)
        .map(|external| format!("p{external}"))
        .collect::<Vec<_>>();
    let external_gram = (0..external_count)
        .map(|row| {
            (0..external_count)
                .map(|column| {
                    if row == column {
                        context.one()
                    } else {
                        context.zero()
                    }
                })
                .collect::<Vec<_>>()
        })
        .collect::<Vec<_>>();
    let denominators = basis
        .into_iter()
        .map(|row| AffineDenominator::new(context.zero(), row))
        .collect::<Vec<_>>();

    IntegralFamily::new(
        name.to_owned(),
        vec!["k".to_owned()],
        external_momenta,
        context.clone(),
        context.parameter("d").unwrap(),
        denominators,
        external_gram,
        vec![context.zero(); size],
    )
}

fn upper_bidiagonal_basis(context: &CoefficientContext, size: usize) -> Vec<Vec<Coefficient>> {
    let x = context.parameter("x").unwrap();
    (0..size)
        .map(|row| {
            (0..size)
                .map(|column| {
                    if row == column {
                        context.integer(i64::try_from(row + 2).unwrap())
                    } else if column == row + 1 {
                        x.clone()
                    } else {
                        context.zero()
                    }
                })
                .collect()
        })
        .collect()
}
