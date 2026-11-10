//! Presentations: an integral family with its momentum routing, edge maps and
//! the lane contracts (owner: M2-I0 rr-iface; frozen after M2-I0).
//!
//! Slots are lines first, then ISPs. Momentum rows are over `(k_1, .., k_L, q)`;
//! a line slot is `D = mom^2`, an ISP slot `D = left.right`, with `q.q = -1`
//! ([`P_EXTERNAL_SQUARE`]) and context `{d}`. An eps line stores the power shift
//! `j (4 - d) / 2`.
use std::fmt;
use std::sync::Arc;

use symbolica::prelude::Rational;

use crate::algebra::{Coefficient, CoefficientContext, ExactAlgebraError, ExactAlgebraLimits};
use crate::family::{AffineDenominator, IntegralFamily, ScalarProductCoordinate};
use crate::reduction::ReductionError;

use super::census::SignedAffineLoopMap;
use super::error::PError;
use super::lane::{BubbleRoles, CarpetRoles, TriangleRoles};
use super::{NodeIndex, PresentationId};

/// `q.q` of every presentation family: Euclidean `Q^2 = 1` (convention C1).
pub const P_EXTERNAL_SQUARE: i64 = -1;

/// Largest `|c|` of a momentum row; keeps every scalar-product row in `i64`.
const MAX_MOMENTUM_ENTRY: u64 = 1 << 30;

/// Widest presentation: supports are `u64` masks.
const MAX_SLOTS: usize = 64;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PresentationKind {
    Top {
        name: String,
    },
    Induced {
        top: PresentationId,
        mask: u64,
    },
    InsertionAdapted,
    CarpetAdapted,
    TriangleAdapted,
    /// M3-T5b.
    DiamondAdapted,
    EpsNode,
    /// M3.
    Caller,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SlotLine {
    /// Over `(k_1, .., k_L, q)`.
    pub momentum: Vec<i64>,
    /// `j` of the line power `n + j eps`.
    pub eps_units: u8,
}

/// The ISP `left . right`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SlotIsp {
    pub left: Vec<i64>,
    pub right: Vec<i64>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Slot {
    Line(SlotLine),
    Isp(SlotIsp),
}

impl Slot {
    fn factors(&self) -> (&[i64], &[i64]) {
        match self {
            Self::Line(line) => (&line.momentum, &line.momentum),
            Self::Isp(isp) => (&isp.left, &isp.right),
        }
    }

    fn eps_units(&self) -> u8 {
        match self {
            Self::Line(line) => line.eps_units,
            Self::Isp(_) => 0,
        }
    }
}

/// The DAG node of a presentation.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NodeRef {
    pub index: NodeIndex,
    pub graph_key: Arc<[u8]>,
}

#[derive(Clone, Debug)]
pub struct Presentation {
    pub id: PresentationId,
    pub kind: PresentationKind,
    pub loops: u8,
    pub slots: Vec<Slot>,
    pub family: Arc<IntegralFamily>,
    pub node: Option<NodeRef>,
}

/// `a.b = constant + Sum_i r_i D_i`, sorted by slot, no zero `r_i`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AffineSlotForm {
    pub constant: Rational,
    pub terms: Vec<(u16, Rational)>,
}

/// The coordinates of [`IntegralFamily`]: upper-triangular loop-loop, then loop-external.
fn coordinates(loops: usize) -> Vec<ScalarProductCoordinate> {
    let products = (0..loops).flat_map(|left| {
        (left..loops).map(move |right| ScalarProductCoordinate::LoopLoop { left, right })
    });
    let externals = (0..loops).map(|loop_index| ScalarProductCoordinate::LoopExternal {
        loop_index,
        external_index: 0,
    });
    products.chain(externals).collect()
}

