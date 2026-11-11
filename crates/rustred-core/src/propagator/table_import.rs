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
use std::collections::BTreeMap;
use std::fmt;
use std::sync::Arc;

use serde_json::Value;

use crate::algebra::{Coefficient, CoefficientContext, ExactAlgebraLimits, IndexedCoefficientContext};
use crate::persistence::{
    BinaryIoLimits, BinaryProgramKind, BinarySection, CoefficientTableBuilder, NativeFamilyRecord,
    SectionTag, encode_program,
};
use crate::sector::OrderingPolicy;

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
use super::PresentationId;

/// Schemas this importer reads (the header's `schema`); every later variant of
/// the campaign (`-defer-gsplit`) keeps the v1 records.
pub const TABLE_IMPORT_SCHEMA_PREFIX: &str = "table_dump.v1";
/// Version of the import rules above; recorded in every import report.
pub const P_TABLE_IMPORT_VERSION: u32 = 1;
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
    parts.into_iter().map(|part| part.trim().to_owned()).collect()
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
    if externals.len() != 1 || gram.as_deref() != Some(&[externals[0].clone(), externals[0].clone(), "1".to_owned()][..]) {
        return Err(gap(
            "kinematics",
            format!("externals {externals:?}, gram {gram:?}: one external with q.q = 1 is required"),
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
            return Err(gap("power_shift", format!("an eps shift on the ISP {}", slot.name)));
        }
        eps_units.push(units);
    }
    let loops_count =
        u8::try_from(loops.len()).map_err(|_| malformed("too many loop momenta"))?;
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
        lines.push((u, v, u8::try_from(stars).map_err(|_| malformed("eps units"))?));
    }
    let topo = PTopo::new(n + 1, s, t, lines);
    canonical_topology(&topo)
}

