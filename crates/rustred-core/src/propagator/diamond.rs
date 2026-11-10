//! The Diamond lane (owner: M3-T5b rr-diamond): Forcer's diamond rule as a
//! template applied by our own step, the pattern of the Triangle lane
//! (architecture section 3.5; Ruijl, Ueda, Vermaseren arXiv:1504.08258, thesis
//! eqs. 26-31, pp. 47-49; Forcer's detection pathforcer.py findDiamond).
//!
//! A diamond has `L + 1` paths top - `x_i` - bottom through internal 3-valent
//! vertices `x_i`, and at most one spectator line top - bottom ([`DiamondRoles`]).
//! In the diamond-adapted presentation the loops `0..L + S` are `k_i = mom(b_i)`
//! (`i < L`) and the spectator's `l`; the last lower line carries
//! `k_L = -Sum k_i - l`, each upper line `a_i = (k_i + p_i)^2` with
//! `p_i = +-mom(c_i)`, and every other line is free of the diamond loops
//! ([`check_diamond_contract`]). The sum over the diamond loops of the rows
//! `(d/dk_i).k_i` of RustRed's own IBP generator then reads
//!
//! ```text
//! E(n) I(n) = Sum_i X_i(n) A_i+ (B_i- - C_i-) I(n)
//! E(n)   = (L + S) d - Sum_i (n_ai + j_ai eps) - 2 Sum_i n_bi - 2 (n_s + j_s eps)
//!          - Sum_ISP deg(ISP) n_ISP
//! X_i(n) = n_ai + j_ai eps,   eps = (4 - d) / 2
//! ```
//!
//! with `deg` the degree of an ISP in the diamond loops (0, 1 or 2) and `j` the
//! eps units of a line. `b_i` and `c_i` are integer lines; the upper lines and the
//! spectator may carry eps.
//!
//! - [`DiamondTemplate::extract`] sums the generator's rows, reads `E` and `X_i`
//!   from the sum and checks its shape against the formula above. The formula
//!   only validates: every weight of a step is evaluated from the row. A row of
//!   another shape is a [`DiamondError::Mismatch`]; the node then falls back to the
//!   Custom lane and assembly records the reason.
//! - [`DiamondStep`] maps `I(n)` to `2 (L + 1)` children weighted by
//!   `+-X_i(n) / E(n)`. A child with a `b_i` or `c_i` at power 0 is a
//!   [`PStage::Route`] key, a subsector of the presentation; the others are
//!   [`PStage::Apply`] keys of the same owner. Every child lowers
//!   `Sum_i (n_bi + n_ci)` by exactly one, the first entry of the step's order.
//!   The children stay on the owner presentation, so the lane needs no edge map,
//!   as the Triangle lane.
//! - The `d` coefficient of `E` is `L + S + Sum j_a / 2 + j_s >= 1`: `E` is never
//!   the zero polynomial, the rule has no exception and the step reports no
//!   condition. `X_i` is never zero on an Apply key.
//!
//! [`P_DIAMOND_TEMPLATE_VERSION`] names this meaning of a recorded step: the
//! rows, their shape, the stages of the children. Bump it on any semantic
//! change.
//!
//! [`P_DIAMOND_TEMPLATE_VERSION`]: super::P_DIAMOND_TEMPLATE_VERSION
use std::collections::BTreeMap;
use std::fmt;
use std::sync::Arc;

use crate::algebra::{
    Coefficient, CoefficientContext, ExactAlgebraError, ExactAlgebraLimits, IndexedAlgebraError,
    IndexedAlgebraLimits, IndexedCoefficient, IndexedCoefficientContext,
};
use crate::identity::{ParametricIbpGenerator, ParametricRelation, RowId};
use crate::reduction::{ReductionError, ReductionLimits};