/// `a.b = constant + Sum_s row[s] S_s` over `coordinates`. Rows hold checked
/// momenta ([`checked_row`]), so nothing overflows.
fn scalar_row(coordinates: &[ScalarProductCoordinate], a: &[i64], b: &[i64]) -> (i64, Vec<i64>) {
    let external = a.len() - 1;
    let row = coordinates
        .iter()
        .map(|coordinate| match *coordinate {
            ScalarProductCoordinate::LoopLoop { left, right } if left == right => a[left] * b[left],
            ScalarProductCoordinate::LoopLoop { left, right } => {
                a[left] * b[right] + a[right] * b[left]
            }
            ScalarProductCoordinate::LoopExternal { loop_index, .. } => {
                a[loop_index] * b[external] + a[external] * b[loop_index]
            }
        })
        .collect();
    (a[external] * b[external] * P_EXTERNAL_SQUARE, row)
}

pub(super) fn checked_row(row: &[i64], width: usize) -> Result<(), PError> {
    if row.len() != width {
        return Err(PError::Contract(format!(
            "a momentum row has {} entries, not {width}",
            row.len()
        )));
    }
    if row.iter().any(|c| c.unsigned_abs() > MAX_MOMENTUM_ENTRY) {
        return Err(PError::Contract(format!(
            "momentum coefficient out of range in {row:?}"
        )));
    }
    Ok(())
}

fn algebra(error: ExactAlgebraError) -> PError {
    ReductionError::ExactAlgebra(error).into()
}

/// The power shift `j (4 - d) / 2` of an eps line.
pub fn eps_power_shift(context: &CoefficientContext, j: u8) -> Result<Coefficient, PError> {
    let limits = ExactAlgebraLimits::default();
    let d = context.parameter("d").ok_or(PError::ContextMismatch)?;
    let eps = context
        .try_sub(&context.integer(4), &d, limits)
        .and_then(|twice| context.try_div(&twice, &context.integer(2), limits))
        .map_err(algebra)?;
    context
        .try_mul(&context.integer(i64::from(j)), &eps, limits)
        .map_err(algebra)
}

fn is_context_d(context: &CoefficientContext) -> bool {
    context.parameter_names() == ["d"]
}

impl Presentation {
    /// The presentation of `slots` with its authenticated family.
    pub fn from_slots(
        id: PresentationId,
        kind: PresentationKind,
        name: &str,
        loops: u8,
        slots: Vec<Slot>,
        context: &CoefficientContext,
        node: Option<NodeRef>,
    ) -> Result<Self, PError> {
        if !is_context_d(context) {
            return Err(PError::ContextMismatch);
        }
        let width = usize::from(loops) + 1;
        let coordinates = coordinates(usize::from(loops));
        let mut denominators = Vec::with_capacity(slots.len());
        let mut power_shifts = Vec::with_capacity(slots.len());
        for slot in &slots {
            let (left, right) = slot.factors();
            checked_row(left, width)?;
            checked_row(right, width)?;
            let (constant, row) = scalar_row(&coordinates, left, right);
            denominators.push(AffineDenominator::new(
                context.integer(constant),
                row.into_iter().map(|c| context.integer(c)).collect(),
            ));
            power_shifts.push(eps_power_shift(context, slot.eps_units())?);
        }
        let dimension = context.parameter("d").ok_or(PError::ContextMismatch)?;
        let family = IntegralFamily::new(
            name,
            (1..=loops).map(|l| format!("k{l}")).collect(),
            vec!["q".to_owned()],
            context.clone(),
            dimension,
            denominators,
            vec![vec![context.integer(P_EXTERNAL_SQUARE)]],
            power_shifts,
        )
        .map_err(|error| PError::Contract(format!("family {name}: {error}")))?;
        let presentation = Self {
            id,
            kind,
            loops,
            slots,
            family: Arc::new(family),
            node,
        };
        presentation.validate()?;
        Ok(presentation)
    }

