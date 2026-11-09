//! The opt-in L2 fall-through of Custom-owner generation, and T075c through
//! it (owner: M3-T13 rr-portfolio; plan M3 D-M3-4 and G2; AD sec.4.3).
//!
//! T075c (`t075/T075c.sym`, the M2-T10b regression file) is a 4L node with
//! one eps line whose top chart SpIRed refuses under the default order ("a
//! branch retains unsupported nonlinear equalities", plan M2 sec.11). Through
//! the fall-through the generator closes it at 0 residuals = LP 0, and the
//! integrals it reduces equal those of the VERIFIED_RS T075p1 set (the same
//! node with lines and ISPs permuted, `t075/T075p1.sym`; Astra's promotion
//! 2026-11-08) at 200 random points, each at two or more values of `d`.
//! Provenance of the copies: `t075/README.md`.
//!
//! Values are compared on a common basis: both rule sets reduce a key of the
//! top support onto keys of lower sectors (exits), which one program of every
//! lower chart of the support, generated here under the payload's order, then
//! reduces onto its residual keys. The RS set is in Astra's convention
//! `q.q = +1`, a presentation in `q.q = -1` (convention C1): a term of the
//! RS reduction of `k` onto `e` carries `(-1)^(sum e - sum k)` (both sides are
//! homogeneous in `q^2`).
use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;
use std::time::Instant;

use rustred::algebra::CoefficientContext;
use rustred::order::{CompiledOrder, Limits, OrderDescriptor};
use rustred::propagator::PresentationId;
use rustred::propagator::custom::{
    CustomPower, custom_payload_dump, custom_payload_rules, inspect_custom_payload,
};
use rustred::propagator::error::PError;
use rustred::propagator::families::P_FAMILIES_V1_JSON;
use rustred::propagator::presentation::{Presentation, PresentationKind, Slot, SlotIsp, SlotLine};
use rustred::sector::{ComplexityKey, Mask, OrderingPolicy, zero};
use rustred_app::propagator::generate::{
    CustomOwnerFallthrough, CustomOwnerOptions, FALLTHROUGH_SOURCE_ORDERS, FallthroughOrder,
    GenerationBudget, custom_owner_nodes, fallthrough_orders, fallthrough_retries,
    generate_custom_owner_on_order, generate_custom_owner_with_report,
};
use rustred_app::{CandidateGenerationSession, FamilyCandidatesRequest, SectorFailurePolicy};
use symbolica::domains::rational::Rational;

const T075C: &str = include_str!("t075/T075c.sym");
const T075P1: &str = include_str!("t075/T075p1.sym");
/// Astra's slot map of T075p1 onto T075c (`t075-rs-promotion.json` RS6).
const T075P1_SLOTS: [usize; 14] = [3, 5, 1, 4, 0, 8, 2, 6, 7, 12, 13, 11, 9, 10];
/// The RS set: `rules_study/ko4/T075/ko4-dump/OUT.table.jsonl` (table_dump.v1).
const RS_TABLE_ENV: &str = "RUSTRED_T075P1_RS_TABLE";
/// Points whose value (not only the difference) is reduced, to show that the
/// values compared are not all zero.
const SAMPLE: usize = 20;
/// Values of `d` that tell a pole at one `d` from a pole at every `d`.
const GENERIC_DS: [(i64, i64); 2] = [(10_007, 2_003), (20_011, 4_001)];

// ----- the fall-through is opt-in -----

/// On an owner that closes (t1star05), the fall-through stops at its first
/// attempt, the default order, with the owner of a plain generation; its
/// options are checked before any generation.
#[test]
fn the_fallthrough_is_opt_in_and_keeps_an_owner_that_closes() {
    let nodes = custom_owner_nodes(P_FAMILIES_V1_JSON, 3).unwrap();
    let kite = &nodes.iter().find(|node| node.eps_lines == 1).unwrap();
    let presentation = &kite.presentation;
    let (plain, plain_report) =
        generate_custom_owner_with_report(presentation, &CustomOwnerOptions::default()).unwrap();
    assert_eq!(plain_report.fallthrough, None);
    let with = |fallthrough| CustomOwnerOptions {
        fallthrough: Some(fallthrough),
        ..CustomOwnerOptions::default()
    };
    let (payload, report) =
        generate_custom_owner_with_report(presentation, &with(CustomOwnerFallthrough::default()))
            .unwrap();
    assert_eq!(payload.meta, plain.meta);
    assert_eq!(
        custom_payload_dump(&payload.bytes).unwrap(),
        custom_payload_dump(&plain.bytes).unwrap()
    );
    let attempts = report.fallthrough.unwrap();
    assert_eq!(attempts.len(), 1);
    assert_eq!(
        (&attempts[0].order, &attempts[0].refusal),
        (&FallthroughOrder::Default, &None)
    );
    for (fallthrough, expected) in [
        (
            CustomOwnerFallthrough {
                seeds: vec![1, 2],
                ..CustomOwnerFallthrough::default()
            },
            "at least 3",
        ),
        (
            CustomOwnerFallthrough {
                source_orders: 2,
                ..CustomOwnerFallthrough::default()
            },
            "from 3 to 6",
        ),
        (
            CustomOwnerFallthrough {
                source_orders: 7,
                ..CustomOwnerFallthrough::default()
            },
            "from 3 to 6",
        ),
    ] {
        let error = generate_custom_owner_with_report(presentation, &with(fallthrough))
            .map(|_| ())
            .unwrap_err();
        assert!(
            matches!(&error, PError::Unsupported(text) if text.contains(expected)),
            "{error}"
        );
    }
}