use super::PresentationId;
use super::error::PError;
use super::lane::{DiamondRoles, LaneRoles, OwnerLane};
use super::library::{PLibrary, POwnerSpec};
use super::presentation::{Presentation, Slot, eps_power_shift};
use super::step::{PChild, PKeyView, PLaneStep, POrderKey, PStage, PStepContext, PStepOut};
use super::triangle::{RowForm, RowTerm};

/// Why no diamond template exists. Only a mismatch says something about the
/// lane.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum DiamondError {
    /// The rows of the presentation do not have the template's shape, or the
    /// roles do not fit it: the node falls back to Custom with this reason.
    Mismatch {
        presentation: PresentationId,
        reason: String,
    },
    /// A failure of the run: stop, never change the lane.
    Failure(PError),
}

impl fmt::Display for DiamondError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Mismatch {
                presentation,
                reason,
            } => write!(
                formatter,
                "diamond template of presentation {}: {reason}",
                presentation.0
            ),
            Self::Failure(error) => error.fmt(formatter),
        }
    }
}

impl std::error::Error for DiamondError {}

impl From<PError> for DiamondError {
    fn from(error: PError) -> Self {
        Self::Failure(error)
    }
}

/// `PError::Contract` is the lane's fault everywhere in the p library.
impl From<DiamondError> for PError {
    fn from(error: DiamondError) -> Self {
        match error {
            mismatch @ DiamondError::Mismatch { .. } => PError::Contract(mismatch.to_string()),
            DiamondError::Failure(error) => error,
        }
    }
}

fn exact(error: ExactAlgebraError) -> PError {
    ReductionError::ExactAlgebra(error).into()
}

fn indexed(error: IndexedAlgebraError) -> PError {
    ReductionError::IndexedAlgebra(error).into()
}

fn up_to_sign(a: &[i64], b: &[i64]) -> bool {
    a == b || a.iter().zip(b).all(|(x, y)| *x == -*y)
}

fn unit(width: usize, position: usize) -> Vec<i64> {
    let mut row = vec![0; width];
    row[position] = 1;
    row
}

/// Every role slot, `a`, `b`, `c`, then the spectator.
fn role_slots(r: &DiamondRoles) -> Vec<u16> {
    (r.a().iter().chain(r.b()).chain(r.c()).chain(&r.spectator))
        .copied()
        .collect()
}