    /// The slots describe the family exactly: `loops >= 1` (no `L = 0`
    /// presentation exists, the trivial node has none), one external with
    /// `q.q = -1`, context `{d}`, dimension `d`, lines first, every denominator
    /// row and power shift.
    pub fn validate(&self) -> Result<(), PError> {
        let family = &self.family;
        let context = family.coefficient_context();
        if !is_context_d(context) {
            return Err(PError::ContextMismatch);
        }
        let fail =
            |detail: String| PError::Contract(format!("presentation {}: {detail}", self.id.0));
        if self.loops == 0
            || family.loop_count() != usize::from(self.loops)
            || family.external_count() != 1
            || family.external_gram() != [vec![context.integer(P_EXTERNAL_SQUARE)]]
        {
            return Err(fail(format!(
                "a family with {} loops and q.q = {P_EXTERNAL_SQUARE} is required",
                self.loops
            )));
        }
        if context.parameter("d").as_ref() != Some(family.dimension()) {
            return Err(fail("the family's dimension must be d".to_owned()));
        }
        if self.slots.len() != family.denominator_count() || self.slots.len() > MAX_SLOTS {
            return Err(fail(format!(
                "{} slots for {} denominators (at most {MAX_SLOTS})",
                self.slots.len(),
                family.denominator_count()
            )));
        }
        let lines = self.line_count();
        if self.slots[..lines]
            .iter()
            .any(|s| matches!(s, Slot::Isp(_)))
            || self.slots[lines..]
                .iter()
                .any(|s| matches!(s, Slot::Line(_)))
        {
            return Err(fail("lines must precede ISPs".to_owned()));
        }
        let width = usize::from(self.loops) + 1;
        for (index, slot) in self.slots.iter().enumerate() {
            let (left, right) = slot.factors();
            checked_row(left, width)?;
            checked_row(right, width)?;
            let (constant, row) = scalar_row(family.coordinates(), left, right);
            let denominator = &family.denominators()[index];
            let same_row = denominator.coefficients().len() == row.len()
                && denominator
                    .coefficients()
                    .iter()
                    .zip(&row)
                    .all(|(actual, &expected)| *actual == context.integer(expected));
            if !same_row || *denominator.constant() != context.integer(constant) {
                return Err(fail(format!("slot {index} differs from its denominator")));
            }
            if family.power_shifts()[index] != eps_power_shift(context, slot.eps_units())? {
                return Err(fail(format!("slot {index} differs from its power shift")));
            }
        }
        Ok(())
    }

    fn line_count(&self) -> usize {
        self.slots
            .iter()
            .take_while(|slot| matches!(slot, Slot::Line(_)))
            .count()
    }

    pub(super) fn lines(&self) -> impl Iterator<Item = (usize, &SlotLine)> {
        self.slots
            .iter()
            .enumerate()
            .filter_map(|(index, slot)| match slot {
                Slot::Line(line) => Some((index, line)),
                Slot::Isp(_) => None,
            })
    }

    fn isps(&self) -> impl Iterator<Item = &SlotIsp> {
        self.slots.iter().filter_map(|slot| match slot {
            Slot::Line(_) => None,
            Slot::Isp(isp) => Some(isp),
        })
    }

