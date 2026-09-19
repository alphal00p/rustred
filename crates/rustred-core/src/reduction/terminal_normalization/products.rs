//! Proposal and exact verification of separable unit-mass products.

use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

use symbolica::domains::SelfRing;
use symbolica::poly::factor::Factorize;
use symbolica::prelude::{Integer, MultivariatePolynomial, PolyVariable, Z};

use crate::algebra::matrix::{
    SymbolicaCoefficientMatrixError, invert_and_verify_coefficient_matrix,
    multiply_coefficient_matrices,
};
use crate::algebra::{Coefficient, CoefficientContext};
use crate::family::{
    IntegralFamily, IntegralKey, ScalarProductCoordinate, symbolica_matrix_limits,
};
use crate::sector::symmetry::{self, CoefficientMatrix, DenominatorAction, Jacobian, MomentumMap};
use crate::sector::{ComplexityKey, OrderingPolicy};

use super::{
    ProductSkipReason as Skip, TerminalAliasError as Error, TerminalAliasPlan,
    TerminalAliasStatistics, VerifiedTerminalAlias,
};

type Quadratic = MultivariatePolynomial<symbolica::prelude::IntegerRing, u16>;

struct Product {
    key: IntegralKey,
    complexity: ComplexityKey,
    slots: Vec<usize>,
    rows: Vec<Vec<Coefficient>>,
    inverse: Vec<Vec<Coefficient>>,
}

impl TerminalAliasPlan {
    /// Recognize products of exactly L independent unit-mass tadpoles.
    ///
    /// Each inactive power must be zero and all analytic shifts must vanish.
    /// A native factorization proposes `D_i = q_i² - 1`; exact square replay,
    /// integer entries, determinant ±1 and generic same-family momentum-map
    /// verification are all mandatory. A signature is the sorted positive
    /// power multiset, and the least existing key in `ordering` represents it.
    pub fn independent_tadpole_products(
        family: &IntegralFamily,
        raw: &BTreeSet<IntegralKey>,
        ordering: OrderingPolicy,
    ) -> Result<Self, Error> {
        ordering
            .require_arity(family.denominator_count())
            .map_err(|error| Error::Ordering(error.to_string()))?;
        for key in raw {
            if key.powers().len() != family.denominator_count() {
                return Err(Error::WrongArity {
                    expected: family.denominator_count(),
                    actual: key.powers().len(),
                });
            }
        }
        let mut plan = Self {
            family_fingerprint: family.fingerprint_owner(),
            arity: family.denominator_count(),
            ordering,
            raw: raw.clone(),
            canonical: raw.clone(),
            aliases: BTreeMap::new(),
            statistics: TerminalAliasStatistics {
                raw_terminals: raw.len(),
                canonical_terminals: raw.len(),
                ..Default::default()
            },
        };
        let context = family.coefficient_context();
        for shift in family.power_shifts() {
            if !context.contains(shift) {
                return Err(Error::InvalidCoefficientContext);
            }
        }
        let family_skip = if family.external_count() != 0 {
            Some(Skip::ExternalMomenta)
        } else if family.power_shifts().iter().any(|shift| !shift.is_zero()) {
            Some(Skip::AnalyticPowerShifts)
        } else {
            None
        };
        if let Some(reason) = family_skip {
            plan.statistics.skipped.insert(reason, raw.len());
            return Ok(plan);
        }

        // Temporary native polynomial variables never escape this preparation.
        // Each denominator is factored at most once, independent of key count.
        let variables = Arc::new(
            (0..family.loop_count())
                .map(PolyVariable::Temporary)
                .collect(),
        );
        let mut momenta: Vec<Option<Result<Vec<Coefficient>, Skip>>> =
            vec![None; family.denominator_count()];
        let mut groups: BTreeMap<Vec<i64>, Vec<Product>> = BTreeMap::new();
        for key in raw {
            let result = product(
                family,
                key,
                ordering,
                &variables,
                &mut momenta,
                &mut plan.statistics,
            )?;
            match result {
                Ok(product) => {
                    plan.statistics.eligible_products += 1;
                    let signature = product
                        .slots
                        .iter()
                        .map(|&slot| key.powers()[slot])
                        .collect();
                    groups.entry(signature).or_default().push(product);
                }
                Err(reason) => plan.statistics.skip(reason),
            }
        }
        for mut group in groups.into_values() {
            group.sort_by(|left, right| left.complexity.cmp(&right.complexity));
            let representative = &group[0];
            for source in &group[1..] {
                match alias(family, source, representative)? {
                    Some(witness) => {
                        plan.canonical.remove(&source.key);
                        plan.aliases.insert(
                            source.key.clone(),
                            VerifiedTerminalAlias {
                                representative: representative.key.clone(),
                                witness: super::TerminalAliasWitness::Momentum(Arc::new(witness)),
                            },
                        );
                    }
                    None => plan.statistics.skip(Skip::ConditionalMomentumMap),
                }
            }
        }
        plan.statistics.verified_aliases = plan.aliases.len();
        plan.statistics.canonical_terminals = plan.canonical.len();
        Ok(plan)
    }
}

