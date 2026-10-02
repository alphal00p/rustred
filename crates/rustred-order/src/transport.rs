use super::descriptor::{permutation, reserve};
use super::{CompiledOrder, DegreeRow, Error, Limits, OrderDescriptor};

impl CompiledOrder {
    /// Express this order in target coordinates with
    /// `target[j] = source[source_for_target[j]]`.
    ///
    /// This is an exact permutation of order data, not verification that a
    /// momentum map is a family symmetry or that an affine expansion descends.
    /// Those obligations belong to the caller's existing proof machinery.
    pub fn transport(&self, source_for_target: &[usize], limits: Limits) -> Result<Self, Error> {
        let size = self.arity();
        // Bound output before allocating a permutation inverse or any vectors.
        OrderDescriptor::dimensions(
            size,
            self.descriptor().pre_support_degree_rows.len(),
            self.descriptor().degree_rows.len(),
            limits,
        )?;
        permutation(source_for_target, size)?;
        let mut target_for_source = reserve(size)?;
        target_for_source.resize(size, 0);
        for (target, &source) in source_for_target.iter().enumerate() {
            target_for_source[source] = target;
        }
        let weights = |values: &[u64]| -> Result<Vec<u64>, Error> {
            let mut result = reserve(size)?;
            result.extend(source_for_target.iter().map(|&source| values[source]));
            Ok(result)
        };
        let priority = |values: &[usize]| -> Result<Vec<usize>, Error> {
            let mut result = reserve(size)?;
            result.extend(values.iter().map(|&source| target_for_source[source]));
            Ok(result)
        };
        let source = self.descriptor();
        let mut pre_support_degree_rows = reserve(source.pre_support_degree_rows.len())?;
        for row in &source.pre_support_degree_rows {
            pre_support_degree_rows.push(DegreeRow {
                active: weights(&row.active)?,
                inactive: weights(&row.inactive)?,
            });
        }
        let mut degree_rows = reserve(source.degree_rows.len())?;
        for row in &source.degree_rows {
            degree_rows.push(DegreeRow {
                active: weights(&row.active)?,
                inactive: weights(&row.inactive)?,
            });
        }
        Self::compile(
            OrderDescriptor {
                pre_support_degree_rows,
                support_weights: weights(&source.support_weights)?,
                support_priority: priority(&source.support_priority)?,
                degree_rows,
                coordinate_priority: priority(&source.coordinate_priority)?,
                coordinate_groups: source.coordinate_groups,
                active_direction: source.active_direction,
                inactive_direction: source.inactive_direction,
            },
            limits,
        )
    }
}
