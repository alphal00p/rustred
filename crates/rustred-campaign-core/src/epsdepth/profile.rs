//! Valuations at d0 and LOCAL pole profiles of one relation at one key (interfaces/epsdepth.md 3).
//!
//! [`Val`] mirrors lane poles' `valuation::Valuation` (algebra crate): `Zero` for an identically zero coefficient (no
//! order), `Order(v)` with `v` the lowest exponent of `t = d - d0`. The trait [`Valuation`] is what this module needs of
//! an exact valuation; the algebra crate implements it (until lane poles' `order_at_key` is merged, then that function
//! implements it). Nothing here does algebra.

use serde::{Deserialize, Serialize};

/// The order of an exact coefficient at d0 (pole-diagnostics.md 1.1).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Val {
    /// Identically zero (at the key): no order, regular, never an edge.
    Zero,
    /// `v_{d0}`: < 0 a pole of order `-v`, 0 regular and non-zero, > 0 a zero of that order.
    Order(i64),
}

impl Val {
    /// `max(0, -v)`; 0 for [`Val::Zero`].
    pub fn pole_order(&self) -> u32 {
        match self {
            Val::Order(v) if *v < 0 => u32::try_from(v.unsigned_abs()).unwrap_or(u32::MAX),
            _ => 0,
        }
    }
    pub fn is_zero(&self) -> bool {
        matches!(self, Val::Zero)
    }
}

/// An exact valuation of a coefficient TEXT at a KEY (the key's index values are put in first: TRICKS 9).
/// `arity` = number of index variables `n0 .. n{arity-1}` the text may use (0: a coefficient of d alone).
pub trait Valuation {
    fn pole_order(
        &mut self,
        coeff: &str,
        arity: usize,
        key: &[i32],
        d0: &str,
    ) -> Result<Val, String>;
}

/// Class of a right-hand-side term of a relation (modulo subsectors only top terms are composed).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TermKind {
    Top,
    Subsector,
    ZeroSector,
    PositiveNumerator,
}

/// One rhs term of a relation at a key with its exact valuation.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TermVal {
    pub key: Vec<i32>,
    pub kind: TermKind,
    pub val: Val,
}

/// The LOCAL profile of a relation at a key (pole-diagnostics.md 1.1; addendum 8.1 "per right-hand-side term").
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct LocalProfile {
    /// max pole order over the non-zero terms (top and subsector).
    pub pole_order: u32,
    /// max pole order over the non-zero TOP terms.
    pub pole_order_top: u32,
    /// non-zero terms with a pole.
    pub singular_terms: u32,
    /// non-zero terms.
    pub terms: u32,
    /// zero terms (exactly zero at the key: not edges).
    pub zero_terms: u32,
    /// worst terms (highest pole order first, then key order), at most `worst` of them.
    pub worst: Vec<(Vec<i32>, u32)>,
}

impl LocalProfile {
    /// Summarise term valuations; `worst` = how many worst terms to keep.
    pub fn of(terms: &[TermVal], worst: usize) -> LocalProfile {
        let mut p = LocalProfile::default();
        let mut w: Vec<(Vec<i32>, u32)> = vec![];
        for t in terms {
            if t.val.is_zero() {
                p.zero_terms += 1;
                continue;
            }
            p.terms += 1;
            let o = t.val.pole_order();
            if o > 0 {
                p.singular_terms += 1;
                w.push((t.key.clone(), o));
            }
            p.pole_order = p.pole_order.max(o);
            if t.kind == TermKind::Top {
                p.pole_order_top = p.pole_order_top.max(o);
            }
        }
        w.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
        w.truncate(worst);
        p.worst = w;
        p
    }

    /// The per-relation verdict word (TRICKS 16: a local finding is never "inherent").
    pub fn verdict(&self) -> String {
        if self.pole_order == 0 {
            "regular_here".into()
        } else {
            format!("singular_here({})", self.pole_order)
        }
    }
}
