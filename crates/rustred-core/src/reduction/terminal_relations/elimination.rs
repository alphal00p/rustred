//! Column orchestration around Symbolica's sparse exact row reducer.
use super::*;
use crate::reduction::terminal_normalization::TerminalAliasPlan;

pub(super) fn sort_keys(keys: &mut [IntegralKey]) {
    keys.sort_by(|a, b| {
        OrderingPolicy::SpiredUncutV1
            .compare(b.powers(), a.powers())
            .expect("common arity validated")
    });
}

pub(super) fn column_offsets(columns: &[IntegralKey]) -> BTreeMap<IntegralKey, u32> {
    columns
        .iter()
        .rev()
        .enumerate()
        .map(|(i, key)| (key.clone(), i as u32))
        .collect()
}

impl TerminalRelationSession {
    pub(super) fn normalized_row(
        &self,
        row: &TerminalRelationRow,
    ) -> Result<TerminalRelationRow, TerminalRelationError> {
        let context = self.family.coefficient_context();
        let mut output = TerminalRelationRow::new();
        for (key, value) in row {
            if let Some(replacement) = self.normalization.terms().get(key) {
                for (target, factor) in replacement {
                    let target = self.aliases.get(target).unwrap_or(target);
                    let coefficient = context
                        .try_mul(value, factor, self.limits.algebra.exact_algebra)
                        .map_err(algebra)?;
                    accumulate(
                        &mut output,
                        target,
                        &coefficient,
                        context,
                        self.limits.algebra,
                    )?;
                }
            } else {
                let target = self.aliases.get(key).unwrap_or(key);
                accumulate(&mut output, target, value, context, self.limits.algebra)?;
            }
        }
        Ok(output)
    }

    pub(super) fn add_row(
        &mut self,
        row: TerminalRelationRow,
    ) -> Result<(), TerminalRelationError> {
        if row.is_empty() {
            return Ok(());
        }
        check(
            "independent rows",
            self.reducer.u().nrows() as usize + 1,
            self.limits.max_rows,
        )?;
        let mut new: Vec<_> = row
            .keys()
            .filter(|k| !self.column_offsets.contains_key(*k))
            .cloned()
            .collect();
        check(
            "columns",
            self.columns.len().saturating_add(new.len()),
            self.limits.max_columns,
        )?;
        let width = self
            .columns
            .len()
            .checked_add(new.len())
            .ok_or_else(|| invalid("integral column count overflow"))?;
        u32::try_from(width).map_err(invalid)?;
        // Forward elimination leaves existing U rows unchanged and appends at
        // most one row, potentially dense across the whole column inventory.
        // Admit that worst case before mutation, so every completed boundary
        // can be decoded again under the same resource limits.
        check(
            "stored nonzeros",
            self.reducer.u().nvalues().saturating_add(width),
            self.limits.max_nonzeros,
        )?;
        sort_keys(&mut new);
        if !new.is_empty() {
            for (index, key) in new.iter().enumerate() {
                self.column_offsets
                    .insert(key.clone(), (width - 1 - index) as u32);
            }
            // Inserting before all existing columns preserves their relative
            // order and keeps every newly encountered auxiliary before targets.
            self.reducer.add_cols(&vec![0; new.len()]);
            new.append(&mut self.columns);
            self.columns = new;
        }
        let mut terms: Vec<_> = row
            .into_iter()
            .map(|(k, c)| (width as u32 - 1 - self.column_offsets[&k], c))
            .collect();
        terms.sort_by_key(|(c, _)| *c);
        let ids: Vec<_> = terms.iter().map(|(i, _)| *i).collect();
        let values: Vec<_> = terms.into_iter().map(|(_, c)| c).collect();
        if self.reducer.add_row(&values, &ids).is_some() {
            let u = self.reducer.u();
            let start = u.row_ptrs()[u.nrows() as usize - 1];
            if u.col_idcs()[start..]
                .iter()
                .all(|i| self.terminals.contains(&self.columns[*i as usize]))
            {
                self.terminal_relations += 1;
            }
        }
        Ok(())
    }

