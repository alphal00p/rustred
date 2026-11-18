//! Import of `table_dump.v1` rule tables as Custom owner payloads (owner:
//! p5l-converter; additive, nothing else reads it yet).
//!
//! The 5L campaign generates each node's rules with `table_dump` (or the face
//! queue, which writes the same format) in the node's own family file: a
//! `.sym` family with `q.q = +1`, its own line order, routing and ISPs, eps
//! lines as `power_shift`s. The p library owns a Custom node in one
//! presentation of the node's graph and reads its rules from a generated
//! candidate bundle over that presentation's family ([`super::custom`]).
//!
//! [`import_table`] turns one table into such a payload without re-deriving
//! anything:
//!
//! 1. the presentation: the table family's own momenta, its line slots
//!    reordered to the canonical lines of the node graph ([`PTopo`] of the
//!    DAG node, `normalise` + `canonical`), each oriented along its canonical
//!    line, `q` entering at the graph's `s` (if the table routes `q` the
//!    other way, the `q` column of every slot is negated: `k -> -k` maps the
//!    two families' integrals onto each other identically), the table's ISPs
//!    after the lines in table order, eps units from the power shifts, and
//!    `q.q = -1` ([`P_EXTERNAL_SQUARE`]); kind [`PresentationKind::Caller`];
//! 2. every rule in table order (the order is part of the artefact:
//!    `notes/artifact-ordering.SPEC.md`), chart by chart in the header's chart
//!    order: keys, cases (face and affine equations), exception branches and
//!    residuals relabelled by the slot map, coefficients renamed `n_j -> n_{P(j)}`
//!    and multiplied by `(-1)^(|target| - |term|)` (the sums of the integer parts
//!    of the powers): with `q.q = Q` a rule term carries `Q^(|term| - |target|)`
//!    by dimension, so `Q = +1 -> -1` is that sign and nothing else;
//! 3. the order: the header's `OrderDescriptor` with every slot-indexed field
//!    permuted by the slot map (`legacy` = `spired_uncut`); an unpermuted
//!    legacy table keeps `SpiredUncutV1`, as `p-generate` writes it;
//! 4. the dispatch policy of each rule (`Partition`, or a v3 dispatch entry).
//!
//! Seeds of the table (the trace sources) are not in the format, so payload
//! rules carry none; nothing in the p path reads them.
//!
//! What the payload format cannot carry is refused with a [`TableImportGap`]
//! (no partial payload): a missing `end`, a `chart_error`, deferred cases or
//! regions, a `zero` chart of the support, a case that is neither a coordinate
//! face nor an affine case, a dispatch policy the bundle has no code for, a
//! power or fixed value outside `i16`, a key whose symbolic slot names
//! another index, a node graph whose lines do not carry the table's momenta.
#![allow(
    clippy::needless_range_loop,
    clippy::collapsible_if,
    clippy::redundant_closure
)] // p5l-converter table parser: index loops over row/column arrays mirror the table_dump format (M5 batch 3 gate)
use std::collections::BTreeMap;
use std::fmt;

use serde_json::Value;

use crate::algebra::{
    Coefficient, CoefficientContext, ExactAlgebraLimits, IndexedCoefficientContext,
};
use crate::persistence::{
    BinaryIoLimits, BinaryProgramKind, BinarySection, CoefficientTableBuilder, NativeFamilyRecord,
    SectionTag, encode_program, encode_program_streamed,
};
use crate::sector::OrderingPolicy;

use super::PresentationId;
use super::bundle_wire::{
    CANDIDATE_BUNDLE_SCHEMA, CANDIDATE_BUNDLE_STATUS, CaseRecord, DISPATCH_CANDIDATE_BUNDLE_SCHEMA,
    EncodedSectorRecord, IntegralRecord, ProgramRecord, RuleDispatchRecord, RuleRecord, TermRecord,
    encode_program_record_with_sectors,
};
use super::certified_keys::CertifiedKeySet;
use super::custom::{CustomPayloadSummary, inspect_custom_payload};
use super::error::PError;
use super::library::CustomOwnerMeta;
use super::presentation::{
    NodeRef, P_EXTERNAL_SQUARE, Presentation, PresentationKind, Slot, SlotIsp, SlotLine,
    eps_power_shift,
};
use super::topology::{GraphKey, Normalised, PTopo, canonical, normalise};

/// Schemas this importer reads (the header's `schema`); every later variant of
/// the campaign (`-defer-gsplit`) keeps the v1 records.
pub const TABLE_IMPORT_SCHEMA_PREFIX: &str = "table_dump.v1";
/// Version of the import rules above; recorded in every import report.
/// v2: the payload records the table's slot order ([`SlotOrderRecord`]).
pub const P_TABLE_IMPORT_VERSION: u32 = 2;
/// The solver policy of an imported payload: the table's numerical depth 2 is
/// the default all-rank policy of generated Custom payloads.
const IMPORT_SOLVER_POLICY: &str = "ordinary-source-port-default-v1";
const IMPORT_INPUT_FORMAT: &str = "auto";
/// `rustred_indexed_coefficient_v1::n<slot>`: the positional index symbols of
/// every indexed context ([`IndexedCoefficientContext`]).
const INDEX_SYMBOL: &str = "rustred_indexed_coefficient_v1::n";

/// Why a table cannot become a payload: one entry of the gap list.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TableImportGap {
    /// A short kind (`chart_error`, `deferred`, `case_kind`, `dispatch`, ...).
    pub kind: &'static str,
    pub detail: String,
}

impl fmt::Display for TableImportGap {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{}: {}", self.kind, self.detail)
    }
}

fn gap(kind: &'static str, detail: impl Into<String>) -> PError {
    PError::Unsupported(format!(
        "table import gap {}",
        TableImportGap {
            kind,
            detail: detail.into()
        }
    ))
}

fn malformed(detail: impl fmt::Display) -> PError {
    PError::MalformedLibrary(format!("table import: {detail}"))
}

/// One slot of the table family (`prop(D, (momentum)^2, 1|0)`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TableSlot {
    pub name: String,
    /// Over `(k_1, .., k_L, q)`.
    pub momentum: Vec<i64>,
    /// `1`: a line; `0`: an ISP.
    pub line: bool,
}

/// The table family, from the header's `family_source`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TableFamily {
    pub name: String,
    pub loops: u8,
    pub slots: Vec<TableSlot>,
    /// Eps units per slot, from the header's `power_shifts`.
    pub eps_units: Vec<u8>,
}

/// What [`import_table`] made of a table.
#[derive(Clone, Debug)]
pub struct ImportedOwner {
    pub presentation: Presentation,
    pub payload: Vec<u8>,
    pub meta: CustomOwnerMeta,
    pub summary: CustomPayloadSummary,
    /// Table slot `j` is presentation slot `slot_map[j]`.
    pub slot_map: Vec<usize>,
    /// The table routes `q` from the graph's `t` to `s`: its `q` column is negated.
    pub q_flipped: bool,
    /// Canonical line `i` carries the table line's momentum times `line_signs[i]`.
    pub line_signs: Vec<i8>,
    pub graph_key: GraphKey,
    pub table_family: TableFamily,
    pub rules: usize,
    pub rhs_terms: usize,
    /// RHS terms whose coefficient changed sign (`q.q = +1 -> -1`).
    pub negated_terms: usize,
    pub dispatched_rules: usize,
    pub affine_rules: usize,
    pub rules_with_exceptions: usize,
    pub order_name: String,
    /// The slot order the payload records and enforces.
    pub slot_order: SlotOrderRecord,
    /// boxed-node: the box record of a boxed table (`None` otherwise).
    pub boxed: Option<BoxedNodeRecord>,
    /// vk5l WP2: the recipe bundle the payload carries.
    pub recipes: Option<super::recipes::RecipeBundle>,
    /// vk5l WP6: [`import_table_streamed`] wrote the payload to its sink
    /// (`payload` is then empty): its length in bytes.
    pub streamed_payload_bytes: Option<u64>,
}

impl ImportedOwner {
    /// The payload's length, held or streamed.
    pub fn payload_len(&self) -> u64 {
        self.streamed_payload_bytes
            .unwrap_or(self.payload.len() as u64)
    }
}

/// vk5l WP6: `RUSTRED_P_IMPORT_TRACE=1` prints the process RSS (VmRSS, VmHWM)
/// at the import's phases on stderr.
fn trace_rss(phase: &str) {
    if std::env::var_os("RUSTRED_P_IMPORT_TRACE").is_none() {
        return;
    }
    let status = std::fs::read_to_string("/proc/self/status").unwrap_or_default();
    let field = |name: &str| {
        status
            .lines()
            .find(|line| line.starts_with(name))
            .and_then(|line| line.split_whitespace().nth(1))
            .and_then(|kb| kb.parse::<f64>().ok())
            .map_or(-1.0, |kb| kb / 1e6)
    };
    eprintln!(
        "[import-rss] {phase}: rss {:.2} GB, peak {:.2} GB",
        field("VmRSS:"),
        field("VmHWM:")
    );
}

// ----- the family -----

fn parse_momentum(text: &str, loops: &[String], externals: &[String]) -> Result<Vec<i64>, PError> {
    let inner = text
        .trim()
        .strip_suffix("^2")
        .and_then(|rest| rest.trim().strip_prefix('('))
        .and_then(|rest| rest.strip_suffix(')'))
        .ok_or_else(|| malformed(format!("momentum {text:?} is not (p)^2")))?;
    let mut row = vec![0_i64; loops.len() + 1];
    let compact: String = inner.chars().filter(|c| !c.is_whitespace()).collect();
    let mut rest = compact.as_str();
    while !rest.is_empty() {
        let (sign, body) = match rest.as_bytes()[0] {
            b'+' => (1, &rest[1..]),
            b'-' => (-1, &rest[1..]),
            _ => (1, rest),
        };
        let end = body.find(['+', '-']).unwrap_or(body.len());
        let term = &body[..end];
        rest = &body[end..];
        let (factor, name) = match term.split_once('*') {
            Some((factor, name)) => (
                factor
                    .parse::<i64>()
                    .map_err(|_| malformed(format!("momentum term {term:?}")))?,
                name,
            ),
            None => (1, term),
        };
        let column = if let Some(index) = loops.iter().position(|loop_| loop_ == name) {
            index
        } else if externals.first().is_some_and(|external| external == name) {
            loops.len()
        } else {
            return Err(malformed(format!("momentum term {term:?} in {text:?}")));
        };
        row[column] += sign * factor;
    }
    Ok(row)
}

fn arguments(text: &str) -> Vec<String> {
    // Top-level comma split of `name(a, (b), c)`'s argument list.
    let mut parts = Vec::new();
    let mut depth = 0_i32;
    let mut current = String::new();
    for c in text.chars() {
        match c {
            '(' => {
                depth += 1;
                current.push(c);
            }
            ')' => {
                depth -= 1;
                current.push(c);
            }
            ',' if depth == 0 => parts.push(std::mem::take(&mut current)),
            _ => current.push(c),
        }
    }
    if !current.trim().is_empty() {
        parts.push(current);
    }
    parts
        .into_iter()
        .map(|part| part.trim().to_owned())
        .collect()
}

/// The `I(...)` family text of a table header.
pub fn parse_table_family(source: &str, power_shifts: &[String]) -> Result<TableFamily, PError> {
    let body = source
        .trim()
        .strip_prefix("I(")
        .and_then(|rest| rest.trim_end().strip_suffix(')'))
        .ok_or_else(|| malformed("family_source is not I(...)"))?;
    let (mut name, mut loops, mut externals) = (String::new(), Vec::new(), Vec::new());
    let mut slots = Vec::new();
    let mut gram = None;
    for entry in arguments(body) {
        let (head, inner) = entry
            .split_once('(')
            .and_then(|(head, rest)| rest.strip_suffix(')').map(|inner| (head.trim(), inner)))
            .ok_or_else(|| malformed(format!("family entry {entry:?}")))?;
        match head {
            "name" => name = inner.trim().to_owned(),
            "loops" => loops = arguments(inner),
            "externals" => externals = arguments(inner),
            "dimension" if inner.trim() == "d" => {}
            "prop" => {
                let parts = arguments(inner);
                let [slot, momentum, power] = parts.as_slice() else {
                    return Err(malformed(format!("prop {inner:?}")));
                };
                let line = match power.as_str() {
                    "1" => true,
                    "0" => false,
                    _ => return Err(gap("prop_power", format!("prop {inner:?}"))),
                };
                slots.push(TableSlot {
                    name: slot.clone(),
                    momentum: parse_momentum(momentum, &loops, &externals)?,
                    line,
                });
            }
            "gram" => gram = Some(arguments(inner)),
            "power_shift" => {} // read from the header's power_shifts
            _ => return Err(gap("family_entry", format!("{entry:?}"))),
        }
    }
    if externals.len() != 1
        || gram.as_deref()
            != Some(&[externals[0].clone(), externals[0].clone(), "1".to_owned()][..])
    {
        return Err(gap(
            "kinematics",
            format!(
                "externals {externals:?}, gram {gram:?}: one external with q.q = 1 is required"
            ),
        ));
    }
    if power_shifts.len() != slots.len() {
        return Err(malformed(format!(
            "{} power shifts for {} slots",
            power_shifts.len(),
            slots.len()
        )));
    }
    let context = CoefficientContext::try_new(["d"]).map_err(|_| PError::ContextMismatch)?;
    let indexed = IndexedCoefficientContext::try_new(&context, "p-table-import-shifts", 1)
        .map_err(|error| malformed(error))?;
    let mut eps_units = Vec::with_capacity(slots.len());
    for (slot, shift) in slots.iter().zip(power_shifts) {
        let value = indexed
            .parse_expression_with_limits(shift, ExactAlgebraLimits::default())
            .map_err(|error| malformed(format!("power shift {shift:?}: {error}")))?;
        let units = (0_u8..=16)
            .find(|&j| {
                eps_power_shift(&context, j)
                    .ok()
                    .and_then(|expected| indexed.lift(&expected).ok())
                    .is_some_and(|expected| expected == value)
            })
            .ok_or_else(|| gap("power_shift", format!("{shift:?} is no j (4 - d) / 2")))?;
        if units != 0 && !slot.line {
            return Err(gap(
                "power_shift",
                format!("an eps shift on the ISP {}", slot.name),
            ));
        }
        eps_units.push(units);
    }
    let loops_count = u8::try_from(loops.len()).map_err(|_| malformed("too many loop momenta"))?;
    Ok(TableFamily {
        name,
        loops: loops_count,
        slots,
        eps_units,
    })
}

// ----- the node graph -----

/// A Forcer/census graph `0-1* 0-2 1-3**` (stars are eps units) with `q` in
/// at `s` and out at `t`, normalised and canonically labelled: the
/// [`PTopo`] and key of its DAG node.
pub fn canonical_node_graph(edges: &str, s: u8, t: u8) -> Result<(GraphKey, PTopo), PError> {
    let mut lines = Vec::new();
    let mut n = s.max(t);
    for token in edges.split_whitespace() {
        let stars = token.bytes().filter(|&b| b == b'*').count();
        let pair = token.trim_end_matches('*');
        let (u, v) = pair
            .split_once('-')
            .and_then(|(u, v)| Some((u.parse::<u8>().ok()?, v.parse::<u8>().ok()?)))
            .ok_or_else(|| malformed(format!("graph edge {token:?}")))?;
        n = n.max(u).max(v);
        lines.push((
            u,
            v,
            u8::try_from(stars).map_err(|_| malformed("eps units"))?,
        ));
    }
    let topo = PTopo::new(n + 1, s, t, lines);
    canonical_topology(&topo)
}

/// The normalised, canonically labelled form of `topo` and its key.
pub fn canonical_topology(topo: &PTopo) -> Result<(GraphKey, PTopo), PError> {
    let Normalised::Topo(normal, _) = normalise(topo) else {
        return Err(malformed(
            "the node graph normalises to zero or the trivial node",
        ));
    };
    let (key, canon) = canonical(&normal);
    Ok((key, canon.apply(&normal)))
}

/// Canonical line `i` gets table line `assignment[i]` with sign `signs[i]`;
/// `q` enters at `graph.s` (`q_sign = 1`) or at `graph.t` (`q_sign = -1`).
struct LineMatch {
    assignment: Vec<usize>,
    signs: Vec<i8>,
    q_sign: i64,
}

fn match_lines(graph: &PTopo, lines: &[(Vec<i64>, u8)]) -> Option<LineMatch> {
    let count = graph.lines.len();
    if count != lines.len() {
        return None;
    }
    let width = lines.first().map_or(1, |(row, _)| row.len());
    // The last line index touching each vertex: the vertex is complete there.
    let mut last = vec![None; usize::from(graph.n)];
    for (index, &(u, v, _)) in graph.lines.iter().enumerate() {
        last[usize::from(u)] = Some(index);
        last[usize::from(v)] = Some(index);
    }
    struct Search<'a> {
        graph: &'a PTopo,
        lines: &'a [(Vec<i64>, u8)],
        last: Vec<Option<usize>>,
        width: usize,
        used: Vec<bool>,
        assignment: Vec<usize>,
        signs: Vec<i8>,
        balance: Vec<Vec<i64>>,
    }
    impl Search<'_> {
        fn vertex_ok(&self, vertex: usize) -> bool {
            self.balance[vertex].iter().all(|&c| c == 0)
        }
        fn go(&mut self, index: usize) -> bool {
            if index == self.graph.lines.len() {
                return true;
            }
            let (u, v, eps) = self.graph.lines[index];
            let (u, v) = (usize::from(u), usize::from(v));
            let lines = self.lines;
            for candidate in 0..lines.len() {
                if self.used[candidate] || lines[candidate].1 != eps {
                    continue;
                }
                for sign in [1_i8, -1] {
                    let row = &lines[candidate].0;
                    for column in 0..self.width {
                        let value = i64::from(sign) * row[column];
                        self.balance[u][column] -= value;
                        self.balance[v][column] += value;
                    }
                    let complete = |vertex: usize| self.last[vertex] == Some(index);
                    let ok =
                        (!complete(u) || self.vertex_ok(u)) && (!complete(v) || self.vertex_ok(v));
                    if ok {
                        self.used[candidate] = true;
                        self.assignment[index] = candidate;
                        self.signs[index] = sign;
                        if self.go(index + 1) {
                            return true;
                        }
                        self.used[candidate] = false;
                    }
                    for column in 0..self.width {
                        let value = i64::from(sign) * row[column];
                        self.balance[u][column] += value;
                        self.balance[v][column] -= value;
                    }
                }
            }
            false
        }
    }
    for q_sign in [1_i64, -1] {
        let mut balance = vec![vec![0_i64; width]; usize::from(graph.n)];
        balance[usize::from(graph.s)][width - 1] += q_sign;
        balance[usize::from(graph.t)][width - 1] -= q_sign;
        let mut search = Search {
            graph,
            lines,
            last: last.clone(),
            width,
            used: vec![false; count],
            assignment: vec![0; count],
            signs: vec![1; count],
            balance,
        };
        if search.go(0) {
            return Some(LineMatch {
                assignment: search.assignment,
                signs: search.signs,
                q_sign,
            });
        }
    }
    None
}

// ----- records -----

/// One power of a table key: `ni+k` (symbolic, offset `k`) or an integer.
fn parse_key(text: &str, arity: usize) -> Result<Vec<(bool, i64)>, PError> {
    let inner = text
        .strip_prefix("I(")
        .and_then(|rest| rest.strip_suffix(')'))
        .ok_or_else(|| malformed(format!("key {text:?}")))?;
    let parts: Vec<&str> = inner.split(',').map(str::trim).collect();
    if parts.len() != arity {
        return Err(malformed(format!(
            "key {text:?} has {} slots, not {arity}",
            parts.len()
        )));
    }
    parts
        .iter()
        .enumerate()
        .map(|(slot, part)| {
            if let Some(rest) = part.strip_prefix('n') {
                let digits = rest.bytes().take_while(u8::is_ascii_digit).count();
                let index: usize = rest[..digits]
                    .parse()
                    .map_err(|_| malformed(format!("key {text:?}")))?;
                if index != slot {
                    return Err(gap(
                        "key_index",
                        format!("slot {slot} of {text:?} names n{index}"),
                    ));
                }
                let offset = match &rest[digits..] {
                    "" => 0,
                    other => other
                        .strip_prefix('+')
                        .unwrap_or(other)
                        .parse::<i64>()
                        .map_err(|_| malformed(format!("key {text:?}")))?,
                };
                Ok((true, offset))
            } else {
                Ok((
                    false,
                    part.parse::<i64>()
                        .map_err(|_| malformed(format!("key {text:?}")))?,
                ))
            }
        })
        .collect()
}

