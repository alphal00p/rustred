use std::sync::Arc;

use crate::family::IntegralKey;
use crate::sector::{ComplexityKey, OrderingPolicy, StrictDescentWitness};

use super::super::permutation::TransportError;
use super::Error;

pub const DEFAULT_MAX_GENERATORS: usize = 4_096;
pub const DEFAULT_MAX_GROUP_ORDER: usize = 1_000_000;
pub const DEFAULT_MAX_GROUP_ENTRIES: usize = 16_000_000;

/// Bounds for sealing and applying one finite denominator-permutation action.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CanonicalizationLimits {
    pub max_generators: usize,
    pub max_group_order: usize,
    /// Aggregate denominator slots retained by the derived group.
    pub max_group_entries: usize,
}

impl Default for CanonicalizationLimits {
    fn default() -> Self {
        Self {
            max_generators: DEFAULT_MAX_GENERATORS,
            max_group_order: DEFAULT_MAX_GROUP_ORDER,
            max_group_entries: DEFAULT_MAX_GROUP_ENTRIES,
        }
    }
}

/// Exact scalar multiplier carried by an internal integral route.
///
/// The authenticated permutation compiler currently admits only unit
/// denominator scales and unit loop Jacobians, so every retained route has
/// coefficient one. Keeping this explicit prevents callers from silently
/// assuming that future, more general maps also have unit coefficient.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum RoutingCoefficient {
    One,
}

/// One exact group element routing source powers into target slots.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct RoutingWitness {
    pub(super) group_element: usize,
    pub(super) source_for_target: Arc<[usize]>,
}

impl RoutingWitness {
    /// Stable ordinal in the owner's lexicographically sorted group.
    pub const fn group_element(&self) -> usize {
        self.group_element
    }

    /// `source_for_target[j]` names the source power routed to target slot
    /// `j`.
    pub fn source_for_target(&self) -> &[usize] {
        &self.source_for_target
    }

    pub const fn coefficient(&self) -> RoutingCoefficient {
        RoutingCoefficient::One
    }

    pub fn transport_into(&self, source: &[i64], target: &mut [i64]) -> Result<(), TransportError> {
        let expected = self.source_for_target.len();
        if source.len() != expected {
            return Err(TransportError::WrongSourceArity {
                expected,
                actual: source.len(),
            });
        }
        if target.len() != expected {
            return Err(TransportError::WrongTargetArity {
                expected,
                actual: target.len(),
            });
        }
        for (target_slot, &source_slot) in self.source_for_target.iter().enumerate() {
            target[target_slot] = source[source_slot];
        }
        Ok(())
    }

    /// Replay this exact route against an owned source/image pair.
    pub fn verify(&self, source: &IntegralKey, image: &IntegralKey) -> bool {
        source.powers().len() == self.source_for_target.len()
            && image.powers().len() == self.source_for_target.len()
            && self.source_for_target.iter().zip(image.powers()).all(
                |(&source_slot, &image_power)| {
                    source.powers().get(source_slot) == Some(&image_power)
                },
            )
    }
}

/// Exact weak-order proof that a routed image is no harder than its source.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct NoHarderWitness {
    policy: OrderingPolicy,
    source: ComplexityKey,
    image: ComplexityKey,
}

impl NoHarderWitness {
    pub(super) fn new(
        policy: OrderingPolicy,
        source: ComplexityKey,
        image: ComplexityKey,
    ) -> Result<Self, Error> {
        if source.policy() != policy || image.policy() != policy || image > source {
            return Err(Error::OrbitInvariant {
                detail: "selected image is harder than its source",
            });
        }
        Ok(Self {
            policy,
            source,
            image,
        })
    }

    pub const fn policy(&self) -> OrderingPolicy {
        self.policy
    }

    pub fn source(&self) -> &ComplexityKey {
        &self.source
    }

    pub fn image(&self) -> &ComplexityKey {
        &self.image
    }

    pub fn is_strict(&self) -> bool {
        self.image < self.source
    }

    pub fn verify(&self) -> bool {
        self.source.policy() == self.policy
            && self.image.policy() == self.policy
            && self.image <= self.source
    }
}

/// One distinct image in an exact finite orbit.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OrbitImage {
    pub(super) integral: IntegralKey,
    pub(super) complexity: ComplexityKey,
    pub(super) route: RoutingWitness,
    pub(super) routing_multiplicity: usize,
}

impl OrbitImage {
    pub fn integral(&self) -> &IntegralKey {
        &self.integral
    }

