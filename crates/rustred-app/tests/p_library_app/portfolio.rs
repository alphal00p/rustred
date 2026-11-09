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

use rustred::algebra::CoefficientContext;
use rustred::order::{CompiledOrder, Limits, OrderDescriptor};
use rustred::propagator::PresentationId;
use rustred::propagator::custom::{
    CustomPower, custom_payload_dump, custom_payload_rules, inspect_custom_payload,
};
use rustred::propagator::error::PError;
use rustred::propagator::families::P_FAMILIES_V1_JSON;
use rustred::propagator::presentation::{
    Presentation, PresentationKind, Slot, SlotIsp, SlotLine,
};
use rustred::sector::{ComplexityKey, Mask, OrderingPolicy, zero};
use rustred_app::propagator::generate::{
    CustomOwnerFallthrough, CustomOwnerOptions, FallthroughOrder, custom_owner_nodes,
    generate_custom_owner_with_report,
};
use rustred_app::{CandidateGenerationSession, FamilyCandidatesRequest, SectorFailurePolicy};
use symbolica::domains::rational::Rational;

const T075C: &str = include_str!("t075/T075c.sym");
const T075P1: &str = include_str!("t075/T075p1.sym");
/// Astra's slot map of T075p1 onto T075c (`t075-rs-promotion.json` RS6).
const T075P1_SLOTS: [usize; 14] = [3, 5, 1, 4, 0, 8, 2, 6, 7, 12, 13, 11, 9, 10];
/// The RS set: `rules_study/ko4/T075/ko4-dump/OUT.table.jsonl` (table_dump.v1).
const RS_TABLE_ENV: &str = "RUSTRED_T075P1_RS_TABLE";

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
        let mut parser = Parser {
            text: text.as_bytes(),
            at: 0,
            slots,
        };
        let expr = parser.sum();
        assert_eq!(parser.at, text.len(), "unparsed tail of {text:?}");
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

fn order_policy(descriptor: OrderDescriptor) -> OrderingPolicy {
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

    /// `key` times `coefficient`, reduced at `d` until every term is a
    /// residual or `stop` (zero-sector terms dropped); `None` at a pole.
    fn reduce(
        &self,
        start: &BTreeMap<Vec<i64>, Rational>,
        d: &Rational,
        stop: &dyn Fn(&[i64]) -> bool,
        zero: &dyn Fn(&[i64]) -> bool,
    ) -> Option<BTreeMap<Vec<i64>, Rational>> {
        type Frontier = BTreeMap<(ComplexityKey, Vec<i64>), Rational>;
        let push = |frontier: &mut Frontier, key: Vec<i64>, value: Rational| {
            let complexity = self.order.complexity_key(&key).unwrap();
            let entry = frontier.entry((complexity, key)).or_insert_with(Rational::zero);
            *entry = &*entry + &value;
        };
        let mut frontier = Frontier::new();
        for (key, value) in start {
            push(&mut frontier, key.clone(), value.clone());
        }
        let mut out = BTreeMap::new();
        while let Some(((_, key), value)) = frontier.pop_last() {
            if value.is_zero() || zero(&key) {
                continue;
            }
            if stop(&key) || self.residuals.contains(&key) {
                let entry = out.entry(key).or_insert_with(Rational::zero);
                *entry = &*entry + &value;
                continue;
            }
            let rules = (self.charts.get(&chart_of(&key)))
                .unwrap_or_else(|| panic!("{key:?} is in no chart of the rule set"));
            let (rule, n) = (rules.iter())
                .find_map(|rule| rule.free_indices(&key, d).map(|n| (rule, n)))
                .unwrap_or_else(|| panic!("no rule covers {key:?}"));
            for (powers, coefficient) in &rule.rhs {
                let child: Vec<i64> = (powers.iter().enumerate())
                    .map(|(slot, power)| match *power {
                        Pow::Free(offset) => n[slot] + offset,
                        Pow::Fixed(fixed) => fixed,
                    })
                    .collect();
                let factor = coefficient.eval(&n, d)?;
                if !factor.is_zero() {
                    push(&mut frontier, child, &value * &factor);
                }
            }
        }
        out.retain(|_, value: &mut Rational| !value.is_zero());
        Some(out)
    }
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
    assert_eq!((end["type"].as_str(), end["rules"].as_u64()), (Some("end"), Some(360)));
    assert_eq!(end["residuals"].as_u64(), Some(0));

    let mut eps = vec![false; arity];
    for (slot, &flag) in two_sided.iter().enumerate() {
        eps[slots[slot]] = flag;
    }
    let mut descriptor = OrderDescriptor::spired_uncut_two_sided(eps);
    descriptor.support_priority = slots.to_vec();
    descriptor.coordinate_priority = slots.to_vec();

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
            equations: record["equations"].as_array().unwrap().iter().map(parse).collect(),
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
        order: order_policy(descriptor),
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
    let top = |chart: &Vec<bool>| (lines.iter()).all(|&slot| chart[slot] || forced >> slot & 1 == 1);
    set.charts.retain(|chart, _| !top(chart));
    (set, zeros)
}