    /// `a.b` over the slots of this presentation, for `a`, `b` over
    /// `(k_1, .., k_L, q)`. One exact solve per call: kernels tabulate their
    /// rewrites at construction.
    pub fn express_scalar_product(&self, a: &[i64], b: &[i64]) -> Result<AffineSlotForm, PError> {
        let width = usize::from(self.loops) + 1;
        checked_row(a, width)?;
        checked_row(b, width)?;
        let coordinates = self.family.coordinates();
        let size = coordinates.len();
        let rows: Vec<_> = self
            .slots
            .iter()
            .map(|slot| {
                let (left, right) = slot.factors();
                scalar_row(coordinates, left, right)
            })
            .collect();
        if rows.len() != size {
            return Err(PError::Contract(format!(
                "presentation {} has {} slots for {size} scalar products",
                self.id.0,
                rows.len()
            )));
        }
        // a.b = c + t.S and D_j = c_j + A_j.S: solve Sum_j r_j A_j = t, the
        // transposed system, then a.b = (c - Sum_j r_j c_j) + Sum_j r_j D_j.
        let transposed = (0..size)
            .map(|s| rows.iter().map(|(_, row)| Rational::from(row[s])).collect())
            .collect();
        let (constant, target) = scalar_row(coordinates, a, b);
        let target = target.into_iter().map(Rational::from).collect();
        let solution = solve(transposed, target).ok_or_else(|| {
            PError::Contract(format!(
                "the slots of presentation {} are not a basis of the scalar products",
                self.id.0
            ))
        })?;
        let mut constant = Rational::from(constant);
        let mut terms = Vec::new();
        for (slot, (value, (slot_constant, _))) in solution.into_iter().zip(&rows).enumerate() {
            if !value.is_zero() {
                constant -= &value * &Rational::from(*slot_constant);
                let slot = u16::try_from(slot).expect("at most 64 slots");
                terms.push((slot, value));
            }
        }
        Ok(AffineSlotForm { constant, terms })
    }

    /// Mask of the eps line slots: axes that are in every support.
    pub fn forced_axes(&self) -> u64 {
        self.lines()
            .filter(|(_, line)| line.eps_units != 0)
            .fold(0, |mask, (index, _)| mask | 1 << index)
    }
}

/// Solve `matrix x = rhs` exactly by Gauss-Jordan elimination.
fn solve(mut matrix: Vec<Vec<Rational>>, mut rhs: Vec<Rational>) -> Option<Vec<Rational>> {
    let size = matrix.len();
    for column in 0..size {
        let pivot = (column..size).find(|&row| !matrix[row][column].is_zero())?;
        matrix.swap(column, pivot);
        rhs.swap(column, pivot);
        let inverse = matrix[column][column].inv();
        for entry in &mut matrix[column][column..] {
            *entry *= &inverse;
        }
        rhs[column] *= &inverse;
        let pivot_row = matrix[column].clone();
        let pivot_value = rhs[column].clone();
        for row in (0..size).filter(|&row| row != column) {
            let factor = matrix[row][column].clone();
            if factor.is_zero() {
                continue;
            }
            for (entry, pivot_entry) in matrix[row][column..].iter_mut().zip(&pivot_row[column..]) {
                *entry -= &factor * pivot_entry;
            }
            rhs[row] -= &factor * &pivot_value;
        }
    }
    Some(rhs)
}

