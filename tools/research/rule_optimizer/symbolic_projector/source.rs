//! In-process source provenance and symbolic span composition; no display parsing.
use super::project::{self, Guard, Limits, Result, Row, Weights};
use rustred::{
    algebra::{IndexedAlgebraLimits, IndexedCoefficientContext},
    foundry::artifact::OriginalSourceContribution,
    identity::{IntegralShift, RowId, SelectedTranslatedSourceBatch},
};

#[derive(Clone)]
pub struct SourceBinding {
    pub row: RowId,
    pub offset: IntegralShift,
}
pub struct Span {
    pub bindings: Vec<SourceBinding>,
    pub originals: Vec<Row>,
    pub weights: Vec<Weights>,
    pub images: Vec<Row>,
    pub guards: Vec<Guard>,
}

impl Span {
    pub fn ordinary(
        c: &IndexedCoefficientContext,
        batch: &SelectedTranslatedSourceBatch,
        fixed: &[(usize, i64)],
        arithmetic: IndexedAlgebraLimits,
        limits: Limits,
    ) -> Result<Self> {
        project::bound(batch.len(), limits.rows, "original source rows")?;
        let mut bindings = Vec::new();
        let mut originals = Vec::new();
        let mut guards = Vec::new();
        for source in batch.sources() {
            let p = source.provenance();
            bindings.push(SourceBinding {
                row: p.source_row().clone(),
                offset: p.offset().clone(),
            });
            for guard in source.nonzero_conditions() {
                project::retain(
                    c,
                    &mut guards,
                    guard.polynomial().clone(),
                    "original source condition",
                    limits,
                )?;
            }
            let mut row = Row::new();
            for (shift, value) in source.terms() {
                project::denominator(
                    c,
                    &mut guards,
                    value,
                    "original source before fixed specialization",
                    limits,
                )?;
                let (value, witness) = c.specialize_fixed_indices(value, fixed, arithmetic)?;
                project::retain(
                    c,
                    &mut guards,
                    witness,
                    "original source fixed-specialization witness",
                    limits,
                )?;
                project::add_term(c, &mut row, shift.clone(), value, &mut guards, limits)?;
            }
            originals.push(row);
        }
        let weights = (0..originals.len())
            .map(|i| Weights::from([(i, c.one())]))
            .collect();
        // Identity W is constructed here, so B=A by construction; do not
        // rebuild and multiply the entire original frame once per source.
        let images = originals.clone();
        Ok(Self {
            bindings,
            originals,
            weights,
            images,
            guards,
        })
    }

    /// W is an already bound native coefficient object. It may come from a
    /// tangent or kernel nominator; all original pre-cancellation guards must
    /// accompany it. No numeric support/rank result confers broad authority.
    pub fn weighted(
        c: &IndexedCoefficientContext,
        bindings: Vec<SourceBinding>,
        originals: Vec<Row>,
        weights: Vec<Weights>,
        mut guards: Vec<Guard>,
        limits: Limits,
    ) -> Result<Self> {
        project::require(
            bindings.len() == originals.len(),
            "original provenance/frame shape differs",
        )?;
        project::bound(weights.len(), limits.rows, "weighted span rows")?;
        for (i, b) in bindings.iter().enumerate() {
            project::require(
                b.offset.values().len() == c.index_count(),
                "original offset arity differs",
            )?;
            project::require(
                !bindings[..i]
                    .iter()
                    .any(|old| old.row == b.row && old.offset == b.offset),
                "duplicate original source binding",
            )?;
        }
        // Retain all provided guards, even if an image/weight later cancels.
        for g in &guards {
            c.validate_polynomial_with_limits(&g.polynomial, limits.arithmetic)?;
            project::require(!g.polynomial.is_zero(), "zero incoming span condition")?;
        }
        project::bound(guards.len(), limits.guards, "incoming span guards")?;
        let images = project::replay_many(c, &originals, &weights, &mut guards, limits)?;
        Ok(Self {
            bindings,
            originals,
            weights,
            images,
            guards,
        })
    }

    pub fn compose(
        &self,
        c: &IndexedCoefficientContext,
        proposal: &project::Proposal,
        limits: Limits,
    ) -> Result<(Vec<OriginalSourceContribution>, Vec<Guard>)> {
        let mut guards = proposal.guards.clone();
        let mut combined = Weights::new();
        let mut operations = 0usize;
        for (&i, u) in &proposal.weights {
            project::require(i < self.weights.len(), "projected span row absent")?;
            for (&j, w) in &self.weights[i] {
                operations = operations
                    .checked_add(2)
                    .ok_or(project::Error::Budget("composition work overflow"))?;
                project::bound(operations, limits.operations, "source composition work")?;
                project::denominator(c, &mut guards, u, "u before source composition", limits)?;
                project::denominator(c, &mut guards, w, "W before source composition", limits)?;
                let value = c.mul_with_limits(u, w, limits.arithmetic)?;
                project::denominator(
                    c,
                    &mut guards,
                    &value,
                    "uW before source coalescing",
                    limits,
                )?;
                let value = if let Some(old) = combined.remove(&j) {
                    project::denominator(
                        c,
                        &mut guards,
                        &old,
                        "old source weight before coalescing",
                        limits,
                    )?;
                    c.add_with_limits(&old, &value, limits.arithmetic)?
                } else {
                    value
                };
                project::denominator(c, &mut guards, &value, "coalesced source weight", limits)?;
                if !value.is_zero() {
                    combined.insert(j, value);
                }
            }
        }
        let image = project::replay(c, &self.originals, &combined, &mut guards, limits)?;
        project::require(
            image == proposal.image,
            "composed original full product differs from projected image",
        )?;
        let contributions = combined
            .into_iter()
            .map(|(i, weight)| {
                let b = &self.bindings[i];
                OriginalSourceContribution {
                    source_row: b.row.clone(),
                    offset: b.offset.clone(),
                    weight,
                }
            })
            .collect();
        Ok((contributions, guards))
    }
}
