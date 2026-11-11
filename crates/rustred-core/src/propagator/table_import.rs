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
    SectionTag, encode_program,
};
use crate::sector::OrderingPolicy;

use super::PresentationId;
use super::bundle_wire::{
    CANDIDATE_BUNDLE_SCHEMA, CANDIDATE_BUNDLE_STATUS, CaseRecord, DISPATCH_CANDIDATE_BUNDLE_SCHEMA,
    IntegralRecord, ProgramRecord, RuleDispatchRecord, RuleRecord, SectorRecord, TermRecord,
    encode_program_record,
};
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
fn permuted_order(
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
}

/// One table (`table_dump.v1` JSONL text) as the Custom payload of `graph`,
/// the canonical topology of its DAG node (see the module documentation).
pub fn import_table(
    table: &str,
    graph: &PTopo,
    options: &TableImportOptions,
) -> Result<ImportedOwner, PError> {
    let records: Vec<Value> = table
        .lines()
        .filter(|line| !line.trim().is_empty())
        .map(serde_json::from_str)
        .collect::<Result<_, _>>()
        .map_err(|error| malformed(format!("JSONL: {error}")))?;
    let header = records
        .first()
        .filter(|record| record["type"] == "header")
        .ok_or_else(|| malformed("no header record"))?;
    let schema = str_field(header, "schema")?;
    if !schema.starts_with(TABLE_IMPORT_SCHEMA_PREFIX) {
        return Err(gap("schema", schema.to_owned()));
    }
    let end = records
        .iter()
        .find(|record| record["type"] == "end")
        .ok_or_else(|| gap("partial", "no end record: a partial run"))?;
    if end["chart_errors"].as_u64().unwrap_or(0) != 0 {
        return Err(gap(
            "chart_error",
            format!("{} chart errors", end["chart_errors"]),
        ));
    }
    for record in &records {
        let kind = record["type"].as_str().unwrap_or("");
        match kind {
            "header" | "zero_sectors" | "chart" | "rule" | "residual" | "chart_end" | "end" => {}
            // Provenance only (Astra's same-order cover-prefix assembly, T206).
            "assembly_provenance" => {}
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
    }

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

    // 2. The rules, chart by chart in the header's order.
    let indexed = IndexedCoefficientContext::try_new(&context, "p-table-import", arity)
        .map_err(|error| malformed(error))?;
    let mut coefficients = Coefficients {
        context: &indexed,
        table: CoefficientTableBuilder::new(BinaryIoLimits::default()),
        slot_map: &slot_map,
    };
    let charts: Vec<String> = header["charts"]
        .as_array()
        .ok_or_else(|| malformed("header without charts"))?
        .iter()
        .map(|chart| {
            chart
                .as_str()
                .map(str::to_owned)
                .ok_or_else(|| malformed("chart"))
        })
        .collect::<Result<_, _>>()?;
    let mut sectors: Vec<SectorRecord> = charts
        .iter()
        .map(|chart| {
            Ok(SectorRecord {
                sector: permuted(&mask_bits(chart)?, &slot_map),
                rules: Vec::new(),
                finite_residuals: Vec::new(),
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
    for record in &records {
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
                if index != sectors[sector].rules.len() {
                    return Err(malformed(format!(
                        "rule {index} of chart {} out of order",
                        charts[sector]
                    )));
                }
                let case_text = str_field(record, "case")?;
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
                let exclusions = exceptions
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
                match record["dispatch"].as_str() {
                    None | Some("Partition") => {}
                    Some("AfterBaselinePartitionWholePiece") => dispatch.push(RuleDispatchRecord {
                        sector,
                        rule: index,
                        policy: 1,
                    }),
                    Some(other) => return Err(gap("dispatch", other.to_owned())),
                }
                sectors[sector].rules.push(RuleRecord {
                    case,
                    target: integral_record(&target_key, &slot_map)?,
                    rhs,
                    sources: Vec::new(),
                    exclusions,
                });
                rules += 1;
            }
            _ => {}
        }
    }
    dispatch.sort_by_key(|entry| (entry.sector, entry.rule));
    let dispatched_rules = dispatch.len();
    let root_sector: Vec<bool> = (0..arity)
        .map(|slot| matches!(presentation.slots[slot], Slot::Line(_)))
        .collect();
    let program = ProgramRecord {
        schema: if dispatch.is_empty() {
            CANDIDATE_BUNDLE_SCHEMA
        } else {
            DISPATCH_CANDIDATE_BUNDLE_SCHEMA
        }
        .to_owned(),
        status: CANDIDATE_BUNDLE_STATUS.to_owned(),
        solver_policy: IMPORT_SOLVER_POLICY.to_owned(),
        family_source: format!(
            "imported from {} ({}), table family {}, p-table-import v{P_TABLE_IMPORT_VERSION}{SLOT_ORDER_MARKER}{slot_order_json}",
            header["family_file"].as_str().unwrap_or("?"),
            schema,
            family.name
        ),
        input_format: IMPORT_INPUT_FORMAT.to_owned(),
        family_fingerprint: presentation.family.fingerprint().to_owned(),
        root_sector,
        permutation: None,
        integral_order: policy.stable_id().to_string(),
        sectors,
        rule_dispatch: dispatch,
    };
    let mut table_builder = coefficients.table;
    let family_record = NativeFamilyRecord::from_family(&presentation.family, &mut table_builder)
        .map_err(|error| malformed(format!("family record: {error}")))?;
    let encoded = table_builder
        .finish()
        .map_err(|error| malformed(format!("coefficient table: {error}")))?;
    let program_bytes = encode_program_record(&program).map_err(|error| malformed(error))?;
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
