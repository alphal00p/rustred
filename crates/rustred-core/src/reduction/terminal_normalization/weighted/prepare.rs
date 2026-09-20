use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

use symbolica::prelude::PolyVariable;

use crate::algebra::Coefficient;
use crate::family::{IntegralFamily, IntegralKey};
use crate::sector::{
    Mask, OrderingPolicy,
    zero::{Analyzer, Decision},
};

use super::super::{
    ProductSkipReason, TerminalAliasPlan, TerminalAliasStatistics, corank_one::proposal,
};
use super::{
    TerminalNormalizationError as Error, TerminalNormalizationLimits, TerminalNormalizationPlan,
    TerminalNormalizationSkipReason as Skip, TerminalNormalizationStatistics, algebra, check,
    projection,
};

pub(super) fn prepare(
    family: &IntegralFamily,
    raw: &BTreeSet<IntegralKey>,
    ordering: OrderingPolicy,
    limits: TerminalNormalizationLimits,
) -> Result<TerminalNormalizationPlan, Error> {
    check("raw terminals", raw.len(), limits.max_terminals)?;
    let aliases = TerminalAliasPlan::vacuum_parametric_equivalences(
        family,
        raw,
        ordering,
        limits.parametric,
    )?;
    let context = family.coefficient_context();
    let mut statistics = TerminalNormalizationStatistics {
        raw_terminals: raw.len(),
        unit_aliases: aliases.aliases().len(),
        ..Default::default()
    };
    let mut terms = BTreeMap::new();
    for key in raw {
        terms.insert(
            key.clone(),
            BTreeMap::from([(aliases.representative(family, key)?.clone(), context.one())]),
        );
    }
    let mut witnesses = BTreeMap::new();
    let family_skip = if family.external_count() != 0 {
        Some(ProductSkipReason::ExternalMomenta)
    } else if family.power_shifts().iter().any(|c| !c.is_zero()) {
        Some(ProductSkipReason::AnalyticPowerShifts)
    } else {
        None
    };
    if let Some(reason) = family_skip {
        statistics
            .skipped
            .insert(Skip::UnsupportedGeometry(reason), raw.len());
    } else {
        let active_count = family
            .loop_count()
            .checked_add(1)
            .ok_or(Error::InvalidWitness("active line count overflow"))?;
        let variables = Arc::new(
            (0..family.loop_count())
                .map(PolyVariable::Temporary)
                .collect(),
        );
        let mut momenta = vec![None; family.denominator_count()];
        let mut support_statistics = TerminalAliasStatistics::default();
        let mut supports = BTreeMap::new();
        for key in aliases.canonical_terminals() {
            if !key.powers().iter().any(|&n| n < 0) {
                continue;
            }
            if key.powers().iter().filter(|&&n| n == 1).count() != active_count
                || key.powers().iter().filter(|&&n| n == -1).count() != 1
                || key.powers().iter().any(|&n| !(-1..=1).contains(&n))
            {
                *statistics.skipped.entry(Skip::NumeratorShape).or_default() += 1;
                continue;
            }
            let slots: Vec<_> = key
                .powers()
                .iter()
                .enumerate()
                .filter_map(|(i, &n)| (n > 0).then_some(i))
                .collect();
            if !supports.contains_key(&slots) {
                check(
                    "support count",
                    supports.len().saturating_add(1),
                    limits.max_supports,
                )?;
                let cells = active_count
                    .checked_mul(family.loop_count())
                    .ok_or(Error::InvalidWitness("momentum matrix size overflow"))?;
                check("momentum matrix cells", cells, limits.max_matrix_cells)?;
                let prepared = match proposal::support(
                    family,
                    &slots,
                    &variables,
                    &mut momenta,
                    &mut support_statistics,
                )? {
                    Ok(support) => projection::prepare(family, &support, limits)?,
                    Err(skip) => Err(Skip::UnsupportedGeometry(skip)),
                };
                if let Ok(support) = &prepared {
                    statistics.verified_generators += support.generators.len();
                }
                statistics.analyzed_supports += 1;
                supports.insert(slots.clone(), prepared);
            }
            match &supports[&slots] {
                Ok(support) => match projection::project(family, key, support.clone())? {
                    Some(witness) => {
                        witnesses.insert(key.clone(), witness);
                    }
                    None => {
                        *statistics
                            .skipped
                            .entry(Skip::IncompleteSymmetrySpan)
                            .or_default() += 1;
                    }
                },
                Err(skip) => {
                    *statistics.skipped.entry(*skip).or_default() += 1;
                }
            }
        }
    }
    let mut union = raw.clone();
    let mut raw_output_count = 0usize;
    for witness in witnesses.values() {
        raw_output_count = raw_output_count
            .checked_add(witness.unbound_terms.len())
            .ok_or(Error::InvalidWitness("projected output count overflow"))?;
        check(
            "projected output terms",
            raw_output_count,
            limits.max_output_terms,
        )?;
        union.extend(witness.unbound_terms.keys().cloned());
        check("output union keys", union.len(), limits.max_terminals)?;
    }
    let positive_aliases = if witnesses.is_empty() {
        aliases.clone()
    } else {
        TerminalAliasPlan::vacuum_parametric_equivalences(
            family,
            &union,
            ordering,
            limits.parametric,
        )?
    };
    let mut ordered_existing = aliases
        .canonical_terminals()
        .iter()
        .map(|key| {
            Ok((
                ordering.complexity_key(key.powers()).map_err(algebra)?,
                key.clone(),
            ))
        })
        .collect::<Result<Vec<_>, Error>>()?;
    ordered_existing.sort();
    let mut positive_bindings = BTreeMap::new();
    for (_, key) in ordered_existing {
        positive_bindings
            .entry(positive_aliases.representative(family, &key)?.clone())
            .or_insert(key);
    }
    let zero = if witnesses.is_empty() {
        None
    } else {
        Some(Analyzer::try_unrestricted(family).map_err(algebra)?)
    };
    let mut zero_certificates = BTreeMap::new();
    let mut accepted = BTreeMap::new();
    for (key, witness) in witnesses {
        let mut row: BTreeMap<IntegralKey, Coefficient> = BTreeMap::new();
        let mut missing = false;
        for (output, c) in &witness.unbound_terms {
            if let Decision::ProvedZero(certificate) = zero
                .as_ref()
                .expect("prepared for projections")
                .analyze(&Mask::try_from_indices(output.powers()).map_err(algebra)?)
                .map_err(algebra)?
            {
                if certificate
                    .domain()
                    .conditions()
                    .iter()
                    .all(|c| c.polynomial().is_constant())
                {
                    zero_certificates.insert(output.clone(), Arc::new(certificate));
                    continue;
                }
            }
            let class = positive_aliases.representative(family, output)?;
            let Some(bound) = positive_bindings.get(class) else {
                missing = true;
                break;
            };
            if bound.powers().iter().any(|&n| n < 0) {
                return Err(Error::InvalidWitness(
                    "positive output bound to a numerator",
                ));
            }
            let value = if let Some(old) = row.get(bound) {
                context
                    .try_add(old, c, Default::default())
                    .map_err(algebra)?
            } else {
                c.clone()
            };
            if value.is_zero() {
                row.remove(bound);
            } else {
                row.insert(bound.clone(), value);
            }
        }
        if missing {
            *statistics
                .skipped
                .entry(Skip::UnboundPositiveOutput)
                .or_default() += 1;
            continue;
        }
        // Flatten onto old declared normal forms; none of these positive keys
        // can itself be a quadratic-numerator target.
        let mut descending = true;
        for output in row.keys() {
            if !aliases.canonical_terminals().contains(output) {
                return Err(Error::InvalidWitness(
                    "output was not already declared canonical",
                ));
            }
            if !ordering
                .compare(output.powers(), key.powers())
                .map_err(algebra)?
                .is_lt()
            {
                descending = false;
            }
        }
        // A valid positive equivalence may bind to an already declared key in
        // a harder sector. This is unsupported by this descending convention,
        // not a failure of the exact symmetry identity.
        if !descending {
            *statistics
                .skipped
                .entry(Skip::NonDescendingOutput)
                .or_default() += 1;
            continue;
        }
        terms.insert(key.clone(), row);
        accepted.insert(key, witness);
        statistics.projected_numerators += 1;
    }
    let mut canonical = BTreeSet::new();
    let mut output_count = 0usize;
    for row in terms.values() {
        output_count = output_count
            .checked_add(row.len())
            .ok_or(Error::InvalidWitness("normalized term count overflow"))?;
        check(
            "normalized output terms",
            output_count,
            limits.max_output_terms,
        )?;
        for (key, c) in row {
            context
                .validate_with_limits(c, Default::default())
                .map_err(algebra)?;
            if c.is_zero() {
                return Err(Error::InvalidWitness("explicit zero coefficient"));
            }
            canonical.insert(key.clone());
        }
    }
    for key in &canonical {
        let row = terms
            .get(key)
            .ok_or(Error::InvalidWitness("output is not a raw terminal"))?;
        if row.len() != 1 || row.get(key) != Some(&context.one()) {
            return Err(Error::InvalidWitness(
                "normalization is not a one-hop fixed point",
            ));
        }
    }
    statistics.canonical_terminals = canonical.len();
    Ok(TerminalNormalizationPlan {
        family: family.fingerprint_owner(),
        ordering,
        raw: raw.clone(),
        canonical,
        terms,
        witnesses: accepted,
        positive_aliases,
        positive_bindings,
        zero_certificates,
        statistics,
    })
}
