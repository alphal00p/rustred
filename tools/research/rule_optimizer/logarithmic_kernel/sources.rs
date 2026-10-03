//! Full native parametric images, then exact physical-point specialization.
use super::*;
use rustred::{
    algebra::{IndexedCoefficient, IndexedCoefficientContext, IndexedPolynomial},
    identity::{IndexShift, SelectedTranslatedSourceBatch},
};

type Image = BTreeMap<IndexShift, IndexedCoefficient>;
pub(super) struct Images {
    pub rows: Vec<Image>,
    pub report: Value,
}

fn charge_terms(c: &Coefficient, total: &mut usize, r: &Value) -> Result<()> {
    *total = add(*total, add(c.numerator.nterms(), c.denominator.nterms())?)?;
    admit(r, "max_coefficient_terms", *total)
}
fn retain_guard(guards: &mut Vec<Value>, guard: Value, r: &Value) -> Result<()> {
    admit(r, "max_conditions", add(guards.len(), 1)?)?;
    guards.push(guard);
    Ok(())
}
pub(super) fn add_image(
    row: &mut Image,
    shift: &IndexShift,
    value: IndexedCoefficient,
    context: &IndexedCoefficientContext,
) -> Result<()> {
    let value = match row.remove(shift) {
        Some(old) => checked(context.add_with_limits(&old, &value, Default::default()))?,
        None => value,
    };
    if !value.is_zero() {
        row.insert(shift.clone(), value);
    }
    Ok(())
}
fn parametric_frame(
    rows: &[Image],
    columns: &BTreeMap<IndexShift, u32>,
    r: &Value,
) -> Result<Matrix> {
    let mut frame = Matrix::new(0, checked(u32::try_from(columns.len()))?, field());
    for row in rows {
        let ordered = row
            .iter()
            .map(|(k, v)| (columns[k], v.raw().clone()))
            .collect::<BTreeMap<_, _>>();
        admit(
            r,
            "max_input_nonzeros",
            add(frame.nvalues(), ordered.len())?,
        )?;
        frame.add_row(
            ordered.values().cloned().collect(),
            ordered.keys().copied().collect(),
        );
    }
    Ok(frame)
}
pub(super) fn images(
    family: &IntegralFamily,
    generator: &ParametricIbpGenerator<'_>,
    batch: &SelectedTranslatedSourceBatch,
    source_indices: &[usize],
    weights: &Matrix,
    r: &Value,
) -> Result<Images> {
    let context = generator.context();
    let base = family.coefficient_context();
    require(
        batch.family_fingerprint() == family.fingerprint()
            && batch.context_fingerprint() == context.fingerprint(),
        "translated source context differs",
    )?;
    require(
        weights.ncols() as usize == source_indices.len(),
        "source/kernel width mismatch",
    )?;
    let mut guards = Vec::new();
    let mut coefficient_terms = 0;
    let mut columns = BTreeSet::new();
    let mut originals = Vec::new();
    for condition in family.domain().conditions() {
        retain_guard(
            &mut guards,
            json!({"kind":"native_family_domain_nonzero","polynomial":condition.polynomial().to_string(),"origins":format!("{:?}",condition.sources())}),
            r,
        )?;
    }
    for (row, &index) in source_indices.iter().enumerate() {
        let source = &batch.sources()[index];
        for condition in source.nonzero_conditions() {
            retain_guard(
                &mut guards,
                json!({"kind":"original_source_nonzero","unknown":row,"polynomial":condition.polynomial().raw().to_string(),"origins":format!("{:?}",condition.sources())}),
                r,
            )?;
        }
        for (shift, value) in source.terms() {
            charge_terms(value.raw(), &mut coefficient_terms, r)?;
            let guard =
                checked(context.denominator_condition_with_limits(value, Default::default()))?;
            require(!guard.is_zero(), "original source denominator zero")?;
            retain_guard(
                &mut guards,
                json!({"kind":"original_term_denominator_before_product","unknown":row,"shift":shift.values(),"polynomial":guard.raw().to_string()}),
                r,
            )?;
            columns.insert(shift.clone());
        }
        admit(r, "max_physical_columns", columns.len())?;
        originals.push(source.terms().clone());
    }
    let registry = columns
        .into_iter()
        .enumerate()
        .map(|(i, k)| (k, i as u32))
        .collect::<BTreeMap<_, _>>();
    let original_frame = parametric_frame(&originals, &registry, r)?;
    // Preflight before any weighted image multiplication.
    let mut operations = 0;
    for &index in weights.col_idcs() {
        operations = add(operations, originals[index as usize].len())?;
    }
    admit(r, "max_product_terms", operations)?;
    admit(r, "max_exact_operations", mul(operations, 2)?)?;
    let mut rows = Vec::new();
    let mut image_report = Vec::new();
    let mut lifted_weights = Matrix::new(0, weights.ncols(), field());
    for row in 0..weights.nrows() as usize {
        let mut image = Image::new();
        let mut lifted = Vec::new();
        let mut ids = Vec::new();
        for at in weights.row_ptrs()[row]..weights.row_ptrs()[row + 1] {
            let index = weights.col_idcs()[at] as usize;
            let weight = &weights.values()[at];
            checked(base.validate_with_limits(weight, Default::default()))?;
            charge_terms(weight, &mut coefficient_terms, r)?;
            retain_guard(
                &mut guards,
                json!({"kind":"kernel_weight_denominator_before_product","kernel_row":row,"unknown":index,"polynomial":weight.denominator.to_string()}),
                r,
            )?;
            let weight = checked(context.lift(weight))?;
            ids.push(index as u32);
            lifted.push(weight.raw().clone());
            for (shift, value) in &originals[index] {
                let product = checked(context.mul_with_limits(&weight, value, Default::default()))?;
                charge_terms(product.raw(), &mut coefficient_terms, r)?;
                add_image(&mut image, shift, product, context)?;
            }
        }
        lifted_weights.add_row(lifted, ids);
        image_report.push(json!({"kernel_row":row,"true_parametric_zero":image.is_empty(),"terms":image.iter().map(|(shift,c)|json!({"shift":shift.values(),"coefficient":coefficient(c.raw())})).collect::<Vec<_>>()}));
        rows.push(image);
    }
    let image_frame = parametric_frame(&rows, &registry, r)?;
    let native_replay = linear::product(&lifted_weights, &original_frame, r)?;
    require(
        native_replay == image_frame,
        "full parametric original-source replay differs",
    )?;
    let rank = if r["compute_parametric_image_rank"]
        .as_bool()
        .unwrap_or(false)
    {
        let mut result = linear::rank(&image_frame, r)?;
        result["coefficient_field"] = json!("K(n)");
        result
    } else {
        json!({"rank":null,"status":"not_computed","coefficient_field":"K(n)"})
    };
    let zeros = rows.iter().filter(|row| row.is_empty()).count();
    Ok(Images {
        report: json!({"kernel_rows":rows.len(),"true_parametric_zero_images":zeros,"nonzero_parametric_images":rows.len()-zeros,
        "counts_are_not_image_rank":true,"image_rank":rank,"all_kernel_vectors_retained_for_point":true,
        "weighted_term_products":operations,"full_original_parametric_replay_checked":true,
        "original_shift_columns":registry.iter().map(|(s,c)|json!({"column":c,"shift":s.values()})).collect::<Vec<_>>(),
        "original_translated_source_matrix":matrix_rows(&original_frame),"images":image_report,"pre_cancellation_conditions":guards}),
        rows,
    })
}

