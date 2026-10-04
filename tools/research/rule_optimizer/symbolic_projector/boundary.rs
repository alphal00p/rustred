//! Finite polynomial-weight endpoint constraints, not a module/syzygy solver.
//! Symbolica owns restriction, coefficient grouping, and field elimination.
//! The ordinary Span and unchanged original-source/chart checker own authority.
use super::project::{self, *};
use rustred::{
    algebra::{Coefficient, IndexedAlgebraLimits, IndexedCoefficient, IndexedCoefficientContext},
    identity::IndexShift,
};
use serde::Deserialize;
use serde_json::Value;
use std::{
    collections::{BTreeMap, BTreeSet},
    fmt,
};
use symbolica::{
    domains::{SelfRing, rational_polynomial::FromNumeratorAndDenominator},
    prelude::Z,
    tensors::sparse::{LuLMode, SparseRowReducer},
};

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum Mode {
    ActivationFaces,
    WholeColumns,
}

#[path = "boundary/target_template.rs"]
mod target_template;
use target_template::{CompiledTemplate, compile_template, template_constraints, verify_template};
pub use target_template::{TargetTemplate, TemplateTerm};

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Config {
    pub mode: Mode,
    pub polynomial_axes: Vec<usize>,
    pub protected_axes: Vec<usize>,
    pub weight_monomials: Vec<Vec<u16>>,
    pub max_total_degree: usize,
    pub max_unknowns: usize,
    pub max_constraints: usize,
    pub max_nonzeros: usize,
    pub max_activation_faces: usize,
    #[serde(default)]
    pub primitive_original_weights: bool,
    #[serde(default)]
    pub target_template: Option<TargetTemplate>,
}

pub fn config(r: &Value) -> super::Result<Option<Config>> {
    let Some(value) = r.get("boundary_polynomial") else {
        return Ok(None);
    };
    let config: Config = super::checked(serde_json::from_value(value.clone()))?;
    let mask = r["owner_mask"]
        .as_str()
        .ok_or("boundary owner mask required")?;
    let fixed = super::fixed(r)?;
    super::checked(config.validate(mask.len()))?;
    for &axis in &config.polynomial_axes {
        super::require(
            !fixed.iter().any(|&(a, _)| a == axis),
            "polynomial axis is fixed",
        )?;
    }
    for axis in 0..mask.len() {
        super::require(
            config.polynomial_axes.contains(&axis) || fixed.iter().any(|&(a, _)| a == axis),
            "every unlisted index must be explicitly fixed",
        )?;
    }
    for &axis in &config.protected_axes {
        super::require(
            mask.as_bytes()[axis] == b'0'
                && r["chart"]["lower"][axis].as_u64() == Some(0)
                && r["chart"]["upper"][axis].is_null(),
            "protected axes require the full inactive nonpositive orthant",
        )?;
    }
    super::require(
        super::number(r, "max_refinements")? == 0
            && r.get("modular_nomination").is_none()
            && !super::reconstructed::enabled(r)
            && !super::direct_l::enabled(r)?,
        "boundary polynomial mode is one exact bounded attempt without other projection backends",
    )?;
    Ok(Some(config))
}

impl Config {
    fn validate(&self, arity: usize) -> Result<()> {
        require(
            !self.polynomial_axes.is_empty()
                && !self.protected_axes.is_empty()
                && self.polynomial_axes.windows(2).all(|p| p[0] < p[1])
                && self.protected_axes.windows(2).all(|p| p[0] < p[1])
                && self.polynomial_axes.iter().all(|&a| a < arity)
                && self
                    .protected_axes
                    .iter()
                    .all(|a| self.polynomial_axes.contains(a)),
            "boundary axes must be sorted, unique, in range and protected within polynomial axes",
        )?;
        require(
            self.max_unknowns > 0
                && self.max_constraints > 0
                && self.max_nonzeros > 0
                && self.max_activation_faces > 0
                && !self.weight_monomials.is_empty(),
            "boundary limits and monomial list must be nonempty",
        )?;
        bound(
            self.weight_monomials.len(),
            self.max_unknowns,
            "weight monomials",
        )?;
        let mut seen = BTreeSet::new();
        for powers in &self.weight_monomials {
            require(
                powers.len() == self.polynomial_axes.len() && seen.insert(powers),
                "monomial arity/duplicate",
            )?;
            let degree = powers
                .iter()
                .try_fold(0usize, |sum, &p| add(sum, usize::from(p)))?;
            bound(degree, self.max_total_degree, "weight degree")?;
        }
        Ok(())
    }
}

