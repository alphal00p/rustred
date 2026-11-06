//! The Triangle lane (owner: M2-T5 rr-triangle): Forcer's triangle rule as a
//! template applied by our own step, not a SpIRed rule (plan decision D3).
//!
//! In a triangle-adapted presentation `k = mom(b)` runs through `b = k^2` and
//! `a_i = (k + p_i)^2`, with `c_i = p_i^2`; every ISP is `k.X` or free of `k`
//! ([`check_triangle_contract`]). The row `(d/dk).k` of RustRed's own IBP
//! generator then reads
//!
//! ```text
//! P(n) I(n) = X_1(n) A1+ (B- - C1-) I(n) + X_2(n) A2+ (B- - C2-) I(n)
//! P(n)   = d + N - n_a1 - n_a2 - 2 n_b - (j1 + j2) (4 - d) / 2
//! X_i(n) = n_ai + j_i (4 - d) / 2
//! ```
//!
//! with `N = -Sum n_i` over the `k`-linear ISPs and `j_i` the eps units of `a_i`;
//! `b` and `c_i` are integer lines, the apex lines may carry eps.
//!
//! - [`TriangleTemplate::extract`] reads `P` and `X_i` from that row and checks
//!   its shape against the formula above. The formula only validates: every
//!   weight of a step is evaluated from the row, so no hand-written coefficient
//!   is trusted. A row of another shape is a [`TemplateMismatch`]; the node then
//!   falls back to the Custom lane and the reason is recorded by assembly.
//! - [`TriangleStep`] maps `I(n)` to four children weighted by `X_i(n) / P(n)`.
//!   A child with `b`, `c1` or `c2` at power 0 is a [`PStage::Route`] key, a
//!   subsector of the presentation; the others are [`PStage::Apply`] keys of the
//!   same owner. Every child lowers `n_b + n_c1 + n_c2` by exactly one.
//! - The `d` coefficient of `P` is `1 + (j1 + j2) / 2`, never zero: `P` is never
//!   the zero polynomial, so the rule has no exception and the step reports no
//!   condition. An eps line may sit at any integer power, so the eps apex needs no
//!   two-sided SpIRed; `X_i` is never zero on an Apply key.
//!
//! [`P_TRIANGLE_TEMPLATE_VERSION`] names this meaning of a recorded step: the row,
//! its shape, the stages of the children. Bump it on any semantic change.
//!
//! [`P_TRIANGLE_TEMPLATE_VERSION`]: super::P_TRIANGLE_TEMPLATE_VERSION
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
use super::lane::{LaneRoles, OwnerLane, TriangleRoles};
use super::library::{PLibrary, POwnerSpec};
use super::presentation::{Presentation, Slot, check_triangle_contract, eps_power_shift};
use super::step::{PChild, PKeyView, PLaneStep, PStage, PStepContext, PStepOut};

/// The row of a presentation does not have the template's shape, or the roles do
/// not fit the presentation: the lane is at fault, and the node falls back to
/// Custom with this reason.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TemplateMismatch {
    pub presentation: PresentationId,
    pub reason: String,
}

impl fmt::Display for TemplateMismatch {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "triangle template of presentation {}: {}",
            self.presentation.0, self.reason
        )
    }
}

impl std::error::Error for TemplateMismatch {}

/// Why no template exists. Only a mismatch says something about the lane.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TemplateError {
    Mismatch(TemplateMismatch),
    /// A failure of the run: stop, never change the lane.
    Failure(PError),
}

impl fmt::Display for TemplateError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Mismatch(mismatch) => mismatch.fmt(formatter),
            Self::Failure(error) => error.fmt(formatter),
        }
    }
}

impl std::error::Error for TemplateError {}

impl From<PError> for TemplateError {
    fn from(error: PError) -> Self {
        Self::Failure(error)
    }
}

/// `PError::Contract` is the lane's fault everywhere in the p library.
impl From<TemplateError> for PError {
    fn from(error: TemplateError) -> Self {
        match error {
            TemplateError::Mismatch(mismatch) => PError::Contract(mismatch.to_string()),
            TemplateError::Failure(error) => error,
        }
    }
}

fn exact(error: ExactAlgebraError) -> PError {
    ReductionError::ExactAlgebra(error).into()
}

fn indexed(error: IndexedAlgebraError) -> PError {
    ReductionError::IndexedAlgebra(error).into()
}

/// An entry of the row as a function of the powers: `constant + Sum slope n_slot`,
/// with `constant` in `Q(d)`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RowForm {
    pub constant: Coefficient,
    /// `(slot, slope)` by ascending slot, no zero slope.
    pub slopes: Vec<(u16, i64)>,
}