/// One slot of a slot-permuted alias key: an integer, or the index of ANOTHER (or the same) slot plus an offset.
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum AliasSlot {
    Fixed(i64),
    Index { slot: usize, offset: i64 },
}

/// [`parse_key`] without the slot-index restriction: slot `i` may read `n<j>+k` for any `j`.
fn parse_key_any(text: &str, arity: usize) -> Result<Vec<AliasSlot>, PError> {
    let inner = text
        .strip_prefix("I(")
        .and_then(|rest| rest.strip_suffix(')'))
        .ok_or_else(|| malformed(format!("key {text:?}")))?;
    let parts: Vec<&str> = inner.split(',').map(str::trim).collect();
    if parts.len() != arity {
        return Err(malformed(format!(
            "key {text:?} has {} slots, not {arity}",
            parts.len()
        )));
    }
    parts
        .iter()
        .map(|part| match part.strip_prefix('n') {
            Some(rest) => {
                let digits = rest.bytes().take_while(u8::is_ascii_digit).count();
                let slot: usize = rest[..digits]
                    .parse()
                    .map_err(|_| malformed(format!("key {text:?}")))?;
                if slot >= arity {
                    return Err(malformed(format!("key {text:?} names n{slot}")));
                }
                let offset = match &rest[digits..] {
                    "" => 0,
                    other => other
                        .strip_prefix('+')
                        .unwrap_or(other)
                        .parse::<i64>()
                        .map_err(|_| malformed(format!("key {text:?}")))?,
                };
                Ok(AliasSlot::Index { slot, offset })
            }
            None => Ok(AliasSlot::Fixed(
                part.parse::<i64>()
                    .map_err(|_| malformed(format!("key {text:?}")))?,
            )),
        })
        .collect()
}

/// A SLOT-PERMUTED ALIAS rule (T178 unboxed F3 / F4: `I(key) = c * I(image)`, the image's slots read the key's
/// indices in another order, a pure slot permutation of the face): kept beside the payload's rules (the shift-based
/// rule format cannot hold a permuted right-hand side) and dispatched by the Custom owner AFTER its ordinary rules,
/// which is exactly the table's first match because the import proves every LATER rule of the chart disjoint from
/// the alias's domain. Everything in presentation slots; `fixed` / `min` / `max` bound the key's powers (the target is
/// canonical), `child[s]` gives slot `s` of the image; `coefficient` carries the import's sign convention.
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct SlotAliasRecord {
    /// The rule's index in the table (its first-match position).
    pub index: usize,
    /// The number of ordinary rules of its chart before it in the table: a rule of the chart at a position at or
    /// beyond this one comes AFTER the alias in first match (the alias wins where both apply).
    pub rules_before: usize,
    pub chart: Vec<bool>,
    pub fixed: Vec<Option<i64>>,
    pub min: Vec<Option<i64>>,
    pub max: Vec<Option<i64>>,
    pub child: Vec<AliasSlot>,
    pub coefficient: i64,
}

impl SlotAliasRecord {
    /// The image of `powers` (presentation slots) when the alias applies to it (chart, fixed values, bounds), else
    /// `None`.
    pub fn image(&self, powers: &[i64]) -> Option<Vec<i64>> {
        if powers.len() != self.chart.len() {
            return None;
        }
        for (slot, &power) in powers.iter().enumerate() {
            if (power > 0) != self.chart[slot]
                || self.fixed[slot].is_some_and(|value| value != power)
                || self.min[slot].is_some_and(|value| power < value)
                || self.max[slot].is_some_and(|value| power > value)
            {
                return None;
            }
        }
        Some(
            self.child
                .iter()
                .map(|slot| match *slot {
                    AliasSlot::Fixed(value) => value,
                    AliasSlot::Index { slot, offset } => powers[slot] + offset,
                })
                .collect(),
        )
    }
}

/// The slot-permuted alias records of a payload are one provenance line after this marker (JSON array of
/// [`SlotAliasRecord`]), before the slot order.
pub const SLOT_ALIASES_MARKER: &str = "\nslot_aliases ";

/// The slot-permuted alias records of a payload's provenance text (empty if none).
pub fn slot_aliases_of_source(family_source: &str) -> Result<Vec<SlotAliasRecord>, PError> {
    let Some(at) = family_source.find(SLOT_ALIASES_MARKER) else {
        return Ok(Vec::new());
    };
    let text = &family_source[at + SLOT_ALIASES_MARKER.len()..];
    let text = text.split('\n').next().unwrap_or("");
    serde_json::from_str(text).map_err(|error| malformed(format!("slot alias records: {error}")))
}

/// The [`SlotAliasRecord`] of a rule record whose right-hand side permutes the key's slots (table slots in,
/// presentation slots out); typed `slot_alias` gap for anything but the alias form.
#[allow(clippy::too_many_arguments)]
fn slot_alias_record(
    record: &Value,
    index: usize,
    chart: &[bool],
    fixed: &[Option<i64>],
    min: &[Option<i64>],
    max: &[Option<i64>],
    arity: usize,
    slot_map: &[usize],
) -> Result<SlotAliasRecord, PError> {
    let refuse = |detail: &str| gap("slot_alias", format!("rule {index}: {detail}"));
    let rhs = record["rhs"]
        .as_array()
        .ok_or_else(|| malformed("rule rhs"))?;
    if rhs.len() != 1 {
        return Err(refuse(
            "a slot-permuted rule must have exactly one right-hand-side term",
        ));
    }
    if !case_text(record)?.starts_with("Coordinate(")
        || !record["equations"].as_array().is_some_and(Vec::is_empty)
        || !record["exceptions"].as_array().is_some_and(Vec::is_empty)
        || !matches!(record["dispatch"].as_str(), None | Some("Partition"))
    {
        return Err(refuse(
            "a slot-permuted rule must be a coordinate case without equations or exceptions, Partition",
        ));
    }
    let target = parse_key(str_field(record, "target")?, arity)?;
    if target
        .iter()
        .any(|&(symbolic, offset)| symbolic && offset != 0)
    {
        return Err(refuse(
            "the target of a slot-permuted rule must be canonical (n<i> in slot i)",
        ));
    }
    for (slot, &(symbolic, value)) in target.iter().enumerate() {
        if symbolic == fixed[slot].is_some() || fixed[slot].is_some_and(|f| f != value) {
            return Err(refuse("fixed values and target disagree"));
        }
    }
    let pair = rhs[0]
        .as_array()
        .filter(|pair| pair.len() == 2)
        .ok_or_else(|| malformed("rhs term"))?;
    let child = parse_key_any(pair[0].as_str().ok_or_else(|| malformed("rhs key"))?, arity)?;
    let mut used = vec![0usize; arity];
    for part in &child {
        if let AliasSlot::Index { slot, .. } = *part {
            if !target[slot].0 {
                return Err(refuse("the image names the index of a fixed slot"));
            }
            used[slot] += 1;
        }
    }
    if (0..arity).any(|slot| target[slot].0 && used[slot] != 1) {
        return Err(refuse(
            "every index of the target must occur exactly once in the image",
        ));
    }
    // a declared slot map (`rhs_slot_maps`, top level or in `symmetry_origin`: per rhs term, per term slot j the
    // target slot whose index the term carries there, null for a constant) must be the one read from the symbols
    let declared = record
        .get("rhs_slot_maps")
        .or_else(|| record["symmetry_origin"].get("rhs_slot_maps"));
    if let Some(declared) = declared {
        let map = declared
            .as_array()
            .filter(|maps| maps.len() == 1)
            .and_then(|maps| maps[0].as_array())
            .filter(|map| map.len() == arity)
            .ok_or_else(|| malformed("rhs_slot_maps: one array of the arity per rhs term"))?;
        for (j, part) in child.iter().enumerate() {
            let agrees = match (*part, &map[j]) {
                (AliasSlot::Fixed(_), Value::Null) => true,
                (AliasSlot::Index { slot, .. }, entry) => entry.as_u64() == Some(slot as u64),
                _ => false,
            };
            if !agrees {
                return Err(refuse(
                    "rhs_slot_maps disagrees with the symbols of the image",
                ));
            }
        }
    }
    let coefficient_text = pair[1]
        .as_str()
        .ok_or_else(|| malformed("rhs coefficient"))?
        .trim();
    let mut coefficient: i64 = coefficient_text
        .parse()
        .map_err(|_| refuse("a slot-permuted rule needs an integer coefficient"))?;
    // the import's sign convention (as for every term): the weights' parity; the indices cancel (each once)
    let target_weight: i64 = target.iter().map(|&(_, value)| value).sum();
    let child_weight: i64 = child
        .iter()
        .map(|part| match *part {
            AliasSlot::Fixed(value) => value,
            AliasSlot::Index { offset, .. } => offset,
        })
        .sum();
    if (target_weight - child_weight).rem_euclid(2) == 1 {
        coefficient = -coefficient;
    }
    let mut child_presented = vec![AliasSlot::Fixed(0); arity];
    for (slot, part) in child.iter().enumerate() {
        child_presented[slot_map[slot]] = match *part {
            AliasSlot::Fixed(value) => AliasSlot::Fixed(value),
            AliasSlot::Index { slot: from, offset } => AliasSlot::Index {
                slot: slot_map[from],
                offset,
            },
        };
    }
    Ok(SlotAliasRecord {
        index,
        rules_before: 0,
        chart: permuted(chart, slot_map),
        fixed: permuted(fixed, slot_map),
        min: permuted(min, slot_map),
        max: permuted(max, slot_map),
        child: child_presented,
        coefficient,
    })
}

/// Per-slot optional bounds.
type OptionalBounds = Vec<Option<i64>>;

/// `domain_bounds` of a rule record as (min, max) per table slot (absent = unbounded).
fn bounds_of_record(
    record: &Value,
    arity: usize,
) -> Result<(OptionalBounds, OptionalBounds), PError> {
    let side = |name: &str| -> Result<Vec<Option<i64>>, PError> {
        match record.get("domain_bounds").and_then(|b| b.get(name)) {
            None | Some(Value::Null) => Ok(vec![None; arity]),
            Some(Value::Array(values)) if values.len() == arity => values
                .iter()
                .map(|v| {
                    if v.is_null() {
                        Ok(None)
                    } else {
                        v.as_i64()
                            .map(Some)
                            .ok_or_else(|| malformed("domain_bounds value"))
                    }
                })
                .collect(),
            Some(_) => Err(malformed("domain_bounds arity")),
        }
    };
    Ok((side("min")?, side("max")?))
}

fn small(value: i64, what: &str) -> Result<i16, PError> {
    i16::try_from(value).map_err(|_| gap("range", format!("{what} {value} outside i16")))
}

/// The record of a parsed key in presentation slot order.
fn integral_record(key: &[(bool, i64)], slot_map: &[usize]) -> Result<IntegralRecord, PError> {
    let mut symbolic = vec![false; key.len()];
    let mut values = vec![0_i16; key.len()];
    for (slot, &(is_symbolic, value)) in key.iter().enumerate() {
        symbolic[slot_map[slot]] = is_symbolic;
        values[slot_map[slot]] = small(value, "power")?;
    }
    Ok(IntegralRecord { symbolic, values })
}

/// `n<j>` (a whole identifier) becomes the index symbol of slot `slot_map[j]`.
fn rename_indices(text: &str, slot_map: &[usize]) -> Result<String, PError> {
    let bytes = text.as_bytes();
    let mut out = String::with_capacity(text.len() + 16);
    let mut i = 0;
    while i < bytes.len() {
        let c = bytes[i];
        let boundary = i == 0 || !(bytes[i - 1].is_ascii_alphanumeric() || bytes[i - 1] == b'_');
        if c == b'n' && boundary && i + 1 < bytes.len() && bytes[i + 1].is_ascii_digit() {
            let digits = bytes[i + 1..]
                .iter()
                .take_while(|b| b.is_ascii_digit())
                .count();
            let after = i + 1 + digits;
            let identifier_continues = after < bytes.len()
                && (bytes[after].is_ascii_alphanumeric() || bytes[after] == b'_');
            if !identifier_continues {
                let index: usize = text[i + 1..after]
                    .parse()
                    .map_err(|_| malformed(format!("coefficient {text:?}")))?;
                let slot = slot_map
                    .get(index)
                    .ok_or_else(|| malformed(format!("coefficient {text:?} names n{index}")))?;
                out.push_str(INDEX_SYMBOL);
                out.push_str(&slot.to_string());
                i = after;
                continue;
            }
        }
        out.push(c as char);
        i += 1;
    }
    Ok(out)
}

struct Coefficients<'a> {
    context: &'a IndexedCoefficientContext,
    table: CoefficientTableBuilder,
    slot_map: &'a [usize],
}

impl Coefficients<'_> {
    fn value(&self, text: &str) -> Result<Coefficient, PError> {
        let renamed = rename_indices(text, self.slot_map)?;
        let parsed = self
            .context
            .parse_expression_with_limits(&renamed, ExactAlgebraLimits::default())
            .map_err(|error| malformed(format!("coefficient {text:?}: {error}")))?;
        Ok(parsed.raw().clone())
    }

    fn intern(&mut self, value: &Coefficient) -> Result<u32, PError> {
        let id = self
            .table
            .intern(value)
            .map_err(|error| malformed(format!("coefficient table: {error}")))?;
        u32::try_from(id.index()).map_err(|_| malformed("coefficient table too large"))
    }

    fn polynomial(&mut self, text: &str) -> Result<u32, PError> {
        let value = self.value(text)?;
        if !value.denominator.is_one() {
            return Err(malformed(format!("equation {text:?} is not a polynomial")));
        }
        self.intern(&value)
    }
}

fn str_field<'a>(record: &'a Value, field: &str) -> Result<&'a str, PError> {
    record[field]
        .as_str()
        .ok_or_else(|| malformed(format!("{} record without {field}", record["type"])))
}

/// A rule record's case text. Records written without `case` (hand-made or appended rules, e.g. T178-unboxed
/// 5232-5241) are read by their equations: none -> a coordinate case, some -> an affine case (the importer uses only
/// this kind; the fixed values come from `fixed`).
fn case_text(record: &Value) -> Result<&str, PError> {
    match record["case"].as_str() {
        Some(text) => Ok(text),
        None => match record["equations"].as_array() {
            Some(equations) if equations.is_empty() => Ok("Coordinate(inferred)"),
            Some(_) => Ok("Affine(inferred)"),
            None => Err(malformed("rule record without case or equations")),
        },
    }
}

fn mask_bits(text: &str) -> Result<Vec<bool>, PError> {
    text.bytes()
        .map(|b| match b {
            b'1' => Ok(true),
            b'0' => Ok(false),
            _ => Err(malformed(format!("mask {text:?}"))),
        })
        .collect()
}

fn permuted<T: Clone>(values: &[T], slot_map: &[usize]) -> Vec<T> {
    let mut out = values.to_vec();
    for (slot, value) in values.iter().enumerate() {
        out[slot_map[slot]] = value.clone();
    }
    out
}

fn u64_list(value: &Value, field: &str) -> Result<Vec<u64>, PError> {
    value
        .as_array()
        .ok_or_else(|| malformed(format!("order field {field}")))?
        .iter()
        .map(|entry| {
            entry
                .as_u64()
                .ok_or_else(|| malformed(format!("order field {field}")))
        })
        .collect()
}

/// The header's `order` as an [`OrderDescriptor`] with every slot permuted.
pub(crate) fn permuted_order(
    order: &Value,
    slot_map: &[usize],
) -> Result<rustred_order::OrderDescriptor, PError> {
    use rustred_order::{CoordinateGroups, DegreeRow, Direction, OrderDescriptor};
    let rows = |field: &str| -> Result<Vec<DegreeRow>, PError> {
        order[field]
            .as_array()
            .ok_or_else(|| malformed(format!("order field {field}")))?
            .iter()
            .map(|row| {
                Ok(DegreeRow {
                    active: permuted(&u64_list(&row["active"], field)?, slot_map),
                    inactive: permuted(&u64_list(&row["inactive"], field)?, slot_map),
                })
            })
            .collect()
    };
    let priority = |field: &str| -> Result<Vec<usize>, PError> {
        u64_list(&order[field], field)?
            .into_iter()
            .map(|slot| {
                slot_map
                    .get(slot as usize)
                    .copied()
                    .ok_or_else(|| malformed(format!("order field {field}")))
            })
            .collect()
    };
    let direction = |field: &str| match order[field].as_str() {
        Some("Ascending") => Ok(Direction::Ascending),
        Some("Descending") => Ok(Direction::Descending),
        _ => Err(malformed(format!("order field {field}"))),
    };
    let two_sided: Vec<bool> = match order["two_sided"].as_array() {
        Some(flags) => flags
            .iter()
            .map(|flag| match flag {
                Value::Bool(b) => Ok(*b),
                Value::Number(n) => Ok(n.as_u64() == Some(1)),
                _ => Err(malformed("order field two_sided")),
            })
            .collect::<Result<_, _>>()?,
        None => Vec::new(),
    };
    if order.get("degree_rows").is_none() && order.get("support_weights").is_none() {
        // The rr_node driver's compact form (rr-consolidate node/driver.rs
        // `descriptor_json`): `OrderDescriptor::spired_uncut` with the recorded
        // priorities; its other fields are the same on every slot, so the slot
        // map moves only the priorities and the two-sided flags (p5l-library).
        let arity = slot_map.len();
        let mut descriptor = if two_sided.iter().any(|&b| b) {
            OrderDescriptor::spired_uncut_two_sided(permuted(&two_sided, slot_map))
        } else {
            OrderDescriptor::spired_uncut(arity)
        };
        descriptor.support_priority = priority("support_priority")?;
        descriptor.coordinate_priority = priority("coordinate_priority")?;
        return Ok(descriptor);
    }
    Ok(OrderDescriptor {
        pre_support_degree_rows: rows("pre_support_degree_rows")?,
        support_weights: permuted(
            &u64_list(&order["support_weights"], "support_weights")?,
            slot_map,
        ),
        support_priority: priority("support_priority")?,
        degree_rows: rows("degree_rows")?,
        coordinate_priority: priority("coordinate_priority")?,
        coordinate_groups: match order["coordinate_groups"].as_str() {
            Some("ActiveFirst") => CoordinateGroups::ActiveFirst,
            Some("InactiveFirst") => CoordinateGroups::InactiveFirst,
            Some("Interleaved") => CoordinateGroups::Interleaved,
            _ => return Err(malformed("order field coordinate_groups")),
        },
        active_direction: direction("active_direction")?,
        inactive_direction: direction("inactive_direction")?,
        two_sided: if two_sided.iter().any(|&b| b) {
            permuted(&two_sided, slot_map)
        } else {
            Vec::new()
        },
    })
}

/// `descriptor` with every slot-indexed field moved by `slot_map`
/// (slot `j` becomes slot `slot_map[j]`).
pub fn permute_descriptor(
    descriptor: &rustred_order::OrderDescriptor,
    slot_map: &[usize],
) -> rustred_order::OrderDescriptor {
    use rustred_order::DegreeRow;
    let rows = |rows: &[DegreeRow]| -> Vec<DegreeRow> {
        rows.iter()
            .map(|row| DegreeRow {
                active: permuted(&row.active, slot_map),
                inactive: permuted(&row.inactive, slot_map),
            })
            .collect()
    };
    let priority = |list: &[usize]| list.iter().map(|&slot| slot_map[slot]).collect();
    rustred_order::OrderDescriptor {
        pre_support_degree_rows: rows(&descriptor.pre_support_degree_rows),
        support_weights: permuted(&descriptor.support_weights, slot_map),
        support_priority: priority(&descriptor.support_priority),
        degree_rows: rows(&descriptor.degree_rows),
        coordinate_priority: priority(&descriptor.coordinate_priority),
        coordinate_groups: descriptor.coordinate_groups,
        active_direction: descriptor.active_direction,
        inactive_direction: descriptor.inactive_direction,
        two_sided: if descriptor.two_sided.is_empty() {
            Vec::new()
        } else {
            permuted(&descriptor.two_sided, slot_map)
        },
    }
}

