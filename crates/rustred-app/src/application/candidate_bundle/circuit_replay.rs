//! Scoped typed observation of selected saved-rule replay circuits.
//! No new serialization, algebra, owner installation or closure authority.

use rustred::foundry::artifact::{
    ReplayCircuitLimits, ReplayedSourceCircuitBatch, SourcePortAudit, SourcePortLimits,
};
use rustred::identity::ParametricIbpGenerator;

use super::{CandidateBundleLimits, codec, preparation};
use crate::AppError;

/// Decode a trusted matching native bundle and replay only the selected rules.
/// The callback cannot return a reference to the borrowed proof/context owner.
/// Any owned data it copies is a proposal, not a durable circuit or proof seal.
/// Callback work has its own resource policy; `retention` bounds native retained
/// observations, not arbitrary callback allocation or native replay scratch.
pub fn with_replayed_candidate_rule_circuits<const N: usize, R>(
    bytes: &[u8],
    input_limits: CandidateBundleLimits,
    sector: [bool; N],
    ordinals: &[usize],
    audit_limits: SourcePortLimits,
    retention: ReplayCircuitLimits,
    visit: impl for<'audit> FnOnce(&ReplayedSourceCircuitBatch<'audit, N>) -> Result<R, AppError>,
) -> Result<R, AppError> {
    if ordinals.len() > retention.max_rules {
        return Err(AppError::limit("retained replay rule count exceeds policy"));
    }
    let bundle = codec::read(bytes, input_limits)?;
    if !rustred::fits_storage(bundle.root_sector.len(), N) {
        return Err(AppError::input("candidate circuit replay arity differs"));
    }
    let family = bundle
        .family
        .to_family(
            &bundle.coefficients,
            input_limits.family_limits(),
            input_limits.binary_limits(),
        )
        .map_err(codec::binary_error)?;
    if family.fingerprint() != bundle.family_fingerprint
        || family.denominator_count() != bundle.root_sector.len()
    {
        return Err(AppError::input(
            "candidate circuit replay family binding differs",
        ));
    }
    let prepared =
        preparation::prepare::<N>(family, &bundle.root_sector, bundle.permutation.as_deref())?;
    let context = ParametricIbpGenerator::try_new(&prepared.family)
        .map_err(|e| AppError::execution(e.to_string()))?
        .context()
        .clone();
    let solutions = codec::solutions::<N>(
        &bundle,
        &context,
        prepared.sources.index_variables(),
        input_limits,
    )?;
    let (_, solution) = solutions
        .iter()
        .find(|(s, _)| *s == sector)
        .ok_or_else(|| AppError::input("candidate circuit replay sector is absent"))?;
    let audit =
        SourcePortAudit::try_new_with_root_sector(&prepared.family, prepared.zeros, prepared.root)
            .map_err(|e| AppError::execution(e.to_string()))?
            .with_limits(audit_limits);
    let observed = audit
        .replay_sector_rule_circuits(sector, prepared.permutation, solution, ordinals, retention)
        .map_err(|e| AppError::execution(e.to_string()))?;
    visit(&observed)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{FamilyCandidatesRequest, family_candidates};

    #[test]
    fn scoped_saved_circuit_adapter_returns_owned_diagnostics_only() {
        let called = std::cell::Cell::new(false);
        let generated = family_candidates(FamilyCandidatesRequest::new(
            "I(loops(k),externals(),dimension(d),prop(D1,k^2-1,1))",
        ))
        .unwrap();
        let (family, ordinals) = with_replayed_candidate_rule_circuits::<1, _>(
            generated.bundle(),
            Default::default(),
            [true],
            &[0],
            Default::default(),
            Default::default(),
            |batch| {
                Ok((
                    batch.family_fingerprint().to_owned(),
                    batch
                        .circuits()
                        .iter()
                        .map(|r| r.ordinal())
                        .collect::<Vec<_>>(),
                ))
            },
        )
        .unwrap();
        assert!(!family.is_empty());
        assert_eq!(ordinals, [0]);
        assert!(
            with_replayed_candidate_rule_circuits::<1, _>(
                generated.bundle(),
                Default::default(),
                [true],
                &[0],
                Default::default(),
                ReplayCircuitLimits {
                    max_rules: 0,
                    ..Default::default()
                },
                |_| {
                    called.set(true);
                    Ok(())
                }
            )
            .is_err()
        );
        assert!(!called.get());
        assert!(
            with_replayed_candidate_rule_circuits::<1, _>(
                generated.bundle(),
                Default::default(),
                [true],
                &[999],
                Default::default(),
                Default::default(),
                |_| {
                    called.set(true);
                    Ok(())
                }
            )
            .is_err()
        );
        assert!(!called.get());
        assert!(
            with_replayed_candidate_rule_circuits::<2, _>(
                generated.bundle(),
                Default::default(),
                [true; 2],
                &[0],
                Default::default(),
                Default::default(),
                |_| Ok(())
            )
            .is_err()
        );
    }
}