/// A deterministic random key of the top support: lines at `1 +` dots (at
/// most 2 in total), the eps line in `-2..=3`, ISPs at `-` rank (at most 2).
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
    for _ in 0..next(3) {
        key[lines[next(lines.len())]] += 1;
    }
    for _ in 0..next(3) {
        key[isps[next(isps.len())]] -= 1;
    }
    key
}

/// G2's T075c item: closed through the fall-through at 0 residuals = LP 0,
/// the refusal of the default order and the winning order recorded, and
/// equal to the VERIFIED_RS T075p1 set at 200 random points, each at two or
/// more values of `d` (a point with a pole at every value fails).
#[test]
#[ignore = "slow: T075c through the fall-through and 200 points against the RS set, minutes optimised; needs RUSTRED_T075P1_RS_TABLE"]
fn t075c_closes_through_the_fallthrough_and_equals_the_rs_set() {
    let rs_path = std::env::var(RS_TABLE_ENV)
        .unwrap_or_else(|_| panic!("{RS_TABLE_ENV}: the path of the T075p1 RS set"));
    let (c, p1) = (parse_sym(T075C), parse_sym(T075P1));
    let slots = slot_map(&p1, &c);
    assert_eq!(slots, T075P1_SLOTS);
    let presentation = presentation("T075c", &c);
    let arity = presentation.slots.len();

    let options = CustomOwnerOptions {
        n_cores: 2,
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
    let provenance = format!(
        "custom owner; L2 fall-through attempt {}: {}",
        attempts.len() - 1,
        winner.order
    );
    assert!(
        (payload.bytes.windows(provenance.len())).any(|window| window == provenance.as_bytes()),
        "{provenance}"
    );
    assert_eq!((report.lp_count, report.lp_given), (0, false));
    assert!(report.residual_keys.is_empty() && payload.meta.residuals == 0);
    assert_eq!(payload.meta.charts.len(), 2);

    let ours = RuleSet::from_payload(&payload.bytes, arity);
    let table = std::fs::read_to_string(&rs_path).unwrap();
    let rs = rs_rule_set(&table, &slots);
    let (lower, zeros) = lower_charts(&presentation, &ours.order);
    let forced = presentation.forced_axes();
    let top_lines: Vec<usize> = (0..arity)
        .filter(|&slot| matches!(presentation.slots[slot], Slot::Line(_)) && forced >> slot & 1 == 0)
        .collect();
    let exit = |key: &[i64]| top_lines.iter().any(|&slot| key[slot] <= 0);
    let zero = |key: &[i64]| zeros.contains(&chart_of(key));
    let never = |_: &[i64]| false;

    let ds = [
        Rational::from((37, 10)),
        Rational::from((29, 7)),
        Rational::from((53, 13)),
    ];
    let mut state = 0x7075_c0de_u64;
    let mut seen = BTreeSet::new();
    let (mut points, mut evaluations, mut poles, mut nonzero, mut same_exits) = (0, 0, 0, 0, 0);
    while points < 200 {
        let key = random_key(&mut state, &presentation);
        if !seen.insert(key.clone()) {
            continue;
        }
        let start = BTreeMap::from([(key.clone(), Rational::one())]);
        let total: i64 = key.iter().sum();
        let mut at = 0;
        for d in &ds {
            let ours_exits = ours.reduce(&start, d, &exit, &zero);
            let rs_exits = rs.reduce(&start, d, &exit, &zero).map(|exits| {
                (exits.into_iter())
                    .map(|(e, value)| {
                        let odd = (e.iter().sum::<i64>() - total) % 2 != 0;
                        let value = if odd { -value } else { value };
                        (e, value)
                    })
                    .collect::<BTreeMap<_, _>>()
            });
            let (Some(ours_exits), Some(rs_exits)) = (ours_exits, rs_exits) else {
                poles += 1;
                continue;
            };
            let a = lower.reduce(&ours_exits, d, &never, &zero);
            let b = lower.reduce(&rs_exits, d, &never, &zero);
            let (Some(a), Some(b)) = (a, b) else {
                poles += 1;
                continue;
            };
            assert_eq!(a, b, "T075c {key:?} at d = {d}");
            same_exits += usize::from(ours_exits == rs_exits);
            nonzero += usize::from(!a.is_empty());
            evaluations += 1;
            at += 1;
        }
        assert!(at >= 2, "T075c {key:?}: poles at {} of {} values of d", 3 - at, ds.len());
        points += 1;
    }
    eprintln!(
        "T075c vs RS T075p1: {points} points, {evaluations} evaluations equal ({same_exits} with the same exits, {nonzero} nonzero), {poles} poles"
    );
    assert!(nonzero * 2 > evaluations, "most values are nonzero");
}
