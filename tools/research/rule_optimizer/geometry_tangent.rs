//! A bounded source nomination, not a reduction/proof service.
//!
//! In one caller-selected coordinate loop direction, find the unique dependent
//! active denominator and require its gradient to be radial. Derive p from the
//! selected numerator's off-diagonal coefficients, then expand
//! V = k^2 p - (k.p) k in the authenticated affine denominator basis. Each
//! constant/linear monomial is an ordinary IBP source at recenter - monomial.
//! The ordinary generator supplies the COMPLETE product rule, including other
//! dependent numerators. No incidence classification authorizes a chart.

use super::{Result, checked, coefficient, require};
use rustred::{
    algebra::{
        ExactAlgebraLimits, IndexedCoefficient, IndexedCoefficientContext, IndexedPolynomial,
    },
    family::{ContractionMomentum, DenominatorExpansion, IntegralFamily, ScalarProductCoordinate},
    identity::{IntegralShift, RowId},
};
use serde_json::{Value, json};
use std::collections::BTreeMap;

#[path = "geometry_gradient.rs"]
pub(super) mod gradient;

pub(super) const SCHEMA: &str = "rustred.geometry-tangent-chart.v1";

#[derive(Clone, Copy)]
pub(super) struct Limits {
    pub arithmetic: ExactAlgebraLimits,
    pub max_sources: usize,
    pub max_conditions: usize,
    pub max_operations: usize,
}

pub(super) struct Nomination {
    pub weights: BTreeMap<(RowId, IntegralShift), IndexedCoefficient>,
    pub conditions: Vec<IndexedPolynomial>,
    pub report: Value,
}

struct Builder<'a> {
    c: &'a IndexedCoefficientContext,
    limits: Limits,
    weights: BTreeMap<(RowId, IntegralShift), IndexedCoefficient>,
    conditions: Vec<IndexedPolynomial>,
    operations: usize,
}

impl Builder<'_> {
    fn charge(&mut self, count: usize) -> Result<()> {
        self.operations = self
            .operations
            .checked_add(count)
            .ok_or("nomination work overflow")?;
        require(
            self.operations <= self.limits.max_operations,
            "nomination exceeds work allowance",
        )
    }

    fn retain(&mut self, value: &IndexedCoefficient) -> Result<()> {
        require(
            self.conditions.len() < self.limits.max_conditions,
            "nomination conditions exceed allowance",
        )?;
        self.conditions.push(checked(
            self.c
                .denominator_condition_with_limits(value, self.limits.arithmetic),
        )?);
        Ok(())
    }

    fn affine_source(
        &mut self,
        row: RowId,
        recenter: &[i64],
        expansion: &DenominatorExpansion,
        scale: &IndexedCoefficient,
    ) -> Result<()> {
        // Record factors BEFORE multiplication/coalescing can cancel a pole.
        self.retain(scale)?;
        for (monomial, raw) in std::iter::once((None, expansion.constant())).chain(
            expansion
                .denominator_coefficients()
                .iter()
                .enumerate()
                .map(|(i, c)| (Some(i), c)),
        ) {
            self.charge(2)?;
            let factor = checked(self.c.lift(raw))?;
            self.retain(&factor)?;
            let value = checked(
                self.c
                    .mul_with_limits(scale, &factor, self.limits.arithmetic),
            )?;
            self.retain(&value)?;
            if value.is_zero() {
                continue;
            }
            let mut offset = recenter.to_vec();
            if let Some(axis) = monomial {
                offset[axis] -= 1;
            }
            let key = (row.clone(), checked(IntegralShift::try_new(offset))?);
            let value = match self.weights.remove(&key) {
                Some(old) => checked(self.c.add_with_limits(&old, &value, self.limits.arithmetic))?,
                None => value,
            };
            if !value.is_zero() {
                self.weights.insert(key, value);
            }
            require(
                self.weights.len() <= self.limits.max_sources,
                "nominated sources exceed allowance",
            )?;
        }
        Ok(())
    }
}

fn affine_is_zero(x: &DenominatorExpansion) -> bool {
    x.constant().is_zero() && x.denominator_coefficients().iter().all(|c| c.is_zero())
}