/// The diamond's lines and loops on `p`, see the module docs: the loops
/// `0..L + S` run through `b_i` (`i < L`) and the spectator, the last `b`
/// carries each of them with a unit coefficient and nothing else,
/// `a_i = +-(b_i +- c_i)`, `b` and `c` are integer lines, no
/// other line carries a diamond loop, and every factor of an ISP is free of the
/// diamond loops or free of everything else.
pub fn check_diamond_contract(p: &Presentation, r: &DiamondRoles) -> Result<(), PError> {
    p.validate()?;
    let fail = |detail: String| {
        PError::Contract(format!(
            "diamond contract, presentation {}: {detail}",
            p.id.0
        ))
    };
    let loops = r.loops();
    let width = usize::from(p.loops) + 1;
    if r.paths() < 2 || loops > usize::from(p.loops) {
        return Err(fail(format!(
            "{} paths and {loops} diamond loops on {} loops",
            r.paths(),
            p.loops
        )));
    }
    let roles = role_slots(r);
    for (index, slot) in roles.iter().enumerate() {
        if roles[..index].contains(slot) {
            return Err(fail(format!("slot {slot} has two roles")));
        }
    }
    let line = |slot: u16| match p.slots.get(usize::from(slot)) {
        Some(Slot::Line(line)) => Ok(line),
        _ => Err(fail(format!("slot {slot} is not a line"))),
    };
    let last = r.paths() - 1;
    for (path, &b) in r.b().iter().enumerate() {
        let line_b = line(b)?;
        // `b_i = +-k_i`; every diamond loop runs through the last `b`, each in
        // the direction of its own lower line.
        let holds = if path < last {
            up_to_sign(&line_b.momentum, &unit(width, path))
        } else {
            let (inner, outer) = line_b.momentum.split_at(loops);
            inner.iter().all(|entry| entry.abs() == 1) && outer.iter().all(|&entry| entry == 0)
        };
        if !holds {
            return Err(fail(format!("b[{path}] is {:?}", line_b.momentum)));
        }
        if line_b.eps_units != 0 {
            return Err(fail(format!("b[{path}] carries eps")));
        }
    }
    if let Some(spectator) = r.spectator {
        let line_s = line(spectator)?;
        if !up_to_sign(&line_s.momentum, &unit(width, last)) {
            return Err(fail(format!("the spectator is {:?}", line_s.momentum)));
        }
    }
    for path in 0..r.paths() {
        let (a, b, c) = (r.a()[path], r.b()[path], r.c()[path]);
        let (line_a, line_b, line_c) = (line(a)?, line(b)?, line(c)?);
        let (inner_a, inner_b) = (&line_a.momentum[..loops], &line_b.momentum[..loops]);
        let sign = if inner_a == inner_b {
            1
        } else if up_to_sign(inner_a, inner_b) {
            -1
        } else {
            return Err(fail(format!(
                "line {a} is {:?}, line {b} is {:?}",
                line_a.momentum, line_b.momentum
            )));
        };
        let rest: Vec<i64> = (line_a.momentum.iter().zip(&line_b.momentum))
            .map(|(x, y)| x - sign * y)
            .collect();
        if !up_to_sign(&rest, &line_c.momentum) {
            return Err(fail(format!(
                "line {a} is +-line {b} + {rest:?}, line {c} is {:?}",
                line_c.momentum
            )));
        }
        if line_c.eps_units != 0 {
            return Err(fail(format!("c[{path}] carries eps")));
        }
    }
    let carriers: Vec<u16> = (r.a().iter().chain(r.b()).chain(&r.spectator))
        .copied()
        .collect();
    for (index, line) in p.lines() {
        let role = u16::try_from(index).is_ok_and(|index| carriers.contains(&index));
        if !role && line.momentum[..loops].iter().any(|&entry| entry != 0) {
            return Err(fail(format!("line {index} carries a diamond loop")));
        }
    }
    for slot in &p.slots {
        let Slot::Isp(isp) = slot else {
            continue;
        };
        for side in [&isp.left, &isp.right] {
            let inner = side[..loops].iter().any(|&entry| entry != 0);
            let outer = side[loops..].iter().any(|&entry| entry != 0);
            if inner && outer {
                return Err(fail(format!(
                    "the ISP {:?}.{:?} mixes diamond loops with other momenta",
                    isp.left, isp.right
                )));
            }
        }
    }
    Ok(())
}

/// The row was built without dividing by anything that depends on `d` or `n`.
fn plain_row(relation: &ParametricRelation) -> bool {
    relation
        .nonzero_conditions()
        .iter()
        .all(|condition| condition.polynomial().is_nonzero_constant())
}

fn integer_value(value: &Coefficient) -> Option<i64> {
    if !value.denominator.is_one() || !value.numerator.is_constant() {
        return None;
    }
    value.numerator.get_constant().to_i64()
}

/// The entries of the summed row
/// `E I(n) + Sum_i (R_i A_i+ B_i- + S_i A_i+ C_i-) I(n) = 0`, path by path.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DiamondRowForms {
    pub pivot: RowForm,
    /// `R_i`.
    pub lower_b: Vec<RowForm>,
    /// `S_i`.
    pub lower_c: Vec<RowForm>,
}

/// `I(n) = Sum_terms weight I(powers)`, read from the summed rows of a
/// diamond-adapted presentation, see the module docs. Valid at every integer
/// `n`: the stages of a step are [`DiamondStep`]'s concern.
#[derive(Clone, Debug)]
pub struct DiamondTemplate {
    presentation: PresentationId,
    roles: DiamondRoles,
    context: CoefficientContext,
    arity: usize,
    pivot: RowForm,
    apex: Vec<RowForm>,
}

