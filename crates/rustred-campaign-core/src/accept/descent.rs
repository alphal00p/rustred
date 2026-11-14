//! d3 descent of a candidate at a key, with the CLOSURE's instantiation code (rule point, offsets, term classes:
//! `closure::bodies`) and the node comparator (full comparator with tie-breaks, R29 (b)); positive-numerator and
//! non-lower terms are followed only if their coefficient is verified ZERO at the key (R9 (d), R13 A2).
//! Also the symbolic pattern check of a FULL node's scope (`NodeComparator::cmp_pattern`, the engine's
//! `compare_mixed`).

use std::cmp::Ordering;
use std::collections::BTreeSet;

use super::facts::{CoefficientFacts, split_ratio};
use super::rule::RuleText;
use crate::closure::bodies::{RhsSlot, RuleBodies, Sectors, TermClass, parse_rhs_pattern};
use crate::closure::facts::CoefFact;
use crate::cover::{Dispatcher, Table};
use crate::domain::{CaseDomain, PoleKind, TargetMap, TargetSlot};
use crate::order::{NodeComparator, parse_pattern};
use crate::poly::Poly;

/// The candidate alone in a one-record dispatcher, with its body: the closure's `RuleBodies::instantiate` applies
/// the rule's point map and the pattern offsets exactly as the closure does.
pub struct Instantiator {
    disp: Dispatcher,
    bodies: RuleBodies,
    arity: usize,
}

impl Instantiator {
    pub fn new(rule: &RuleText) -> Result<Instantiator, String> {
        let dom = rule.domain.clone().ok_or("candidate without a domain")?;
        let arity = dom.arity;
        let mut t = Table::new("candidate", arity);
        t.push(rule.index().unwrap_or(0), dom)
            .map_err(|e| e.to_string())?;
        let disp = Dispatcher::new(vec![t], vec![0]).map_err(|e| e.to_string())?;
        let mut bodies = RuleBodies::new(&disp);
        let pats: Vec<&str> = rule.rhs.iter().map(|(k, _)| k.as_str()).collect();
        bodies
            .set(&disp, 0, 0, &pats)
            .map_err(|e| format!("{e:?}"))?;
        Ok(Instantiator {
            disp,
            bodies,
            arity,
        })
    }

    /// The RHS keys at `key` with their constant-slot masks.
    pub fn terms(&self, key: &[i32]) -> Result<Vec<(Vec<i32>, u64)>, String> {
        let mut out = Vec::new();
        let mut cm = Vec::new();
        let r = self.disp.rule_ref(0, 0);
        let n = self
            .bodies
            .instantiate(&self.disp, r, key, &mut out, &mut cm)
            .map_err(|e| format!("{e:?}"))?
            .ok_or("no body")?;
        Ok((0..n)
            .map(|j| (out[j * self.arity..(j + 1) * self.arity].to_vec(), cm[j]))
            .collect())
    }
}

/// Why a key fails descent.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum DescentFailure {
    /// A top-sector term not lower than the key whose coefficient is NOT zero.
    NotLower { term: usize, rhs: Vec<i32> },
    /// A positive-numerator term whose coefficient is NOT zero.
    PositiveNumeratorNonZero { term: usize, rhs: Vec<i32> },
}

/// d3 at one key.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum DescentAt {
    Pass {
        /// terms dropped because their coefficient is verified zero
        dropped_zero: usize,
        /// hand-downs (subsector / zero sector)
        hand_downs: usize,
    },
    Fail(DescentFailure),
    Unknown {
        term: usize,
        rhs: Vec<i32>,
        why: String,
    },
}