// These are linear-constraint labels, never synthetic integral-shift columns.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
enum Constraint {
    Global(IndexShift, Vec<u16>),
    Face(IndexShift, usize, i64, Vec<u16>),
    Template(Vec<u16>),
}
struct BasisRow {
    source: usize,
    monomial: IndexedCoefficient,
    constraints: BTreeMap<Constraint, IndexedCoefficient>,
    target: BTreeMap<Vec<u16>, IndexedCoefficient>,
}

#[derive(Default)]
struct Admission {
    entries: usize,
    terms: usize,
}

fn work(charged: &mut usize, count: usize, limits: Limits) -> Result<()> {
    *charged = add(*charged, count)?;
    bound(*charged, limits.operations, "boundary aggregate operations")
}

fn register_label<T: Ord>(
    key: T,
    labels: &mut BTreeSet<T>,
    count: &mut usize,
    config: &Config,
) -> Result<()> {
    if !labels.contains(&key) {
        let next = add(*count, 1)?;
        bound(next, config.max_constraints, "constraint and target labels")?;
        labels.insert(key);
        *count = next;
    }
    Ok(())
}

fn index_positions(c: &IndexedCoefficientContext) -> std::ops::Range<usize> {
    let first = c.base().parameter_names().len();
    first..first + c.index_count()
}

fn scalar(c: &IndexedCoefficientContext, value: &IndexedCoefficient, limits: Limits) -> Result<()> {
    c.validate_with_limits(value, limits.arithmetic)?;
    require(
        index_positions(c)
            .all(|p| !value.raw().numerator.contains(p) && !value.raw().denominator.contains(p)),
        "linear system coefficient escaped the base-parameter field",
    )
}

fn polynomial_input(
    c: &IndexedCoefficientContext,
    value: &IndexedCoefficient,
    config: &Config,
    limits: Limits,
) -> Result<()> {
    c.validate_with_limits(value, limits.arithmetic)?;
    let base = c.base().parameter_names().len();
    require(
        index_positions(c).all(|p| !value.raw().denominator.contains(p)),
        "boundary polynomial mode refuses index-dependent source denominators",
    )?;
    require(
        (0..c.index_count()).all(|axis| {
            config.polynomial_axes.contains(&axis) || !value.raw().numerator.contains(base + axis)
        }),
        "source retains an undeclared polynomial index",
    )
}

/// Exact native grouping with the authenticated full variable map retained.
fn split(
    c: &IndexedCoefficientContext,
    value: &IndexedCoefficient,
    config: &Config,
    limits: Limits,
    admission: &mut Admission,
    operations: &mut usize,
) -> Result<BTreeMap<Vec<u16>, IndexedCoefficient>> {
    polynomial_input(c, value, config, limits)?;
    bound(
        value.raw().numerator.nterms(),
        limits.coefficient_terms,
        "coefficient grouping input",
    )?;
    // A native grouping has at most one output per input term. Admit that
    // upper bound before requesting its temporary HashMap allocation.
    let upper = value.raw().numerator.nterms();
    bound(upper, config.max_constraints, "native grouping upper bound")?;
    bound(
        add(admission.entries, upper)?,
        config.max_nonzeros.min(limits.nonzeros),
        "grouping entry admission",
    )?;
    work(operations, upper, limits)?;
    let positions = index_positions(c).collect::<Vec<_>>();
    let groups = native(|| {
        value
            .raw()
            .numerator
            .to_multivariate_polynomial_list(&positions, true)
    })?;
    bound(groups.len(), config.max_constraints, "coefficient groups")?;
    let mut result = BTreeMap::new();
    for (powers, numerator) in groups {
        let raw = native(|| {
            Coefficient::from_num_den(numerator, value.raw().denominator.clone(), &Z, true)
        })?;
        let coefficient = c.admit_native_result_with_limits(raw, limits.arithmetic)?;
        scalar(c, &coefficient, limits)?;
        if !coefficient.is_zero() {
            let entries = add(admission.entries, 1)?;
            let terms = add(
                admission.terms,
                add(
                    coefficient.raw().numerator.nterms(),
                    coefficient.raw().denominator.nterms(),
                )?,
            )?;
            bound(
                entries,
                config.max_nonzeros.min(limits.nonzeros),
                "assembled constraint nonzeros",
            )?;
            bound(
                terms,
                limits.coefficient_terms,
                "assembled coefficient terms",
            )?;
            admission.entries = entries;
            admission.terms = terms;
            require(
                result.insert(powers.to_vec(), coefficient).is_none(),
                "duplicate native coefficient group",
            )?;
        }
    }
    Ok(result)
}

