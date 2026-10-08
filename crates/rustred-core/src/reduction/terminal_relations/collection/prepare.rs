use super::*;
use crate::family::ContractionMomentum;
use crate::reduction::terminal_normalization::VacuumFamilyAliasError;
use crate::reduction::terminal_relations::{TerminalEquation, sources::Sources};

pub(super) struct Prepared {
    pub families: BTreeMap<String, Arc<IntegralFamily>>,
    pub aliases: VacuumFamilyAliasPlan,
    pub raw: BTreeSet<VacuumIntegralKey>,
    pub targets: BTreeMap<VacuumIntegralKey, VacuumIntegralKey>,
    pub sources: Vec<VacuumDiagonalSource>,
    pub rows: Vec<Row>,
    pub columns: Vec<VacuumIntegralKey>,
    pub auxiliary_count: usize,
    pub conditions: Vec<Coefficient>,
    pub native_rows: usize,
}

impl VacuumDiagonalCollectionPlan {
    /// Prepare a finite collection of scalar vacuum keys. External momenta,
    /// analytic shifts, negative requested powers, and non-unit masses on
    /// requested active denominators are explicitly unsupported. Inactive
    /// scalar-product coordinates may have other constants. No generated
    /// column is clipped, and no numeric value of dimension is sampled.
    pub fn prepare(
        inputs: &[(Arc<IntegralFamily>, BTreeSet<IntegralKey>)],
        limits: VacuumDiagonalCollectionLimits,
    ) -> Result<Self> {
        check("families", inputs.len(), limits.aliases.max_families)?;
        let mut families = BTreeMap::new();
        let mut inventories = BTreeMap::new();
        let mut raw_count = 0usize;
        for (family, keys) in inputs {
            let unsupported = |reason| VacuumCollectionError::Unsupported {
                family: family.fingerprint().to_owned(),
                reason,
            };
            if family.external_count() != 0 {
                return Err(unsupported("external momenta"));
            }
            if family.power_shifts().iter().any(|x| !x.is_zero()) {
                return Err(unsupported("analytic power shifts"));
            }
            for key in keys {
                if key.powers().len() != family.denominator_count() {
                    return Err(VacuumCollectionError::WrongArity);
                }
                if key.powers().iter().any(|&x| x < 0) {
                    return Err(unsupported("numerator-bearing requested key"));
                }
                for (axis, &power) in key.powers().iter().enumerate() {
                    if power > 0
                        && family.denominators()[axis].constant()
                            != &family.coefficient_context().integer(-1)
                    {
                        return Err(unsupported(
                            "active denominator must have stored unit-mass constant -1",
                        ));
                    }
                }
            }
            raw_count = raw_count
                .checked_add(keys.len())
                .ok_or(VacuumCollectionError::ArithmeticOverflow)?;
            check(
                "requested terminals",
                raw_count,
                limits.aliases.max_terminals,
            )?;
            check("requested terminals", raw_count, limits.max_columns)?;
            if families
                .insert(family.fingerprint().to_owned(), Arc::clone(family))
                .is_some()
            {
                return Err(VacuumCollectionError::Alias(
                    VacuumFamilyAliasError::DuplicateFamily,
                ));
            }
            inventories.insert(family.fingerprint().to_owned(), keys.clone());
        }
        // Bind coefficient maps, dimension and loop count before combining
        // coefficients from different families or preparing source rows.
        if let Some(first) = families.values().next() {
            for family in families.values() {
                if family.loop_count() != first.loop_count()
                    || family.dimension() != first.dimension()
                    || !family
                        .coefficient_context()
                        .has_same_variable_map(first.coefficient_context())
                {
                    return Err(VacuumCollectionError::Alias(
                        VacuumFamilyAliasError::IncompatibleFamilies,
                    ));
                }
            }
        }
        let mut support = inventories.clone();
        let mut sources = Vec::new();
        let mut conditions = Vec::new();
        let mut native_rows = 0usize;
        let mut source_terms = 0usize;
        let mut condition_occurrences = 0usize;
        let mut template_rows = 0usize;
        for (fingerprint, family) in &families {
            let corners: BTreeSet<_> = inventories[fingerprint]
                .iter()
                .map(|key| {
                    IntegralKey::try_new(
                        key.powers()
                            .iter()
                            .map(|&p| i64::from(p > 0))
                            .collect::<Vec<_>>(),
                    )
                })
                .collect::<std::result::Result<_, _>>()
                .map_err(algebra)?;
            check(
                "corner seeds",
                sources.len().saturating_add(corners.len()),
                limits.max_corner_seeds,
            )?;
            if corners.is_empty() {
                continue;
            }
            template_rows = template_rows
                .checked_add(
                    family
                        .loop_count()
                        .checked_mul(family.loop_count())
                        .ok_or(VacuumCollectionError::ArithmeticOverflow)?,
                )
                .ok_or(VacuumCollectionError::ArithmeticOverflow)?;
            check(
                "ordinary source templates",
                template_rows,
                limits.max_source_rows,
            )?;
            check(
                "native source rows",
                native_rows.saturating_add(corners.len().saturating_mul(family.loop_count())),
                limits.max_source_rows,
            )?;
            let generator = Sources::new(family)?;
            let ordinals: Vec<_> = (0..generator.len())
                .filter(|&ordinal| match generator.row_id(ordinal) {
                    Some(RowId::OrdinaryIbp {
                        contraction_momentum,
                        differentiated_loop,
                    }) => {
                        family.contraction_momenta()[*contraction_momentum]
                            == ContractionMomentum::Loop(*differentiated_loop)
                    }
                    _ => false,
                })
                .collect();
            if ordinals.len() != family.loop_count() {
                return Err(VacuumCollectionError::ReplayFailed);
            }
            for corner in corners {
                let mut original = Vec::new();
                let mut sum = BTreeMap::new();
                let mut sum_conditions = Vec::new();
                for &ordinal in &ordinals {
                    let (terms, row_conditions) =
                        generator.row(&corner, ordinal, limits.algebra)?;
                    native_rows += 1;
                    source_terms = source_terms.saturating_add(terms.len());
                    check("stored source terms", source_terms, limits.max_source_terms)?;
                    condition_occurrences =
                        condition_occurrences.saturating_add(row_conditions.len());
                    check(
                        "stored source conditions",
                        condition_occurrences,
                        limits.max_conditions,
                    )?;
                    for condition in &row_conditions {
                        retain(
                            &mut sum_conditions,
                            condition.clone(),
                            limits.max_conditions,
                        )?;
                        retain(&mut conditions, condition.clone(), limits.max_conditions)?;
                    }
                    for (key, value) in &terms {
                        accumulate(
                            &mut sum,
                            key.clone(),
                            value,
                            family.coefficient_context(),
                            limits,
                        )?;
                    }
                    original.push(VacuumOrdinarySource {
                        row_id: generator
                            .row_id(ordinal)
                            .ok_or(VacuumCollectionError::ReplayFailed)?
                            .clone(),
                        equation: TerminalEquation {
                            terms,
                            nonzero_conditions: row_conditions,
                        },
                    });
                }
                source_terms = source_terms.saturating_add(sum.len());
                check("stored source terms", source_terms, limits.max_source_terms)?;
                condition_occurrences = condition_occurrences.saturating_add(sum_conditions.len());
                check(
                    "stored source conditions",
                    condition_occurrences,
                    limits.max_conditions,
                )?;
                support
                    .get_mut(fingerprint)
                    .expect("declared family")
                    .extend(sum.keys().cloned());
                check(
                    "source support columns",
                    support.values().map(BTreeSet::len).sum(),
                    limits.max_columns,
                )?;
                sources.push(VacuumDiagonalSource {
                    family: fingerprint.clone(),
                    corner,
                    rows: original,
                    sum: TerminalEquation {
                        terms: sum,
                        nonzero_conditions: sum_conditions,
                    },
                });
            }
        }
        let alias_inputs: Vec<_> = families
            .iter()
            .map(|(id, f)| (Arc::clone(f), support[id].clone()))
            .collect();
        let aliases = VacuumFamilyAliasPlan::prepare(&alias_inputs, limits.aliases)?;
        let mut raw = BTreeSet::new();
        let mut targets = BTreeMap::new();
        for key in aliases.raw_terminals() {
            if inventories[key.family_fingerprint()].contains(key.integral()) {
                raw.insert(key.clone());
                let rep = aliases
                    .representative(&families[key.family_fingerprint()], key.integral())
                    .map_err(|e| VacuumCollectionError::Alias(e.into()))?;
                // Iteration is family/key ordered: choose an actual requested
                // member, even if the global representative was auxiliary.
                targets.entry(rep.clone()).or_insert_with(|| key.clone());
            }
        }
        let mut rows = Vec::new();
        let mut auxiliaries = BTreeSet::new();
        for source in &sources {
            let family = &families[&source.family];
            let mut row = Row::new();
            for (key, value) in &source.sum.terms {
                let rep = aliases
                    .representative(family, key)
                    .map_err(|e| VacuumCollectionError::Alias(e.into()))?;
                accumulate(
                    &mut row,
                    rep.clone(),
                    value,
                    family.coefficient_context(),
                    limits,
                )?;
            }
            auxiliaries.extend(
                row.keys()
                    .filter(|key| !targets.contains_key(*key))
                    .cloned(),
            );
            rows.push(row);
        }
        let mut ordered = Vec::new();
        for (canonical, requested) in &targets {
            let mut powers = 0u128;
            let mut excess = 0u128;
            for &power in requested.integral().powers() {
                powers = powers
                    .checked_add(power.max(0) as u128)
                    .ok_or(VacuumCollectionError::ArithmeticOverflow)?;
                excess = excess
                    .checked_add(power.saturating_sub(1).max(0) as u128)
                    .ok_or(VacuumCollectionError::ArithmeticOverflow)?;
            }
            ordered.push((
                std::cmp::Reverse(powers),
                std::cmp::Reverse(excess),
                requested.clone(),
                canonical.clone(),
            ));
        }
        ordered.sort();
        let auxiliary_count = auxiliaries.len();
        let columns: Vec<_> = auxiliaries
            .into_iter()
            .chain(ordered.into_iter().map(|(_, _, _, key)| key))
            .collect();
        check("quotient columns", columns.len(), limits.max_columns)?;
        super::elimination::finish(
            Prepared {
                families,
                aliases,
                raw,
                targets,
                sources,
                rows,
                columns,
                auxiliary_count,
                conditions,
                native_rows,
            },
            limits,
        )
    }
}
