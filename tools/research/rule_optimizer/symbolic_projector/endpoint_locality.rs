//! Optional endpoint locality, not source-row locality or proof authority.
//! Whole coefficient-weighted columns cancel through the existing projector.
use super::{Result, limit, project, require};
use rustred::identity::IndexShift;
use serde_json::{Value, json};
use std::collections::BTreeSet;

const FIELD: &str = "forbid_endpoint_changes_on_axes";
const INCREASE_FIELD: &str = "forbid_endpoint_increases_on_axes";

pub(super) struct Policy {
    axes: Vec<usize>,
    increases: Vec<usize>,
    arity: usize,
}

#[derive(Clone, Copy)]
pub(super) struct Counts {
    universe: usize,
    matched: usize,
    added: usize,
    increases_matched: usize,
    increases_added: usize,
}

impl Policy {
    pub(super) fn parse(request: &Value, arity: usize) -> Result<Self> {
        Ok(Self {
            axes: Self::parse_axes(request, FIELD, arity)?,
            increases: Self::parse_axes(request, INCREASE_FIELD, arity)?,
            arity,
        })
    }

    fn parse_axes(request: &Value, field: &str, arity: usize) -> Result<Vec<usize>> {
        let Some(value) = request.get(field) else {
            return Ok(Vec::new());
        };
        let values = value
            .as_array()
            .ok_or_else(|| format!("{field} must be an array"))?;
        require(values.len() <= arity, &format!("{field} has too many axes"))?;
        let mut axes = Vec::with_capacity(values.len());
        for value in values {
            let axis = value
                .as_u64()
                .and_then(|v| usize::try_from(v).ok())
                .ok_or_else(|| format!("{field} axis must be a nonnegative integer"))?;
            require(axis < arity, &format!("{field} axis outside arity"))?;
            require(
                axes.last().is_none_or(|&old| old < axis),
                &format!("{field} axes must be sorted and unique"),
            )?;
            axes.push(axis);
        }
        Ok(axes)
    }

    fn enabled(&self) -> bool {
        !self.axes.is_empty() || !self.increases.is_empty()
    }

    fn admit_scan(&self, request: &Value, columns: usize) -> Result<()> {
        require(
            columns <= limit(request, "max_augmented_columns")?,
            "endpoint locality universe exceeds column allowance",
        )?;
        let operations = columns
            .checked_mul(
                self.axes
                    .len()
                    .checked_add(self.increases.len())
                    .ok_or("endpoint locality axis count overflow")?,
            )
            .ok_or("endpoint locality inspection overflow")?;
        require(
            operations <= limit(request, "max_term_operations")?,
            "endpoint locality inspections exceed operation allowance",
        )
    }

    fn violations(&self, shift: &IndexShift) -> Result<(bool, bool)> {
        require(
            shift.values().len() == self.arity,
            "endpoint locality shift arity differs",
        )?;
        Ok((
            self.axes.iter().any(|&axis| shift.values()[axis] != 0),
            self.increases.iter().any(|&axis| shift.values()[axis] > 0),
        ))
    }

    /// Call only with the complete post-fixed universe, before any row selection.
    /// Admit the worst-case additional F coordinates before cloning any shift.
    /// Existing phase limits and outer RSS/time remain the resource authority;
    /// this does not claim to bound Symbolica's internal reducer scratch.
    pub(super) fn extend_forbidden(
        &self,
        request: &Value,
        universe: &BTreeSet<IndexShift>,
        forbidden: &mut BTreeSet<IndexShift>,
    ) -> Result<Option<Counts>> {
        if !self.enabled() {
            return Ok(None);
        }
        self.admit_scan(request, universe.len())?;
        let coordinates = universe
            .len()
            .checked_mul(self.arity)
            .ok_or("endpoint locality coordinate overflow")?;
        require(
            coordinates <= limit(request, "max_coordinate_cells")?,
            "endpoint locality coordinates exceed allowance",
        )?;
        let mut counts = Counts {
            universe: universe.len(),
            matched: 0,
            added: 0,
            increases_matched: 0,
            increases_added: 0,
        };
        for shift in universe {
            let (changes, increases) = self.violations(shift)?;
            if changes {
                counts.matched += 1;
                counts.added += usize::from(forbidden.insert(shift.clone()));
            }
            if increases {
                counts.increases_matched += 1;
                counts.increases_added += usize::from(forbidden.insert(shift.clone()));
            }
        }
        Ok(Some(counts))
    }

    /// Redundant check AFTER exact original-source composition/replay. No
    /// coefficient displays, native guards or forbidden columns are discarded.
    pub(super) fn verify_image(&self, request: &Value, image: &project::Row) -> Result<()> {
        if !self.enabled() {
            return Ok(());
        }
        self.admit_scan(request, image.len())?;
        for (shift, coefficient) in image {
            let (changes, increases) = self.violations(shift)?;
            require(
                !(changes || increases) || coefficient.is_zero(),
                "replayed endpoint violates axis constraint",
            )?;
        }
        Ok(())
    }

    pub(super) fn annotate(&self, mut report: Value, counts: Option<Counts>) -> Value {
        if let Some(counts) = counts {
            if !self.axes.is_empty() {
                report["endpoint_locality"] = json!({
                    "axes":self.axes, "full_post_fixed_universe_columns":counts.universe,
                    "forbidden_columns":counts.matched, "new_forbidden_columns":counts.added,
                    "scope":"Whole weighted endpoint coefficients vanish for every shift changing a listed axis; sufficient search restriction, not source-row filtering or rule authority.",
                });
            }
            if !self.increases.is_empty() {
                report["endpoint_no_raising"] = json!({
                    "axes":self.increases,
                    "full_post_fixed_universe_columns":counts.universe,
                    "forbidden_columns":counts.increases_matched,
                    "new_forbidden_columns":counts.increases_added,
                    "scope":"Whole weighted endpoint coefficients vanish for every positive shift on a listed axis; negative shifts and pinches remain allowed. Sufficient search restriction, not source-row filtering or rule authority. New columns counted after strict locality and prior mandatory constraints.",
                });
            }
        }
        report
    }
}