// Small observational witnesses only. Formatting cannot grow this retained
// buffer beyond its cap; displays never enter a coefficient/proof constructor.
fn diagnostic(value: &impl fmt::Display) -> Value {
    struct Bounded(String);
    impl fmt::Write for Bounded {
        fn write_str(&mut self, text: &str) -> fmt::Result {
            if text.len() > 4096 - self.0.len() {
                return Err(fmt::Error);
            }
            self.0.push_str(text);
            Ok(())
        }
    }
    let mut text = Bounded(String::new());
    let truncated = fmt::write(&mut text, format_args!("{value}")).is_err();
    serde_json::json!({"display":text.0,"truncated":truncated,"display_only":true})
}

fn primitive_work(left: usize, right: usize, operations: &mut usize, limits: Limits) -> Result<()> {
    let pairs = left
        .checked_mul(right)
        .ok_or(Error::Budget("primitive term pairs"))?;
    // This admission count does not bound native GCD scratch/internal work.
    // The outer owned-process RSS/time limits remain required.
    work(operations, add(pairs, 1)?, limits)
}

fn primitive_admit(
    c: &IndexedCoefficientContext,
    raw: Coefficient,
    admission: &mut Admission,
    limits: Limits,
) -> Result<IndexedCoefficient> {
    let value = c.admit_native_result_with_limits(raw, limits.arithmetic)?;
    let count = add(
        value.raw().numerator.nterms(),
        value.raw().denominator.nterms(),
    )?;
    let total = add(admission.terms, count)?;
    bound(
        total,
        limits.coefficient_terms,
        "primitive retained coefficient terms",
    )?;
    admission.terms = total;
    Ok(value)
}