/// Every order of a fall-through closes an owner that closes (t1star05):
/// three slot orders, recorded in the payload's order program, the numerator
/// before eps order (an eps owner; also recorded, with the residual keys of
/// the plain owner: masters stay at eps `n = 1`), and the six source orders,
/// with the LP and residuals of the plain owner.
#[test]
fn every_order_of_the_fallthrough_closes_an_owner_that_closes() {
    let nodes = custom_owner_nodes(P_FAMILIES_V1_JSON, 3).unwrap();
    let kite = nodes.iter().find(|node| node.eps_lines == 1).unwrap();
    let presentation = &kite.presentation;
    let options = CustomOwnerOptions::default();
    let (plain, plain_report) = generate_custom_owner_with_report(presentation, &options).unwrap();
    let all = CustomOwnerFallthrough {
        source_orders: FALLTHROUGH_SOURCE_ORDERS.len(),
        ..CustomOwnerFallthrough::default()
    };
    let orders = fallthrough_orders(presentation, &all).unwrap();
    assert_eq!(orders.len(), 1 + 3 + 1 + 6);
    assert_eq!(orders[4], FallthroughOrder::NumeratorBeforeEps);
    // After a refused chart as listed; after a default order over its
    // budget, numerator before eps first.
    assert_eq!(fallthrough_retries(&orders, false), orders[1..]);
    let over = fallthrough_retries(&orders, true);
    assert_eq!(over[0], FallthroughOrder::NumeratorBeforeEps);
    assert_eq!(over[1..4], orders[1..4]);
    assert_eq!(over[4..], orders[5..]);
    let arity = presentation.slots.len();
    let forced = presentation.forced_axes();
    for order in &orders[1..] {
        let (payload, report) =
            generate_custom_owner_on_order(presentation, &options, order).unwrap();
        assert_eq!(
            (payload.meta.lp_count, payload.meta.residuals),
            (plain.meta.lp_count, plain.meta.residuals),
            "{order}"
        );
        let ordering = match order {
            FallthroughOrder::Slots { priority, .. } => {
                slot_order_policy(mask_bits(forced, arity), priority)
                    .stable_id()
                    .to_string()
            }
            FallthroughOrder::NumeratorBeforeEps => {
                assert_eq!(report.residual_keys, plain_report.residual_keys);
                let mut descriptor =
                    OrderDescriptor::spired_uncut_two_sided(mask_bits(forced, arity));
                descriptor.degree_rows.swap(1, 2);
                OrderingPolicy::try_programmed(
                    CompiledOrder::compile(descriptor, Limits::default()).unwrap(),
                )
                .unwrap()
                .stable_id()
                .to_string()
            }
            _ => plain_report.ordering.clone(),
        };
        assert_eq!(report.ordering, ordering, "{order}");
        let generation: toml::Value = toml::from_str(&report.generation_report).unwrap();
        let rows = (generation.get("discovery_strategy")).and_then(|s| s["rows"]["kind"].as_str());
        assert_eq!(
            rows == Some("features"),
            matches!(order, FallthroughOrder::Source { .. }),
            "{order}"
        );
    }
}

// ----- T075c -----

/// A compact `I(...)` family: momentum rows over `(k_1, .., k_L, q)`, line
/// (target power 1) or ISP (0), and the eps units of each slot.
struct SymFamily {
    loops: u8,
    rows: Vec<Vec<i64>>,
    lines: Vec<bool>,
    eps_units: Vec<u8>,
}

fn momentum_row(text: &str, loops: usize) -> Vec<i64> {
    let mut row = vec![0; loops + 1];
    let mut sign = 1;
    let mut chars = text.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            '+' => sign = 1,
            '-' => sign = -1,
            'q' => row[loops] += sign,
            'k' => {
                let digits: String =
                    std::iter::from_fn(|| chars.next_if(char::is_ascii_digit)).collect();
                row[digits.parse::<usize>().unwrap() - 1] += sign;
            }
            other => panic!("momentum {text:?}: {other:?}"),
        }
    }
    row
}

fn parse_sym(text: &str) -> SymFamily {
    let loops = text
        .split("loops(")
        .nth(1)
        .and_then(|rest| rest.split(')').next())
        .unwrap()
        .split(',')
        .count();
    let mut family = SymFamily {
        loops: loops as u8,
        rows: Vec::new(),
        lines: Vec::new(),
        eps_units: Vec::new(),
    };
    for line in text.lines().map(str::trim) {
        if let Some(prop) = line.strip_prefix("prop(") {
            let (momentum, power) = prop.split_once(")^2,").unwrap();
            let momentum = momentum.split_once(",(").unwrap().1;
            family.rows.push(momentum_row(momentum, loops));
            family.lines.push(power.trim_end_matches([')', ',']) == "1");
            family.eps_units.push(0);
        } else if let Some(shift) = line.strip_prefix(",power_shift(D") {
            let (slot, shift) = shift.split_once(',').unwrap();
            assert_eq!(shift, "2-d/2)", "one eps unit");
            family.eps_units[slot.parse::<usize>().unwrap() - 1] = 1;
        }
    }
    family
}

