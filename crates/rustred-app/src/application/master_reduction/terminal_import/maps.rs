//! Relations-only transport and exact containment of published terminal maps.
//! Source authority is checked by the caller; no auxiliary search state or
//! ordinary-source cursor is inherited here.
use std::collections::{BTreeMap, HashSet};
use std::sync::Arc;

use rustred::reduction::terminal_relations::{
    ProvedTerminalReduction, TerminalRelationRow, TerminalRelationSession,
};

use super::super::{AppError, MasterNormalizationProfile, MasterReductionArtifact, io};

pub(super) fn compact(
    artifact: &MasterReductionArtifact,
    binding: &str,
    normalization: MasterNormalizationProfile,
) -> Result<TerminalRelationSession, AppError> {
    if !artifact.member_sessions().is_empty() {
        return Err(AppError::input(
            "terminal proof import requires a single-family collection",
        ));
    }
    let primary = artifact.primary_session();
    let family = primary.family_owner();
    let mut expected = BTreeMap::new();
    let mut reductions = Vec::with_capacity(primary.raw_terminals().len());
    for target in primary.raw_terminals() {
        // The full collection is authoritative: primary-session maps alone can
        // omit previously collected finite relations and their guards.
        let exact = artifact.apply_terminal(family, target)?;
        let mut terms = TerminalRelationRow::new();
        for (key, coefficient) in exact.terms() {
            if key.family_fingerprint() != family.fingerprint() {
                return Err(AppError::input(
                    "terminal proof import cannot discard a cross-family output",
                ));
            }
            terms.insert(key.integral().clone(), coefficient.clone());
        }
        expected.insert(target.clone(), terms.clone());
        reductions.push(ProvedTerminalReduction::new_with_shared_conditions(
            target.clone(),
            terms,
            exact.shared_nonzero_conditions(),
        ));
    }
    let session = TerminalRelationSession::fresh_with_proved_reductions(
        Arc::clone(family),
        primary.raw_terminals().clone(),
        0,
        binding.to_owned(),
        reductions,
        normalization.relation_limits(),
    )
    .map_err(io)?;
    if session.apply_all_terminals().map_err(io)? != expected {
        return Err(AppError::input(
            "compact terminal import changed an existing published map",
        ));
    }
    Ok(session)
}

pub(super) fn flatten(
    session: &TerminalRelationSession,
    binding: &str,
    normalization: MasterNormalizationProfile,
) -> Result<TerminalRelationSession, AppError> {
    let expected = session.apply_all_terminals().map_err(io)?;
    let guards = Arc::new(session.nonzero_conditions().to_vec());
    let reductions = expected
        .iter()
        .map(|(target, terms)| {
            ProvedTerminalReduction::new_with_shared_conditions(
                target.clone(),
                terms.clone(),
                Arc::clone(&guards),
            )
        })
        .collect();
    // Native fresh import protects the supplied output basis and rejects
    // unknown outputs, duplicate/missing targets, bad coefficients or guards.
    let result = TerminalRelationSession::fresh_with_proved_reductions(
        Arc::clone(session.family_owner()),
        session.raw_terminals().clone(),
        0,
        binding.to_owned(),
        reductions,
        normalization.relation_limits(),
    )
    .map_err(io)?;
    if result.apply_all_terminals().map_err(io)? != expected {
        return Err(AppError::input(
            "flat terminal import changed the selected exact output basis",
        ));
    }
    let stats = result.statistics();
    if stats.auxiliary_columns != 0 || stats.completed_source_rows != 0 {
        return Err(AppError::input(
            "flat terminal import retained auxiliary search work",
        ));
    }
    contains(&result, session)?;
    Ok(result)
}

/// Every old guarded map must remain an exact consequence in the new basis.
/// This coalesces coefficients only through the existing Symbolica context;
/// neither approximate zero tests nor sampled dimensions establish authority.
pub(super) fn contains(
    after: &TerminalRelationSession,
    before: &TerminalRelationSession,
) -> Result<(), AppError> {
    let guards: HashSet<_> = after.nonzero_conditions().iter().collect();
    if before
        .nonzero_conditions()
        .iter()
        .any(|condition| !condition.is_constant() && !guards.contains(condition))
    {
        return Err(AppError::input(
            "terminal containment dropped a retained nonzero condition",
        ));
    }
    contains_maps(after, before)
}

/// Compare exact coefficients against an optional expected result, without
/// transferring that result's guard authority. Such a result may have acquired
/// additional conservative conditions through a different elimination history.
/// Retained baseline and flattening authority must use [`contains`] instead.
pub(super) fn contains_maps(
    after: &TerminalRelationSession,
    before: &TerminalRelationSession,
) -> Result<(), AppError> {
    if after.family_owner().fingerprint() != before.family_owner().fingerprint()
        || !before.raw_terminals().is_subset(after.raw_terminals())
    {
        return Err(AppError::input(
            "terminal containment changed family or dropped original raw keys",
        ));
    }
    let new_maps = after.apply_all_terminals().map_err(io)?;
    let remaining = after.remaining_terminals();
    if new_maps
        .values()
        .flat_map(BTreeMap::keys)
        .any(|key| !remaining.contains(key))
    {
        return Err(AppError::input(
            "terminal containment found an unresolved nonterminal output",
        ));
    }
    let context = after.family_owner().coefficient_context();
    let limits = rustred::algebra::ExactAlgebraLimits::unbounded_resources();
    for (target, rhs) in before.apply_all_terminals().map_err(io)? {
        let expected = new_maps
            .get(&target)
            .ok_or_else(|| AppError::input("terminal containment lost an original map target"))?;
        let mut composed = TerminalRelationRow::new();
        for (key, coefficient) in rhs {
            // Canonical terminal outputs need not themselves be raw targets.
            // A surviving output has its exact identity map by construction.
            let identity;
            let expansion = if let Some(map) = new_maps.get(&key) {
                map
            } else if remaining.contains(&key) {
                identity = TerminalRelationRow::from([(key.clone(), context.one())]);
                &identity
            } else {
                return Err(AppError::input(
                    "terminal containment cannot resolve a prior output key",
                ));
            };
            for (output, factor) in expansion {
                let value = context.try_mul(&coefficient, factor, limits).map_err(io)?;
                let old = composed.remove(output).unwrap_or_else(|| context.zero());
                let value = context.try_add(&old, &value, limits).map_err(io)?;
                if !value.is_zero() {
                    composed.insert(output.clone(), value);
                }
            }
        }
        if &composed != expected {
            return Err(AppError::input(
                "terminal containment no longer implies a prior exact map",
            ));
        }
    }
    Ok(())
}
