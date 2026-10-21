//! Fresh finite searches seeded only by authenticated exact terminal maps.
//!
//! The predecessor binding transfers trusted exact-map authority; it is not an
//! ordinary-source replay certificate.  Construction deliberately starts a new
//! ordinary-source inventory and retains none of the predecessor's auxiliary U,
//! source cursor, rebuild queue, assistance state, or modular discovery state.

use super::*;
use crate::reduction::guard_union::GuardUnion;
use std::collections::HashSet;

/// One exact, guarded reduction supplied by an already authenticated artifact.
///
/// `target = sum(terms)` uses the same unit-mass coefficient convention as a
/// [`TerminalRelationSession`].  An empty right-hand side is an exact zero map;
/// an identity map is accepted but does not add a reducer row.  Construction of
/// a fresh session validates the complete exhaustive map set in its family and
/// normalization context.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProvedTerminalReduction {
    target: IntegralKey,
    terms: TerminalRelationRow,
    nonzero_conditions: Arc<Vec<Coefficient>>,
}

impl ProvedTerminalReduction {
    pub fn new(
        target: IntegralKey,
        terms: TerminalRelationRow,
        nonzero_conditions: Vec<Coefficient>,
    ) -> Self {
        Self::new_with_shared_conditions(target, terms, Arc::new(nonzero_conditions))
    }

    /// Retain one immutable guard owner across any number of flat maps.
    pub fn new_with_shared_conditions(
        target: IntegralKey,
        terms: TerminalRelationRow,
        nonzero_conditions: Arc<Vec<Coefficient>>,
    ) -> Self {
        Self {
            target,
            terms,
            nonzero_conditions,
        }
    }

    pub fn target(&self) -> &IntegralKey {
        &self.target
    }

    pub fn terms(&self) -> &TerminalRelationRow {
        &self.terms
    }

    pub fn nonzero_conditions(&self) -> &[Coefficient] {
        self.nonzero_conditions.as_slice()
    }
}