fn product(
    family: &IntegralFamily,
    key: &IntegralKey,
    ordering: OrderingPolicy,
    variables: &Arc<Vec<PolyVariable>>,
    momenta: &mut [Option<Result<Vec<Coefficient>, Skip>>],
    statistics: &mut TerminalAliasStatistics,
) -> Result<Result<Product, Skip>, Error> {
    if key.powers().iter().any(|&power| power < 0) {
        return Ok(Err(Skip::NumeratorPowers));
    }
    let mut slots: Vec<_> = key
        .powers()
        .iter()
        .enumerate()
        .filter_map(|(slot, &power)| (power > 0).then_some(slot))
        .collect();
    if slots.len() != family.loop_count() {
        return Ok(Err(Skip::ActiveLineCount));
    }
    slots.sort_by_key(|&slot| (key.powers()[slot], slot));
    let mut rows = Vec::with_capacity(slots.len());
    for &slot in &slots {
        if momenta[slot].is_none() {
            statistics.analyzed_denominators += 1;
            momenta[slot] = Some(momentum(family, slot, variables)?);
        }
        match momenta[slot]
            .as_ref()
            .expect("denominator proposal initialized")
        {
            Ok(row) => rows.push(row.clone()),
            Err(reason) => return Ok(Err(*reason)),
        }
    }
    let context = family.coefficient_context();
    let inverse = match invert_and_verify_coefficient_matrix(
        context,
        &rows,
        symbolica_matrix_limits(family.construction_limits()),
    ) {
        Ok(result) => result,
        Err(SymbolicaCoefficientMatrixError::Singular) => {
            return Ok(Err(Skip::SingularMomentumBasis));
        }
        Err(error) => return Err(Error::ExactAlgebra(error.to_string())),
    };
    let (inverse, determinant, _) = inverse.into_parts();
    if determinant != context.one() && determinant != context.integer(-1) {
        return Ok(Err(Skip::NonUnimodularMomentumBasis));
    }
    Ok(Ok(Product {
        key: key.clone(),
        complexity: ordering
            .complexity_key(key.powers())
            .map_err(|error| Error::Ordering(error.to_string()))?,
        slots,
        rows,
        inverse,
    }))
}

pub(super) fn integer(
    value: &Coefficient,
    context: &CoefficientContext,
) -> Result<Option<Integer>, Error> {
    if !context.contains(value) {
        return Err(Error::InvalidCoefficientContext);
    }
    Ok(
        (value.denominator.is_one() && value.numerator.is_constant())
            .then(|| value.numerator.get_constant()),
    )
}