// ----- the slot order -----

/// vk5l WP2: the revision name of the relation binding (an empty recipe
/// bundle) of a boxed table imported without recipe records.
pub const RELATION_BINDING_REVISION: &str = "relations";

/// Schema of the slot-order record of an imported payload.
pub const SLOT_ORDER_SCHEMA: &str = "rustred.p-table-slot-order.v1";
/// The record is the last line of the payload's provenance text (the program
/// record's `family_source`), after this marker.
pub const SLOT_ORDER_MARKER: &str = "\nslot_order ";

/// How the table family relates to the census family of the node: table slot
/// `census_to_table[i]` carries census slot `i`, its momentum
/// `signs[i] * loop_map * (census momentum)` over `(k_1, .., k_L, q)`
/// (`loop_map` unimodular, `q -> +-q`). Order variants of the 5L repair
/// (`ops/p5l_orders.py`: slot permutations of the same family) have
/// `loop_map = 1`; Astra's D1/D9 swap of T206 re-routes the loops.
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct CensusRelation {
    /// `momenta` (found from the two families) or `order-map` (the map of an
    /// ORDER-MAP.json, checked against the two families).
    pub source: String,
    pub census_to_table: Vec<usize>,
    /// Row `r`: table coordinate `r` as a combination of census coordinates.
    pub loop_map: Vec<Vec<i64>>,
    pub signs: Vec<i8>,
}

/// The slot order the table was generated in, as the payload records it.
/// Load-time rule ([`enforce_slot_order`]): the payload's runtime descent
/// order (`integral_order`) is exactly `table_order` moved by
/// `table_to_presentation` (`SpiredUncutV1` for a `legacy` table whose slots
/// are already the presentation's).
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct SlotOrderRecord {
    pub schema: String,
    /// The table header's `order_name` (`legacy`, `two_sided`, `isp_reverse`, ..).
    pub order_name: String,
    pub order_is_program: bool,
    /// The table header's `family_file` (variant files carry the variant name).
    pub table_family_file: String,
    /// Canonical bytes (hex) of the header's order in TABLE slots.
    pub table_order: String,
    /// Table slot `j` is presentation slot `table_to_presentation[j]`.
    pub table_to_presentation: Vec<usize>,
    /// Stable id of the payload's descent order (= `integral_order`).
    pub presentation_order: String,
    /// Relation to the census family, when the import was given it.
    pub census: Option<CensusRelation>,
}

