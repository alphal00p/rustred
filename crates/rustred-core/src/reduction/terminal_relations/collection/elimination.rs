//! Native sparse elimination and explicit full-row provenance replay.
use super::prepare::Prepared;
use super::*;
use symbolica::domains::SelfRing;
use symbolica::{
    domains::rational_polynomial::RationalPolynomialField,
    prelude::{IntegerRing, Z},
    tensors::sparse::{LuLMode, SparseMatrix, SparseRowReducer},
};
type Matrix = SparseMatrix<RationalPolynomialField<IntegerRing, u16>>;

fn native_row(matrix: &Matrix, row: usize) -> impl Iterator<Item = (u32, &Coefficient)> {
    let range = matrix.row_ptrs()[row]..matrix.row_ptrs()[row + 1];
    matrix.col_idcs()[range.clone()]
        .iter()
        .copied()
        .zip(&matrix.values()[range])
}
fn checked_matrix(
    matrix: &Matrix,
    context: &CoefficientContext,
    limits: VacuumDiagonalCollectionLimits,
) -> Result<()> {
    check(
        "native matrix nonzeros",
        matrix.nvalues(),
        limits.max_reducer_nonzeros,
    )?;
    let mut terms = 0usize;
    for value in matrix.values() {
        context
            .validate_with_limits(value, limits.algebra.exact_algebra)
            .map_err(algebra)?;
        terms = terms
            .saturating_add(value.numerator.nterms())
            .saturating_add(value.denominator.nterms());
        check(
            "native coefficient terms",
            terms,
            limits.max_coefficient_terms,
        )?;
    }
    Ok(())
}

pub(super) fn finish(
    mut p: Prepared,
    limits: VacuumDiagonalCollectionLimits,
) -> Result<VacuumDiagonalCollectionPlan> {
    let sources = std::mem::take(&mut p.sources);
    let mut result = finish_rows(p, limits, false, None)?;
    result.statistics.corner_seeds = sources.len();
    Ok(VacuumDiagonalCollectionPlan {
        families: result.families,
        raw: result.raw,
        remaining: result.remaining,
        aliases: result.aliases,
        sources,
        equations: result.equations,
        reductions: result.reductions,
        conditions: result.conditions,
        statistics: result.statistics,
    })
}

/// Exact finite rowspace result. Source authority belongs to the caller:
/// diagonal ordinary sums and inherited finite session rows share this native
/// elimination/replay kernel without claiming the same source provenance.
#[derive(Clone, Debug)]
pub(super) struct Eliminated {
    pub families: BTreeMap<String, Arc<IntegralFamily>>,
    pub raw: BTreeSet<VacuumIntegralKey>,
    pub remaining: BTreeSet<VacuumIntegralKey>,
    pub aliases: VacuumFamilyAliasPlan,
    pub equations: Vec<VacuumCollectionEquation>,
    pub reductions: BTreeMap<String, BTreeMap<IntegralKey, GuardedVacuumReduction>>,
    pub conditions: Arc<Vec<Coefficient>>,
    pub statistics: VacuumCollectionStatistics,
}