/// d3 of `rule` at `key`.
pub fn descent_at(
    inst: &Instantiator,
    sectors: &Sectors,
    cmp: &NodeComparator,
    facts: &mut dyn CoefficientFacts,
    rule: &RuleText,
    key: &[i32],
) -> DescentAt {
    let terms = match inst.terms(key) {
        Ok(t) => t,
        Err(e) => {
            return DescentAt::Unknown {
                term: usize::MAX,
                rhs: vec![],
                why: e,
            };
        }
    };
    let mut dropped_zero = 0;
    let mut hand_downs = 0;
    for (j, (t, cm)) in terms.into_iter().enumerate() {
        match sectors.classify_masked(&t, cm) {
            TermClass::Subsector { .. } | TermClass::ZeroSector { .. } => hand_downs += 1,
            TermClass::Top => match cmp.try_cmp(&t, key) {
                Ok(Ordering::Less) => {}
                Ok(_) => match facts.coefficient(rule, key, j) {
                    CoefFact::Zero => dropped_zero += 1,
                    CoefFact::NonZero => {
                        return DescentAt::Fail(DescentFailure::NotLower { term: j, rhs: t });
                    }
                    CoefFact::Unknown => {
                        return DescentAt::Unknown {
                            term: j,
                            rhs: t,
                            why: "not lower, coefficient unknown".into(),
                        };
                    }
                },
                Err(e) => {
                    return DescentAt::Unknown {
                        term: j,
                        rhs: t,
                        why: format!("comparator: {e:?}"),
                    };
                }
            },
            TermClass::PositiveNumerator { .. } => match facts.coefficient(rule, key, j) {
                CoefFact::Zero => dropped_zero += 1,
                CoefFact::NonZero => {
                    return DescentAt::Fail(DescentFailure::PositiveNumeratorNonZero {
                        term: j,
                        rhs: t,
                    });
                }
                CoefFact::Unknown => {
                    return DescentAt::Unknown {
                        term: j,
                        rhs: t,
                        why: "positive numerator, coefficient unknown".into(),
                    };
                }
            },
        }
    }
    DescentAt::Pass {
        dropped_zero,
        hand_downs,
    }
}

/// Range of one coordinate over a rule's case domain (an OVER-approximation: equations, bounds and exceptions only
/// shrink the domain, so a property proved on the range holds on the domain).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Range {
    Exactly(i64),
    AtLeast(i64),
    AtMost(i64),
    Any,
}

impl Range {
    fn shift(self, o: i64) -> Range {
        match self {
            Range::Exactly(v) => Range::Exactly(v + o),
            Range::AtLeast(v) => Range::AtLeast(v + o),
            Range::AtMost(v) => Range::AtMost(v + o),
            Range::Any => Range::Any,
        }
    }
    /// Some value of the range is >= 1.
    pub fn can_be_positive(self) -> bool {
        match self {
            Range::Exactly(v) | Range::AtMost(v) => v >= 1,
            Range::AtLeast(_) | Range::Any => true,
        }
    }
    /// Every value of the range is <= 0.
    pub fn always_nonpositive(self) -> bool {
        match self {
            Range::Exactly(v) | Range::AtMost(v) => v <= 0,
            Range::AtLeast(_) | Range::Any => false,
        }
    }
    /// The integer `r` is outside the range.
    pub fn excludes(self, r: i64) -> bool {
        match self {
            Range::Exactly(v) => v != r,
            Range::AtLeast(v) => r < v,
            Range::AtMost(v) => r > v,
            Range::Any => false,
        }
    }
}

/// Ranges of the RULE POINT's coordinates over the case domain: from the target map (key slot j = `n<p>+o` gives
/// `pt[p] = k[j] - o`), the fixed slots (tested on the key) and the chart (line slots >= 1, numerator slots <= 0); a
/// parameter no slot maps is the fixed value of its slot, else 0 (the engine's `find_rule`). Without a chart a free
/// slot is unbounded.
pub fn point_ranges(dom: &CaseDomain) -> Vec<Range> {
    let key_range = |j: usize| -> Range {
        if let Some(v) = dom.fixed.get(j).copied().flatten() {
            return Range::Exactly(i64::from(v));
        }
        match &dom.chart {
            Some(c) if c[j] => Range::AtLeast(1),
            Some(_) => Range::AtMost(0),
            None => Range::Any,
        }
    };
    match &dom.target {
        TargetMap::Canonical => (0..dom.arity).map(key_range).collect(),
        TargetMap::Pattern(t) => (0..dom.arity)
            .map(|p| {
                t.iter()
                    .enumerate()
                    .find_map(|(j, s)| match *s {
                        TargetSlot::Param { param, offset } if param == p => {
                            Some(key_range(j).shift(-offset))
                        }
                        _ => None,
                    })
                    .unwrap_or(Range::Exactly(i64::from(
                        dom.fixed.get(p).copied().flatten().unwrap_or(0),
                    )))
            })
            .collect(),
    }
}

