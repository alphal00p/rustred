//! Shared concrete/shift comparison semantics of persisted ordering policies.

use std::cmp::Ordering;

use super::{ComplexityComponent, MAX_PACKED_ORDERING_PRIORITY_ARITY, Mask, OrderingPolicy};

impl OrderingPolicy {
    pub(crate) fn compare_degrees<T: Ord>(
        self,
        left_dots: &T,
        left_numerators: &T,
        right_dots: &T,
        right_numerators: &T,
    ) -> Ordering {
        if self.is_spired() {
            left_numerators
                .cmp(right_numerators)
                .then_with(|| left_dots.cmp(right_dots))
        } else {
            left_dots
                .cmp(right_dots)
                .then_with(|| left_numerators.cmp(right_numerators))
        }
    }

    pub(crate) fn first_differing_degree<T: Eq>(
        self,
        left_dots: &T,
        left_numerators: &T,
        right_dots: &T,
        right_numerators: &T,
    ) -> Option<ComplexityComponent> {
        if self.is_spired() && left_numerators != right_numerators {
            Some(ComplexityComponent::NumeratorPower)
        } else if left_dots != right_dots {
            Some(ComplexityComponent::DotPower)
        } else if left_numerators != right_numerators {
            Some(ComplexityComponent::NumeratorPower)
        } else {
            None
        }
    }

    /// Both vectors contain coordinate excess, not signed physical powers.
    /// SpIRed reverses excess in both sign groups. Using `reverse()` avoids
    /// negating a potentially extreme integer or unsigned magnitude.
    pub(crate) fn compare_coordinate_slices<T: Ord>(
        self,
        sector: &Mask,
        left: &[T],
        right: &[T],
    ) -> Ordering {
        let Some(slot) = self.first_differing_coordinate(sector, left, right) else {
            return Ordering::Equal;
        };
        let comparison = left[slot].cmp(&right[slot]);
        if self.is_spired() {
            comparison.reverse()
        } else {
            comparison
        }
    }

    pub(crate) fn first_differing_coordinate<T: Eq>(
        self,
        sector: &Mask,
        left: &[T],
        right: &[T],
    ) -> Option<usize> {
        debug_assert_eq!(left.len(), right.len());
        debug_assert_eq!(left.len(), sector.arity());
        if self.coordinate_priority_arity().is_some() {
            let (ranks, arity) = self.decoded_rank_by_slot();
            debug_assert_eq!(left.len(), arity);
            let mut slots = [0_u8; MAX_PACKED_ORDERING_PRIORITY_ARITY];
            for (slot, &rank) in ranks[..arity].iter().enumerate() {
                slots[usize::from(rank)] = slot as u8;
            }
            first_coordinate(
                slots[..arity].iter().copied().map(usize::from),
                self.is_spired(),
                sector,
                left,
                right,
            )
        } else {
            first_coordinate(0..left.len(), self.is_spired(), sector, left, right)
        }
    }
}

fn first_coordinate<T: Eq>(
    mut slots: impl Iterator<Item = usize> + Clone,
    spired: bool,
    sector: &Mask,
    left: &[T],
    right: &[T],
) -> Option<usize> {
    if spired {
        for active in [true, false] {
            if let Some(slot) = slots
                .clone()
                .find(|&slot| sector.active_bits()[slot] == active && left[slot] != right[slot])
            {
                return Some(slot);
            }
        }
        None
    } else {
        slots.find(|&slot| left[slot] != right[slot])
    }
}