fn physical<const N: usize>(assignment: &[i64], shift: &IndexShift) -> Result<Integral<N>> {
    require(
        assignment.len() == N && shift.values().len() == N,
        "physical shift arity mismatch",
    )?;
    let mut power = [0_i16; N];
    for axis in 0..N {
        power[axis] = checked(i16::try_from(
            assignment[axis]
                .checked_add(shift.values()[axis])
                .ok_or("physical index overflow")?,
        ))?;
    }
    checked(Integral::numeric(power))
}
fn point_guard(
    context: &IndexedCoefficientContext,
    guard: &IndexedPolynomial,
    assignment: &[i64],
    label: Value,
    guards: &mut Vec<Value>,
    r: &Value,
) -> Result<()> {
    let polynomial = checked(context.specialize_polynomial(guard, assignment, Default::default()))?;
    require(
        !polynomial.is_zero(),
        "pre-cancellation condition vanishes at frozen point",
    )?;
    retain_guard(
        guards,
        json!({"origin":label,"nonzero":polynomial.to_string()}),
        r,
    )
}
pub(super) fn specialize_row<const N: usize>(
    row: &Image,
    context: &IndexedCoefficientContext,
    base: &CoefficientContext,
    assignment: &[i64],
    guards: &mut Vec<Value>,
    r: &Value,
) -> Result<BTreeMap<Integral<N>, Coefficient>> {
    let mut result = BTreeMap::new();
    let mut terms = 0;
    for (shift, value) in row {
        let k = physical(assignment, shift)?;
        let (value, guard) = checked(context.specialize(value, assignment, Default::default()))?;
        require(base.contains(&value), "point coefficient map differs")?;
        charge_terms(&value, &mut terms, r)?;
        if let Some(guard) = guard {
            require(!guard.is_zero(), "point denominator vanishes")?;
            retain_guard(
                guards,
                json!({"kind":"specialized_term_denominator_before_normalization","shift":shift.values(),"nonzero":guard.to_string()}),
                r,
            )?;
        }
        if !value.is_zero() {
            require(
                result.insert(k, value).is_none(),
                "translation physical map not injective",
            )?;
        }
    }
    Ok(result)
}
fn physical_frame<const N: usize>(
    rows: &[BTreeMap<Integral<N>, Coefficient>],
    registry: &BTreeMap<Integral<N>, u32>,
    r: &Value,
) -> Result<Matrix> {
    let mut frame = Matrix::new(0, checked(u32::try_from(registry.len()))?, field());
    for row in rows {
        let ordered = row
            .iter()
            .map(|(k, v)| (registry[k], v.clone()))
            .collect::<BTreeMap<_, _>>();
        admit(
            r,
            "max_input_nonzeros",
            add(frame.nvalues(), ordered.len())?,
        )?;
        frame.add_row(
            ordered.values().cloned().collect(),
            ordered.keys().copied().collect(),
        );
    }
    Ok(frame)
}
pub(super) fn point<const N: usize>(
    family: &IntegralFamily,
    generator: &ParametricIbpGenerator<'_>,
    batch: &SelectedTranslatedSourceBatch,
    source_indices: &[usize],
    kernel: &Matrix,
    images: &Images,
    target: &Integral<N>,
    order: &IntegralOrder<N>,
    r: &Value,
) -> Result<Value> {
    let context = generator.context();
    let base = family.coefficient_context();
    let assignment = powers(target)
        .into_iter()
        .map(i64::from)
        .collect::<Vec<_>>();
    let mut guards = Vec::new();
    let mut rows = Vec::new();
    let mut columns = BTreeSet::from([*target]);
    for &index in source_indices {
        let source = &batch.sources()[index];
        for condition in source.nonzero_conditions() {
            point_guard(
                context,
                condition.polynomial(),
                &assignment,
                json!({"source":source.provenance().stable_string(),"kind":"original_source_condition"}),
                &mut guards,
                r,
            )?;
        }
        let row = specialize_row::<N>(source.terms(), context, base, &assignment, &mut guards, r)?;
        columns.extend(row.keys().copied());
        admit(r, "max_physical_columns", columns.len())?;
        rows.push(row);
    }
    let mut image_rows = Vec::new();
    for image in &images.rows {
        image_rows.push(specialize_row::<N>(
            image,
            context,
            base,
            &assignment,
            &mut guards,
            r,
        )?);
    }
    let columns = columns.into_iter().collect::<Vec<_>>();
    let registry = columns
        .iter()
        .enumerate()
        .map(|(i, &k)| (k, i as u32))
        .collect::<BTreeMap<_, _>>();
    require(
        image_rows
            .iter()
            .all(|row| row.keys().all(|k| registry.contains_key(k))),
        "image contains a non-original physical key",
    )?;
    let original = physical_frame(&rows, &registry, r)?;
    let image_frame = physical_frame(&image_rows, &registry, r)?;
    let replay = linear::product(kernel, &original, r)?;
    require(
        replay == image_frame,
        "specialized full source replay differs from parametric image specialization",
    )?;
    let point_rank = linear::rank(&image_frame, r)?;
    let solved = linear::target_pivot(&image_frame, &columns, target, order, &base.one(), r)?;
    let composed = if let Some(weights) = solved.weights {
        let composed = linear::product(&weights, kernel, r)?;
        let full_replay = linear::product(&composed, &original, r)?;
        let image_replay = linear::product(&weights, &image_frame, r)?;
        require(
            full_replay == image_replay,
            "composed original source replay differs",
        )?;
        let tail = linear::full_tail(&full_replay, &columns, target, order)?;
        json!({"kernel_row_weights":matrix_rows(&weights),"original_source_weights":matrix_rows(&composed),"rhs":tail,
            "all_original_columns_replayed":true,"all_F_zero":true,"target_coefficient_one":true,"same_saved_order_strict_descent":true})
    } else {
        Value::Null
    };
    Ok(
        json!({"target":powers(target),"recenter":r["recenter"],"physical_columns":columns.iter().map(powers).collect::<Vec<_>>(),
        "full_source_specialization_and_kernel_replay_equal":true,"point_image_rank_over_K":point_rank,
        "true_parametric_zero_count":images.rows.iter().filter(|x|x.is_empty()).count(),
        "nonzero_parametric_images_vanishing_at_point":images.rows.iter().zip(&image_rows).filter(|(a,b)|!a.is_empty()&&b.is_empty()).count(),
        "specialization_guards_before_cancellation":guards,"result":solved.report,"original_source_replay":composed,
        "all_kernel_images_including_zero_rows_passed_to_F_solver":true,"kernel_vector_subset_selection":false}),
    )
}
