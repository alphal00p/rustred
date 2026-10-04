//! Input-declared target proportionality over the base field; no extra unknown.
use super::*;

/// Input declarations, not coefficient displays or trusted native objects.
/// i64 integer literals are an intentional bounded input subset.
#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TemplateTerm {
    pub integer: i64,
    pub base_powers: Vec<u16>,
    pub index_powers: Vec<u16>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TargetTemplate {
    pub base_parameters: Vec<String>,
    pub numerator: Vec<TemplateTerm>,
    pub denominator: Vec<TemplateTerm>,
}

pub(super) struct CompiledTemplate {
    pub(super) value: IndexedCoefficient,
    pub(super) coefficients: BTreeMap<Vec<u16>, IndexedCoefficient>,
}

fn template_value_charge(
    value: &IndexedCoefficient,
    admission: &mut Admission,
    limits: Limits,
) -> Result<()> {
    let count = add(
        value.raw().numerator.nterms(),
        value.raw().denominator.nterms(),
    )?;
    let total = add(admission.terms, count)?;
    bound(
        total,
        limits.coefficient_terms,
        "template cumulative coefficient terms",
    )?;
    admission.terms = total;
    Ok(())
}

pub(super) fn compile_template(
    c: &IndexedCoefficientContext,
    spec: &TargetTemplate,
    config: &Config,
    admission: &mut Admission,
    operations: &mut usize,
    guards: &mut Vec<Guard>,
    limits: Limits,
) -> Result<CompiledTemplate> {
    require(
        spec.base_parameters == c.base().parameter_names(),
        "foreign template base map",
    )?;
    require(
        !spec.numerator.is_empty() && !spec.denominator.is_empty(),
        "empty template polynomial",
    )?;
    let terms = add(spec.numerator.len(), spec.denominator.len())?;
    bound(terms, limits.coefficient_terms, "template input terms")?;
    let width = add(c.base().parameter_names().len(), c.index_count())?;
    let coordinates = terms
        .checked_mul(width)
        .ok_or(Error::Budget("template coordinates"))?;
    bound(
        add(admission.entries, coordinates)?,
        config.max_nonzeros.min(limits.nonzeros),
        "template coordinate admission",
    )?;
    // Admit complete coordinate/exponent and operation bounds BEFORE building
    // any powers. Input vectors already belong to the bounded JSON request.
    let mut native_work = 1usize; // final division
    for (denominator, list) in [(false, &spec.numerator), (true, &spec.denominator)] {
        for term in list {
            require(
                term.base_powers.len() == spec.base_parameters.len()
                    && term.index_powers.len() == c.index_count(),
                "template exponent map/arity",
            )?;
            require(
                term.index_powers.iter().enumerate().all(|(axis, &power)| {
                    power == 0 || (!denominator && config.polynomial_axes.contains(&axis))
                }),
                "template has undeclared index support or index-dependent denominator",
            )?;
            for &power in term.base_powers.iter().chain(&term.index_powers) {
                native_work = add(native_work, usize::from(power))?;
            }
            native_work = add(native_work, 1)?; // sum this monomial
        }
    }
    work(operations, native_work, limits)?;
    admission.entries = add(admission.entries, coordinates)?;
    let mut build = |terms: &[TemplateTerm]| -> Result<IndexedCoefficient> {
        let mut sum = c.zero();
        for term in terms {
            let mut value = c.integer(term.integer);
            for (position, &power) in term
                .base_powers
                .iter()
                .chain(&term.index_powers)
                .enumerate()
            {
                if power == 0 {
                    continue;
                }
                let variable = if position < spec.base_parameters.len() {
                    c.lift(
                        &c.base()
                            .parameter(&spec.base_parameters[position])
                            .ok_or_else(|| Error::Invalid("foreign template parameter".into()))?,
                    )?
                } else {
                    c.index(position - spec.base_parameters.len())?
                };
                for _ in 0..power {
                    value = c.mul_with_limits(&value, &variable, limits.arithmetic)?;
                    template_value_charge(&value, admission, limits)?;
                }
            }
            sum = c.add_with_limits(&sum, &value, limits.arithmetic)?;
            template_value_charge(&sum, admission, limits)?;
        }
        Ok(sum)
    };
    let numerator = build(&spec.numerator)?;
    let denominator_value = build(&spec.denominator)?;
    require(
        !numerator.is_zero() && !denominator_value.is_zero(),
        "zero target template numerator/denominator",
    )?;
    // Retain the ORIGINAL declared denominator even if division cancels it.
    retain(
        c,
        guards,
        c.numerator_condition_with_limits(&denominator_value, limits.arithmetic)?,
        "target template raw denominator",
        limits,
    )?;
    let value = c.div_with_limits(&numerator, &denominator_value, limits.arithmetic)?;
    template_value_charge(&value, admission, limits)?;
    let coefficients = split(c, &value, config, limits, admission, operations)?;
    require(!coefficients.is_empty(), "zero target template")?;
    Ok(CompiledTemplate {
        value,
        coefficients,
    })
}

