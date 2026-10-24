//! Incremental native finite-field elimination and L-pattern reachability.
//! Graph traversal here contains no coefficient arithmetic.

use super::*;
use rand::{Rng, SeedableRng, rngs::StdRng};
use symbolica::domains::finite_field::{
    FiniteFieldCore, FiniteFieldElement, PrimeIteratorU64, ToFiniteField, Zp64,
};
use symbolica::domains::{Field, Ring};

pub(super) struct DiscoveryImage {
    pub(super) prime: u64,
    pub(super) sample: u64,
    /// Canonical residues, independent generic coordinates (never d=4).
    pub(super) point: Vec<u64>,
    pub(super) columns: Vec<IntegralKey>,
    /// Distance from the physical end, invariant when auxiliaries prepend.
    pub(super) column_offsets: BTreeMap<IntegralKey, u32>,
    pub(super) reducer: SparseRowReducer<Zp64>,
    /// Original source position for every accepted U row.
    pub(super) accepted_sources: Vec<usize>,
    /// Native L column references, hence accepted-U row IDs, NOT L row IDs.
    pub(super) parents: Vec<Vec<usize>>,
    pub(super) pivot_columns: Vec<u32>,
    pub(super) rows_processed: usize,
}

impl DiscoveryImage {
    pub(super) fn new(
        session: &TerminalRelationSession,
        sample: u64,
    ) -> Result<Self, TerminalRelationError> {
        let prime = PrimeIteratorU64::new(1 << 61)
            .nth(sample as usize)
            .ok_or_else(|| invalid("discovery prime inventory exhausted"))?;
        let field = Zp64::new(prime);
        let mut random = StdRng::seed_from_u64(0x6d6f64756c617200 ^ sample);
        let point = (0..session.family.coefficient_context().variables().len())
            .map(|_| random.random_range(5..prime))
            .collect();
        let columns = session.columns.clone();
        let width = u32::try_from(columns.len())
            .map_err(invalid)?
            .checked_add(1)
            .ok_or_else(|| invalid("modular sentinel column overflow"))?;
        let column_offsets = elimination::column_offsets(&columns);
        Ok(Self {
            prime,
            sample,
            point,
            columns,
            column_offsets,
            reducer: SparseRowReducer::new(width, field, LuLMode::Pattern),
            accepted_sources: Vec::new(),
            parents: Vec::new(),
            pivot_columns: Vec::new(),
            rows_processed: 0,
        })
    }

