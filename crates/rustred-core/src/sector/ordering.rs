use std::cmp::Ordering;
use std::fmt;
use std::sync::Arc;

use super::error::{Error, try_copy_string, try_reserve_exact};
use super::mask::Mask;

/// Stable identifier of RustRed's first deterministic integral order.
pub(super) const RUSTRED_UNSHIFTED_ORDER_V1_ID: &str = "rustred.unshifted-sector-order.v1";
#[cfg(test)]
const TEST_ONLY_DISTINCT_ORDER_ID: &str = "rustred.test-only-distinct-sector-order";

/// Persisted choice of integral-ordering semantics.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum OrderingPolicy {
    #[default]
    RustRedUnshiftedV1,
    /// Test-only distinct identity with the same arithmetic order. It exists
    /// solely to exercise exact owner-ordering rejection and cannot enter a
    /// production build or persisted artifact.
    #[cfg(test)]
    TestOnlyDistinct,
}

impl OrderingPolicy {
    pub fn try_from_stable_id(id: &str) -> Result<Self, Error> {
        match id {
            RUSTRED_UNSHIFTED_ORDER_V1_ID => Ok(Self::RustRedUnshiftedV1),
            #[cfg(test)]
            TEST_ONLY_DISTINCT_ORDER_ID => Ok(Self::TestOnlyDistinct),
            _ => Err(Error::UnknownOrderingPolicy {
                id: try_copy_string(id, "ordering policy identifier")?,
            }),
        }
    }

    pub const fn stable_id(self) -> &'static str {
        match self {
            Self::RustRedUnshiftedV1 => RUSTRED_UNSHIFTED_ORDER_V1_ID,
            #[cfg(test)]
            Self::TestOnlyDistinct => TEST_ONLY_DISTINCT_ORDER_ID,
        }
    }

    /// Build an exact, injective complexity key from unshifted indices.
    pub fn complexity_key(self, indices: &[i64]) -> Result<ComplexityKey, Error> {
        let sector = Mask::try_from_indices(indices)?;
        let mut dots = 0_u128;
        let mut numerators = 0_u128;
        let mut index_excess = Vec::new();
        try_reserve_exact(
            &mut index_excess,
            indices.len(),
            "integral complexity index excess",
        )?;
        for (&active, &index) in sector.active.iter().zip(indices) {
            let excess: u64 = if active {
                debug_assert!(index >= 1);
                (index - 1) as u64
            } else {
                index.unsigned_abs()
            };
            index_excess.push(excess);
            let target = if active { &mut dots } else { &mut numerators };
            *target = target
                .checked_add(u128::from(excess))
                .ok_or(Error::ComplexityOverflow {
                    measure: if active { "dots" } else { "numerators" },
                })?;
        }
        let corner_distance = dots
            .checked_add(numerators)
            .ok_or(Error::ComplexityOverflow {
                measure: "corner distance",
            })?;
        Ok(ComplexityKey {
            policy: self,
            arity: indices.len(),
            propagators: sector.active_count(),
            sector,
            corner_distance,
            dots,
            numerators,
            index_excess: Arc::new(index_excess),
        })
    }

    /// Compare integrals by the persisted exact key. `Less` means simpler.
    pub fn compare(self, left: &[i64], right: &[i64]) -> Result<Ordering, Error> {
        if left.len() != right.len() {
            return Err(Error::WrongArity {
                expected: left.len(),
                actual: right.len(),
            });
        }
        Ok(self.complexity_key(left)?.cmp(&self.complexity_key(right)?))
    }

    /// Prove that `target` is strictly simpler than `source` under this exact
    /// serialized policy.
    pub fn prove_strict_descent(
        self,
        source: &[i64],
        target: &[i64],
    ) -> Result<StrictDescentWitness, Error> {
        if source.len() != target.len() {
            return Err(Error::WrongArity {
                expected: source.len(),
                actual: target.len(),
            });
        }
        let source_key = self.complexity_key(source)?;
        let target_key = self.complexity_key(target)?;
        if target_key >= source_key {
            return Err(Error::NotStrictDescent);
        }
        let decisive_component = first_differing_component(&source_key, &target_key)
            .expect("strictly different keys have a first differing component");
        Ok(StrictDescentWitness {
            policy: self,
            source: source_key,
            target: target_key,
            decisive_component,
        })
    }
}