fn is_permutation(map: &[usize], arity: usize) -> bool {
    let mut seen = vec![false; arity];
    map.len() == arity
        && map
            .iter()
            .all(|&image| image < arity && !std::mem::replace(&mut seen[image], true))
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

fn unhex(text: &str) -> Option<Vec<u8>> {
    text.len()
        .is_multiple_of(2)
        .then(|| {
            (0..text.len())
                .step_by(2)
                .map(|i| u8::from_str_radix(text.get(i..i + 2)?, 16).ok())
                .collect()
        })
        .flatten()
}

/// The descent order a payload must carry for `record` (its stable id).
fn expected_order(record: &SlotOrderRecord, arity: usize) -> Result<String, PError> {
    let fail = |detail: String| malformed(format!("slot order: {detail}"));
    if !is_permutation(&record.table_to_presentation, arity) {
        return Err(fail(
            "table_to_presentation is no permutation of the slots".into(),
        ));
    }
    let bytes = unhex(&record.table_order).ok_or_else(|| fail("table_order is no hex".into()))?;
    let table = rustred_order::CompiledOrder::from_canonical_bytes(
        &bytes,
        rustred_order::Limits::default(),
    )
    .map_err(|error| fail(format!("table_order: {error}")))?;
    if table.arity() != arity {
        return Err(fail("table_order has another arity".into()));
    }
    let identity = record
        .table_to_presentation
        .iter()
        .enumerate()
        .all(|(slot, &image)| slot == image);
    let policy = if record.order_name == "legacy" && identity {
        OrderingPolicy::SpiredUncutV1
    } else {
        let moved = permute_descriptor(table.descriptor(), &record.table_to_presentation);
        let compiled =
            rustred_order::CompiledOrder::compile(moved, rustred_order::Limits::default())
                .map_err(|error| fail(format!("moved order: {error}")))?;
        OrderingPolicy::try_programmed(compiled)
            .map_err(|error| fail(format!("moved order: {error}")))?
    };
    Ok(policy.stable_id().to_string())
}

/// The load-time rule of an imported payload (called on every payload read,
/// [`super::custom`]): if its provenance carries a slot-order record, the
/// payload's descent order `integral_order` must be the table's order moved
/// onto the presentation slots, and the census relation (if any) well formed.
/// Payloads without a record (generated by `p-generate`) pass unchanged.
pub fn enforce_slot_order(
    family_source: &str,
    integral_order: &str,
    arity: usize,
) -> Result<Option<SlotOrderRecord>, PError> {
    let Some(at) = family_source.rfind(SLOT_ORDER_MARKER) else {
        return Ok(None);
    };
    let text = &family_source[at + SLOT_ORDER_MARKER.len()..];
    let record: SlotOrderRecord = serde_json::from_str(text.trim())
        .map_err(|error| malformed(format!("slot order record: {error}")))?;
    if record.schema != SLOT_ORDER_SCHEMA {
        return Err(malformed(format!("slot order schema {:?}", record.schema)));
    }
    if record.presentation_order != integral_order {
        return Err(malformed(format!(
            "slot order: the payload descends in {integral_order}, the record says {}",
            record.presentation_order
        )));
    }
    let expected = expected_order(&record, arity)?;
    if expected != integral_order {
        return Err(malformed(format!(
            "slot order: the table's {} order moved onto the presentation is {expected}, the payload descends in {integral_order}",
            record.order_name
        )));
    }
    if let Some(census) = &record.census {
        let columns = census.loop_map.len();
        if !is_permutation(&census.census_to_table, arity)
            || census.signs.len() != arity
            || census.signs.iter().any(|&s| s != 1 && s != -1)
            || census.loop_map.iter().any(|row| row.len() != columns)
        {
            return Err(malformed("slot order: malformed census relation"));
        }
    }
    Ok(Some(record))
}

/// Integer determinant (Bareiss).
fn determinant(matrix: &[Vec<i64>]) -> Option<i128> {
    let n = matrix.len();
    let mut a: Vec<Vec<i128>> = matrix
        .iter()
        .map(|row| row.iter().map(|&x| i128::from(x)).collect())
        .collect();
    let (mut sign, mut previous) = (1_i128, 1_i128);
    for k in 0..n {
        if a[k][k] == 0 {
            let swap = (k + 1..n).find(|&r| a[r][k] != 0)?;
            a.swap(k, swap);
            sign = -sign;
        }
        for i in k + 1..n {
            for j in k + 1..n {
                a[i][j] =
                    (a[i][j].checked_mul(a[k][k])? - a[i][k].checked_mul(a[k][j])?) / previous;
            }
        }
        previous = a[k][k];
    }
    Some(sign * a[n - 1][n - 1])
}

/// `x` with `basis * x = rhs` (square, f64 Gauss with pivoting).
fn solve(basis: &[Vec<f64>], rhs: &[Vec<f64>]) -> Option<Vec<Vec<f64>>> {
    let n = basis.len();
    let m = rhs[0].len();
    let mut a: Vec<Vec<f64>> = basis
        .iter()
        .zip(rhs)
        .map(|(row, r)| row.iter().chain(r).copied().collect())
        .collect();
    for k in 0..n {
        let pivot = (k..n).max_by(|&x, &y| a[x][k].abs().total_cmp(&a[y][k].abs()))?;
        if a[pivot][k].abs() < 1e-9 {
            return None;
        }
        a.swap(k, pivot);
        for i in 0..n {
            if i != k {
                let factor = a[i][k] / a[k][k];
                for j in k..n + m {
                    a[i][j] -= factor * a[k][j];
                }
            }
        }
    }
    Some(
        (0..n)
            .map(|i| (0..m).map(|j| a[i][n + j] / a[i][i]).collect())
            .collect(),
    )
}

/// The relation of `table` to `census` (see [`CensusRelation`]): the slot
/// map from `order_map` (an ORDER-MAP.json) or, without one, the unique
/// match of equal momenta up to sign; then the loop map, found from a basis
/// of census momenta under every sign choice and checked exactly on every
/// slot (integer, unimodular, `q -> +-q`, lines to lines, ISPs to ISPs).
pub fn census_relation(
    census: &TableFamily,
    table: &TableFamily,
    order_map: Option<&Value>,
) -> Result<CensusRelation, PError> {
    let fail = |detail: String| gap("slot_order", detail);
    let arity = table.slots.len();
    if census.slots.len() != arity || census.loops != table.loops {
        return Err(fail(format!(
            "the census family {} has {} slots / {} loops, the table {} / {}",
            census.name,
            census.slots.len(),
            census.loops,
            arity,
            table.loops
        )));
    }
    let (census_to_table, source) = match order_map.and_then(|map| {
        map.get("original_slot_to_new_order_slot")
            .or_else(|| map.get("original_family_slot_to_table_slot_0based"))
    }) {
        Some(list) => {
            let map: Vec<usize> = list
                .as_array()
                .ok_or_else(|| fail("ORDER-MAP slot map is no list".into()))?
                .iter()
                .map(|v| v.as_u64().map(|v| v as usize))
                .collect::<Option<_>>()
                .ok_or_else(|| fail("ORDER-MAP slot map entries".into()))?;
            (map, "order-map")
        }
        None => {
            let mut map = Vec::with_capacity(arity);
            for slot in &census.slots {
                let negated: Vec<i64> = slot.momentum.iter().map(|c| -c).collect();
                let hits: Vec<usize> = (0..arity)
                    .filter(|&j| {
                        table.slots[j].line == slot.line
                            && (table.slots[j].momentum == slot.momentum
                                || table.slots[j].momentum == negated)
                    })
                    .collect();
                match hits.as_slice() {
                    [j] => map.push(*j),
                    _ => {
                        return Err(fail(format!(
                            "census slot {} matches {} table slots by momentum: an ORDER-MAP is needed",
                            slot.name,
                            hits.len()
                        )));
                    }
                }
            }
            (map, "momenta")
        }
    };
    if !is_permutation(&census_to_table, arity) {
        return Err(fail(
            "the census -> table slot map is no permutation".into(),
        ));
    }
    for (i, &j) in census_to_table.iter().enumerate() {
        if census.slots[i].line != table.slots[j].line {
            return Err(fail(format!(
                "census slot {} and table slot {} differ in kind (line / ISP)",
                census.slots[i].name, table.slots[j].name
            )));
        }
    }
    let width = usize::from(table.loops) + 1;
    let u: Vec<Vec<f64>> = census
        .slots
        .iter()
        .map(|s| s.momentum.iter().map(|&c| c as f64).collect())
        .collect();
    let w: Vec<&Vec<i64>> = census_to_table
        .iter()
        .map(|&j| &table.slots[j].momentum)
        .collect();
    // A basis of census momenta (the rows of `u`); q joins as a unit row
    // if no census momentum is needed for it (it always is: q flows).
    let mut basis: Vec<usize> = Vec::new();
    for i in 0..arity {
        let mut candidate = basis.clone();
        candidate.push(i);
        let rows: Vec<Vec<f64>> = candidate.iter().map(|&b| u[b].clone()).collect();
        // rank test: Gram matrix of the candidate rows is invertible
        let gram: Vec<Vec<f64>> = rows
            .iter()
            .map(|a| {
                rows.iter()
                    .map(|b| a.iter().zip(b).map(|(x, y)| x * y).sum())
                    .collect()
            })
            .collect();
        let identity: Vec<Vec<f64>> = (0..rows.len())
            .map(|r| {
                (0..rows.len())
                    .map(|c| f64::from(u8::from(r == c)))
                    .collect()
            })
            .collect();
        if solve(&gram, &identity).is_some() {
            basis = candidate;
            if basis.len() == width {
                break;
            }
        }
    }
    if basis.len() != width {
        return Err(fail(
            "the census momenta do not span the loop momenta and q".into(),
        ));
    }
    let ub: Vec<Vec<f64>> = basis.iter().map(|&b| u[b].clone()).collect();
    for signs_mask in 0_u32..(1 << width) {
        let rhs: Vec<Vec<f64>> = basis
            .iter()
            .enumerate()
            .map(|(k, &b)| {
                let sign = if signs_mask >> k & 1 == 1 { -1.0 } else { 1.0 };
                w[b].iter().map(|&c| sign * c as f64).collect()
            })
            .collect();
        // Row form: w_i = s_i u_i X, X = loop_map^T.
        let Some(x) = solve(&ub, &rhs) else { continue };
        if x.iter().flatten().any(|v| (v - v.round()).abs() > 1e-6) {
            continue;
        }
        let x: Vec<Vec<i64>> = x
            .iter()
            .map(|row| row.iter().map(|v| v.round() as i64).collect())
            .collect();
        let q_row = &x[width - 1];
        if q_row[..width - 1].iter().any(|&c| c != 0) || q_row[width - 1].abs() != 1 {
            continue;
        }
        if determinant(&x).map(i128::abs) != Some(1) {
            continue;
        }
        let mut signs = Vec::with_capacity(arity);
        for i in 0..arity {
            let image: Vec<i64> = (0..width)
                .map(|c| {
                    (0..width)
                        .map(|r| census.slots[i].momentum[r] * x[r][c])
                        .sum()
                })
                .collect();
            let negated: Vec<i64> = image.iter().map(|c| -c).collect();
            if *w[i] == image {
                signs.push(1);
            } else if *w[i] == negated {
                signs.push(-1);
            } else {
                break;
            }
        }
        if signs.len() == arity {
            let loop_map = (0..width)
                .map(|r| (0..width).map(|c| x[c][r]).collect())
                .collect();
            return Ok(CensusRelation {
                source: source.to_owned(),
                census_to_table,
                loop_map,
                signs,
            });
        }
    }
    Err(fail(format!(
        "no unimodular loop map carries the census family {} onto the table family under the {source} slot map",
        census.name
    )))
}

/// Checks of an ORDER-MAP.json against the table header: family, order name,
/// order descriptor and family text where the map records them, and its
/// descriptor in census slots if given.
fn check_order_map(
    map: &Value,
    header: &Value,
    table_descriptor: &rustred_order::OrderDescriptor,
    relation: Option<&CensusRelation>,
) -> Result<(), PError> {
    let fail = |detail: String| gap("order_map", detail);
    if let Some(family) = map.get("family").and_then(Value::as_str) {
        if family != str_field(header, "family")? {
            return Err(fail(format!(
                "family {family} vs the table's {}",
                header["family"]
            )));
        }
    }
    if let Some(source) = map.get("family_source").and_then(Value::as_str) {
        if source != str_field(header, "family_source")? {
            return Err(fail("family_source differs from the table header's".into()));
        }
    }
    let name = map
        .get("order_name")
        .or_else(|| map.get("native_order_name"))
        .and_then(Value::as_str);
    match name {
        Some(name) if name == str_field(header, "order_name")? => {}
        other => {
            return Err(fail(format!(
                "order name {other:?} vs the table's {}",
                header["order_name"]
            )));
        }
    }
    match map
        .get("order")
        .or_else(|| map.get("native_order_descriptor_in_new_slots"))
    {
        Some(order) if *order == header["order"] => {}
        Some(_) => {
            return Err(fail(
                "the order descriptor differs from the table header's".into(),
            ));
        }
        None => return Err(fail("no order descriptor".into())),
    }
    if let (Some(original), Some(relation)) =
        (map.get("equivalent_descriptor_in_original_slots"), relation)
    {
        let arity = relation.census_to_table.len();
        let mut table_to_census = vec![0; arity];
        for (census, &table) in relation.census_to_table.iter().enumerate() {
            table_to_census[table] = census;
        }
        let ours = rustred_order::CompiledOrder::compile(
            permute_descriptor(table_descriptor, &table_to_census),
            rustred_order::Limits::default(),
        )
        .map_err(|error| fail(format!("order in census slots: {error}")))?;
        let theirs = rustred_order::CompiledOrder::compile(
            permuted_order(original, &(0..arity).collect::<Vec<_>>())?,
            rustred_order::Limits::default(),
        )
        .map_err(|error| fail(format!("ORDER-MAP order in census slots: {error}")))?;
        if ours.canonical_bytes() != theirs.canonical_bytes() {
            return Err(fail(
                "its descriptor in census slots is not the table order moved back".into(),
            ));
        }
    }
    if let (Some(inverse), Some(relation)) = (map.get("new_order_slot_to_original_slot"), relation)
    {
        let inverse: Vec<u64> = u64_list(inverse, "new_order_slot_to_original_slot")?;
        let consistent = inverse.len() == relation.census_to_table.len()
            && relation
                .census_to_table
                .iter()
                .enumerate()
                .all(|(census, &table)| inverse[table] == census as u64);
        if !consistent {
            return Err(fail("its two slot maps are not inverse".into()));
        }
    }
    Ok(())
}

// ----- boxed nodes (boxed-node) -----

/// Schema of the box record of a boxed table (header field `boxed_node`) and
/// of the payload's copy of it.
pub const BOXED_NODE_SCHEMA: &str = "rustred.boxed-node.v1";
/// The box record is a line of the payload's provenance text (the program
/// record's `family_source`), after this marker and before the slot order.
pub const BOXED_NODE_MARKER: &str = "\nboxed_node ";
/// The one box measure: on the chart (every line slot), `D = sum (a_i - 1)`
/// over the lines and `r = sum |a_i|` over the ISPs; every line `a_i >= 1`,
/// every ISP `a_i <= 0`. Invariant under every slot permutation that maps
/// lines to lines and ISPs to ISPs, so the table frame and the presentation
/// frame give the same box.
pub const BOX_MEASURE: &str = "dots-rank-v1";
/// The importer enumerates the box to check its coverage; larger boxes are
/// refused.
pub const MAX_BOX_KEYS: usize = 20_000_000;

/// A finite box of keys on one chart ([`BOX_MEASURE`]): `(D, r_max)` layers
/// and an optional cap on every line power.
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct BoxDefinition {
    pub name: String,
    pub measure: String,
    /// `[D, r_max]`: dots `D` with rank `r <= r_max`; a `D` not listed is outside.
    pub layers: Vec<[u32; 2]>,
    pub cap: Option<i64>,
}

impl BoxDefinition {
    /// `powers` lies in the box on `chart` (`chart[slot]`: a line slot of
    /// the chart).
    pub fn contains(&self, powers: &[i64], chart: &[bool]) -> bool {
        if powers.len() != chart.len() {
            return false;
        }
        let (mut dots, mut rank) = (0_i64, 0_i64);
        for (&power, &line) in powers.iter().zip(chart) {
            if line {
                if power < 1 || self.cap.is_some_and(|cap| power > cap) {
                    return false;
                }
                dots += power - 1;
            } else {
                if power > 0 {
                    return false;
                }
                rank -= power;
            }
        }
        self.layers
            .iter()
            .any(|&[d, r]| i64::from(d) == dots && rank <= i64::from(r))
    }

    /// vk5l WP1: the first bound of the box that `powers` violates on
    /// `chart`, in the order arity, chart, cap, dots, rank (`None`: inside).
    pub fn violation(&self, powers: &[i64], chart: &[bool]) -> Option<super::error::BoxViolation> {
        use super::error::BoxViolation;
        if powers.len() != chart.len() {
            return Some(BoxViolation::Arity {
                found: powers.len(),
                expected: chart.len(),
            });
        }
        let (mut dots, mut rank) = (0_i64, 0_i64);
        for (slot, (&power, &line)) in powers.iter().zip(chart).enumerate() {
            if line && power < 1 || !line && power > 0 {
                return Some(BoxViolation::OffChart { slot, power, line });
            }
            if line {
                dots += power - 1;
            } else {
                rank -= power;
            }
        }
        if let Some((slot, &power)) = (powers.iter().enumerate())
            .find(|&(slot, &power)| chart[slot] && self.cap.is_some_and(|cap| power > cap))
        {
            return Some(BoxViolation::LineCap {
                slot,
                power,
                cap: self.cap.unwrap_or(0),
            });
        }
        let layers: Vec<&[u32; 2]> = (self.layers.iter())
            .filter(|&&[d, _]| i64::from(d) == dots)
            .collect();
        if layers.is_empty() {
            let mut all: Vec<u32> = self.layers.iter().map(|&[d, _]| d).collect();
            all.sort_unstable();
            all.dedup();
            return Some(BoxViolation::Dots { dots, layers: all });
        }
        let r_max = layers.iter().map(|&&[_, r]| r).max().unwrap_or(0);
        if rank > i64::from(r_max) {
            return Some(BoxViolation::Rank {
                dots,
                rank,
                r_max,
                lines: chart.iter().filter(|&&line| line).count(),
            });
        }
        None
    }

    /// Every key of the box on `chart`, at most `limit` of them.
    pub fn keys(&self, chart: &[bool], limit: usize) -> Result<Vec<Vec<i64>>, String> {
        #[allow(clippy::too_many_arguments)]
        fn spread(
            slots: &[usize],
            total: i64,
            bound: Option<i64>,
            sign: i64,
            base: &mut Vec<i64>,
            out: &mut Vec<Vec<i64>>,
            limit: usize,
        ) -> Result<(), String> {
            let Some((&slot, rest)) = slots.split_first() else {
                if total == 0 {
                    if out.len() == limit {
                        return Err(format!("the box has more than {limit} keys"));
                    }
                    out.push(base.clone());
                }
                return Ok(());
            };
            let most = bound.map_or(total, |bound| total.min(bound));
            for amount in 0..=most {
                base[slot] += sign * amount;
                spread(rest, total - amount, bound, sign, base, out, limit)?;
                base[slot] -= sign * amount;
            }
            Ok(())
        }
        let lines: Vec<usize> = (0..chart.len()).filter(|&slot| chart[slot]).collect();
        let isps: Vec<usize> = (0..chart.len()).filter(|&slot| !chart[slot]).collect();
        let corner: Vec<i64> = chart.iter().map(|&line| i64::from(line)).collect();
        let mut layers = self.layers.clone();
        layers.sort_unstable();
        layers.dedup();
        let mut out = Vec::new();
        for [d, r_max] in layers {
            let dot_bound = self.cap.map(|cap| (cap - 1).max(0));
            let mut dotted = Vec::new();
            spread(
                &lines,
                i64::from(d),
                dot_bound,
                1,
                &mut corner.clone(),
                &mut dotted,
                limit,
            )?;
            for base in dotted {
                for r in 0..=i64::from(r_max) {
                    spread(&isps, r, None, -1, &mut base.clone(), &mut out, limit)?;
                }
            }
        }
        out.sort_unstable();
        out.dedup();
        Ok(out)
    }
}

/// The provenance of one `symmetry_group` record of a boxed table: dispatch
/// never reads it (a certified rule is an identity whose right-hand side is
/// applied as is).
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct SymmetryGroupProvenance {
    pub chart: String,
    /// BLAKE3 of the record's JSONL line.
    pub blake3: String,
    pub group_order: Option<u64>,
}

/// What a boxed table declares and what the importer checked: the node's
/// keys inside the box are covered by exact-key point rules or are one of
/// its finitely many terminals ("unknown integral": returned as symbolic
/// residual terminals, never evaluated); a key of the chart outside the box
/// is refused at run time with [`PError::OutsideBox`].
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct BoxedNodeRecord {
    pub schema: String,
    /// The campaign node (`T162`).
    pub node: String,
    pub definition: BoxDefinition,
    /// The chart in presentation slots (every line slot).
    pub chart: Vec<bool>,
    /// Keys of the box (enumerated by the importer).
    pub box_keys: u64,
    pub point_rules: u64,
    /// The terminals (every residual record), presentation slots, ascending.
    pub terminals: Vec<Vec<i64>>,
    /// Terminals whose residual record carries `"boxed_terminal": true`.
    pub flagged_terminals: u64,
    /// Every box key covered, every chart RHS key in the box and covered.
    pub complete: bool,
    pub uncovered_box_keys: u64,
    pub rhs_outside_box: u64,
    pub rhs_without_rule: u64,
    /// Rules generated with symmetry rows in their source (`sym_rows > 0`).
    pub sym_row_rules: u64,
    /// Parametric (non-point: symbolic, face, bank) rules beside the point
    /// rules (header `parametric_after_points` or the import option
    /// `mixed_box`), in ANY order: first match over the chart's rules in table
    /// order (a parametric rule before a point rule wins at the keys it
    /// admits), else [`PError::OutsideBox`] outside the box. The box keys without point rule
    /// or terminal (`uncovered_box_keys`) are left to them; the load-time
    /// check steps every box key ([`super::custom::verify_custom_payload`]).
    #[serde(default)]
    pub parametric_rules: u64,
    pub symmetry_groups: Vec<SymmetryGroupProvenance>,
    /// t162-export, mixed box only: point rules whose target an earlier point
    /// rule already has (unreachable under first match; the earlier one
    /// applies). Not written when zero (payload bytes unchanged).
    #[serde(default, skip_serializing_if = "is_zero")]
    pub shadowed_point_rules: u64,
    /// vk5l WP1: the certified key set of the node (its line
    /// [`CERTIFIED_KEYS_MARKER`] in the provenance holds the keys), when the
    /// import was given one. Not written without one (payload bytes
    /// unchanged).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub certified: Option<CertifiedSummary>,
}

/// vk5l WP1: what a boxed record says about its certified key set
/// ([`super::certified_keys`]).
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct CertifiedSummary {
    pub schema: String,
    pub version: u32,
    /// Keys of the set (terminals included).
    pub keys: u64,
    /// Of them inside the user box (every box key is one).
    pub in_box: u64,
    /// Bytes of the encoding.
    pub bytes: u64,
}

/// vk5l WP1: the certified key set of a boxed payload is the provenance line
/// after this marker (base64 of [`CertifiedKeySet::encode`], presentation
/// frame), after the box record and before the slot order.
pub const CERTIFIED_KEYS_MARKER: &str = "\ncertified_keys ";

/// vk5l WP1: the certified key set of a payload's provenance text, if any.
pub fn certified_keys_of_source(family_source: &str) -> Result<Option<CertifiedKeySet>, PError> {
    let Some(at) = family_source.find(CERTIFIED_KEYS_MARKER) else {
        return Ok(None);
    };
    let text = &family_source[at + CERTIFIED_KEYS_MARKER.len()..];
    let text = text.split('\n').next().unwrap_or("");
    let bytes = super::certified_keys::base64_decode(text)?;
    CertifiedKeySet::decode(&bytes).map(Some)
}

fn is_zero(value: &u64) -> bool {
    *value == 0
}

impl BoxedNodeRecord {
    /// `powers` (presentation slots) lies in the box.
    pub fn contains(&self, powers: &[i64]) -> bool {
        self.definition.contains(powers, &self.chart)
    }

    /// vk5l WP1: the first box bound `powers` violates (`None`: inside).
    pub fn violation(&self, powers: &[i64]) -> Option<super::error::BoxViolation> {
        self.definition.violation(powers, &self.chart)
    }

    /// The box as one line of text (error messages).
    pub fn describe(&self) -> String {
        format!(
            "box {} of node {} ({}: [D, r_max] {:?}, cap {:?}; {} keys, {} terminals)",
            self.definition.name,
            self.node,
            self.definition.measure,
            self.definition.layers,
            self.definition.cap,
            self.box_keys,
            self.terminals.len()
        )
    }
}

/// The box record of a payload's provenance text, if any.
pub fn boxed_node_of_source(family_source: &str) -> Result<Option<BoxedNodeRecord>, PError> {
    let Some(at) = family_source.find(BOXED_NODE_MARKER) else {
        return Ok(None);
    };
    let text = &family_source[at + BOXED_NODE_MARKER.len()..];
    let text = text.split('\n').next().unwrap_or("");
    let record: BoxedNodeRecord = serde_json::from_str(text)
        .map_err(|error| malformed(format!("boxed node record: {error}")))?;
    if record.schema != BOXED_NODE_SCHEMA || record.definition.measure != BOX_MEASURE {
        return Err(malformed(format!(
            "boxed node record schema {:?}, measure {:?}",
            record.schema, record.definition.measure
        )));
    }
    Ok(Some(record))
}

#[derive(serde::Deserialize)]
struct BoxedHeader {
    schema: String,
    node: String,
    chart: String,
    #[serde(rename = "box")]
    definition: BoxDefinition,
    #[serde(default)]
    box_keys: Option<u64>,
    #[serde(default)]
    complete: Option<bool>,
    /// Non-point (parametric, e.g. an existing bank) rules may follow the
    /// point rules (see [`BoxedNodeRecord::parametric_rules`]).
    #[serde(default)]
    parametric_after_points: bool,
}

fn charts_of(header: &Value) -> Result<Vec<String>, PError> {
    header["charts"]
        .as_array()
        .ok_or_else(|| malformed("header without charts"))?
        .iter()
        .map(|chart| {
            chart
                .as_str()
                .map(str::to_owned)
                .ok_or_else(|| malformed("chart"))
        })
        .collect()
}

/// A numeric key (no symbolic slot) of a boxed table.
fn numeric_key(text: &str, arity: usize) -> Result<Vec<i64>, PError> {
    let key = parse_key(text, arity)?;
    if key.iter().any(|&(symbolic, _)| symbolic) {
        return Err(gap(
            "boxed_node",
            format!("a symbolic key {text} in a boxed table"),
        ));
    }
    Ok(key.into_iter().map(|(_, value)| value).collect())
}

/// The checks of a boxed table and its record (see [`BoxedNodeRecord`]),
/// fed record by record (the streaming import keeps only the keys).
struct BoxChecker {
    spec: BoxedHeader,
    arity: usize,
    chart: Vec<bool>,
    targets: std::collections::HashSet<Vec<i64>>,
    chart_rhs: std::collections::HashSet<Vec<i64>>,
    terminals: std::collections::HashSet<Vec<i64>>,
    flagged: u64,
    sym_row_rules: u64,
    symmetry_groups: Vec<SymmetryGroupProvenance>,
    parametric_rules: u64,
    shadowed_point_rules: u64,
    /// vk5l WP1: the certified key set (table frame): the domain of the
    /// node's rules and terminals instead of the layer box.
    certified: Option<CertifiedKeySet>,
    /// vk5l WP2: the explicit keys of the import's recipe records (table
    /// frame): covered by their recipes (solved at the key at run time; the
    /// load check steps them).
    recipe_keys: std::collections::HashSet<Vec<i64>>,
}

fn box_fail(detail: String) -> PError {
    gap("boxed_node", detail)
}

impl BoxChecker {
    fn new(
        value: &Value,
        family: &TableFamily,
        charts: &[String],
        mixed: bool,
        certified: Option<CertifiedKeySet>,
    ) -> Result<Self, PError> {
        let mut spec: BoxedHeader = serde_json::from_value(value.clone())
            .map_err(|error| box_fail(format!("header field boxed_node: {error}")))?;
        spec.parametric_after_points |= mixed;
        if spec.schema != BOXED_NODE_SCHEMA {
            return Err(box_fail(format!("schema {:?}", spec.schema)));
        }
        if spec.definition.measure != BOX_MEASURE {
            return Err(box_fail(format!(
                "box measure {:?}",
                spec.definition.measure
            )));
        }
        if spec.definition.layers.is_empty() || spec.definition.cap.is_some_and(|cap| cap < 1) {
            return Err(box_fail("an empty box".into()));
        }
        if family.eps_units.iter().any(|&units| units != 0) {
            return Err(box_fail(
                "an eps node (not supported for boxed tables)".into(),
            ));
        }
        let chart: Vec<bool> = family.slots.iter().map(|slot| slot.line).collect();
        if charts.len() != 1 || charts[0] != spec.chart || mask_bits(&spec.chart)? != chart {
            return Err(box_fail(format!(
                "the boxed chart {} must be the table's one chart {charts:?}, every line",
                spec.chart
            )));
        }
        Ok(Self {
            spec,
            arity: family.slots.len(),
            chart,
            targets: Default::default(),
            chart_rhs: Default::default(),
            terminals: Default::default(),
            flagged: 0,
            sym_row_rules: 0,
            symmetry_groups: Vec::new(),
            parametric_rules: 0,
            shadowed_point_rules: 0,
            certified,
            recipe_keys: Default::default(),
        })
    }

    /// vk5l WP2: the explicit keys of recipe records: on the chart, inside the
    /// domain (certified set, else box), none a terminal or a point-rule
    /// target (first match would never reach the recipe).
    fn add_recipe_keys(&mut self, records: &[super::recipes::RecipeRecord]) -> Result<(), PError> {
        for record in records {
            let super::recipes::RecipeDomain::Keys(keys) = &record.domain else {
                continue;
            };
            for key in keys {
                if key.len() != self.arity {
                    return Err(box_fail(format!(
                        "recipe {}: a key of {} slots on a table of {}",
                        record.id,
                        key.len(),
                        self.arity
                    )));
                }
                if !self.in_box(key) {
                    return Err(box_fail(format!(
                        "recipe {}: the key {key:?} lies outside {}",
                        record.id,
                        self.domain()
                    )));
                }
                if self.terminals.contains(key) || self.targets.contains(key) {
                    return Err(box_fail(format!(
                        "recipe {}: the key {key:?} is also a terminal or a point-rule target",
                        record.id
                    )));
                }
                self.recipe_keys.insert(key.clone());
            }
        }
        Ok(())
    }

    /// The domain of rule targets, terminals and chart right-hand sides: the
    /// certified set when the import has one, else the box.
    fn in_box(&self, key: &[i64]) -> bool {
        match &self.certified {
            Some(set) => set.contains(key),
            None => self.spec.definition.contains(key, &self.chart),
        }
    }

    fn domain(&self) -> &'static str {
        if self.certified.is_some() {
            "the certified key set"
        } else {
            "the box"
        }
    }

    /// One record of the table and its JSONL line.
    fn feed(&mut self, record: &Value, line: &str) -> Result<(), PError> {
        let arity = self.arity;
        match record["type"].as_str() {
            Some("rule") => {
                let target_text = str_field(record, "target")?;
                let point = case_text(record)?.starts_with("Coordinate(")
                    && record["equations"].as_array().is_some_and(Vec::is_empty)
                    && record["exceptions"].as_array().is_some_and(Vec::is_empty)
                    && matches!(record["dispatch"].as_str(), None | Some("Partition"));
                let fixed: Option<Vec<i64>> = record["fixed"]
                    .as_array()
                    .and_then(|fixed| fixed.iter().map(Value::as_i64).collect());
                let numeric = parse_key(target_text, arity)?
                    .iter()
                    .all(|&(symbolic, _)| !symbolic);
                let is_point =
                    point && numeric && fixed.as_ref() == Some(&numeric_key(target_text, arity)?);
                if !is_point && self.spec.parametric_after_points {
                    // Dispatch reads it; its keys and right-hand side are
                    // checked by the load-time closure check, not here.
                    self.parametric_rules += 1;
                    return Ok(());
                }
                let target = numeric_key(target_text, arity)?;
                if !point || fixed.as_ref() != Some(&target) {
                    return Err(box_fail(format!(
                        "rule {} ({target_text}) is no point rule (Coordinate case fixed at its target, no equations, no exceptions, Partition)",
                        record["index"]
                    )));
                }
                if !self.in_box(&target) {
                    return Err(box_fail(format!(
                        "rule target {target_text} outside {}",
                        self.domain()
                    )));
                }
                if self.targets.contains(&target) {
                    // Mixed box: first match, the earlier point rule applies.
                    if self.spec.parametric_after_points {
                        self.shadowed_point_rules += 1;
                        return Ok(());
                    }
                    return Err(box_fail(format!("two rules for {target_text}")));
                }
                self.targets.insert(target);
                if record["sym_rows"].as_u64().is_some_and(|rows| rows > 0) {
                    self.sym_row_rules += 1;
                }
                for term in record["rhs"]
                    .as_array()
                    .ok_or_else(|| malformed("rule rhs"))?
                {
                    let text = term
                        .get(0)
                        .and_then(Value::as_str)
                        .ok_or_else(|| malformed("rhs key"))?;
                    let key = numeric_key(text, arity)?;
                    let on_chart = key
                        .iter()
                        .zip(&self.chart)
                        .all(|(&power, &line)| (power > 0) == line);
                    if on_chart {
                        self.chart_rhs.insert(key);
                    }
                }
            }
            Some("residual") => {
                let text = str_field(record, "key")?;
                let key = numeric_key(text, arity)?;
                if !self.in_box(&key) {
                    return Err(box_fail(format!(
                        "terminal {text} outside {}",
                        self.domain()
                    )));
                }
                if record["boxed_terminal"].as_bool() == Some(true) {
                    self.flagged += 1;
                }
                self.terminals.insert(key);
            }
            Some("symmetry_group") => {
                let group = match &record["group"] {
                    Value::String(text) => serde_json::from_str(text).unwrap_or(Value::Null),
                    other => other.clone(),
                };
                self.symmetry_groups.push(SymmetryGroupProvenance {
                    chart: record["chart"].as_str().unwrap_or("").to_owned(),
                    blake3: blake3::hash(line.trim().as_bytes()).to_hex().to_string(),
                    group_order: group["group_order"].as_u64(),
                });
            }
            _ => {}
        }
        Ok(())
    }

    /// Coverage and closure; the record in presentation slots.
    /// vk5l WP1: and the certified set in presentation slots.
    fn finish(
        self,
        slot_map: &[usize],
        partial: bool,
    ) -> Result<(BoxedNodeRecord, Option<CertifiedKeySet>), PError> {
        let Self {
            spec,
            chart,
            targets,
            chart_rhs,
            terminals,
            flagged,
            sym_row_rules,
            symmetry_groups,
            parametric_rules,
            shadowed_point_rules,
            certified,
            recipe_keys,
            ..
        } = self;
        let definition = spec.definition;
        let in_box = |key: &[i64]| match &certified {
            Some(set) => set.contains(key),
            None => definition.contains(key, &chart),
        };
        if let Some(key) = terminals.iter().find(|key| targets.contains(*key)) {
            return Err(box_fail(format!(
                "the terminal {key:?} is also a rule target"
            )));
        }
        let covered = |key: &Vec<i64>| {
            targets.contains(key) || terminals.contains(key) || recipe_keys.contains(key)
        };
        let rhs_outside_box = chart_rhs.iter().filter(|key| !in_box(key)).count() as u64;
        let rhs_without_rule = chart_rhs
            .iter()
            .filter(|key| in_box(key) && !covered(key))
            .count() as u64;
        let box_keys = definition
            .keys(&chart, MAX_BOX_KEYS)
            .map_err(|detail| box_fail(detail))?;
        if let Some(declared) = spec.box_keys
            && declared != box_keys.len() as u64
        {
            return Err(box_fail(format!(
                "the header declares {declared} box keys, the box has {}",
                box_keys.len()
            )));
        }
        // vk5l WP1: the certified set is the closure OF the box: it holds
        // every box key and only keys of the chart.
        if let Some(set) = &certified {
            if let Some(key) = box_keys.iter().find(|key| !set.contains(key)) {
                return Err(box_fail(format!(
                    "the box key {key:?} is not in the certified key set ({} keys)",
                    set.len()
                )));
            }
            if let Some(key) = set.keys().find(|key| {
                matches!(
                    definition.violation(key, &chart),
                    Some(
                        super::error::BoxViolation::OffChart { .. }
                            | super::error::BoxViolation::Arity { .. }
                    )
                )
            }) {
                return Err(box_fail(format!(
                    "the certified key {key:?} is not on the boxed chart"
                )));
            }
        }
        // The keys to cover: the certified set, else the box.
        let keys: Vec<Vec<i64>> = match &certified {
            Some(set) => set.keys().collect(),
            None => box_keys.clone(),
        };
        let uncovered: Vec<&Vec<i64>> = keys.iter().filter(|key| !covered(key)).collect();
        // A mixed box leaves the box keys without point rule or terminal to
        // its parametric rules (checked at load by stepping them).
        let left_to_parametric = parametric_rules > 0;
        let complete = rhs_outside_box == 0
            && (left_to_parametric || (uncovered.is_empty() && rhs_without_rule == 0));
        if !complete && (!partial || spec.complete == Some(true)) {
            return Err(box_fail(format!(
                "the box is not closed: {} of {} {} keys without rule or terminal (first {:?}), {rhs_outside_box} chart RHS keys outside it, {rhs_without_rule} in it without rule or terminal{}",
                uncovered.len(),
                keys.len(),
                if certified.is_some() {
                    "certified"
                } else {
                    "box"
                },
                uncovered.first(),
                if partial {
                    "; the header says complete"
                } else {
                    ""
                }
            )));
        }
        let mut presented: Vec<Vec<i64>> = terminals
            .iter()
            .map(|key| permuted(key, slot_map))
            .collect();
        presented.sort_unstable();
        let certified = match certified {
            Some(set) => Some(set.permuted(slot_map, "presentation")?),
            None => None,
        };
        let certified_summary = certified.as_ref().map(|set| CertifiedSummary {
            schema: super::certified_keys::CERTIFIED_KEYS_SCHEMA.to_owned(),
            version: super::certified_keys::P_CERTIFIED_KEYS_VERSION,
            keys: set.len() as u64,
            in_box: box_keys.len() as u64,
            bytes: set.encode().len() as u64,
        });
        let record = BoxedNodeRecord {
            schema: BOXED_NODE_SCHEMA.to_owned(),
            node: spec.node,
            definition,
            chart: permuted(&chart, slot_map),
            box_keys: box_keys.len() as u64,
            point_rules: targets.len() as u64,
            terminals: presented,
            flagged_terminals: flagged,
            complete,
            uncovered_box_keys: uncovered.len() as u64,
            rhs_outside_box,
            rhs_without_rule,
            sym_row_rules,
            symmetry_groups,
            parametric_rules,
            shadowed_point_rules,
            certified: certified_summary,
        };
        Ok((record, certified))
    }
}

