//! Native coefficient-map compaction, following discovery::FrameVariables.
//! Remove only globally absent variables, never substitute a variable or alter
//! integral columns. Restore the authenticated map before indexed admission.
use super::*;
use rustred::algebra::Coefficient;
use std::sync::Arc;
use symbolica::{domains::rational_polynomial::FromNumeratorAndDenominator, poly::PolyVariable};

pub(crate) struct Variables {
    original: Arc<Vec<PolyVariable>>,
    active: Arc<Vec<PolyVariable>>,
}
impl Variables {
    pub(crate) fn new(
        c: &IndexedCoefficientContext,
        rows: &[Row],
        compact: bool,
        limits: Limits,
    ) -> Result<Self> {
        let original = c.one().raw().numerator.variables().clone();
        if !compact {
            return Ok(Self {
                active: original.clone(),
                original,
            });
        }
        let mut used = vec![false; original.len()];
        for value in rows.iter().flat_map(|r| r.values()) {
            c.validate_with_limits(value, limits.arithmetic)?;
            Self::validate(value.raw(), &original)?;
            if compact {
                for (axis, present) in used.iter_mut().enumerate() {
                    if !*present {
                        *present = value.raw().numerator.contains(axis)
                            || value.raw().denominator.contains(axis);
                    }
                }
            }
        }
        let active = if used.iter().all(|x| *x) {
            original.clone()
        } else {
            Arc::new(
                original
                    .iter()
                    .zip(used)
                    .filter_map(|(v, used)| used.then(|| v.clone()))
                    .collect(),
            )
        };
        if compact {
            super::super::progress::event("coefficient_variable_compaction", || {
                serde_json::json!({
                "original_variables":original.len(),"active_variables":active.len(),
                "transport":"native injective map; guards and physical keys unchanged"})
            });
        }
        Ok(Self { original, active })
    }
    fn validate(value: &Coefficient, expected: &Arc<Vec<PolyVariable>>) -> Result<()> {
        require(
            value.numerator.variables() == expected && value.denominator.variables() == expected,
            "native coefficient variable map mismatch",
        )
    }
    fn remap(
        value: &Coefficient,
        from: &Arc<Vec<PolyVariable>>,
        to: &Arc<Vec<PolyVariable>>,
    ) -> Result<Coefficient> {
        Self::validate(value, from)?;
        if from == to {
            return Ok(value.clone());
        }
        let numerator = native(|| value.numerator.rearrange_with_growth(to))?
            .map_err(|e| Error::Invalid(format!("native numerator remap: {e}")))?;
        let denominator = native(|| value.denominator.rearrange_with_growth(to))?
            .map_err(|e| Error::Invalid(format!("native denominator remap: {e}")))?;
        // An injective variable-map change preserves coprimality. This is the
        // same native constructor/flag as production FrameVariables::remap.
        native(|| Coefficient::from_num_den(numerator, denominator, &Z, false))
    }
    pub(crate) fn map(&self, value: &Coefficient) -> Result<Coefficient> {
        Self::remap(value, &self.original, &self.active)
    }
    pub(crate) fn restore(&self, value: &Coefficient) -> Result<Coefficient> {
        Self::remap(value, &self.active, &self.original)
    }
    pub(crate) fn admit(
        &self,
        c: &IndexedCoefficientContext,
        value: &Coefficient,
        limits: Limits,
    ) -> Result<IndexedCoefficient> {
        Ok(c.admit_native_result_with_limits(self.restore(value)?, limits.arithmetic)?)
    }
    pub(crate) fn admit_values(
        &self,
        c: &IndexedCoefficientContext,
        values: &[Coefficient],
        retained_terms: &mut usize,
        limits: Limits,
    ) -> Result<Vec<IndexedCoefficient>> {
        if self.original == self.active {
            return super::admit_values(c, values, retained_terms, limits);
        }
        let mut admitted = Vec::with_capacity(values.len());
        for raw in values {
            let value = self.admit(c, raw, limits)?;
            *retained_terms = add(
                *retained_terms,
                add(raw.numerator.nterms(), raw.denominator.nterms())?,
            )?;
            bound(
                *retained_terms,
                limits.coefficient_terms,
                "retained native U/L coefficient terms",
            )?;
            admitted.push(value);
        }
        Ok(admitted)
    }
    pub(crate) fn matrix_bound(
        &self,
        c: &IndexedCoefficientContext,
        m: &Matrix,
        limits: Limits,
    ) -> Result<()> {
        if self.original == self.active {
            return super::matrix_bound(c, m, limits);
        }
        bound(m.nvalues(), limits.nonzeros, "native matrix nonzeros")?;
        let mut terms = 0;
        for raw in m.values() {
            self.admit(c, raw, limits)?;
            terms = add(
                terms,
                add(raw.numerator.nterms(), raw.denominator.nterms())?,
            )?;
            bound(
                terms,
                limits.coefficient_terms,
                "native matrix coefficient terms",
            )?;
        }
        Ok(())
    }
}

#[cfg(test)]
#[path = "native_variables_tests.rs"]
mod tests;