/// The determinant of a square row-major integer matrix, exactly.
pub(super) fn determinant(size: usize, row_major: &[i64]) -> Rational {
    let mut matrix: Vec<Vec<Rational>> = row_major
        .chunks(size.max(1))
        .map(|row| row.iter().map(|&entry| Rational::from(entry)).collect())
        .collect();
    let mut value = Rational::one();
    for column in 0..size {
        let Some(pivot) = (column..size).find(|&row| !matrix[row][column].is_zero()) else {
            return Rational::zero();
        };
        if pivot != column {
            matrix.swap(column, pivot);
            value = -value;
        }
        value *= &matrix[column][column];
        let inverse = matrix[column][column].inv();
        let pivot_row = matrix[column].clone();
        for row in &mut matrix[column + 1..] {
            let factor = &row[column] * &inverse;
            for (entry, pivot_entry) in row[column..].iter_mut().zip(&pivot_row[column..]) {
                *entry -= &factor * pivot_entry;
            }
        }
    }
    value
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum EdgeKind {
    Insertion,
    Carpet,
}

/// One rule edge of the DAG.
///
/// `outer_map` is the substitution from the parent's loops without
/// `integrated_loop`, in order, to the child's loops. Its external column is
/// the parent's `q` on an Insertion edge; on a Carpet edge it is the parent's
/// outer loop `p`, which becomes the child's `q` (the blob's external momentum).
///
/// Every map joins two presentations, so a rule edge into the trivial node
/// (`1L -> L = 0`) has none: the owner emits [`PTerminal::Unit`] there.
///
/// [`PTerminal::Unit`]: super::library::PTerminal::Unit
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EdgeMap {
    pub parent: PresentationId,
    pub child: PresentationId,
    pub kind: EdgeKind,
    pub integrated_loop: u8,
    pub outer_map: SignedAffineLoopMap,
    /// Parent slot to child line slot (series-merged lines share one); `None`
    /// for integrated lines and ISPs (ISPs go through `express_scalar_product`).
    pub line_slots: Vec<Option<u16>>,
    /// Child slot of the merged line of an Insertion edge; `None` = parallel
    /// to `q`, absorbed. Always `None` on a Carpet edge.
    pub merged_slot: Option<u16>,
}

fn unit(width: usize, position: usize) -> Vec<i64> {
    let mut row = vec![0; width];
    row[position] = 1;
    row
}

fn up_to_sign(a: &[i64], b: &[i64]) -> bool {
    a == b || (a.len() == b.len() && a.iter().zip(b).all(|(x, y)| *x == -*y))
}

/// `row` scaled by `factor`, with `shift` subtracted.
fn scaled_minus(row: &[i64], factor: i64, shift: &[i64]) -> Vec<i64> {
    row.iter().zip(shift).map(|(x, y)| x * factor - y).collect()
}

/// The parent row of the child's input momenta (see [`EdgeMap`]), or `None`
/// when the row depends on what the edge removes.
fn outer_row(kind: EdgeKind, integrated: usize, momentum: &[i64]) -> Option<Vec<i64>> {
    let external = momentum.len() - 1;
    let dropped = match kind {
        EdgeKind::Insertion => integrated,
        EdgeKind::Carpet => external,
    };
    if momentum[dropped] != 0 {
        return None;
    }
    let mut row = Vec::with_capacity(external);
    row.extend(
        (0..external)
            .filter(|&c| c != integrated)
            .map(|c| momentum[c]),
    );
    row.push(match kind {
        EdgeKind::Insertion => momentum[external],
        EdgeKind::Carpet => momentum[integrated],
    });
    Some(row)
}

/// Exact check of an edge map: both presentations are valid, arities, a
/// unimodular `outer_map`, every kept parent line lands on its child line up to
/// sign, the integrated lines are the ones the rule removes, every child line
/// is hit, and eps units add (`+ 1` on the merged line of an insertion).
pub fn verify_edge_map(
    parent: &Presentation,
    child: &Presentation,
    map: &EdgeMap,
) -> Result<(), PError> {
    parent.validate()?;
    child.validate()?;
    let fail = |detail: String| {
        PError::Contract(format!(
            "edge map {} -> {}: {detail}",
            parent.id.0, child.id.0
        ))
    };
    if map.parent != parent.id || map.child != child.id {
        return Err(fail(format!(
            "it names the presentations {} -> {}",
            map.parent.0, map.child.0
        )));
    }
    let integrated = usize::from(map.integrated_loop);
    if child.loops.checked_add(1) != Some(parent.loops) || integrated >= usize::from(parent.loops) {
        return Err(fail(format!(
            "{} -> {} loops integrating loop {integrated}",
            parent.loops, child.loops
        )));
    }
    if map.outer_map.loops_in != child.loops
        || map.outer_map.loops_out != child.loops
        || !map.outer_map.is_unimodular()
    {
        return Err(fail(
            "outer_map is not unimodular on the outer loops".to_owned(),
        ));
    }
    if map.line_slots.len() != parent.slots.len() {
        return Err(fail(format!(
            "{} line_slots for {} parent slots",
            map.line_slots.len(),
            parent.slots.len()
        )));
    }

    // Eps units arriving at each child line; `None` until a line lands there.
    let mut arriving: Vec<Option<u32>> = vec![None; child.slots.len()];
    let mut arrive = |target: u16, image: &[i64], eps_units: u32| -> Result<(), PError> {
        let slot = usize::from(target);
        let Some(Slot::Line(line)) = child.slots.get(slot) else {
            return Err(fail(format!("child slot {target} is not a line")));
        };
        if !up_to_sign(image, &line.momentum) {
            return Err(fail(format!(
                "{image:?} does not land on child line {target}, {:?}",
                line.momentum
            )));
        }
        *arriving[slot].get_or_insert(0) += eps_units;
        Ok(())
    };

    let mut removed = Vec::new();
    for (index, (slot, target)) in parent.slots.iter().zip(&map.line_slots).enumerate() {
        match (slot, target) {
            (Slot::Isp(_), None) => {}
            (Slot::Isp(_), Some(_)) => return Err(fail(format!("ISP slot {index} has a target"))),
            (Slot::Line(line), None) => removed.push(line),
            (Slot::Line(line), Some(target)) => {
                let row = outer_row(map.kind, integrated, &line.momentum).ok_or_else(|| {
                    fail(format!("line slot {index} depends on the removed momentum"))
                })?;
                let image = map.outer_map.map_momentum(&row)?;
                arrive(*target, &image, u32::from(line.eps_units))?;
            }
        }
    }

    let child_q = unit(usize::from(child.loops) + 1, usize::from(child.loops));
    match map.kind {
        EdgeKind::Insertion => {
            let [first, second] = removed.as_slice() else {
                return Err(fail(format!("{} integrated lines, not 2", removed.len())));
            };
            let (a, b) = (&first.momentum, &second.momentum);
            if a[integrated].abs() != 1 || b[integrated].abs() != 1 {
                return Err(fail(
                    "the bubble lines must carry the integrated loop".to_owned(),
                ));
            }
            // a = s (k + R1), b = t (k + R2): the bubble momentum is R1 - R2.
            let scaled_b: Vec<i64> = b.iter().map(|c| c * b[integrated]).collect();
            let bubble = scaled_minus(a, a[integrated], &scaled_b);
            let row = outer_row(map.kind, integrated, &bubble)
                .ok_or_else(|| fail("the bubble momentum keeps the loop".to_owned()))?;
            let image = map.outer_map.map_momentum(&row)?;
            match map.merged_slot {
                Some(target) => {
                    let eps_units = u32::from(first.eps_units) + u32::from(second.eps_units) + 1;
                    arrive(target, &image, eps_units)?;
                }
                None if up_to_sign(&image, &child_q) => {}
                None => return Err(fail(format!("the absorbed line {image:?} is not +-q"))),
            }
        }
        EdgeKind::Carpet => {
            if map.merged_slot.is_some() {
                return Err(fail("a carpet merges no line".to_owned()));
            }
            let width = usize::from(parent.loops) + 1;
            let p = unit(width, integrated);
            let r = scaled_minus(&p, 1, &unit(width, width - 1));
            let carrying_q = removed.iter().filter(|line| up_to_sign(&line.momentum, &r));
            let third = removed.iter().filter(|line| up_to_sign(&line.momentum, &p));
            if carrying_q.count() != 1 || removed.len() != 1 + third.count() {
                return Err(fail(
                    "a carpet removes r = (p - q)^2 and third = p^2".to_owned(),
                ));
            }
        }
    }

    for (index, slot) in child.slots.iter().enumerate() {
        if let Slot::Line(line) = slot {
            match arriving[index] {
                None => return Err(fail(format!("child line {index} is not hit"))),
                Some(units) if units != u32::from(line.eps_units) => {
                    return Err(fail(format!(
                        "child line {index} has {} eps units, the edge gives {units}",
                        line.eps_units
                    )));
                }
                Some(_) => {}
            }
        }
    }
    Ok(())
}

/// Lookups shared by the contract checkers.
struct ContractCheck<'a> {
    presentation: &'a Presentation,
    rule: &'static str,
}