/// The EpsNode presentation of a compact family (`q.q = -1`, convention C1).
fn presentation(name: &str, family: &SymFamily) -> Presentation {
    let slots = (family.rows.iter().zip(&family.lines).zip(&family.eps_units))
        .map(|((row, &line), &eps_units)| {
            if line {
                Slot::Line(SlotLine {
                    momentum: row.clone(),
                    eps_units,
                })
            } else {
                Slot::Isp(SlotIsp {
                    left: row.clone(),
                    right: row.clone(),
                })
            }
        })
        .collect();
    let context = CoefficientContext::try_new(["d"]).unwrap();
    Presentation::from_slots(
        PresentationId(0),
        PresentationKind::EpsNode,
        name,
        family.loops,
        slots,
        &context,
        None,
    )
    .unwrap()
}

/// `sigma[j]`: the slot of `to` with the momentum (up to sign) of slot `j`
/// of `from`.
fn slot_map(from: &SymFamily, to: &SymFamily) -> Vec<usize> {
    let negated = |row: &[i64]| row.iter().map(|c| -c).collect::<Vec<_>>();
    (from.rows.iter().zip(&from.lines))
        .map(|(row, line)| {
            (0..to.rows.len())
                .find(|&slot| {
                    to.lines[slot] == *line
                        && (to.rows[slot] == *row || to.rows[slot] == negated(row))
                })
                .unwrap()
        })
        .collect()
}

// ----- rational expressions of the rules (Symbolica's printing, table_dump) -----

#[derive(Clone, Debug)]
enum Expr {
    Number(Rational),
    Index(usize),
    D,
    Neg(Box<Expr>),
    Sum(Box<Expr>, Box<Expr>),
    Difference(Box<Expr>, Box<Expr>),
    Product(Box<Expr>, Box<Expr>),
    Quotient(Box<Expr>, Box<Expr>),
    Power(Box<Expr>, u64),
}

struct Parser<'a> {
    text: &'a [u8],
    at: usize,
    /// Slot of the index variable `n<j>`.
    slots: &'a [usize],
}

impl Parser<'_> {
    fn parse(text: &str, slots: &[usize]) -> Expr {
        let compact: String = text.chars().filter(|c| !c.is_whitespace()).collect();
        let mut parser = Parser {
            text: compact.as_bytes(),
            at: 0,
            slots,
        };
        let expr = parser.sum();
        assert_eq!(parser.at, compact.len(), "unparsed tail of {text:?}");
        expr
    }

    fn peek(&self) -> Option<u8> {
        self.text.get(self.at).copied()
    }

    fn sum(&mut self) -> Expr {
        let mut left = self.product();
        while let Some(op @ (b'+' | b'-')) = self.peek() {
            self.at += 1;
            let right = Box::new(self.product());
            left = if op == b'+' {
                Expr::Sum(Box::new(left), right)
            } else {
                Expr::Difference(Box::new(left), right)
            };
        }
        left
    }

    fn product(&mut self) -> Expr {
        let mut left = self.unary();
        while let Some(op @ (b'*' | b'/')) = self.peek() {
            self.at += 1;
            let right = Box::new(self.unary());
            left = if op == b'*' {
                Expr::Product(Box::new(left), right)
            } else {
                Expr::Quotient(Box::new(left), right)
            };
        }
        left
    }

    fn unary(&mut self) -> Expr {
        if self.peek() == Some(b'-') {
            self.at += 1;
            return Expr::Neg(Box::new(self.unary()));
        }
        let base = self.atom();
        if self.peek() != Some(b'^') {
            return base;
        }
        self.at += 1;
        let digits = self.digits();
        Expr::Power(Box::new(base), digits.parse().unwrap())
    }

    fn digits(&mut self) -> String {
        let start = self.at;
        while self.peek().is_some_and(|c| c.is_ascii_digit()) {
            self.at += 1;
        }
        String::from_utf8(self.text[start..self.at].to_vec()).unwrap()
    }

    fn atom(&mut self) -> Expr {
        match self.peek() {
            Some(b'(') => {
                self.at += 1;
                let inner = self.sum();
                assert_eq!(self.peek(), Some(b')'));
                self.at += 1;
                inner
            }
            Some(b'd') => {
                self.at += 1;
                Expr::D
            }
            Some(b'n') => {
                self.at += 1;
                Expr::Index(self.slots[self.digits().parse::<usize>().unwrap()])
            }
            Some(c) if c.is_ascii_digit() => {
                let value = (self.digits().bytes()).fold(Rational::zero(), |value, digit| {
                    &(&value * &Rational::from(10)) + &Rational::from(i64::from(digit - b'0'))
                });
                Expr::Number(value)
            }
            other => panic!(
                "unexpected {:?} at {} of {:?}",
                other.map(char::from),
                self.at,
                String::from_utf8_lossy(self.text)
            ),
        }
    }
}

