//! One exact elimination of an already retained finite rowspace.
//!
//! No source seed, ordinary-source cursor, or equation provider is advanced.
//! Legacy local rows retain their generic rational-function authority. The
//! shared native kernel certifies new pivots and replays every emitted relation
//! against the complete aliased rows, including all auxiliary columns.
#[cfg(test)]
mod audit_tests;
mod authentication;
mod replay;
use replay::inherited_rows;

use super::super::{
    elimination::{Eliminated, finish_rows},
    prepare::Prepared,
};
use super::*;
pub(super) use authentication::authenticate;
use std::sync::atomic::{AtomicBool, Ordering};
use symbolica::domains::SelfRing;

#[derive(Clone, Debug)]
pub(super) struct LocalRows {
    pub binding: Option<String>,
    pub conditions: Vec<Coefficient>,
    pub rows: Vec<TerminalRelationRow>,
    pub columns: Vec<IntegralKey>,
}

#[derive(Clone, Debug)]
pub(super) struct FiniteFeedback {
    /// Apply after this many diagonal collection layers. Multiple compatible
    /// groups at one boundary retain their deterministic insertion order.
    pub after_layer: usize,
    pub local: BTreeMap<String, LocalRows>,
    pub result: Eliminated,
}

pub(super) fn compatible(a: &IntegralFamily, b: &IntegralFamily) -> bool {
    a.loop_count() == b.loop_count()
        && a.dimension() == b.dimension()
        && a.coefficient_context()
            .has_same_variable_map(b.coefficient_context())
}

pub(super) fn retained_rows(session: &TerminalRelationSession) -> Result<Vec<TerminalRelationRow>> {
    let mut rows = session.basis_rows();
    // An inventory extension may have moved the entire old U into the saved
    // rebuild queue. These are retained proved rows, not unconsumed sources.
    for row in &session.rebuild[session.rebuild_cursor..] {
        rows.push(session.normalized_row(row)?);
    }
    Ok(rows)
}

pub(super) fn session_conditions(
    session: &TerminalRelationSession,
    limits: TerminalCollectionLimits,
) -> Result<Vec<Coefficient>> {
    let mut conditions = Vec::new();
    for value in session.nonzero_conditions() {
        retain(
            &mut conditions,
            value.clone(),
            limits.diagonal.max_conditions,
        )?;
    }
    for condition in session.family_owner().domain().conditions() {
        retain(
            &mut conditions,
            condition.polynomial().clone().into(),
            limits.diagonal.max_conditions,
        )?;
    }
    for row in retained_rows(session)? {
        for value in row.values() {
            retain(
                &mut conditions,
                value.denominator.clone().into(),
                limits.diagonal.max_conditions,
            )?;
        }
    }
    Ok(conditions)
}

impl TerminalCollectionPlan {
    /// Explicit finite refinement: keep the existing diagonal source inventory,
    /// then combine it with every retained local row. This differs from
    /// `prepare` only by auxiliary cancellation and support-wide proven aliases.
    pub fn prepare_with_finite_feedback(
        sessions: &[&TerminalRelationSession],
        limits: TerminalCollectionLimits,
    ) -> Result<Self> {
        Self::prepare_with_finite_feedback_cancellable(sessions, limits, &AtomicBool::new(false))
    }

    pub fn prepare_with_finite_feedback_cancellable(
        sessions: &[&TerminalRelationSession],
        limits: TerminalCollectionLimits,
        cancel: &AtomicBool,
    ) -> Result<Self> {
        cancelled(cancel)?;
        Self::prepare(sessions, limits)?.add_finite_feedback(sessions, limits, cancel)
    }

    /// Refine without advancing a local source or provider. Old feedback layers
    /// survive rebind; one new exact elimination sees the current finite basis.
    pub fn refine_with_finite_feedback(
        &self,
        sessions: &[&TerminalRelationSession],
        limits: TerminalCollectionLimits,
    ) -> Result<Self> {
        self.refine_with_finite_feedback_cancellable(sessions, limits, &AtomicBool::new(false))
    }

    pub fn refine_with_finite_feedback_cancellable(
        &self,
        sessions: &[&TerminalRelationSession],
        limits: TerminalCollectionLimits,
        cancel: &AtomicBool,
    ) -> Result<Self> {
        cancelled(cancel)?;
        self.refine(sessions, limits)?
            .add_finite_feedback(sessions, limits, cancel)
    }

