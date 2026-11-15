//! The composed reduction of a key onto the closure's terminals, as the regular-basis construction consumes it.
//!
//! `K = sum_i c_i(d) T_i + subsector terms` (modulo subsectors). The construction needs, per band key, every `c_i` as a
//! TRUNCATED LAURENT SERIES in `t = d - d0` with exact rational coefficients and a known absolute precision (every exponent
//! below `prec` known), and, for the few keys of the regular set, the EXACT coefficients (rational functions of d).
//! Lane epsdepth's composition (`epsdepth::compose`, generic over its `Laurent` and `ReductionSource` traits) delivers
//! exactly this; an adapter implements [`Compose`] on it. Constructed tests use the exact stand-in of the algebra crate
//! (`rustred_campaign_algebra::regbasis::standin`).
//!
//! The exchange is TEXT (exact rationals `a` or `a/b`, rational functions of `d` in the house syntax) so that this crate
//! stays Symbolica-free; the algebra crate parses with Symbolica.

use serde::{Deserialize, Serialize};

use crate::key::Key;

/// A truncated Laurent series in `t = d - d0`: coefficient `coeffs[k]` belongs to `t^(start + k)`; every exponent
/// `< prec` is known (exponents in `[start + coeffs.len(), prec)` have coefficient 0; exponents below `start` too).
/// Exponents `>= prec` are unknown. A series that is zero to its precision has `coeffs` empty (or all `"0"`).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SeriesText {
    pub start: i64,
    pub coeffs: Vec<String>,
    pub prec: i64,
}

impl SeriesText {
    /// The series that is zero to precision `prec`.
    pub fn zero(prec: i64) -> SeriesText {
        SeriesText { start: prec, coeffs: vec![], prec }
    }

    /// The constant `c` (one coefficient at exponent 0), known to precision `prec >= 1`.
    pub fn constant(c: &str, prec: i64) -> SeriesText {
        SeriesText { start: 0, coeffs: vec![c.to_string()], prec }
    }

    /// Structural check: listed coefficients lie below the precision.
    pub fn validate(&self) -> Result<(), String> {
        let end = self
            .start
            .checked_add(self.coeffs.len() as i64)
            .ok_or_else(|| "series exponent overflow".to_string())?;
        if !self.coeffs.is_empty() && end > self.prec {
            return Err(format!(
                "series lists exponents up to {} but its precision is {}",
                end - 1,
                self.prec
            ));
        }
        Ok(())
    }
}

/// The composed vector of one key: one series per terminal (terminal order of the closure).
pub type ComposedVec = Vec<SeriesText>;

/// Why a key has no composed vector.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ComposeError {
    /// Not decided (budget, an open key in its cone, a precision cap): the key is EXCLUDED and the band verdict drops
    /// to the ranks below it ("regular through rank r (lower bound)").
    Undecided(String),
    /// The source cannot provide this kind of answer (e.g. exact coefficients) for this key.
    Unavailable(String),
}

impl std::fmt::Display for ComposeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ComposeError::Undecided(s) => write!(f, "undecided: {s}"),
            ComposeError::Unavailable(s) => write!(f, "unavailable: {s}"),
        }
    }
}

/// The composed reduction onto the closure's terminals (lane epsdepth's composition behind an adapter).
pub trait Compose {
    /// Number of terminals n (the length of every [`ComposedVec`]).
    fn n_terminals(&self) -> usize;

    /// Compose a batch of keys; `visit(i, result)` once per `keys[i]` (any order). Every returned series must be known
    /// for all exponents `< abs_prec` (a series with a smaller precision is refused by the consumer as undecided).
    /// `Err` = the source itself failed (the run stops; nothing is guessed).
    #[allow(clippy::type_complexity)]
    fn stream(
        &mut self,
        keys: &[Key],
        abs_prec: i64,
        visit: &mut dyn FnMut(usize, Result<ComposedVec, ComposeError>),
    ) -> Result<(), String>;

    /// The exact coefficients of `key` on the terminals (rational functions of `d`, house syntax), one per terminal.
    fn exact(&mut self, key: &[i32]) -> Result<Vec<String>, ComposeError>;
}
