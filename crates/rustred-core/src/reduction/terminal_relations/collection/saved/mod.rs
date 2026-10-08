//! Persistent composition of finite per-family refinement and vacuum collection.
//!
//! The source sessions retain their original authority: they are generic exact
//! rational-function reductions, not certificates at every exceptional value
//! of dimension. New diagonal identities have independent native-source replay.
//! Neither composition nor loading upgrades an inherited equation provider.

#[cfg(test)]
mod audit_tests;
mod codec;
mod feedback;
#[cfg(test)]
mod tests;

use super::*;
use crate::persistence::BinaryIoLimits;
use crate::reduction::terminal_relations::{TerminalRelationRow, TerminalRelationSession};

#[derive(Clone, Copy, Debug)]
pub struct TerminalCollectionLimits {
    pub diagonal: VacuumDiagonalCollectionLimits,
    /// Retained finite sessions have a larger support than diagonal discovery.
    /// These are checked retained-matrix bounds, not a peak-memory guarantee.
    pub finite_feedback: VacuumDiagonalCollectionLimits,
    /// Bound on the composed maps, independent of diagonal discovery limits.
    pub max_composed_terms: usize,
    pub max_proof_layers: usize,
}
impl Default for TerminalCollectionLimits {
    fn default() -> Self {
        Self {
            diagonal: Default::default(),
            finite_feedback: VacuumDiagonalCollectionLimits {
                max_source_rows: 1_000_000,
                max_source_terms: 16_000_000,
                max_columns: 1_000_000,
                max_reducer_nonzeros: 32_000_000,
                max_replay_operations: 32_000_000,
                max_flat_map_terms: 1_000_000,
                max_conditions: 1_000_000,
                max_coefficient_terms: 32_000_000,
                aliases: crate::reduction::terminal_normalization::VacuumFamilyAliasLimits {
                    max_terminals: 1_000_000,
                    ..Default::default()
                },
                ..Default::default()
            },
            max_composed_terms: 1_000_000,
            max_proof_layers: 64,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct TerminalCollectionStatistics {
    pub raw_terminals: usize,
    pub normalized_terminals: usize,
    pub precollection_remaining: usize,
    /// Cumulative finite input count across retained proof layers, not a
    /// current master count after repeated explicit refinements.
    pub collection_input_terminals: usize,
    /// Cumulative alias class count in the retained proof layers.
    pub global_alias_classes: usize,
    pub terminal_equations: usize,
    pub passthrough_terminals: usize,
    pub remaining_terminals: usize,
    pub collection_groups: usize,
    pub proof_layers: usize,
    /// Cumulative retained local rows in explicit finite feedback layers.
    pub finite_feedback_rows: usize,
    pub finite_feedback_columns: usize,
    pub finite_feedback_auxiliary_columns: usize,
    pub finite_feedback_aliases: usize,
    pub finite_feedback_equations: usize,
    pub finite_feedback_nonzeros: usize,
    pub finite_feedback_replay_operations: usize,
}

#[derive(Clone, Debug)]
struct Predecessor {
    maps: BTreeMap<IntegralKey, TerminalRelationRow>,
    conditions: Vec<Coefficient>,
    assistance_binding: Option<String>,
    normalized: usize,
    remaining: BTreeSet<IntegralKey>,
}

/// Immutable application layer over one or more saved finite sessions.
/// Families and integral indices always travel together. Cold loading verifies
/// the native stored maps against the supplied sessions and regenerates the
/// small diagonal collection proof once; application is a flat-map lookup.
#[derive(Clone, Debug)]
pub struct TerminalCollectionPlan {
    families: BTreeMap<String, Arc<IntegralFamily>>,
    predecessors: BTreeMap<String, Predecessor>,
    proofs: Vec<Arc<VacuumDiagonalCollectionPlan>>,
    /// Exclusive end of each ordered layer in `proofs`. Within a layer every
    /// family/key has at most one replacement; layers compose sequentially.
    layer_ends: Vec<usize>,
    feedbacks: Vec<Arc<feedback::FiniteFeedback>>,
    reductions: BTreeMap<String, BTreeMap<IntegralKey, GuardedVacuumReduction>>,
    raw: BTreeSet<VacuumIntegralKey>,
    remaining: BTreeSet<VacuumIntegralKey>,
    statistics: TerminalCollectionStatistics,
}

impl TerminalCollectionPlan {
    /// Compose local results without discovering any new identity. This is
    /// suitable for publication before the user explicitly requests refine.
    pub fn from_sessions(
        sessions: &[&TerminalRelationSession],
        limits: TerminalCollectionLimits,
    ) -> Result<Self> {
        Self::compose(sessions, Vec::new(), Vec::new(), Vec::new(), limits)
    }

    /// Discover aliases/diagonal identities only for eligible scalar vacuum
    /// keys. Numerators, external families, shifted indices and nonunit active
    /// masses pass through unchanged. Compatible families are grouped without
    /// any topology name or loop-count dispatch.
    pub fn prepare(
        sessions: &[&TerminalRelationSession],
        limits: TerminalCollectionLimits,
    ) -> Result<Self> {
        Self::validate_sessions(sessions, limits)?;
        let inputs: Vec<_> = sessions
            .iter()
            .map(|s| (Arc::clone(s.family_owner()), s.remaining_terminals()))
            .collect();
        let proofs = Self::prepare_proofs(&inputs, limits)?;
        let layer_ends = if proofs.is_empty() {
            Vec::new()
        } else {
            vec![proofs.len()]
        };
        Self::compose(sessions, proofs, layer_ends, Vec::new(), limits)
    }

    fn prepare_proofs(
        inputs: &[(Arc<IntegralFamily>, BTreeSet<IntegralKey>)],
        limits: TerminalCollectionLimits,
    ) -> Result<Vec<Arc<VacuumDiagonalCollectionPlan>>> {
        let mut groups: Vec<Vec<(Arc<IntegralFamily>, BTreeSet<IntegralKey>)>> = Vec::new();
        for (family, keys) in inputs {
            if family.external_count() != 0 || family.power_shifts().iter().any(|x| !x.is_zero()) {
                continue;
            }
            let eligible: BTreeSet<_> = keys
                .iter()
                .cloned()
                .filter(|key| {
                    key.powers().iter().enumerate().all(|(axis, &power)| {
                        power >= 0
                            && (power == 0
                                || family.denominators()[axis].constant()
                                    == &family.coefficient_context().integer(-1))
                    })
                })
                .collect();
            if eligible.is_empty() {
                continue;
            }
            if let Some(group) = groups.iter_mut().find(|group| {
                let first = &group[0].0;
                first.loop_count() == family.loop_count()
                    && first.dimension() == family.dimension()
                    && first
                        .coefficient_context()
                        .has_same_variable_map(family.coefficient_context())
            }) {
                group.push((Arc::clone(family), eligible));
            } else {
                groups.push(vec![(Arc::clone(family), eligible)]);
            }
        }
        // Deterministic grouping and provenance irrespective of supplied order.
        for group in &mut groups {
            group.sort_by(|a, b| a.0.fingerprint().cmp(b.0.fingerprint()));
        }
        groups.sort_by(|a, b| a[0].0.fingerprint().cmp(b[0].0.fingerprint()));
        groups
            .into_iter()
            .map(|group| {
                VacuumDiagonalCollectionPlan::prepare(&group, limits.diagonal).map(Arc::new)
            })
            .collect::<Result<Vec<_>>>()
    }

    /// Transport previously proved identities onto new local session maps.
    /// No new diagonal sources or aliases are generated. Unmatched new free
    /// outputs are retained, including numerators. Previously retained outputs
    /// may remain as a nonminimal basis even if the newer local solver happens
    /// to choose a different basis; this prevents cyclic mixed substitutions.
    pub fn rebind(
        &self,
        sessions: &[&TerminalRelationSession],
        limits: TerminalCollectionLimits,
    ) -> Result<Self> {
        feedback::authenticate(&self.feedbacks, sessions, limits)?;
        Self::compose(
            sessions,
            self.proofs.clone(),
            self.layer_ends.clone(),
            self.feedbacks.clone(),
            limits,
        )
    }

    /// Refine the rebound output basis while retaining every previously proved
    /// collection layer. A new layer sees the actual current flat output keys,
    /// not the obsolete local basis. Identical repeated results do not append
    /// history. This does not advance the per-family ordinary-source cursors.
    pub fn refine(
        &self,
        sessions: &[&TerminalRelationSession],
        limits: TerminalCollectionLimits,
    ) -> Result<Self> {
        let rebound = self.rebind(sessions, limits)?;
        let inputs: Vec<_> = rebound
            .families
            .values()
            .map(|family| {
                (
                    Arc::clone(family),
                    rebound
                        .remaining
                        .iter()
                        .filter(|key| key.family_fingerprint() == family.fingerprint())
                        .map(|key| key.integral().clone())
                        .collect(),
                )
            })
            .collect();
        let new = Self::prepare_proofs(&inputs, limits)?;
        if new.is_empty() {
            return Ok(rebound);
        }
        let mut proofs = rebound.proofs.clone();
        proofs.extend(new);
        let mut ends = rebound.layer_ends.clone();
        ends.push(proofs.len());
        // One trial layer is allowed for deciding a no-op at the configured
        // history boundary; a changed result must still satisfy that boundary.
        let mut trial_limits = limits;
        trial_limits.max_proof_layers = trial_limits.max_proof_layers.saturating_add(1);
        let candidate = Self::compose(
            sessions,
            proofs,
            ends,
            rebound.feedbacks.clone(),
            trial_limits,
        )?;
        let unchanged = rebound.reductions.iter().all(|(family, rows)| {
            rows.iter()
                .all(|(key, row)| candidate.reductions[family][key].terms() == row.terms())
        });
        if unchanged {
            return Ok(rebound);
        }
        check(
            "collection proof layers",
            candidate.layer_ends.len(),
            limits.max_proof_layers,
        )?;
        Ok(candidate)
    }

    fn validate_sessions(
        sessions: &[&TerminalRelationSession],
        limits: TerminalCollectionLimits,
    ) -> Result<()> {
        check(
            "collection families",
            sessions.len(),
            limits.diagonal.aliases.max_families,
        )?;
        let mut seen = BTreeSet::new();
        for session in sessions {
            if !seen.insert(session.family_owner().fingerprint()) {
                return Err(VacuumCollectionError::Alias(
                    crate::reduction::terminal_normalization::VacuumFamilyAliasError::DuplicateFamily));
            }
        }
        Ok(())
    }

    fn compose(
        sessions: &[&TerminalRelationSession],
        proofs: Vec<Arc<VacuumDiagonalCollectionPlan>>,
        layer_ends: Vec<usize>,
        feedbacks: Vec<Arc<feedback::FiniteFeedback>>,
        limits: TerminalCollectionLimits,
    ) -> Result<Self> {
        Self::validate_sessions(sessions, limits)?;
        check(
            "collection proof layers",
            layer_ends.len(),
            limits.max_proof_layers,
        )?;
        if layer_ends.last().copied().unwrap_or(0) != proofs.len()
            || layer_ends.windows(2).any(|pair| pair[0] >= pair[1])
            || layer_ends.first() == Some(&0)
        {
            return Err(VacuumCollectionError::ReplayFailed);
        }
        check(
            "finite feedback layers",
            feedbacks.len(),
            limits.max_proof_layers,
        )?;
        if feedbacks.iter().any(|f| f.after_layer > layer_ends.len())
            || feedbacks
                .windows(2)
                .any(|f| f[0].after_layer > f[1].after_layer)
        {
            return Err(VacuumCollectionError::ReplayFailed);
        }
        let families: BTreeMap<_, _> = sessions
            .iter()
            .map(|s| {
                (
                    s.family_owner().fingerprint().to_owned(),
                    Arc::clone(s.family_owner()),
                )
            })
            .collect();
        let mut inherited = Vec::new();
        let mut statistics = TerminalCollectionStatistics {
            collection_groups: proofs.len(),
            proof_layers: layer_ends.len(),
            ..Default::default()
        };
        let mut start = 0;
        for (layer_index, &end) in layer_ends.iter().enumerate() {
            let mut layer = BTreeMap::new();
            for proof in &proofs[start..end] {
                for family in proof.families() {
                    if !families.contains_key(family.fingerprint()) {
                        return Err(VacuumCollectionError::WrongFamily);
                    }
                }
                statistics.collection_input_terminals += proof.raw_terminals().len();
                statistics.global_alias_classes += proof.statistics().global_target_classes;
                statistics.terminal_equations += proof.equations().len();
                for key in proof.raw_terminals() {
                    let family = &families[key.family_fingerprint()];
                    if layer
                        .insert(key.clone(), proof.apply(family, key.integral())?)
                        .is_some()
                    {
                        return Err(VacuumCollectionError::ReplayFailed);
                    }
                }
            }
            inherited.push(layer);
            for feedback in feedbacks
                .iter()
                .filter(|f| f.after_layer == layer_index + 1)
            {
                inherited.push(feedback.layer());
            }
            start = end;
        }
        for feedback in feedbacks.iter().filter(|f| f.after_layer == 0).rev() {
            inherited.insert(0, feedback.layer());
        }
        for feedback in &feedbacks {
            statistics.finite_feedback_rows +=
                feedback.local.values().map(|s| s.rows.len()).sum::<usize>();
            statistics.finite_feedback_columns += feedback.result.statistics.columns;
            statistics.finite_feedback_auxiliary_columns +=
                feedback.result.statistics.auxiliary_columns;
            statistics.finite_feedback_aliases += feedback.result.aliases.aliases().len();
            statistics.finite_feedback_equations += feedback.result.equations.len();
            statistics.finite_feedback_nonzeros += feedback.result.statistics.reducer_nonzeros;
            statistics.finite_feedback_replay_operations +=
                feedback.result.statistics.replay_operations;
        }
        let mut raw = BTreeSet::new();
        let mut remaining = BTreeSet::new();
        let mut reductions = BTreeMap::new();
        let mut predecessors = BTreeMap::new();
        let mut total_terms = 0usize;
        for session in sessions {
            let family = session.family_owner();
            let context = family.coefficient_context();
            let local_remaining = session.remaining_terminals();
            let local = session.apply_all_terminals()?;
            let mut conditions = Vec::new();
            for condition in session.nonzero_conditions() {
                retain(
                    &mut conditions,
                    condition.clone(),
                    limits.diagonal.max_conditions,
                )?;
            }
            for condition in family.domain().conditions() {
                retain(
                    &mut conditions,
                    condition.polynomial().clone().into(),
                    limits.diagonal.max_conditions,
                )?;
            }
            // Authentication of retained feedback can use a newer local basis.
            // Its entire compatible source-domain remains part of application
            // authority, including guards from other participating families.
            if !feedbacks.is_empty() {
                for other in sessions
                    .iter()
                    .filter(|other| feedback::compatible(family, other.family_owner()))
                {
                    for condition in feedback::session_conditions(other, limits)? {
                        retain(&mut conditions, condition, limits.diagonal.max_conditions)?;
                    }
                }
            }
            // Legacy sparse sessions normalized intermediate pivots in place:
            // these retained denominators do NOT recover canceled historical
            // pivot factors. The inherited authority stays generic-d only.
            for value in session
                .reducer
                .u()
                .values()
                .iter()
                .chain(local.values().flat_map(|row| row.values()))
            {
                retain(
                    &mut conditions,
                    value.denominator.clone().into(),
                    limits.diagonal.max_conditions,
                )?;
            }
            let predecessor = Predecessor {
                maps: local,
                conditions: conditions.clone(),
                assistance_binding: session.assistance_binding().map(str::to_owned),
                normalized: session.statistics().normalized_terminals,
                remaining: local_remaining.clone(),
            };
            statistics.raw_terminals += predecessor.maps.len();
            statistics.normalized_terminals += predecessor.normalized;
            statistics.precollection_remaining += local_remaining.len();
            statistics.passthrough_terminals += local_remaining
                .iter()
                .filter(|key| {
                    !inherited.iter().any(|layer| {
                        layer.contains_key(&VacuumIntegralKey::from_family(family, (*key).clone()))
                    })
                })
                .count();
            let mut rows = BTreeMap::new();
            for (target, local_row) in &predecessor.maps {
                let mut terms: Row = local_row
                    .iter()
                    .map(|(key, value)| {
                        (
                            VacuumIntegralKey::from_family(family, key.clone()),
                            value.clone(),
                        )
                    })
                    .collect();
                let mut row_conditions = conditions.clone();
                for layer in &inherited {
                    let mut next_terms = Row::new();
                    for (tagged, coefficient) in &terms {
                        if let Some(next) = layer.get(tagged) {
                            for condition in next.nonzero_conditions() {
                                retain(
                                    &mut row_conditions,
                                    condition.clone(),
                                    limits.diagonal.max_conditions,
                                )?;
                            }
                            for (output, factor) in next.terms() {
                                let value = context
                                    .try_mul(
                                        coefficient,
                                        factor,
                                        limits.diagonal.algebra.exact_algebra,
                                    )
                                    .map_err(algebra)?;
                                accumulate(
                                    &mut next_terms,
                                    output.clone(),
                                    &value,
                                    context,
                                    limits.diagonal,
                                )?;
                            }
                        } else {
                            accumulate(
                                &mut next_terms,
                                tagged.clone(),
                                coefficient,
                                context,
                                limits.diagonal,
                            )?;
                        }
                    }
                    check(
                        "intermediate composed map terms",
                        next_terms.len(),
                        limits.max_composed_terms,
                    )?;
                    terms = next_terms;
                }
                total_terms = total_terms.saturating_add(terms.len());
                check("composed map terms", total_terms, limits.max_composed_terms)?;
                remaining.extend(terms.keys().cloned());
                raw.insert(VacuumIntegralKey::from_family(family, target.clone()));
                rows.insert(
                    target.clone(),
                    GuardedVacuumReduction {
                        terms,
                        conditions: Arc::new(row_conditions),
                    },
                );
            }
            predecessors.insert(family.fingerprint().to_owned(), predecessor);
            reductions.insert(family.fingerprint().to_owned(), rows);
        }
        // A newer local basis can conflict with retained old terminal names:
        // old A -> B and new local B -> C would otherwise expose B both as a
        // terminal and as a further reducible output. Keep the chosen finite
        // output names as identities. The stronger equations remain in the
        // predecessor/proof records; we deliberately choose a nonminimal basis
        // rather than recursively chasing mixed recurrences or creating cycles.
        for output in &remaining {
            if let Some(row) = reductions
                .get_mut(output.family_fingerprint())
                .and_then(|rows| rows.get_mut(output.integral()))
            {
                row.terms = BTreeMap::from([(
                    output.clone(),
                    families[output.family_fingerprint()]
                        .coefficient_context()
                        .one(),
                )]);
            }
        }
        remaining = reductions
            .values()
            .flat_map(|rows| rows.values())
            .flat_map(|row| row.terms.keys().cloned())
            .collect();
        statistics.remaining_terminals = remaining.len();
        Ok(Self {
            families,
            predecessors,
            proofs,
            layer_ends,
            feedbacks,
            reductions,
            raw,
            remaining,
            statistics,
        })
    }

    pub fn apply(
        &self,
        family: &IntegralFamily,
        key: &IntegralKey,
    ) -> Result<&GuardedVacuumReduction> {
        let rows = self
            .reductions
            .get(family.fingerprint())
            .ok_or(VacuumCollectionError::WrongFamily)?;
        if key.powers().len() != family.denominator_count() {
            return Err(VacuumCollectionError::WrongArity);
        }
        rows.get(key)
            .ok_or(VacuumCollectionError::UndeclaredTerminal)
    }
    pub fn families(&self) -> impl ExactSizeIterator<Item = &Arc<IntegralFamily>> {
        self.families.values()
    }
    pub fn family(&self, fingerprint: &str) -> Option<&IntegralFamily> {
        self.families.get(fingerprint).map(Arc::as_ref)
    }
    pub fn raw_terminals(&self) -> &BTreeSet<VacuumIntegralKey> {
        &self.raw
    }
    pub fn remaining_terminals(&self) -> &BTreeSet<VacuumIntegralKey> {
        &self.remaining
    }
    pub fn proofs(&self) -> &[Arc<VacuumDiagonalCollectionPlan>] {
        &self.proofs
    }
    pub fn statistics(&self) -> TerminalCollectionStatistics {
        self.statistics
    }
    pub fn common_mass_squared_power(
        &self,
        family: &IntegralFamily,
        target: &IntegralKey,
        output: &VacuumIntegralKey,
    ) -> Result<i64> {
        if !self.apply(family, target)?.terms().contains_key(output) {
            return Err(VacuumCollectionError::OutputNotInReduction);
        }
        let output_family = self
            .family(output.family_fingerprint())
            .ok_or(VacuumCollectionError::WrongFamily)?;
        for (owner, key) in [(family, target), (output_family, output.integral())] {
            if owner.external_count() != 0
                || owner.power_shifts().iter().any(|s| !s.is_zero())
                || key.powers().iter().enumerate().any(|(axis, &power)| {
                    let constant = owner.denominators()[axis].constant();
                    let unit = constant == &owner.coefficient_context().integer(-1);
                    (power > 0 && !unit) || (power < 0 && !unit && !constant.is_zero())
                })
            {
                return Err(VacuumCollectionError::Unsupported {
                    family: owner.fingerprint().to_owned(),
                    reason: "common-mass homogeneity requires an unshifted unit-mass vacuum input and output",
                });
            }
        }
        let sum = |key: &IntegralKey| key.powers().iter().map(|&p| i128::from(p)).sum::<i128>();
        i64::try_from(sum(output.integral()) - sum(target))
            .map_err(|_| VacuumCollectionError::ArithmeticOverflow)
    }
    pub fn to_native_bytes(&self, io: BinaryIoLimits) -> Result<Vec<u8>> {
        codec::encode(self, io)
    }
    pub fn from_native_bytes(
        bytes: &[u8],
        sessions: &[&TerminalRelationSession],
        limits: TerminalCollectionLimits,
        io: BinaryIoLimits,
    ) -> Result<Self> {
        codec::decode(bytes, sessions, limits, io)
    }
}