/// The normalised, canonically labelled form of `topo` and its key.
pub fn canonical_topology(topo: &PTopo) -> Result<(GraphKey, PTopo), PError> {
    let Normalised::Topo(normal, _) = normalise(topo) else {
        return Err(malformed("the node graph normalises to zero or the trivial node"));
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
                    let ok = (!complete(u) || self.vertex_ok(u)) && (!complete(v) || self.vertex_ok(v));
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
        return Err(malformed(format!("key {text:?} has {} slots, not {arity}", parts.len())));
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
                    return Err(gap("key_index", format!("slot {slot} of {text:?} names n{index}")));
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
                Ok((false, part.parse::<i64>().map_err(|_| malformed(format!("key {text:?}")))?))
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
            let digits = bytes[i + 1..].iter().take_while(|b| b.is_ascii_digit()).count();
            let after = i + 1 + digits;
            let identifier_continues =
                after < bytes.len() && (bytes[after].is_ascii_alphanumeric() || bytes[after] == b'_');
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
        .map(|entry| entry.as_u64().ok_or_else(|| malformed(format!("order field {field}"))))
        .collect()
}

/// The header's `order` as an [`OrderDescriptor`] with every slot permuted.
fn permuted_order(order: &Value, slot_map: &[usize]) -> Result<rustred_order::OrderDescriptor, PError> {
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
    Ok(OrderDescriptor {
        pre_support_degree_rows: rows("pre_support_degree_rows")?,
        support_weights: permuted(&u64_list(&order["support_weights"], "support_weights")?, slot_map),
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

/// Options of one import.
#[derive(Clone, Debug)]
pub struct TableImportOptions {
    /// The id of the node presentation in its library.
    pub id: PresentationId,
    /// The DAG node, when the presentation is placed in a DAG.
    pub node: Option<NodeRef>,
    /// The node's LP master count (the false-knock-out guard).
    pub lp_count: u32,
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
        return Err(gap("chart_error", format!("{} chart errors", end["chart_errors"])));
    }
    for record in &records {
        let kind = record["type"].as_str().unwrap_or("");
        match kind {
            "header" | "zero_sectors" | "chart" | "rule" | "residual" | "chart_end" | "end" => {}
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
            return Err(gap("zero_chart", format!("chart {} is zero", record["chart"])));
        }
    }

    let power_shifts: Vec<String> = header["power_shifts"]
        .as_array()
        .ok_or_else(|| malformed("header without power_shifts"))?
        .iter()
        .map(|shift| shift.as_str().map(str::to_owned).ok_or_else(|| malformed("power shift")))
        .collect::<Result<_, _>>()?;
    let family = parse_table_family(str_field(header, "family_source")?, &power_shifts)?;
    let arity = family.slots.len();
    let mask = mask_bits(str_field(header, "mask")?)?;
    let line_slots: Vec<usize> = (0..arity).filter(|&slot| family.slots[slot].line).collect();
    if mask.len() != arity || (0..arity).any(|slot| mask[slot] != family.slots[slot].line) {
        return Err(gap(
            "mask",
            format!("the table mask {} is not the family's lines", str_field(header, "mask")?),
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
    let identity = slot_map.iter().enumerate().all(|(slot, &image)| slot == image);
    let policy = if order_name == "legacy" && identity {
        OrderingPolicy::SpiredUncutV1
    } else {
        let descriptor = permuted_order(&header["order"], &slot_map)?;
        let compiled = rustred_order::CompiledOrder::compile(descriptor, rustred_order::Limits::default())
            .map_err(|error| malformed(format!("order: {error}")))?;
        if order_name != "legacy" {
            // The header's canonical bytes must be the unpermuted descriptor.
            let unpermuted = permuted_order(&header["order"], &(0..arity).collect::<Vec<_>>())?;
            let bytes = rustred_order::CompiledOrder::compile(unpermuted, rustred_order::Limits::default())
                .map_err(|error| malformed(format!("order: {error}")))?;
            let hex: String = bytes.canonical_bytes().iter().map(|b| format!("{b:02x}")).collect();
            if header["order_bytes"].as_str().is_some_and(|recorded| !recorded.is_empty() && recorded != hex) {
                return Err(malformed("the header's order fields differ from its order_bytes"));
            }
        }
        OrderingPolicy::try_programmed(compiled).map_err(|error| malformed(format!("order: {error}")))?
    };

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
        .map(|chart| chart.as_str().map(str::to_owned).ok_or_else(|| malformed("chart")))
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
        charts
            .iter()
            .position(|c| c == chart)
            .ok_or_else(|| malformed(format!("record on chart {chart} outside the header's charts")))
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
                sectors[sector].finite_residuals.push(integral_record(&key, &slot_map)?);
            }
            Some("rule") => {
                let sector = ordinal(str_field(record, "chart")?)?;
                let index = record["index"].as_u64().ok_or_else(|| malformed("rule index"))? as usize;
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
                    return Err(gap("case_kind", case_text.chars().take(40).collect::<String>()));
                };
                let fixed = record["fixed"].as_array().ok_or_else(|| malformed("rule fixed"))?;
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
                            coefficients.polynomial(
                                equation.as_str().ok_or_else(|| malformed("equation"))?,
                            )
                        })
                        .collect::<Result<_, _>>()?,
                };
                let target_key = parse_key(str_field(record, "target")?, arity)?;
                let target_weight: i64 = target_key.iter().map(|&(_, value)| value).sum();
                let mut rhs = Vec::new();
                for term in record["rhs"].as_array().ok_or_else(|| malformed("rule rhs"))? {
                    let pair = term.as_array().filter(|pair| pair.len() == 2).ok_or_else(|| malformed("rhs term"))?;
                    let key = parse_key(pair[0].as_str().ok_or_else(|| malformed("rhs key"))?, arity)?;
                    if key.iter().zip(&target_key).any(|(a, b)| a.0 != b.0) {
                        return Err(malformed("an RHS key with another symbolic layout"));
                    }
                    let weight: i64 = key.iter().map(|&(_, value)| value).sum();
                    let mut value = coefficients.value(pair[1].as_str().ok_or_else(|| malformed("rhs coefficient"))?)?;
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
            "imported from {} ({}), table family {}, p-table-import v{P_TABLE_IMPORT_VERSION}",
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
        assert_eq!(parse_momentum("(-k1-k2+q)^2", &loops, &externals).unwrap(), vec![-1, -1, 1]);
        assert_eq!(parse_momentum("(k2)^2", &loops, &externals).unwrap(), vec![0, 1, 0]);
        assert_eq!(parse_momentum("(2*k1-q)^2", &loops, &externals).unwrap(), vec![2, 0, -1]);
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

    #[test]
    fn a_two_loop_eps_family_is_matched_to_its_graph() {
        let source = "I(\n  name(T012e),\n  loops(k1,k2),\n  externals(q),\n  dimension(d),\n  prop(D1,(k1)^2,1),\n  prop(D2,(-k1-k2)^2,1),\n  prop(D3,(k2)^2,1),\n  prop(D4,(k1+k2-q)^2,1),\n  prop(D5,(-k2+q)^2,1),\n  gram(q,q,1)\n  ,power_shift(D1,6-3*d/2)\n)\n";
        let shifts: Vec<String> = ["(12-3*d)/2", "0", "0", "0", "0"].map(str::to_owned).to_vec();
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
