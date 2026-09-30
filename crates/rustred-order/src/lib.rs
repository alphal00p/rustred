//! Validated, runtime-selected integral orders, independent of algebra and search.
//!
//! An order is simpler-first. Support cardinality is always first, so a strict
//! pinch is simpler. Within one support, nonnegative weighted excess rows are
//! compared lexicographically. Every coordinate, with either sign, must occur
//! positively in at least one row. Consequently a fixed vector of row values
//! has finitely many concrete integrals, making even reversed final ties safe.
//!
//! This crate establishes integer ordering semantics, not rule applicability,
//! coefficient identities, or descent on a domain. In particular, comparing
//! shifts requires the caller to prove that both translated integrals stay in
//! the stated support. No callback executes during comparison.

mod codec;
mod compare;
mod descriptor;
mod transport;

pub use compare::{Comparison, Component};
pub use descriptor::{CoordinateGroups, DegreeRow, Direction, Error, Limits, OrderDescriptor};

use std::cmp::Ordering;
use std::hash::{Hash, Hasher};
use std::sync::Arc;

/// Shared, immutable compiled order. Cloning is an ownership-boundary action;
/// comparisons borrow it and neither allocate nor clone an `Arc`.
#[derive(Clone, Debug)]
pub struct CompiledOrder(Arc<Program>);

#[derive(Debug)]
struct Program {
    descriptor: OrderDescriptor,
    canonical: Box<[u8]>,
}

impl CompiledOrder {
    pub fn compile(descriptor: OrderDescriptor, limits: Limits) -> Result<Self, Error> {
        descriptor.validate(limits)?;
        let canonical = codec::encode(&descriptor)?;
        Ok(Self(Arc::new(Program {
            descriptor,
            canonical,
        })))
    }

    /// Evaluate a caller's finite builder exactly once, then retain only data.
    /// The callback type is not part of this type or the comparison machinery.
    pub fn from_builder(
        limits: Limits,
        build: impl FnOnce() -> OrderDescriptor,
    ) -> Result<Self, Error> {
        Self::compile(build(), limits)
    }

    pub fn from_canonical_bytes(bytes: &[u8], limits: Limits) -> Result<Self, Error> {
        Self::compile(codec::decode(bytes, limits)?, limits)
    }

    /// Versioned, injective encoding of the validated descriptor, not an algebra
    /// hash or a promise that different descriptors cannot induce equal orders.
    pub fn canonical_bytes(&self) -> &[u8] {
        &self.0.canonical
    }

    pub fn descriptor(&self) -> &OrderDescriptor {
        &self.0.descriptor
    }

    pub fn arity(&self) -> usize {
        self.descriptor().support_weights.len()
    }

    /// On one fixed support, descent under this program cannot increase
    /// unweighted total excess if its first row is a positive multiple of E.
    /// This is not a bound on a support-changing or transported successor.
    pub fn has_total_excess_primary(&self) -> bool {
        let row = &self.descriptor().degree_rows[0];
        let weight = row.active[0];
        weight != 0
            && row
                .active
                .iter()
                .chain(&row.inactive)
                .all(|&value| value == weight)
    }
}

impl PartialEq for CompiledOrder {
    fn eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.0, &other.0) || self.canonical_bytes() == other.canonical_bytes()
    }
}
impl Eq for CompiledOrder {}
impl PartialOrd for CompiledOrder {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}
impl Ord for CompiledOrder {
    fn cmp(&self, other: &Self) -> Ordering {
        if Arc::ptr_eq(&self.0, &other.0) {
            return Ordering::Equal;
        }
        self.canonical_bytes().cmp(other.canonical_bytes())
    }
}
impl Hash for CompiledOrder {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.canonical_bytes().hash(state);
    }
}

#[cfg(test)]
mod tests;