/// Options of one import.
#[derive(Clone, Debug)]
pub struct TableImportOptions {
    /// The id of the node presentation in its library.
    pub id: PresentationId,
    /// The DAG node, when the presentation is placed in a DAG.
    pub node: Option<NodeRef>,
    /// The node's LP master count (the false-knock-out guard).
    pub lp_count: u32,
    /// The census family text of the node (`rules_study/classify/families/T*.sym`):
    /// the table's slot order is then related to it ([`census_relation`]) and
    /// the relation recorded; a table that is no relabelling of it is refused.
    pub census_family: Option<String>,
    /// An ORDER-MAP.json of the table (Astra imports): checked against the
    /// header; its slot map is the census relation's.
    pub order_map: Option<Value>,
    /// boxed-node: accept a boxed table whose box is not closed (box keys
    /// with neither rule nor terminal, or chart RHS keys outside the box or
    /// without a rule): a pilot or partial table. Off: such a table is
    /// refused. Ignored for a table without `boxed_node`.
    pub partial_box: bool,
    /// boxed-node (t162-export): accept parametric (non-point) rules in a
    /// boxed table, in any order beside its point rules (first match), as the
    /// header flag `parametric_after_points` does. Off: only the flag does.
    pub mixed_box: bool,
    /// vk5l WP1: the certified key set of a boxed table (its forward
    /// closure, in the TABLE's slot order, e.g. read from a closure
    /// statement with [`CertifiedKeySet::read`]): rule targets, terminals and
    /// chart right-hand sides must lie in it (not in the layer box), it must
    /// hold every box key, and the payload carries it (presentation slots)
    /// for the dispatch check. Ignored for a table without `boxed_node`.
    pub certified_keys: Option<CertifiedKeySet>,
    /// vk5l WP2: recipe records of a boxed table ([`super::recipes`]): the
    /// payload carries them as its recipe bundle (family text and node order
    /// from the table header, the import's slot map); their explicit keys
    /// count as covered. Ignored without one; refused for a table without
    /// `boxed_node`.
    pub recipes: Option<super::recipes::RecipeImport>,
}

/// One table (`table_dump.v1` JSONL text) as the Custom payload of `graph`,
/// the canonical topology of its DAG node (see the module documentation).
pub fn import_table(
    table: &str,
    graph: &PTopo,
    options: &TableImportOptions,
) -> Result<ImportedOwner, PError> {
    import_table_from(table.as_bytes(), graph, options)
}

/// The JSONL records of a table, one at a time (blank lines skipped): the
/// record and its line.
struct TableRecords<R> {
    reader: R,
    line: String,
}

impl<R: std::io::BufRead> TableRecords<R> {
    fn next(&mut self) -> Result<Option<Value>, PError> {
        loop {
            self.line.clear();
            let read = self
                .reader
                .read_line(&mut self.line)
                .map_err(|error| malformed(format!("JSONL: {error}")))?;
            if read == 0 {
                return Ok(None);
            }
            if self.line.trim().is_empty() {
                continue;
            }
            return serde_json::from_str(&self.line)
                .map(Some)
                .map_err(|error| malformed(format!("JSONL: {error}")));
        }
    }
}

/// The checks of one record that do not depend on its content (record kind,
/// deferred counters, zero charts).
fn check_record_kind(record: &Value, boxed: bool) -> Result<(), PError> {
    let kind = record["type"].as_str().unwrap_or("");
    match kind {
        "header" | "zero_sectors" | "chart" | "rule" | "residual" | "chart_end" | "end" => {}
        // Provenance only (Astra's same-order cover-prefix assembly, T206).
        "assembly_provenance" => {}
        // boxed-node: provenance only in a boxed table (its point rules are
        // certified identities; dispatch never reads the group). Refused in
        // any other table, as before.
        "symmetry_group" if boxed => {}
        "chart_error" => return Err(gap("chart_error", record["error"].to_string())),
        other => return Err(gap("record", format!("record type {other:?}"))),
    }
    if kind == "chart_end" {
        for counter in ["deferred_cases", "deferred_branches", "deferred_regions"] {
            if record[counter].as_u64().unwrap_or(0) != 0 {
                return Err(gap(
                    "deferred",
                    format!("chart {}: {counter} = {}", record["chart"], record[counter]),
                ));
            }
        }
    }
    if kind == "chart" && record["zero"] == true {
        return Err(gap(
            "zero_chart",
            format!("chart {} is zero", record["chart"]),
        ));
    }
    Ok(())
}

/// [`import_table`] reading the table record by record (t162-export): only
/// the header, the current record, the box record's key sets and the encoded
/// payload are held, never the table text or its parsed records. Same
/// payload bytes and the same refusals as [`import_table`] (a refusal found
/// only at the end of the table, as a missing `end`, comes after the
/// refusals of the records before it).
pub fn import_table_from<R: std::io::BufRead>(
    reader: R,
    graph: &PTopo,
    options: &TableImportOptions,
) -> Result<ImportedOwner, PError> {
    import_table_impl::<R, std::io::Sink>(reader, graph, options, None)
}

/// vk5l WP6: [`import_table_from`] with bounded memory for a large table: the
/// unique coefficients are kept in `scratch` (an empty read-write file the
/// caller removes afterwards) instead of memory, and the payload is written
/// to `payload` section by section instead of being returned
/// (`ImportedOwner::payload` is empty, `streamed_payload_bytes` its length).
/// The payload bytes are those of [`import_table_from`]; the summary and the
/// meta are read from the payload's structure (program and family sections),
/// which never decodes a coefficient.
pub fn import_table_streamed<R: std::io::BufRead, W: std::io::Write>(
    reader: R,
    graph: &PTopo,
    options: &TableImportOptions,
    scratch: std::fs::File,
    payload: &mut W,
) -> Result<ImportedOwner, PError> {
    import_table_impl(
        reader,
        graph,
        options,
        Some((StreamScratch::Atoms(scratch), payload)),
    )
}

/// vk5l WP6: the scratch storage of a streamed import: the expanded form's
/// atom frames, or the SHARED form's dictionary entries and coefficient parts.
pub enum StreamScratch {
    Atoms(std::fs::File),
    Shared {
        entries: std::fs::File,
        parts: std::fs::File,
    },
}

/// vk5l WP6: [`import_table_streamed`] with the payload's coefficient section
/// in the form `scratch` selects (`Shared`: the section `--coef-form shared`
/// writes, built while the rules are read, without the expanded table).
pub fn import_table_streamed_with<R: std::io::BufRead, W: std::io::Write>(
    reader: R,
    graph: &PTopo,
    options: &TableImportOptions,
    scratch: StreamScratch,
    payload: &mut W,
) -> Result<ImportedOwner, PError> {
    import_table_impl(reader, graph, options, Some((scratch, payload)))
}

