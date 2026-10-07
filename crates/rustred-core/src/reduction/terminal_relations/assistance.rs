//! Finite, checkpointed orchestration of externally authorized exact rows.
//! The provider's authority/provenance is the caller's responsibility; the
//! binding identifies that fixed provider across checkpoints and extensions.
use super::*;

pub(super) struct Assistance {
    pub binding: String,
    pub pending: BTreeSet<IntegralKey>,
    pub queried: BTreeSet<IntegralKey>,
    pub equations: Vec<TerminalEquation>,
    pub equation_cursor: usize,
    pub completed_rows: u64,
}

impl Assistance {
    pub fn is_complete(&self) -> bool {
        self.pending.is_empty() && self.equation_cursor == self.equations.len()
    }
}

impl TerminalRelationSession {
    /// Bind a fixed equation provider before the first ordinary source row.
    /// Calling this again with the same binding is the supported resume path.
    /// Old unassisted checkpoints remain readable; already processed ordinary
    /// support cannot be reconstructed from its reduced basis, so enabling a
    /// provider on such a checkpoint requires a fresh finite session.
    pub fn enable_assistance(&mut self, binding: String) -> Result<(), TerminalRelationError> {
        if binding.is_empty() {
            return Err(invalid("empty equation-provider binding"));
        }
        if let Some(assistance) = &self.assistance {
            return if assistance.binding == binding {
                Ok(())
            } else {
                Err(invalid("equation-provider binding changed"))
            };
        }
        if self.seed_cursor != 0 || self.source_cursor != 0 || !self.rebuild.is_empty() {
            return Err(invalid(
                "enable equation assistance before processing ordinary sources",
            ));
        }
        let pending: BTreeSet<_> = self
            .raw
            .iter()
            .chain(self.normalization.canonical_terminals())
            .chain(&self.seeds)
            .cloned()
            .collect();
        check(
            "assistance keys",
            pending.len(),
            self.assistance_key_limit(),
        )?;
        self.assistance = Some(Assistance {
            binding,
            pending,
            queried: BTreeSet::new(),
            equations: Vec::new(),
            equation_cursor: 0,
            completed_rows: 0,
        });
        Ok(())
    }

    pub fn assistance_binding(&self) -> Option<&str> {
        self.assistance.as_ref().map(|a| a.binding.as_str())
    }

    pub(super) fn assistance_key_limit(&self) -> usize {
        self.limits
            .max_columns
            .saturating_add(self.limits.max_seeds)
            .saturating_add(self.limits.normalization.max_terminals)
    }

    pub(super) fn assistance_support<'a>(
        &self,
        keys: impl Iterator<Item = &'a IntegralKey>,
    ) -> Result<BTreeSet<IntegralKey>, TerminalRelationError> {
        let Some(assistance) = &self.assistance else {
            return Ok(BTreeSet::new());
        };
        let additions: BTreeSet<_> = keys
            .filter(|key| !assistance.queried.contains(*key) && !assistance.pending.contains(*key))
            .cloned()
            .collect();
        check(
            "assistance keys",
            assistance
                .queried
                .len()
                .saturating_add(assistance.pending.len())
                .saturating_add(additions.len()),
            self.assistance_key_limit(),
        )?;
        Ok(additions)
    }

    pub(super) fn validate_equations(
        &self,
        equations: &[TerminalEquation],
    ) -> Result<(), TerminalRelationError> {
        check(
            "pending assistance rows",
            equations.len(),
            self.limits.max_rows,
        )?;
        let context = self.family.coefficient_context();
        let mut nonzeros = 0usize;
        let mut conditions = 0usize;
        for equation in equations {
            nonzeros = nonzeros.saturating_add(equation.terms.len());
            conditions = conditions.saturating_add(equation.nonzero_conditions.len());
            check(
                "pending assistance nonzeros",
                nonzeros,
                self.limits.max_nonzeros,
            )?;
            check(
                "pending assistance conditions",
                conditions,
                self.limits.max_nonzeros,
            )?;
            for (key, value) in &equation.terms {
                if key.powers().len() != self.family.denominator_count() {
                    return Err(invalid("assistance equation arity differs from family"));
                }
                context
                    .validate_with_limits(value, self.limits.algebra.exact_algebra)
                    .map_err(algebra)?;
            }
            for condition in &equation.nonzero_conditions {
                context
                    .validate_with_limits(condition, self.limits.algebra.exact_algebra)
                    .map_err(algebra)?;
                if condition.is_zero() {
                    return Err(invalid("assistance nonzero condition vanishes"));
                }
            }
        }
        Ok(())
    }

    /// Execute one provider request, saved equation, ordinary row, or rebuild
    /// step. Requests are cached once per distinct finite-support key, including
    /// empty results. Successful batches and their row cursor are checkpointed;
    /// a provider error leaves its key pending for a deterministic retry.
    ///
    /// Providers must enumerate a deterministic finite list of homogeneous
    /// exact equations for the bound authority. Their right-hand-side keys stay
    /// auxiliary unless also encountered independently in an ordinary source.
    pub fn step_with_provider<F>(
        &mut self,
        cancel: &AtomicBool,
        mut provider: F,
    ) -> Result<TerminalRelationStats, TerminalRelationError>
    where
        F: FnMut(&IntegralKey) -> Result<Vec<TerminalEquation>, TerminalRelationError>,
    {
        let assistance = self
            .assistance
            .as_ref()
            .ok_or_else(|| invalid("equation assistance is not enabled"))?;
        if cancel.load(Ordering::Relaxed) || self.is_complete() {
            return Ok(self.statistics());
        }
        if let Some(equation) = assistance.equations.get(assistance.equation_cursor) {
            let row = self.normalized_row(&equation.terms)?;
            let conditions = equation.nonzero_conditions.clone();
            self.add_row(row)?;
            for condition in conditions {
                if !self.conditions.contains(&condition) {
                    self.conditions.push(condition);
                }
            }
            let assistance = self.assistance.as_mut().expect("enabled assistance");
            assistance.equation_cursor += 1;
            assistance.completed_rows += 1;
            if assistance.equation_cursor == assistance.equations.len() {
                assistance.equations.clear();
                assistance.equation_cursor = 0;
            }
            // Adding a row after an extension can introduce columns outside
            // the previous quotient; they must join final normalization too.
            self.column_normalized = false;
        } else if let Some(key) = assistance.pending.first().cloned() {
            let equations = provider(&key)?;
            self.validate_equations(&equations)?;
            let assistance = self.assistance.as_mut().expect("enabled assistance");
            assistance.pending.remove(&key);
            assistance.queried.insert(key);
            assistance.equations = equations;
            assistance.equation_cursor = 0;
        } else {
            return self.step_ordinary(cancel);
        }
        Ok(self.statistics())
    }
}
