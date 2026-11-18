//! helper-vh2: node-level t-adic reduction of ONE candidate table (no package).
//!
//! [`Table::reduce_tadic`] applies the table's rules exactly as [`Table::reduce_mod`] does (same rule choice, same
//! graph, same stop at the node's masters and at keys outside the node's sector = the node's exits), but carries
//! every weight as a truncated Laurent series in `t = d - d0` modulo a 62/63-bit prime, with a planned absolute
//! precision:
//!
//! * every edge coefficient `c(n, d)` of a used rule is turned into an EXACT rational function of `d` at the rule
//!   point (Symbolica, gcd-normalised); its valuation `v` at `d0` is computed exactly over Q
//!   ([`coefficient_valuation`]);
//! * a backward pass over the reduction DAG (children before parents) fixes the absolute precision each key needs:
//!   `need(terminal) = order`, `need(inner) = max over its edges (need(child) - v(edge))`;
//! * the forward pass (parents before children) computes every value to exactly its need: the outputs reach `order`
//!   by construction. A value whose known coefficients vanish modulo `p` is `O(t^P)`, never zero;
//! * a prime is unusable when it divides the leading Taylor coefficient of an edge denominator (the expansion is
//!   then not p-integral); the next prime is drawn.
//!
//! The DAG has no recorded erasures (no zero decision on a weight). Its only zero decisions are the rule terms whose
//! integral lies outside the family support (an ISP in a denominator): `reduce_mod` drops them after testing the
//! coefficient with its exact-in-d test (`Expr::vanishes_in_d`); here each dropped coefficient is re-checked EXACTLY zero over Q(d)
//! at the rule point (else the request fails). Rule applicability (case equations, exception conjunctions) is the
//! table's responsibility and is decided by the same `find_rule` as `reduce_mod`.
//!
//! The graph walk is a copy of the first half of `reduce_mod` (kept separate so that the gated numeric path stays
//! byte-for-byte unchanged).
use std::collections::{BTreeMap, HashMap, HashSet};
use std::hash::{DefaultHasher, Hash, Hasher};

use rand::{Rng, SeedableRng, rngs::StdRng};
use serde_json::{Value, json};
use symbolica::domains::finite_field::PrimeIteratorU64;

use super::super::replay::arith;
use super::super::replay::expr::{Expr, Node};
use super::{Key, Table, at, key_text};
use crate::algebra::{Coefficient, CoefficientContext, ExactAlgebraLimits};
use crate::reduction::ordered_reduction::coefficient_valuation;
use crate::reduction::{TadicSeries, expand_coefficient};
/// (lowest valuation, edges, precision losses) of one rule over the live edges.
type RuleStat = (i32, usize, usize);
/// (prime, per seed: (terminal, series)) of one replayed prime.
type PrimeImage = (u64, Vec<Vec<(u32, TadicSeries)>>);

/// Schema of the JSON document written by `rr_node tadic`.
pub const NODE_TADIC_SCHEMA: &str = "rustred.node-tadic.v1";

/// Need of a key that no output reads.
const DEAD: i32 = i32::MIN / 4;

/// Node-level t-adic primes: `[2^62 + 2^61 + 2^60 + 2^59, ...)`, disjoint from the package-level t-adic range
/// (`[2^62 + 2^61 + 2^60, + 2^59)`) and from the 61-bit replay primes.
const NODE_TADIC_BASE: u64 = (1 << 62) + (1 << 61) + (1 << 60) + (1 << 59);
const NODE_TADIC_SALT: u64 = 0x6e6f_6465_7461_6469;

/// Options of [`Table::reduce_tadic`].
#[derive(Clone, Debug)]
pub struct NodeTadicOptions {
    /// Expansion point `d0` (integer).
    pub d0: i64,
    /// Target absolute precision of every output (master and exit) series.
    pub order: i32,
    /// Usable primes wanted.
    pub primes: usize,
    /// Primes that may be consumed (usable or not) before the request fails.
    pub max_primes: usize,
    /// Seed of the prime stream.
    pub seed: u64,
    /// Longest series (coefficients) any key may need; a longer plan fails closed.
    pub max_length: usize,
    /// Output the exits (keys outside the node's sector) as well as the masters.
    pub keep_lower: bool,
}