fn import_table_impl<R: std::io::BufRead, W: std::io::Write>(
    reader: R,
    graph: &PTopo,
    options: &TableImportOptions,
    stream: Option<(StreamScratch, &mut W)>,
) -> Result<ImportedOwner, PError> {
    trace_rss("start");
    let (scratch, mut sink) = match stream {
        Some((scratch, sink)) => (Some(scratch), Some(sink)),
        None => (None, None),
    };
    let mut records = TableRecords {
        reader,
        line: String::new(),
    };
    let header = records
        .next()?
        .filter(|record| record["type"] == "header")
        .ok_or_else(|| malformed("no header record"))?;
    let header = &header;
    let schema = str_field(header, "schema")?;
    if !schema.starts_with(TABLE_IMPORT_SCHEMA_PREFIX) {
        return Err(gap("schema", schema.to_owned()));
    }
    let boxed_header = header.get("boxed_node").filter(|value| !value.is_null());
    check_record_kind(header, boxed_header.is_some())?;

    let power_shifts: Vec<String> = header["power_shifts"]
        .as_array()
        .ok_or_else(|| malformed("header without power_shifts"))?
        .iter()
        .map(|shift| {
            shift
                .as_str()
                .map(str::to_owned)
                .ok_or_else(|| malformed("power shift"))
        })
        .collect::<Result<_, _>>()?;
    let family = parse_table_family(str_field(header, "family_source")?, &power_shifts)?;
    let arity = family.slots.len();
    let mask = mask_bits(str_field(header, "mask")?)?;
    let line_slots: Vec<usize> = (0..arity).filter(|&slot| family.slots[slot].line).collect();
    if mask.len() != arity || (0..arity).any(|slot| mask[slot] != family.slots[slot].line) {
        return Err(gap(
            "mask",
            format!(
                "the table mask {} is not the family's lines",
                str_field(header, "mask")?
            ),
        ));
    }

    // 1. The presentation.
    let lines: Vec<(Vec<i64>, u8)> = line_slots
        .iter()
        .map(|&slot| (family.slots[slot].momentum.clone(), family.eps_units[slot]))
        .collect();
    let matched = match_lines(graph, &lines).ok_or_else(|| {
        gap(
            "graph",
            format!(
                "the {} lines of {} carry no momentum routing of the node graph",
                lines.len(),
                family.name
            ),
        )
    })?;
    let q_flipped = matched.q_sign == -1;
    let flip_q = |row: &[i64]| -> Vec<i64> {
        let mut row = row.to_vec();
        if q_flipped {
            if let Some(last) = row.last_mut() {
                *last = -*last;
            }
        }
        row
    };
    let mut slot_map = vec![usize::MAX; arity];
    let mut slots = Vec::with_capacity(arity);
    for (canonical_line, (&table_line, &sign)) in
        matched.assignment.iter().zip(&matched.signs).enumerate()
    {
        let table_slot = line_slots[table_line];
        slot_map[table_slot] = canonical_line;
        let momentum: Vec<i64> = flip_q(&family.slots[table_slot].momentum)
            .into_iter()
            .map(|c| c * i64::from(sign))
            .collect();
        slots.push(Slot::Line(SlotLine {
            momentum,
            eps_units: family.eps_units[table_slot],
        }));
    }
    for slot in (0..arity).filter(|&slot| !family.slots[slot].line) {
        slot_map[slot] = slots.len();
        let momentum = flip_q(&family.slots[slot].momentum);
        slots.push(Slot::Isp(SlotIsp {
            left: momentum.clone(),
            right: momentum,
        }));
    }
    let context = CoefficientContext::try_new(["d"]).map_err(|_| PError::ContextMismatch)?;
    let presentation = Presentation::from_slots(
        options.id,
        PresentationKind::Caller,
        &family.name,
        family.loops,
        slots,
        &context,
        options.node.clone(),
    )?;
    debug_assert_eq!(P_EXTERNAL_SQUARE, -1);
    let (graph_key, _) = canonical(graph);

    // 3. The order.
    let order_name = str_field(header, "order_name")?.to_owned();
    let identity = slot_map
        .iter()
        .enumerate()
        .all(|(slot, &image)| slot == image);
    // The header's order in TABLE slots; its canonical bytes are recorded.
    let table_descriptor = permuted_order(&header["order"], &(0..arity).collect::<Vec<_>>())?;
    let table_order = rustred_order::CompiledOrder::compile(
        table_descriptor.clone(),
        rustred_order::Limits::default(),
    )
    .map_err(|error| malformed(format!("order: {error}")))?;
    let table_order_hex = hex(table_order.canonical_bytes());
    if order_name != "legacy"
        && header["order_bytes"]
            .as_str()
            .is_some_and(|recorded| !recorded.is_empty() && recorded != table_order_hex)
    {
        return Err(malformed(
            "the header's order fields differ from its order_bytes",
        ));
    }
    let policy = if order_name == "legacy" && identity {
        OrderingPolicy::SpiredUncutV1
    } else {
        let descriptor = permute_descriptor(&table_descriptor, &slot_map);
        debug_assert_eq!(descriptor, permuted_order(&header["order"], &slot_map)?);
        let compiled =
            rustred_order::CompiledOrder::compile(descriptor, rustred_order::Limits::default())
                .map_err(|error| malformed(format!("order: {error}")))?;
        OrderingPolicy::try_programmed(compiled)
            .map_err(|error| malformed(format!("order: {error}")))?
    };
    // The slot order: relation to the census family, ORDER-MAP checks.
    let census = match &options.census_family {
        Some(source) => {
            let zero_shifts = vec!["0".to_owned(); arity];
            let census_family = parse_table_family(source, &zero_shifts)?;
            Some(census_relation(
                &census_family,
                &family,
                options.order_map.as_ref(),
            )?)
        }
        None => None,
    };
    if let Some(map) = &options.order_map {
        check_order_map(map, header, &table_descriptor, census.as_ref())?;
    }
    let slot_order = SlotOrderRecord {
        schema: SLOT_ORDER_SCHEMA.to_owned(),
        order_name: order_name.clone(),
        order_is_program: header["order_is_program"].as_bool().unwrap_or(false),
        table_family_file: header["family_file"].as_str().unwrap_or("?").to_owned(),
        table_order: table_order_hex,
        table_to_presentation: slot_map.clone(),
        presentation_order: policy.stable_id().to_string(),
        census,
    };
    let slot_order_json = serde_json::to_string(&slot_order).map_err(|error| malformed(error))?;
    // boxed-node: the box record, checked on the table (box, point rules,
    // terminals, closure) while its records are read, in presentation slots.
    let charts = charts_of(header)?;
    let mut box_checker = match boxed_header {
        Some(value) => Some(BoxChecker::new(
            value,
            &family,
            &charts,
            options.mixed_box,
            options.certified_keys.clone(),
        )?),
        None => None,
    };

    // 2. The rules, chart by chart in the header's order.
    let indexed = IndexedCoefficientContext::try_new(&context, "p-table-import", arity)
        .map_err(|error| malformed(error))?;
    let mut coefficients = Coefficients {
        context: &indexed,
        table: match scratch {
            Some(StreamScratch::Atoms(scratch)) => {
                CoefficientTableBuilder::with_spill(BinaryIoLimits::default(), scratch)
            }
            Some(StreamScratch::Shared { entries, parts }) => {
                CoefficientTableBuilder::with_shared_spill(
                    BinaryIoLimits::default(),
                    entries,
                    parts,
                )
            }
            None => CoefficientTableBuilder::new(BinaryIoLimits::default()),
        },
        slot_map: &slot_map,
    };
    let mut sectors: Vec<EncodedSectorRecord> = charts
        .iter()
        .map(|chart| {
            Ok(EncodedSectorRecord {
                sector: permuted(&mask_bits(chart)?, &slot_map),
                ..Default::default()
            })
        })
        .collect::<Result<_, PError>>()?;
    let ordinal = |chart: &str| {
        charts.iter().position(|c| c == chart).ok_or_else(|| {
            malformed(format!(
                "record on chart {chart} outside the header's charts"
            ))
        })
    };
    let mut dispatch = Vec::new();
    let (mut rules, mut rhs_terms, mut negated, mut affine_rules, mut excepted) = (0, 0, 0, 0, 0);
    // slot-permuted alias rules (SlotAliasRecord): per chart, kept in table order beside the payload's rules
    let mut slot_aliases: Vec<Vec<SlotAliasRecord>> = vec![Vec::new(); charts.len()];
    // batch 12a: the table indices of each chart's ordinary rules (the first-match place of positioned recipes)
    let mut rule_indices: Vec<Vec<usize>> = vec![Vec::new(); charts.len()];
    let mut end: Option<Value> = None;
    while let Some(record) = records.next()? {
        let record = &record;
        check_record_kind(record, boxed_header.is_some())?;
        if end.is_none() && record["type"] == "end" {
            end = Some(record.clone());
        }
        if let Some(checker) = &mut box_checker {
            checker.feed(record, &records.line)?;
        }
        match record["type"].as_str() {
            Some("residual") => {
                let sector = ordinal(str_field(record, "chart")?)?;
                let key = parse_key(str_field(record, "key")?, arity)?;
                if key.iter().any(|&(symbolic, _)| symbolic) {
                    return Err(malformed("a symbolic residual"));
                }
                sectors[sector]
                    .finite_residuals
                    .push(integral_record(&key, &slot_map)?);
            }
            Some("rule") => {
                let sector = ordinal(str_field(record, "chart")?)?;
                let index = record["index"]
                    .as_u64()
                    .ok_or_else(|| malformed("rule index"))? as usize;
                if index != sectors[sector].rules + slot_aliases[sector].len() {
                    return Err(malformed(format!(
                        "rule {index} of chart {} out of order",
                        charts[sector]
                    )));
                }
                // a slot-permuted alias rule (rhs key slots naming other slots' indices) is kept as a SlotAliasRecord
                let rhs_values = record["rhs"]
                    .as_array()
                    .ok_or_else(|| malformed("rule rhs"))?;
                let permuted_rhs = rhs_values.iter().any(|term| {
                    term.get(0).and_then(Value::as_str).is_some_and(|text| {
                        parse_key_any(text, arity).is_ok_and(|key| {
                            key.iter().enumerate().any(|(slot, part)| {
                                matches!(part, AliasSlot::Index { slot: other, .. } if *other != slot)
                            })
                        })
                    })
                });
                let (rule_min, rule_max) = bounds_of_record(record, arity)?;
                let rule_fixed: Vec<Option<i64>> = record["fixed"]
                    .as_array()
                    .ok_or_else(|| malformed("rule fixed"))?
                    .iter()
                    .map(Value::as_i64)
                    .collect();
                if rule_fixed.len() != arity {
                    return Err(malformed("rule fixed arity"));
                }
                if permuted_rhs {
                    let alias = slot_alias_record(
                        record,
                        index,
                        &mask_bits(&charts[sector])?,
                        &rule_fixed,
                        &rule_min,
                        &rule_max,
                        arity,
                        &slot_map,
                    )?;
                    slot_aliases[sector].push(SlotAliasRecord {
                        rules_before: sectors[sector].rules,
                        ..alias
                    });
                    continue;
                }
                let case_text = case_text(record)?;
                let equations = record["equations"]
                    .as_array()
                    .ok_or_else(|| malformed("rule equations"))?;
                let kind = if case_text.starts_with("Coordinate(") {
                    if !equations.is_empty() {
                        return Err(malformed("a coordinate case with equations"));
                    }
                    "coordinate"
                } else if case_text.starts_with("Affine(") {
                    if equations.is_empty() {
                        return Err(malformed("an affine case without equations"));
                    }
                    affine_rules += 1;
                    "affine"
                } else {
                    return Err(gap(
                        "case_kind",
                        case_text.chars().take(40).collect::<String>(),
                    ));
                };
                let fixed = record["fixed"]
                    .as_array()
                    .ok_or_else(|| malformed("rule fixed"))?;
                if fixed.len() != arity {
                    return Err(malformed("rule fixed arity"));
                }
                let mut fixed_pairs: Vec<(usize, i16)> = Vec::new();
                for (slot, value) in fixed.iter().enumerate() {
                    if let Some(value) = value.as_i64() {
                        fixed_pairs.push((slot_map[slot], small(value, "fixed value")?));
                    } else if !value.is_null() {
                        return Err(malformed("rule fixed value"));
                    }
                }
                fixed_pairs.sort_unstable();
                let case = CaseRecord {
                    kind: kind.to_owned(),
                    fixed_axes: fixed_pairs.iter().map(|&(axis, _)| axis).collect(),
                    fixed_values: fixed_pairs.iter().map(|&(_, value)| value).collect(),
                    equations: equations
                        .iter()
                        .map(|equation| {
                            coefficients
                                .polynomial(equation.as_str().ok_or_else(|| malformed("equation"))?)
                        })
                        .collect::<Result<_, _>>()?,
                };
                let target_key = parse_key(str_field(record, "target")?, arity)?;
                let target_weight: i64 = target_key.iter().map(|&(_, value)| value).sum();
                let mut rhs = Vec::new();
                for term in record["rhs"]
                    .as_array()
                    .ok_or_else(|| malformed("rule rhs"))?
                {
                    let pair = term
                        .as_array()
                        .filter(|pair| pair.len() == 2)
                        .ok_or_else(|| malformed("rhs term"))?;
                    let key =
                        parse_key(pair[0].as_str().ok_or_else(|| malformed("rhs key"))?, arity)?;
                    if key.iter().zip(&target_key).any(|(a, b)| a.0 != b.0) {
                        return Err(malformed("an RHS key with another symbolic layout"));
                    }
                    let weight: i64 = key.iter().map(|&(_, value)| value).sum();
                    let mut value = coefficients.value(
                        pair[1]
                            .as_str()
                            .ok_or_else(|| malformed("rhs coefficient"))?,
                    )?;
                    if (target_weight - weight).rem_euclid(2) == 1 {
                        value = -value;
                        negated += 1;
                    }
                    rhs.push(TermRecord {
                        integral: integral_record(&key, &slot_map)?,
                        coefficient: coefficients.intern(&value)?,
                    });
                }
                rhs_terms += rhs.len();
                let exceptions = record["exceptions"]
                    .as_array()
                    .ok_or_else(|| malformed("rule exceptions"))?;
                if !exceptions.is_empty() {
                    excepted += 1;
                }
                let mut exclusions: Vec<_> = exceptions
                    .iter()
                    .map(|branch| {
                        branch
                            .as_array()
                            .ok_or_else(|| malformed("exception branch"))?
                            .iter()
                            .map(|equation| {
                                coefficients.polynomial(
                                    equation.as_str().ok_or_else(|| malformed("exception"))?,
                                )
                            })
                            .collect::<Result<Vec<_>, _>>()
                    })
                    .collect::<Result<_, _>>()?;
                // engine line: an optional `domain_bounds` (imported bank rules) becomes extra exclusion branches
                // (node::bounds); absent = nothing added, so every existing package is byte-identical
                let bounds = crate::node::bounds::bound_exclusions(record).map_err(malformed)?;
                if !bounds.is_empty() && exceptions.is_empty() {
                    excepted += 1;
                }
                for branch in bounds {
                    exclusions.push(
                        branch
                            .iter()
                            .map(|equation| coefficients.polynomial(equation))
                            .collect::<Result<Vec<_>, _>>()?,
                    );
                }
                match record["dispatch"].as_str() {
                    None | Some("Partition") => {}
                    Some("AfterBaselinePartitionWholePiece") => dispatch.push(RuleDispatchRecord {
                        sector,
                        rule: index,
                        policy: 1,
                    }),
                    Some(other) => return Err(gap("dispatch", other.to_owned())),
                }
                sectors[sector]
                    .push_rule(&RuleRecord {
                        case,
                        target: integral_record(&target_key, &slot_map)?,
                        rhs,
                        sources: Vec::new(),
                        exclusions,
                    })
                    .map_err(|error| malformed(error))?;
                rule_indices[sector].push(index);
                rules += 1;
            }
            _ => {}
        }
    }
    trace_rss("records read");
    let end = end.ok_or_else(|| gap("partial", "no end record: a partial run"))?;
    if end["chart_errors"].as_u64().unwrap_or(0) != 0 {
        return Err(gap(
            "chart_error",
            format!("{} chart errors", end["chart_errors"]),
        ));
    }
    // batch 12a: an UNBOXED table takes recipe records that stand at a table position (bounded faces dispatched
    // in first match); their place = the ordinary rules of their chart before that table record
    let mut positioned_records: Option<Vec<super::recipes::RecipeRecord>> = None;
    if let Some(recipes) = &options.recipes
        && box_checker.is_none()
    {
        if recipes.records.is_empty()
            || recipes
                .records
                .iter()
                .any(|record| record.position.is_none())
        {
            return Err(gap(
                "recipes",
                "recipe records need a boxed table (header boxed_node), or every record a table position".to_owned(),
            ));
        }
        let mut records = recipes.records.clone();
        for record in &mut records {
            let chart = record.recipe["chart"].as_str().unwrap_or("");
            let sector = charts
                .iter()
                .position(|candidate| candidate == chart)
                .ok_or_else(|| {
                    malformed(format!(
                        "recipe {}: chart {chart} is no chart of the table",
                        record.id
                    ))
                })?;
            let position = record.position.as_mut().expect("checked");
            position.rules_before = rule_indices[sector]
                .iter()
                .filter(|&&index| index < position.index)
                .count();
        }
        positioned_records = Some(records);
    }
    if let (Some(checker), Some(recipes)) = (&mut box_checker, &options.recipes) {
        checker.add_recipe_keys(&recipes.records)?;
    }
    let (boxed, certified) = match box_checker {
        Some(checker) => {
            let (record, certified) = checker.finish(&slot_map, options.partial_box)?;
            (Some(record), certified)
        }
        None => (None, None),
    };
    // vk5l WP2: the recipe bundle (family text and node order of the header).
    let recipe_bundle = match (&options.recipes, &boxed) {
        (Some(recipes), Some(record)) => {
            let family_text = header["original_family_source"]
                .as_str()
                .or_else(|| header["family_source"].as_str())
                .ok_or_else(|| malformed("a recipe bundle needs the header's family source"))?;
            let order = header
                .get("node_order")
                .filter(|order| !order.is_null())
                .cloned()
                .ok_or_else(|| {
                    gap(
                        "recipes",
                        "a recipe bundle needs the table header's node_order".to_owned(),
                    )
                })?;
            Some(super::recipes::RecipeBundle::new(
                record.node.clone(),
                recipes.revision.clone(),
                family_text,
                order,
                slot_map.clone(),
                recipes.dispatch,
                recipes.records.clone(),
            )?)
        }
        // batch 12a: positioned recipes of an unboxed table (node = the table's family)
        (Some(recipes), None) => {
            let family_text = header["original_family_source"]
                .as_str()
                .or_else(|| header["family_source"].as_str())
                .ok_or_else(|| malformed("a recipe bundle needs the header's family source"))?;
            let order = header
                .get("node_order")
                .filter(|order| !order.is_null())
                .cloned()
                .ok_or_else(|| {
                    gap(
                        "recipes",
                        "a recipe bundle needs the table header's node_order".to_owned(),
                    )
                })?;
            let node = header["family"].as_str().ok_or_else(|| {
                malformed("a recipe bundle of an unboxed table needs the header's family")
            })?;
            Some(super::recipes::RecipeBundle::new(
                node.to_owned(),
                recipes.revision.clone(),
                family_text,
                order,
                slot_map.clone(),
                recipes.dispatch,
                positioned_records.clone().expect("checked above"),
            )?)
        }
        // vk5l WP2: a boxed table without recipe records still carries its
        // RELATION BINDING (node, family text and node order of the header,
        // slot map): an empty bundle, so shipped relation entries and the
        // persistent cache serve the keys its rules do not take. A header
        // without a node order keeps the old payload.
        (None, Some(record)) => match (
            header["original_family_source"]
                .as_str()
                .or_else(|| header["family_source"].as_str()),
            header.get("node_order").filter(|order| !order.is_null()),
        ) {
            (Some(family_text), Some(order)) => Some(super::recipes::RecipeBundle::new(
                record.node.clone(),
                RELATION_BINDING_REVISION,
                family_text,
                order.clone(),
                slot_map.clone(),
                super::recipes::RecipeDispatchOrder::RulesFirst,
                Vec::new(),
            )?),
            _ => None,
        },
        _ => None,
    };
    let boxed_provenance = match &boxed {
        Some(record) => format!(
            "{BOXED_NODE_MARKER}{}{}{}",
            serde_json::to_string(record).map_err(|error| malformed(error))?,
            certified.as_ref().map_or_else(String::new, |set| format!(
                "{CERTIFIED_KEYS_MARKER}{}",
                super::certified_keys::base64_encode(&set.encode())
            )),
            match &recipe_bundle {
                Some(bundle) => bundle.provenance_line()?,
                None => String::new(),
            }
        ),
        // batch 12a: an unboxed table's positioned recipes
        None => match &recipe_bundle {
            Some(bundle) => bundle.provenance_line()?,
            None => String::new(),
        },
    };
    dispatch.sort_by_key(|entry| (entry.sector, entry.rule));
    let dispatched_rules = dispatch.len();
    let alias_records: Vec<SlotAliasRecord> = slot_aliases.into_iter().flatten().collect();
    let alias_provenance = if alias_records.is_empty() {
        String::new()
    } else {
        format!(
            "{SLOT_ALIASES_MARKER}{}",
            serde_json::to_string(&alias_records).map_err(|error| malformed(error))?
        )
    };
    // batch 12a: reader features this payload needs; then schema v4, so a reader built before them refuses
    // the payload instead of skipping provenance it does not know (s12349c-era payloads keep v2 / v3)
    let mut features: Vec<&str> = Vec::new();
    if !alias_records.is_empty() {
        features.push("slot_aliases");
    }
    if recipe_bundle.as_ref().is_some_and(|bundle| {
        bundle
            .records
            .iter()
            .any(|record| record.position.is_some())
    }) {
        features.push("positioned_recipes");
    }
    let features_provenance = if features.is_empty() {
        String::new()
    } else {
        format!(
            "{}{}",
            super::bundle_wire::REQUIRED_FEATURES_MARKER,
            features.join(",")
        )
    };
    let root_sector: Vec<bool> = (0..arity)
        .map(|slot| matches!(presentation.slots[slot], Slot::Line(_)))
        .collect();
    let program = ProgramRecord {
        schema: if !features.is_empty() {
            super::bundle_wire::FEATURES_CANDIDATE_BUNDLE_SCHEMA
        } else if dispatch.is_empty() {
            CANDIDATE_BUNDLE_SCHEMA
        } else {
            DISPATCH_CANDIDATE_BUNDLE_SCHEMA
        }
        .to_owned(),
        status: CANDIDATE_BUNDLE_STATUS.to_owned(),
        // Batch 6: a table generated with witness-pole guards (header
        // guard_version 2, D1) stamps its imported payload; absent = 1.
        solver_policy: if header["guard_version"].as_u64()
            == Some(u64::from(super::custom::BUNDLE_GUARD_VERSION))
        {
            format!("{IMPORT_SOLVER_POLICY}{}", super::custom::BUNDLE_GUARD_TAG)
        } else {
            IMPORT_SOLVER_POLICY.to_owned()
        },
        family_source: format!(
            "imported from {} ({}), table family {}, p-table-import v{P_TABLE_IMPORT_VERSION}{boxed_provenance}{alias_provenance}{features_provenance}{SLOT_ORDER_MARKER}{slot_order_json}",
            header["family_file"].as_str().unwrap_or("?"),
            schema,
            family.name
        ),
        input_format: IMPORT_INPUT_FORMAT.to_owned(),
        family_fingerprint: presentation.family.fingerprint().to_owned(),
        root_sector,
        permutation: None,
        integral_order: policy.stable_id().to_string(),
        sectors: Vec::new(),
        rule_dispatch: dispatch,
    };
    let mut table_builder = coefficients.table;
    let family_record = NativeFamilyRecord::from_family(&presentation.family, &mut table_builder)
        .map_err(|error| malformed(format!("family record: {error}")))?;
    if let Some(sink) = sink.as_mut() {
        // Streamed: state, then the atoms copied from the scratch file, then
        // the family and program records; the summary from a skeleton
        // payload (same sections, empty coefficient table).
        let state = table_builder
            .encoded_state()
            .map_err(|error| malformed(format!("coefficient table: {error}")))?;
        let atom_bytes = table_builder.encoded_atom_bytes();
        let program_bytes = encode_program_record_with_sectors(&program, &sectors)
            .map_err(|error| malformed(error))?;
        drop(sectors);
        let family_bytes = bincode::encode_to_vec(&family_record, bincode::config::standard())
            .map_err(|error| malformed(error))?;
        let empty_atoms = CoefficientTableBuilder::new(BinaryIoLimits::default())
            .finish()
            .map_err(|error| malformed(format!("coefficient table: {error}")))?
            .atoms;
        let skeleton = encode_program(
            BinaryProgramKind::Candidates,
            &[
                BinarySection {
                    tag: SectionTag::SYMBOLICA_STATE,
                    bytes: &state,
                },
                BinarySection {
                    tag: SectionTag::COEFFICIENTS,
                    bytes: &empty_atoms,
                },
                BinarySection {
                    tag: SectionTag::FAMILY,
                    bytes: &family_bytes,
                },
                BinarySection {
                    tag: SectionTag::PROGRAM,
                    bytes: &program_bytes,
                },
            ],
            BinaryIoLimits::default(),
        )
        .map_err(|error| malformed(format!("payload: {error}")))?;
        let summary = inspect_custom_payload(&skeleton)?;
        let meta = summary.meta(options.lp_count)?;
        drop(skeleton);
        trace_rss("before streaming the payload");
        let mut builder = Some(table_builder);
        let written = encode_program_streamed(
            &mut **sink,
            BinaryProgramKind::Candidates,
            &[
                (SectionTag::SYMBOLICA_STATE, state.len()),
                (SectionTag::COEFFICIENTS, atom_bytes),
                (SectionTag::FAMILY, family_bytes.len()),
                (SectionTag::PROGRAM, program_bytes.len()),
            ],
            BinaryIoLimits::default(),
            |index, writer| {
                let io = |result: std::io::Result<()>| {
                    result.map_err(|error| {
                        crate::persistence::BinaryIoError::Native(error.to_string())
                    })
                };
                use std::io::Write as _;
                match index {
                    0 => io(writer.write_all(&state)),
                    1 => builder
                        .take()
                        .expect("the coefficient section is written once")
                        .write_atoms(writer),
                    2 => io(writer.write_all(&family_bytes)),
                    _ => io(writer.write_all(&program_bytes)),
                }
            },
        )
        .map_err(|error| malformed(format!("payload: {error}")))?;
        trace_rss("payload streamed");
        return Ok(ImportedOwner {
            presentation,
            payload: Vec::new(),
            meta,
            summary,
            slot_map,
            q_flipped,
            line_signs: matched.signs,
            graph_key,
            table_family: family,
            rules,
            rhs_terms,
            negated_terms: negated,
            dispatched_rules,
            affine_rules,
            rules_with_exceptions: excepted,
            order_name,
            slot_order,
            boxed,
            recipes: recipe_bundle,
            streamed_payload_bytes: Some(written),
        });
    }
    let encoded = table_builder
        .finish()
        .map_err(|error| malformed(format!("coefficient table: {error}")))?;
    trace_rss("coefficient table encoded");
    let program_bytes =
        encode_program_record_with_sectors(&program, &sectors).map_err(|error| malformed(error))?;
    drop(sectors);
    let family_bytes = bincode::encode_to_vec(&family_record, bincode::config::standard())
        .map_err(|error| malformed(error))?;
    let payload = encode_program(
        BinaryProgramKind::Candidates,
        &[
            BinarySection {
                tag: SectionTag::SYMBOLICA_STATE,
                bytes: &encoded.state,
            },
            BinarySection {
                tag: SectionTag::COEFFICIENTS,
                bytes: &encoded.atoms,
            },
            BinarySection {
                tag: SectionTag::FAMILY,
                bytes: &family_bytes,
            },
            BinarySection {
                tag: SectionTag::PROGRAM,
                bytes: &program_bytes,
            },
        ],
        BinaryIoLimits::default(),
    )
    .map_err(|error| malformed(format!("payload: {error}")))?;
    trace_rss("payload encoded");
    let summary = inspect_custom_payload(&payload)?;
    let meta = summary.meta(options.lp_count)?;
    Ok(ImportedOwner {
        presentation,
        payload,
        meta,
        summary,
        slot_map,
        q_flipped,
        line_signs: matched.signs,
        graph_key,
        table_family: family,
        rules,
        rhs_terms,
        negated_terms: negated,
        dispatched_rules,
        affine_rules,
        rules_with_exceptions: excepted,
        order_name,
        slot_order,
        boxed,
        recipes: recipe_bundle,
        streamed_payload_bytes: None,
    })
}