impl TerminalRelationSession {
    /// Start a new ordinary-source search from an exhaustive set of previously
    /// proved local-family terminal maps.
    ///
    /// The new session has precisely the requested signed-L1 seed depth and a
    /// zero source cursor.  Only terminal equations and their conservative guard
    /// union enter its exact reducer.  Every supplied map target must occur once
    /// in `raw`; after the new normalization, every RHS must lie in the protected
    /// terminal block.  Thus an unqualified local session cannot silently absorb
    /// a cross-family or new-master output.
    pub fn fresh_with_proved_reductions(
        family: Arc<IntegralFamily>,
        raw: BTreeSet<IntegralKey>,
        seed_depth: u32,
        authority_binding: String,
        reductions: Vec<ProvedTerminalReduction>,
        limits: TerminalRelationLimits,
    ) -> Result<Self, TerminalRelationError> {
        if authority_binding.is_empty() {
            return Err(invalid("empty retained-relations authority binding"));
        }
        Self::validate_keys(&family, &raw)?;

        let reduction_count = reductions.len();
        let mut supplied = BTreeMap::new();
        for reduction in reductions {
            if reduction.target.powers().len() != family.denominator_count()
                || !raw.contains(&reduction.target)
            {
                return Err(invalid(
                    "proved terminal reduction target is outside the raw inventory",
                ));
            }
            if supplied
                .insert(reduction.target.clone(), reduction)
                .is_some()
            {
                return Err(invalid("duplicate proved terminal reduction target"));
            }
        }
        if reduction_count != raw.len()
            || supplied.len() != raw.len()
            || supplied.keys().ne(raw.iter())
        {
            return Err(invalid(
                "proved terminal reductions do not exhaust the raw inventory",
            ));
        }

        let mut session = Self::new(Arc::clone(&family), raw, seed_depth, limits)?;
        let context = family.coefficient_context();
        let mut expected = BTreeMap::new();
        let mut admitted_condition_owners = HashSet::new();
        let mut retained_condition_owners = Vec::new();

        for (target, reduction) in supplied {
            let mut rhs = TerminalRelationRow::new();
            for (key, coefficient) in reduction.terms {
                if key.powers().len() != family.denominator_count() {
                    return Err(invalid(
                        "proved terminal reduction RHS has the wrong family arity",
                    ));
                }
                context
                    .validate_with_limits(&coefficient, limits.algebra.exact_algebra)
                    .map_err(algebra)?;
                if coefficient.is_zero() {
                    return Err(invalid(
                        "proved terminal reduction contains an explicit zero term",
                    ));
                }
                rhs.insert(key, coefficient);
            }

            // Normalize aliases before deciding whether an output is protected.
            // An unknown same-arity key would otherwise become a new auxiliary
            // column and silently enlarge the inherited master inventory.
            let rhs = session.normalized_row(&rhs)?;
            if rhs.keys().any(|key| !session.terminals.contains(key)) {
                return Err(invalid(
                    "proved terminal reduction RHS is outside the protected terminal inventory",
                ));
            }

            // Collection flat maps normally share one potentially large guard
            // owner. Keep one reference per allocation so pointer identities
            // cannot be recycled while the remaining maps are inspected.
            if admitted_condition_owners.insert(Arc::as_ptr(&reduction.nonzero_conditions)) {
                retained_condition_owners.push(Arc::clone(&reduction.nonzero_conditions));
            }
            expected.insert(target.clone(), rhs.clone());
        }

        // One insertion-ordered guard payload plus hash/index metadata. Repeated
        // conditions never clone a second coefficient, and all exact equality
        // decisions remain in the existing GuardUnion boundary.
        {
            let mut conditions = GuardUnion::try_new(&mut session.conditions).map_err(algebra)?;
            for owner in &retained_condition_owners {
                for condition in owner.iter() {
                    retain_condition(&mut conditions, context, limits.algebra, condition)?;
                }
            }
            for coefficient in expected.values().flat_map(BTreeMap::values) {
                if !coefficient.denominator.is_constant() {
                    let condition: Coefficient = coefficient.denominator.clone().into();
                    retain_condition(&mut conditions, context, limits.algebra, &condition)?;
                }
            }
            for condition in family.domain().conditions() {
                let condition: Coefficient = condition.polynomial().clone().into();
                retain_condition(&mut conditions, context, limits.algebra, &condition)?;
            }
        }

        // Preserve the predecessor's deliberately nonminimal basis. Symbolica
        // still performs all elimination; RustRed only gives eliminable targets
        // precedence over the exact outputs selected by the flat maps.
        let protected_outputs = expected
            .values()
            .flat_map(BTreeMap::keys)
            .cloned()
            .collect();
        session.retained_relations = Some(RetainedRelationsAuthority {
            binding: authority_binding,
            protected_outputs,
        });
        let mut columns = std::mem::take(&mut session.columns);
        session.sort_terminal_columns(&mut columns);
        session.column_offsets = elimination::column_offsets(&columns);
        session.reducer = SparseRowReducer::new(
            u32::try_from(columns.len()).map_err(invalid)?,
            RationalPolynomialField::new(Z),
            LuLMode::None,
        );
        session.columns = columns;

        for (target, rhs) in &expected {
            let mut equation = TerminalRelationRow::new();
            equation.insert(target.clone(), context.one());
            for (key, coefficient) in rhs {
                let coefficient = context
                    .try_neg(coefficient, limits.algebra.exact_algebra)
                    .map_err(algebra)?;
                accumulate_imported(
                    &mut equation,
                    key.clone(),
                    coefficient,
                    context,
                    limits.algebra,
                )?;
            }
            if equation.is_empty() {
                continue;
            }
            let equation = session.normalized_row(&equation)?;
            if equation.keys().any(|key| !session.terminals.contains(key)) {
                return Err(invalid(
                    "proved terminal equation escaped the protected terminal inventory",
                ));
            }
            session.add_row(equation)?;
        }

        if session.columns.len() != session.terminals.len() {
            return Err(invalid(
                "proved terminal import retained a nonterminal auxiliary column",
            ));
        }
        let actual = session.apply_all_terminals()?;
        if actual != expected {
            return Err(invalid(
                "fresh terminal maps differ from the supplied proved reductions",
            ));
        }
        Ok(session)
    }