impl Expr {
    /// The numerator of the coefficient vanishes at `n` for every `d` (three
    /// generic values; Symbolica prints a rational function as `num/den`).
    fn numerator_vanishes(&self, n: &[i64]) -> bool {
        let numerator = match self {
            Expr::Quotient(numerator, _) => numerator,
            other => other,
        };
        (GENERIC_DS.iter().chain([&(30_011, 6_007)])).all(|&g| {
            numerator
                .eval(n, &Rational::from(g))
                .is_some_and(|v| v.is_zero())
        })
    }

    /// The value at free indices `n` and `d`; `None` at a pole.
    fn eval(&self, n: &[i64], d: &Rational) -> Option<Rational> {
        Some(match self {
            Expr::Number(value) => value.clone(),
            Expr::Index(slot) => Rational::from(n[*slot]),
            Expr::D => d.clone(),
            Expr::Neg(inner) => -inner.eval(n, d)?,
            Expr::Sum(a, b) => &a.eval(n, d)? + &b.eval(n, d)?,
            Expr::Difference(a, b) => &a.eval(n, d)? - &b.eval(n, d)?,
            Expr::Product(a, b) => &a.eval(n, d)? * &b.eval(n, d)?,
            Expr::Quotient(a, b) => {
                let denominator = b.eval(n, d)?;
                if denominator.is_zero() {
                    return None;
                }
                &a.eval(n, d)? / &denominator
            }
            Expr::Power(base, exponent) => base.eval(n, d)?.pow(*exponent),
        })
    }
}

// ----- rule sets and their reduction -----

/// One power of a rule key: the free index plus an offset, or a number.
#[derive(Clone, Copy, Debug)]
enum Pow {
    Free(i64),
    Fixed(i64),
}

struct Rule {
    target: Vec<Pow>,
    equations: Vec<Expr>,
    exclusions: Vec<Vec<Expr>>,
    rhs: Vec<(Vec<Pow>, Expr)>,
}

/// Rules by raw chart (slots of positive power), the residual keys and the
/// order every rule descends in.
struct RuleSet {
    order: OrderingPolicy,
    charts: BTreeMap<Vec<bool>, Vec<Rule>>,
    residuals: BTreeSet<Vec<i64>>,
}

fn chart_of(key: &[i64]) -> Vec<bool> {
    key.iter().map(|&power| power >= 1).collect()
}

fn mask_bits(mask: u64, arity: usize) -> Vec<bool> {
    (0..arity).map(|slot| mask >> slot & 1 == 1).collect()
}

/// The default two-sided order on the eps axes `two_sided` with the support
/// and coordinate priorities `priority`.
fn slot_order_policy(two_sided: Vec<bool>, priority: &[usize]) -> OrderingPolicy {
    let mut descriptor = OrderDescriptor::spired_uncut_two_sided(two_sided);
    descriptor.support_priority = priority.to_vec();
    descriptor.coordinate_priority = priority.to_vec();
    OrderingPolicy::try_programmed(CompiledOrder::compile(descriptor, Limits::default()).unwrap())
        .unwrap()
}

impl Rule {
    /// The rule's free indices at `key` if it applies there.
    fn free_indices(&self, key: &[i64], d: &Rational) -> Option<Vec<i64>> {
        let mut n = vec![0; key.len()];
        for ((slot, power), &value) in self.target.iter().enumerate().zip(key) {
            match *power {
                Pow::Fixed(fixed) if fixed != value => return None,
                Pow::Fixed(_) => n[slot] = value,
                Pow::Free(offset) => n[slot] = value - offset,
            }
        }
        let vanishes = |expr: &Expr| expr.eval(&n, d).is_some_and(|value| value.is_zero());
        let excluded = (self.exclusions.iter()).any(|branch| branch.iter().all(vanishes));
        (self.equations.iter().all(vanishes) && !excluded).then_some(n)
    }
}

impl RuleSet {
    fn from_payload(bytes: &[u8], arity: usize) -> Self {
        let summary = inspect_custom_payload(bytes).unwrap();
        let identity: Vec<usize> = (0..arity).collect();
        let powers = |powers: &[CustomPower]| -> Vec<Pow> {
            (powers.iter())
                .map(|power| match *power {
                    CustomPower::Symbolic(offset) => Pow::Free(offset),
                    CustomPower::Fixed(value) => Pow::Fixed(value),
                })
                .collect()
        };
        let mut charts: BTreeMap<Vec<bool>, Vec<Rule>> = BTreeMap::new();
        for rule in custom_payload_rules(bytes).unwrap() {
            let parse = |text: &String| Parser::parse(text, &identity);
            charts
                .entry(mask_bits(rule.chart, arity))
                .or_default()
                .push(Rule {
                    target: powers(&rule.target),
                    equations: rule.equations.iter().map(parse).collect(),
                    exclusions: (rule.exclusions.iter())
                        .map(|branch| branch.iter().map(parse).collect())
                        .collect(),
                    rhs: (rule.rhs.iter())
                        .map(|(key, coefficient)| (powers(key), parse(coefficient)))
                        .collect(),
                });
        }
        RuleSet {
            order: OrderingPolicy::try_from_stable_id(&summary.ordering).unwrap(),
            charts,
            residuals: summary.residual_keys().into_iter().collect(),
        }
    }