/// The import's slot map as text: `table slot -> presentation slot`.
pub fn slot_map_text(imported: &ImportedOwner) -> BTreeMap<String, usize> {
    imported
        .table_family
        .slots
        .iter()
        .zip(&imported.slot_map)
        .map(|(slot, &image)| (slot.name.clone(), image))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// T178 unboxed F3 (table index 5240): a slot-permuted alias rule becomes a SlotAliasRecord; its image, sign and
    /// domain; a later rule overlapping its domain is detected, a disjoint one is not.
    #[test]
    fn a_rule_without_case_is_read_by_its_equations() {
        let coordinate = serde_json::json!({"type": "rule", "equations": []});
        let affine = serde_json::json!({"type": "rule", "equations": ["n0-n1"]});
        assert!(case_text(&coordinate).unwrap().starts_with("Coordinate("));
        assert!(case_text(&affine).unwrap().starts_with("Affine("));
        assert!(case_text(&serde_json::json!({"type": "rule"})).is_err());
        let given = serde_json::json!({"type": "rule", "case": "Affine(x)", "equations": []});
        assert_eq!(case_text(&given).unwrap(), "Affine(x)");
    }

    #[test]
    fn slot_permuted_alias_rule_f3_of_t178() {
        let record: Value = serde_json::from_str(r#"{"type":"rule","chart":"11111111110000000000","index":5240,
            "case":"Coordinate(CoordinateCase { fixed: [] })",
            "fixed":[1,1,null,1,1,null,1,1,1,null,0,0,0,0,0,0,0,0,0,0],"equations":[],
            "target":"I(1,1,n2,1,1,n5,1,1,1,n9,0,0,0,0,0,0,0,0,0,0)",
            "rhs":[["I(1,n2,1,1,n5,1,1,1,n9,1,0,0,0,0,0,0,0,0,0,0)","1"]],"exceptions":[],
            "domain_bounds":{"min":[null,null,2,null,null,2,null,null,null,2,null,null,null,null,null,null,null,null,null,null],
                             "max":[null,null,null,null,null,null,null,null,null,null,null,null,null,null,null,null,null,null,null,null]},
            "dispatch":"Partition"}"#).unwrap();
        let arity = 20;
        let identity: Vec<usize> = (0..arity).collect();
        let fixed: Vec<Option<i64>> = record["fixed"]
            .as_array()
            .unwrap()
            .iter()
            .map(Value::as_i64)
            .collect();
        let (min, max) = bounds_of_record(&record, arity).unwrap();
        let chart = mask_bits("11111111110000000000").unwrap();
        let alias =
            slot_alias_record(&record, 5240, &chart, &fixed, &min, &max, arity, &identity).unwrap();
        assert_eq!(alias.coefficient, 1);
        let key = [1, 1, 3, 1, 1, 2, 1, 1, 1, 4, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0];
        assert_eq!(
            alias.image(&key).unwrap(),
            vec![1, 3, 1, 1, 2, 1, 1, 1, 4, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0]
        );
        // outside the domain: n2 = 1 (bound), a fixed slot differs, a numerator
        let mut low = key;
        low[2] = 1;
        assert!(alias.image(&low).is_none());
        let mut other = key;
        other[0] = 2;
        assert!(alias.image(&other).is_none());
        let mut numerator = key;
        numerator[12] = -1;
        assert!(alias.image(&numerator).is_none());
        // the provenance line round-trips
        let line = format!(
            "imported{SLOT_ALIASES_MARKER}{}{SLOT_ORDER_MARKER}{{}}",
            serde_json::to_string(&vec![alias.clone()]).unwrap()
        );
        assert_eq!(slot_aliases_of_source(&line).unwrap(), vec![alias]);
        assert!(slot_aliases_of_source("imported").unwrap().is_empty());
        // a declared rhs_slot_maps (implementer's form, in symmetry_origin or top level) is checked, and a record
        // without "case" is read by its equations
        let mut declared = record.clone();
        declared.as_object_mut().unwrap().remove("case");
        let mut map = vec![serde_json::Value::Null; arity];
        map[1] = 2.into();
        map[4] = 5.into();
        map[8] = 9.into();
        declared["symmetry_origin"] = serde_json::json!({"rhs_slot_maps": [map.clone()]});
        assert!(
            slot_alias_record(
                &declared, 5240, &chart, &fixed, &min, &max, arity, &identity
            )
            .is_ok()
        );
        map[4] = 2.into();
        declared["symmetry_origin"] = serde_json::json!({});
        declared["rhs_slot_maps"] = serde_json::json!([map]);
        assert!(
            slot_alias_record(
                &declared, 5240, &chart, &fixed, &min, &max, arity, &identity
            )
            .is_err()
        );
        // not the alias form: two terms, or an index used twice
        let mut two = record.clone();
        two["rhs"] = serde_json::json!([
            ["I(1,n2,1,1,n5,1,1,1,n9,1,0,0,0,0,0,0,0,0,0,0)", "1"],
            ["I(1,n2,1,1,n5,1,1,1,n9,1,0,0,0,0,0,0,0,0,0,0)", "1"]
        ]);
        assert!(
            slot_alias_record(&two, 5240, &chart, &fixed, &min, &max, arity, &identity).is_err()
        );
        let mut twice = record.clone();
        twice["rhs"] = serde_json::json!([["I(1,n2,1,1,n2,1,1,1,n9,1,0,0,0,0,0,0,0,0,0,0)", "1"]]);
        assert!(
            slot_alias_record(&twice, 5240, &chart, &fixed, &min, &max, arity, &identity).is_err()
        );
    }

    #[test]
    fn momenta_and_keys_parse() {
        let loops = vec!["k1".to_owned(), "k2".to_owned()];
        let externals = vec!["q".to_owned()];
        assert_eq!(
            parse_momentum("(-k1-k2+q)^2", &loops, &externals).unwrap(),
            vec![-1, -1, 1]
        );
        assert_eq!(
            parse_momentum("(k2)^2", &loops, &externals).unwrap(),
            vec![0, 1, 0]
        );
        assert_eq!(
            parse_momentum("(2*k1-q)^2", &loops, &externals).unwrap(),
            vec![2, 0, -1]
        );
        assert_eq!(
            parse_key("I(n0,n1+1,2,n3-12)", 4).unwrap(),
            vec![(true, 0), (true, 1), (false, 2), (true, -12)]
        );
        assert!(parse_key("I(n1,n1)", 2).is_err());
        assert_eq!(
            rename_indices("n10/(n1-2*n0+d)", &[2, 0, 1, 3, 4, 5, 6, 7, 8, 9, 10]).unwrap(),
            format!("{INDEX_SYMBOL}10/({INDEX_SYMBOL}0-2*{INDEX_SYMBOL}2+d)")
        );
    }

    fn family(props: &[(&str, u8)]) -> TableFamily {
        let mut source =
            String::from("I(\n  name(T),\n  loops(k1,k2),\n  externals(q),\n  dimension(d),\n");
        for (i, (momentum, power)) in props.iter().enumerate() {
            source.push_str(&format!("  prop(D{},{momentum},{power}),\n", i + 1));
        }
        source.push_str("  gram(q,q,1)\n)\n");
        parse_table_family(&source, &vec!["0".to_owned(); props.len()]).unwrap()
    }

    #[test]
    fn census_relations_of_order_variants() {
        let census = family(&[
            ("(k1)^2", 1),
            ("(-k1-k2)^2", 1),
            ("(k2)^2", 1),
            ("(k1+k2-q)^2", 1),
            ("(-k2+q)^2", 1),
            ("(k1+q)^2", 0),
        ]);
        // A physical-slot permutation of the same family (p5l_orders phys_swap_0_2).
        let swapped = family(&[
            ("(k2)^2", 1),
            ("(-k1-k2)^2", 1),
            ("(k1)^2", 1),
            ("(k1+k2-q)^2", 1),
            ("(-k2+q)^2", 1),
            ("(k1+q)^2", 0),
        ]);
        let relation = census_relation(&census, &swapped, None).unwrap();
        assert_eq!(relation.census_to_table, vec![2, 1, 0, 3, 4, 5]);
        assert_eq!(
            relation.loop_map,
            vec![vec![1, 0, 0], vec![0, 1, 0], vec![0, 0, 1]]
        );
        // A re-routed family (k1 <-> k2, Astra's D1/D9 kind): momenta alone do
        // not match, an ORDER-MAP slot map does, with a nontrivial loop map.
        let rerouted = family(&[
            ("(k2)^2", 1),
            ("(-k1-k2)^2", 1),
            ("(k1)^2", 1),
            ("(k1+k2-q)^2", 1),
            ("(-k1+q)^2", 1),
            ("(k2+q)^2", 0),
        ]);
        assert!(census_relation(&census, &rerouted, None).is_err());
        let map = serde_json::json!({"original_slot_to_new_order_slot": [0, 1, 2, 3, 4, 5]});
        let relation = census_relation(&census, &rerouted, Some(&map)).unwrap();
        assert_eq!(
            relation.loop_map,
            vec![vec![0, 1, 0], vec![1, 0, 0], vec![0, 0, 1]]
        );
        // A map that sends a line onto an ISP is refused.
        let bad = serde_json::json!({"original_slot_to_new_order_slot": [5, 1, 2, 3, 4, 0]});
        assert!(census_relation(&census, &rerouted, Some(&bad)).is_err());
    }

    #[test]
    fn the_slot_order_is_enforced() {
        let order = serde_json::json!({
            "pre_support_degree_rows": [], "support_weights": [0, 0, 0],
            "support_priority": [0, 1, 2],
            "degree_rows": [{"active": [1, 1, 1], "inactive": [1, 1, 1]}, {"active": [0, 0, 0], "inactive": [1, 1, 1]}],
            "coordinate_priority": [2, 1, 0], "coordinate_groups": "ActiveFirst",
            "active_direction": "Descending", "inactive_direction": "Descending", "two_sided": []
        });
        let table = permuted_order(&order, &[0, 1, 2]).unwrap();
        let bytes =
            rustred_order::CompiledOrder::compile(table, rustred_order::Limits::default()).unwrap();
        let mut record = SlotOrderRecord {
            schema: SLOT_ORDER_SCHEMA.to_owned(),
            order_name: "isp_reverse".to_owned(),
            order_is_program: true,
            table_family_file: "T.sym".to_owned(),
            table_order: hex(bytes.canonical_bytes()),
            table_to_presentation: vec![1, 0, 2],
            presentation_order: String::new(),
            census: None,
        };
        record.presentation_order = expected_order(&record, 3).unwrap();
        let source = |record: &SlotOrderRecord| {
            format!(
                "imported{SLOT_ORDER_MARKER}{}",
                serde_json::to_string(record).unwrap()
            )
        };
        let id = record.presentation_order.clone();
        assert!(
            enforce_slot_order(&source(&record), &id, 3)
                .unwrap()
                .is_some()
        );
        assert!(
            enforce_slot_order("generated by p-generate", &id, 3)
                .unwrap()
                .is_none()
        );
        // The payload descending in another order than the record's: refused.
        let identity = expected_order(
            &SlotOrderRecord {
                table_to_presentation: vec![0, 1, 2],
                ..record.clone()
            },
            3,
        )
        .unwrap();
        assert_ne!(identity, id);
        assert!(enforce_slot_order(&source(&record), &identity, 3).is_err());
        // A record whose map does not give its stated order: refused.
        let tampered = SlotOrderRecord {
            table_to_presentation: vec![0, 1, 2],
            ..record.clone()
        };
        assert!(enforce_slot_order(&source(&tampered), &id, 3).is_err());
    }

    // ----- boxed-node -----

    /// The T162 boxes of t162-box (tools/box.py counts): 9 lines, 11 ISPs.
    #[test]
    fn the_t162_boxes_have_their_counted_keys() {
        let chart: Vec<bool> = (0..20).map(|slot| slot < 9).collect();
        let union = BoxDefinition {
            name: "union".into(),
            measure: BOX_MEASURE.into(),
            layers: vec![[0, 1], [1, 2], [2, 2], [3, 2], [4, 2]],
            cap: Some(5),
        };
        let keys = union.keys(&chart, MAX_BOX_KEYS).unwrap();
        assert_eq!(keys.len(), 55_704);
        assert!(keys.iter().all(|key| union.contains(key, &chart)));
        let band = BoxDefinition {
            layers: vec![[2, 0], [3, 1], [4, 2]],
            ..union.clone()
        };
        assert_eq!(band.keys(&chart, MAX_BOX_KEYS).unwrap().len(), 40_635);
        let mut corner = vec![0_i64; 20];
        corner[..9].fill(1);
        assert!(union.contains(&corner, &chart) && !band.contains(&corner, &chart));
        let mut outside = corner.clone();
        outside[9] = -1;
        outside[10] = -1;
        assert!(!union.contains(&outside, &chart));
        outside[0] = 0;
        assert!(!union.contains(&outside, &chart));
        assert!(union.keys(&chart, 1000).is_err());
    }

    const KITE: [&str; 5] = ["(k1)^2", "(-k1-k2)^2", "(k2)^2", "(k1+k2-q)^2", "(-k2+q)^2"];

    fn kite_key(powers: [i64; 5]) -> String {
        let parts: Vec<String> = powers.iter().map(i64::to_string).collect();
        format!("I({})", parts.join(","))
    }

    /// A toy boxed table on the two-loop kite (5 lines, no ISP): the box
    /// `D <= 1` (6 keys), the corner a terminal, one point rule per dotted
    /// key: `I(1 + e_i) = (d-4)/(d-3) I(corner) + (2d-7) I(corner - e_j)`.
    fn kite_table(boxed: bool, skip_rule: Option<usize>, extra: &[serde_json::Value]) -> String {
        let mut source =
            String::from("I(\n  name(KITE),\n  loops(k1,k2),\n  externals(q),\n  dimension(d),\n");
        for (i, momentum) in KITE.iter().enumerate() {
            source.push_str(&format!("  prop(D{},{momentum},1),\n", i + 1));
        }
        source.push_str("  gram(q,q,1)\n)\n");
        let mut header = serde_json::json!({
            "type": "header", "schema": "table_dump.v1", "family": "KITE",
            "family_file": "KITE.sym", "family_source": source,
            "power_shifts": ["0", "0", "0", "0", "0"], "mask": "11111",
            "order_name": "legacy", "order_is_program": false,
            "order": {"support_priority": [0, 1, 2, 3, 4], "coordinate_priority": [0, 1, 2, 3, 4], "two_sided": []},
            "charts": ["11111"],
        });
        if boxed {
            header["boxed_node"] = serde_json::json!({
                "schema": BOXED_NODE_SCHEMA, "node": "KITE", "chart": "11111",
                "box": {"name": "d1", "measure": BOX_MEASURE, "layers": [[0, 0], [1, 0]], "cap": null},
                "box_keys": 6,
            });
        }
        let mut lines = vec![
            header,
            serde_json::json!({"type": "zero_sectors", "scope": "downset", "count": 0, "masks": []}),
            serde_json::json!({"type": "chart", "chart": "11111", "zero": false}),
        ];
        lines.extend(extra.iter().cloned());
        let mut index = 0;
        for i in 0..5 {
            if skip_rule == Some(i) {
                continue;
            }
            let mut target = [1_i64; 5];
            target[i] = 2;
            let mut lower = [1_i64; 5];
            lower[(i + 1) % 5] = 0;
            lines.push(serde_json::json!({
                "type": "rule", "chart": "11111", "index": index,
                "case": format!("Coordinate(CoordinateCase {{ fixed: {target:?} }})"),
                "fixed": target, "equations": [], "target": kite_key(target),
                "rhs": [[kite_key([1; 5]), "(d-4)/(d-3)"], [kite_key(lower), "2*d-7"]],
                "exceptions": [], "dispatch": "Partition", "sym_rows": i % 2,
            }));
            index += 1;
        }
        lines.push(
            serde_json::json!({"type": "residual", "chart": "11111", "key": kite_key([1; 5]),
            "boxed_terminal": true, "status": "unknown integral"}),
        );
        lines.push(
            serde_json::json!({"type": "chart_end", "chart": "11111", "deferred_cases": 0,
            "deferred_branches": 0, "deferred_regions": 0}),
        );
        lines.push(serde_json::json!({"type": "end", "chart_errors": 0, "state": "complete"}));
        lines
            .iter()
            .map(|line| format!("{line}\n"))
            .collect::<String>()
    }

    fn kite_options(partial_box: bool) -> TableImportOptions {
        TableImportOptions {
            id: PresentationId(0),
            node: None,
            lp_count: 1,
            census_family: None,
            order_map: None,
            partial_box,
            mixed_box: false,
            certified_keys: None,
            recipes: None,
        }
    }

    fn kite_graph() -> PTopo {
        canonical_node_graph("0-1 0-2 0-3 1-2 1-3", 2, 3).unwrap().1
    }

    fn symmetry_record() -> serde_json::Value {
        serde_json::json!({"type": "symmetry_group", "chart": "11111", "rows": true,
            "group": {"group_order": 2, "elements": []}})
    }

    /// A boxed table: imported with its box record; the importer's checks
    /// pass; each dotted key reduces to the terminal plus a subsector key
    /// with exactly the table's right-hand side (sign rule `q.q = +1 -> -1`);
    /// the terminal comes back as itself; a key of the chart outside the box
    /// is `PError::OutsideBox` naming the box.
    #[test]
    fn a_boxed_table_imports_reduces_inside_and_refuses_outside_its_box() {
        let text = kite_table(true, None, &[symmetry_record()]);
        let imported = import_table(&text, &kite_graph(), &kite_options(false)).unwrap();
        let boxed = imported.boxed.clone().expect("a boxed import");
        assert_eq!(
            (boxed.box_keys, boxed.point_rules, boxed.terminals.len()),
            (6, 5, 1)
        );
        assert!(boxed.complete && boxed.flagged_terminals == 1 && boxed.sym_row_rules == 2);
        assert_eq!(boxed.symmetry_groups.len(), 1);
        assert_eq!(boxed.symmetry_groups[0].group_order, Some(2));
        assert_eq!(
            super::super::custom::custom_payload_boxed_node(&imported.payload).unwrap(),
            Some(boxed.clone())
        );
        let presentation = &imported.presentation;
        let check = super::super::custom::verify_custom_payload(
            presentation,
            &imported.payload,
            &imported.meta,
        )
        .unwrap();
        // 1 corner + 5 dotted keys inside the box; the 15 D = 2 keys skipped.
        assert_eq!((check.witness_keys, check.terminals), (6, 1));
        let to_presentation = |key: [i64; 5]| permuted(&key, &imported.slot_map);
        let context = presentation.family.coefficient_context();
        let parse = |text: &str| {
            super::super::basis::parse_coefficient(
                context,
                text,
                crate::algebra::ExactAlgebraLimits::default(),
            )
            .unwrap()
        };
        let corner = to_presentation([1; 5]);
        for i in 0..5 {
            let mut target = [1_i64; 5];
            target[i] = 2;
            let mut lower = [1_i64; 5];
            lower[(i + 1) % 5] = 0;
            let replay = super::super::custom::replay_custom_payload(
                presentation,
                &imported.payload,
                &imported.meta,
                &[to_presentation(target)],
            )
            .unwrap()
            .remove(0);
            // |target| - |corner| = 1: negated; |target| - |lower| = 2: kept.
            assert_eq!(
                replay.residuals,
                vec![(corner.clone(), -parse("(d-4)/(d-3)"))]
            );
            assert_eq!(
                replay.boundary,
                vec![(to_presentation(lower), parse("2*d-7"))]
            );
            assert_eq!(replay.steps, 2);
        }
        let terminal = super::super::custom::replay_custom_payload(
            presentation,
            &imported.payload,
            &imported.meta,
            std::slice::from_ref(&corner),
        )
        .unwrap()
        .remove(0);
        assert_eq!(terminal.residuals, vec![(corner.clone(), context.one())]);
        assert!(terminal.boundary.is_empty());
        let outside = to_presentation([3, 1, 1, 1, 1]);
        match super::super::custom::replay_custom_payload(
            presentation,
            &imported.payload,
            &imported.meta,
            std::slice::from_ref(&outside),
        ) {
            Err(PError::OutsideBox {
                key,
                node,
                definition,
                ..
            }) => {
                assert_eq!(key.powers(), outside.as_slice());
                assert_eq!(node, "KITE");
                assert!(definition.contains("[[0, 0], [1, 0]]"), "{definition}");
            }
            other => panic!("expected OutsideBox, got {other:?}"),
        }
    }

    /// vk5l WP1 (a): a certified key set as the domain of a boxed table: a
    /// box `d0` (the corner) whose rules target dotted keys outside it is
    /// refused without a set and imported with one; the set travels in the
    /// payload (presentation frame), the load check steps every certified
    /// key, and a set that misses a rule target, a box key or leaves the
    /// chart is refused. A key outside the set is `OutsideCertifiedSet` with
    /// the violated box bound.
    #[test]
    fn a_certified_key_set_is_the_domain_of_a_boxed_table() {
        use super::super::certified_keys::CertifiedKeySet;
        use super::super::error::BoxViolation;
        use super::super::testing::{boxed_kite_certified_keys, boxed_kite_table_with_box};
        let text = boxed_kite_table_with_box(&[[0, 0]], 1);
        let refused = import_table(&text, &kite_graph(), &kite_options(false)).unwrap_err();
        assert!(refused.to_string().contains("outside the box"), "{refused}");
        let with = |set: CertifiedKeySet| TableImportOptions {
            certified_keys: Some(set),
            recipes: None,
            ..kite_options(false)
        };
        let set = boxed_kite_certified_keys();
        let imported = import_table(&text, &kite_graph(), &with(set.clone())).unwrap();
        let boxed = imported.boxed.clone().unwrap();
        assert!(boxed.complete);
        let summary = boxed.certified.clone().expect("a certified summary");
        assert_eq!((summary.keys, summary.in_box, boxed.box_keys), (6, 1, 1));
        assert_eq!(
            summary.version,
            super::super::certified_keys::P_CERTIFIED_KEYS_VERSION
        );
        let payload_record =
            super::super::custom::custom_payload_boxed_node(&imported.payload).unwrap();
        assert_eq!(payload_record, Some(boxed.clone()));
        let check = super::super::custom::verify_custom_payload(
            &imported.presentation,
            &imported.payload,
            &imported.meta,
        )
        .unwrap();
        assert_eq!((check.certified_keys, check.box_keys_stepped), (6, 5));
        assert_eq!((check.witness_keys, check.terminals), (6, 1));
        let load = super::super::custom::custom_payload_load(
            &imported.presentation,
            &imported.payload,
            &imported.meta,
        )
        .unwrap();
        assert_eq!(
            (load.node.as_deref(), load.certified_keys),
            (Some("KITE"), 6)
        );
        assert!(load.certified_heap_bytes >= 30);
        // A dotted key replays (in the set, outside the box); a doubly dotted
        // one is outside the set.
        let to_presentation = |key: [i64; 5]| permuted(&key, &imported.slot_map);
        let replay = super::super::custom::replay_custom_payload(
            &imported.presentation,
            &imported.payload,
            &imported.meta,
            &[to_presentation([2, 1, 1, 1, 1])],
        )
        .unwrap();
        assert_eq!(replay[0].steps, 2);
        let outside = to_presentation([1, 1, 2, 2, 1]);
        match super::super::custom::replay_custom_payload(
            &imported.presentation,
            &imported.payload,
            &imported.meta,
            std::slice::from_ref(&outside),
        ) {
            Err(PError::OutsideCertifiedSet {
                node,
                key,
                violated,
                certified_keys,
                ..
            }) => {
                assert_eq!((node.as_str(), certified_keys), ("KITE", 6));
                assert_eq!(key.powers(), outside.as_slice());
                assert_eq!(
                    violated.map(|bound| *bound),
                    Some(BoxViolation::Dots {
                        dots: 2,
                        layers: vec![0]
                    })
                );
            }
            other => panic!("expected OutsideCertifiedSet, got {other:?}"),
        }
        // A set without one rule target, without the box key, or with a key
        // off the chart is refused.
        let keys: Vec<Vec<i64>> = set.keys().collect();
        let without = |drop: &[i64]| {
            CertifiedKeySet::new(
                "KITE",
                5,
                "table",
                keys.iter().filter(|key| key.as_slice() != drop).cloned(),
            )
            .unwrap()
        };
        for (set, part) in [
            (without(&[1, 1, 2, 1, 1]), "outside the certified key set"),
            (
                without(&[1, 1, 1, 1, 1]),
                "terminal I(1,1,1,1,1) outside the certified key set",
            ),
            (
                CertifiedKeySet::new(
                    "KITE",
                    5,
                    "table",
                    keys.iter().cloned().chain([vec![1, 1, 1, 1, 0]]),
                )
                .unwrap(),
                "not on the boxed chart",
            ),
        ] {
            let error = import_table(&text, &kite_graph(), &with(set)).unwrap_err();
            assert!(error.to_string().contains(part), "{part}: {error}");
        }
        // The layer violations, one per bound.
        let definition = BoxDefinition {
            name: "B".into(),
            measure: BOX_MEASURE.into(),
            layers: vec![[0, 1], [1, 0]],
            cap: Some(2),
        };
        let chart = [true, true, false];
        assert_eq!(definition.violation(&[1, 1, -1], &chart), None);
        assert_eq!(
            definition.violation(&[1, 1], &chart),
            Some(BoxViolation::Arity {
                found: 2,
                expected: 3
            })
        );
        assert_eq!(
            definition.violation(&[0, 1, 0], &chart),
            Some(BoxViolation::OffChart {
                slot: 0,
                power: 0,
                line: true
            })
        );
        assert_eq!(
            definition.violation(&[1, 3, 0], &chart),
            Some(BoxViolation::LineCap {
                slot: 1,
                power: 3,
                cap: 2
            })
        );
        assert_eq!(
            definition.violation(&[2, 2, 0], &chart),
            Some(BoxViolation::Dots {
                dots: 2,
                layers: vec![0, 1]
            })
        );
        let rank = definition.violation(&[2, 1, -1], &chart).unwrap();
        assert_eq!(
            rank,
            BoxViolation::Rank {
                dots: 1,
                rank: 1,
                r_max: 0,
                lines: 2
            }
        );
        assert_eq!(rank.to_string(), "D = 1: r = 1 > r_max = 0 (E_S = 2)");
    }

    /// Without `boxed_node` nothing changes: the payload provenance is the
    /// pre-boxed-node text (same bytes), no box record, the dispatch selects
    /// the same rules; a `symmetry_group` record is still refused there.
    #[test]
    fn a_table_without_a_box_imports_as_before() {
        let text = kite_table(false, None, &[]);
        let imported = import_table(&text, &kite_graph(), &kite_options(false)).unwrap();
        assert!(imported.boxed.is_none());
        let source = format!(
            "imported from KITE.sym (table_dump.v1), table family KITE, p-table-import v2{SLOT_ORDER_MARKER}{}",
            serde_json::to_string(&imported.slot_order).unwrap()
        );
        let structure = super::super::custom::inspect_custom_payload(&imported.payload).unwrap();
        assert_eq!(structure.rules(), 5);
        assert_eq!(
            super::super::custom::custom_payload_boxed_node(&imported.payload).unwrap(),
            None
        );
        assert!(boxed_node_of_source(&source).unwrap().is_none());
        // The payload's provenance is exactly the old text: the boxed import
        // of the same table differs only by its record line.
        let boxed = import_table(
            &kite_table(true, None, &[]),
            &kite_graph(),
            &kite_options(false),
        )
        .unwrap();
        let record = format!(
            "{BOXED_NODE_MARKER}{}",
            serde_json::to_string(boxed.boxed.as_ref().unwrap()).unwrap()
        );
        assert!(
            imported
                .payload
                .windows(source.len())
                .any(|w| w == source.as_bytes()),
            "the non-boxed payload carries the old provenance text"
        );
        assert!(
            !imported
                .payload
                .windows(BOXED_NODE_MARKER.len())
                .any(|w| w == BOXED_NODE_MARKER.as_bytes())
        );
        let boxed_source = source.replacen(
            SLOT_ORDER_MARKER,
            &format!("{record}{SLOT_ORDER_MARKER}"),
            1,
        );
        assert!(
            boxed
                .payload
                .windows(boxed_source.len())
                .any(|w| w == boxed_source.as_bytes())
        );
        // The non-boxed replay: same reduction as the boxed one.
        let replay = super::super::custom::replay_custom_payload(
            &imported.presentation,
            &imported.payload,
            &imported.meta,
            &[permuted(&[2_i64, 1, 1, 1, 1], &imported.slot_map)],
        )
        .unwrap();
        assert_eq!(replay[0].steps, 2);
        // Outside any rule: the plain typed Uncovered, as before.
        assert!(matches!(
            super::super::custom::replay_custom_payload(
                &imported.presentation,
                &imported.payload,
                &imported.meta,
                &[permuted(&[3_i64, 1, 1, 1, 1], &imported.slot_map)],
            ),
            Err(PError::Uncovered { .. })
        ));
        let refused = import_table(
            &kite_table(false, None, &[symmetry_record()]),
            &kite_graph(),
            &kite_options(false),
        );
        assert!(
            matches!(&refused, Err(PError::Unsupported(text)) if text.contains("symmetry_group")),
            "{refused:?}"
        );
    }

    /// `kite_table(true, Some(0))` with `parametric_after_points` and the
    /// parametric rule `I(n0,1,1,1,1) -> 1/(d-3) I(n0-1,1,1,1,1)` (or the
    /// given right-hand side) after the four point rules (or before them).
    fn mixed_kite_table(rhs: &str, parametric_first: bool) -> String {
        let text = kite_table(true, Some(0), &[]);
        let mut records: Vec<serde_json::Value> = text
            .lines()
            .map(|line| serde_json::from_str(line).unwrap())
            .collect();
        records[0]["boxed_node"]["parametric_after_points"] = serde_json::json!(true);
        let parametric = serde_json::json!({
            "type": "rule", "chart": "11111", "index": 4,
            "case": "Coordinate(CoordinateCase { fixed: [None, Some(1), Some(1), Some(1), Some(1)] })",
            "fixed": [null, 1, 1, 1, 1], "equations": [], "target": "I(n0,1,1,1,1)",
            "rhs": [[rhs, "1/(d-3)"]], "exceptions": [], "dispatch": "Partition",
        });
        let first_rule = records.iter().position(|r| r["type"] == "rule").unwrap();
        let residual = records
            .iter()
            .position(|r| r["type"] == "residual")
            .unwrap();
        if parametric_first {
            for record in records.iter_mut().filter(|r| r["type"] == "rule") {
                record["index"] = serde_json::json!(record["index"].as_u64().unwrap() + 1);
            }
            let mut first = parametric;
            first["index"] = serde_json::json!(0);
            records.insert(first_rule, first);
        } else {
            records.insert(residual, parametric);
        }
        records.iter().map(|r| format!("{r}\n")).collect()
    }

    /// t162-export, mixed box: point rules and parametric rules (an existing
    /// bank) in any order. First match over the table order: the point rule of
    /// a key, else the first parametric rule that applies (also outside the
    /// box), else OutsideBox. The box keys left to the parametric rules are
    /// stepped at load; a parametric rule leaving the box is refused there.
    #[test]
    fn a_boxed_node_may_carry_point_rules_and_parametric_rules() {
        let text = mixed_kite_table("I(n0-1,1,1,1,1)", false);
        let imported = import_table(&text, &kite_graph(), &kite_options(false)).unwrap();
        let boxed = imported.boxed.clone().unwrap();
        assert_eq!(
            (
                boxed.point_rules,
                boxed.parametric_rules,
                boxed.uncovered_box_keys
            ),
            (4, 1, 1)
        );
        assert!(boxed.complete);
        let presentation = &imported.presentation;
        let check = super::super::custom::verify_custom_payload(
            presentation,
            &imported.payload,
            &imported.meta,
        )
        .unwrap();
        assert_eq!(check.box_keys_stepped, 5);
        let to_presentation = |key: [i64; 5]| permuted(&key, &imported.slot_map);
        let replay = |key: [i64; 5]| {
            super::super::custom::replay_custom_payload(
                presentation,
                &imported.payload,
                &imported.meta,
                &[to_presentation(key)],
            )
        };
        // Inside the box by the parametric rule, outside by it too (3 -> 2 -> 1).
        assert_eq!(replay([2, 1, 1, 1, 1]).unwrap()[0].steps, 2);
        assert_eq!(replay([3, 1, 1, 1, 1]).unwrap()[0].steps, 3);
        // A point rule still wins at its key.
        assert_eq!(replay([1, 2, 1, 1, 1]).unwrap()[0].boundary.len(), 1);
        assert!(matches!(
            replay([1, 3, 1, 1, 1]),
            Err(PError::OutsideBox { .. })
        ));
        // A parametric rule whose child leaves the box fails the load check.
        let leaving = mixed_kite_table("I(n0,2,1,1,1)", false);
        let imported = import_table(&leaving, &kite_graph(), &kite_options(false)).unwrap();
        assert!(
            super::super::custom::verify_custom_payload(
                &imported.presentation,
                &imported.payload,
                &imported.meta,
            )
            .is_err()
        );
        // Any order: the parametric rule first gives the same reductions (it
        // admits no key of a point rule here).
        let first = import_table(
            &mixed_kite_table("I(n0-1,1,1,1,1)", true),
            &kite_graph(),
            &kite_options(false),
        )
        .unwrap();
        assert_eq!(first.boxed.as_ref().unwrap().parametric_rules, 1);
        for key in [[2, 1, 1, 1, 1], [3, 1, 1, 1, 1], [1, 2, 1, 1, 1]] {
            let again = super::super::custom::replay_custom_payload(
                &first.presentation,
                &first.payload,
                &first.meta,
                &[permuted(&key, &first.slot_map)],
            )
            .unwrap();
            let before = replay(key).unwrap();
            assert_eq!(
                (again[0].steps, &again[0].residuals, &again[0].boundary),
                (before[0].steps, &before[0].residuals, &before[0].boundary)
            );
        }
        // Without the header flag a parametric rule is refused, as before,
        // unless the import option mixed_box accepts it.
        let flagless = text.replace(
            "\"parametric_after_points\":true",
            "\"parametric_after_points\":false",
        );
        assert_ne!(flagless, text);
        assert!(import_table(&flagless, &kite_graph(), &kite_options(false)).is_err());
        let mixed = TableImportOptions {
            mixed_box: true,
            certified_keys: None,
            recipes: None,
            ..kite_options(false)
        };
        assert_eq!(
            import_table(&flagless, &kite_graph(), &mixed)
                .unwrap()
                .boxed
                .unwrap()
                .parametric_rules,
            1
        );
        // A second point rule for a key (here after the parametric rule) is
        // shadowed in a mixed box: counted, the earlier rule applies.
        let mut records: Vec<serde_json::Value> = text
            .lines()
            .map(|line| serde_json::from_str(line).unwrap())
            .collect();
        let rules: Vec<usize> = (0..records.len())
            .filter(|&i| records[i]["type"] == "rule")
            .collect();
        let mut duplicate = records[rules[0]].clone();
        duplicate["index"] = serde_json::json!(rules.len());
        duplicate["rhs"] = serde_json::json!([[duplicate["rhs"][0][0].clone(), "7"]]);
        records.insert(rules[rules.len() - 1] + 1, duplicate.clone());
        let shadowed_text: String = records.iter().map(|r| format!("{r}\n")).collect();
        let shadowed = import_table(&shadowed_text, &kite_graph(), &kite_options(false)).unwrap();
        assert_eq!(shadowed.boxed.as_ref().unwrap().shadowed_point_rules, 1);
        let key: Vec<i64> = parse_key(duplicate["target"].as_str().unwrap(), 5)
            .unwrap()
            .into_iter()
            .map(|(_, value)| value)
            .collect();
        let key: [i64; 5] = key.try_into().unwrap();
        let again = super::super::custom::replay_custom_payload(
            &shadowed.presentation,
            &shadowed.payload,
            &shadowed.meta,
            &[permuted(&key, &shadowed.slot_map)],
        )
        .unwrap();
        let before = replay(key).unwrap();
        assert_eq!(
            (&again[0].residuals, &again[0].boundary),
            (&before[0].residuals, &before[0].boundary)
        );
        // In a box of point rules only it stays refused.
        let point_only = kite_table(true, None, &[]);
        let mut records: Vec<serde_json::Value> = point_only
            .lines()
            .map(|line| serde_json::from_str(line).unwrap())
            .collect();
        let last_rule = records.iter().rposition(|r| r["type"] == "rule").unwrap();
        let mut duplicate = records[last_rule].clone();
        duplicate["index"] = serde_json::json!(5);
        records.insert(last_rule + 1, duplicate);
        let point_only: String = records.iter().map(|r| format!("{r}\n")).collect();
        assert!(import_table(&point_only, &kite_graph(), &kite_options(false)).is_err());
    }

    /// The streaming import (t162-export): a reader with a tiny buffer, blank
    /// and CRLF lines give the payload bytes of the in-memory import, boxed
    /// or not; a table without its end record is refused after its records.
    #[test]
    fn the_streaming_import_gives_the_same_payload() {
        for boxed in [false, true] {
            let text = kite_table(boxed, None, &[]);
            let whole = import_table(&text, &kite_graph(), &kite_options(false)).unwrap();
            let spaced = text.replace('\n', "\r\n\n  \n");
            let streamed = import_table_from(
                std::io::BufReader::with_capacity(7, spaced.as_bytes()),
                &kite_graph(),
                &kite_options(false),
            )
            .unwrap();
            assert_eq!(streamed.payload, whole.payload);
            assert_eq!(streamed.boxed, whole.boxed);
            assert_eq!(streamed.rules, whole.rules);
            assert_eq!(streamed.rhs_terms, whole.rhs_terms);
            let cut = text
                .lines()
                .filter(|line| {
                    !line.contains("\"type\":\"end\"") && !line.contains("\"type\": \"end\"")
                })
                .collect::<Vec<_>>()
                .join("\n");
            assert!(cut.len() < text.len(), "the toy table has an end record");
            let refused = import_table_from(
                std::io::BufReader::new(cut.as_bytes()),
                &kite_graph(),
                &kite_options(false),
            );
            assert!(
                matches!(&refused, Err(PError::Unsupported(text)) if text.contains("partial")),
                "{refused:?}"
            );
        }
    }

    /// vk5l WP6: the bounded-memory import (coefficients spilled to a scratch
    /// file, payload streamed to a writer) writes the payload bytes of the
    /// in-memory import, boxed or not, with the same summary and meta.
    #[test]
    fn the_spilled_streamed_import_writes_the_same_payload() {
        for boxed in [false, true] {
            let text = kite_table(boxed, None, &[]);
            let whole = import_table(&text, &kite_graph(), &kite_options(false)).unwrap();
            let path = std::env::temp_dir().join(format!(
                "rustred-spill-test-{}-{boxed}.scratch",
                std::process::id()
            ));
            let scratch = std::fs::OpenOptions::new()
                .read(true)
                .write(true)
                .create(true)
                .truncate(true)
                .open(&path)
                .unwrap();
            let mut payload = Vec::new();
            let streamed = import_table_streamed(
                std::io::BufReader::with_capacity(7, text.as_bytes()),
                &kite_graph(),
                &kite_options(false),
                scratch,
                &mut payload,
            )
            .unwrap();
            std::fs::remove_file(&path).unwrap();
            assert!(streamed.payload.is_empty());
            assert_eq!(streamed.streamed_payload_bytes, Some(payload.len() as u64));
            assert_eq!(payload, whole.payload);
            assert_eq!(streamed.summary, whole.summary);
            assert_eq!(streamed.meta, whole.meta);
            assert_eq!(streamed.boxed, whole.boxed);
            assert_eq!(streamed.payload_len(), whole.payload_len());
        }
    }

    /// The boxed checks: an open box is refused unless partial; a partial box
    /// imports and its missing box key is plain `Uncovered` (inside the box);
    /// a non-point rule, a target outside the box, a wrong key count are refused.
    #[test]
    fn a_boxed_table_must_be_a_closed_box_of_point_rules() {
        let open = kite_table(true, Some(2), &[]);
        let refused = import_table(&open, &kite_graph(), &kite_options(false));
        assert!(
            matches!(&refused, Err(PError::Unsupported(text)) if text.contains("not closed")),
            "{refused:?}"
        );
        let partial = import_table(&open, &kite_graph(), &kite_options(true)).unwrap();
        let record = partial.boxed.clone().unwrap();
        assert!(!record.complete);
        assert_eq!((record.uncovered_box_keys, record.point_rules), (1, 4));
        let missing = permuted(&[1_i64, 1, 2, 1, 1], &partial.slot_map);
        assert!(matches!(
            super::super::custom::replay_custom_payload(
                &partial.presentation,
                &partial.payload,
                &partial.meta,
                &[missing],
            ),
            Err(PError::Uncovered { .. })
        ));
        let edit = |from: &str, to: &str| {
            let text = kite_table(true, None, &[]);
            assert!(text.contains(from), "{from}");
            import_table(
                &text.replacen(from, to, 1),
                &kite_graph(),
                &kite_options(true),
            )
        };
        for (from, to, why) in [
            (
                "\"fixed\":[2,1,1,1,1]",
                "\"fixed\":[null,1,1,1,1]",
                "no point rule",
            ),
            ("\"box_keys\":6", "\"box_keys\":7", "box keys"),
            (
                "\"layers\":[[0,0],[1,0]]",
                "\"layers\":[[0,0]]",
                "outside the box",
            ),
            (
                "\"measure\":\"dots-rank-v1\"",
                "\"measure\":\"other\"",
                "measure",
            ),
        ] {
            let result = edit(from, to);
            assert!(
                matches!(&result, Err(PError::Unsupported(text)) if text.contains(why)),
                "{why}: {result:?}"
            );
        }
    }

    #[test]
    fn a_two_loop_eps_family_is_matched_to_its_graph() {
        let source = "I(\n  name(T012e),\n  loops(k1,k2),\n  externals(q),\n  dimension(d),\n  prop(D1,(k1)^2,1),\n  prop(D2,(-k1-k2)^2,1),\n  prop(D3,(k2)^2,1),\n  prop(D4,(k1+k2-q)^2,1),\n  prop(D5,(-k2+q)^2,1),\n  gram(q,q,1)\n  ,power_shift(D1,6-3*d/2)\n)\n";
        let shifts: Vec<String> = ["(12-3*d)/2", "0", "0", "0", "0"]
            .map(str::to_owned)
            .to_vec();
        let family = parse_table_family(source, &shifts).unwrap();
        assert_eq!(family.eps_units, vec![3, 0, 0, 0, 0]);
        // T012 of the census: the kite with three eps units on 0-1.
        let (_, graph) = canonical_node_graph("0-1*** 0-2 0-3 1-2 1-3", 2, 3).unwrap();
        let lines: Vec<_> = family
            .slots
            .iter()
            .zip(&family.eps_units)
            .map(|(slot, &eps)| (slot.momentum.clone(), eps))
            .collect();
        let matched = match_lines(&graph, &lines).expect("a routing");
        let mut seen = matched.assignment.clone();
        seen.sort_unstable();
        assert_eq!(seen, vec![0, 1, 2, 3, 4]);
    }
}