impl<'a> ContractCheck<'a> {
    /// A checker reads the rows unguarded, so the presentation is validated first.
    fn new(presentation: &'a Presentation, rule: &'static str) -> Result<Self, PError> {
        presentation.validate()?;
        Ok(Self { presentation, rule })
    }

    fn fail(&self, detail: impl fmt::Display) -> PError {
        PError::Contract(format!(
            "{} contract, presentation {}: {detail}",
            self.rule, self.presentation.id.0
        ))
    }

    fn width(&self) -> usize {
        usize::from(self.presentation.loops) + 1
    }

    fn line(&self, slot: u16) -> Result<&'a SlotLine, PError> {
        match self.presentation.slots.get(usize::from(slot)) {
            Some(Slot::Line(line)) => Ok(line),
            _ => Err(self.fail(format!("slot {slot} is not a line"))),
        }
    }

    fn loop_index(&self, index: u8) -> Result<usize, PError> {
        if index < self.presentation.loops {
            Ok(usize::from(index))
        } else {
            Err(self.fail(format!("loop {index} does not exist")))
        }
    }

    /// Roles name distinct slots.
    fn distinct(&self, slots: &[u16]) -> Result<(), PError> {
        for (index, slot) in slots.iter().enumerate() {
            if slots[..index].contains(slot) {
                return Err(self.fail(format!("slot {slot} has two roles")));
            }
        }
        Ok(())
    }

    /// Only `allowed` lines contain `k_position`.
    fn only_lines_carry(&self, position: usize, allowed: &[u16]) -> Result<(), PError> {
        for (index, line) in self.presentation.lines() {
            let role = u16::try_from(index).is_ok_and(|index| allowed.contains(&index));
            if !role && line.momentum[position] != 0 {
                return Err(self.fail(format!("line {index} contains loop {position}")));
            }
        }
        Ok(())
    }

    /// Every ISP is free of `k_position` or exactly `+-k_position . Y`, `Y` free of it.
    fn isps_linear_in(&self, position: usize) -> Result<(), PError> {
        let k = unit(self.width(), position);
        for isp in self.presentation.isps() {
            let pure = match (isp.left[position] != 0, isp.right[position] != 0) {
                (false, false) => true,
                (true, false) => up_to_sign(&isp.left, &k),
                (false, true) => up_to_sign(&isp.right, &k),
                (true, true) => false,
            };
            if !pure {
                return Err(self.fail(format!(
                    "the ISP {:?}.{:?} is not linear in loop {position}",
                    isp.left, isp.right
                )));
            }
        }
        Ok(())
    }
}