    /// Start keys with their coefficients by point, reduced at `d` until
    /// every term is a residual or `stop` (zero-sector terms dropped). Each
    /// key is reduced once for every point that reaches it. A pole at a key
    /// voids the points that reach it (`poles`).
    fn reduce(
        &self,
        start: Vec<(Vec<i64>, Vector)>,
        d: &Rational,
        stop: &dyn Fn(&[i64]) -> bool,
        zero: &dyn Fn(&[i64]) -> bool,
        poles: &mut BTreeSet<usize>,
    ) -> BTreeMap<Vec<i64>, Vector> {
        type Frontier = BTreeMap<(ComplexityKey, Vec<i64>), Vector>;
        let push = |frontier: &mut Frontier, key: Vec<i64>, value: &Vector, factor: &Rational| {
            let complexity = self.order.complexity_key(&key).unwrap();
            add_scaled(
                frontier.entry((complexity, key)).or_default(),
                value,
                factor,
            );
        };
        let mut frontier = Frontier::new();
        for (key, value) in &start {
            push(&mut frontier, key.clone(), value, &Rational::one());
        }
        let mut out: BTreeMap<Vec<i64>, Vector> = BTreeMap::new();
        let (clock, mut reduced) = (Instant::now(), 0usize);
        while let Some(((complexity, key), mut value)) = frontier.pop_last() {
            reduced += 1;
            if reduced % 20_000 == 0 {
                eprintln!(
                    "T075c reduce at d = {d}: {reduced} keys, {} pending, {:.1} s",
                    frontier.len(),
                    clock.elapsed().as_secs_f64()
                );
            }
            value.retain(|point, coefficient| !coefficient.is_zero() && !poles.contains(point));
            if value.is_empty() || zero(&key) {
                continue;
            }
            if stop(&key) || self.residuals.contains(&key) {
                add_scaled(out.entry(key).or_default(), &value, &Rational::one());
                continue;
            }
            let rules = (self.charts.get(&chart_of(&key)))
                .unwrap_or_else(|| panic!("{key:?} is in no chart of the rule set"));
            let (rule, n) = (rules.iter())
                .find_map(|rule| rule.free_indices(&key, d).map(|n| (rule, n)))
                .unwrap_or_else(|| panic!("no rule covers {key:?}"));
            // As the M2 evaluator (twosided.rs, `DynRule::apply`): a term whose
            // numerator vanishes at `n` is absent; a pole at this `d` voids the
            // points; a pole at every `d` is a rule applied off its domain.
            let mut terms = Vec::with_capacity(rule.rhs.len());
            let mut pole = false;
            for (powers, coefficient) in &rule.rhs {
                let factor = coefficient.eval(&n, d);
                if !factor.as_ref().is_some_and(|f| !f.is_zero())
                    && coefficient.numerator_vanishes(&n)
                {
                    continue;
                }
                let Some(factor) = factor else {
                    assert!(
                        (GENERIC_DS.iter())
                            .any(|&g| coefficient.eval(&n, &Rational::from(g)).is_some()),
                        "{key:?}: a coefficient with a pole at every d"
                    );
                    pole = true;
                    break;
                };
                let child: Vec<i64> = (powers.iter().enumerate())
                    .map(|(slot, power)| match *power {
                        Pow::Free(offset) => n[slot] + offset,
                        Pow::Fixed(fixed) => fixed,
                    })
                    .collect();
                // Every step descends strictly in the rule set's order.
                assert!(
                    self.order.complexity_key(&child).unwrap() < complexity,
                    "{key:?} -> {child:?} does not descend"
                );
                if !factor.is_zero() {
                    terms.push((child, factor));
                }
            }
            if pole {
                poles.extend(value.keys());
                continue;
            }
            for (child, factor) in terms {
                push(&mut frontier, child, &value, &factor);
            }
        }
        out
    }
}

/// Coefficients of one key, by point.
type Vector = BTreeMap<usize, Rational>;

fn add_scaled(into: &mut Vector, from: &Vector, factor: &Rational) {
    for (&point, value) in from {
        let entry = into.entry(point).or_insert_with(Rational::zero);
        *entry = &*entry + &(value * factor);
    }
}

/// The terms of point `point` in `reduced`, without zeros.
fn point_terms(reduced: &BTreeMap<Vec<i64>, Vector>, point: usize) -> BTreeMap<Vec<i64>, Rational> {
    (reduced.iter())
        .filter_map(|(key, value)| {
            (value.get(&point))
                .filter(|coefficient| !coefficient.is_zero())
                .map(|coefficient| (key.clone(), coefficient.clone()))
        })
        .collect()
}