/// Symbolic descent of every RHS term on the WHOLE case domain (FULL scope (d), audit-accept-A C1): the terms that
/// are NOT proved. A term is skipped as a hand-down when one of its line slots is <= 0 on the whole domain. A term
/// with a NUMERATOR slot that can be positive somewhere on the domain (a constant > 0, or `n<p>+o` whose range
/// reaches 1) is NOT proved (a positive numerator needs a zero coefficient, which only a per-key fact can show).
/// Every other term must compare `Less` than the target pattern under `cmp_pattern` (the engine's `compare_mixed`) on
/// the RULE'S chart (`lines` when the domain has none).
pub fn symbolic_not_proved(
    rule: &RuleText,
    cmp: &NodeComparator,
    lines: &[bool],
) -> BTreeSet<usize> {
    let all = || (0..rule.rhs.len()).collect();
    let Some(dom) = &rule.domain else {
        return all();
    };
    let chart: Vec<bool> = dom.chart.clone().unwrap_or_else(|| lines.to_vec());
    let ranges = point_ranges(dom);
    let target = match rule.record.target.as_deref().map(parse_pattern) {
        Some(Ok(t)) => t,
        _ => return all(),
    };
    let mut bad = BTreeSet::new();
    for (j, (k, _)) in rule.rhs.iter().enumerate() {
        let (Ok(p), Ok(slots)) = (parse_pattern(k), parse_rhs_pattern(k, dom.arity)) else {
            bad.insert(j);
            continue;
        };
        let vals: Vec<Range> = slots
            .iter()
            .map(|s| match *s {
                RhsSlot::Param { param, offset } => ranges
                    .get(param)
                    .copied()
                    .unwrap_or(Range::Any)
                    .shift(i64::from(offset)),
                RhsSlot::Const(c) => Range::Exactly(i64::from(c)),
            })
            .collect();
        let hand_down = vals
            .iter()
            .zip(lines)
            .any(|(v, &line)| line && v.always_nonpositive());
        if hand_down {
            continue;
        }
        let positive_numerator = vals
            .iter()
            .zip(lines)
            .any(|(v, &line)| !line && v.can_be_positive());
        if positive_numerator || cmp.cmp_pattern(&chart, &p, &target) != Ok(Ordering::Less) {
            bad.insert(j);
        }
    }
    bad
}

/// Split a denominator text at its top-level `*` into factor texts, a trailing top-level `^<digits>` dropped (the
/// zero set of `f^k` is that of `f`). This reads the PRINTED product structure; nothing is factored (R38).
fn printed_factors(den: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut depth = 0i32;
    let mut cur = String::new();
    for c in den.trim().chars() {
        match c {
            '(' => depth += 1,
            ')' => depth -= 1,
            _ => {}
        }
        if c == '*' && depth == 0 {
            out.push(std::mem::take(&mut cur));
        } else {
            cur.push(c);
        }
    }
    out.push(cur);
    out.into_iter()
        .map(|f| {
            let t = f.trim();
            match t.rfind('^') {
                Some(i)
                    if t[i + 1..].chars().all(|c| c.is_ascii_digit())
                        && i + 1 < t.len()
                        && t[..i].ends_with(')') =>
                {
                    t[..i].to_string()
                }
                _ => t.to_string(),
            }
        })
        .collect()
}

