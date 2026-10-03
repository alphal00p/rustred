//! Optional one-protected-gradient nomination. This does not change the radial
//! nominator or confer target, chart-descent, coverage or cost authority.
//!
//! Let g = dP/dk_i. The affine component formula below is exactly
//! V_s = sum_r (p_s*g_r - g_s*p_r) (q_r.g), hence V = g^2 p - (g.p) g
//! and V.g = 0. Here g and p have native base-field coefficients; cached native
//! derivative contractions supply every q_r.g, including external Gram terms.
//! No Gram division, square root, custom polynomial algebra or index fitting.

use super::{Builder, Limits, Nomination, affine_is_zero};
use crate::{Result, checked, coefficient, require};
use rustred::{
    algebra::{Coefficient, IndexedCoefficient, IndexedCoefficientContext},
    family::{ContractionMomentum, IntegralFamily, ScalarProductCoordinate},
    identity::RowId,
};
use serde_json::json;
use std::collections::BTreeMap;

fn lift(b: &mut Builder<'_>, raw: &Coefficient) -> Result<IndexedCoefficient> {
    b.charge(1)?;
    let value = checked(b.c.lift(raw))?;
    b.retain(&value)?;
    Ok(value)
}

fn mul(
    b: &mut Builder<'_>,
    x: &IndexedCoefficient,
    y: &IndexedCoefficient,
) -> Result<IndexedCoefficient> {
    b.charge(1)?;
    b.retain(x)?;
    b.retain(y)?;
    let value = checked(b.c.mul_with_limits(x, y, b.limits.arithmetic))?;
    b.retain(&value)?;
    Ok(value)
}

fn sub(
    b: &mut Builder<'_>,
    x: &IndexedCoefficient,
    y: &IndexedCoefficient,
) -> Result<IndexedCoefficient> {
    b.charge(1)?;
    // Keep both inputs even when their difference cancels a denominator.
    b.retain(x)?;
    b.retain(y)?;
    let value = checked(b.c.sub_with_limits(x, y, b.limits.arithmetic))?;
    b.retain(&value)?;
    Ok(value)
}

pub(crate) fn nominate(
    family: &IntegralFamily,
    c: &IndexedCoefficientContext,
    active: &[bool],
    loop_index: usize,
    numerator: usize,
    limits: Limits,
) -> Result<Nomination> {
    let n = family.denominator_count();
    require(
        active.len() == n && loop_index < family.loop_count() && numerator < n,
        "gradient nomination axis/loop/owner mismatch",
    )?;
    require(!active[numerator], "selected numerator must be inactive")?;
    let mut b = Builder {
        c,
        limits,
        weights: BTreeMap::new(),
        conditions: Vec::new(),
        operations: 0,
    };
    let mut dependent = Vec::with_capacity(n);
    for axis in 0..n {
        let mut nonzero = false;
        for &q in family.contraction_momenta() {
            b.charge(n + 1)?;
            nonzero |=
                !affine_is_zero(checked(family.derivative_contraction(axis, loop_index, q))?);
        }
        dependent.push(nonzero);
    }
    let protected: Vec<_> = (0..n).filter(|&i| active[i] && dependent[i]).collect();
    require(
        protected.len() == 1,
        "gradient nomination requires exactly one dependent active denominator",
    )?;
    let protected = protected[0];
    let radial = checked(family.coordinate_index(ScalarProductCoordinate::LoopLoop {
        left: loop_index,
        right: loop_index,
    }))?;
    let a_p = lift(
        &mut b,
        &family.denominators()[protected].coefficients()[radial],
    )?;
    let a_j = lift(
        &mut b,
        &family.denominators()[numerator].coefficients()[radial],
    )?;
    // If aP=0, this particular p is parallel to g (or zero), hence V=0.
    // No alternate fitted direction or relaxed chart is silently substituted.
    require(
        !a_p.is_zero(),
        "zero squared-loop coefficient degenerates the canonical gradient nomination",
    )?;
    let mut g = Vec::new();
    let mut p = Vec::new();
    for &q in family.contraction_momenta() {
        if q == ContractionMomentum::Loop(loop_index) {
            g.push(mul(&mut b, &c.integer(2), &a_p)?);
            p.push(c.zero());
            continue;
        }
        let coordinate = match q {
            ContractionMomentum::Loop(other) => ScalarProductCoordinate::LoopLoop {
                left: loop_index,
                right: other,
            },
            ContractionMomentum::External(external_index) => {
                ScalarProductCoordinate::LoopExternal {
                    loop_index,
                    external_index,
                }
            }
        };
        let coordinate = checked(family.coordinate_index(coordinate))?;
        // Off-diagonal entries already contain the polynomial's factor two.
        let b_p = lift(
            &mut b,
            &family.denominators()[protected].coefficients()[coordinate],
        )?;
        let b_j = lift(
            &mut b,
            &family.denominators()[numerator].coefficients()[coordinate],
        )?;
        let left = mul(&mut b, &a_p, &b_j)?;
        let right = mul(&mut b, &a_j, &b_p)?;
        p.push(sub(&mut b, &left, &right)?);
        g.push(b_p);
    }
    require(
        p.iter().any(|x| !x.is_zero()),
        "selected numerator has zero effective cross direction",
    )?;
    let mut recenter = vec![0_i64; n];
    recenter[numerator] = 1;
    for s in 0..g.len() {
        for (r, &q) in family.contraction_momenta().iter().enumerate() {
            let left = mul(&mut b, &p[s], &g[r])?;
            let right = mul(&mut b, &g[s], &p[r])?;
            let scale = sub(&mut b, &left, &right)?;
            // Even a zero scale retains the cached expansion's factor poles
            // before affine_source decides which exact source weights survive.
            b.affine_source(
                RowId::OrdinaryIbp {
                    contraction_momentum: s,
                    differentiated_loop: loop_index,
                },
                &recenter,
                checked(family.derivative_contraction(protected, loop_index, q))?,
                &scale,
            )?;
        }
    }
    require(
        !b.weights.is_empty(),
        "gradient vector field vanished; nomination miss",
    )?;
    let sources = b.weights.iter().map(|((row,offset),weight)|json!({"source_row":row.stable_string(),"offset":offset.values(),"weight":coefficient(weight)})).collect::<Vec<_>>();
    let components = family.contraction_momenta().iter().enumerate().map(|(s,q)|json!({"contraction_momentum":s,"momentum":format!("{q:?}"),"g":coefficient(&g[s]),"p":coefficient(&p[s])})).collect::<Vec<_>>();
    let report = json!({"method":"one-protected-gradient-affine-v1","differentiated_loop":loop_index,"numerator_axis":numerator,
        "protected_active_axis":protected,"dependent_axes":(0..n).filter(|&i|dependent[i]).collect::<Vec<_>>(),
        "inactive_spectator_axes":(0..n).filter(|&i|!active[i]&&!dependent[i]).collect::<Vec<_>>(),
        "other_dependent_inactive_axes":(0..n).filter(|&i|!active[i]&&dependent[i]&&i!=numerator).collect::<Vec<_>>(),
        "components":components,"recenter":recenter,"sources":sources,"nomination_operations":b.operations,
        "native_coefficients_kept_in_process":true,"mixed_tail_terms_suppressed":false,"nomination_is_source_proof":false,
        "target_coefficient_assumed":false,"descent_assumed":false,"gram_division":false});
    Ok(Nomination {
        weights: b.weights,
        conditions: b.conditions,
        report,
    })
}

#[cfg(test)]
#[path = "geometry_gradient_tests.rs"]
mod tests;
