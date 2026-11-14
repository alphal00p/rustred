//! Univariate rational reconstruction over prime fields, lifted to Q.
//!
//! Univariate rational functions are interpolated from black-box probes
//! modulo word-size primes and lifted to Q. Everything is RustRed code on
//! numerica primitives present in every Symbolica lineage RustRed builds
//! against (`Zp64`, `Integer::chinese_remainder`,
//! `Rational::maximal_quotient_reconstruction`); no Symbolica
//! `poly::reconstruction` is needed.
//!
//! Algorithms, implemented from the papers (no third-party code):
//!
//! | Piece | Source |
//! |---|---|
//! | Newton interpolation, early termination | FireFly, arXiv:1904.00009 §2.1 |
//! | Thiele continued fractions, early termination, unlucky zeros | FireFly §2.2 |
//! | Racing interpolation strategies on shared probes | FireFly 2, arXiv:2004.01463 §2.1 |
//! | Known degrees after the first prime; pruning of finished functions | FireFly §2.3 |
//! | CRT, Wang's reconstruction raced with Monagan's MQRR, acceptance over two consecutive primes | FireFly §2.3, Alg. 1, 2, 4 |
//! | Termination check in a fresh prime | FireFly §2.3, Alg. 3 |
//! | Bunches of points per black-box call | FireFly 2 §3.1 |
//! | Per-prime probe state for resumption | FireFly 2 §4 (`ff_save`) |
//!
//! Layout:
//! - [`poly`]: dense `Zp64` polynomials and the normalisation of a
//!   fraction (denominator's lowest nonzero coefficient one);
//! - [`interpolate`]: [`Newton`], [`Thiele`], their race [`Interpolator`];
//! - [`known`]: [`Shape`] and [`fit_known_shape`];
//! - [`rational`]: [`RationalFunction`] over Q, Wang and MQRR;
//! - [`lift`]: [`Lift`] (CRT + race + acceptance);
//!
//! The black-box driver (`reconstruct_univariate`, fresh-prime
//! verification, probe bunches and a resumable probe log) builds on these
//! primitives and is added separately.
//!
//! The LiteRed lane's `solver/litered/reconstruct.rs` (Thiele with early
//! termination, `CrtAccumulator` with MQRR, the same normalisation) is the
//! special case `Interpolator` + `Lift` of this module; that lane moves
//! onto it when both are on main.

pub mod interpolate;
pub mod known;
pub mod lift;
pub mod poly;
pub mod rational;

/// A `Zp64` element.
pub type Zp = symbolica::domains::finite_field::FiniteFieldElement<u64>;

pub use interpolate::{Interpolator, Newton, Thiele};
pub use known::{Shape, fit_known_shape};
pub use lift::{Lift, LiftMethod, LiftStatus};
pub use rational::{RationalFunction, mqrr_reconstruction, rational_image, wang_reconstruction};

#[cfg(test)]
mod audit_primitive_tests;
#[cfg(test)]
mod primitive_tests;