/// A template in the making, on roles that meet the lane contract.
struct Extraction<'a> {
    presentation: &'a Presentation,
    roles: &'a DiamondRoles,
}

impl<'a> Extraction<'a> {
    fn new(presentation: &'a Presentation, roles: &'a DiamondRoles) -> Result<Self, DiamondError> {
        let extraction = Self {
            presentation,
            roles,
        };
        match check_diamond_contract(presentation, roles) {
            Ok(()) => Ok(extraction),
            Err(PError::Contract(reason)) => Err(extraction.mismatch(reason)),
            Err(error) => Err(error.into()),
        }
    }

    fn mismatch(&self, reason: impl Into<String>) -> DiamondError {
        DiamondError::Mismatch {
            presentation: self.presentation.id,
            reason: reason.into(),
        }
    }

    /// The sum of `relations`, shift by shift, without its zero entries.
    fn summed(
        &self,
        context: &IndexedCoefficientContext,
        relations: &[&ParametricRelation],
    ) -> Result<BTreeMap<Vec<i64>, IndexedCoefficient>, DiamondError> {
        let mut sum: BTreeMap<Vec<i64>, IndexedCoefficient> = BTreeMap::new();
        for relation in relations {
            if !plain_row(relation) {
                return Err(self.mismatch("a row carries a nonconstant condition"));
            }
            for (shift, value) in relation.terms() {
                let entry = sum
                    .entry(shift.values().to_vec())
                    .or_insert_with(|| context.zero());
                *entry = context.add(entry, value).map_err(indexed)?;
            }
        }
        sum.retain(|_, value| !value.is_zero());
        Ok(sum)
    }

    /// The `1 + 2 (L + 1)` entries of the summed row as forms, which must be its
    /// only terms.
    fn row_forms(
        &self,
        context: &IndexedCoefficientContext,
        row: &BTreeMap<Vec<i64>, IndexedCoefficient>,
    ) -> Result<DiamondRowForms, DiamondError> {
        let paths = self.roles.paths();
        if row.len() != 1 + 2 * paths {
            return Err(self.mismatch(format!(
                "the summed row has {} terms, not one at zero shift and {} shifted",
                row.len(),
                2 * paths
            )));
        }
        let shift = |raised: Option<u16>, lowered: Option<u16>| {
            (0..context.index_count())
                .map(|slot| {
                    i64::from(raised.map(usize::from) == Some(slot))
                        - i64::from(lowered.map(usize::from) == Some(slot))
                })
                .collect::<Vec<i64>>()
        };
        let entry = |raised, lowered| {
            let wanted = shift(raised, lowered);
            match row.get(&wanted) {
                Some(value) => self.lift(context, value),
                None => {
                    Err(self.mismatch(format!("the summed row has no term at shift {wanted:?}")))
                }
            }
        };
        let mut forms = DiamondRowForms {
            pivot: entry(None, None)?,
            lower_b: Vec::with_capacity(paths),
            lower_c: Vec::with_capacity(paths),
        };
        for path in 0..paths {
            let a = Some(self.roles.a()[path]);
            forms.lower_b.push(entry(a, Some(self.roles.b()[path]))?);
            forms.lower_c.push(entry(a, Some(self.roles.c()[path]))?);
        }
        Ok(forms)
    }

