//! Diagnostic comparison with a text rule export produced by SpIRed.
//!
//! This example helper is an oracle for an already-generated Rust candidate.
//! Reference rules never supply equations, seeds, or hints to the solver.
//! `compare` checks required case domains and exact RHS equations only.
//! `compare_rule` additionally checks Positive/NonPositive annotations and exact
//! applicability guards, including exact affine equality cases.
//! `compare_sector_with_aliases` also checks the entire nonempty reference set.
//! `compare_pre_rules` matches unrestricted linear-cut rules by their excluded
//! coordinate axis and checks the entire pre-rule set, not just matching RHSs.

use std::collections::BTreeMap;
use std::error::Error;
use std::io::{Error as IoError, ErrorKind};

use rustred::algebra::Coefficient;
use rustred::solver::{Case, Integral, LinearCutRule, Power, RuleCandidate, SourceSystem};
use symbolica::domains::rational_polynomial::FromNumeratorAndDenominator;
use symbolica::parser::{ParseSettings, Token};
use symbolica::prelude::{Integer, IntegerRing, PolyVariable, Q, Z};

type Result<T> = std::result::Result<T, Box<dyn Error>>;

#[path = "spired_reference/domains.rs"]
mod domains;
pub use domains::{SectorComparison, compare_sector_with_aliases};

fn invalid(message: impl Into<String>) -> Box<dyn Error> {
    Box::new(IoError::new(ErrorKind::InvalidData, message.into()))
}

/// Explicit oracle notation for one already-declared symbolic parameter.
///
/// For example, SpIRed prints the independent invariant p² as `dot[p,p]`,
/// while a Rust family may call it `s`. The reference must be a whole function
/// call with identifier arguments; the destination must be a source scalar,
/// never a numerical expression or integral index. No kinematics is evaluated.
#[derive(Clone, Copy, Debug)]
pub struct CoefficientAlias<'a> {
    pub reference: &'a str,
    pub parameter: &'a str,
}

/// Compare one already-generated candidate against its unique required case
/// in a SpIRed `.dat` rule list. This does not compare applicability guards.
///
/// `n1`, ..., `nN` in the reference map directly to the source system's native
/// index variables. Other reference identifiers use the source's scalar symbol
/// names. Fixed coordinates are specialized before exact coefficient comparison.
///
/// If `reference_mass_one` is `Some(name)`, that named *reference* parameter is
/// explicitly specialized to one. The candidate is left as supplied, so its
/// family must already represent the same mass specialization. `None` performs
/// no mass substitution. Unknown or ambiguous parameter names are errors.
pub fn compare<const N: usize>(
    reference: &str,
    candidate: &RuleCandidate<N>,
    system: &SourceSystem<N>,
    reference_mass_one: Option<&str>,
) -> Result<()> {
    compare_with_aliases(reference, candidate, system, reference_mass_one, &[])
}

