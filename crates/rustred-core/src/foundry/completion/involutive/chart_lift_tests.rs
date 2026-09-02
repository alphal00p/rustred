use std::sync::Arc;

use symbolica::prelude::Integer;

use crate::algebra::{
    CoefficientContext, CoefficientPolynomial, ExactAlgebraError, ExactAlgebraOperation,
    IndexedAlgebraError, IndexedCoefficient, IndexedCoefficientContext, IndexedPolynomial,
};
use crate::family::{AffineDenominator, IntegralFamily};
use crate::foundry::artifact::derive_two_loop_unit_mass_sunset;
use crate::identity::{
    CompletedIbpSourceRows, IndexShift, ParametricIbpGenerator, ParametricRelation,
    RelationBuilder, RelationLimits, RowId,
};
use crate::sector::{Mask, OrderingPolicy};

use super::super::super::CompletionGeometryLimits;
use super::super::{
    ForwardShift, InvolutiveError, InvolutiveLimits, JanetBasisEpoch, OreConsequence,
    OreOrderingAdapter, OreRow,
};
use super::*;

#[path = "chart_lift_tests/support.rs"]
mod support;

use support::*;

// Textual inclusion keeps the established test-filter paths stable while the
// fixtures and cases remain split into reviewable semantic files.
include!("chart_lift_tests/owner.rs");
include!("chart_lift_tests/determinism.rs");
include!("chart_lift_tests/preflight.rs");