    /// The form of a row entry: its values at the zero point and the unit points,
    /// with the check that they rebuild the entry, a polynomial of `Q(d)[n]`.
    fn lift(
        &self,
        context: &IndexedCoefficientContext,
        value: &IndexedCoefficient,
    ) -> Result<RowForm, DiamondError> {
        let limits = IndexedAlgebraLimits::default();
        let base = context.base();
        let mut point = vec![0_i64; context.index_count()];
        let at_point = |point: &[i64]| {
            let (value, _) = context.specialize(value, point, limits).map_err(indexed)?;
            Ok::<_, PError>(value)
        };
        let constant = at_point(&point)?;
        let mut slopes = Vec::new();
        let mut rebuilt = context.lift(&constant).map_err(indexed)?;
        for slot in 0..point.len() {
            point[slot] = 1;
            let difference = base
                .try_sub(&at_point(&point)?, &constant, limits.exact_algebra)
                .map_err(exact)?;
            point[slot] = 0;
            if difference.is_zero() {
                continue;
            }
            let slope = integer_value(&difference).ok_or_else(|| {
                self.mismatch(format!("the slope {difference} of slot {slot} in the row"))
            })?;
            let term = context
                .mul(
                    &context.integer(slope),
                    &context.index(slot).map_err(indexed)?,
                )
                .map_err(indexed)?;
            rebuilt = context.add(&rebuilt, &term).map_err(indexed)?;
            slopes.push((u16::try_from(slot).expect("at most 64 slots"), slope));
        }
        if rebuilt != *value {
            return Err(self.mismatch("a row entry is not affine in the powers"));
        }
        Ok(RowForm { constant, slopes })
    }

    fn eps_units(&self, slot: u16) -> Result<u8, DiamondError> {
        match self.presentation.slots.get(usize::from(slot)) {
            Some(Slot::Line(line)) => Ok(line.eps_units),
            _ => Err(self.mismatch(format!("slot {slot} is not a line"))),
        }
    }

    /// `X_i`: `n_ai + j_ai (4 - d) / 2`.
    fn expected_apex(&self, line: u16) -> Result<RowForm, DiamondError> {
        let context = self.presentation.family.coefficient_context();
        Ok(RowForm {
            constant: eps_power_shift(context, self.eps_units(line)?)?,
            slopes: vec![(line, 1)],
        })
    }

    /// `E`, see the module docs.
    fn expected_pivot(&self) -> Result<RowForm, DiamondError> {
        let context = self.presentation.family.coefficient_context();
        let roles = self.roles;
        let mut units = 0_u8;
        for &a in roles.a() {
            units = units
                .checked_add(self.eps_units(a)?)
                .ok_or_else(|| self.mismatch("too many eps units"))?;
        }
        if let Some(spectator) = roles.spectator {
            let twice = self.eps_units(spectator)?.checked_mul(2);
            units = twice
                .and_then(|twice| units.checked_add(twice))
                .ok_or_else(|| self.mismatch("too many eps units"))?;
        }
        let loops = i64::try_from(roles.loops()).expect("at most 8 diamond loops");
        let d = context.parameter("d").ok_or(PError::ContextMismatch)?;
        let limits = ExactAlgebraLimits::default();
        let scaled = context
            .try_mul(&context.integer(loops), &d, limits)
            .map_err(exact)?;
        let constant = context
            .try_sub(&scaled, &eps_power_shift(context, units)?, limits)
            .map_err(exact)?;
        let degree =
            |side: &[i64]| i64::from(side[..roles.loops()].iter().any(|&entry| entry != 0));
        let mut slopes: Vec<(u16, i64)> = Vec::with_capacity(self.presentation.slots.len());
        for (slot, entry) in self.presentation.slots.iter().enumerate() {
            let slot = u16::try_from(slot).expect("at most 64 slots");
            let slope = match entry {
                Slot::Line(_) if roles.a().contains(&slot) => -1,
                Slot::Line(_) if roles.b().contains(&slot) || roles.spectator == Some(slot) => -2,
                Slot::Line(_) => 0,
                Slot::Isp(isp) => -(degree(&isp.left) + degree(&isp.right)),
            };
            if slope != 0 {
                slopes.push((slot, slope));
            }
        }
        Ok(RowForm { constant, slopes })
    }

