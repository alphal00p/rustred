use crate::sector::CoordinatePriority;

use super::{Canonicalizer, Error};

pub const DEFAULT_MAX_PRIORITY_ORBIT_IMAGES: usize = 1_000_000;
pub const DEFAULT_MAX_PRIORITY_QUOTIENT_PRIORITIES: usize = 1_000_000;
pub const DEFAULT_MAX_PRIORITY_RETAINED_RANK_ENTRIES: usize = 16_000_000;
pub const DEFAULT_MAX_PRIORITY_TRANSPORT_RANK_ENTRIES: usize = 64_000_000;

/// Hard bounds for exact coordinate-priority group actions and exhaustive
/// quotients. `max_transport_rank_entries` bounds arithmetic work as the
/// number of copied ranks, while `max_retained_rank_entries` bounds the
/// implementation-owned priority/permutation rank payload at peak.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CoordinatePriorityActionLimits {
    pub max_orbit_images: usize,
    pub max_quotient_priorities: usize,
    pub max_retained_rank_entries: usize,
    pub max_transport_rank_entries: usize,
}

impl Default for CoordinatePriorityActionLimits {
    fn default() -> Self {
        Self {
            max_orbit_images: DEFAULT_MAX_PRIORITY_ORBIT_IMAGES,
            max_quotient_priorities: DEFAULT_MAX_PRIORITY_QUOTIENT_PRIORITIES,
            max_retained_rank_entries: DEFAULT_MAX_PRIORITY_RETAINED_RANK_ENTRIES,
            max_transport_rank_entries: DEFAULT_MAX_PRIORITY_TRANSPORT_RANK_ENTRIES,
        }
    }
}

/// One complete, value-distinct orbit in lexicographic priority order.
#[derive(Debug, PartialEq, Eq)]
pub struct CoordinatePriorityOrbit {
    source_index: usize,
    images: Vec<CoordinatePriority>,
    group_order: usize,
}

impl CoordinatePriorityOrbit {
    pub fn source(&self) -> &CoordinatePriority {
        &self.images[self.source_index]
    }

    pub fn images(&self) -> &[CoordinatePriority] {
        &self.images
    }

    /// Lexicographically least `rank_by_slot` vector in the exact orbit.
    pub fn canonical(&self) -> &CoordinatePriority {
        self.images
            .first()
            .expect("an authenticated finite group contains its identity")
    }

    pub fn orbit_size(&self) -> usize {
        self.images.len()
    }

    pub const fn group_order(&self) -> usize {
        self.group_order
    }
}

/// The exact partition of every priority permutation at one arity by the
/// complete authenticated denominator-permutation group.
#[derive(Debug, PartialEq, Eq)]
pub struct CoordinatePriorityQuotient {
    arity: usize,
    priority_count: usize,
    group_order: usize,
    classes: Vec<CoordinatePriorityOrbit>,
}

impl CoordinatePriorityQuotient {
    pub const fn arity(&self) -> usize {
        self.arity
    }

    pub const fn priority_count(&self) -> usize {
        self.priority_count
    }

    pub const fn group_order(&self) -> usize {
        self.group_order
    }

    pub fn class_count(&self) -> usize {
        self.classes.len()
    }

    /// Classes sorted by their lexicographically least representative.
    pub fn classes(&self) -> &[CoordinatePriorityOrbit] {
        &self.classes
    }

    pub fn representatives(&self) -> impl ExactSizeIterator<Item = &CoordinatePriority> {
        self.classes.iter().map(CoordinatePriorityOrbit::canonical)
    }
}

impl Canonicalizer {
    /// Transport a priority through one exact authenticated group element.
    /// The group uses `source_for_target`, hence the returned rank obeys
    /// `out[target] = input[source_for_target[target]]`.
    pub fn transport_coordinate_priority(
        &self,
        priority: &CoordinatePriority,
        group_element: usize,
        limits: CoordinatePriorityActionLimits,
    ) -> Result<CoordinatePriority, Error> {
        self.check_priority(priority)?;
        let source_for_target =
            self.group_elements()
                .nth(group_element)
                .ok_or(Error::UnknownGroupElement {
                    ordinal: group_element,
                    group_order: self.group_order(),
                })?;
        let retained_entries = self.arity();
        admit_limit(
            "priority retained rank entries",
            retained_entries,
            limits.max_retained_rank_entries,
        )?;
        admit_limit(
            "priority transport rank entries",
            retained_entries,
            limits.max_transport_rank_entries,
        )?;
        transport(priority, source_for_target)
    }

