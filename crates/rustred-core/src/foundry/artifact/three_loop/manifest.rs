/// One exact sector-orbit entry under the authenticated `S4` action.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct SectorOrbit {
    pub(crate) representative: [i64; 6],
    pub(crate) size: usize,
}

pub(crate) const ZERO_ORBITS: [SectorOrbit; 5] = [
    SectorOrbit {
        representative: [0, 0, 0, 0, 0, 0],
        size: 1,
    },
    SectorOrbit {
        representative: [0, 0, 0, 0, 0, 1],
        size: 6,
    },
    SectorOrbit {
        representative: [0, 0, 0, 0, 1, 1],
        size: 12,
    },
    SectorOrbit {
        representative: [0, 0, 0, 1, 1, 1],
        size: 4,
    },
    SectorOrbit {
        representative: [0, 0, 1, 0, 1, 0],
        size: 3,
    },
];

/// Full-loop-rank sector orbits that remain closure obligations.
///
/// Full active-momentum rank excludes the elementary scaleless-loop proof; it
/// is not, by itself, an analytic nonzero certificate for the integral.
pub(crate) const FULL_RANK_ORBITS: [SectorOrbit; 6] = [
    SectorOrbit {
        representative: [0, 0, 1, 0, 1, 1],
        size: 12,
    },
    SectorOrbit {
        representative: [0, 0, 1, 1, 0, 1],
        size: 4,
    },
    SectorOrbit {
        representative: [0, 0, 1, 1, 1, 1],
        size: 12,
    },
    SectorOrbit {
        representative: [0, 1, 1, 1, 1, 0],
        size: 3,
    },
    SectorOrbit {
        representative: [0, 1, 1, 1, 1, 1],
        size: 6,
    },
    SectorOrbit {
        representative: [1, 1, 1, 1, 1, 1],
        size: 1,
    },
];

/// GammaLoop revision from which the test-only Vakint class snapshot was
/// derived. The source blob identifies `crates/vakint/src/topologies.rs` at
/// that revision. These values make drift reviewable; the local tests below
/// authenticate the frozen snapshot's RustRed semantics, not a live checkout.
pub(crate) const VAKINT_SOURCE_REVISION: &str = "7d96a79602498c8c52cad067e3ea600af9a26e05";
pub(crate) const VAKINT_TOPOLOGIES_BLOB: &str = "7c79eb9d7e43b05b04f258ff40f4b54184017d8e";

/// Test-only frozen integration snapshot. Powers are always assembled by
/// stable propagator slot, never by dense position in a contracted graph.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct VakintClassWitness {
    pub(crate) label: &'static str,
    pub(crate) active_slots: [bool; 6],
    pub(crate) routing_rows: [i64; 9],
    pub(crate) canonical_sector: [i64; 6],
}

impl VakintClassWitness {
    pub(crate) fn powers_by_slot(self, powers: [i64; 6]) -> [i64; 6] {
        std::array::from_fn(|slot| {
            if self.active_slots[slot] {
                powers[slot]
            } else {
                0
            }
        })
    }
}

pub(crate) const VAKINT_CLASSES: [VakintClassWitness; 5] = [
    VakintClassWitness {
        label: "I3L",
        active_slots: [true, true, true, true, true, true],
        routing_rows: [1, 0, 0, 0, 1, 0, 0, 0, 1],
        canonical_sector: [1, 1, 1, 1, 1, 1],
    },
    VakintClassWitness {
        label: "I3L_pinch_6",
        active_slots: [true, true, true, true, true, false],
        routing_rows: [1, 0, 0, 0, 1, 0, 0, 0, 1],
        canonical_sector: [0, 1, 1, 1, 1, 1],
    },
    VakintClassWitness {
        label: "I3L_pinch_1_6",
        active_slots: [false, true, true, true, true, false],
        routing_rows: [0, 1, 0, 0, 0, 1, -1, 0, 1],
        canonical_sector: [0, 1, 1, 1, 1, 0],
    },
    VakintClassWitness {
        label: "I3L_pinch_3_6",
        active_slots: [true, true, false, true, true, false],
        routing_rows: [1, 0, 0, 0, 1, 0, -1, 0, 1],
        canonical_sector: [0, 0, 1, 1, 1, 1],
    },
    VakintClassWitness {
        label: "I3L_pinch_1_3_6",
        active_slots: [false, true, false, true, true, false],
        routing_rows: [0, 1, 0, -1, 0, 1, 1, -1, 0],
        canonical_sector: [0, 0, 1, 0, 1, 1],
    },
];