    pub fn complexity(&self) -> &ComplexityKey {
        &self.complexity
    }

    /// Lexicographically least exact group route to this image.
    pub fn route(&self) -> &RoutingWitness {
        &self.route
    }

    /// Number of group elements routing the source to this same image.
    pub const fn routing_multiplicity(&self) -> usize {
        self.routing_multiplicity
    }
}

/// All value-distinct images of one integral under a sealed exact action.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ExactOrbit {
    pub(super) source: IntegralKey,
    pub(super) source_complexity: ComplexityKey,
    pub(super) images: Box<[OrbitImage]>,
    pub(super) group_order: usize,
}

impl ExactOrbit {
    pub fn source(&self) -> &IntegralKey {
        &self.source
    }

    pub fn source_complexity(&self) -> &ComplexityKey {
        &self.source_complexity
    }

    /// Images sorted by the owner's exact complexity key. The first image is
    /// therefore canonical.
    pub fn images(&self) -> &[OrbitImage] {
        &self.images
    }

    pub fn canonical(&self) -> &OrbitImage {
        self.images
            .first()
            .expect("a sealed finite group always has a nonempty orbit")
    }

    pub const fn group_order(&self) -> usize {
        self.group_order
    }

    pub fn orbit_size(&self) -> usize {
        self.images.len()
    }
}

/// Canonical image and the exact route/proof selected for one source key.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Canonicalization {
    source: IntegralKey,
    canonical: IntegralKey,
    route: RoutingWitness,
    no_harder: NoHarderWitness,
    orbit_size: usize,
    routing_multiplicity: usize,
}

impl Canonicalization {
    pub(super) fn from_orbit(orbit: ExactOrbit) -> Result<Self, Error> {
        let selected = orbit.canonical().clone();
        let no_harder = NoHarderWitness::new(
            orbit.source_complexity.policy(),
            orbit.source_complexity,
            selected.complexity,
        )?;
        Ok(Self {
            source: orbit.source,
            canonical: selected.integral,
            route: selected.route,
            no_harder,
            orbit_size: orbit.images.len(),
            routing_multiplicity: selected.routing_multiplicity,
        })
    }

    pub fn source(&self) -> &IntegralKey {
        &self.source
    }

    pub fn canonical(&self) -> &IntegralKey {
        &self.canonical
    }

    pub fn route(&self) -> &RoutingWitness {
        &self.route
    }

    pub fn no_harder(&self) -> &NoHarderWitness {
        &self.no_harder
    }

    pub const fn orbit_size(&self) -> usize {
        self.orbit_size
    }

    pub const fn routing_multiplicity(&self) -> usize {
        self.routing_multiplicity
    }

    pub fn is_identity(&self) -> bool {
        self.source == self.canonical
    }

    pub fn verify(&self) -> bool {
        self.route.verify(&self.source, &self.canonical)
            && self.no_harder.verify()
            && self.no_harder.source().index_excess()
                == self
                    .source
                    .powers()
                    .iter()
                    .map(|&power| {
                        if power >= 1 {
                            (power - 1) as u64
                        } else {
                            power.unsigned_abs()
                        }
                    })
                    .collect::<Vec<_>>()
            && self.no_harder.image().index_excess()
                == self
                    .canonical
                    .powers()
                    .iter()
                    .map(|&power| {
                        if power >= 1 {
                            (power - 1) as u64
                        } else {
                            power.unsigned_abs()
                        }
                    })
                    .collect::<Vec<_>>()
    }
}

/// The reducer-facing proof chain `parent > raw_child >= canonical_child`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DescendingCanonicalization {
    raw_descent: StrictDescentWitness,
    child: Canonicalization,
}

impl DescendingCanonicalization {
    pub(super) fn new(
        raw_descent: StrictDescentWitness,
        child: Canonicalization,
    ) -> Result<Self, Error> {
        if raw_descent.target() != child.no_harder().source() {
            return Err(Error::OrbitInvariant {
                detail: "raw descent target differs from the canonical route source",
            });
        }
        Ok(Self { raw_descent, child })
    }

    pub fn raw_descent(&self) -> &StrictDescentWitness {
        &self.raw_descent
    }

    pub fn child(&self) -> &Canonicalization {
        &self.child
    }

    pub fn into_child(self) -> Canonicalization {
        self.child
    }

    pub fn verify(&self) -> bool {
        self.raw_descent.verify()
            && self.child.verify()
            && self.raw_descent.target() == self.child.no_harder().source()
            && self.child.no_harder().image() < self.raw_descent.source()
    }
}
