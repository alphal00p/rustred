//! TRICKS 1 x 8: rows of one element at K and K' combined with ONE catalogue-rule application to a remaining
//! non-descending term. TYPED STUB: lane repairflow2's composite function (`repairflow::composite::compose_image`, the
//! catalogue replacement of non-lower image terms with exact composed coefficients) is not on `ac/repairflow` yet
//! (2026-11-15T02:30Z: uncommitted in its worktree). When it lands, the step is: for a key with no valid combination,
//! take the subsets that pass the cancellation pre-filter except for exactly ONE non-lower term T (`near misses`, a
//! by-product of the same enumeration with T moved to the end of the column order), solve for K, and hand the relation
//! `I(K) = c_T I(T) + lower` to `compose_image` with `max_depth = 1`; accept iff the composed relation strictly
//! descends (TRICKS 1). Nothing is guessed here: the stub reports that the step is not available.

/// The outcome of the composite step.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CompositeStep {
    /// The composite function it needs is not available on this branch (the reason names what is missing).
    NotAvailable { reason: String },
}

/// The composite step at one key (stub; see the module documentation).
pub fn with_catalogue_rule(key: &[i32]) -> CompositeStep {
    CompositeStep::NotAvailable {
        reason: format!(
            "key {key:?}: the composite with one catalogue rule needs lane repairflow2's composite function \
             (repairflow::composite::compose_image), not on ac/repairflow yet"
        ),
    }
}