    /// The shape check: the forms against the formula of the module docs.
    fn validated(&self, forms: DiamondRowForms) -> Result<DiamondTemplate, DiamondError> {
        let presentation = self.presentation;
        let context = presentation.family.coefficient_context();
        let compare = |what: String, found: &RowForm, expected: &RowForm| {
            if found == expected {
                Ok(())
            } else {
                Err(self.mismatch(format!(
                    "{what} of the summed row is {found:?}, the template has {expected:?}"
                )))
            }
        };
        let paths = self.roles.paths();
        if forms.lower_b.len() != paths || forms.lower_c.len() != paths {
            return Err(self.mismatch(format!(
                "{} and {} shifted entries for {paths} paths",
                forms.lower_b.len(),
                forms.lower_c.len()
            )));
        }
        compare(
            "the zero-shift entry".to_owned(),
            &forms.pivot,
            &self.expected_pivot()?,
        )?;
        let mut apex = Vec::with_capacity(paths);
        for path in 0..paths {
            let expected = self.expected_apex(self.roles.a()[path])?;
            let lower_b = &forms.lower_b[path];
            compare(
                format!("A{path}+ B{path}-"),
                lower_b,
                &negated(context, &expected)?,
            )?;
            compare(
                format!("A{path}+ C{path}-"),
                &forms.lower_c[path],
                &expected,
            )?;
            // `X_i` as the row has it: minus the entry of `A_i+ B_i-`.
            apex.push(negated(context, lower_b)?);
        }
        Ok(DiamondTemplate {
            presentation: presentation.id,
            roles: *self.roles,
            context: context.clone(),
            arity: presentation.slots.len(),
            pivot: forms.pivot,
            apex,
        })
    }
}

fn negated(context: &CoefficientContext, form: &RowForm) -> Result<RowForm, PError> {
    let constant = context
        .try_neg(&form.constant, ExactAlgebraLimits::default())
        .map_err(exact)?;
    let slopes = form.slopes.iter().map(|&(slot, slope)| (slot, -slope));
    Ok(RowForm {
        constant,
        slopes: slopes.collect(),
    })
}

impl DiamondTemplate {
    /// The template of `(p, roles)`, from the sum of the rows `(d/dk_i).k_i`
    /// over the diamond loops `i < L + S`.
    ///
    /// The rows are the generator's own, by their [`RowId`]s, as for the
    /// Triangle template.
    pub fn extract(p: &Presentation, roles: &DiamondRoles) -> Result<Self, DiamondError> {
        let extraction = Extraction::new(p, roles)?;
        let generation = |error: &dyn fmt::Display| {
            PError::Generation(format!("the IBP rows of presentation {}: {error}", p.id.0))
        };
        let generator =
            ParametricIbpGenerator::try_new(&p.family).map_err(|error| generation(&error))?;
        let batch = generator
            .prepare_ordinary_ibp()
            .map_err(|error| generation(&error))?;
        let rows = (0..batch.len()).map(|row| batch.generate(row)).collect();
        let relations = batch
            .complete(rows)
            .map_err(|error| generation(&error))?
            .into_relations();
        let mut chosen = Vec::with_capacity(roles.loops());
        for loop_index in 0..roles.loops() {
            let wanted = RowId::OrdinaryIbp {
                contraction_momentum: loop_index,
                differentiated_loop: loop_index,
            };
            let relation = relations
                .iter()
                .find(|relation| *relation.row_id() == wanted)
                .ok_or_else(|| {
                    extraction.mismatch(format!(
                        "the generator has no row (d/dk{loop_index}).k{loop_index}"
                    ))
                })?;
            chosen.push(relation);
        }
        let context = generator.context();
        let row = extraction.summed(context, &chosen)?;
        let forms = extraction.row_forms(context, &row)?;
        extraction.validated(forms)
    }