    /// Enumerate the value-distinct orbit under the complete authenticated
    /// group and return it in stable lexicographic order.
    pub fn coordinate_priority_orbit(
        &self,
        priority: &CoordinatePriority,
        limits: CoordinatePriorityActionLimits,
    ) -> Result<CoordinatePriorityOrbit, Error> {
        self.check_priority(priority)?;
        admit_limit(
            "priority orbit images",
            self.group_order(),
            limits.max_orbit_images,
        )?;
        let retained_entries = checked_product(
            "priority orbit retained rank entries",
            self.group_order(),
            self.arity(),
        )?;
        admit_limit(
            "priority retained rank entries",
            retained_entries,
            limits.max_retained_rank_entries,
        )?;
        admit_limit(
            "priority transport rank entries",
            retained_entries,
            limits.max_transport_rank_entries,
        )?;

        let mut images = Vec::new();
        images
            .try_reserve_exact(self.group_order())
            .map_err(|_| Error::AllocationFailure {
                resource: "coordinate-priority orbit images",
                requested: self.group_order(),
            })?;
        for source_for_target in self.group_elements() {
            images.push(transport(priority, source_for_target)?);
        }
        images.sort_unstable();
        if images.windows(2).any(|pair| pair[0] == pair[1]) {
            return Err(Error::OrbitInvariant {
                detail: "distinct group elements collided on a bijective coordinate priority",
            });
        }
        if images.len() != self.group_order() {
            return Err(Error::OrbitInvariant {
                detail: "coordinate-priority orbit does not cover the authenticated group",
            });
        }
        let source_index =
            images
                .iter()
                .position(|image| image == priority)
                .ok_or(Error::OrbitInvariant {
                    detail: "coordinate-priority orbit omitted its identity image",
                })?;
        Ok(CoordinatePriorityOrbit {
            source_index,
            images,
            group_order: self.group_order(),
        })
    }

    /// Exhaustively partition all `arity!` complete priorities by the exact
    /// authenticated action. This deliberately fails before enumeration when
    /// factorial growth, retained payload, or transport work exceeds a caller
    /// limit; larger-arity portfolio heuristics belong above this proof seam.
    pub fn coordinate_priority_quotient(
        &self,
        limits: CoordinatePriorityActionLimits,
    ) -> Result<CoordinatePriorityQuotient, Error> {
        if self.arity() == 0 {
            return Err(Error::OrbitInvariant {
                detail: "a coordinate-priority quotient needs at least one slot",
            });
        }
        let priority_count = checked_factorial(self.arity())?;
        admit_limit(
            "priority quotient priorities",
            priority_count,
            limits.max_quotient_priorities,
        )?;
        admit_limit(
            "priority orbit images",
            self.group_order(),
            limits.max_orbit_images,
        )?;
        if priority_count % self.group_order() != 0 {
            return Err(Error::OrbitInvariant {
                detail: "authenticated group order does not divide the complete priority space",
            });
        }
        let expected_classes = priority_count / self.group_order();
        let retained_entries = checked_product(
            "priority quotient retained rank entries",
            priority_count,
            self.arity(),
        )?;
        let transient_entries = checked_product(
            "priority quotient transient rank entries",
            self.group_order(),
            self.arity(),
        )?;
        let cursor_entries =
            checked_product("priority quotient cursor rank entries", 2, self.arity())?;
        let peak_entries = retained_entries
            .checked_add(transient_entries)
            .and_then(|entries| entries.checked_add(cursor_entries))
            .ok_or(Error::ResourceCountOverflow {
                resource: "priority quotient peak retained rank entries",
            })?;
        admit_limit(
            "priority retained rank entries",
            peak_entries,
            limits.max_retained_rank_entries,
        )?;
        let transport_entries = checked_product(
            "priority quotient transport rank entries",
            priority_count,
            checked_product(
                "priority quotient transport group entries",
                self.group_order(),
                self.arity(),
            )?,
        )?;
        admit_limit(
            "priority transport rank entries",
            transport_entries,
            limits.max_transport_rank_entries,
        )?;

        let mut classes = Vec::new();
        classes
            .try_reserve_exact(expected_classes)
            .map_err(|_| Error::AllocationFailure {
                resource: "coordinate-priority quotient classes",
                requested: expected_classes,
            })?;
        let mut permutation = Vec::new();
        permutation
            .try_reserve_exact(self.arity())
            .map_err(|_| Error::AllocationFailure {
                resource: "coordinate-priority permutation cursor",
                requested: self.arity(),
            })?;
        permutation.extend(0..self.arity());

        loop {
            let priority = copy_validated_priority(&permutation)?;
            let orbit = self.coordinate_priority_orbit(&priority, limits)?;
            if orbit.canonical() == &priority {
                classes.push(orbit);
            }
            if !next_permutation(&mut permutation) {
                break;
            }
        }
        if classes.len() != expected_classes
            || classes
                .iter()
                .try_fold(0usize, |count, class| count.checked_add(class.orbit_size()))
                != Some(priority_count)
        {
            return Err(Error::OrbitInvariant {
                detail: "coordinate-priority quotient did not partition the complete priority space",
            });
        }
        Ok(CoordinatePriorityQuotient {
            arity: self.arity(),
            priority_count,
            group_order: self.group_order(),
            classes,
        })
    }