pub(super) fn momentum(
    family: &IntegralFamily,
    slot: usize,
    variables: &Arc<Vec<PolyVariable>>,
) -> Result<Result<Vec<Coefficient>, Skip>, Error> {
    let context = family.coefficient_context();
    let denominator = &family.denominators()[slot];
    if integer(denominator.constant(), context)? != Some(Integer::from(-1)) {
        return Ok(Err(Skip::NonUnitMass));
    }
    let mut quadratic = Quadratic::new(&Z, None, variables.clone());
    for (coordinate, value) in family.coordinates().iter().zip(denominator.coefficients()) {
        let Some(value) = integer(value, context)? else {
            return Ok(Err(Skip::NonIntegerQuadratic));
        };
        let ScalarProductCoordinate::LoopLoop { left, right } = *coordinate else {
            return Ok(Err(Skip::ExternalMomenta));
        };
        let mut powers = vec![0_u16; family.loop_count()];
        powers[left] += 1;
        powers[right] += 1;
        quadratic.append_monomial(value, &powers);
    }
    if quadratic.is_zero() {
        return Ok(Err(Skip::NotLinearSquare));
    }
    let mut scalar = Integer::from(1);
    let mut linear = None;
    for (factor, multiplicity) in quadratic.factor() {
        if factor.is_constant() {
            scalar *= factor.get_constant().pow(multiplicity as u64);
        } else if multiplicity == 2
            && linear.is_none()
            && (0..factor.nterms())
                .all(|term| factor.exponents(term).iter().copied().sum::<u16>() == 1)
        {
            linear = Some(factor);
        } else {
            return Ok(Err(Skip::NotLinearSquare));
        }
    }
    let Some(linear) = linear else {
        return Ok(Err(Skip::NotLinearSquare));
    };
    if scalar <= Integer::from(0) {
        return Ok(Err(Skip::NotLinearSquare));
    }
    // Symbolica owns integer root extraction; the proposal is independently
    // checked by exact polynomial multiplication, not trusted by shape alone.
    let root = scalar.root(2);
    let proposed = &linear * &linear.constant(root);
    if &proposed * &proposed != quadratic {
        return Ok(Err(Skip::NotLinearSquare));
    }
    let mut row = vec![context.zero(); family.loop_count()];
    for term in 0..proposed.nterms() {
        let slot = proposed
            .exponents(term)
            .iter()
            .position(|&power| power == 1)
            .ok_or(Error::InvalidProductWitness)?;
        row[slot] = context
            .zero()
            .numerator
            .constant(proposed.coefficients[term].clone())
            .into();
    }
    Ok(Ok(row))
}

fn alias(
    family: &IntegralFamily,
    source: &Product,
    representative: &Product,
) -> Result<Option<symmetry::VerifiedMap>, Error> {
    let context = family.coefficient_context();
    // q_source = U_source k_source = U_rep k_rep.
    let (matrix, _) = multiply_coefficient_matrices(
        context,
        &source.inverse,
        &representative.rows,
        symbolica_matrix_limits(family.construction_limits()),
    )
    .map_err(|error| Error::ExactAlgebra(error.to_string()))?;
    for entry in matrix.iter().flatten() {
        if integer(entry, context)?.is_none() {
            return Err(Error::InvalidProductWitness);
        }
    }
    let loops = family.loop_count();
    let checked = |rows, columns, entries| {
        CoefficientMatrix::try_new(rows, columns, entries)
            .map_err(|error| Error::MomentumVerification(error.to_string()))
    };
    let map = MomentumMap::new(
        checked(
            loops,
            loops,
            matrix.into_iter().flatten().collect::<Vec<_>>(),
        )?,
        checked(loops, 0, vec![])?,
        checked(0, 0, vec![])?,
    );
    let verified = symmetry::verify(family, family, map, symmetry::Limits::default())
        .map_err(|error| Error::MomentumVerification(error.to_string()))?;
    if !matches!(verified.jacobian(), Jacobian::Unit { .. }) {
        return Err(Error::InvalidProductWitness);
    }
    for (&source_slot, &target_slot) in source.slots.iter().zip(&representative.slots) {
        let valid = matches!(&verified.row_actions()[source_slot],
            DenominatorAction::Monomial { target, scale }
            if *target == target_slot && context.contains(scale) && scale.is_one());
        if !valid || source.key.powers()[source_slot] != representative.key.powers()[target_slot] {
            return Err(Error::InvalidProductWitness);
        }
    }
    // Conservatively retain conditional cases rather than claiming a new
    // unconditional equality. No sampled parameter value discharges a guard.
    if verified
        .nonzero_conditions()
        .iter()
        .any(|condition| !condition.polynomial().is_constant())
    {
        return Ok(None);
    }
    Ok(Some(verified))
}