    /// Recheck that this (possibly further refined) session still implies every
    /// terminal relation and guard in its compact fresh-search authority capsule.
    ///
    /// This performs exact terminal substitution with the current native basis;
    /// it does not load or replay the predecessor's discarded auxiliary row
    /// space.  A later refinement may improve the terminal basis, so equality of
    /// the two flat output maps is not required—every retained equation must
    /// instead reduce exactly to zero in the current basis.
    pub fn validate_retained_baseline(
        &self,
        authority: &TerminalRelationSession,
    ) -> Result<(), TerminalRelationError> {
        self.validate_retained_baseline_identity(authority)?;

        let context = self.family.coefficient_context();
        let solved = self.solved_terminal_rows()?;
        for (target, rhs) in authority.apply_all_terminals()? {
            let mut equation = TerminalRelationRow::from([(target, context.one())]);
            for (key, coefficient) in rhs {
                let coefficient = context
                    .try_neg(&coefficient, self.limits.algebra.exact_algebra)
                    .map_err(algebra)?;
                accumulate_imported(
                    &mut equation,
                    key,
                    coefficient,
                    context,
                    self.limits.algebra,
                )?;
            }
            let equation = self.normalized_row(&equation)?;
            let mut residual = TerminalRelationRow::new();
            for (key, coefficient) in equation {
                let expansion = solved.get(&key).ok_or_else(|| {
                    invalid("retained terminal relation references an absent protected output")
                })?;
                for (output, factor) in expansion {
                    let value = context
                        .try_mul(&coefficient, factor, self.limits.algebra.exact_algebra)
                        .map_err(algebra)?;
                    accumulate_imported(
                        &mut residual,
                        output.clone(),
                        value,
                        context,
                        self.limits.algebra,
                    )?;
                }
            }
            if !residual.is_empty() {
                return Err(invalid(
                    "current session no longer implies a retained terminal relation",
                ));
            }
        }
        Ok(())
    }

    /// Check capsule identity and guard retention without requiring the current
    /// terminal basis to be application-ready.  This is the cold-resume gate
    /// while an exact inventory extension still has pending rebuild rows; call
    /// [`Self::validate_retained_baseline`] after that rebuild drains and before
    /// publication or application.
    pub fn validate_retained_baseline_identity(
        &self,
        authority: &TerminalRelationSession,
    ) -> Result<(), TerminalRelationError> {
        let Some(binding) = self.retained_relations_binding() else {
            return Err(invalid("current session has no retained-relations binding"));
        };
        if authority.retained_relations_binding() != Some(binding)
            || self.family.fingerprint() != authority.family.fingerprint()
            || !authority.raw.is_subset(&self.raw)
            || authority.seed_cursor != 0
            || authority.source_cursor != 0
            || !authority.rebuild.is_empty()
            || authority.assistance.is_some()
            || authority.columns.len() != authority.terminals.len()
        {
            return Err(invalid(
                "retained-relations authority capsule has incompatible identity or search state",
            ));
        }
        let mut current_conditions = HashSet::new();
        current_conditions
            .try_reserve(self.conditions.len())
            .map_err(|_| invalid("could not reserve retained guard membership"))?;
        current_conditions.extend(self.conditions.iter());
        for condition in &authority.conditions {
            if !condition.is_constant() && !current_conditions.contains(condition) {
                return Err(invalid(
                    "current session dropped a retained terminal-map guard",
                ));
            }
        }
        Ok(())
    }

    /// Compatibility spelling for callers which treat the capsule as retained
    /// relation authority rather than as the fresh-search baseline.
    pub fn validate_retained_relations(
        &self,
        authority: &TerminalRelationSession,
    ) -> Result<(), TerminalRelationError> {
        self.validate_retained_baseline(authority)
    }
}

fn retain_condition(
    union: &mut GuardUnion<'_>,
    context: &crate::algebra::CoefficientContext,
    limits: IndexedAlgebraLimits,
    condition: &Coefficient,
) -> Result<(), TerminalRelationError> {
    context
        .validate_with_limits(condition, limits.exact_algebra)
        .map_err(algebra)?;
    if condition.is_zero() {
        return Err(invalid("proved terminal reduction has a vanishing guard"));
    }
    if !condition.is_constant() {
        union
            .extend(std::slice::from_ref(condition))
            .map_err(algebra)?;
    }
    Ok(())
}

fn accumulate_imported(
    row: &mut TerminalRelationRow,
    key: IntegralKey,
    value: Coefficient,
    context: &crate::algebra::CoefficientContext,
    limits: IndexedAlgebraLimits,
) -> Result<(), TerminalRelationError> {
    let old = row.remove(&key).unwrap_or_else(|| context.zero());
    let value = context
        .try_add(&old, &value, limits.exact_algebra)
        .map_err(algebra)?;
    if !value.is_zero() {
        row.insert(key, value);
    }
    Ok(())
}

#[cfg(test)]
mod tests;
