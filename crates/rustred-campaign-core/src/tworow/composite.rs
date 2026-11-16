//! TRICKS 1 x 8: rows of one element at K and K' combined with ONE catalogue-rule application to a remaining
//! non-descending term. Input: a [`NearMiss`] of the search (`I(K) = c_T I(T) + lower`, every non-lower term but `T`
//! cancelled by the combination). The step calls lane repairflow2's composite descent
//! (`repairflow::composite::compose_image`, on ac/repairflow since 01bc359f / f0b25bbb): every term that is not lower
//! than K is replaced, highest first, by the catalogue rule that serves it (first match, exact coefficients of the
//! caller's [`ComposeAlgebra`]), at most `opts.max_depth` replacements deep (default 1 = ONE rule), exact
//! cancellations seen, the key reappearing solved through `(1 - c)`; accepted iff every nonzero term of the COMPOSED
//! relation is strictly lower than K under `lower` (TRICKS 1: never the alias edge). The record names the rows, the
//! weights and the rule used, so the import can re-derive the relation (rows at the combination's seeds + the rule).

use std::cmp::Ordering;

use super::NearMiss;
use crate::repairflow::composite::{
    Catalogue, ComposeAlgebra, ComposeOptions, Composite, Refusal, compose_image,
};

/// The outcome of the composite step at one key.
#[derive(Clone, Debug)]
pub enum CompositeStep<E> {
    /// The composed relation descends: the near miss it came from and the composite (rhs, replacements).
    Accepted {
        near: Box<NearMiss>,
        composite: Composite<E>,
    },
    /// Every near miss was refused; the refusal of each (in order).
    Refused { refusals: Vec<(usize, Refusal)> },
    /// The search left no near miss for this key.
    NoNearMiss,
}

impl<E> CompositeStep<E> {
    pub fn word(&self) -> &'static str {
        match self {
            CompositeStep::Accepted { .. } => "accepted",
            CompositeStep::Refused { .. } => "refused",
            CompositeStep::NoNearMiss => "no_near_miss",
        }
    }
}

/// Try the near misses of one key in order (best first); the first composed relation that descends is accepted.
/// `lower` = the import's descent test (e.g. `repairflow::composite::import_lower`), `cmp` = the node comparator.
/// `Err` only on a failure of the exact backend (never guessed).
pub fn with_catalogue_rule<A: ComposeAlgebra, C: Catalogue<A>>(
    alg: &A,
    cat: &C,
    near_misses: &[NearMiss],
    lower: &dyn Fn(&[i32], &[i32]) -> bool,
    cmp: &dyn Fn(&[i32], &[i32]) -> Ordering,
    opts: &ComposeOptions,
) -> Result<CompositeStep<A::E>, String> {
    if near_misses.is_empty() {
        return Ok(CompositeStep::NoNearMiss);
    }
    let mut refusals = vec![];
    for (i, n) in near_misses.iter().enumerate() {
        let image = &n.comb.rhs;
        match compose_image(alg, cat, image, &n.comb.key, lower, cmp, opts)? {
            Ok(mut c) => {
                c.element = n.comb.rows.first().map(|r| r.spec.element).unwrap_or(0);
                return Ok(CompositeStep::Accepted {
                    near: Box::new(n.clone()),
                    composite: c,
                });
            }
            Err(r) => refusals.push((i, r)),
        }
    }
    Ok(CompositeStep::Refused { refusals })
}