    pub(super) fn add(
        &mut self,
        source: usize,
        row: &SourceRow,
        session: &TerminalRelationSession,
    ) -> Result<Option<Option<Candidate>>, TerminalRelationError> {
        let field = Zp64::new(self.prime);
        let point: Vec<_> = self
            .point
            .iter()
            .map(|&value| field.to_element(value))
            .collect();
        let evaluate = |coefficient: &Coefficient| -> Option<FiniteFieldElement<u64>> {
            let numerator = coefficient.numerator.evaluate_with_coeff_map(
                |value| value.to_finite_field(&field),
                &point,
                &field,
            );
            let denominator = coefficient.denominator.evaluate_with_coeff_map(
                |value| value.to_finite_field(&field),
                &point,
                &field,
            );
            (!field.is_zero(&denominator)).then(|| field.div(&numerator, &denominator))
        };
        for condition in session.conditions.iter().chain(&row.conditions) {
            if evaluate(condition).is_none_or(|value| field.is_zero(&value)) {
                return Ok(None);
            }
        }
        // Evaluate before mutation, so a pole leaves the previous image usable.
        let mut evaluated = Vec::with_capacity(row.terms.len());
        for (key, coefficient) in &row.terms {
            let Some(value) = evaluate(coefficient) else {
                return Ok(None);
            };
            evaluated.push((key, value));
        }
        let mut new: Vec<_> = row
            .terms
            .keys()
            .filter(|key| !self.column_offsets.contains_key(*key))
            .cloned()
            .collect();
        elimination::sort_keys(&mut new);
        if !new.is_empty() {
            check(
                "modular columns",
                self.columns.len().saturating_add(new.len()),
                session.limits.max_columns,
                session.limits.diagnostics_enabled,
            )?;
            let width = self
                .columns
                .len()
                .checked_add(new.len())
                .and_then(|n| n.checked_add(1))
                .and_then(|n| u32::try_from(n).ok())
                .ok_or_else(|| invalid("modular column count overflow"))?;
            let offset = new.len() as u32;
            for (index, key) in new.iter().enumerate() {
                self.column_offsets
                    .insert(key.clone(), width - 2 - index as u32);
            }
            self.reducer.add_cols(&vec![0; new.len()]);
            for column in &mut self.pivot_columns {
                *column += offset;
            }
            new.append(&mut self.columns);
            self.columns = new;
            debug_assert_eq!(self.reducer.u().ncols(), width);
        }
        let width = self.columns.len() as u32;
        let mut entries: Vec<_> = evaluated
            .into_iter()
            .filter(|(_, value)| !field.is_zero(value))
            .map(|(key, value)| (width - 1 - self.column_offsets[key], value))
            .collect();
        entries.sort_by_key(|(column, _)| *column);
        let (ids, values): (Vec<_>, Vec<_>) = entries.into_iter().unzip();
        let before_l = self.reducer.l().nrows() as usize;
        let before_u = self.reducer.u().nrows() as usize;
        let pivot = self.reducer.add_row(&values, &ids);
        self.rows_processed += 1;
        let Some(pivot) = pivot else {
            return Ok(Some(None));
        };
        if self.reducer.l().nrows() as usize != before_l + 1
            || self.reducer.u().nrows() as usize != before_u + 1
        {
            return Err(invalid("unexpected native discovery L/U growth"));
        }
        let lower = self.reducer.l();
        let range = lower.row_ptrs()[before_l]..lower.row_ptrs()[before_l + 1];
        let parents: Vec<_> = lower.col_idcs()[range]
            .iter()
            .copied()
            .filter(|&parent| (parent as usize) < before_u)
            .map(|parent| parent as usize)
            .collect();
        self.parents.push(parents);
        self.accepted_sources.push(source);
        self.pivot_columns.push(pivot);
        let target = &self.columns[pivot as usize];
        // Terminal columns form the final protected block. A saved pivot in
        // that block is already a proved terminal rule; consult its structural
        // position without forming or dividing any symbolic rule coefficient.
        let already_known = session.column_offsets.get(target).is_some_and(|offset| {
            session.reducer.pivots()[session.columns.len() - 1 - *offset as usize].is_some()
        });
        if !session.terminals.contains(target) || already_known {
            return Ok(Some(None));
        }
        let mut ancestors = BTreeSet::new();
        let mut pending = vec![before_u];
        while let Some(row) = pending.pop() {
            if ancestors.insert(row) {
                pending.extend(self.parents[row].iter().copied());
            }
        }
        // Every parent has a harder pivot. The chosen exact sources therefore
        // have a witnessed independent harder prefix, even with dependent and
        // easier-pivot inputs interleaved in the original source stream.
        if ancestors
            .iter()
            .any(|&row| row != before_u && self.pivot_columns[row] >= pivot)
        {
            return Err(invalid("native discovery ancestry is not a harder prefix"));
        }
        let sources = ancestors
            .into_iter()
            .map(|row| self.accepted_sources[row])
            .collect();
        Ok(Some(Some(Candidate {
            target: target.clone(),
            sources,
            status: TerminalRelationModularProposalStatus::Pending,
        })))
    }
}

impl TerminalRelationModularSession {
    fn retain_candidate(&mut self, candidate: Candidate) {
        if !self
            .candidates
            .iter()
            .any(|old| old.target == candidate.target && old.sources == candidate.sources)
        {
            self.candidates.push(candidate);
        }
    }
    pub(super) fn discover_last(&mut self) -> Result<Option<()>, TerminalRelationError> {
        let source = self.rows.len() - 1;
        match self.image.add(source, &self.rows[source], &self.session)? {
            Some(candidate) => {
                if let Some(candidate) = candidate {
                    self.retain_candidate(candidate);
                }
                Ok(Some(()))
            }
            None => Ok(None),
        }
    }
    pub(super) fn rebuild_image(&mut self) -> Result<(), TerminalRelationError> {
        for attempt in 0..self.options.discovery_samples {
            // The initial empty image can use sample zero. Subsequent retries
            // always move to a fresh prime and independent generic point.
            let sample = if self.image.rows_processed == 0 && attempt == 0 {
                self.image.sample
            } else {
                self.image.sample + attempt as u64 + 1
            };
            let mut image = DiscoveryImage::new(&self.session, sample)?;
            let mut candidates = Vec::new();
            let mut valid = true;
            for (source, row) in self.rows.iter().enumerate() {
                match image.add(source, row, &self.session)? {
                    Some(Some(candidate)) => candidates.push(candidate),
                    Some(None) => (),
                    None => {
                        valid = false;
                        break;
                    }
                }
            }
            if valid {
                self.image = image;
                for candidate in candidates {
                    self.retain_candidate(candidate);
                }
                return Ok(());
            }
        }
        Err(invalid(
            "unlucky discovery samples exhausted; previous exact maps remain valid",
        ))
    }
}
