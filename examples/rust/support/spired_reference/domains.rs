//! Oracle-only domain syntax and exact case comparison.
//!
//! Boolean/list parsing lives here; Symbolica parses polynomial arithmetic and
//! RustRed's shared Case geometry owns all restriction, emptiness and inclusion.

use super::*;
use rustred::algebra::CoefficientPolynomial;
use rustred::solver::{CoordinateCase, ExceptionalConditions, SearchStats, SectorRule};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SectorComparison {
    pub matched_rules: usize,
    /// Reference required domains proved empty over integer sector indices.
    pub integer_empty_rules: usize,
}

pub(super) struct ReferenceEntry<'a, const N: usize> {
    pub lhs: &'a str,
    pub rhs: &'a str,
    pub case: Case<N>,
    exclusions: Vec<&'a str>,
}

fn rule_entries(reference: &str) -> Result<Vec<(&str, &str)>> {
    let body = reference
        .trim()
        .strip_prefix('{')
        .and_then(|s| s.strip_suffix('}'))
        .ok_or_else(|| invalid("expected a SpIRed rule list enclosed in braces"))?;
    split_top_level(body, ",")?
        .into_iter()
        .map(|entry| {
            let parts = split_top_level(entry, "->")?;
            if parts.len() != 2 {
                return Err(invalid("each reference rule requires one top-level ->"));
            }
            Ok((parts[0], parts[1]))
        })
        .collect()
}

fn split_conditions(suffix: &str) -> Result<(Vec<&str>, Vec<&str>)> {
    if suffix.trim().is_empty() {
        return Ok((Vec::new(), Vec::new()));
    }
    let condition = suffix
        .trim()
        .strip_prefix("/;")
        .ok_or_else(|| invalid("expected /; before reference conditions"))?;
    let condition = strip_guard_parentheses(condition)?;
    let mut required = Vec::new();
    let mut excluded = Vec::new();
    for part in split_top_level(condition, "&&")? {
        let part = strip_guard_parentheses(part)?;
        if let Some(inner) = part.strip_prefix('!') {
            let inner = inner.trim();
            let unwrapped = strip_guard_parentheses(inner)?;
            if inner.len() == unwrapped.len() {
                return Err(invalid(
                    "reference exception negation must wrap the whole expression",
                ));
            }
            excluded.push(unwrapped);
        } else {
            if split_top_level(part, "||")?.len() > 1 {
                return Err(invalid("a required reference case must be a conjunction"));
            }
            required.push(part);
        }
    }
    Ok((required, excluded))
}

fn reference_sector<const N: usize>(lhs: &str, supplied: Option<&[bool; N]>) -> Result<[bool; N]> {
    let fixed = parse_lhs::<N>(lhs)?;
    let (arguments, _) = integral_arguments(lhs)?;
    let mut sector = [true; N];
    for (axis, argument) in split_top_level(arguments, ",")?.into_iter().enumerate() {
        if let Some(value) = fixed[axis] {
            sector[axis] = value > 0;
        } else {
            let compact: String = argument.chars().filter(|c| !c.is_whitespace()).collect();
            sector[axis] = if compact == format!("n{}_?NonPositive", axis + 1) {
                false
            } else if compact == format!("n{}_?Positive", axis + 1) || supplied.is_none() {
                true
            } else {
                return Err(invalid(
                    "reference rule lacks the exact sector-sign annotation",
                ));
            };
        }
    }
    if supplied.is_some_and(|supplied| supplied != &sector) {
        return Err(invalid(
            "reference sector annotations disagree with the requested sector",
        ));
    }
    Ok(sector)
}

fn parse_entry<'a, const N: usize>(
    lhs: &'a str,
    rhs: &'a str,
    system: &SourceSystem<N>,
    sector: Option<&[bool; N]>,
) -> Result<Option<ReferenceEntry<'a, N>>> {
    let sector = reference_sector(lhs, sector)?;
    let face = CoordinateCase::new(parse_lhs(lhs)?)?;
    let (_, suffix) = integral_arguments(lhs)?;
    let (required, exclusions) = split_conditions(suffix)?;
    let equations = required
        .into_iter()
        .map(|equation| parse_equation(equation, system))
        .collect::<Result<Vec<_>>>()?;
    let initial: Case<N> = face.into();
    Ok(initial
        .intersect(&equations, system.index_variables(), &sector)?
        .map(|case| ReferenceEntry {
            lhs,
            rhs,
            case,
            exclusions,
        }))
}