    fn add_finite_feedback(
        self,
        sessions: &[&TerminalRelationSession],
        limits: TerminalCollectionLimits,
        cancel: &AtomicBool,
    ) -> Result<Self> {
        let mut groups: Vec<Vec<&TerminalRelationSession>> = Vec::new();
        let mut ordered = sessions.to_vec();
        ordered.sort_by_key(|s| s.family_owner().fingerprint());
        for session in ordered {
            if let Some(group) = groups
                .iter_mut()
                .find(|group| compatible(group[0].family_owner(), session.family_owner()))
            {
                group.push(session);
            } else {
                groups.push(vec![session]);
            }
        }
        let mut feedbacks = self.feedbacks.clone();
        for group in groups {
            cancelled(cancel)?;
            let families: BTreeMap<_, _> = group
                .iter()
                .map(|s| {
                    (
                        s.family_owner().fingerprint().to_owned(),
                        Arc::clone(s.family_owner()),
                    )
                })
                .collect();
            let targets: BTreeSet<_> = self
                .remaining
                .iter()
                .filter(|k| families.contains_key(k.family_fingerprint()))
                .cloned()
                .collect();
            if targets.is_empty() {
                continue;
            }
            let inherited = inherited_rows(
                &families,
                &self.proofs,
                &feedbacks,
                self.layer_ends.len(),
                &self.layer_ends,
                limits,
            )?;
            let mut support: BTreeSet<_> = targets.clone();
            support.extend(inherited.0.iter().flat_map(|r| r.keys()).cloned());
            let mut local = BTreeMap::new();
            for session in &group {
                let rows = retained_rows(session)?;
                support.extend(
                    rows.iter()
                        .flat_map(|r| r.keys())
                        .map(|k| VacuumIntegralKey::from_family(session.family_owner(), k.clone())),
                );
                // Normalization identities connect original diagonal columns
                // and retained output names to the local reducer coordinates.
                support.extend(
                    session
                        .raw_terminals()
                        .iter()
                        .map(|k| VacuumIntegralKey::from_family(session.family_owner(), k.clone())),
                );
                local.insert(
                    session.family_owner().fingerprint().to_owned(),
                    LocalRows {
                        binding: session.assistance_binding().map(str::to_owned),
                        conditions: session_conditions(session, limits)?,
                        rows,
                        columns: session.columns.clone(),
                    },
                );
            }
            for session in &group {
                let family = session.family_owner();
                let context = family.coefficient_context();
                let owner = local
                    .get_mut(family.fingerprint())
                    .expect("declared family");
                for key in support
                    .iter()
                    .filter(|k| k.family_fingerprint() == family.fingerprint())
                {
                    let unit = BTreeMap::from([(key.integral().clone(), context.one())]);
                    let normalized = session.normalized_row(&unit)?;
                    let mut relation = unit;
                    for (output, factor) in normalized {
                        accumulate(
                            &mut relation,
                            output,
                            &context
                                .try_neg(&factor, limits.diagonal.algebra.exact_algebra)
                                .map_err(algebra)?,
                            context,
                            limits.diagonal,
                        )?;
                    }
                    if !relation.is_empty() {
                        owner.rows.push(relation);
                    }
                }
            }
            let feedback = FiniteFeedback::replay(
                self.layer_ends.len(),
                local,
                targets,
                &self.families,
                &self.proofs,
                &self.layer_ends,
                &feedbacks,
                limits,
                Some(cancel),
            )?;
            // Avoid recording a vacuous trial layer or claiming terminal-only
            // reinsertion as a gain. No rank or source count is fabricated.
            if feedback
                .result
                .reductions
                .values()
                .flat_map(|r| r.iter())
                .any(|(key, row)| {
                    row.terms.len() != 1
                        || row.terms.iter().next().is_some_and(|(output, value)| {
                            output.integral() != key || !value.is_one()
                        })
                })
                || feedback.result.raw.len() != feedback.result.remaining.len()
            {
                feedbacks.push(Arc::new(feedback));
            }
        }
        if feedbacks.len() == self.feedbacks.len() {
            return Ok(self);
        }
        Self::compose(
            sessions,
            self.proofs.clone(),
            self.layer_ends.clone(),
            feedbacks,
            limits,
        )
    }
}

fn cancelled(cancel: &AtomicBool) -> Result<()> {
    if cancel.load(Ordering::Relaxed) {
        Err(VacuumCollectionError::Cancelled)
    } else {
        Ok(())
    }
}
