//! In-process source provenance and symbolic span composition; no display parsing.
use super::project::{self, Guard, Limits, Result, Row, Weights};
use rustred::{
    algebra::{IndexedAlgebraLimits, IndexedCoefficientContext},
    foundry::artifact::OriginalSourceContribution,
    identity::{IntegralShift, RowId, SelectedTranslatedSourceBatch},
};
use std::collections::BTreeSet;
use symbolica::domains::SelfRing;

/// Construction provenance, not a promise inferred from displayed weights.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum SpanProvenance {
    Ordinary,
    Weighted,
}

#[derive(Clone)]
pub struct SourceBinding {
    pub row: RowId,
    pub offset: IntegralShift,
}
pub struct Span {
    pub(super) provenance: SpanProvenance,
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
            provenance: SpanProvenance::Ordinary,
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
            provenance: SpanProvenance::Weighted,
            bindings,
            originals,
            weights,
            images,
            guards,
        })
    }

    /// Fresh certification is restricted to an ordinary identity frame or an
    /// injective selection/permutation of it. A weighted constructor cannot
    /// become eligible merely because its numeric/symbolic W resembles I.
    pub fn validate_fresh_ordinary(
        &self,
        c: &IndexedCoefficientContext,
        limits: Limits,
    ) -> Result<()> {
        project::require(
            self.provenance == SpanProvenance::Ordinary,
            "fresh certificate requires ordinary constructor provenance",
        )?;
        project::require(
            self.bindings.len() == self.originals.len() && self.images.len() == self.weights.len(),
            "ordinary frame shape differs",
        )?;
        project::bound(self.originals.len(), limits.rows, "fresh original rows")?;
        project::bound(self.guards.len(), limits.guards, "fresh original guards")?;
        let mut bindings = BTreeSet::new();
        for binding in &self.bindings {
            project::require(
                matches!(binding.row, RowId::OrdinaryIbp { .. }),
                "nonordinary source binding",
            )?;
            project::require(
                binding.offset.values().len() == c.index_count(),
                "ordinary offset arity differs",
            )?;
            project::require(
                bindings.insert((binding.row.clone(), binding.offset.clone())),
                "duplicate ordinary binding",
            )?;
        }
        // Include every original, not only the current selected frame.
        for row in &self.originals {
            for (shift, value) in row {
                project::require(
                    shift.values().len() == c.index_count(),
                    "original shift arity differs",
                )?;
                c.validate_with_limits(value, limits.arithmetic)?;
                project::require(!value.is_zero(), "explicit zero original coefficient")?;
            }
        }
        for guard in &self.guards {
            c.validate_polynomial_with_limits(&guard.polynomial, limits.arithmetic)?;
            project::require(!guard.polynomial.is_zero(), "zero original condition")?;
        }
        let mut seen = BTreeSet::new();
        for (weights, image) in self.weights.iter().zip(&self.images) {
            project::require(weights.len() == 1, "ordinary W row is not one unit binding")?;
            let (&ordinal, weight) = weights.first_key_value().expect("one entry checked");
            c.validate_with_limits(weight, limits.arithmetic)?;
            project::require(
                weight.raw().is_one() && ordinal < self.originals.len() && seen.insert(ordinal),
                "ordinary W is not an injective unit selection",
            )?;
            project::require(
                *image == self.originals[ordinal],
                "ordinary image differs from bound original",
            )?;
        }
        Ok(())
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