/// `k_l` occurs only in the two bubble lines (`+-k_l` in `lines[0]`, `+-(k_l - P)`
/// in `lines[1]`, `P` free of `k_l`) and in ISPs `k_l.Y` (`Y` free of `k_l`).
pub fn check_insertion_contract(p: &Presentation, r: &BubbleRoles) -> Result<(), PError> {
    let check = ContractCheck::new(p, "insertion")?;
    let l = check.loop_index(r.loop_index)?;
    check.distinct(&r.lines)?;
    let (first, second) = (check.line(r.lines[0])?, check.line(r.lines[1])?);
    if !up_to_sign(&first.momentum, &unit(check.width(), l)) {
        return Err(check.fail(format!("the first bubble line is {:?}", first.momentum)));
    }
    if second.momentum[l].abs() != 1 {
        return Err(check.fail(format!("the second bubble line is {:?}", second.momentum)));
    }
    check.only_lines_carry(l, &r.lines)?;
    check.isps_linear_in(l)
}

/// `q` is routed only through `r = (p - q)^2`, `p = k_l`, `l = outer_loop`;
/// `third`, when present, is `p^2`; when absent no line is `+-p`, since a step
/// would drop the power of that slot. Every other line is `q`-free. Every ISP is
/// `q`-free or `l.q` with `l` a blob loop momentum (no `p` and no `q` component);
/// so no ISP contains `p.q`. `p` itself does occur in blob lines: the blob is a
/// two-point function of `p`.
pub fn check_carpet_contract(p: &Presentation, r: &CarpetRoles) -> Result<(), PError> {
    let check = ContractCheck::new(p, "carpet")?;
    let outer = check.loop_index(r.outer_loop)?;
    let width = check.width();
    let external = width - 1;
    let outer_momentum = unit(width, outer);
    let expected_r = scaled_minus(&outer_momentum, 1, &unit(width, external));
    let line_r = check.line(r.r)?;
    if !up_to_sign(&line_r.momentum, &expected_r) {
        return Err(check.fail(format!("r is {:?}", line_r.momentum)));
    }
    match r.third {
        Some(third) => {
            check.distinct(&[r.r, third])?;
            let line_third = check.line(third)?;
            if !up_to_sign(&line_third.momentum, &outer_momentum) {
                return Err(check.fail(format!("third is {:?}", line_third.momentum)));
            }
        }
        None => {
            let unclaimed = p
                .lines()
                .find(|(_, line)| up_to_sign(&line.momentum, &outer_momentum));
            if let Some((index, _)) = unclaimed {
                return Err(check.fail(format!("line {index} is p^2, the roles have no third")));
            }
        }
    }
    check.only_lines_carry(external, &[r.r])?;
    let q = unit(width, external);
    for isp in p.isps() {
        let blob = match (isp.left[external] != 0, isp.right[external] != 0) {
            (false, false) => continue,
            (true, false) if up_to_sign(&isp.left, &q) => &isp.right,
            (false, true) if up_to_sign(&isp.right, &q) => &isp.left,
            _ => return Err(check.fail(format!("the ISP {:?}.{:?}", isp.left, isp.right))),
        };
        if blob[outer] != 0 {
            return Err(check.fail(format!("the ISP {blob:?}.q contains p.q")));
        }
    }
    Ok(())
}