/// The RS set (table_dump.v1) in the slots of T075c (`slots[j]` is the T075c
/// slot of T075p1 slot `j`), under its order (the default two-sided order of
/// T075p1, so `slots` as priorities).
fn rs_rule_set(table: &str, slots: &[usize]) -> RuleSet {
    let arity = slots.len();
    let records: Vec<serde_json::Value> = (table.lines())
        .map(|line| serde_json::from_str(line).unwrap())
        .collect();
    let header = &records[0];
    assert_eq!(header["schema"], "table_dump.v1");
    assert_eq!(header["family_source"], T075P1, "the RS set is of T075p1");
    assert_eq!(header["order_name"], "two_sided");
    let order = &header["order"];
    let identity: Vec<usize> = (0..arity).collect();
    for priority in ["support_priority", "coordinate_priority"] {
        assert_eq!(
            serde_json::from_value::<Vec<usize>>(order[priority].clone()).unwrap(),
            identity
        );
    }
    let two_sided: Vec<bool> = serde_json::from_value(header["two_sided_axes"].clone()).unwrap();
    let end = records.last().unwrap();
    assert_eq!(
        (end["type"].as_str(), end["rules"].as_u64()),
        (Some("end"), Some(360))
    );
    assert_eq!(end["residuals"].as_u64(), Some(0));

    let mut eps = vec![false; arity];
    for (slot, &flag) in two_sided.iter().enumerate() {
        eps[slots[slot]] = flag;
    }

    let key = |text: &str| -> Vec<Pow> {
        let inner = text.strip_prefix("I(").unwrap().strip_suffix(')').unwrap();
        let mut powers = vec![Pow::Fixed(0); arity];
        for (slot, entry) in inner.split(',').enumerate() {
            powers[slots[slot]] = match entry.strip_prefix('n') {
                None => Pow::Fixed(entry.parse().unwrap()),
                Some(rest) => {
                    let digits = rest.find(['+', '-']).unwrap_or(rest.len());
                    assert_eq!(rest[..digits].parse::<usize>().unwrap(), slot);
                    Pow::Free(if digits == rest.len() {
                        0
                    } else {
                        rest[digits..].trim_start_matches('+').parse().unwrap()
                    })
                }
            };
        }
        powers
    };
    let parse = |value: &serde_json::Value| Parser::parse(value.as_str().unwrap(), slots);
    let mut charts: BTreeMap<Vec<bool>, Vec<Rule>> = BTreeMap::new();
    for record in records.iter().filter(|record| record["type"] == "rule") {
        let mut chart = vec![false; arity];
        for (slot, bit) in record["chart"].as_str().unwrap().bytes().enumerate() {
            chart[slots[slot]] = bit == b'1';
        }
        let mut target = key(record["target"].as_str().unwrap());
        for (slot, fixed) in record["fixed"].as_array().unwrap().iter().enumerate() {
            if let Some(value) = fixed.as_i64() {
                target[slots[slot]] = Pow::Fixed(value);
            }
        }
        charts.entry(chart).or_default().push(Rule {
            target,
            equations: record["equations"]
                .as_array()
                .unwrap()
                .iter()
                .map(parse)
                .collect(),
            exclusions: (record["exceptions"].as_array().unwrap().iter())
                .map(|branch| branch.as_array().unwrap().iter().map(parse).collect())
                .collect(),
            rhs: (record["rhs"].as_array().unwrap().iter())
                .map(|term| (key(term[0].as_str().unwrap()), parse(&term[1])))
                .collect(),
        });
    }
    assert_eq!(charts.values().map(Vec::len).sum::<usize>(), 360);
    RuleSet {
        order: slot_order_policy(eps, slots),
        charts,
        residuals: BTreeSet::new(),
    }
}

/// Every nonzero raw chart below the top support of `presentation`, generated
/// under `order` (the payload's); the zero raw masks of the support.
fn lower_charts(
    presentation: &Presentation,
    order: &OrderingPolicy,
) -> (RuleSet, BTreeSet<Vec<bool>>) {
    let family = &presentation.family;
    let arity = presentation.slots.len();
    let lines: Vec<usize> = (0..arity)
        .filter(|&slot| matches!(presentation.slots[slot], Slot::Line(_)))
        .collect();
    let analyzer = zero::Analyzer::try_unrestricted(family).unwrap();
    let mut zeros = BTreeSet::new();
    for bits in 0u64..1 << lines.len() {
        let mut chart = vec![false; arity];
        for (bit, &slot) in lines.iter().enumerate() {
            chart[slot] = bits >> bit & 1 == 1;
        }
        let mask = Mask::try_new(chart.iter().copied()).unwrap();
        if let zero::Decision::ProvedZero(_) = analyzer.analyze(&mask).unwrap() {
            zeros.insert(chart);
        }
    }
    let mut request = FamilyCandidatesRequest::new("T075c lower charts");
    request.n_cores = 4;
    request.nonpositive_indices = (0..arity).filter(|slot| !lines.contains(slot)).collect();
    request.two_sided_eps_axes = true;
    request.integral_order = order.program().cloned();
    request.sector_failures = SectorFailurePolicy::Report;
    let (session, job) =
        CandidateGenerationSession::from_family(Arc::clone(family), request, 1024).unwrap();
    job.run();
    let generated = session.result().unwrap();
    assert!(
        !generated.to_toml().contains("[[failed_sectors]]"),
        "{}",
        generated.to_toml()
    );
    let mut set = RuleSet::from_payload(generated.bundle(), arity);
    let forced = presentation.forced_axes();
    let top =
        |chart: &Vec<bool>| (lines.iter()).all(|&slot| chart[slot] || forced >> slot & 1 == 1);
    set.charts.retain(|chart, _| !top(chart));
    (set, zeros)
}