    /// [`Self::extract`] from the given rows of `generator`, whatever they are.
    /// Tests of the shape check use it.
    #[doc(hidden)]
    pub fn from_relations(
        p: &Presentation,
        roles: &DiamondRoles,
        generator: &ParametricIbpGenerator<'_>,
        relations: &[&ParametricRelation],
    ) -> Result<Self, DiamondError> {
        let extraction = Extraction::new(p, roles)?;
        let row = extraction.summed(generator.context(), relations)?;
        let forms = extraction.row_forms(generator.context(), &row)?;
        extraction.validated(forms)
    }

    /// The shape check alone, on forms given by hand. Tests of the shape check
    /// use it.
    #[doc(hidden)]
    pub fn from_forms(
        p: &Presentation,
        roles: &DiamondRoles,
        forms: DiamondRowForms,
    ) -> Result<Self, DiamondError> {
        Extraction::new(p, roles)?.validated(forms)
    }

    pub fn presentation(&self) -> PresentationId {
        self.presentation
    }

    pub fn roles(&self) -> &DiamondRoles {
        &self.roles
    }

    /// `E`, the entry of the summed row at zero shift.
    pub fn pivot(&self) -> &RowForm {
        &self.pivot
    }

    /// `X_i`, the common factor of the terms of path `i`.
    pub fn apex(&self) -> &[RowForm] {
        &self.apex
    }

    /// The solved row at `powers`, for any integer powers: per path `i` the term
    /// lowering `b_i`, then the one lowering `c_i`.
    pub fn solve(
        &self,
        powers: &[i64],
        limits: ExactAlgebraLimits,
    ) -> Result<Vec<RowTerm>, PError> {
        if powers.len() != self.arity {
            return Err(PError::Contract(format!(
                "a key of {} powers on the diamond template of {}, arity {}",
                powers.len(),
                self.presentation.0,
                self.arity
            )));
        }
        let context = &self.context;
        let pivot = self.pivot.at(context, powers, limits)?;
        let term = |raised: u16, lowered: u16, weight: Coefficient| {
            let mut child = powers.to_vec();
            let (up, down) = (usize::from(raised), usize::from(lowered));
            child[up] = child[up].checked_add(1).ok_or_else(overflow)?;
            child[down] = child[down].checked_sub(1).ok_or_else(overflow)?;
            Ok::<_, PError>(RowTerm {
                powers: child,
                weight,
                lowered,
            })
        };
        let mut terms = Vec::with_capacity(2 * self.apex.len());
        for (path, apex) in self.apex.iter().enumerate() {
            let value = apex.at(context, powers, limits)?;
            let weight = context.try_div(&value, &pivot, limits).map_err(exact)?;
            let minus = context.try_neg(&weight, limits).map_err(exact)?;
            let a = self.roles.a()[path];
            terms.push(term(a, self.roles.b()[path], weight)?);
            terms.push(term(a, self.roles.c()[path], minus)?);
        }
        Ok(terms)
    }
}

fn overflow() -> PError {
    PError::Unsupported("a power of a diamond key overflows".to_owned())
}

/// The Diamond program of one owner, see the module docs.
#[derive(Debug)]
pub struct DiamondStep {
    presentation: Arc<Presentation>,
    template: DiamondTemplate,
    lines: usize,
    forced: u64,
}

impl DiamondStep {
    pub fn new(presentation: Arc<Presentation>, roles: DiamondRoles) -> Result<Self, DiamondError> {
        let template = DiamondTemplate::extract(&presentation, &roles)?;
        let lines = presentation.lines().count();
        let forced = presentation.forced_axes();
        Ok(Self {
            presentation,
            template,
            lines,
            forced,
        })
    }