/// `k = k_l` (`l = k_loop`) occurs only in `b = +-k`, `a[i] = +-(k + p_i)` with
/// `p_i = +-mom(c[i])`, and in ISPs `k.X` (`X` free of `k`). `b` and `c` are
/// integer lines; the apex lines `a` may carry eps.
pub fn check_triangle_contract(p: &Presentation, r: &TriangleRoles) -> Result<(), PError> {
    let check = ContractCheck::new(p, "triangle")?;
    let l = check.loop_index(r.k_loop)?;
    let roles = [r.b, r.a[0], r.a[1], r.c[0], r.c[1]];
    check.distinct(&roles)?;
    let k = unit(check.width(), l);
    let line_b = check.line(r.b)?;
    if !up_to_sign(&line_b.momentum, &k) {
        return Err(check.fail(format!("b is {:?}", line_b.momentum)));
    }
    for (a, c) in r.a.into_iter().zip(r.c) {
        let (line_a, line_c) = (check.line(a)?, check.line(c)?);
        if line_a.momentum[l].abs() != 1 {
            return Err(check.fail(format!("line {a} is {:?}", line_a.momentum)));
        }
        let rest = scaled_minus(&line_a.momentum, line_a.momentum[l], &k);
        if !up_to_sign(&rest, &line_c.momentum) {
            return Err(check.fail(format!(
                "line {a} is k + {rest:?}, line {c} is {:?}",
                line_c.momentum
            )));
        }
        if line_c.eps_units != 0 {
            return Err(check.fail(format!("line {c} carries eps")));
        }
    }
    if line_b.eps_units != 0 {
        return Err(check.fail("b carries eps"));
    }
    check.only_lines_carry(l, &[r.b, r.a[0], r.a[1]])?;
    check.isps_linear_in(l)
}
