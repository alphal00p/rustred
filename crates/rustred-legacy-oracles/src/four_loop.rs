//! Structural families for Vakint's equal-mass four-loop vacuum parents.
//!
//! The physical momentum routings are transcribed from Vakint's checked-in
//! topology registry.  Four loops have ten independent scalar products, while
//! these graphs have eight or nine physical lines, so
//! [`VacuumFamily::new_with_standard_auxiliaries`] appends deterministic ISP
//! basis entries.  Only the identity symmetry is registered for now: graph
//! automorphisms generally act linearly, rather than by permutation, on those
//! generated auxiliaries.
//!
//! This module is a family/IBP foundation.  It does not claim a complete
//! four-loop reduction or a master basis.

use crate::{Denominator, FamilyError, VacuumFamily};
use rustred::{CoefficientContext, ExactRational};

/// Equal-mass four-loop parent topologies currently registered by Vakint.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum FourLoopTopology {
    H,
    X,
    Bmw,
    Fg,
}

impl FourLoopTopology {
    pub const ALL: [Self; 4] = [Self::H, Self::X, Self::Bmw, Self::Fg];

    pub const fn name(self) -> &'static str {
        match self {
            Self::H => "equal_mass_four_loop_h",
            Self::X => "equal_mass_four_loop_x",
            Self::Bmw => "equal_mass_four_loop_bmw",
            Self::Fg => "equal_mass_four_loop_fg",
        }
    }

    pub const fn vakint_head(self) -> &'static str {
        match self {
            Self::H => "I4L_H",
            Self::X => "I4L_X",
            Self::Bmw => "I4L_BMW",
            Self::Fg => "I4L_FG",
        }
    }

    /// Physical momentum rows in Vakint propagator order.
    pub const fn routings(self) -> &'static [[i8; 4]] {
        match self {
            Self::H => &FOUR_LOOP_H_ROUTINGS,
            Self::X => &FOUR_LOOP_X_ROUTINGS,
            Self::Bmw => &FOUR_LOOP_BMW_ROUTINGS,
            Self::Fg => &FOUR_LOOP_FG_ROUTINGS,
        }
    }
}

pub const FOUR_LOOP_H_ROUTINGS: [[i8; 4]; 9] = [
    [1, 0, 0, 0],
    [0, 1, 0, 0],
    [0, 0, 1, 0],
    [0, 0, 0, 1],
    [1, 0, -1, 0],
    [0, 1, -1, 0],
    [-1, 0, 1, 1],
    [0, -1, 1, 1],
    [0, 0, 1, 1],
];

pub const FOUR_LOOP_X_ROUTINGS: [[i8; 4]; 9] = [
    [1, 0, 0, 0],
    [0, 1, 0, 0],
    [0, 0, 1, 0],
    [0, 0, 0, 1],
    [1, 0, -1, 0],
    [0, 1, -1, 0],
    [-1, 0, 1, 1],
    [0, -1, 1, 1],
    [-1, -1, 1, 1],
];

pub const FOUR_LOOP_BMW_ROUTINGS: [[i8; 4]; 8] = [
    [1, 0, 0, 0],
    [0, 1, 0, 0],
    [0, 0, 1, 0],
    [0, 0, 0, 1],
    [1, -1, 0, 0],
    [0, 0, 1, -1],
    [-1, 1, 1, 0],
    [-1, 0, 1, -1],
];

pub const FOUR_LOOP_FG_ROUTINGS: [[i8; 4]; 8] = [
    [1, 0, 0, 0],
    [0, 1, 0, 0],
    [0, 0, 1, 0],
    [1, 0, -1, 0],
    [0, 0, 0, 1],
    [0, 1, -1, 0],
    [1, 0, -1, 1],
    [1, -1, 0, 0],
];

/// Construct one complete scalar-product family in Vakint propagator order.
pub fn equal_mass_four_loop_vacuum(
    topology: FourLoopTopology,
) -> Result<VacuumFamily, FamilyError> {
    let coefficients = CoefficientContext::new(["d", "m2"]);
    let mass = coefficients
        .parameter("m2")
        .expect("the built-in context defines m2");
    let propagators = topology
        .routings()
        .iter()
        .map(|routing| {
            Denominator::propagator(
                routing
                    .iter()
                    .map(|&component| ExactRational::from(i64::from(component)))
                    .collect(),
                mass.clone(),
            )
        })
        .collect();
    VacuumFamily::new_with_standard_auxiliaries(
        topology.name(),
        4,
        coefficients,
        "d",
        propagators,
        Vec::new(),
    )
}

pub fn equal_mass_four_loop_h() -> Result<VacuumFamily, FamilyError> {
    equal_mass_four_loop_vacuum(FourLoopTopology::H)
}

pub fn equal_mass_four_loop_x() -> Result<VacuumFamily, FamilyError> {
    equal_mass_four_loop_vacuum(FourLoopTopology::X)
}

pub fn equal_mass_four_loop_bmw() -> Result<VacuumFamily, FamilyError> {
    equal_mass_four_loop_vacuum(FourLoopTopology::Bmw)
}

pub fn equal_mass_four_loop_fg() -> Result<VacuumFamily, FamilyError> {
    equal_mass_four_loop_vacuum(FourLoopTopology::Fg)
}
