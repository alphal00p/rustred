//! Optional endpoint locality, not source-row locality or proof authority.
//! Whole coefficient-weighted columns cancel through the existing projector.
use super::{Result, limit, project, require};
use rustred::identity::IndexShift;
use serde_json::{Value, json};
use std::collections::BTreeSet;

const FIELD: &str = "forbid_endpoint_changes_on_axes";

pub(super) struct Policy {
    axes: Vec<usize>,
    arity: usize,
}

#[derive(Clone, Copy)]
pub(super) struct Counts {
    universe: usize,
    matched: usize,
    added: usize,
}

impl Policy {
    pub(super) fn parse(request: &Value, arity: usize) -> Result<Self> {
        let Some(value) = request.get(FIELD) else {
            return Ok(Self {
                axes: Vec::new(),
                arity,
            });
        };
        let values = value
            .as_array()
            .ok_or("endpoint locality axes must be an array")?;
        require(values.len() <= arity, "too many endpoint locality axes")?;
        let mut axes = Vec::with_capacity(values.len());
        for value in values {
            let axis = value
                .as_u64()
                .and_then(|v| usize::try_from(v).ok())
                .ok_or("endpoint locality axis must be a nonnegative integer")?;
            require(axis < arity, "endpoint locality axis outside arity")?;
            require(
                axes.last().is_none_or(|&old| old < axis),
                "endpoint locality axes must be sorted and unique",
            )?;
            axes.push(axis);
        }
        Ok(Self { axes, arity })
    }

    fn admit_scan(&self, request: &Value, columns: usize) -> Result<()> {
        require(
            columns <= limit(request, "max_augmented_columns")?,
            "endpoint locality universe exceeds column allowance",
        )?;
        let operations = columns
            .checked_mul(self.axes.len())
            .ok_or("endpoint locality inspection overflow")?;
        require(
            operations <= limit(request, "max_term_operations")?,
            "endpoint locality inspections exceed operation allowance",
        )
    }

    fn changes(&self, shift: &IndexShift) -> Result<bool> {
        require(
            shift.values().len() == self.arity,
            "endpoint locality shift arity differs",
        )?;
        Ok(self.axes.iter().any(|&axis| shift.values()[axis] != 0))
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
        if self.axes.is_empty() {
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
        };
        for shift in universe {
            if self.changes(shift)? {
                counts.matched += 1;
                counts.added += usize::from(forbidden.insert(shift.clone()));
            }
        }
        Ok(Some(counts))
    }

    /// Redundant check AFTER exact original-source composition/replay. No
    /// coefficient displays, native guards or forbidden columns are discarded.
    pub(super) fn verify_image(&self, request: &Value, image: &project::Row) -> Result<()> {
        if self.axes.is_empty() {
            return Ok(());
        }
        self.admit_scan(request, image.len())?;
        for (shift, coefficient) in image {
            require(
                !self.changes(shift)? || coefficient.is_zero(),
                "replayed endpoint changes a preserved axis",
            )?;
        }
        Ok(())
    }

    pub(super) fn annotate(&self, mut report: Value, counts: Option<Counts>) -> Value {
        if let Some(counts) = counts {
            report["endpoint_locality"] = json!({
                "axes":self.axes, "full_post_fixed_universe_columns":counts.universe,
                "forbidden_columns":counts.matched, "new_forbidden_columns":counts.added,
                "scope":"Whole weighted endpoint coefficients vanish for every shift changing a listed axis; sufficient search restriction, not source-row filtering or rule authority.",
            });
        }
        report
    }
}