    pub(super) fn basis_rows(&self) -> Vec<TerminalRelationRow> {
        let u = self.reducer.u();
        u.row_ptrs()
            .windows(2)
            .map(|p| {
                u.col_idcs()[p[0]..p[1]]
                    .iter()
                    .zip(&u.values()[p[0]..p[1]])
                    .map(|(i, c)| (self.columns[*i as usize].clone(), c.clone()))
                    .collect()
            })
            .collect()
    }

    /// Exact unit-equivalence quotient of generated auxiliary columns. Each
    /// equivalence class meeting the requested terminal block is rebound to an
    /// original terminal, even if Symbolica's canonical representative is an
    /// auxiliary. This can reveal relations without introducing any new master.
    pub(super) fn normalize_generated(&mut self) -> Result<(), TerminalRelationError> {
        let keys: BTreeSet<_> = self
            .columns
            .iter()
            .chain(self.normalization.canonical_terminals())
            .cloned()
            .collect();
        let (aliases, terminals) = self.make_aliases(&keys)?;
        let basis = self.basis_rows();
        self.alias_keys = keys;
        self.aliases = aliases;
        self.terminals = terminals;
        self.columns = self.terminals.iter().cloned().collect();
        sort_keys(&mut self.columns);
        self.column_offsets = column_offsets(&self.columns);
        self.reducer = SparseRowReducer::new(
            u32::try_from(self.columns.len()).map_err(invalid)?,
            RationalPolynomialField::new(Z),
            LuLMode::None,
        );
        self.terminal_relations = 0;
        self.rebuild = basis;
        self.rebuild_cursor = 0;
        self.column_normalized = true;
        Ok(())
    }

    pub(super) fn make_aliases(
        &self,
        keys: &BTreeSet<IntegralKey>,
    ) -> Result<(BTreeMap<IntegralKey, IntegralKey>, BTreeSet<IntegralKey>), TerminalRelationError>
    {
        let plan = TerminalAliasPlan::vacuum_parametric_equivalences(
            &self.family,
            keys,
            OrderingPolicy::SpiredUncutV1,
            self.limits.normalization.parametric,
        )
        .map_err(algebra)?;
        let mut protected: BTreeMap<IntegralKey, IntegralKey> = BTreeMap::new();
        for key in self.normalization.canonical_terminals() {
            let rep = plan.representative(&self.family, key).map_err(algebra)?;
            protected
                .entry(rep.clone())
                .and_modify(|old| {
                    if OrderingPolicy::SpiredUncutV1
                        .compare(key.powers(), old.powers())
                        .expect("checked arity")
                        .is_lt()
                    {
                        *old = key.clone();
                    }
                })
                .or_insert_with(|| key.clone());
        }
        let mut aliases = BTreeMap::new();
        for key in keys {
            let rep = plan.representative(&self.family, key).map_err(algebra)?;
            let target = protected.get(rep).unwrap_or(rep);
            if key != target {
                aliases.insert(key.clone(), target.clone());
            }
        }
        Ok((aliases, protected.into_values().collect()))
    }

    /// Every emitted row is triangular within the existing terminal block.
    /// Rows containing any auxiliary column are deliberately not substitutions.
    pub fn terminal_rules(&self) -> BTreeMap<IntegralKey, TerminalRelationRow> {
        let mut rules = BTreeMap::new();
        let u = self.reducer.u();
        for p in u.row_ptrs().windows(2) {
            if p[0] == p[1] {
                continue;
            }
            let ids = &u.col_idcs()[p[0]..p[1]];
            if !ids
                .iter()
                .all(|i| self.terminals.contains(&self.columns[*i as usize]))
            {
                continue;
            }
            let pivot = &self.columns[ids[0] as usize];
            let values = &u.values()[p[0]..p[1]];
            let rhs = ids[1..]
                .iter()
                .zip(&values[1..])
                .map(|(i, c)| (self.columns[*i as usize].clone(), -(c / &values[0])))
                .collect();
            rules.insert(pivot.clone(), rhs);
        }
        rules
    }

    pub(super) fn terminal_relation_count(&self) -> usize {
        let u = self.reducer.u();
        u.row_ptrs()
            .windows(2)
            .filter(|p| {
                p[0] < p[1]
                    && u.col_idcs()[p[0]..p[1]]
                        .iter()
                        .all(|i| self.terminals.contains(&self.columns[*i as usize]))
            })
            .count()
    }