pub(super) fn matching_entry<'a, const N: usize>(
    reference: &'a str,
    case: &Case<N>,
    system: &SourceSystem<N>,
    sector: Option<&[bool; N]>,
) -> Result<ReferenceEntry<'a, N>> {
    let mut matching = None;
    for (lhs, rhs) in rule_entries(reference)? {
        if let Some(entry) = parse_entry(lhs, rhs, system, sector)? {
            if equivalent(&entry.case, case)? {
                if matching.replace(entry).is_some() {
                    return Err(invalid(
                        "multiple reference rules have the same required case",
                    ));
                }
            }
        }
    }
    matching.ok_or_else(|| invalid(format!("no reference rule matches required case {case:?}")))
}

pub(super) fn equivalent<const N: usize>(left: &Case<N>, right: &Case<N>) -> Result<bool> {
    Ok(left == right || (left.contains(right)? && right.contains(left)?))
}

/// Whole-sector oracle validation; no reference expression is sent to search.
/// Required integer-empty domains may be omitted, but every nonempty case must
/// match exactly once, including its exact coefficient functions and exclusions.
pub fn compare_sector_with_aliases<const N: usize>(
    reference: &str,
    rules: &[SectorRule<N>],
    system: &SourceSystem<N>,
    sector: &[bool; N],
    reference_mass_one: Option<&str>,
    aliases: &[CoefficientAlias<'_>],
) -> Result<SectorComparison> {
    let mut entries: Vec<ReferenceEntry<'_, N>> = Vec::new();
    let mut integer_empty_rules = 0;
    for (lhs, rhs) in rule_entries(reference)? {
        match parse_entry(lhs, rhs, system, Some(sector))? {
            Some(entry) => {
                for previous in &entries {
                    if equivalent(&entry.case, &previous.case)? {
                        return Err(invalid("duplicate nonempty reference required case"));
                    }
                }
                entries.push(entry);
            }
            None => integer_empty_rules += 1,
        }
    }
    if entries.len() != rules.len() {
        return Err(invalid(format!(
            "different nonempty rule counts: reference {} ({} integer-empty omitted); candidate {}",
            entries.len(),
            integer_empty_rules,
            rules.len()
        )));
    }
    let mut matched = vec![false; entries.len()];
    for rule in rules {
        if rule.candidate.target != rule.candidate.case.integral() {
            return Err(invalid("candidate target is not its canonical case"));
        }
        if !rule.candidate.case.is_in_sector(sector) {
            return Err(invalid("candidate required case is outside the sector"));
        }
        let mut found = None;
        for (position, entry) in entries.iter().enumerate() {
            if equivalent(&entry.case, &rule.candidate.case)? {
                if found.replace(position).is_some() {
                    return Err(invalid("ambiguous required-case match"));
                }
            }
        }
        let position =
            found.ok_or_else(|| invalid("candidate required case is absent from reference"))?;
        if std::mem::replace(&mut matched[position], true) {
            return Err(invalid("duplicate candidate required case"));
        }
        let entry = &entries[position];
        compare_rhs(
            entry.rhs,
            &parse_lhs(entry.lhs)?,
            &rule.candidate,
            system,
            reference_mass_one,
            aliases,
        )?;
        compare_exceptions(entry, rule, system, sector)?;
    }
    Ok(SectorComparison {
        matched_rules: rules.len(),
        integer_empty_rules,
    })
}

pub(super) fn compare_exceptions<const N: usize>(
    reference: &ReferenceEntry<'_, N>,
    rule: &SectorRule<N>,
    system: &SourceSystem<N>,
    sector: &[bool; N],
) -> Result<()> {
    let mut branches = Vec::new();
    for expression in &reference.exclusions {
        branches.extend(parse_guard(expression, system)?);
    }
    let expected_rule = SectorRule {
        candidate: RuleCandidate {
            case: reference.case.clone(),
            target: reference.case.integral(),
            rhs: Vec::new(),
            sources: Vec::new(),
            stats: SearchStats::default(),
        },
        exceptions: ExceptionalConditions { branches },
    };
    let expected = expected_rule.exceptional_cases(system.index_variables(), sector)?;
    let actual = rule.exceptional_cases(system.index_variables(), sector)?;
    if !same_case_union(&expected, &actual)? {
        return Err(invalid(format!(
            "different exceptional domains: reference {expected:?}; candidate {actual:?}"
        )));
    }
    Ok(())
}

fn same_case_union<const N: usize>(left: &[Case<N>], right: &[Case<N>]) -> Result<bool> {
    for (subset, superset) in [(left, right), (right, left)] {
        for case in subset {
            let mut covered = false;
            for other in superset {
                if other.contains(case)? {
                    covered = true;
                    break;
                }
            }
            if !covered {
                return Ok(false);
            }
        }
    }
    Ok(true)
}

fn template<const N: usize>(system: &SourceSystem<N>) -> Result<&CoefficientPolynomial> {
    Ok(&system
        .rows()
        .iter()
        .flatten()
        .next()
        .ok_or_else(|| invalid("source system has no coefficient template"))?
        .coefficient)
}

/// Keep the coordinate-only parser as the common path. The fallback still
/// parses Boolean structure only; native tokens own all polynomial arithmetic.
fn parse_guard<const N: usize>(
    text: &str,
    system: &SourceSystem<N>,
) -> Result<Vec<Vec<CoefficientPolynomial>>> {
    if let Ok(branches) = parse_coordinate_guard::<N>(text) {
        let template = template(system)?;
        return Ok(branches
            .into_iter()
            .map(|branch| {
                branch
                    .into_iter()
                    .map(|(axis, value)| {
                        let variable = template
                            .variable(&template.variables()[system.index_variables()[axis]])
                            .expect("validated source index");
                        &variable + &template.constant(-value)
                    })
                    .collect()
            })
            .collect());
    }
    let text = strip_guard_parentheses(text)?;
    let alternatives = split_top_level(text, "||")?;
    if alternatives.len() > 1 {
        let mut result = Vec::new();
        for branch in alternatives {
            result.extend(parse_guard(branch, system)?);
        }
        return Ok(result);
    }
    let conjunction = split_top_level(text, "&&")?;
    if conjunction.len() > 1 {
        let mut product = vec![Vec::new()];
        for factor in conjunction {
            let alternatives = parse_guard(factor, system)?;
            let mut next = Vec::new();
            for left in &product {
                for right in &alternatives {
                    let mut combined = left.clone();
                    combined.extend(right.iter().cloned());
                    next.push(combined);
                }
            }
            product = next;
        }
        return Ok(product);
    }
    Ok(vec![vec![parse_equation(text, system)?]])
}

fn parse_equation<const N: usize>(
    text: &str,
    system: &SourceSystem<N>,
) -> Result<CoefficientPolynomial> {
    let text = strip_guard_parentheses(text)?;
    let sides = split_top_level(text, "==")?;
    if sides.len() != 2 {
        return Err(invalid("reference equality requires exactly one =="));
    }
    let template = template(system)?;
    let variables = template.variables();
    let names = variables
        .iter()
        .enumerate()
        .map(|(position, variable)| {
            if let Some(axis) = system
                .index_variables()
                .iter()
                .position(|&index| index == position)
            {
                Ok(format!("n{}", axis + 1).into())
            } else if let PolyVariable::Symbol(symbol) = variable {
                Ok(symbol.get_stripped_name().into())
            } else {
                Err(invalid("reference guards require named source parameters"))
            }
        })
        .collect::<Result<Vec<_>>>()?;
    if names
        .iter()
        .enumerate()
        .any(|(position, name)| names[..position].contains(name))
    {
        return Err(invalid(
            "ambiguous reference variable names in guard context",
        ));
    }
    let parse = |side: &str| -> Result<CoefficientPolynomial> {
        let token = Token::parse(side, ParseSettings::polynomial())
            .map_err(|error| invalid(format!("invalid guard polynomial: {error}")))?;
        let coefficient: Coefficient = token
            .to_rational_polynomial(&Q, &Z, variables, &names)
            .map_err(|error| invalid(format!("invalid guard polynomial: {error}")))?;
        if !coefficient.denominator.is_one() {
            return Err(invalid("reference guard must be an integer polynomial"));
        }
        Ok(coefficient.numerator)
    };
    Ok(&parse(sides[0])? - &parse(sides[1])?)
}

#[cfg(test)]
#[path = "domains/tests.rs"]
mod tests;