impl Default for NodeTadicOptions {
    fn default() -> Self {
        Self {
            d0: 4,
            order: 1,
            primes: 2,
            max_primes: 8,
            seed: 0,
            max_length: 4096,
            keep_lower: true,
        }
    }
}

/// One request: `sum_k weight_k I(key_k)` (integer weights, constant in d).
#[derive(Clone, Debug)]
pub struct NodeRequest {
    pub label: String,
    pub inputs: Vec<(Key, i64)>,
}

/// Typed error prefix (the text before the first ':').
fn typed(code: &str, detail: impl std::fmt::Display) -> String {
    format!("{code}: {detail}")
}

/// Exact value of a rule-coefficient expression at the integer point `pt` as a rational function of `d`.
fn exact(
    node: &Node,
    n: usize,
    pt: &[i64],
    ctx: &CoefficientContext,
    d: &Coefficient,
) -> Result<Coefficient, String> {
    let limits = ExactAlgebraLimits::default();
    let err = |e: crate::algebra::ExactAlgebraError| typed("TADIC_EXACT_ALGEBRA", e);
    Ok(match node {
        Node::Num(t) => {
            // arbitrary length: base 10^18 chunks
            let mut acc = ctx.zero();
            let base = ctx.unsigned_integer(1_000_000_000_000_000_000);
            let bytes = t.as_bytes();
            let head = bytes.len() % 18;
            let mut chunks: Vec<&[u8]> = vec![];
            if head > 0 {
                chunks.push(&bytes[..head]);
            }
            let mut i = head;
            while i < bytes.len() {
                chunks.push(&bytes[i..i + 18]);
                i += 18;
            }
            for chunk in chunks {
                let value: u128 = std::str::from_utf8(chunk)
                    .ok()
                    .and_then(|s| s.parse().ok())
                    .ok_or_else(|| typed("TADIC_TABLE_EXPRESSION", format!("number {t}")))?;
                acc = ctx.try_mul(&acc, &base, limits).map_err(err)?;
                acc = ctx
                    .try_add(&acc, &ctx.unsigned_integer(value), limits)
                    .map_err(err)?;
            }
            acc
        }
        Node::Var(v) => {
            if *v == n {
                d.clone()
            } else {
                ctx.integer(pt[*v])
            }
        }
        Node::Neg(a) => ctx
            .try_neg(&exact(a, n, pt, ctx, d)?, limits)
            .map_err(err)?,
        Node::Sum(ts) => {
            let mut acc = ctx.zero();
            for (neg, t) in ts {
                let x = exact(t, n, pt, ctx, d)?;
                acc = if *neg {
                    ctx.try_sub(&acc, &x, limits)
                } else {
                    ctx.try_add(&acc, &x, limits)
                }
                .map_err(err)?;
            }
            acc
        }
        Node::Prod(fs) => {
            let mut acc = ctx.one();
            for (op, f) in fs {
                let x = exact(f, n, pt, ctx, d)?;
                acc = if *op == '/' {
                    if x.numerator.is_zero() {
                        return Err(typed(
                            "TADIC_RULE_POLE",
                            "a rule coefficient divides by an expression that vanishes identically in d at the rule point",
                        ));
                    }
                    ctx.try_div(&acc, &x, limits)
                } else {
                    ctx.try_mul(&acc, &x, limits)
                }
                .map_err(err)?;
            }
            acc
        }
        Node::Bin(op, a, b) => {
            let x = exact(a, n, pt, ctx, d)?;
            if *op == '^' {
                let e: i64 = match &**b {
                    Node::Num(t) => t
                        .parse()
                        .map_err(|_| typed("TADIC_TABLE_EXPRESSION", "exponent"))?,
                    Node::Neg(bb) => match &**bb {
                        Node::Num(t) => -t
                            .parse::<i64>()
                            .map_err(|_| typed("TADIC_TABLE_EXPRESSION", "exponent"))?,
                        _ => return Err(typed("TADIC_TABLE_EXPRESSION", "exponent")),
                    },
                    _ => return Err(typed("TADIC_TABLE_EXPRESSION", "exponent")),
                };
                let mut acc = ctx.one();
                for _ in 0..e.unsigned_abs() {
                    acc = ctx.try_mul(&acc, &x, limits).map_err(err)?;
                }
                if e < 0 {
                    if acc.numerator.is_zero() {
                        return Err(typed(
                            "TADIC_RULE_POLE",
                            "a negative power of an expression that vanishes identically in d at the rule point",
                        ));
                    }
                    acc = ctx.try_div(&ctx.one(), &acc, limits).map_err(err)?;
                }
                acc
            } else {
                let y = exact(b, n, pt, ctx, d)?;
                match op {
                    '+' => ctx.try_add(&x, &y, limits),
                    '-' => ctx.try_sub(&x, &y, limits),
                    '*' => ctx.try_mul(&x, &y, limits),
                    '/' => {
                        if y.numerator.is_zero() {
                            return Err(typed(
                                "TADIC_RULE_POLE",
                                "a rule coefficient divides by an expression that vanishes identically in d",
                            ));
                        }
                        ctx.try_div(&x, &y, limits)
                    }
                    _ => return Err(typed("TADIC_TABLE_EXPRESSION", format!("operator {op}"))),
                }
                .map_err(err)?
            }
        }
    })
}