/// Exact strict total-order key. Field declaration order is the policy.
#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct ComplexityKey {
    policy: OrderingPolicy,
    arity: usize,
    propagators: usize,
    sector: Mask,
    corner_distance: u128,
    dots: u128,
    numerators: u128,
    // Retain the single fallibly reserved caller-sized buffer. Each coordinate
    // is derived from an i64 and therefore fits u64; only aggregate sums widen
    // to u128.
    index_excess: Arc<Vec<u64>>,
}

impl ComplexityKey {
    pub fn policy(&self) -> OrderingPolicy {
        self.policy
    }

    pub fn arity(&self) -> usize {
        self.arity
    }

    pub fn propagators(&self) -> usize {
        self.propagators
    }

    pub fn sector(&self) -> &Mask {
        &self.sector
    }

    pub fn corner_distance(&self) -> u128 {
        self.corner_distance
    }

    pub fn dots(&self) -> u128 {
        self.dots
    }

    pub fn numerators(&self) -> u128 {
        self.numerators
    }

    pub fn index_excess(&self) -> &[u64] {
        self.index_excess.as_slice()
    }
}

impl fmt::Display for ComplexityKey {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "{}|arity={}|propagators={}|sector={}|corner={}|dots={}|numerators={}|excess=[",
            self.policy.stable_id(),
            self.arity,
            self.propagators,
            self.sector,
            self.corner_distance,
            self.dots,
            self.numerators,
        )?;
        for (position, excess) in self.index_excess.iter().enumerate() {
            if position != 0 {
                formatter.write_str(",")?;
            }
            write!(formatter, "{excess}")?;
        }
        formatter.write_str("]")
    }
}

/// First field that proves strict descent in the named lexicographic key.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ComplexityComponent {
    Arity,
    PropagatorCount,
    SectorBit { position: usize },
    CornerDistance,
    DotPower,
    NumeratorPower,
    IndexExcess { position: usize },
}

/// Exact witness that a target key is strictly below a source key.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct StrictDescentWitness {
    policy: OrderingPolicy,
    source: ComplexityKey,
    target: ComplexityKey,
    decisive_component: ComplexityComponent,
}

impl StrictDescentWitness {
    pub fn policy(&self) -> OrderingPolicy {
        self.policy
    }

    pub fn source(&self) -> &ComplexityKey {
        &self.source
    }

    pub fn target(&self) -> &ComplexityKey {
        &self.target
    }

    pub fn decisive_component(&self) -> ComplexityComponent {
        self.decisive_component
    }

    pub fn verify(&self) -> bool {
        self.source.policy == self.policy
            && self.target.policy == self.policy
            && self.target < self.source
            && first_differing_component(&self.source, &self.target)
                == Some(self.decisive_component)
    }
}

fn first_differing_component(
    source: &ComplexityKey,
    target: &ComplexityKey,
) -> Option<ComplexityComponent> {
    if source.arity != target.arity {
        return Some(ComplexityComponent::Arity);
    }
    if source.propagators != target.propagators {
        return Some(ComplexityComponent::PropagatorCount);
    }
    if source.sector != target.sector {
        let position = source
            .sector
            .active
            .iter()
            .zip(target.sector.active.iter())
            .position(|(left, right)| left != right)
            .expect("different equal-arity sectors have a differing bit");
        return Some(ComplexityComponent::SectorBit { position });
    }
    if source.corner_distance != target.corner_distance {
        return Some(ComplexityComponent::CornerDistance);
    }
    if source.dots != target.dots {
        return Some(ComplexityComponent::DotPower);
    }
    if source.numerators != target.numerators {
        return Some(ComplexityComponent::NumeratorPower);
    }
    source
        .index_excess
        .iter()
        .zip(target.index_excess.iter())
        .position(|(left, right)| left != right)
        .map(|position| ComplexityComponent::IndexExcess { position })
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use super::OrderingPolicy;

    #[test]
    fn complexity_key_clones_share_the_fallibly_built_excess_buffer() {
        let key = OrderingPolicy::default()
            .complexity_key(&[i64::MIN, 1, i64::MAX])
            .unwrap();
        assert_eq!(
            key.index_excess(),
            &[i64::MIN.unsigned_abs(), 0, (i64::MAX - 1) as u64]
        );
        assert_eq!(key.numerators(), u128::from(i64::MIN.unsigned_abs()));
        assert_eq!(key.dots(), u128::from((i64::MAX - 1) as u64));
        let cloned = key.clone();
        assert!(Arc::ptr_eq(&key.index_excess, &cloned.index_excess));
        assert_eq!(key, cloned);
    }
}