/// As [`compare`], with explicit reference-function-to-scalar notation aliases.
/// Aliases apply only to parsed RHS coefficient tokens, not integral patterns,
/// guards, candidate coefficients, or family data.
pub fn compare_with_aliases<const N: usize>(
    reference: &str,
    candidate: &RuleCandidate<N>,
    system: &SourceSystem<N>,
    reference_mass_one: Option<&str>,
    aliases: &[CoefficientAlias<'_>],
) -> Result<()> {
    if candidate.target != candidate.case.integral() {
        return Err(invalid("candidate target is not its canonical case"));
    }
    let entry = domains::matching_entry(reference, &candidate.case, system, None)?;
    compare_rhs(
        entry.rhs,
        &parse_lhs(entry.lhs)?,
        candidate,
        system,
        reference_mass_one,
        aliases,
    )
}

fn compare_rhs<const N: usize>(
    rhs: &str,
    reference_fixed: &[Option<i16>; N],
    candidate: &RuleCandidate<N>,
    system: &SourceSystem<N>,
    reference_mass_one: Option<&str>,
    aliases: &[CoefficientAlias<'_>],
) -> Result<()> {
    let template = &system
        .rows()
        .iter()
        .flatten()
        .next()
        .ok_or_else(|| invalid("the source system has no coefficient variable map"))?
        .coefficient;
    let native_variables = template.variables();
    let mut variable_names = Vec::with_capacity(native_variables.len() + 1);
    for (position, variable) in native_variables.iter().enumerate() {
        let name = if let Some(coordinate) = system
            .index_variables()
            .iter()
            .position(|&index| index == position)
        {
            format!("n{}", coordinate + 1)
        } else {
            match variable {
                PolyVariable::Symbol(symbol) => symbol.get_stripped_name().to_owned(),
                _ => {
                    return Err(invalid(
                        "reference comparison requires named scalar variables",
                    ));
                }
            }
        };
        if variable_names.contains(&name) {
            return Err(invalid(format!("ambiguous reference variable name {name}")));
        }
        variable_names.push(name);
    }
    let aliases = compile_aliases(aliases, &variable_names, system.index_variables())?;
    let mut variables = native_variables.as_ref().clone();
    let mass_position = if let Some(name) = reference_mass_one {
        if !is_identifier(name) {
            return Err(invalid(
                "the mass substitution must name one scalar identifier",
            ));
        }
        if (1..=N).any(|coordinate| name == format!("n{coordinate}")) {
            return Err(invalid(
                "a reference index cannot be substituted as a mass parameter",
            ));
        }
        Some(
            if let Some(position) = variable_names.iter().position(|value| value == name) {
                position
            } else {
                // A local polynomial variable lets native substitution detect a
                // zero denominator at mass=1 before comparison. It registers no
                // global symbol and has zero degree after this specialization.
                let temporary = (0..=variables.len())
                    .find(|index| !variables.contains(&PolyVariable::Temporary(*index)))
                    .expect("a finite variable map always has a fresh temporary index");
                variables.push(PolyVariable::Temporary(temporary));
                variable_names.push(name.to_owned());
                variables.len() - 1
            },
        )
    } else {
        None
    };
    let variables = std::sync::Arc::new(variables);
    let names = variable_names
        .into_iter()
        .map(Into::into)
        .collect::<Vec<_>>();

    let mut expected = BTreeMap::new();
    if rhs.trim() != "0" {
        for term in split_top_level(rhs, "+")? {
            let factors = split_top_level(term, "*")?;
            if factors.len() != 2 {
                return Err(invalid("expected each RHS term as (coefficient)*int[...]"));
            }
            let integral = parse_integral::<N>(factors[1])?;
            require_coordinate_pattern(&integral, reference_fixed)?;
            let integral = specialize_reference_integral(integral, candidate.case.fixed())?;
            let mut token = Token::parse(factors[0], ParseSettings::polynomial())
                .map_err(|error| invalid(format!("invalid reference coefficient: {error}")))?;
            if !aliases.is_empty() {
                alias_coefficient_tokens(&mut token, &aliases);
            }
            let coefficient: Coefficient = token
                .to_rational_polynomial(&Q, &Z, &variables, &names)
                .map_err(|error| invalid(format!("invalid reference coefficient: {error}")))?;
            let mut numerator = coefficient.numerator;
            let mut denominator = coefficient.denominator;
            if let Some(variable) = mass_position {
                numerator = numerator.replace(variable, &Integer::from(1));
                denominator = denominator.replace(variable, &Integer::from(1));
            }
            if denominator.is_zero() {
                return Err(invalid(format!(
                    "reference coefficient is undefined after the requested specialization at {integral:?}"
                )));
            }
            let coefficient = <Coefficient as FromNumeratorAndDenominator<
                IntegerRing,
                IntegerRing,
                u16,
            >>::from_num_den(numerator, denominator, &Z, true);
            let coefficient = restrict_coefficient(coefficient, &candidate.case, system)?;
            add_term(&mut expected, integral, coefficient);
        }
    }

    let mut actual = BTreeMap::new();
    for term in &candidate.rhs {
        require_coordinate_pattern(&term.integral, candidate.case.fixed())?;
        if term.coefficient.numerator.variables() != native_variables
            || term.coefficient.denominator.variables() != native_variables
        {
            return Err(invalid(
                "candidate coefficients do not use the supplied source variable map",
            ));
        }
        add_term(
            &mut actual,
            term.integral,
            restrict_coefficient(term.coefficient.clone(), &candidate.case, system)?,
        );
    }
    expected.retain(|_, coefficient| !coefficient.is_zero());
    actual.retain(|_, coefficient| !coefficient.is_zero());
    for (integral, expected_coefficient) in &expected {
        let actual_coefficient = actual.get(integral).ok_or_else(|| {
            invalid(format!(
                "candidate is missing reference RHS integral {integral:?}"
            ))
        })?;
        // Native subtraction unifies the optional, now inactive mass variable
        // and compares rational functions independently of their printed form.
        if !(expected_coefficient - actual_coefficient).is_zero() {
            return Err(invalid(format!(
                "different exact coefficient for {integral:?}: reference {expected_coefficient}; candidate {actual_coefficient}"
            )));
        }
    }
    for integral in actual.keys() {
        if !expected.contains_key(integral) {
            return Err(invalid(format!(
                "candidate has additional RHS integral {integral:?}"
            )));
        }
    }
    Ok(())
}

fn specialize_reference_integral<const N: usize>(
    integral: Integral<N>,
    fixed: &[Option<i16>; N],
) -> Result<Integral<N>> {
    let mut powers = *integral.powers();
    for (axis, value) in fixed.iter().enumerate() {
        if let Some(value) = value {
            if powers[axis].is_symbolic() {
                powers[axis] = Power::new(
                    false,
                    value
                        .checked_add(powers[axis].value())
                        .ok_or_else(|| invalid("reference integral specialization overflow"))?,
                )?;
            }
        }
    }
    Ok(Integral::new(powers))
}

fn restrict_coefficient<const N: usize>(
    coefficient: Coefficient,
    case: &Case<N>,
    system: &SourceSystem<N>,
) -> Result<Coefficient> {
    let variables = system
        .rows()
        .iter()
        .flatten()
        .next()
        .ok_or_else(|| invalid("the source system has no coefficient variable map"))?
        .coefficient
        .variables();
    // Native remapping removes an optional reference-only mass variable after
    // specialization; it rejects any still-active variable absent from Rust.
    let mut numerator = coefficient
        .numerator
        .rearrange_with_growth(variables)
        .map_err(invalid)?;
    let mut denominator = coefficient
        .denominator
        .rearrange_with_growth(variables)
        .map_err(invalid)?;
    if let Some(affine) = case.affine() {
        // Restrict the exact quotient jointly: independently making its
        // numerator and denominator primitive would lose rational chart scales.
        let remapped = Coefficient {
            numerator,
            denominator,
        };
        return Ok(affine.restrict_coefficient(&remapped)?);
    } else {
        for (axis, value) in case.fixed().iter().enumerate() {
            if let Some(value) = value {
                numerator =
                    numerator.replace(system.index_variables()[axis], &Integer::from(*value));
                denominator =
                    denominator.replace(system.index_variables()[axis], &Integer::from(*value));
            }
        }
    }
    if denominator.is_zero() {
        return Err(invalid("coefficient is undefined on its required case"));
    }
    Ok(<Coefficient as FromNumeratorAndDenominator<
        IntegerRing,
        IntegerRing,
        u16,
    >>::from_num_den(numerator, denominator, &Z, true))
}

fn compile_aliases<const N: usize>(
    aliases: &[CoefficientAlias<'_>],
    variable_names: &[String],
    indices: &[usize; N],
) -> Result<Vec<(Token, Token)>> {
    let mut compiled = Vec::with_capacity(aliases.len());
    for alias in aliases {
        let parameter = variable_names
            .iter()
            .position(|name| name == alias.parameter)
            .filter(|position| !indices.contains(position))
            .ok_or_else(|| {
                invalid(format!(
                    "oracle alias destination {} must be a declared scalar parameter",
                    alias.parameter
                ))
            })?;
        let token = Token::parse(alias.reference, ParseSettings::polynomial())
            .map_err(|error| invalid(format!("invalid oracle alias: {error}")))?;
        if !matches!(&token, Token::Fn(_, _, arguments)
            if arguments.len() >= 2 && arguments.iter().all(|argument|
                matches!(argument, Token::ID(name) if is_identifier(name))))
        {
            return Err(invalid(
                "oracle alias must name a complete function call with identifier arguments",
            ));
        }
        if compiled
            .iter()
            .any(|(previous, _)| same_alias_call(previous, &token))
        {
            return Err(invalid("duplicate oracle coefficient alias"));
        }
        compiled.push((token, Token::ID(variable_names[parameter].as_str().into())));
    }
    Ok(compiled)
}

/// Not algebra: rename exact function-call tokens in Symbolica's parsed tree.
/// Native parsing handles brackets/whitespace and native polynomial conversion
/// still owns all arithmetic, cancellation, and normalization. Whole-token
/// equality prevents accidental replacement inside names or other functions.
/// Symbolica's function token retains its input delimiter, which is syntax,
/// not a distinction between `dot[p,p]` and `dot(p,p)`.
fn alias_coefficient_tokens(token: &mut Token, aliases: &[(Token, Token)]) {
    if let Some((_, replacement)) = aliases
        .iter()
        .find(|(reference, _)| same_alias_call(reference, token))
    {
        *token = replacement.clone();
        return;
    }
    if let Token::Op(_, _, _, arguments) | Token::Fn(_, _, arguments) = token {
        for argument in arguments {
            alias_coefficient_tokens(argument, aliases);
        }
    }
}

fn same_alias_call(reference: &Token, candidate: &Token) -> bool {
    matches!((reference, candidate),
        (Token::Fn(false, _, left), Token::Fn(false, _, right)) if left == right)
}

/// Compare all independently generated linear-cut pre-rules with an export.
///
/// Every target is the generic integral I(n), so rules are matched by their
/// exact excluded face `n_i == 1`, not by their identical target pattern.
/// The reference must use unrestricted index patterns and one such exclusion
/// per rule. Missing, extra, or duplicate axes fail before RHS comparison.
/// This is an oracle-only operation called after source/rule generation.
pub fn compare_pre_rules<const N: usize>(
    reference: &str,
    rules: &[LinearCutRule<N>],
    system: &SourceSystem<N>,
) -> Result<()> {
    use rustred::solver::{CoordinateCase, SearchStats};

    let body = reference
        .trim()
        .strip_prefix('{')
        .and_then(|text| text.strip_suffix('}'))
        .ok_or_else(|| invalid("expected a SpIRed pre-rule list enclosed in braces"))?;
    let mut expected = BTreeMap::new();
    for entry in split_top_level(body, ",")? {
        let parts = split_top_level(entry, "->")?;
        if parts.len() != 2 {
            return Err(invalid(
                "each reference pre-rule must contain exactly one top-level ->",
            ));
        }
        let axis = pre_rule_axis::<N>(parts[0])?;
        if expected.insert(axis, entry).is_some() {
            return Err(invalid(format!(
                "duplicate reference pre-rule for cut coordinate {}",
                axis + 1
            )));
        }
    }
    let mut actual = BTreeMap::new();
    for rule in rules {
        if rule.axis >= N {
            return Err(invalid("candidate pre-rule cut coordinate is out of range"));
        }
        if actual.insert(rule.axis, rule).is_some() {
            return Err(invalid(format!(
                "duplicate candidate pre-rule for cut coordinate {}",
                rule.axis + 1
            )));
        }
    }
    if actual.keys().ne(expected.keys()) {
        return Err(invalid(format!(
            "different pre-rule cut coordinates: reference {:?}; candidate {:?}",
            expected.keys().collect::<Vec<_>>(),
            actual.keys().collect::<Vec<_>>()
        )));
    }
    for (axis, rule) in actual {
        // This temporary candidate is only an adapter for the existing native
        // exact comparator. Prepared sources are fixed at cut power one, but
        // their pre-rules are NOT: leave every coordinate symbolic here.
        let candidate = RuleCandidate {
            case: CoordinateCase::generic().into(),
            target: rule.target,
            rhs: rule.rhs.clone(),
            sources: Vec::new(),
            stats: SearchStats::default(),
        };
        compare(
            &format!("{{{}}}", expected[&axis]),
            &candidate,
            system,
            None,
        )?;
    }
    Ok(())
}

fn pre_rule_axis<const N: usize>(lhs: &str) -> Result<usize> {
    let (arguments, suffix) = integral_arguments(lhs)?;
    let arguments = split_top_level(arguments, ",")?;
    if arguments.len() != N {
        return Err(invalid(format!(
            "expected {N} coordinates on the reference pre-rule LHS"
        )));
    }
    for (axis, argument) in arguments.iter().enumerate() {
        let compact: String = argument.chars().filter(|c| !c.is_whitespace()).collect();
        if compact != format!("n{}_", axis + 1) {
            return Err(invalid(
                "reference pre-rule requires unrestricted symbolic coordinates n_i_",
            ));
        }
    }
    let guard = suffix
        .trim()
        .strip_prefix("/;")
        .map(str::trim)
        .and_then(|text| text.strip_prefix('!'))
        .map(str::trim)
        .ok_or_else(|| invalid("reference pre-rule requires the guard /;!(n_i==1)"))?;
    let inner = strip_guard_parentheses(guard)?;
    if inner.len() == guard.len() {
        return Err(invalid(
            "reference pre-rule exclusion must be wholly parenthesized",
        ));
    }
    let branches = parse_coordinate_guard::<N>(inner)?;
    let [branch] = branches.as_slice() else {
        return Err(invalid(
            "reference pre-rule requires one excluded coordinate face",
        ));
    };
    let [(axis, value)] = branch.as_slice() else {
        return Err(invalid(
            "reference pre-rule exclusion must be exactly n_i==1",
        ));
    };
    if value != &Integer::from(1) {
        return Err(invalid(
            "reference pre-rule must exclude cut power one, not another power",
        ));
    }
    Ok(*axis)
}

/// Compare an independently generated equation AND its exact required domain
/// against one SpIRed export. Unlike [`compare`], this requires the exported
/// Positive/NonPositive annotations to agree and compares excluded faces.
///
/// Required conditions are polynomial-equality conjunctions. Excluded conditions
/// are negated AND/OR expressions of polynomial equalities. Symbolica and shared
/// RustRed geometry own exact normalization, sector pruning and subsumption;
/// unsupported geometry still fails explicitly.
pub fn compare_rule<const N: usize>(
    reference: &str,
    rule: &rustred::solver::SectorRule<N>,
    system: &SourceSystem<N>,
    sector: &[bool; N],
    reference_mass_one: Option<&str>,
) -> Result<()> {
    compare_rule_with_aliases(reference, rule, system, sector, reference_mass_one, &[])
}

/// As [`compare_rule`], with explicit symbolic coefficient notation aliases.
/// Coordinate guards retain their existing exact parser and are not rewritten.
pub fn compare_rule_with_aliases<const N: usize>(
    reference: &str,
    rule: &rustred::solver::SectorRule<N>,
    system: &SourceSystem<N>,
    sector: &[bool; N],
    reference_mass_one: Option<&str>,
    aliases: &[CoefficientAlias<'_>],
) -> Result<()> {
    if !rule.candidate.case.is_in_sector(sector) {
        return Err(invalid("candidate case is outside the comparison sector"));
    }
    if rule.candidate.target != rule.candidate.case.integral() {
        return Err(invalid("candidate target is not its canonical case"));
    }
    let entry = domains::matching_entry(reference, &rule.candidate.case, system, Some(sector))?;
    compare_rhs(
        entry.rhs,
        &parse_lhs(entry.lhs)?,
        &rule.candidate,
        system,
        reference_mass_one,
        aliases,
    )?;
    domains::compare_exceptions(&entry, rule, system, sector)
}

/// Structural Boolean parsing only; integer and polynomial operations remain
/// native. The returned outer vector is OR and each inner vector is AND.
fn parse_coordinate_guard<const N: usize>(text: &str) -> Result<Vec<Vec<(usize, Integer)>>> {
    let text = strip_guard_parentheses(text.trim())?;
    let disjunction = split_top_level(text, "||")?;
    if disjunction.len() > 1 {
        let mut branches = Vec::new();
        for branch in disjunction {
            branches.extend(parse_coordinate_guard::<N>(branch)?);
        }
        return Ok(branches);
    }
    let conjunction = split_top_level(text, "&&")?;
    if conjunction.len() > 1 {
        let mut branches = vec![Vec::new()];
        for factor in conjunction {
            let alternatives = parse_coordinate_guard::<N>(factor)?;
            let mut product = Vec::new();
            for left in &branches {
                for right in &alternatives {
                    let mut combined = left.clone();
                    combined.extend(right.iter().cloned());
                    product.push(combined);
                }
            }
            branches = product;
        }
        return Ok(branches);
    }
    if matches!(text, "True" | "true") {
        return Ok(vec![Vec::new()]);
    }
    if matches!(text, "False" | "false") {
        return Ok(Vec::new());
    }
    let atom = split_top_level(text, "==")?;
    if atom.len() != 2 {
        return Err(invalid(
            "unsupported reference guard atom: expected n_i == integer",
        ));
    }
    let index = atom[0]
        .trim()
        .strip_prefix('n')
        .and_then(|index| index.parse::<usize>().ok())
        .filter(|index| (1..=N).contains(index))
        .ok_or_else(|| invalid("unsupported reference guard: left side must be one index n_i"))?;
    if atom[0].trim() != format!("n{index}") {
        return Err(invalid(
            "reference guard must use the canonical coordinate name n_i",
        ));
    }
    let literal = atom[1].trim();
    let digits = literal.strip_prefix(['-', '+']).unwrap_or(literal);
    if digits.is_empty() || !digits.bytes().all(|byte| byte.is_ascii_digit()) {
        return Err(invalid(
            "unsupported reference guard: right side must be an integer literal",
        ));
    }
    let value = literal
        .parse::<Integer>()
        .map_err(|_| invalid("unsupported reference guard: right side must be an integer"))?;
    Ok(vec![vec![(index - 1, value)]])
}

#[cfg(test)]
mod guard_tests {
    use super::*;
    use rustred::algebra::CoefficientContext;
    use rustred::solver::{CoordinateCase, ExceptionalConditions, SearchStats, SectorRule, Term};

    fn fixture() -> (CoefficientContext, SourceSystem<2>, SectorRule<2>) {
        let context = CoefficientContext::try_new(["d", "a", "b"]).unwrap();
        let source = SourceSystem::new(
            vec![vec![Term {
                integral: Integral::symbolic([0; 2]).unwrap(),
                coefficient: context.one().numerator,
            }]],
            [1, 2],
        )
        .unwrap();
        let rule = SectorRule {
            candidate: RuleCandidate {
                case: CoordinateCase::generic().into(),
                target: Integral::symbolic([0; 2]).unwrap(),
                rhs: vec![Term {
                    integral: Integral::symbolic([-1, 0]).unwrap(),
                    coefficient: context.one(),
                }],
                sources: Vec::new(),
                stats: SearchStats::default(),
            },
            exceptions: ExceptionalConditions {
                branches: vec![vec![
                    (&context.parameter("a").unwrap() - &context.one()).numerator,
                ]],
            },
        };
        (context, source, rule)
    }

    fn generic_reference(guard: &str) -> String {
        format!("{{int[n1_?Positive,n2_?Positive]{guard}->(1)*int[-1+n1,0+n2]}}")
    }

    #[test]
    fn affine_oracle_preserves_numerator_denominator_relative_scale() {
        let (context, source, _) = fixture();
        let a = context.parameter("a").unwrap();
        let b = context.parameter("b").unwrap();
        let constraint = &(&(&context.integer(2) * &a) - &b) - &context.integer(4);
        let equation = constraint.numerator.clone();
        let case = Case::generic()
            .intersect(&[equation], &[1, 2], &[true; 2])
            .unwrap()
            .unwrap();
        let restricted_a = &(&b + &context.integer(4)) / &context.integer(2);
        assert_eq!(
            restrict_coefficient(a.clone(), &case, &source).unwrap(),
            restricted_a
        );
        assert_eq!(
            restrict_coefficient(&a / &(&a + &context.one()), &case, &source).unwrap(),
            &(&b + &context.integer(4)) / &(&b + &context.integer(6))
        );
        assert!(restrict_coefficient(&context.one() / &constraint, &case, &source,).is_err());
    }

    #[test]
    fn exact_guard_comparison_rejects_missing_wrong_and_extra_faces() {
        let (_, source, rule) = fixture();
        compare_rule(
            &generic_reference("/;!((n1==1))"),
            &rule,
            &source,
            &[true; 2],
            None,
        )
        .unwrap();
        for guard in ["", "/;!(n1==2)", "/;!((n1==1)||(n2==1))", "/;!(False)"] {
            assert!(
                compare_rule(&generic_reference(guard), &rule, &source, &[true; 2], None).is_err(),
                "{guard}"
            );
        }
    }

    #[test]
    fn guard_or_normalization_prunes_duplicates_subfaces_and_impossible_faces() {
        let (_, source, rule) = fixture();
        for guard in [
            "/;!((n1==1)||(n1==1)||(n1==1&&n2==2)||(n1==0))",
            "/;!((n1==1)||((n1==2)&&(n1==3)))",
            "/;!(((n1==1)||(n1==0))&&((n2==2)||(n1==1)))",
        ] {
            compare_rule(&generic_reference(guard), &rule, &source, &[true; 2], None).unwrap();
        }
    }

    #[test]
    fn reference_sector_annotations_are_required_and_exact() {
        let (_, source, rule) = fixture();
        let good = generic_reference("/;!(n1==1)");
        for bad in [
            good.replace("n1_?Positive", "n1_?NonPositive"),
            good.replace("n2_?Positive", "n2_"),
        ] {
            assert!(compare_rule(&bad, &rule, &source, &[true; 2], None).is_err());
        }
        let inactive = good.replace("n2_?Positive", "n2_?NonPositive");
        compare_rule(&inactive, &rule, &source, &[true, false], None).unwrap();
    }

    #[test]
    fn affine_nonlinear_and_partial_negation_grammar_fails_explicitly() {
        let (_, source, rule) = fixture();
        for guard in [
            "/;!((n1+n2)==2)",
            "/;!(n1==n2)",
            "/;!(n1*n2==1)",
            "/;!(n1==1)||(n2==1)",
            "/;n1!=1",
            "/;!(n01==1)",
            "/;!(n1==1 2)",
            "/;!(!(n1==1))",
        ] {
            assert!(
                compare_rule(&generic_reference(guard), &rule, &source, &[true; 2], None).is_err(),
                "{guard}"
            );
        }
    }

    #[test]
    fn guards_are_intersected_with_fixed_coordinates_and_sector() {
        let (context, source, mut rule) = fixture();
        rule.candidate.case = CoordinateCase::new([Some(2), None]).unwrap().into();
        rule.candidate.target = rule.candidate.case.integral();
        rule.candidate.rhs[0].integral =
            Integral::new([Power::new(false, 1).unwrap(), Power::new(true, 0).unwrap()]);
        rule.exceptions.branches = vec![vec![context.parameter("b").unwrap().numerator]];
        compare_rule(
            "{int[2,n2_?NonPositive]/;!((n1==1)||((n1==2)&&(n2==0))||(n2==1))->(1)*int[1,0+n2]}",
            &rule,
            &source,
            &[true, false],
            None,
        )
        .unwrap();
        assert!(
            compare_rule(
                "{int[2,n2_?NonPositive]/;!(n1==2)->(1)*int[1,0+n2]}",
                &rule,
                &source,
                &[true, false],
                None,
            )
            .is_err()
        );
    }

    #[test]
    fn no_exceptions_agrees_with_an_impossible_reference_exception() {
        let (_, source, mut rule) = fixture();
        rule.exceptions.branches.clear();
        for guard in ["", "/;!(False)", "/;!(n1==0)", "/;!((n1==1)&&(n1==2))"] {
            compare_rule(&generic_reference(guard), &rule, &source, &[true; 2], None).unwrap();
        }
        assert!(
            compare_rule(
                &generic_reference("/;!(True)"),
                &rule,
                &source,
                &[true; 2],
                None
            )
            .is_err()
        );
    }

    #[test]
    fn native_candidate_affine_guard_is_not_silently_ignored() {
        let (context, source, mut rule) = fixture();
        rule.exceptions.branches = vec![vec![
            (&context.parameter("a").unwrap() - &context.parameter("b").unwrap()).numerator,
        ]];
        assert!(compare_rule(&generic_reference(""), &rule, &source, &[true; 2], None).is_err());
    }
}

/// Remove only parentheses enclosing the WHOLE expression. In particular,
/// `(a)||(b)` is not mistaken for one wrapped expression.
fn strip_guard_parentheses(mut text: &str) -> Result<&str> {
    split_top_level(text, "\0")?; // Validate delimiter balance first.
    loop {
        text = text.trim();
        if !text.starts_with('(') {
            return Ok(text);
        }
        let mut depth = 0usize;
        let mut closing = None;
        for (position, byte) in text.bytes().enumerate() {
            match byte {
                b'(' => depth += 1,
                b')' => {
                    depth -= 1;
                    if depth == 0 {
                        closing = Some(position);
                        break;
                    }
                }
                _ => (),
            }
        }
        if closing != Some(text.len() - 1) {
            return Ok(text);
        }
        text = &text[1..text.len() - 1];
    }
}

fn add_term<const N: usize>(
    terms: &mut BTreeMap<Integral<N>, Coefficient>,
    integral: Integral<N>,
    coefficient: Coefficient,
) {
    if let Some(previous) = terms.get_mut(&integral) {
        *previous = &*previous + &coefficient;
    } else {
        terms.insert(integral, coefficient);
    }
}

fn require_coordinate_pattern<const N: usize>(
    integral: &Integral<N>,
    fixed: &[Option<i16>; N],
) -> Result<()> {
    for (coordinate, value) in fixed.iter().enumerate() {
        if integral[coordinate].is_symbolic() != value.is_none() {
            return Err(invalid(format!(
                "RHS coordinate {} does not preserve the LHS symbolic/fixed pattern",
                coordinate + 1
            )));
        }
    }
    Ok(())
}

fn parse_lhs<const N: usize>(text: &str) -> Result<[Option<i16>; N]> {
    let (coordinates, suffix) = integral_arguments(text)?;
    if !suffix.trim().is_empty()
        && !suffix
            .trim()
            .strip_prefix("/;")
            .is_some_and(|guard| !guard.trim().is_empty())
    {
        return Err(invalid("unexpected text after the reference LHS integral"));
    }
    let coordinates = split_top_level(coordinates, ",")?;
    if coordinates.len() != N {
        return Err(invalid(format!(
            "expected {N} coordinates on the reference LHS"
        )));
    }
    let mut fixed = [None; N];
    for (coordinate, text) in coordinates.into_iter().enumerate() {
        let text: String = text.chars().filter(|c| !c.is_whitespace()).collect();
        if let Some((name, annotation)) = text.split_once('_') {
            if name != format!("n{}", coordinate + 1)
                || !matches!(annotation, "" | "?Positive" | "?NonPositive")
            {
                return Err(invalid(format!(
                    "invalid reference pattern coordinate {text}"
                )));
            }
        } else {
            let value = text
                .parse::<i16>()
                .map_err(|_| invalid(format!("invalid fixed reference coordinate {text}")))?;
            Power::new(false, value)?;
            fixed[coordinate] = Some(value);
        }
    }
    Ok(fixed)
}

fn parse_integral<const N: usize>(text: &str) -> Result<Integral<N>> {
    let (coordinates, suffix) = integral_arguments(text)?;
    if !suffix.trim().is_empty() {
        return Err(invalid("unexpected text after an RHS integral"));
    }
    let coordinates = split_top_level(coordinates, ",")?;
    if coordinates.len() != N {
        return Err(invalid(format!(
            "expected {N} coordinates in an RHS integral"
        )));
    }
    let mut powers = [Power::default(); N];
    for (coordinate, text) in coordinates.into_iter().enumerate() {
        let text: String = text.chars().filter(|c| !c.is_whitespace()).collect();
        let symbolic = text
            .split_once("+n")
            .map(|(shift, index)| (shift, index))
            .or_else(|| text.strip_prefix('n').map(|index| ("0", index)));
        powers[coordinate] = if let Some((shift, index)) = symbolic {
            if index != (coordinate + 1).to_string() {
                return Err(invalid(format!(
                    "reference index n{index} appears in coordinate {}",
                    coordinate + 1
                )));
            }
            Power::new(
                true,
                shift
                    .parse::<i16>()
                    .map_err(|_| invalid(format!("invalid reference integral shift {shift}")))?,
            )?
        } else {
            Power::new(
                false,
                text.parse::<i16>().map_err(|_| {
                    invalid(format!("invalid reference integral coordinate {text}"))
                })?,
            )?
        };
    }
    Ok(Integral::new(powers))
}

fn integral_arguments(text: &str) -> Result<(&str, &str)> {
    let body = text
        .trim()
        .strip_prefix("int[")
        .ok_or_else(|| invalid("expected int[...] in the reference rule"))?;
    let end = body
        .find(']')
        .ok_or_else(|| invalid("unclosed reference integral"))?;
    Ok((&body[..end], &body[end + 1..]))
}

/// Split only outside balanced (), [] and {}. This recognizes the export's
/// containers and separators; Symbolica owns coefficient expression parsing.
fn split_top_level<'a>(text: &'a str, separator: &str) -> Result<Vec<&'a str>> {
    if text.trim().is_empty() {
        return Ok(Vec::new());
    }
    let bytes = text.as_bytes();
    let mut stack = Vec::new();
    let mut start = 0;
    let mut position = 0;
    let mut parts = Vec::new();
    while position < bytes.len() {
        if stack.is_empty() && bytes[position..].starts_with(separator.as_bytes()) {
            let part = text[start..position].trim();
            if part.is_empty() {
                return Err(invalid("empty element in reference rule syntax"));
            }
            parts.push(part);
            position += separator.len();
            start = position;
            continue;
        }
        match bytes[position] {
            b'(' => stack.push(b')'),
            b'[' => stack.push(b']'),
            b'{' => stack.push(b'}'),
            close @ (b')' | b']' | b'}') => {
                if stack.pop() != Some(close) {
                    return Err(invalid("mismatched delimiters in reference rule syntax"));
                }
            }
            _ => {}
        }
        position += 1;
    }
    if !stack.is_empty() {
        return Err(invalid("unclosed delimiter in reference rule syntax"));
    }
    let last = text[start..].trim();
    if last.is_empty() {
        return Err(invalid("trailing separator in reference rule syntax"));
    }
    parts.push(last);
    Ok(parts)
}

fn is_identifier(name: &str) -> bool {
    let mut chars = name.chars();
    chars
        .next()
        .is_some_and(|c| c.is_ascii_alphabetic() || c == '_')
        && chars.all(|c| c.is_ascii_alphanumeric() || c == '_')
}

#[cfg(test)]
#[path = "spired_reference/alias_tests.rs"]
mod alias_tests;

#[cfg(test)]
mod pre_rule_tests {
    use super::*;
    use rustred::algebra::CoefficientContext;
    use rustred::solver::Term;

    fn fixture() -> (SourceSystem<2>, Vec<LinearCutRule<2>>) {
        // Coordinate variables have different names and a permuted map.
        // Sources are fixed at both cuts; pre-rules must nevertheless remain
        // generic and must NOT specialize their poles to those fixed values.
        let context = CoefficientContext::try_new(["b", "d", "a"]).unwrap();
        let source = SourceSystem::new_with_fixed(
            vec![vec![Term {
                integral: Integral::numeric([1, 1]).unwrap(),
                coefficient: context.one().numerator,
            }]],
            [2, 0],
            [Some(1), Some(1)],
        )
        .unwrap();
        let a = context.parameter("a").unwrap();
        let b = context.parameter("b").unwrap();
        let rules = vec![
            LinearCutRule {
                axis: 0,
                source_ordinal: 3,
                target: Integral::symbolic([0, 0]).unwrap(),
                rhs: vec![Term {
                    integral: Integral::symbolic([-1, 0]).unwrap(),
                    coefficient: &b / &(&a - &context.one()),
                }],
            },
            LinearCutRule {
                axis: 1,
                source_ordinal: 1,
                target: Integral::symbolic([0, 0]).unwrap(),
                rhs: vec![Term {
                    integral: Integral::symbolic([0, -1]).unwrap(),
                    coefficient: &a / &(&b - &context.one()),
                }],
            },
        ];
        (source, rules)
    }

    fn entry(axis: usize, guard: &str, rhs: Option<&str>) -> String {
        let rhs = rhs.unwrap_or(if axis == 0 {
            "(n2/(n1-1))*int[-1+n1,0+n2]"
        } else {
            "(n1/(n2-1))*int[0+n1,-1+n2]"
        });
        format!("int[n1_,n2_]{guard}->{rhs}")
    }

    fn reference() -> String {
        format!(
            "{{{},{}}}",
            entry(1, "/;!((n2==1))", None),
            entry(0, "/;!((n1==1))", None)
        )
    }

    #[test]
    fn pre_rules_match_by_guard_axis_and_compare_native_generic_equations() {
        let (source, rules) = fixture();
        compare_pre_rules(&reference(), &rules, &source).unwrap();
        let equivalent = reference().replace("n2/(n1-1)", "(n1*n2+n2)/(n1^2-1)");
        compare_pre_rules(&equivalent, &rules, &source).unwrap();
        compare_pre_rules("{}", &[], &source).unwrap();
    }

    #[test]
    fn pre_rules_reject_wrong_or_missing_guards_even_when_rhs_is_zero() {
        let (source, mut rules) = fixture();
        rules.truncate(1);
        rules[0].rhs.clear();
        compare_pre_rules(
            &format!("{{{}}}", entry(0, "/;!(n1==1)", Some("0"))),
            &rules,
            &source,
        )
        .unwrap();
        for guard in [
            "",
            "/;!(n1==0)",
            "/;!(n1==2)",
            "/;!(n2==1)",
            "/;!(False)",
            "/;!((n1==1)||(n2==1))",
            "/;!((n1==1)&&(n2==1))",
            "/;n1!=1",
        ] {
            assert!(
                compare_pre_rules(
                    &format!("{{{}}}", entry(0, guard, Some("0"))),
                    &rules,
                    &source,
                )
                .is_err(),
                "{guard}"
            );
        }
    }

    #[test]
    fn pre_rules_reject_duplicate_missing_and_extra_axes_on_either_side() {
        let (source, mut rules) = fixture();
        let one = entry(0, "/;!(n1==1)", None);
        for reference in [
            "{}".to_owned(),
            format!("{{{one}}}"),
            format!("{{{one},{one}}}"),
        ] {
            assert!(compare_pre_rules(&reference, &rules, &source).is_err());
        }
        assert!(compare_pre_rules(&reference(), &rules[..1], &source).is_err());
        rules[1].axis = 0;
        assert!(compare_pre_rules(&reference(), &rules, &source).is_err());
        rules[1].axis = 2;
        assert!(compare_pre_rules(&reference(), &rules, &source).is_err());
    }

    #[test]
    fn pre_rules_reject_rhs_changes_noncanonical_targets_and_restricted_patterns() {
        let (source, mut rules) = fixture();
        let changed = reference().replace("n2/(n1-1)", "(n2+1)/(n1-1)");
        assert!(compare_pre_rules(&changed, &rules, &source).is_err());
        for pattern in ["n1_?Positive", "n1_?NonPositive", "1"] {
            let restricted = reference().replace("n1_", pattern);
            assert!(compare_pre_rules(&restricted, &rules, &source).is_err());
        }
        rules[0].target = Integral::symbolic([1, 0]).unwrap();
        assert!(compare_pre_rules(&reference(), &rules, &source).is_err());
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rustred::algebra::CoefficientContext;
    use rustred::solver::{CoordinateCase, SearchStats, Term};

    fn fixture() -> (SourceSystem<1>, RuleCandidate<1>) {
        // The reference n1 must map by coordinate, not by the native symbol's
        // spelling or position: its actual source variable here is "a".
        let context = CoefficientContext::try_new(["d", "a"]).unwrap();
        let source = SourceSystem::new(
            vec![vec![Term {
                integral: Integral::symbolic([0]).unwrap(),
                coefficient: context.one().numerator,
            }]],
            [1],
        )
        .unwrap();
        let candidate = RuleCandidate {
            case: CoordinateCase::generic().into(),
            target: Integral::symbolic([0]).unwrap(),
            rhs: vec![Term {
                integral: Integral::symbolic([-1]).unwrap(),
                coefficient: &context.parameter("a").unwrap() + &context.one(),
            }],
            sources: Vec::new(),
            stats: SearchStats::default(),
        };
        (source, candidate)
    }

    #[test]
    fn exact_comparison_uses_native_normalization_and_coordinate_variable_mapping() {
        let (source, candidate) = fixture();
        let reference = "{int[n1_?Positive]/;!((n1==1))->((n1^2-1)/(n1-1))*int[-1+n1]}";
        compare(reference, &candidate, &source, None).unwrap();
        assert!(
            compare(
                "{int[n1_?Positive]->(n1+2)*int[-1+n1]}",
                &candidate,
                &source,
                None,
            )
            .is_err()
        );
        assert!(compare("{int[n1_?Positive]->0}", &candidate, &source, None).is_err());
        assert!(compare("{int[1]->0}", &candidate, &source, None).is_err());
    }

    #[test]
    fn mass_specialization_is_explicit_and_undefined_specializations_fail() {
        let (source, candidate) = fixture();
        let reference = "{int[n1_?Positive]->((n1+1)/m)*int[-1+n1]}";
        compare(reference, &candidate, &source, Some("m")).unwrap();
        assert!(compare(reference, &candidate, &source, None).is_err());
        assert!(
            compare(
                "{int[n1_?Positive]->((n1+1)/(m-1))*int[-1+n1]}",
                &candidate,
                &source,
                Some("m"),
            )
            .is_err()
        );
        assert!(compare(reference, &candidate, &source, Some("n1")).is_err());
    }

    #[test]
    fn fixed_coordinates_are_specialized_and_ambiguous_patterns_rejected() {
        let (source, mut candidate) = fixture();
        candidate.case = CoordinateCase::new([Some(2)]).unwrap().into();
        candidate.target = candidate.case.integral();
        candidate.rhs[0].integral = Integral::numeric([1]).unwrap();
        candidate.rhs[0].coefficient = source.rows()[0][0]
            .coefficient
            .constant(Integer::from(3))
            .into();
        compare("{int[2]->(n1+1)*int[1]}", &candidate, &source, None).unwrap();
        assert!(
            compare(
                "{int[2]->(3)*int[1],int[2]/;!(n1==1)->(3)*int[1]}",
                &candidate,
                &source,
                None,
            )
            .is_err()
        );
    }

    #[test]
    fn split_respects_nested_guards_and_integral_coordinates() {
        let rules = "int[n1_?Positive,1]/;!((n1==1))->((n1+2)/(3*n1-3))*int[-1+n1,1],int[1,1]->0";
        let split = split_top_level(rules, ",").unwrap();
        assert_eq!(split.len(), 2);
        let rule = split_top_level(split[0], "->").unwrap();
        assert_eq!(rule.len(), 2);
        assert_eq!(parse_lhs::<2>(rule[0]).unwrap(), [None, Some(1)]);
        assert!(split_top_level("int[1,2)", ",").is_err());
        assert!(split_top_level("int[1,2],", ",").is_err());
    }

    #[test]
    fn integral_parser_checks_coordinate_names_types_and_compact_bounds() {
        let integral = parse_integral::<3>("int[-2+n1, 0+n2, -1]").unwrap();
        assert_eq!(integral.powers().map(Power::value), [-2, 0, -1]);
        assert!(integral[0].is_symbolic());
        assert!(!integral[2].is_symbolic());
        assert!(parse_integral::<2>("int[-1+n2,0+n1]").is_err());
        assert!(parse_integral::<1>("int[64+n1]").is_err());
        assert!(parse_lhs::<2>("int[n2_?Positive,1]").is_err());
        assert!(require_coordinate_pattern(&integral, &[None, Some(1), None]).is_err());
    }
}