/// `O(t^precision)`.
fn known_zero(precision: i32) -> TadicSeries {
    TadicSeries {
        start: precision,
        coefficients: vec![],
    }
}

fn strip(mut s: TadicSeries) -> TadicSeries {
    let lead = s.coefficients.iter().take_while(|c| **c == 0).count();
    if lead > 0 {
        s.coefficients.drain(..lead);
        s.start += lead as i32;
    }
    s
}

fn truncate(s: &TadicSeries, precision: i32) -> TadicSeries {
    if precision >= s.precision() {
        return s.clone();
    }
    if precision <= s.start {
        return known_zero(precision);
    }
    TadicSeries {
        start: s.start,
        coefficients: s.coefficients[..(precision - s.start) as usize].to_vec(),
    }
}

/// `a + b`, precision min.
fn add(p: u64, a: &TadicSeries, b: &TadicSeries) -> TadicSeries {
    let precision = a.precision().min(b.precision());
    let start = a.start.min(b.start);
    if precision <= start {
        return known_zero(precision);
    }
    let mut sum = vec![0u64; (precision - start) as usize];
    for s in [a, b] {
        for (k, c) in s.coefficients.iter().enumerate() {
            let order = s.start + k as i32;
            if order >= precision {
                break;
            }
            let slot = &mut sum[(order - start) as usize];
            *slot = arith::add(p, *slot, *c);
        }
    }
    strip(TadicSeries {
        start,
        coefficients: sum,
    })
}

/// `a * c` to absolute precision `precision`; `c_valuation` is the EXACT valuation of `c` over Q (a lower bound of
/// its order even when its expansion is only `O(t^P)` with `P <= c_valuation`). The caller guarantees
/// `a.precision() + c_valuation >= precision` and `c.precision() + a.start >= precision`; checked.
fn mul_to(
    p: u64,
    a: &TadicSeries,
    c: &TadicSeries,
    c_valuation: i32,
    precision: i32,
) -> Result<TadicSeries, String> {
    let c_low = c.start.max(c_valuation);
    if a.coefficients.is_empty() || c.coefficients.is_empty() {
        // a = O(t^Pa): the product is O(t^(Pa + v(c))) whatever c's precision; c = O(t^Pc) with a known: the
        // product is O(t^min(a.start + Pc, Pa + v(c))).
        let available = if a.coefficients.is_empty() {
            a.precision() + c_low
        } else {
            (a.precision() + c_low).min(c.precision() + a.start)
        };
        if available < precision {
            return Err(typed(
                "TADIC_PRECISION_FAILURE",
                format!(
                    "a product needs precision {precision}, its factors give {available} (inconsistent plan)"
                ),
            ));
        }
        return Ok(known_zero(precision));
    }
    let available = (a.precision() + c_low).min(c.precision() + a.start);
    if available < precision {
        return Err(typed(
            "TADIC_PRECISION_FAILURE",
            format!(
                "a product needs precision {precision}, its factors give {available} (inconsistent plan)"
            ),
        ));
    }
    let start = a.start + c.start;
    if precision <= start {
        return Ok(known_zero(precision));
    }
    let len = (precision - start) as usize;
    let mut out = vec![0u64; len];
    for (i, x) in a.coefficients.iter().enumerate().take(len) {
        if *x == 0 {
            continue;
        }
        for (j, y) in c.coefficients.iter().enumerate().take(len - i) {
            out[i + j] = arith::add(p, out[i + j], arith::mul(p, *x, *y));
        }
    }
    Ok(strip(TadicSeries {
        start,
        coefficients: out,
    }))
}

