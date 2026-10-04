//! Per-sector master-integral counts from the Lee–Pomeransky polynomial.
//!
//! For a sector `S` let `G_S` be `G = U + F` with `x_i = 0` for every
//! inactive `i`. The counter samples `G_S` modulo independent primes at
//! independent random kinematic points and compares two counts of the
//! masters whose top sector is `S`, without symmetries:
//!
//! - the Morse count `mu(S) = dim Zp[x_S, t] / <dG_S/dx_i, 1 - t G_S>`, the
//!   proper critical points of `G_S` on the affine space `C^S` (Lee and
//!   Pomeransky);
//! - the Euler count `e(S) = sum_{T <= S} (-1)^(|S|-|T|) c(T)`, the Moebius
//!   inversion over the down-closure of
//!   `c(T) = dim Zp[x_T, t] / <x_i dG_T/dx_i + u_i G_T, 1 - t G_T>` with
//!   random `u_i`. By Huh's theorem `c(T)` counts the critical points of
//!   `log G_T + sum u_i log x_i` and equals `|chi|` of the torus complement
//!   of `G_T = 0`, the masters supported on `T` and its subsectors for
//!   generic indices.
//!
//! For integer indices `c(T)` can fall below the number of masters when null
//! external shifts form a cycle, for example in an equal-mass box with
//! light-like legs. The per-sector split then fails only on faces such as the
//! adjacent null bubbles, whose Morse locus is non-isolated or whose Euler
//! count is negative, so they receive no verdict. Do not use `c(T)` as a bound
//! on the residuals of several sectors together.
//!
//! A sector is [`MasterCount::Zero`] when the zero-sector analyzer proves it
//! scaleless or `G_S` vanishes identically. It is [`MasterCount::Counted`]
//! only when `mu` is finite and `mu == e` agree across every sample and the
//! external Gram determinant does not vanish identically: with a singular
//! Gram matrix (null momenta, forward kinematics) the parametric count can be
//! below what momentum-space IBP reaches, and in degenerate kinematics `mu`
//! may be non-isolated or low while `e` may be low or negative. Every other
//! case is a [`MasterCount::NoVerdict`] with the most specific
//! [`NoVerdictReason`].
//!
//! Quotient dimensions are read from grevlex Groebner bases over 32-bit
//! prime fields (Symbolica's specialized `F4` path). Complete-family
//! auxiliary denominators need no special treatment: they are never active
//! in physical sectors. Nonzero power shifts are rejected.

mod counter;
mod error;
mod ideal;
mod model;
mod sample;
mod staircase;

pub use counter::{MAX_SAMPLES, MasterCounter};
pub use error::MasterCountError;
pub use model::{MasterCount, MasterCountOptions, NoVerdictReason};