    /// [`Self::new`] on an owner of a library: a mismatch is
    /// [`PError::Contract`], since assembly has settled the lane before.
    pub fn from_owner(library: &PLibrary, owner: &POwnerSpec) -> Result<Self, PError> {
        let LaneRoles::Diamond(roles) = owner.roles else {
            return Err(PError::Contract(format!(
                "a diamond step with roles {:?}",
                owner.roles
            )));
        };
        if owner.lane != OwnerLane::Diamond || !owner.edge_maps.is_empty() {
            return Err(PError::Contract(format!(
                "a diamond step on lane {:?} with {} edge maps",
                owner.lane,
                owner.edge_maps.len()
            )));
        }
        Ok(Self::new(
            Arc::clone(library.presentation(owner.presentation)),
            roles,
        )?)
    }

    pub fn presentation(&self) -> &Arc<Presentation> {
        &self.presentation
    }

    pub fn template(&self) -> &DiamondTemplate {
        &self.template
    }

    /// `Sum_i (n_bi + n_ci)`, which every step lowers by one.
    pub fn measure(&self, powers: &[i64]) -> Result<i64, PError> {
        let roles = self.template.roles();
        let mut lowered = 0_i64;
        for &slot in roles.b().iter().chain(roles.c()) {
            let power = powers.get(usize::from(slot)).ok_or_else(|| {
                PError::Contract(format!(
                    "diamond slot {slot} on a key of {} slots",
                    powers.len()
                ))
            })?;
            lowered = lowered.saturating_add(*power);
        }
        Ok(lowered)
    }

    /// An Apply key: every integer line at a positive power, no ISP in a
    /// denominator. The Ordered adapter routes everything else.
    fn check_key(&self, key: &PKeyView<'_>) -> Result<(), PError> {
        if key.presentation != self.presentation.id
            || key.powers.len() != self.presentation.slots.len()
        {
            return Err(PError::Contract(format!(
                "key of presentation {} with {} powers on diamond step {}",
                key.presentation.0,
                key.powers.len(),
                self.presentation.id.0
            )));
        }
        for (slot, &power) in key.powers.iter().enumerate() {
            if slot >= self.lines {
                if power > 0 {
                    return Err(PError::Unsupported(format!(
                        "ISP slot {slot} at the positive power {power}"
                    )));
                }
            } else if self.forced >> slot & 1 == 0 && power < 1 {
                return Err(PError::Contract(format!(
                    "line {slot} at power {power} in an Apply key of diamond step {}",
                    self.presentation.id.0
                )));
            }
        }
        Ok(())
    }

    /// The step on `key`, without a library context. The tag of the key passes
    /// through unchanged: the rule integrates nothing.
    pub fn apply(&self, key: &PKeyView<'_>, limits: &ReductionLimits) -> Result<PStepOut, PError> {
        self.check_key(key)?;
        let terms = self.template.solve(key.powers, limits.exact_algebra)?;
        let child = |term: RowTerm| {
            let stage = if term.powers[usize::from(term.lowered)] == 0 {
                PStage::Route
            } else {
                PStage::Apply
            };
            PChild {
                stage,
                presentation: self.presentation.id,
                powers: term.powers,
                tag: key.tag.clone(),
                coefficient: term.weight,
            }
        };
        Ok(PStepOut {
            children: terms.into_iter().map(child).collect(),
            ..PStepOut::default()
        })
    }
}

impl PLaneStep for DiamondStep {
    fn lane(&self) -> OwnerLane {
        OwnerLane::Diamond
    }

    fn step(&self, cx: &PStepContext<'_>, key: &PKeyView<'_>) -> Result<PStepOut, PError> {
        self.apply(key, cx.limits)
    }

    /// `Sum_i (n_bi + n_ci)`, then the distance from the corner of the sector,
    /// as [`super::step::default_order_key`] has it.
    fn order_key(&self, _cx: &PStepContext<'_>, powers: &[i64]) -> Result<POrderKey, PError> {
        let distance = powers.iter().fold(0_i64, |distance, &power| {
            distance.saturating_add(if power > 0 {
                power - 1
            } else {
                power.saturating_neg()
            })
        });
        Ok(POrderKey::Lexicographic(vec![
            self.measure(powers)?,
            distance,
        ]))
    }
}