impl RowForm {
    /// The value at `powers`, which must cover every slot of the form.
    pub fn at(
        &self,
        context: &CoefficientContext,
        powers: &[i64],
        limits: ExactAlgebraLimits,
    ) -> Result<Coefficient, PError> {
        let mut sum = 0_i64;
        for &(slot, slope) in &self.slopes {
            let power = powers.get(usize::from(slot)).ok_or_else(|| {
                PError::Contract(format!("no slot {slot} on a key of {}", powers.len()))
            })?;
            sum = slope
                .checked_mul(*power)
                .and_then(|term| sum.checked_add(term))
                .ok_or_else(|| PError::Unsupported(format!("the powers {powers:?} overflow")))?;
        }
        context
            .try_add(&self.constant, &context.integer(sum), limits)
            .map_err(exact)
    }

    fn negated(&self, context: &CoefficientContext) -> Result<Self, PError> {
        let constant = context
            .try_neg(&self.constant, ExactAlgebraLimits::default())
            .map_err(exact)?;
        let negated = self.slopes.iter().map(|&(slot, slope)| (slot, -slope));
        Ok(Self {
            constant,
            slopes: negated.collect(),
        })
    }
}

/// The five entries of the row `P I(n) + Sum_i (R_i A_i+ B- + S_i A_i+ C_i-) I(n) = 0`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RowForms {
    pub pivot: RowForm,
    /// `R_i`.
    pub lower_b: [RowForm; 2],
    /// `S_i`.
    pub lower_c: [RowForm; 2],
}

/// One term `weight * I(powers)` of the solved row.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RowTerm {
    pub powers: Vec<i64>,
    pub weight: Coefficient,
    /// The slot this term lowers: `b` or a `c`.
    pub lowered: u16,
}

/// `I(n) = Sum_terms weight I(powers)`, read from the row `(d/dk).k` of a
/// triangle-adapted presentation, see the module docs. Valid at every integer `n`:
/// the stages of a step are [`TriangleStep`]'s concern.
#[derive(Clone, Debug)]
pub struct TriangleTemplate {
    presentation: PresentationId,
    roles: TriangleRoles,
    context: CoefficientContext,
    arity: usize,
    pivot: RowForm,
    apex: [RowForm; 2],
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

/// A template in the making, on roles that meet the lane contract.
struct Extraction<'a> {
    presentation: &'a Presentation,
    roles: &'a TriangleRoles,
}

impl<'a> Extraction<'a> {
    fn new(
        presentation: &'a Presentation,
        roles: &'a TriangleRoles,
    ) -> Result<Self, TemplateError> {
        let extraction = Self {
            presentation,
            roles,
        };
        match check_triangle_contract(presentation, roles) {
            Ok(()) => Ok(extraction),
            Err(PError::Contract(reason)) => Err(extraction.mismatch(reason)),
            Err(error) => Err(error.into()),
        }
    }

    fn mismatch(&self, reason: impl Into<String>) -> TemplateError {
        TemplateError::Mismatch(TemplateMismatch {
            presentation: self.presentation.id,
            reason: reason.into(),
        })
    }

    /// The five entries of `relation` as forms, which must be its only terms.
    fn row_forms(
        &self,
        generator: &ParametricIbpGenerator<'_>,
        relation: &ParametricRelation,
    ) -> Result<RowForms, TemplateError> {
        if !plain_row(relation) {
            return Err(self.mismatch("the row carries a nonconstant condition"));
        }
        if relation.terms().len() != 5 {
            return Err(self.mismatch(format!(
                "the row has {} terms, not one at zero shift and four shifted",
                relation.terms().len()
            )));
        }
        let context = generator.context();
        let [a1, a2] = self.roles.a.map(usize::from);
        let [c1, c2] = self.roles.c.map(usize::from);
        let b = usize::from(self.roles.b);
        let shift = |raised: Option<usize>, lowered: Option<usize>| {
            (0..context.index_count())
                .map(|slot| i64::from(Some(slot) == raised) - i64::from(Some(slot) == lowered))
                .collect::<Vec<i64>>()
        };
        let entry = |raised, lowered| {
            let wanted = shift(raised, lowered);
            let found = relation
                .terms()
                .iter()
                .find(|(shift, _)| shift.values() == wanted);
            match found {
                Some((_, value)) => self.lift(context, value),
                None => Err(self.mismatch(format!("the row has no term at shift {wanted:?}"))),
            }
        };
        Ok(RowForms {
            pivot: entry(None, None)?,
            lower_b: [entry(Some(a1), Some(b))?, entry(Some(a2), Some(b))?],
            lower_c: [entry(Some(a1), Some(c1))?, entry(Some(a2), Some(c2))?],
        })
    }