pub(super) fn finish_rows(
    mut p: Prepared,
    limits: VacuumDiagonalCollectionLimits,
    terminal_only_back_substitution: bool,
    cancel: Option<&std::sync::atomic::AtomicBool>,
) -> Result<Eliminated> {
    let conditions = Arc::new(p.conditions.clone());
    if p.raw.is_empty() {
        return Ok(Eliminated {
            reductions: p
                .families
                .keys()
                .cloned()
                .map(|f| (f, BTreeMap::new()))
                .collect(),
            families: p.families,
            raw: p.raw,
            remaining: BTreeSet::new(),
            aliases: p.aliases,
            equations: Vec::new(),
            conditions,
            statistics: Default::default(),
        });
    }
    let owner = Arc::clone(
        p.families
            .values()
            .next()
            .ok_or(VacuumCollectionError::WrongFamily)?,
    );
    let context = owner.coefficient_context();
    let field = RationalPolynomialField::new(Z);
    let width = p
        .columns
        .len()
        .checked_add(p.rows.len())
        .ok_or(VacuumCollectionError::ArithmeticOverflow)?;
    let width = u32::try_from(width).map_err(|_| VacuumCollectionError::ArithmeticOverflow)?;
    let physical =
        u32::try_from(p.columns.len()).map_err(|_| VacuumCollectionError::ArithmeticOverflow)?;
    let offsets: BTreeMap<_, _> = p
        .columns
        .iter()
        .cloned()
        .enumerate()
        .map(|(i, k)| (k, i as u32))
        .collect();
    let mut sources = Matrix::new(0, physical, field.clone());
    for row in &p.rows {
        let mut entries: Vec<_> = row.iter().map(|(k, v)| (offsets[k], v.clone())).collect();
        entries.sort_by_key(|(i, _)| *i);
        sources.add_row(
            entries.iter().map(|(_, v)| v.clone()).collect(),
            entries.iter().map(|(i, _)| *i).collect(),
        );
    }
    checked_matrix(&sources, context, limits)?;
    let mut reducer = SparseRowReducer::new(width, field.clone(), LuLMode::Full);
    for row in 0..sources.nrows() as usize {
        if cancel.is_some_and(|flag| flag.load(std::sync::atomic::Ordering::Relaxed)) {
            return Err(VacuumCollectionError::Cancelled);
        }
        check(
            "prospective reducer nonzeros",
            reducer
                .u()
                .nvalues()
                .saturating_add(reducer.l().nvalues())
                .saturating_add((width as usize).saturating_mul(2)),
            limits.max_reducer_nonzeros,
        )?;
        let mut entries: Vec<_> = native_row(&sources, row)
            .map(|(i, v)| (i, v.clone()))
            .collect();
        entries.push((physical + row as u32, context.one()));
        let ids: Vec<_> = entries.iter().map(|(i, _)| *i).collect();
        let values: Vec<_> = entries.into_iter().map(|(_, v)| v).collect();
        if reducer.add_row(&values, &ids).is_none() {
            return Err(VacuumCollectionError::ReplayFailed);
        }
        // Native L*U=A: the final L value is the uncancelled pivot scale.
        // Capture both parts before native back substitution clears L.
        let (_, _, values) = reducer
            .l()
            .last_row()
            .ok_or(VacuumCollectionError::ReplayFailed)?;
        let scale = values.last().ok_or(VacuumCollectionError::ReplayFailed)?;
        retain(
            &mut p.conditions,
            scale.numerator.clone().into(),
            limits.max_conditions,
        )?;
        retain(
            &mut p.conditions,
            scale.denominator.clone().into(),
            limits.max_conditions,
        )?;
        check(
            "reducer nonzeros",
            reducer.u().nvalues().saturating_add(reducer.l().nvalues()),
            limits.max_reducer_nonzeros,
        )?;
    }
    // The inputs crossed the checked coefficient boundary once; Symbolica's
    // internal updates preserve the field. Validate retained coefficients at
    // phase boundaries, not by rescanning every preceding row per insertion.
    checked_matrix(reducer.u(), context, limits)?;
    checked_matrix(reducer.l(), context, limits)?;
    let reducer_nonzeros = reducer.u().nvalues().saturating_add(reducer.l().nvalues());
    if terminal_only_back_substitution {
        // Auxiliary pivots precede every protected target. Forward elimination
        // has already canceled them from every terminal-pivot row; their RREF
        // is irrelevant to terminal consequences and can be vastly denser.
        let mut terminal = Matrix::new(0, width, RationalPolynomialField::new(Z));
        let mut pivots = vec![None; width as usize];
        for row in 0..reducer.u().nrows() as usize {
            let entries: Vec<_> = native_row(reducer.u(), row).collect();
            if let Some(&(pivot, _)) = entries.first() {
                if pivot >= p.auxiliary_count as u32 && pivot < physical {
                    pivots[pivot as usize] = Some(terminal.nrows());
                    terminal.add_row(
                        entries.iter().map(|(_, v)| (*v).clone()).collect(),
                        entries.iter().map(|(i, _)| *i).collect(),
                    );
                }
            }
        }
        reducer = SparseRowReducer::from_upper_triangular_matrix(terminal, pivots);
    } else {
        check(
            "prospective back substitution nonzeros",
            (reducer.u().nrows() as usize).saturating_mul(width as usize),
            limits.max_reducer_nonzeros,
        )?;
    }
    reducer.back_substitute();
    let u = reducer.u();
    checked_matrix(u, context, limits)?;
    let mut weights = Matrix::new(0, sources.nrows(), field.clone());
    let mut expected = Matrix::new(0, physical, field);
    let mut equations = Vec::new();
    let mut replacements = BTreeMap::<VacuumIntegralKey, Row>::new();
    let conditions = Arc::new(p.conditions);
    for row in 0..u.nrows() as usize {
        let entries: Vec<_> = native_row(u, row).collect();
        let Some(&(pivot, value)) = entries.first() else {
            continue;
        };
        if pivot < p.auxiliary_count as u32 || pivot >= physical {
            continue;
        }
        if !value.is_one() {
            return Err(VacuumCollectionError::ReplayFailed);
        }
        let physical_entries: Vec<_> = entries
            .iter()
            .copied()
            .filter(|(i, _)| *i < physical)
            .collect();
        let provenance: Vec<_> = entries
            .iter()
            .copied()
            .filter(|(i, _)| *i >= physical)
            .collect();
        expected.add_row(
            physical_entries.iter().map(|(_, v)| (*v).clone()).collect(),
            physical_entries.iter().map(|(i, _)| *i).collect(),
        );
        weights.add_row(
            provenance.iter().map(|(_, v)| (*v).clone()).collect(),
            provenance.iter().map(|(i, _)| *i - physical).collect(),
        );
        let mut terms = Row::new();
        let mut rhs = Row::new();
        for &(column, value) in &physical_entries {
            let target = p
                .targets
                .get(&p.columns[column as usize])
                .ok_or(VacuumCollectionError::ReplayFailed)?;
            terms.insert(target.clone(), value.clone());
            if column != pivot {
                rhs.insert(
                    target.clone(),
                    context
                        .try_neg(value, limits.algebra.exact_algebra)
                        .map_err(algebra)?,
                );
            }
        }
        let target = p.targets[&p.columns[pivot as usize]].clone();
        replacements.insert(target, rhs);
        equations.push(VacuumCollectionEquation {
            terms,
            source_weights: provenance
                .iter()
                .map(|(i, v)| ((*i - physical) as usize, (*v).clone()))
                .collect(),
            conditions: Arc::clone(&conditions),
        });
    }
    let mut replay_operations = 0usize;
    for &row in weights.col_idcs() {
        replay_operations = replay_operations
            .checked_add(sources.row_ptrs()[row as usize + 1] - sources.row_ptrs()[row as usize])
            .ok_or(VacuumCollectionError::ArithmeticOverflow)?;
        check(
            "source replay operations",
            replay_operations,
            limits.max_replay_operations,
        )?;
    }
    check(
        "prospective source replay nonzeros",
        replay_operations,
        limits.max_reducer_nonzeros,
    )?;
    if !equations.is_empty() {
        let replay = &weights * &sources;
        checked_matrix(&replay, context, limits)?;
        let difference = &replay - &expected;
        checked_matrix(&difference, context, limits)?;
        if difference.values().iter().any(|v| !v.is_zero()) {
            return Err(VacuumCollectionError::ReplayFailed);
        }
    }
    let remaining: BTreeSet<_> = p
        .targets
        .values()
        .filter(|k| !replacements.contains_key(*k))
        .cloned()
        .collect();
    if replacements
        .values()
        .flat_map(BTreeMap::keys)
        .any(|k| !remaining.contains(k))
    {
        return Err(VacuumCollectionError::ReplayFailed);
    }
    let mut reductions: BTreeMap<String, BTreeMap<IntegralKey, GuardedVacuumReduction>> = p
        .families
        .keys()
        .cloned()
        .map(|f| (f, BTreeMap::new()))
        .collect();
    let mut flat_terms = 0usize;
    for key in &p.raw {
        let family = &p.families[key.family_fingerprint()];
        let canonical = p
            .aliases
            .representative(family, key.integral())
            .map_err(|e| VacuumCollectionError::Alias(e.into()))?;
        let target = &p.targets[canonical];
        let terms = replacements
            .get(target)
            .cloned()
            .unwrap_or_else(|| BTreeMap::from([(target.clone(), context.one())]));
        flat_terms = flat_terms.saturating_add(terms.len());
        check(
            "flat reduction terms",
            flat_terms,
            limits.max_flat_map_terms,
        )?;
        reductions
            .get_mut(key.family_fingerprint())
            .expect("declared family")
            .insert(
                key.integral().clone(),
                GuardedVacuumReduction {
                    terms,
                    conditions: Arc::clone(&conditions),
                },
            );
    }
    let statistics = VacuumCollectionStatistics {
        raw_terminals: p.raw.len(),
        global_target_classes: p.targets.len(),
        corner_seeds: p.sources.len(),
        native_source_rows: p.native_rows,
        columns: p.columns.len(),
        auxiliary_columns: p.auxiliary_count,
        terminal_equations: equations.len(),
        remaining_terminals: remaining.len(),
        replay_operations,
        reducer_nonzeros,
    };
    Ok(Eliminated {
        families: p.families,
        raw: p.raw,
        remaining,
        aliases: p.aliases,
        equations,
        reductions,
        conditions,
        statistics,
    })
}
