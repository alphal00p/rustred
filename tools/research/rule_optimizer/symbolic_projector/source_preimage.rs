//! Complete raw ordinary support preimages and exact unnormalized pairings.
//! A caller may retain its historical nonpositive-pinch filter; the general
//! source diagnostic passes no filter. No source bank is changed here.
use super::*;
use rustred::{
    algebra::IndexedCoefficientContext,
    identity::{RowId, SelectedTranslatedSourceBatch},
};

pub(super) type Census = BTreeMap<TranslatedSourceRequest, BTreeSet<(usize, IndexShift)>>;

pub(super) fn census(
    c: &IndexedCoefficientContext,
    inventory: &SelectedTranslatedSourceBatch,
    columns: &[IndexShift],
    values: &[rustred::algebra::IndexedCoefficient],
    pin: Option<(usize, i64)>,
    coordinate_cap: usize,
    limits: project::Limits,
) -> Result<(Census, usize)> {
    require(
        columns.len() == values.len() && pin.is_none_or(|(axis, _)| axis < c.index_count()),
        "nomination witness shape differs",
    )?;
    require(
        columns.windows(2).all(|w| w[0] < w[1]),
        "nomination columns not canonical",
    )?;
    require(
        inventory.is_complete_ordinary()
            && inventory.len() == inventory.completed_source_row_count()
            && inventory.context_fingerprint() == c.fingerprint(),
        "raw ordinary inventory scope differs",
    )?;
    for (i, source) in inventory.sources().iter().enumerate() {
        require(
            source.provenance().source_ordinal() == i
                && matches!(source.row_id(), RowId::OrdinaryIbp { .. })
                && source
                    .provenance()
                    .offset()
                    .values()
                    .iter()
                    .all(|&v| v == 0),
            "nomination requires complete zero-offset raw inventory before fixed specialization",
        )?;
        for (shift, value) in source.terms() {
            require(
                shift.values().len() == c.index_count(),
                "raw shift arity differs",
            )?;
            checked(c.validate_with_limits(value, limits.arithmetic))?;
            require(!value.is_zero(), "explicit zero in raw ordinary support")?;
        }
    }
    for value in values {
        checked(c.validate_with_limits(value, limits.arithmetic))?;
    }
    let support = values
        .iter()
        .enumerate()
        .filter_map(|(i, v)| (!v.is_zero()).then_some(i))
        .collect::<Vec<_>>();
    require(!support.is_empty(), "empty separator support")?;
    let raw_terms = inventory.sources().iter().try_fold(0usize, |sum, row| {
        sum.checked_add(row.terms().len())
            .ok_or("raw support count overflow")
    })?;
    let pairs = raw_terms
        .checked_mul(support.len())
        .ok_or("preimage pair count overflow")?;
    checked(project::bound(
        pairs,
        limits.operations,
        "raw preimage enumeration work",
    ))?;
    checked(project::bound(
        pairs,
        limits.nonzeros,
        "raw preimage binding storage",
    ))?;
    checked(project::bound(
        pairs
            .checked_mul(c.index_count())
            .ok_or("preimage coordinate overflow")?,
        coordinate_cap,
        "raw preimage coordinates",
    ))?;
    let mut found = Census::new();
    let mut pin_matches = 0usize;
    for source in inventory.sources() {
        for tau in source.terms().keys() {
            for &column in &support {
                let offset = columns[column]
                    .values()
                    .iter()
                    .zip(tau.values())
                    .map(|(&f, &t)| f.checked_sub(t).ok_or("source preimage offset overflow"))
                    .collect::<std::result::Result<Vec<_>, _>>()?;
                if pin.is_some_and(|(axis, value)| i128::from(value) + i128::from(offset[axis]) > 0)
                {
                    continue;
                }
                pin_matches += 1;
                let request = TranslatedSourceRequest::new(
                    source.provenance().source_ordinal(),
                    checked(IntegralShift::try_new(offset))?,
                );
                found
                    .entry(request)
                    .or_default()
                    .insert((column, tau.clone()));
            }
        }
    }
    Ok((found, pin_matches))
}

pub(super) fn pairing(
    c: &IndexedCoefficientContext,
    row: &project::Row,
    columns: &[IndexShift],
    values: &[rustred::algebra::IndexedCoefficient],
    guards: &mut Vec<project::Guard>,
    operations: &mut usize,
    limits: project::Limits,
) -> Result<rustred::algebra::IndexedCoefficient> {
    require(
        columns.len() == values.len(),
        "pairing witness shape differs",
    )?;
    let mut sum = c.zero();
    for value in values {
        checked(project::denominator(
            c,
            guards,
            value,
            "nomination separator denominator",
            limits,
        ))?;
    }
    for (shift, value) in row {
        checked(c.validate_with_limits(value, limits.arithmetic))?;
        checked(project::denominator(
            c,
            guards,
            value,
            "nomination full image denominator",
            limits,
        ))?;
        if let Ok(i) = columns.binary_search(shift) {
            *operations = operations
                .checked_add(2)
                .ok_or("nomination operation overflow")?;
            checked(project::bound(
                *operations,
                limits.operations,
                "all nomination pairings",
            ))?;
            let product = checked(c.mul_with_limits(value, &values[i], limits.arithmetic))?;
            checked(project::denominator(
                c,
                guards,
                &product,
                "nomination pairing product",
                limits,
            ))?;
            sum = checked(c.add_with_limits(&sum, &product, limits.arithmetic))?;
            checked(project::denominator(
                c,
                guards,
                &sum,
                "nomination pairing sum",
                limits,
            ))?;
        }
    }
    Ok(sum)
}