    /// The form of a row entry: its values at the zero point and the unit points,
    /// with the check that they rebuild the entry, a polynomial of `Q(d)[n]`.
    fn lift(
        &self,
        context: &IndexedCoefficientContext,
        value: &IndexedCoefficient,
    ) -> Result<RowForm, TemplateError> {
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

    fn eps_units(&self, slot: u16) -> Result<u8, TemplateError> {
        match self.presentation.slots.get(usize::from(slot)) {
            Some(Slot::Line(line)) => Ok(line.eps_units),
            _ => Err(self.mismatch(format!("slot {slot} is not a line"))),
        }
    }

    /// `X_i`: `n_ai + j_i (4 - d) / 2`.
    fn expected_apex(&self, line: u16) -> Result<RowForm, TemplateError> {
        let context = self.presentation.family.coefficient_context();
        Ok(RowForm {
            constant: eps_power_shift(context, self.eps_units(line)?)?,
            slopes: vec![(line, 1)],
        })
    }

    /// `P`: `d + N - n_a1 - n_a2 - 2 n_b - (j1 + j2) (4 - d) / 2`.
    fn expected_pivot(&self) -> Result<RowForm, TemplateError> {
        let context = self.presentation.family.coefficient_context();
        let [a1, a2] = self.roles.a;
        let units = self
            .eps_units(a1)?
            .checked_add(self.eps_units(a2)?)
            .ok_or_else(|| self.mismatch("too many eps units on the apex lines"))?;
        let d = context.parameter("d").ok_or(PError::ContextMismatch)?;
        let constant = context
            .try_sub(
                &d,
                &eps_power_shift(context, units)?,
                ExactAlgebraLimits::default(),
            )
            .map_err(exact)?;
        let l = usize::from(self.roles.k_loop);
        let mut slopes: Vec<(u16, i64)> = Vec::with_capacity(self.presentation.slots.len());
        for (slot, entry) in self.presentation.slots.iter().enumerate() {
            let slot = u16::try_from(slot).expect("at most 64 slots");
            let slope = match entry {
                Slot::Line(_) if slot == self.roles.b => -2,
                Slot::Line(_) if slot == a1 || slot == a2 => -1,
                Slot::Line(_) => 0,
                Slot::Isp(isp) if isp.left[l] != 0 || isp.right[l] != 0 => -1,
                Slot::Isp(_) => 0,
            };
            if slope != 0 {
                slopes.push((slot, slope));
            }
        }
        Ok(RowForm { constant, slopes })
    }

    /// The shape check: the forms against the formula of the module docs.
    fn validated(&self, forms: RowForms) -> Result<TriangleTemplate, TemplateError> {
        let presentation = self.presentation;
        let context = presentation.family.coefficient_context();
        let compare = |what: &str, found: &RowForm, expected: &RowForm| {
            if found == expected {
                Ok(())
            } else {
                Err(self.mismatch(format!(
                    "{what} of the row is {found:?}, the template has {expected:?}"
                )))
            }
        };
        compare(
            "the zero-shift entry",
            &forms.pivot,
            &self.expected_pivot()?,
        )?;
        let RowForms {
            pivot,
            lower_b: [b1, b2],
            lower_c: [c1, c2],
        } = forms;
        let expected = [
            self.expected_apex(self.roles.a[0])?,
            self.expected_apex(self.roles.a[1])?,
        ];
        for (index, (lower_b, lower_c)) in [(&b1, &c1), (&b2, &c2)].into_iter().enumerate() {
            let apex = &expected[index];
            compare("A+ B-", lower_b, &apex.negated(context)?)?;
            compare("A+ C-", lower_c, apex)?;
        }
        Ok(TriangleTemplate {
            presentation: presentation.id,
            roles: *self.roles,
            context: context.clone(),
            arity: presentation.slots.len(),
            pivot,
            // `X_i` as the row has it: minus the entry of `A_i+ B-`.
            apex: [b1.negated(context)?, b2.negated(context)?],
        })
    }
}

impl TriangleTemplate {
    /// The template of `(p, roles)`, from the row `(d/dk).k` with `k` the loop
    /// `roles.k_loop`.
    ///
    /// The row is the generator's own, by its [`RowId`]. `SourceSystem` lowers the
    /// same relations, but it clears denominators and keeps no row identity.
    pub fn extract(p: &Presentation, roles: &TriangleRoles) -> Result<Self, TemplateError> {
        let extraction = Extraction::new(p, roles)?;
        let generation = |error: &dyn fmt::Display| {
            PError::Generation(format!("the IBP row of presentation {}: {error}", p.id.0))
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
        let wanted = RowId::OrdinaryIbp {
            contraction_momentum: usize::from(roles.k_loop),
            differentiated_loop: usize::from(roles.k_loop),
        };
        let relation = relations
            .iter()
            .find(|relation| *relation.row_id() == wanted)
            .ok_or_else(|| extraction.mismatch("the generator has no row (d/dk).k"))?;
        let forms = extraction.row_forms(&generator, relation)?;
        extraction.validated(forms)
    }

    /// [`Self::extract`] from the given row of `generator`, whatever it is. Tests
    /// of the shape check use it.
    #[doc(hidden)]
    pub fn from_relation(
        p: &Presentation,
        roles: &TriangleRoles,
        generator: &ParametricIbpGenerator<'_>,
        relation: &ParametricRelation,
    ) -> Result<Self, TemplateError> {
        let extraction = Extraction::new(p, roles)?;
        let forms = extraction.row_forms(generator, relation)?;
        extraction.validated(forms)
    }

    /// The shape check alone, on forms given by hand: no real row reaches its
    /// branches. Tests of the shape check use it.
    #[doc(hidden)]
    pub fn from_forms(
        p: &Presentation,
        roles: &TriangleRoles,
        forms: RowForms,
    ) -> Result<Self, TemplateError> {
        Extraction::new(p, roles)?.validated(forms)
    }

    pub fn presentation(&self) -> PresentationId {
        self.presentation
    }

    pub fn roles(&self) -> &TriangleRoles {
        &self.roles
    }

    /// `P`, the entry of the row at zero shift.
    pub fn pivot(&self) -> &RowForm {
        &self.pivot
    }

    /// `X_i`, the common factor of the terms of apex `i`.
    pub fn apex(&self) -> &[RowForm; 2] {
        &self.apex
    }

    /// The solved row at `powers`, for any integer powers.
    pub fn solve(
        &self,
        powers: &[i64],
        limits: ExactAlgebraLimits,
    ) -> Result<[RowTerm; 4], PError> {
        if powers.len() != self.arity {
            return Err(PError::Contract(format!(
                "a key of {} powers on the triangle template of {}, arity {}",
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
        let weight = |apex: &RowForm| {
            let value = apex.at(context, powers, limits)?;
            context.try_div(&value, &pivot, limits).map_err(exact)
        };
        let [a1, a2] = self.roles.a;
        let [c1, c2] = self.roles.c;
        let w1 = weight(&self.apex[0])?;
        let w2 = weight(&self.apex[1])?;
        let minus_w1 = context.try_neg(&w1, limits).map_err(exact)?;
        let minus_w2 = context.try_neg(&w2, limits).map_err(exact)?;
        Ok([
            term(a1, self.roles.b, w1)?,
            term(a1, c1, minus_w1)?,
            term(a2, self.roles.b, w2)?,
            term(a2, c2, minus_w2)?,
        ])
    }
}

fn overflow() -> PError {
    PError::Unsupported("a power of a triangle key overflows".to_owned())
}

/// The Triangle program of one owner, see the module docs.
#[derive(Debug)]
pub struct TriangleStep {
    presentation: Arc<Presentation>,
    template: TriangleTemplate,
    lines: usize,
    forced: u64,
}

impl TriangleStep {
    pub fn new(
        presentation: Arc<Presentation>,
        roles: TriangleRoles,
    ) -> Result<Self, TemplateError> {
        let template = TriangleTemplate::extract(&presentation, &roles)?;
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
        let LaneRoles::Triangle(roles) = owner.roles else {
            return Err(PError::Contract(format!(
                "a triangle step with roles {:?}",
                owner.roles
            )));
        };
        if owner.lane != OwnerLane::Triangle || !owner.edge_maps.is_empty() {
            return Err(PError::Contract(format!(
                "a triangle step on lane {:?} with {} edge maps",
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

    pub fn template(&self) -> &TriangleTemplate {
        &self.template
    }

    /// An Apply key: every integer line at a positive power, no ISP in a
    /// denominator. The Ordered adapter routes everything else.
    fn check_key(&self, key: &PKeyView<'_>) -> Result<(), PError> {
        if key.presentation != self.presentation.id
            || key.powers.len() != self.presentation.slots.len()
        {
            return Err(PError::Contract(format!(
                "key of presentation {} with {} powers on triangle step {}",
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
                    "line {slot} at power {power} in an Apply key of triangle step {}",
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

impl PLaneStep for TriangleStep {
    fn lane(&self) -> OwnerLane {
        OwnerLane::Triangle
    }

    fn step(&self, cx: &PStepContext<'_>, key: &PKeyView<'_>) -> Result<PStepOut, PError> {
        self.apply(key, cx.limits)
    }
}