/// A new polynomial ORIGINAL-weight proposal, before any target pivot exists.
/// Numerator GCD suffices up to base-field units because every denominator is
/// index-independent. Never divide an endpoint image or introduce a g!=0 guard.
fn primitive_weights(
    c: &IndexedCoefficientContext,
    weights: &mut Weights,
    config: &Config,
    admission: &mut Admission,
    operations: &mut usize,
    limits: Limits,
) -> Result<Option<IndexedCoefficient>> {
    require(!weights.is_empty(), "empty primitive original circuit")?;
    bound(weights.len(), limits.rows, "primitive original weights")?;
    for value in weights.values() {
        polynomial_input(c, value, config, limits)?;
        require(!value.is_zero(), "explicit zero primitive weight")?;
    }
    let mut iter = weights.values();
    let first = iter.next().expect("nonempty checked");
    primitive_work(first.raw().numerator.nterms(), 1, operations, limits)?;
    let mut common = primitive_admit(
        c,
        native(|| {
            Coefficient::from_num_den(
                first.raw().numerator.clone(),
                c.one().raw().denominator.clone(),
                &Z,
                true,
            )
        })?,
        admission,
        limits,
    )?;
    for value in iter {
        if !index_positions(c).any(|p| common.raw().numerator.contains(p)) {
            break;
        }
        primitive_work(
            common.raw().numerator.nterms(),
            value.raw().numerator.nterms(),
            operations,
            limits,
        )?;
        let numerator = native(|| common.raw().numerator.gcd(&value.raw().numerator))?;
        common = primitive_admit(
            c,
            native(|| {
                Coefficient::from_num_den(numerator, c.one().raw().denominator.clone(), &Z, true)
            })?,
            admission,
            limits,
        )?;
    }
    require(!common.is_zero(), "native primitive divisor is zero")?;
    if !index_positions(c).any(|p| common.raw().numerator.contains(p)) {
        super::progress::event("boundary_primitive_skipped", || {
            serde_json::json!({
            "reason":"index-constant divisor","original_weights":weights.len(),"divisor":diagnostic(common.raw())})
        });
        return Ok(None);
    }
    polynomial_input(c, &common, config, limits)?;
    super::progress::event("boundary_primitive_divisor_candidate", || {
        serde_json::json!({
        "original_weights":weights.len(),"divisor":diagnostic(common.raw()),
        "division_authorized":false,"endpoint_image_division":false})
    });
    let mut divided = Weights::new();
    for (&source, weight) in weights.iter() {
        primitive_work(
            weight.raw().numerator.nterms(),
            common.raw().numerator.nterms(),
            operations,
            limits,
        )?;
        let numerator = native(|| weight.raw().numerator.try_div(&common.raw().numerator))?
            .ok_or_else(|| {
                Error::Invalid("primitive original-weight division was not exact".into())
            })?;
        primitive_work(
            numerator.nterms(),
            weight.raw().denominator.nterms(),
            operations,
            limits,
        )?;
        let quotient = primitive_admit(
            c,
            native(|| {
                Coefficient::from_num_den(numerator, weight.raw().denominator.clone(), &Z, true)
            })?,
            admission,
            limits,
        )?;
        polynomial_input(c, &quotient, config, limits)?;
        primitive_work(
            common.raw().numerator.nterms(),
            quotient.raw().numerator.nterms(),
            operations,
            limits,
        )?;
        require(
            c.mul_with_limits(&common, &quotient, limits.arithmetic)? == *weight,
            "primitive original-weight multiplication check failed",
        )?;
        for powers in split(c, &quotient, config, limits, admission, operations)?.keys() {
            let monomial = config
                .polynomial_axes
                .iter()
                .map(|&axis| powers[c.base().parameter_names().len() + axis])
                .collect::<Vec<_>>();
            if !config.weight_monomials.contains(&monomial) {
                super::progress::event("boundary_primitive_ansatz_refused", || {
                    serde_json::json!({
                    "selected_frame_ordinal":source,"quotient_monomial":monomial,
                    "original_weight":diagnostic(weight.raw()),"quotient":diagnostic(quotient.raw())})
                });
                return Err(Error::Invalid(
                    "primitive quotient escaped the declared monomial ansatz".into(),
                ));
            }
        }
        require(
            !quotient.is_zero(),
            "nonzero original weight divided to zero",
        )?;
        bound(
            add(divided.len(), 1)?,
            config.max_nonzeros.min(limits.nonzeros),
            "primitive quotient entries",
        )?;
        divided.insert(source, quotient);
    }
    super::progress::event("boundary_primitive_original_weights", || {
        serde_json::json!({
        "original_weights":weights.len(),"quotient_weights":divided.len(),
        "native_exact_divisions":divided.len(),"independent_multiplication_checks":divided.len(),
        "quotient_ansatz_checked":true,"original_conditions_removed":0,
        "divisor":diagnostic(common.raw()),"endpoint_image_division":false})
    });
    *weights = divided;
    Ok(Some(common))
}

fn primitive_after_reducer(
    c: &IndexedCoefficientContext,
    weights: &mut Weights,
    config: &Config,
    admission: &mut Admission,
    retained_terms: usize,
    operations: &mut usize,
    limits: Limits,
) -> Result<Option<IndexedCoefficient>> {
    // The reducer counter includes the earlier assembly charge and every U/L
    // native payload. Transfer that total, not a second independent allowance.
    require(
        retained_terms >= admission.terms,
        "primitive retained-term chronology",
    )?;
    admission.terms = retained_terms;
    primitive_weights(c, weights, config, admission, operations, limits)
}