    fn check_priority(&self, priority: &CoordinatePriority) -> Result<(), Error> {
        if priority.arity() == self.arity() {
            Ok(())
        } else {
            Err(Error::WrongPriorityArity {
                expected: self.arity(),
                actual: priority.arity(),
            })
        }
    }
}

fn transport(
    priority: &CoordinatePriority,
    source_for_target: &[usize],
) -> Result<CoordinatePriority, Error> {
    let arity = priority.arity();
    debug_assert_eq!(source_for_target.len(), arity);
    let mut rank_by_slot = Vec::new();
    rank_by_slot
        .try_reserve_exact(arity)
        .map_err(|_| Error::AllocationFailure {
            resource: "transported coordinate-priority ranks",
            requested: arity,
        })?;
    rank_by_slot.extend(
        source_for_target
            .iter()
            .map(|&source| priority.rank_by_slot()[source]),
    );
    Ok(CoordinatePriority::from_validated_rank_by_slot(
        rank_by_slot,
    ))
}

fn copy_validated_priority(ranks: &[usize]) -> Result<CoordinatePriority, Error> {
    let mut retained = Vec::new();
    retained
        .try_reserve_exact(ranks.len())
        .map_err(|_| Error::AllocationFailure {
            resource: "coordinate-priority quotient candidate",
            requested: ranks.len(),
        })?;
    retained.extend_from_slice(ranks);
    Ok(CoordinatePriority::from_validated_rank_by_slot(retained))
}

fn checked_factorial(arity: usize) -> Result<usize, Error> {
    (2..=arity).try_fold(1usize, |product, factor| {
        product
            .checked_mul(factor)
            .ok_or(Error::ResourceCountOverflow {
                resource: "priority quotient cardinality",
            })
    })
}

fn checked_product(resource: &'static str, left: usize, right: usize) -> Result<usize, Error> {
    left.checked_mul(right)
        .ok_or(Error::ResourceCountOverflow { resource })
}

fn admit_limit(resource: &'static str, requested: usize, limit: usize) -> Result<(), Error> {
    if requested <= limit {
        Ok(())
    } else {
        Err(Error::ResourceLimit {
            resource,
            requested,
            limit,
        })
    }
}

fn next_permutation(values: &mut [usize]) -> bool {
    let Some(pivot) = (1..values.len()).rfind(|&slot| values[slot - 1] < values[slot]) else {
        return false;
    };
    let pivot = pivot - 1;
    let successor = (pivot + 1..values.len())
        .rfind(|&slot| values[pivot] < values[slot])
        .expect("a lexicographic successor exists after the pivot");
    values.swap(pivot, successor);
    values[pivot + 1..].reverse();
    true
}