/// A deterministic random key of the top support: lines at `1 +` dots (at
/// most one), the eps line in `-2..=3`, ISPs at `-` rank (at most one): 324
/// keys. (Two dots and rank two make the exact pass of the lower charts
/// exceed 30 GB and an hour, run-t075c-5.)
fn random_key(state: &mut u64, presentation: &Presentation) -> Vec<i64> {
    let mut next = |bound: usize| {
        *state = state
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1_442_695_040_888_963_407);
        (*state >> 33) as usize % bound
    };
    let forced = presentation.forced_axes();
    let mut key = vec![0; presentation.slots.len()];
    let (mut lines, mut isps) = (Vec::new(), Vec::new());
    for (slot, kind) in presentation.slots.iter().enumerate() {
        match kind {
            Slot::Line(_) if forced >> slot & 1 == 1 => key[slot] = next(6) as i64 - 2,
            Slot::Line(_) => {
                key[slot] = 1;
                lines.push(slot);
            }
            Slot::Isp(_) => isps.push(slot),
        }
    }
    for _ in 0..next(2) {
        key[lines[next(lines.len())]] += 1;
    }
    for _ in 0..next(2) {
        key[isps[next(isps.len())]] -= 1;
    }
    key
}

/// G2's T075c item: closed through the fall-through at 0 residuals = LP 0,
/// the refusal of the default order and the winning order recorded, and
/// equal to the VERIFIED_RS T075p1 set at 200 random points, each at two or
/// more values of `d` (a point with a pole at every value fails).
#[test]
#[ignore = "slow: T075c through the fall-through and 200 points against the RS set, 35 min and 21 GB optimised on 4 CPUs; needs RUSTRED_T075P1_RS_TABLE"]
fn t075c_closes_through_the_fallthrough_and_equals_the_rs_set() {
    let rs_path = std::env::var(RS_TABLE_ENV)
        .unwrap_or_else(|_| panic!("{RS_TABLE_ENV}: the path of the T075p1 RS set"));
    let (c, p1) = (parse_sym(T075C), parse_sym(T075P1));
    let slots = slot_map(&p1, &c);
    assert_eq!(slots, T075P1_SLOTS);
    let presentation = presentation("T075c", &c);
    let arity = presentation.slots.len();

    // Each attempt under a budget (M3-I1), as generation runs in M3.
    let options = CustomOwnerOptions {
        n_cores: 2,
        budget: GenerationBudget {
            max_seconds: Some(900.0),
            max_rss_bytes: Some(8 << 30),
        },
        fallthrough: Some(CustomOwnerFallthrough::default()),
        ..CustomOwnerOptions::default()
    };
    let (payload, report) = generate_custom_owner_with_report(&presentation, &options).unwrap();
    let attempts = report.fallthrough.clone().unwrap();
    for (index, attempt) in attempts.iter().enumerate() {
        eprintln!(
            "T075c attempt {index}, {}, {:.1} s: {}",
            attempt.order,
            attempt.seconds,
            attempt.refusal.as_deref().unwrap_or("closed")
        );
    }
    eprintln!(
        "T075c: {} rules, charts {:?}, LP {} ({:.1} s), payload {} bytes, order {}",
        report.rules,
        (report.charts.iter())
            .map(|chart| (chart.chart, chart.rules))
            .collect::<Vec<_>>(),
        report.lp_count,
        report.lp_seconds,
        report.payload_bytes,
        report.ordering
    );
    let refusal = attempts[0].refusal.as_deref().unwrap();
    assert_eq!(attempts[0].order, FallthroughOrder::Default);
    assert!(
        refusal.contains("chart 11111111100000: unsupported-nonlinear-equalities")
            && refusal.contains("unsupported nonlinear equalities"),
        "{refusal}"
    );
    let winner = attempts.last().unwrap();
    assert!(winner.refusal.is_none() && attempts.len() >= 2);
    // The payload records the winning slot order: its order program.
    let FallthroughOrder::Slots { priority, .. } = &winner.order else {
        panic!("T075c closed on {}", winner.order);
    };
    let forced = presentation.forced_axes();
    let order = slot_order_policy(mask_bits(forced, arity), priority);
    assert_eq!(order.stable_id().to_string(), report.ordering);
    assert_eq!((report.lp_count, report.lp_given), (0, false));
    assert!(report.residual_keys.is_empty() && payload.meta.residuals == 0);
    assert_eq!(payload.meta.charts.len(), 2);

    let ours = RuleSet::from_payload(&payload.bytes, arity);
    let table = std::fs::read_to_string(&rs_path).unwrap();
    let rs = rs_rule_set(&table, &slots);
    let (lower, zeros) = lower_charts(&presentation, &ours.order);
    let top_lines: Vec<usize> = (0..arity)
        .filter(|&slot| {
            matches!(presentation.slots[slot], Slot::Line(_)) && forced >> slot & 1 == 0
        })
        .collect();
    let exit = |key: &[i64]| top_lines.iter().any(|&slot| key[slot] <= 0);
    let zero = |key: &[i64]| zeros.contains(&chart_of(key));
    let never = |_: &[i64]| false;

    // Prime denominators near 100: no small linear factor of a coefficient
    // vanishes there (37/10 voided 68 of the 200 points in a first run).
    let ds = [
        Rational::from((373, 101)),
        Rational::from((421, 103)),
        Rational::from((397, 107)),
    ];
    let mut state = 0x7075_c0de_u64;
    let mut keys = Vec::new();
    while keys.len() < 200 {
        let key = random_key(&mut state, &presentation);
        if !keys.contains(&key) {
            keys.push(key);
        }
    }
    let points = keys.len();
    let unit = |point: usize| BTreeMap::from([(point, Rational::one())]);
    // One value of d: the points without a pole, each with its masters
    // nonzero or not and its exits equal or not.
    let compare = |d: &Rational| -> Vec<(usize, Option<bool>, bool)> {
        let clock = Instant::now();
        let mut poles = BTreeSet::new();
        let starts: Vec<_> = (keys.iter().enumerate())
            .map(|(point, key)| (key.clone(), unit(point)))
            .collect();
        let ours_exits = ours.reduce(starts.clone(), d, &exit, &zero, &mut poles);
        let mut rs_exits = rs.reduce(starts, d, &exit, &zero, &mut poles);
        let reduced = clock.elapsed().as_secs_f64();
        // Convention q.q = +1 of the RS set -> q.q = -1.
        for (e, value) in &mut rs_exits {
            let total: i64 = e.iter().sum();
            for (&point, coefficient) in value.iter_mut() {
                if (total - keys[point].iter().sum::<i64>()) % 2 != 0 {
                    *coefficient = -coefficient.clone();
                }
            }
        }
        // One pass of the lower charts: component p is ours minus RS of point
        // p (it must vanish), component `points + p` ours alone for the first
        // SAMPLE points (they must not all vanish).
        let mut lower_start: BTreeMap<Vec<i64>, Vector> = ours_exits.clone();
        for (e, value) in &rs_exits {
            add_scaled(
                lower_start.entry(e.clone()).or_default(),
                value,
                &-Rational::one(),
            );
        }
        let same: Vec<bool> = (0..points)
            .map(|point| point_terms(&lower_start, point).is_empty())
            .collect();
        for (e, value) in &ours_exits {
            let sample: Vector = (value.range(..SAMPLE))
                .map(|(&p, c)| (points + p, c.clone()))
                .collect();
            add_scaled(
                lower_start.entry(e.clone()).or_default(),
                &sample,
                &Rational::one(),
            );
        }
        let mut lower_poles = BTreeSet::new();
        let masters = lower.reduce(
            lower_start.into_iter().collect(),
            d,
            &never,
            &zero,
            &mut lower_poles,
        );
        poles.extend(lower_poles.iter().map(|index| index % points));
        let mut compared = Vec::new();
        for point in (0..points).filter(|point| !poles.contains(point)) {
            let difference = point_terms(&masters, point);
            assert!(
                difference.is_empty(),
                "T075c {:?} at d = {d}: ours - RS = {difference:?}",
                keys[point]
            );
            let value = (point < SAMPLE).then(|| !point_terms(&masters, points + point).is_empty());
            compared.push((point, value, same[point]));
        }
        eprintln!(
            "T075c d = {d}: {} exits ours, {} RS, {} lower keys at the masters, {} points with a pole; {reduced:.1} s + {:.1} s",
            ours_exits.len(),
            rs_exits.len(),
            masters.len(),
            poles.len(),
            clock.elapsed().as_secs_f64() - reduced
        );
        compared
    };
    // The values of d side by side, one thread each.
    let compared: Vec<_> = std::thread::scope(|scope| {
        let threads: Vec<_> = (ds.iter()).map(|d| scope.spawn(|| compare(d))).collect();
        threads.into_iter().map(|t| t.join().unwrap()).collect()
    });
    let mut pole_free = vec![0; points];
    let (mut evaluations, mut sampled, mut nonzero, mut same_exits) = (0, 0, 0, 0);
    for &(point, value, same) in compared.iter().flatten() {
        pole_free[point] += 1;
        evaluations += 1;
        sampled += usize::from(value.is_some());
        nonzero += usize::from(value == Some(true));
        same_exits += usize::from(same);
    }
    for (key, &at) in keys.iter().zip(&pole_free) {
        assert!(
            at >= 2,
            "T075c {key:?}: poles at {} of {} values of d",
            ds.len() - at,
            ds.len()
        );
    }
    eprintln!(
        "T075c vs RS T075p1: {points} points, {evaluations} evaluations equal ({same_exits} with the same exits; {nonzero} of {sampled} sampled values nonzero)"
    );
    assert!(nonzero * 2 > sampled, "most values are nonzero");
}