fn faces(
    shift: &IndexShift,
    config: &Config,
    charged: &mut usize,
    limits: Limits,
) -> Result<Vec<(usize, i64)>> {
    let mut result = Vec::new();
    for &axis in &config.protected_axes {
        let positive = shift.values()[axis].max(0);
        let count = usize::try_from(positive).map_err(|_| Error::Budget("activation faces"))?;
        *charged = add(*charged, count)?;
        bound(
            *charged,
            config.max_activation_faces.min(limits.operations),
            "activation faces",
        )?;
        result.extend((0..positive).map(|k| (axis, -k)));
    }
    Ok(result)
}

fn restriction(
    c: &IndexedCoefficientContext,
    value: &IndexedCoefficient,
    face: (usize, i64),
    arithmetic: IndexedAlgebraLimits,
    guards: &mut Vec<Guard>,
    limits: Limits,
    operations: &mut usize,
) -> Result<IndexedCoefficient> {
    work(operations, 1, limits)?;
    let (restricted, witness) = c.specialize_fixed_indices(value, &[face], arithmetic)?;
    retain(
        c,
        guards,
        witness,
        "boundary restriction pre-cancellation denominator",
        limits,
    )?;
    Ok(restricted)
}

/// Check after normalization; a raw boundary factor or a discarded face is not enough.
fn verify_faces(
    c: &IndexedCoefficientContext,
    image: &Row,
    config: &Config,
    arithmetic: IndexedAlgebraLimits,
    guards: &mut Vec<Guard>,
    limits: Limits,
    operations: &mut usize,
) -> Result<()> {
    let mut count = 0;
    for (shift, value) in image {
        for face in faces(shift, config, &mut count, limits)? {
            let restricted = restriction(c, value, face, arithmetic, guards, limits, operations)?;
            if !restricted.is_zero() {
                super::progress::event("boundary_face_endpoint_refused", || {
                    serde_json::json!({
                    "axis":face.0,"physical_value":face.1,"endpoint_shift":shift.values(),
                    "normalized_endpoint":diagnostic(value.raw()),"restriction":diagnostic(restricted.raw())})
                });
                return Err(Error::Invalid(
                    "normalized endpoint does not vanish on an activation face".into(),
                ));
            }
            for (ordinal, guard) in guards.iter().enumerate() {
                work(operations, 1, limits)?;
                let on_face =
                    c.specialize_fixed_polynomial(&guard.polynomial, &[face], arithmetic)?;
                if on_face.is_zero() {
                    super::progress::event("boundary_face_guard_refused", || {
                        serde_json::json!({
                        "axis":face.0,"physical_value":face.1,"endpoint_shift":shift.values(),
                        "guard_ordinal":ordinal,"guard_origin":diagnostic(&guard.origin),
                        "guard":diagnostic(guard.polynomial.raw()),"normalized_endpoint_vanishes":true})
                    });
                    return Err(Error::Invalid(
                        "a retained pole/pivot/source condition excludes an entire activation face"
                            .into(),
                    ));
                }
            }
        }
    }
    Ok(())
}

pub fn project(
    c: &IndexedCoefficientContext,
    rows: &[Row],
    target: &IndexShift,
    forbidden: &BTreeSet<IndexShift>,
    input_guards: &[Guard],
    arithmetic: IndexedAlgebraLimits,
    limits: Limits,
    config: &Config,
) -> Result<Projection> {
    require(
        !config.primitive_original_weights,
        "primitive weights require authenticated ordinary source bindings",
    )?;
    project_bound(
        c,
        rows,
        target,
        forbidden,
        input_guards,
        arithmetic,
        limits,
        config,
    )
}

/// The unit frame is injective by (RowId, offset), so the per-row sum over
/// monomials below is already exactly coalesced ORIGINAL-source provenance.
pub fn project_original(
    c: &IndexedCoefficientContext,
    span: &super::source::Span,
    target: &IndexShift,
    forbidden: &BTreeSet<IndexShift>,
    arithmetic: IndexedAlgebraLimits,
    limits: Limits,
    config: &Config,
) -> Result<Projection> {
    span.validate_fresh_ordinary(c, limits)?;
    project_bound(
        c,
        &span.images,
        target,
        forbidden,
        &span.guards,
        arithmetic,
        limits,
        config,
    )
}