/// `c` (d-free) is a nonzero rational multiple of `l`.
fn proportional(c: &Poly, l: &Poly) -> bool {
    // a big exact literal (coefficient beyond i128): not decided here (false = not proven, conservative)
    let (Some(a), Some(b)) = (c.small_terms(), l.small_terms()) else {
        return false;
    };
    if a.is_empty() || a.len() != b.len() {
        return false;
    }
    let (a0, b0) = (a[0].1, b[0].1);
    a.iter().zip(b).all(|((ma, ca), (mb, cb))| {
        ma == mb
            && match (ca.checked_mul(b0), cb.checked_mul(a0)) {
                (Some(x), Some(y)) => x == y,
                _ => false,
            }
    })
}

/// A d-free polynomial that NEVER vanishes on the effective domain, by one of three sufficient criteria: a nonzero
/// constant; a nonzero multiple of a single-literal exception branch (the domain excludes its zero set); affine in
/// ONE slot `a n_p + b` whose root `-b/a` is not an integer or lies outside the range of `pt[p]`.
fn never_zero(c: &Poly, singles: &[&Poly], ranges: &[Range]) -> bool {
    if let Some(v) = c.as_constant() {
        return v != 0;
    }
    if singles.iter().any(|l| proportional(c, l)) {
        return true;
    }
    let sup = c.support();
    if sup.len() == 1 && c.degree() == 1 && !c.has_d() {
        let p = sup[0];
        let (mut a, mut b) = (0i128, 0i128);
        let Some(ct) = c.small_terms() else {
            return false;
        };
        for (m, k) in ct {
            if m.is_empty() {
                b = *k;
            } else {
                a = *k;
            }
        }
        if a == 0 {
            return false;
        }
        if b % a != 0 {
            return true;
        }
        let Ok(r) = i64::try_from(-b / a) else {
            return true;
        };
        return ranges.get(p).is_some_and(|rg| rg.excludes(r));
    }
    false
}

/// A polynomial in (n, d) that is never IDENTICALLY ZERO IN d on the effective domain: some d-coefficient never
/// vanishes there ([`never_zero`]).
fn never_identically_zero(p: &Poly, singles: &[&Poly], ranges: &[Range]) -> bool {
    p.d_coefficients()
        .iter()
        .any(|(_, c)| never_zero(c, singles, ranges))
}

/// Symbolic REGULARITY on the whole effective domain (FULL scope, audit-accept-B C3): every printed factor of every
/// coefficient denominator and every witness locus must be shown nonvanishing on the effective domain
/// ([`never_identically_zero`]); the reasons why not (empty = proved). Per-key exact regularity (check 3) is
/// unchanged; this only decides whether the record may claim the whole domain.
pub fn symbolic_regularity_not_proved(rule: &RuleText) -> Vec<String> {
    let Some(dom) = &rule.domain else {
        return vec!["no case domain".into()];
    };
    let n = dom.arity;
    let ranges = point_ranges(dom);
    let singles: Vec<&Poly> = dom
        .exceptions
        .iter()
        .filter(|b| b.len() == 1)
        .map(|b| &b[0])
        .collect();
    let mut why = Vec::new();
    for (j, (_, coef)) in rule.rhs.iter().enumerate() {
        let den = match split_ratio(coef.trim()) {
            Ok((_, None)) => continue,
            Ok((_, Some(d))) => d,
            Err(e) => {
                why.push(format!("term {j}: coefficient form: {e}"));
                continue;
            }
        };
        for f in printed_factors(den) {
            match Poly::parse(&f, n, true) {
                Ok(p) if never_identically_zero(&p, &singles, &ranges) => {}
                Ok(_) => why.push(format!(
                    "term {j}: denominator factor {f} not shown nonzero on the domain"
                )),
                Err(e) => why.push(format!("term {j}: denominator factor {f}: {e:?}")),
            }
        }
    }
    for (i, p) in dom.poles.iter().enumerate() {
        if p.kind != PoleKind::Witness {
            continue;
        }
        if !p.coeffs.iter().any(|c| never_zero(c, &singles, &ranges)) {
            why.push(format!(
                "witness locus {i} not shown nonzero on the domain (the rule would be blocked there)"
            ));
        }
    }
    why
}