pub(super) fn template_constraints(
    c: &IndexedCoefficientContext,
    basis: &mut [BasisRow],
    labels: &mut BTreeSet<Constraint>,
    targets: &mut BTreeSet<Vec<u16>>,
    label_count: &mut usize,
    template: &CompiledTemplate,
    config: &Config,
    admission: &mut Admission,
    operations: &mut usize,
    limits: Limits,
) -> Result<()> {
    let (pivot, pivot_value) = template
        .coefficients
        .first_key_value()
        .ok_or_else(|| Error::Invalid("zero target template".into()))?;
    scalar(c, pivot_value, limits)?;
    require(!pivot_value.is_zero(), "zero template pivot")?;
    // Count the COMPLETE union without allocating it. Template-only monomials
    // must produce constraints too; omitting them admits false proportionality.
    let missing = template
        .coefficients
        .keys()
        .filter(|m| !targets.contains(*m))
        .count();
    let union_count = add(targets.len(), missing)?;
    let extra = union_count
        .checked_sub(1)
        .ok_or(Error::Budget("template union"))?;
    bound(
        add(add(*label_count, missing)?, extra)?,
        config.max_constraints,
        "complete template union labels",
    )?;
    let columns = add(add(labels.len(), extra)?, 1)?;
    bound(
        add(columns, basis.len())?,
        limits.columns,
        "template augmented columns",
    )?;
    let coordinate_count = union_count
        .checked_mul(add(c.base().parameter_names().len(), c.index_count())?)
        .ok_or(Error::Budget("template union coordinates"))?;
    let entries_upper = basis
        .len()
        .checked_mul(extra)
        .ok_or(Error::Budget("template constraint entries"))?;
    bound(
        add(add(admission.entries, entries_upper)?, coordinate_count)?,
        config.max_nonzeros.min(limits.nonzeros),
        "complete template union entry admission",
    )?;
    work(operations, add(union_count, entries_upper)?, limits)?;
    admission.entries = add(admission.entries, coordinate_count)?;
    let mut union = targets.clone();
    union.extend(template.coefficients.keys().cloned());
    for m in &union {
        if m != pivot {
            register_label(Constraint::Template(m.clone()), labels, label_count, config)?;
        }
    }
    let zero = c.zero();
    for row in basis {
        let cp = row.target.get(pivot).unwrap_or(&zero).clone();
        for m in &union {
            if m == pivot {
                continue;
            }
            let cm = row.target.get(m).unwrap_or(&zero);
            let pm = template.coefficients.get(m).unwrap_or(&zero);
            // No lambda unknown: every row contains C_m P_p - C_p P_m.
            work(operations, 3, limits)?;
            let left = c.mul_with_limits(cm, pivot_value, limits.arithmetic)?;
            template_value_charge(&left, admission, limits)?;
            let right = c.mul_with_limits(&cp, pm, limits.arithmetic)?;
            template_value_charge(&right, admission, limits)?;
            let coefficient = c.sub_with_limits(&left, &right, limits.arithmetic)?;
            template_value_charge(&coefficient, admission, limits)?;
            scalar(c, &coefficient, limits)?;
            if !coefficient.is_zero() {
                admission.entries = add(admission.entries, 1)?;
                bound(
                    admission.entries,
                    config.max_nonzeros.min(limits.nonzeros),
                    "template constraint nonzeros",
                )?;
                require(
                    row.constraints
                        .insert(Constraint::Template(m.clone()), coefficient)
                        .is_none(),
                    "duplicate template constraint",
                )?;
            }
        }
        row.target.clear();
        if !cp.is_zero() {
            row.target.insert(pivot.clone(), cp);
        }
    }
    targets.clear();
    targets.insert(pivot.clone());
    super::super::progress::event("boundary_target_template_ready", || {
        serde_json::json!({"union_monomials":union_count,"proportionality_constraints":extra,
            "canonical_pivot_monomial":pivot,"additional_weight_unknowns":0,
            "template":diagnostic(template.value.raw())})
    });
    Ok(())
}

pub(super) fn verify_template(
    c: &IndexedCoefficientContext,
    target: &IndexedCoefficient,
    template: &CompiledTemplate,
    config: &Config,
    admission: &mut Admission,
    operations: &mut usize,
    guards: &mut Vec<Guard>,
    limits: Limits,
) -> Result<()> {
    let parts = split(c, target, config, limits, admission, operations)?;
    let (pivot, pp) = template
        .coefficients
        .first_key_value()
        .ok_or_else(|| Error::Invalid("zero target template".into()))?;
    let cp = parts
        .get(pivot)
        .ok_or_else(|| Error::Invalid("replayed template pivot is zero".into()))?;
    require(!cp.is_zero(), "replayed template pivot is zero")?;
    denominator(
        c,
        guards,
        cp,
        "replayed template target coefficient",
        limits,
    )?;
    denominator(c, guards, pp, "template pivot coefficient", limits)?;
    retain(
        c,
        guards,
        c.numerator_condition_with_limits(pp, limits.arithmetic)?,
        "template proportionality divisor",
        limits,
    )?;
    work(operations, 2, limits)?;
    let lambda = c.div_with_limits(cp, pp, limits.arithmetic)?;
    template_value_charge(&lambda, admission, limits)?;
    scalar(c, &lambda, limits)?;
    require(!lambda.is_zero(), "zero target template scalar")?;
    denominator(
        c,
        guards,
        &lambda,
        "replayed target template scalar",
        limits,
    )?;
    let expected = c.mul_with_limits(&lambda, &template.value, limits.arithmetic)?;
    template_value_charge(&expected, admission, limits)?;
    require(
        expected == *target,
        "replayed target is not base-field proportional to template",
    )?;
    super::super::progress::event("boundary_target_template_replayed", || {
        serde_json::json!({"base_only_nonzero_scalar":true,"full_proportionality_verified":true,
            "scalar":diagnostic(lambda.raw())})
    });
    Ok(())
}
