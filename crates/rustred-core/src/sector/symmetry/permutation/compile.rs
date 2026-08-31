use crate::family::IntegralFamily;

use super::super::{DenominatorAction, Jacobian, VerifiedMap};
use super::{Error, Verified};

/// Compile one verified affine self-map into a restriction-independent family
/// permutation.
pub fn compile(family: &IntegralFamily, affine: VerifiedMap) -> Result<Verified, Error> {
    let family_fingerprint = family.fingerprint_owner();
    if affine.source_family_fingerprint() != family_fingerprint.as_str()
        || affine.target_family_fingerprint() != family_fingerprint.as_str()
    {
        return Err(Error::ForeignFamily);
    }
    if !matches!(affine.jacobian(), Jacobian::Unit { .. }) {
        return Err(Error::UnsupportedJacobian);
    }

    let denominator_count = family.denominator_count();
    let mut source_for_target = Vec::new();
    source_for_target
        .try_reserve_exact(denominator_count)
        .map_err(|_| Error::AllocationFailure {
            resource: "inverse-permutation entries",
            requested: denominator_count,
        })?;
    source_for_target.resize(denominator_count, None);

    let one = family.coefficient_context().one();
    for (source, action) in affine.row_actions().iter().enumerate() {
        let DenominatorAction::Monomial { target, scale } = action else {
            return Err(Error::NonMonomial { source });
        };
        if scale != &one {
            return Err(Error::NonUnitScale {
                source,
                target: *target,
            });
        }
        let Some(slot) = source_for_target.get_mut(*target) else {
            return Err(Error::NonBijective { target: *target });
        };
        if slot.replace(source).is_some() {
            return Err(Error::NonBijective { target: *target });
        }
    }

    for (target, source) in source_for_target.iter().enumerate() {
        let Some(source) = *source else {
            return Err(Error::NonBijective { target });
        };
        if family.power_shifts()[source] != family.power_shifts()[target] {
            return Err(Error::PowerShiftMismatch { source, target });
        }
    }

    let mut inverse = Vec::new();
    inverse
        .try_reserve_exact(denominator_count)
        .map_err(|_| Error::AllocationFailure {
            resource: "verified inverse-permutation entries",
            requested: denominator_count,
        })?;
    inverse.extend(source_for_target.into_iter().flatten());

    Ok(Verified {
        family_fingerprint,
        source_for_target: inverse.into_boxed_slice(),
    })
}