fn project_bound(
    c: &IndexedCoefficientContext,
    rows: &[Row],
    target: &IndexShift,
    forbidden: &BTreeSet<IndexShift>,
    input_guards: &[Guard],
    arithmetic: IndexedAlgebraLimits,
    limits: Limits,
    config: &Config,
) -> Result<Projection> {
    config.validate(c.index_count())?;
    require(
        !rows.is_empty() && target.values().len() == c.index_count() && !forbidden.contains(target),
        "boundary source/target shape",
    )?;
    require(
        target.values().iter().all(|&value| value == 0),
        "boundary activation faces require zero target shift",
    )?;
    require(
        forbidden
            .iter()
            .all(|shift| shift.values().len() == c.index_count()),
        "forbidden shift arity",
    )?;
    bound(rows.len(), limits.rows, "boundary original rows")?;
    let unknowns = rows
        .len()
        .checked_mul(config.weight_monomials.len())
        .ok_or(Error::Budget("weight unknowns"))?;
    bound(unknowns, config.max_unknowns, "weight unknowns")?;
    let mut guards = Vec::new();
    for guard in input_guards {
        retain(
            c,
            &mut guards,
            guard.polynomial.clone(),
            guard.origin.clone(),
            limits,
        )?;
    }
    let mut universe = BTreeSet::new();
    let mut operations = 0usize;
    for row in rows {
        for (shift, value) in row {
            require(
                shift.values().len() == c.index_count(),
                "source shift arity",
            )?;
            polynomial_input(c, value, config, limits)?;
            denominator(c, &mut guards, value, "boundary original input", limits)?;
            universe.insert(shift.clone());
        }
    }
    bound(
        universe.len(),
        limits.columns,
        "boundary full image columns",
    )?;
    let mut face_count = 0;
    let face_map = universe
        .iter()
        .map(|s| Ok((s.clone(), faces(s, config, &mut face_count, limits)?)))
        .collect::<Result<BTreeMap<_, _>>>()?;
    let mut monomials = Vec::new();
    for powers in &config.weight_monomials {
        let mut value = c.one();
        for (&axis, &power) in config.polynomial_axes.iter().zip(powers) {
            let index = c.index(axis)?;
            for _ in 0..power {
                work(&mut operations, 1, limits)?;
                value = c.mul_with_limits(&value, &index, limits.arithmetic)?;
            }
        }
        monomials.push(value);
    }
    let mut basis = Vec::new();
    let mut labels = BTreeSet::new();
    let mut target_labels = BTreeSet::new();
    let mut admission = Admission {
        entries: unknowns,
        terms: 0,
    };
    bound(
        admission.entries,
        config.max_nonzeros.min(limits.nonzeros),
        "identity entry admission",
    )?;
    let mut label_count = 0usize;
    let template = config
        .target_template
        .as_ref()
        .map(|spec| {
            compile_template(
                c,
                spec,
                config,
                &mut admission,
                &mut operations,
                &mut guards,
                limits,
            )
        })
        .transpose()?;
    for (source, row) in rows.iter().enumerate() {
        for monomial in &monomials {
            let mut b = BasisRow {
                source,
                monomial: monomial.clone(),
                constraints: BTreeMap::new(),
                target: BTreeMap::new(),
            };
            for (shift, value) in row {
                work(&mut operations, 1, limits)?;
                let product = c.mul_with_limits(monomial, value, limits.arithmetic)?;
                let fs = &face_map[shift];
                if forbidden.contains(shift)
                    || (config.mode == Mode::WholeColumns && !fs.is_empty())
                {
                    for (powers, coefficient) in
                        split(c, &product, config, limits, &mut admission, &mut operations)?
                    {
                        let key = Constraint::Global(shift.clone(), powers);
                        register_label(key.clone(), &mut labels, &mut label_count, config)?;
                        require(
                            b.constraints.insert(key, coefficient).is_none(),
                            "duplicate global constraint",
                        )?;
                    }
                } else if config.mode == Mode::ActivationFaces {
                    for &face in fs {
                        let restricted = restriction(
                            c,
                            &product,
                            face,
                            arithmetic,
                            &mut guards,
                            limits,
                            &mut operations,
                        )?;
                        for (powers, coefficient) in split(
                            c,
                            &restricted,
                            config,
                            limits,
                            &mut admission,
                            &mut operations,
                        )? {
                            let key = Constraint::Face(shift.clone(), face.0, face.1, powers);
                            register_label(key.clone(), &mut labels, &mut label_count, config)?;
                            require(
                                b.constraints.insert(key, coefficient).is_none(),
                                "duplicate face constraint",
                            )?;
                        }
                    }
                }
                if shift == target {
                    for (key, coefficient) in
                        split(c, &product, config, limits, &mut admission, &mut operations)?
                    {
                        register_label(key.clone(), &mut target_labels, &mut label_count, config)?;
                        require(
                            b.target.insert(key, coefficient).is_none(),
                            "duplicate target coefficient",
                        )?;
                    }
                }
            }
            basis.push(b);
        }
    }
    if let Some(template) = &template {
        template_constraints(
            c,
            &mut basis,
            &mut labels,
            &mut target_labels,
            &mut label_count,
            template,
            config,
            &mut admission,
            &mut operations,
            limits,
        )?;
    }
    if target_labels.is_empty() {
        return Ok(Projection::NoTarget {
            guards,
            rows: basis.len(),
        });
    }
    let labels = labels.into_iter().collect::<Vec<_>>();
    let target_labels = target_labels.into_iter().collect::<Vec<_>>();
    let physical = add(labels.len(), target_labels.len())?;
    let width = add(physical, unknowns)?;
    bound(width, limits.columns, "boundary augmented columns")?;
    bound(
        admission.entries,
        config.max_nonzeros.min(limits.nonzeros),
        "boundary augmented input nonzeros",
    )?;
    let native_width = u32::try_from(width).map_err(|_| Error::Budget("boundary native width"))?;
    let mut reducer = native(|| SparseRowReducer::new(native_width, Field::new(Z), LuLMode::Full))?;
    let mut retained_terms = admission.terms;
    super::progress::event("boundary_constraints_ready", || {
        serde_json::json!({
        "unknowns":unknowns,"constraints":labels.len(),"target_coefficient_columns":target_labels.len(),
        "activation_faces":face_count,"coefficient_field":"base parameters only", "mode":format!("{:?}",config.mode)})
    });
    for (ordinal, b) in basis.iter().enumerate() {
        let mut entries = BTreeMap::new();
        for (key, coefficient) in &b.constraints {
            entries.insert(
                labels.binary_search(key).expect("collected constraint"),
                coefficient.clone(),
            );
        }
        for (key, coefficient) in &b.target {
            entries.insert(
                labels.len()
                    + target_labels
                        .binary_search(key)
                        .expect("collected target coefficient"),
                coefficient.clone(),
            );
        }
        entries.insert(physical + ordinal, c.one());
        let ids = entries.keys().map(|&i| i as u32).collect::<Vec<_>>();
        let values = entries
            .values()
            .map(|c| c.raw().clone())
            .collect::<Vec<_>>();
        let pivot = native(|| reducer.add_row(&values, &ids))?.ok_or_else(|| {
            Error::Invalid("identity-augmented boundary row lost independence".into())
        })? as usize;
        bound(
            add(reducer.u().nvalues(), reducer.l().nvalues())?,
            config.max_nonzeros.min(limits.nonzeros),
            "boundary U/L nonzeros",
        )?;
        let (urow, ids, raw) = reducer
            .u()
            .last_row()
            .ok_or_else(|| Error::Invalid("missing boundary U row".into()))?;
        let (lrow, _, lraw) = reducer
            .l()
            .last_row()
            .ok_or_else(|| Error::Invalid("missing boundary L row".into()))?;
        require(
            urow as usize == ordinal && lrow as usize == ordinal,
            "boundary row chronology",
        )?;
        let values = project::admit_values(c, raw, &mut retained_terms, limits)?;
        let lvalues = project::admit_values(c, lraw, &mut retained_terms, limits)?;
        for value in values.iter().chain(&lvalues) {
            scalar(c, value, limits)?;
            denominator(
                c,
                &mut guards,
                value,
                "boundary native field result",
                limits,
            )?;
        }
        if pivot < physical {
            let scale = lvalues
                .last()
                .ok_or_else(|| Error::Invalid("missing boundary pivot".into()))?;
            retain(
                c,
                &mut guards,
                c.numerator_condition_with_limits(scale, limits.arithmetic)?,
                "boundary field pivot numerator",
                limits,
            )?;
        }
        if !(labels.len()..physical).contains(&pivot) {
            continue;
        }
        require(
            ids.iter().all(|&i| i as usize >= labels.len()),
            "boundary target retains a constraint",
        )?;
        let mut weights = Weights::new();
        for (&id, scalar_weight) in ids
            .iter()
            .zip(&values)
            .filter(|(id, _)| **id as usize >= physical)
        {
            let index = id as usize - physical;
            require(index <= ordinal, "boundary provenance beyond visited basis")?;
            let source = basis[index].source;
            work(&mut operations, 1, limits)?;
            let weight =
                c.mul_with_limits(scalar_weight, &basis[index].monomial, limits.arithmetic)?;
            let weight = if let Some(previous) = weights.remove(&source) {
                work(&mut operations, 1, limits)?;
                c.add_with_limits(&previous, &weight, limits.arithmetic)?
            } else {
                weight
            };
            denominator(
                c,
                &mut guards,
                &weight,
                "boundary reconstructed polynomial weight",
                limits,
            )?;
            if !weight.is_zero() {
                weights.insert(source, weight);
            }
        }
        // This is the first target-bearing dependency in declared basis order.
        // A later normalization/guard/proof refusal rejects this candidate only;
        // it does not establish absence of another useful kernel combination.
        if config.primitive_original_weights {
            primitive_after_reducer(
                c,
                &mut weights,
                config,
                &mut admission,
                retained_terms,
                &mut operations,
                limits,
            )?;
        }
        let raw_image = project::replay(c, rows, &weights, &mut guards, limits)?;
        let target_value = raw_image
            .get(target)
            .ok_or_else(|| Error::Invalid("boundary target vanished in full replay".into()))?;
        require(
            !target_value.is_zero() && forbidden.iter().all(|s| !raw_image.contains_key(s)),
            "boundary full target/F replay",
        )?;
        if let Some(template) = &template {
            // Includes the primitive option: dividing original weights may
            // change target shape. Recheck AFTER that change, never before it.
            admission.terms = admission.terms.max(retained_terms);
            verify_template(
                c,
                target_value,
                template,
                config,
                &mut admission,
                &mut operations,
                &mut guards,
                limits,
            )?;
        }
        super::progress::event("boundary_target_before_normalization", || {
            serde_json::json!({
            "prefix_rows":ordinal+1,"original_weights":weights.len(),
            "primitive_original_weights":config.primitive_original_weights,
            "full_original_image_replayed":true,"target":diagnostic(target_value.raw())})
        });
        retain(
            c,
            &mut guards,
            c.numerator_condition_with_limits(target_value, limits.arithmetic)?,
            "boundary target normalization pivot",
            limits,
        )?;
        denominator(
            c,
            &mut guards,
            target_value,
            "boundary target before normalization",
            limits,
        )?;
        for weight in weights.values_mut() {
            work(&mut operations, 1, limits)?;
            *weight = c.div_with_limits(weight, target_value, limits.arithmetic)?;
            denominator(
                c,
                &mut guards,
                weight,
                "boundary normalized source weight",
                limits,
            )?;
        }
        let image = project::replay(c, rows, &weights, &mut guards, limits)?;
        require(
            image.get(target).is_some_and(|v| v.raw().is_one())
                && forbidden.iter().all(|s| !image.contains_key(s)),
            "boundary normalized full replay",
        )?;
        if config.mode == Mode::WholeColumns {
            require(
                image
                    .keys()
                    .all(|s| face_map.get(s).is_none_or(Vec::is_empty)),
                "global-zero control retained a protected endpoint",
            )?;
        }
        verify_faces(
            c,
            &image,
            config,
            arithmetic,
            &mut guards,
            limits,
            &mut operations,
        )?;
        return Ok(Projection::Target(Proposal {
            weights,
            image,
            guards,
            prefix_rows: ordinal + 1,
        }));
    }
    Ok(Projection::NoTarget {
        guards,
        rows: basis.len(),
    })
}

#[cfg(test)]
#[path = "boundary/tests.rs"]
mod tests;
