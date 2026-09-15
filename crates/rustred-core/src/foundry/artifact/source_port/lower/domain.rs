//! Exact proof bounds and the separate machine-integer execution carrier.
use symbolica::prelude::Integer;

use crate::algebra::indexed::IntegerZeroLocusDomainResolution;
use crate::algebra::{IndexedCoefficient, IndexedCoefficientContext, IndexedPolynomial};
use crate::foundry::completion::LatticeBox;
use crate::identity::IndexShift;
use crate::sector::{InteriorBounds, Mask, SectorMonotoneDomain};

use super::super::{SourcePortAuditError, error};

pub(super) fn runtime_domain<const N: usize>(
    piece: &LatticeBox,
    sector: &[bool; N],
    rhs: &[(IndexShift, IndexedCoefficient)],
) -> Result<SectorMonotoneDomain, SourcePortAuditError> {
    let mask = Mask::try_new(*sector).map_err(error)?;
    let shifts: Vec<_> = rhs.iter().map(|(shift, _)| shift.values()).collect();
    let carrier = SectorMonotoneDomain::try_maximal_for_rule(mask.clone(), &[0; N], &shifts)
        .map_err(error)?;
    let mut bounds = Vec::with_capacity(N);
    for axis in 0..N {
        let lower = i128::from(piece.lower()[axis]);
        let upper = piece.upper()[axis].map(i128::from);
        let (lower, upper) = if sector[axis] {
            (lower + 1, upper.map(|v| v + 1))
        } else {
            (
                upper.map(|v| -v).unwrap_or(i128::from(i64::MIN)),
                Some(-lower),
            )
        };
        let lower = lower.max(i128::from(carrier.bounds()[axis].lower()));
        let upper = upper
            .unwrap_or(i128::from(i64::MAX))
            .min(i128::from(carrier.bounds()[axis].upper()));
        if lower > upper {
            return Err(error(
                "combined proof cell has no machine-representable execution carrier",
            ));
        }
        bounds.push(InteriorBounds::new(
            i64::try_from(lower).map_err(error)?,
            i64::try_from(upper).map_err(error)?,
        ));
    }
    SectorMonotoneDomain::try_new_for_rule(mask, bounds, &[0; N], &shifts).map_err(error)
}

pub(super) fn validate_guard<const N: usize>(
    context: &IndexedCoefficientContext,
    polynomial: &IndexedPolynomial,
    piece: &LatticeBox,
    sector: &[bool; N],
) -> Result<(), SourcePortAuditError> {
    let system = context
        .base_coefficient_system(polynomial, Default::default(), Default::default())
        .map_err(error)?;
    let resolution = context
        .integer_zero_locus_domain_resolution(&system, Default::default(), |axis, root| {
            let local = if sector[axis] {
                root - &Integer::from(1)
            } else {
                -root.clone()
            };
            local >= Integer::from(piece.lower()[axis])
                && piece.upper()[axis].is_none_or(|upper| local <= Integer::from(upper))
        })
        .map_err(error)?;
    match resolution {
        IntegerZeroLocusDomainResolution::MissesDomain => Ok(()),
        _ => Err(error(
            "combined source guard is not proved nonzero over its complete unbounded cell",
        )),
    }
}