fn other_momentum(
    coordinate: ScalarProductCoordinate,
    loop_index: usize,
) -> Option<ContractionMomentum> {
    match coordinate {
        ScalarProductCoordinate::LoopLoop { left, right }
            if left != right && left == loop_index =>
        {
            Some(ContractionMomentum::Loop(right))
        }
        ScalarProductCoordinate::LoopLoop { left, right }
            if left != right && right == loop_index =>
        {
            Some(ContractionMomentum::Loop(left))
        }
        ScalarProductCoordinate::LoopExternal {
            loop_index: i,
            external_index,
        } if i == loop_index => Some(ContractionMomentum::External(external_index)),
        _ => None,
    }
}

pub(super) fn nominate(
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
        "nomination axis/loop/owner mismatch",
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
        "nomination requires exactly one active denominator depending on the chosen loop",
    )?;
    let protected = protected[0];
    let radial = checked(family.coordinate_index(ScalarProductCoordinate::LoopLoop {
        left: loop_index,
        right: loop_index,
    }))?;
    let protected_coefficients = family.denominators()[protected].coefficients();
    require(
        !protected_coefficients[radial].is_zero(),
        "protected active denominator has no radial coefficient",
    )?;
    for (&coordinate, value) in family.coordinates().iter().zip(protected_coefficients) {
        require(
            other_momentum(coordinate, loop_index).is_none() || value.is_zero(),
            "protected active denominator has non-radial loop dependence",
        )?;
    }
    let square = checked(family.scalar_product_expansion(radial))?;
    let radial_contraction = family
        .contraction_momenta()
        .iter()
        .position(|&q| q == ContractionMomentum::Loop(loop_index))
        .ok_or("radial contraction missing")?;
    let mut recenter = vec![0_i64; n];
    recenter[numerator] = 1;
    let mut p = Vec::new();
    for (&coordinate, value) in family
        .coordinates()
        .iter()
        .zip(family.denominators()[numerator].coefficients())
    {
        let Some(momentum) = other_momentum(coordinate, loop_index) else {
            continue;
        };
        if value.is_zero() {
            continue;
        }
        b.charge(2)?;
        let value = checked(c.lift(value))?;
        b.retain(&value)?;
        let weight = checked(c.div_with_limits(&value, &c.integer(2), limits.arithmetic))?;
        b.retain(&weight)?;
        let contraction = family
            .contraction_momenta()
            .iter()
            .position(|&q| q == momentum)
            .ok_or("cross contraction missing")?;
        p.push(json!({"contraction_momentum":contraction,"momentum":format!("{momentum:?}"),"weight":coefficient(&weight)}));
        b.affine_source(
            RowId::OrdinaryIbp {
                contraction_momentum: contraction,
                differentiated_loop: loop_index,
            },
            &recenter,
            &square,
            &weight,
        )?;
        let dot = checked(
            family.scalar_product_expansion(checked(family.coordinate_index(coordinate))?),
        )?;
        let minus_weight = checked(c.neg_with_limits(&weight, limits.arithmetic))?;
        b.affine_source(
            RowId::OrdinaryIbp {
                contraction_momentum: radial_contraction,
                differentiated_loop: loop_index,
            },
            &recenter,
            &dot,
            &minus_weight,
        )?;
    }
    require(
        !p.is_empty() && !b.weights.is_empty(),
        "selected numerator has no usable off-diagonal loop dependence",
    )?;
    let sources = b.weights.iter().map(|((row, offset), weight)| json!({"source_row":row.stable_string(),"offset":offset.values(),"weight":coefficient(weight)})).collect::<Vec<_>>();
    let report = json!({"method":"coordinate-radial-tangent-affine-v1","differentiated_loop":loop_index,"numerator_axis":numerator,
        "protected_active_axis":protected,"dependent_axes":(0..n).filter(|&i|dependent[i]).collect::<Vec<_>>(),
        "inactive_spectator_axes":(0..n).filter(|&i|!active[i]&&!dependent[i]).collect::<Vec<_>>(),
        "other_dependent_inactive_axes":(0..n).filter(|&i|!active[i]&&dependent[i]&&i!=numerator).collect::<Vec<_>>(),
        "p":p,"recenter":recenter,"sources":sources,"nomination_operations":b.operations,
        "native_coefficients_kept_in_process":true,"mixed_tail_terms_suppressed":false,"nomination_is_source_proof":false});
    Ok(Nomination {
        weights: b.weights,
        conditions: b.conditions,
        report,
    })
}

#[cfg(test)]
#[path = "geometry_tangent_tests.rs"]
mod tests;