/// One used edge of the reduction DAG.
struct Edge {
    child: u32,
    valuation: i32,
    coefficient: Coefficient,
}

impl Table {
    /// Reduce the weighted `requests` with this table alone (see the module documentation). Returns the
    /// `rustred.node-tadic.v1` document. Errors are `CODE: detail` strings with a `TADIC_*` code.
    pub fn reduce_tadic(
        &mut self,
        requests: &[NodeRequest],
        opt: &NodeTadicOptions,
    ) -> Result<Value, String> {
        let t0 = std::time::Instant::now();
        let n = self.n;
        if requests.is_empty() || requests.iter().any(|r| r.inputs.is_empty()) {
            return Err(typed("TADIC_JOB_INVALID", "an empty request"));
        }
        if opt.primes == 0 || opt.max_primes < opt.primes {
            return Err(typed(
                "TADIC_JOB_INVALID",
                "primes must be >= 1 and <= max_primes",
            ));
        }
        let masters: HashSet<Key> = self.masters.iter().cloned().collect();
        // ---- graph (copy of reduce_mod's walk); kind 0 inner, 1 master, 2 lower (exit)
        let mut id: HashMap<Key, u32> = HashMap::new();
        let mut nodes: Vec<Key> = vec![];
        let mut kind: Vec<u8> = vec![];
        let mut rule_of: Vec<(u32, Vec<i64>)> = vec![];
        let mut kids: Vec<Vec<u32>> = vec![];
        let mut kid_term: Vec<Vec<usize>> = vec![];
        let mut state: Vec<u8> = vec![];
        let mut post: Vec<u32> = vec![];
        let zero_sector = |t: &Table, k: &Key| {
            t.zero
                .contains(&k.iter().map(|&x| x > 0).collect::<Vec<bool>>())
        };
        let mut intern = |t: &Table,
                          k: Key,
                          nodes: &mut Vec<Key>,
                          kind: &mut Vec<u8>,
                          rule_of: &mut Vec<(u32, Vec<i64>)>,
                          kids: &mut Vec<Vec<u32>>,
                          kid_term: &mut Vec<Vec<usize>>,
                          state: &mut Vec<u8>|
         -> Result<Option<u32>, String> {
            if let Some(&i) = id.get(&k) {
                return Ok(Some(i));
            }
            let kd = if t
                .in_sector(&k)
                .map_err(|e| typed("TADIC_TABLE_INVALID", e))?
            {
                u8::from(masters.contains(&k))
            } else {
                if zero_sector(t, &k) {
                    return Ok(None);
                }
                2
            };
            let i = nodes.len() as u32;
            id.insert(k.clone(), i);
            nodes.push(k);
            kind.push(kd);
            rule_of.push((u32::MAX, vec![]));
            kids.push(vec![]);
            kid_term.push(vec![]);
            state.push(0);
            Ok(Some(i))
        };
        // distinct seeds in first-seen order
        let mut seed_ids: Vec<Option<u32>> = vec![];
        let mut seed_index: HashMap<Key, usize> = HashMap::new();
        for r in requests {
            for (k, _) in &r.inputs {
                if k.len() != n {
                    return Err(typed(
                        "TADIC_JOB_INVALID",
                        format!("key {} has {} indices, the table {n}", key_text(k), k.len()),
                    ));
                }
                if seed_index.contains_key(k) {
                    continue;
                }
                seed_index.insert(k.clone(), seed_ids.len());
                seed_ids.push(intern(
                    self,
                    k.clone(),
                    &mut nodes,
                    &mut kind,
                    &mut rule_of,
                    &mut kids,
                    &mut kid_term,
                    &mut state,
                )?);
            }
        }
        let mut uncovered: Vec<Key> = vec![];
        let mut vanishing: Vec<(u32, usize, Vec<i64>)> = vec![];
        for &r in seed_ids.iter().flatten() {
            let mut stack: Vec<(u32, usize)> = vec![(r, 0)];
            while let Some((u, ci)) = stack.pop() {
                let ui = u as usize;
                if ci == 0 {
                    if state[ui] == 2 {
                        continue;
                    }
                    if state[ui] == 1 {
                        return Err(typed(
                            "TADIC_TABLE_CYCLE",
                            format!("cycle at {}", key_text(&nodes[ui])),
                        ));
                    }
                    state[ui] = 1;
                    if kind[ui] == 0 {
                        match self.find_rule(&nodes[ui]) {
                            Some((ri, pt)) => {
                                if self.rules[ri].rhs_expr.is_none() {
                                    let ex = self.rules[ri]
                                        .rhs_text
                                        .iter()
                                        .map(|t| Expr::parse(t, n))
                                        .collect::<Result<Vec<_>, _>>()
                                        .map_err(|e| typed("TADIC_TABLE_EXPRESSION", e))?;
                                    self.rules[ri].rhs_expr = Some(ex);
                                    self.rules[ri].rhs_text = vec![];
                                }
                                let mut ch: Vec<u32> = vec![];
                                let mut terms: Vec<usize> = vec![];
                                let npat = self.rules[ri].rhs_pats.len();
                                for ti in 0..npat {
                                    let k = at(&self.rules[ri].rhs_pats[ti], &pt);
                                    if (0..n).any(|i| !self.mask[i] && k[i] > 0) {
                                        let e =
                                            &self.rules[ri].rhs_expr.as_ref().expect("parsed")[ti];
                                        // the same exact-in-d test as `reduce_mod` (batch 11, b42390d6); the
                                        // dropped term is re-checked EXACTLY zero over Q(d) below
                                        if !e.vanishes_in_d(&pt).unwrap_or(false) {
                                            return Err(typed(
                                                "TADIC_TABLE_INVALID",
                                                format!(
                                                    "ISP in a denominator with a nonzero coefficient: {} from {} (rule {ri})",
                                                    key_text(&k),
                                                    key_text(&nodes[ui])
                                                ),
                                            ));
                                        }
                                        vanishing.push((ri as u32, ti, pt.clone()));
                                        continue;
                                    }
                                    if let Some(c) = intern(
                                        self,
                                        k,
                                        &mut nodes,
                                        &mut kind,
                                        &mut rule_of,
                                        &mut kids,
                                        &mut kid_term,
                                        &mut state,
                                    )? {
                                        ch.push(c);
                                        terms.push(ti);
                                    }
                                }
                                rule_of[ui] = (ri as u32, pt);
                                kids[ui] = ch;
                                kid_term[ui] = terms;
                            }
                            None => {
                                uncovered.push(nodes[ui].clone());
                                kind[ui] = 3;
                            }
                        }
                    }
                }
                let ch = &kids[ui];
                let mut next = ci;
                let mut pushed = false;
                while next < ch.len() {
                    let c = ch[next];
                    next += 1;
                    if state[c as usize] == 0 {
                        stack.push((u, next));
                        stack.push((c, 0));
                        pushed = true;
                        break;
                    } else if state[c as usize] == 1 {
                        return Err(typed(
                            "TADIC_TABLE_CYCLE",
                            format!("cycle below {}", key_text(&nodes[ui])),
                        ));
                    }
                }
                if !pushed {
                    state[ui] = 2;
                    post.push(u);
                }
            }
        }
        if !uncovered.is_empty() {
            return Err(typed(
                "TADIC_TABLE_UNCOVERED",
                format!(
                    "uncovered top-sector keys (no rule, not a master): {}",
                    uncovered
                        .iter()
                        .take(20)
                        .map(|k| key_text(k))
                        .collect::<Vec<_>>()
                        .join(" ")
                ),
            ));
        }
        let graph_seconds = t0.elapsed().as_secs_f64();
        // ---- exact edge coefficients and valuations
        let t1 = std::time::Instant::now();
        let ctx =
            CoefficientContext::try_new(["d"]).map_err(|e| typed("TADIC_EXACT_ALGEBRA", e))?;
        let d = ctx.parameter("d").expect("d");
        let mut edges: Vec<Vec<Edge>> = (0..nodes.len()).map(|_| vec![]).collect();
        let mut zero_edges = 0usize;
        for &u in &post {
            let ui = u as usize;
            if kind[ui] != 0 {
                continue;
            }
            let (ri, pt) = &rule_of[ui];
            let ex = self.rules[*ri as usize].rhs_expr.as_ref().expect("parsed");
            for (&c, &ti) in kids[ui].iter().zip(&kid_term[ui]) {
                let coefficient = exact(&ex[ti].root, n, pt, &ctx, &d).map_err(|e| {
                    format!("{e} (rule {ri}, term {ti}, at {})", key_text(&nodes[ui]))
                })?;
                match coefficient_valuation(&coefficient, opt.d0)
                    .map_err(|e| typed("TADIC_EXACT_ALGEBRA", e))?
                {
                    None => zero_edges += 1,
                    Some(valuation) => edges[ui].push(Edge {
                        child: c,
                        valuation,
                        coefficient,
                    }),
                }
            }
        }
        // the dropped (outside-support) terms must vanish exactly
        for (ri, ti, pt) in &vanishing {
            let ex = self.rules[*ri as usize].rhs_expr.as_ref().expect("parsed");
            let c = exact(&ex[*ti].root, n, pt, &ctx, &d)?;
            if !c.numerator.is_zero() {
                return Err(typed(
                    "TADIC_ERASURE_FAILED",
                    format!(
                        "rule {ri} term {ti}: an outside-support term dropped as zero (vanishes_in_d) is nonzero over Q(d) at the rule point"
                    ),
                ));
            }
        }
        let exact_seconds = t1.elapsed().as_secs_f64();
        // ---- plan: children before parents (post order)
        let mut need = vec![DEAD; nodes.len()];
        for &u in &post {
            let ui = u as usize;
            need[ui] = match kind[ui] {
                1 => opt.order,
                2 if opt.keep_lower => opt.order,
                2 => DEAD,
                _ => {
                    let mut best = DEAD;
                    for e in &edges[ui] {
                        let nc = need[e.child as usize];
                        if nc != DEAD {
                            best = best.max(nc - e.valuation);
                        }
                    }
                    best
                }
            };
        }
        let live: Vec<bool> = need.iter().map(|&x| x != DEAD).collect();
        let (mut max_need, mut min_need) = (i32::MIN, i32::MAX);
        let mut dead_keys = 0usize;
        let mut plan_hasher = DefaultHasher::new();
        let mut graph_hasher = DefaultHasher::new();
        for &u in &post {
            let ui = u as usize;
            nodes[ui].hash(&mut graph_hasher);
            kind[ui].hash(&mut graph_hasher);
            rule_of[ui].hash(&mut graph_hasher);
            for e in &edges[ui] {
                (e.child, e.valuation).hash(&mut graph_hasher);
            }
            need[ui].hash(&mut plan_hasher);
            if live[ui] {
                max_need = max_need.max(need[ui]);
                min_need = min_need.min(need[ui]);
            } else {
                dead_keys += 1;
            }
        }
        // length budget: the longest series a key may carry is need - (lowest start), bounded by the plan
        // (the start of a value is >= the minimum over paths of the summed valuations; checked during the replay)
        // ---- per-rule valuations (precision losses met) over live edges
        // rule -> (lowest valuation, edges, losses)
        let mut rule_stats: BTreeMap<u32, RuleStat> = BTreeMap::new();
        let mut histogram: BTreeMap<i32, usize> = BTreeMap::new();
        for &u in &post {
            let ui = u as usize;
            if kind[ui] != 0 || !live[ui] {
                continue;
            }
            let ri = rule_of[ui].0;
            for e in &edges[ui] {
                if !live[e.child as usize] {
                    continue;
                }
                *histogram.entry(e.valuation).or_default() += 1;
                let entry = rule_stats.entry(ri).or_insert((i32::MAX, 0, 0));
                entry.0 = entry.0.min(e.valuation);
                entry.1 += 1;
                if e.valuation < 0 {
                    entry.2 += 1;
                }
            }
        }
        // ---- forward replay per prime
        let t2 = std::time::Instant::now();
        let mut rng = StdRng::seed_from_u64(opt.seed ^ NODE_TADIC_SALT);
        let mut stream = PrimeIteratorU64::new(NODE_TADIC_BASE + (rng.random::<u64>() >> 6));
        let mut images: Vec<PrimeImage> = vec![];
        let mut unusable: Vec<u64> = vec![];
        let mut consumed = 0usize;
        let mut max_length = 0usize;
        let order_rev: Vec<u32> = post.iter().rev().copied().collect();
        let nreq = requests.len();
        while images.len() < opt.primes {
            if consumed >= opt.max_primes {
                return Err(typed(
                    "TADIC_PRIMES_EXHAUSTED",
                    format!(
                        "{} usable primes within max_primes = {}",
                        images.len(),
                        opt.max_primes
                    ),
                ));
            }
            let p = stream
                .next()
                .ok_or_else(|| typed("TADIC_PRIMES_EXHAUSTED", "no 64-bit prime left"))?;
            consumed += 1;
            // values[node][request]
            let mut values: Vec<Option<Vec<Option<TadicSeries>>>> = vec![None; nodes.len()];
            for (j, r) in requests.iter().enumerate() {
                for (k, w) in &r.inputs {
                    let Some(u) = seed_ids[seed_index[k]] else {
                        continue; // a seed in a zero sector: exactly zero
                    };
                    let ui = u as usize;
                    if !live[ui] {
                        continue;
                    }
                    let wv = arith::from_i128(p, *w as i128);
                    let s = if need[ui] > 0 {
                        strip(TadicSeries {
                            start: 0,
                            coefficients: {
                                let mut v = vec![0u64; need[ui] as usize];
                                v[0] = wv;
                                v
                            },
                        })
                    } else {
                        known_zero(need[ui])
                    };
                    let slot = values[ui].get_or_insert_with(|| vec![None; nreq]);
                    slot[j] = Some(match slot[j].take() {
                        Some(old) => add(p, &old, &s),
                        None => s,
                    });
                }
            }
            let mut bad = false;
            let mut outputs: Vec<Vec<(u32, TadicSeries)>> = vec![vec![]; nreq];
            'nodes: for &u in &order_rev {
                let ui = u as usize;
                let Some(vals) = values[ui].take() else {
                    continue;
                };
                if kind[ui] != 0 {
                    if live[ui] {
                        for (j, v) in vals.into_iter().enumerate() {
                            if let Some(v) = v {
                                outputs[j].push((u, truncate(&v, need[ui])));
                            }
                        }
                    }
                    continue;
                }
                for e in &edges[ui] {
                    let ci = e.child as usize;
                    if !live[ci] {
                        continue;
                    }
                    let target = need[ci];
                    // the coefficient to the precision the lowest-start value needs
                    let lowest = vals
                        .iter()
                        .flatten()
                        .map(|v| v.start)
                        .min()
                        .unwrap_or(target);
                    let len = target - lowest - e.valuation;
                    if len > opt.max_length as i32 {
                        return Err(typed(
                            "TADIC_LENGTH_BUDGET",
                            format!(
                                "a key needs {len} coefficients > max_length {}",
                                opt.max_length
                            ),
                        ));
                    }
                    let Some(cs) = expand_coefficient(&e.coefficient, opt.d0, p, target - lowest)
                        .map_err(|e| typed("TADIC_EXACT_ALGEBRA", e))?
                    else {
                        bad = true;
                        break 'nodes;
                    };
                    // `cs` starts at its first coefficient that is nonzero modulo p (>= the exact valuation); an
                    // all-zero expansion is O(t^(target - lowest)): both are valid inputs of `mul_to`.
                    let slot = values[ci].get_or_insert_with(|| vec![None; nreq]);
                    for (j, v) in vals.iter().enumerate() {
                        let Some(v) = v else { continue };
                        let a = truncate(v, target - e.valuation);
                        if a.precision() < target - e.valuation {
                            return Err(typed(
                                "TADIC_PRECISION_FAILURE",
                                format!(
                                    "{} has precision {} < {} (inconsistent plan)",
                                    key_text(&nodes[ui]),
                                    a.precision(),
                                    target - e.valuation
                                ),
                            ));
                        }
                        let product = mul_to(p, &a, &cs, e.valuation, target)?;
                        max_length =
                            max_length.max((product.precision() - product.start).max(0) as usize);
                        slot[j] = Some(match slot[j].take() {
                            Some(old) => add(p, &old, &product),
                            None => product,
                        });
                    }
                }
            }
            if bad {
                unusable.push(p);
                continue;
            }
            for out in &mut outputs {
                out.sort_by(|a, b| nodes[a.0 as usize].cmp(&nodes[b.0 as usize]));
                for (u, s) in out.iter() {
                    if s.precision() < need[*u as usize] {
                        return Err(typed(
                            "TADIC_PRECISION_FAILURE",
                            format!("output {} below its target", key_text(&nodes[*u as usize])),
                        ));
                    }
                }
            }
            images.push((p, outputs));
        }
        let replay_seconds = t2.elapsed().as_secs_f64();
        // ---- document
        let kind_text = |k: u8| if k == 1 { "master" } else { "exit" };
        let mut reqs = vec![];
        for (j, r) in requests.iter().enumerate() {
            let imgs: Vec<Value> = images
                .iter()
                .map(|(p, outs)| {
                    json!({"prime": p.to_string(), "terms": outs[j].iter().map(|(u, s)| {
                        let ui = *u as usize;
                        let valuation = s.coefficients.iter().position(|c| *c != 0).map(|i| s.start + i as i32);
                        let coefficients: Vec<String> = match valuation {
                            Some(v) => (v..s.precision()).map(|o| s.coefficients[(o - s.start) as usize].to_string()).collect(),
                            None => vec![],
                        };
                        json!({"key": key_text(&nodes[ui]), "kind": kind_text(kind[ui]),
                               "valuation": valuation, "precision": s.precision(), "coefficients": coefficients})
                    }).collect::<Vec<_>>()})
                })
                .collect();
            reqs.push(json!({"label": r.label,
                "inputs": r.inputs.iter().map(|(k, w)| json!({"key": key_text(k), "weight": w})).collect::<Vec<_>>(),
                "images": imgs}));
        }
        let inner = kind.iter().filter(|&&k| k == 0).count();
        let nedges: usize = edges.iter().map(Vec::len).sum();
        Ok(json!({
            "schema": NODE_TADIC_SCHEMA,
            "table": self.path,
            "n": n,
            "d0": opt.d0, "order": opt.order, "seed": opt.seed.to_string(),
            "keep_lower": opt.keep_lower,
            "masters": self.masters.iter().map(|k| key_text(k)).collect::<Vec<_>>(),
            "primes": images.iter().map(|(p, _)| p.to_string()).collect::<Vec<_>>(),
            "unusable_primes": unusable.iter().map(u64::to_string).collect::<Vec<_>>(),
            "requests": reqs,
            "certificate": {
                "graph_digest": format!("{:016x}", graph_hasher.finish()),
                "plan_digest": format!("{:016x}", plan_hasher.finish()),
                "plan": {"max_need": if max_need == i32::MIN { Value::Null } else { json!(max_need) },
                         "min_need": if min_need == i32::MAX { Value::Null } else { json!(min_need) },
                         "dead_keys": dead_keys, "max_length": max_length},
                "erasures": {"recorded": 0, "outside_support_terms_dropped": vanishing.len(),
                             "outside_support_terms_checked_exact": vanishing.len(),
                             "exact_zero_edges": zero_edges},
                "edge_valuation_histogram": histogram.iter().map(|(v, c)| json!([v, c])).collect::<Vec<_>>(),
                "rule_valuations": rule_stats.iter().filter(|(_, s)| s.0 < 0).map(|(ri, s)| json!({
                    "rule": ri, "label": self.rules[*ri as usize].label, "min_valuation": s.0,
                    "uses": s.1, "negative_uses": s.2})).collect::<Vec<_>>(),
            },
            "stats": {"keys": nodes.len(), "inner": inner, "edges": nedges,
                      "graph_seconds": graph_seconds, "exact_seconds": exact_seconds,
                      "replay_seconds": replay_seconds, "seconds": t0.elapsed().as_secs_f64()},
        }))
    }
}