    pub fn remaining_terminals(&self) -> BTreeSet<IntegralKey> {
        let rules = self.terminal_rules();
        self.terminals
            .iter()
            .filter(|k| !rules.contains_key(*k))
            .cloned()
            .collect()
    }

    /// Exact unit-mass substitution using currently known terminal relations.
    /// This also applies before finite search completion: unprocessed sources
    /// may improve the result, but cannot invalidate already proved rows.
    /// For a common mass m² adapters restore (m²)^(sum(output)-sum(input)); the
    /// dimension remains symbolic. No numerical catalog is required here.
    pub fn apply_terminal(
        &self,
        target: &IntegralKey,
    ) -> Result<TerminalRelationRow, TerminalRelationError> {
        if !self.raw.contains(target) {
            return Err(invalid(
                "integral is not in the original terminal inventory",
            ));
        }
        let solved = self.solved_terminal_rows()?;
        self.apply_terminal_to_solved(target, &solved)
    }

    /// Materialize all finite output maps with one triangular substitution.
    /// This is equivalent to calling `apply_terminal` for every raw key, but
    /// shares the exact solved basis and avoids quadratic repeated work.
    pub fn apply_all_terminals(
        &self,
    ) -> Result<BTreeMap<IntegralKey, TerminalRelationRow>, TerminalRelationError> {
        let solved = self.solved_terminal_rows()?;
        self.raw
            .iter()
            .map(|key| Ok((key.clone(), self.apply_terminal_to_solved(key, &solved)?)))
            .collect()
    }

    fn solved_terminal_rows(
        &self,
    ) -> Result<BTreeMap<IntegralKey, TerminalRelationRow>, TerminalRelationError> {
        if self.rebuild_cursor < self.rebuild.len() {
            return Err(invalid("terminal basis is being rebuilt"));
        }
        let rules = self.terminal_rules();
        let mut solved: BTreeMap<IntegralKey, TerminalRelationRow> = BTreeMap::new();
        let context = self.family.coefficient_context();
        for key in self
            .columns
            .iter()
            .rev()
            .filter(|k| self.terminals.contains(*k))
        {
            let mut row = TerminalRelationRow::new();
            if let Some(rhs) = rules.get(key) {
                for (child, value) in rhs {
                    let expansion = solved
                        .get(child)
                        .ok_or_else(|| invalid("nontriangular terminal row"))?;
                    for (output, factor) in expansion {
                        let c = context
                            .try_mul(value, factor, self.limits.algebra.exact_algebra)
                            .map_err(algebra)?;
                        accumulate(&mut row, output, &c, context, self.limits.algebra)?;
                    }
                }
            } else {
                row.insert(key.clone(), context.one());
            }
            solved.insert(key.clone(), row);
        }
        Ok(solved)
    }

    fn apply_terminal_to_solved(
        &self,
        target: &IntegralKey,
        solved: &BTreeMap<IntegralKey, TerminalRelationRow>,
    ) -> Result<TerminalRelationRow, TerminalRelationError> {
        let context = self.family.coefficient_context();
        let normalized = self.normalized_row(&BTreeMap::from([(target.clone(), context.one())]))?;
        let mut result = TerminalRelationRow::new();
        for (key, value) in normalized {
            for (output, factor) in solved
                .get(&key)
                .ok_or_else(|| invalid("normalized terminal is absent from basis"))?
            {
                let c = context
                    .try_mul(&value, factor, self.limits.algebra.exact_algebra)
                    .map_err(algebra)?;
                accumulate(&mut result, output, &c, context, self.limits.algebra)?;
            }
        }
        Ok(result)
    }
}

fn accumulate(
    row: &mut TerminalRelationRow,
    key: &IntegralKey,
    value: &Coefficient,
    context: &crate::algebra::CoefficientContext,
    limits: IndexedAlgebraLimits,
) -> Result<(), TerminalRelationError> {
    let old = row.remove(key).unwrap_or_else(|| context.zero());
    let value = context
        .try_add(&old, value, limits.exact_algebra)
        .map_err(algebra)?;
    if !value.is_zero() {
        row.insert(key.clone(), value);
    }
    Ok(())
}
