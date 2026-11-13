//! The node comparator C on concrete integer keys (`&[i32]`), exactly as the RustRed engine evaluates it.
//!
//! # DIRECTION (read this first)
//!
//! **In this module `Ordering::Less` means: `a` is LOWER (simpler) than `b`.** A rule `target -> sum c_k I_k` descends
//! iff `lower(I_k, target)` for every right-hand-side key, i.e. `cmp(I_k, target) == Less`.
//!
//! This is the orientation of `rustred-order` (`compare.rs`, "simpler-first") and of the checker's `rankC` ("smaller
//! = lower"). It is the OPPOSITE of the engine's `IntegralOrder::compare`
//! (`crates/rustred-core/src/solver/index.rs:358-363`: "Returns `Less` when `left` is harder"), whose program path
//! reverses the `rustred-order` result (`index.rs:375-384`) and whose descent check reads
//! `compare(target, term) == Less` as "term lower" (`node/descent.rs:120-139`). Nothing here inherits that convention.
//!
//! # The two engine paths
//!
//! * **Program orders** (`Engine::Program`): a `rustred_order::CompiledOrder`, compared with
//!   `CompiledOrder::compare` on concrete keys (the crate is used, not re-implemented). On concrete keys this is the
//!   engine's `compare_mixed` with an all-false symbolic mask (`index.rs:364-385`), un-reversed.
//! * **Legacy non-program order** (`Engine::Legacy`): the SpIRed `intLessStatic` aggregate ported from
//!   `index.rs:386-491` (no cuts: the node driver never builds a cut order, `descent.rs:66-69`): number of
//!   denominators, sector lexicographic order, absolute sum, numerator sum, then coordinate ties (denominators by
//!   increasing power = larger power lower, then numerators by decreasing power = larger |n| lower) in slot order or
//!   in an optional tie permutation (`index.rs:196-219`, `448-453`).
//!
//! On concrete keys the legacy order with tie permutation `p` equals the program `OrderDescriptor::spired_uncut(n)`
//! with `coordinate_priority = p` (absolute sum = total excess + support count on a common support; numerator sum =
//! the second degree row); the differential tests `order_differential_*` check this on random keys.
//!
//! # Semantics on all integer keys
//!
//! The order is positional and frame-free: a slot is ACTIVE iff its index is > 0, whether it is a line or a numerator
//! slot. Consequently (both paths):
//! * fewer active slots is lower: a subsector key (a line at index <= 0) is lower than every key of a larger support,
//!   in particular every top-sector key; a key with a positive numerator index has one more active slot and is HIGHER
//!   than the chart key with that numerator at 0 and the same lines;
//! * keys of different supports with the same count are decided by the support bits in support priority (legacy:
//!   slot order): at the first differing slot the key WITHOUT the active slot is lower;
//! * on a common support: total excess (D + r on the chart), then numerator excess (r), then the ties.
//!
//! Every `i32` key is accepted: sums are taken in `i64`/`i128`, so the order is total on all of Z^n. The engine stores
//! powers in 7 signed bits (`index.rs:32-53`, -64..=63); [`NodeComparator::check_engine_range`] reports keys the
//! engine could not represent, it does not change the order. Comparing keys of different arity is a caller error:
//! [`NodeComparator::try_cmp`] returns [`OrderError::Arity`], [`NodeComparator::cmp`] panics.
//!
//! Symbolic patterns (`I(n0, n1+1, 0, ...)`) are compared with [`NodeComparator::cmp_pattern`], which ports the
//! engine's symbolic branches; where the engine asserts (different symbolic patterns, `index.rs:366-369` and
//! `394-398`) it returns [`OrderError::PatternMismatch`] instead of guessing.
//!
//! # Header selection
//!
//! [`NodeComparator::from_header`] reproduces `node::descent::check_table_file` (`descent.rs:180-246`) and
//! `header_order` (`descent.rs:144-176`) exactly; see its documentation.
//!
//! # Presentation frames
//!
//! Table keys are in the PRESENTED frame of the header's `node_order.slot_permutation` (new slot `j` carries old slot
//! `perm[j]`, `node/order.rs:66-73`); the comparator acts positionally on presented keys. [`Presentation`] maps keys
//! between the original and presented frames. The `source_order` (`Rotate(k)`, `Reverse`) only steers the search and
//! does not move slots (`node/order.rs:14-15`).

use std::cmp::Ordering;
use std::fmt;

use rustred_order::{
    CompiledOrder, CoordinateGroups, DegreeRow, Direction, Limits, OrderDescriptor,
};
use serde::{Deserialize, Serialize};

/// Smallest power the engine's compact `Power` stores (`index.rs:41`).
pub const ENGINE_POWER_MIN: i32 = -64;
/// Largest power the engine's compact `Power` stores (`index.rs:42`).
pub const ENGINE_POWER_MAX: i32 = 63;

/// Errors of comparator construction and checked comparison.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum OrderError {
    /// A key (or priority list) whose length differs from the comparator arity.
    Arity { expected: usize, actual: usize },
    /// A table header that does not select a comparator (missing or malformed field). Message as close as possible
    /// to the engine's.
    Header(String),
    /// A list that must be a permutation of `0..n` is not.
    Permutation(String),
    /// `rustred_order` rejected the descriptor.
    Descriptor(rustred_order::Error),
    /// The two patterns do not agree on which slots are symbolic: the engine asserts here (`index.rs:366-369`, `394-398`).
    PatternMismatch { slot: usize },
    /// A value the engine's compact power cannot store (`index.rs:44-53`).
    OutsideEngineRange { slot: usize, value: i32 },
}

impl fmt::Display for OrderError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Arity { expected, actual } => write!(f, "comparator arity {expected}, key of length {actual}"),
            Self::Header(s) => write!(f, "header order: {s}"),
            Self::Permutation(s) => write!(f, "{s}"),
            Self::Descriptor(e) => write!(f, "order program: {e}"),
            Self::PatternMismatch { slot } => write!(
                f,
                "integral ordering requires the same symbolic coordinate pattern (slot {slot})"
            ),
            Self::OutsideEngineRange { slot, value } => write!(
                f,
                "slot {slot}: power {value} is outside the engine's compact range -64..=63"
            ),
        }
    }
}
impl std::error::Error for OrderError {}

impl From<rustred_order::Error> for OrderError {
    fn from(e: rustred_order::Error) -> Self {
        Self::Descriptor(e)
    }
}

/// Mirror of the engine's `node::order::SourceOrder` (`node/order.rs:24-30`), same serde form
/// (`"Input"`, `"Reverse"`, `{"Rotate": k}`). It never changes the comparator.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum SourceOrder {
    Input,
    Reverse,
    Rotate(usize),
}

/// Mirror of the engine's `node::order::NodeOrder` (`node/order.rs:63-73`), same serde form, so that a header's
/// `node_order` deserialises (or fails) exactly as in `header_order` (`descent.rs:145-147`).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct NodeOrderRecord {
    pub name: String,
    pub slot_permutation: Option<Vec<usize>>,
    pub coordinate_priority: Option<Vec<usize>>,
    pub source_order: SourceOrder,
}

impl NodeOrderRecord {
    /// `NodeOrder::identity()` (`node/order.rs:82-89`).
    pub fn identity() -> Self {
        Self { name: "id".into(), slot_permutation: None, coordinate_priority: None, source_order: SourceOrder::Input }
    }

    /// `NodeOrder::validate` (`node/order.rs:98-117`), same messages.
    pub fn validate(&self, n: usize) -> Result<(), OrderError> {
        for (what, v) in [("slot_permutation", &self.slot_permutation), ("coordinate_priority", &self.coordinate_priority)]
        {
            if let Some(p) = v {
                check_permutation(what, p, n)?;
            }
        }
        Ok(())
    }
}

fn check_permutation(what: &str, p: &[usize], n: usize) -> Result<(), OrderError> {
    if p.len() != n {
        return Err(OrderError::Permutation(format!("{what}: length {} != arity {n}", p.len())));
    }
    let mut seen = vec![false; n];
    for &i in p {
        if i >= n || std::mem::replace(&mut seen[i], true) {
            return Err(OrderError::Permutation(format!("{what}: not a permutation of 0..{n}")));
        }
    }
    Ok(())
}

/// Which branch of the engine's header selection produced a comparator.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Selection {
    /// `descent.rs:205-232`: no `node_order`, `order_is_program == true` and `order.degree_rows` present: the FULL
    /// descriptor of the header (`table_import::permuted_order` with the identity slot map).
    FullHeaderDescriptor,
    /// `descent.rs:145-147`: the header's `node_order`.
    NodeOrder,
    /// `descent.rs:152-154`: no `node_order`, `order_is_program` false or absent: the legacy identity order.
    LegacyNoProgram,
    /// `descent.rs:155-175`: no `node_order`, `order_is_program == true` without `degree_rows`:
    /// `spired_uncut` with `order.coordinate_priority` only (`order.support_priority` is NOT read).
    ProgramCoordinatePriority,
    /// Built explicitly (descriptor, legacy constructor), not from a header.
    Explicit,
}

/// The engine path a comparator takes.
#[derive(Clone, Debug)]
pub enum Engine {
    /// Legacy `intLessStatic` aggregate (`index.rs:386-491`); `ties` = the tie permutation (slot visit order of the
    /// coordinate ties), `None` = identity (`index.rs:217`: the identity is normalised to `None`).
    Legacy { ties: Option<Vec<usize>> },
    /// A compiled `rustred-order` program.
    Program(CompiledOrder),
}

/// Maps keys between the ORIGINAL family slots and the PRESENTED slots of a `slot_permutation`
/// (`presented[j] = original[new_to_old[j]]`, `node/order.rs:66-68`, `126-139`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Presentation {
    new_to_old: Vec<usize>,
    old_to_new: Vec<usize>,
}

impl Presentation {
    pub fn identity(n: usize) -> Self {
        Self { new_to_old: (0..n).collect(), old_to_new: (0..n).collect() }
    }

    /// From a `slot_permutation` (new slot `j` carries old slot `new_to_old[j]`).
    pub fn from_new_to_old(new_to_old: &[usize]) -> Result<Self, OrderError> {
        let n = new_to_old.len();
        check_permutation("slot_permutation", new_to_old, n)?;
        let mut old_to_new = vec![0; n];
        for (j, &i) in new_to_old.iter().enumerate() {
            old_to_new[i] = j;
        }
        Ok(Self { new_to_old: new_to_old.to_vec(), old_to_new })
    }

    /// `node_order.slot_permutation` or the identity (`None`), as `NodeOrder::new_to_old`.
    pub fn from_slot_permutation(perm: Option<&[usize]>, n: usize) -> Result<Self, OrderError> {
        match perm {
            None => Ok(Self::identity(n)),
            Some(p) => {
                if p.len() != n {
                    return Err(OrderError::Arity { expected: n, actual: p.len() });
                }
                Self::from_new_to_old(p)
            }
        }
    }

    pub fn arity(&self) -> usize {
        self.new_to_old.len()
    }
    pub fn new_to_old(&self) -> &[usize] {
        &self.new_to_old
    }
    pub fn old_to_new(&self) -> &[usize] {
        &self.old_to_new
    }
    pub fn is_identity(&self) -> bool {
        self.new_to_old.iter().enumerate().all(|(j, &i)| i == j)
    }

    /// ORIGINAL -> PRESENTED: `out[j] = original[new_to_old[j]]`.
    pub fn present(&self, original: &[i32]) -> Vec<i32> {
        assert_eq!(original.len(), self.arity(), "key arity");
        self.new_to_old.iter().map(|&i| original[i]).collect()
    }

    /// PRESENTED -> ORIGINAL: `out[new_to_old[j]] = presented[j]`.
    pub fn original(&self, presented: &[i32]) -> Vec<i32> {
        assert_eq!(presented.len(), self.arity(), "key arity");
        let mut out = vec![0; self.arity()];
        for (j, &i) in self.new_to_old.iter().enumerate() {
            out[i] = presented[j];
        }
        out
    }
}

/// One slot of a symbolic key pattern: a fixed integer, or the slot's own symbol `n<slot>` plus an offset.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PatternSlot {
    Fixed(i32),
    Symbolic(i32),
}

impl PatternSlot {
    fn is_symbolic(self) -> bool {
        matches!(self, Self::Symbolic(_))
    }
    fn value(self) -> i64 {
        match self {
            Self::Fixed(v) | Self::Symbolic(v) => i64::from(v),
        }
    }
}

/// Parse an engine key pattern `I(1,n1+1,0,n3-2,...)` (the engine's Display form, `descent.rs:15-55`).
pub fn parse_pattern(text: &str) -> Result<Vec<PatternSlot>, String> {
    let inner = text
        .trim()
        .strip_prefix("I(")
        .and_then(|s| s.strip_suffix(')'))
        .ok_or_else(|| format!("not an integral key: {text}"))?;
    let mut out = Vec::new();
    for (i, p) in inner.split(',').enumerate() {
        let p = p.trim();
        if let Some(rest) = p.strip_prefix('n') {
            let digits: String = rest.chars().take_while(char::is_ascii_digit).collect();
            let axis: usize = digits.parse().map_err(|_| format!("key {text}: bad symbol {p}"))?;
            if axis != i {
                return Err(format!("key {text}: symbol n{axis} at position {i}"));
            }
            let offset = &rest[digits.len()..];
            let value: i32 = if offset.is_empty() {
                0
            } else {
                offset.trim_start_matches('+').parse().map_err(|_| format!("key {text}: bad offset {p}"))?
            };
            out.push(PatternSlot::Symbolic(value));
        } else {
            out.push(PatternSlot::Fixed(p.parse().map_err(|_| format!("key {text}: bad value {p}"))?));
        }
    }
    Ok(out)
}

/// The comparator C of one node. See the module documentation for the direction (`Less` = lower).
#[derive(Clone, Debug)]
pub struct NodeComparator {
    arity: usize,
    engine: Engine,
    name: String,
    selection: Selection,
    presentation: Presentation,
    source_order: Option<SourceOrder>,
}

/// Stack capacity for the i32 -> i64 conversion of the program path (larger arities allocate).
const STACK: usize = 32;

impl NodeComparator {
    // ----- construction -----

    /// The legacy non-program order (`IntegralOrder::new(sector, [false; N])`, optionally `with_permutation`).
    /// `ties` = slot visit order of the coordinate ties (`None` or the identity = slot order).
    pub fn legacy(arity: usize, ties: Option<Vec<usize>>) -> Result<Self, OrderError> {
        if arity == 0 {
            return Err(OrderError::Arity { expected: 1, actual: 0 });
        }
        let ties = match ties {
            Some(p) => {
                check_permutation("tie permutation", &p, arity)?;
                // index.rs:215-217: the identity permutation is normalised to the static path
                (!p.iter().enumerate().all(|(i, &s)| i == s)).then_some(p)
            }
            None => None,
        };
        Ok(Self {
            arity,
            engine: Engine::Legacy { ties },
            name: "legacy".into(),
            selection: Selection::Explicit,
            presentation: Presentation::identity(arity),
            source_order: None,
        })
    }

    /// An explicit program descriptor, compiled with `Limits::default()` as the engine does (`driver.rs:1217`,
    /// `descent.rs:72`, `descent.rs:255`).
    pub fn from_descriptor(descriptor: OrderDescriptor) -> Result<Self, OrderError> {
        Self::from_program(CompiledOrder::compile(descriptor, Limits::default())?)
    }

    /// An already compiled program.
    pub fn from_program(program: CompiledOrder) -> Result<Self, OrderError> {
        let arity = program.arity();
        Ok(Self {
            arity,
            engine: Engine::Program(program),
            name: "program".into(),
            selection: Selection::Explicit,
            presentation: Presentation::identity(arity),
            source_order: None,
        })
    }

    /// The comparator a node order selects (`descent.rs:59-77` `integral_order`, `driver.rs:1199-1222`
    /// `order_program`): `coordinate_priority` `None` = legacy order, `Some(p)` = `spired_uncut(n)` with that
    /// coordinate priority. The `slot_permutation` is recorded as the presentation; it does not enter the order.
    pub fn from_node_order(order: &NodeOrderRecord, arity: usize) -> Result<Self, OrderError> {
        order.validate(arity)?;
        let mut c = match &order.coordinate_priority {
            None => Self::legacy(arity, None)?,
            Some(priority) => {
                let mut d = OrderDescriptor::spired_uncut(arity);
                d.coordinate_priority = priority.clone();
                Self::from_descriptor(d)?
            }
        };
        c.name = order.name.clone();
        c.selection = Selection::NodeOrder;
        c.presentation = Presentation::from_slot_permutation(order.slot_permutation.as_deref(), arity)?;
        c.source_order = Some(order.source_order.clone());
        Ok(c)
    }

    /// The comparator a `table_dump.v1` header selects, exactly as `check_table_file` (`descent.rs:180-246`) with
    /// no explicit order argument:
    ///
    /// 1. arity = `denominators` (required, `descent.rs:201-203`);
    /// 2. if there is NO `node_order` key, `order_is_program` is `true` and `order.degree_rows` exists: the full
    ///    header descriptor (`descent.rs:205-232`, `table_import.rs:635-718` with the identity slot map);
    /// 3. else `header_order` (`descent.rs:144-176`): a `node_order` key (even `null`, which then fails to
    ///    deserialise as in the engine) selects that node order; else `order_is_program` false/absent selects the
    ///    legacy identity order; else `spired_uncut` with `order.coordinate_priority` (required) and NOTHING else of
    ///    `order` (in particular not `support_priority`), named `order_name` or `"program"`.
    ///
    /// The engine additionally rejects arities other than 5, 9, 12, 14, 20 in this check (`descent.rs:222-228`,
    /// `239-244`, compiled storage sizes); this function accepts every arity.
    ///
    /// Also accepts the helper's wrapped form `{"header": {...}}` (z27 / h13b `*.keys.jsonl`).
    pub fn from_header(header: &serde_json::Value) -> Result<Self, OrderError> {
        let header = match header.get("header") {
            Some(inner) if header.get("type").is_none() => inner,
            _ => header,
        };
        let arity = header["denominators"]
            .as_u64()
            .ok_or_else(|| OrderError::Header("header denominators".into()))? as usize;
        if header.get("node_order").is_none()
            && header["order_is_program"].as_bool() == Some(true)
            && header["order"].get("degree_rows").is_some()
        {
            let identity: Vec<usize> = (0..arity).collect();
            let descriptor = descriptor_from_order_json(&header["order"], &identity)
                .map_err(|e| OrderError::Header(format!("header order descriptor: {e}")))?;
            let mut c = Self::from_descriptor(descriptor)?;
            let name = header["order_name"].as_str().unwrap_or("program");
            c.name = format!("{name} (full header descriptor)");
            c.selection = Selection::FullHeaderDescriptor;
            return Ok(c);
        }
        if let Some(v) = header.get("node_order") {
            let order: NodeOrderRecord = serde_json::from_value(v.clone())
                .map_err(|e| OrderError::Header(format!("header node_order: {e}")))?;
            return Self::from_node_order(&order, arity);
        }
        let is_program = header.get("order_is_program").and_then(|v| v.as_bool()).unwrap_or(false);
        if !is_program {
            let mut c = Self::legacy(arity, None)?;
            c.name = "id".into();
            c.selection = Selection::LegacyNoProgram;
            return Ok(c);
        }
        let priority: Vec<usize> = header
            .pointer("/order/coordinate_priority")
            .and_then(|v| v.as_array())
            .ok_or_else(|| OrderError::Header("header order program without coordinate_priority".into()))?
            .iter()
            .map(|x| x.as_u64().map(|x| x as usize).ok_or_else(|| OrderError::Header("coordinate_priority entry".into())))
            .collect::<Result<_, _>>()?;
        let name = header.get("order_name").and_then(|v| v.as_str()).unwrap_or("program").to_string();
        let order = NodeOrderRecord { name, coordinate_priority: Some(priority), ..NodeOrderRecord::identity() };
        let mut c = Self::from_node_order(&order, arity)?;
        c.selection = Selection::ProgramCoordinatePriority;
        Ok(c)
    }

    // ----- accessors -----

    pub fn arity(&self) -> usize {
        self.arity
    }
    pub fn engine(&self) -> &Engine {
        &self.engine
    }
    pub fn name(&self) -> &str {
        &self.name
    }
    pub fn selection(&self) -> &Selection {
        &self.selection
    }
    /// The presentation of the keys this comparator is applied to (recorded from the header's node order).
    pub fn presentation(&self) -> &Presentation {
        &self.presentation
    }
    pub fn source_order(&self) -> Option<&SourceOrder> {
        self.source_order.as_ref()
    }

    /// The program descriptor equivalent on concrete keys: the program's own, or for the legacy path
    /// `spired_uncut(n)` with `coordinate_priority` = the tie permutation (see the module documentation).
    pub fn equivalent_descriptor(&self) -> OrderDescriptor {
        match &self.engine {
            Engine::Program(p) => p.descriptor().clone(),
            Engine::Legacy { ties } => {
                let mut d = OrderDescriptor::spired_uncut(self.arity);
                if let Some(t) = ties {
                    d.coordinate_priority = t.clone();
                }
                d
            }
        }
    }

    // ----- comparison (Less = a LOWER than b) -----

    /// `Less` iff `a` is LOWER (simpler) than `b`; `Equal` iff `a == b` (the order is total and antisymmetric).
    /// Panics if a key length differs from [`Self::arity`]; use [`Self::try_cmp`] for a checked call.
    #[inline]
    pub fn cmp(&self, a: &[i32], b: &[i32]) -> Ordering {
        assert!(
            a.len() == self.arity && b.len() == self.arity,
            "NodeComparator::cmp: keys of length {} and {}, arity {}",
            a.len(),
            b.len(),
            self.arity
        );
        match &self.engine {
            Engine::Legacy { ties } => legacy_harder_first(ties.as_deref(), a, b).reverse(),
            Engine::Program(p) => program_cmp(p, a, b),
        }
    }

    /// [`Self::cmp`] with arity errors instead of a panic.
    pub fn try_cmp(&self, a: &[i32], b: &[i32]) -> Result<Ordering, OrderError> {
        for k in [a, b] {
            if k.len() != self.arity {
                return Err(OrderError::Arity { expected: self.arity, actual: k.len() });
            }
        }
        Ok(self.cmp(a, b))
    }

    /// `a` strictly LOWER than `b` (a right-hand-side key `a` of a rule with target `b` descends).
    #[inline]
    pub fn lower(&self, a: &[i32], b: &[i32]) -> bool {
        self.cmp(a, b) == Ordering::Less
    }

    /// Compare keys given in the ORIGINAL frame: both are presented first ([`Presentation::present`]).
    pub fn cmp_original(&self, a: &[i32], b: &[i32]) -> Ordering {
        if self.presentation.is_identity() {
            return self.cmp(a, b);
        }
        self.cmp(&self.presentation.present(a), &self.presentation.present(b))
    }

    /// Arity check plus the engine's compact power range (-64..=63) on every slot.
    pub fn check_engine_range(&self, key: &[i32]) -> Result<(), OrderError> {
        if key.len() != self.arity {
            return Err(OrderError::Arity { expected: self.arity, actual: key.len() });
        }
        for (slot, &value) in key.iter().enumerate() {
            if !(ENGINE_POWER_MIN..=ENGINE_POWER_MAX).contains(&value) {
                return Err(OrderError::OutsideEngineRange { slot, value });
            }
        }
        Ok(())
    }

    /// Compare two symbolic patterns on a chart (`chart[i]` = the chart's sector bit, the engine's
    /// `IntegralOrder::sector`), `Less` = `a` LOWER. Program path: `compare_mixed` (`index.rs:364-385`); legacy path:
    /// the symbolic branches of `index.rs:403-419` and `465-482`. Different symbolic patterns are an error, as the
    /// engine asserts. A symbolic slot carries its offset from the common symbol.
    pub fn cmp_pattern(&self, chart: &[bool], a: &[PatternSlot], b: &[PatternSlot]) -> Result<Ordering, OrderError> {
        for k in [a.len(), b.len(), chart.len()] {
            if k != self.arity {
                return Err(OrderError::Arity { expected: self.arity, actual: k });
            }
        }
        for slot in 0..self.arity {
            if a[slot].is_symbolic() != b[slot].is_symbolic() {
                return Err(OrderError::PatternMismatch { slot });
            }
        }
        match &self.engine {
            Engine::Legacy { ties } => Ok(legacy_pattern_harder_first(ties.as_deref(), chart, a, b).reverse()),
            Engine::Program(p) => {
                let symbolic: Vec<bool> = a.iter().map(|s| s.is_symbolic()).collect();
                let l: Vec<i64> = a.iter().map(|s| s.value()).collect();
                let r: Vec<i64> = b.iter().map(|s| s.value()).collect();
                Ok(p.compare_mixed(chart, &symbolic, &l, &r)?.ordering)
            }
        }
    }

    // ----- rank -----

    /// A sort key: `rank(a) < rank(b)` (lexicographic) iff `cmp(a, b) == Less`. One allocation per key; prefer
    /// [`Self::cmp`] for single comparisons. Components: see [`Rank`].
    pub fn rank(&self, key: &[i32]) -> Rank {
        assert_eq!(key.len(), self.arity, "NodeComparator::rank: key arity");
        match &self.engine {
            Engine::Legacy { ties } => legacy_rank(ties.as_deref(), key),
            Engine::Program(p) => program_rank(p.descriptor(), key),
        }
    }
}

/// Lexicographic sort key of [`NodeComparator::rank`] (smaller = lower).
///
/// Legacy: `[#active, active bit of slot 0..n, sum|n_i|, sum_{n_i<=0} -n_i, -n_s for active s in tie order,
/// n_s for inactive s in tie order]`. Program: `[pre-support rows.., #active, support weight, active bits in support
/// priority, degree rows.., coordinate entries (excess, negated when Descending) in the descriptor's passes]`.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Rank(pub Vec<i128>);

// ----- legacy path: port of index.rs:386-491 -----

/// `IntegralOrder::compare` without a program, on concrete (all numeric) keys, uncut: HARDER-first as in the engine
/// (`Less` = `left` harder). Line references are to `crates/rustred-core/src/solver/index.rs` at b9f06de7.
fn legacy_harder_first(ties: Option<&[usize]>, left: &[i32], right: &[i32]) -> Ordering {
    // index.rs:386-390
    let (mut denominators_left, mut denominators_right) = (0usize, 0usize);
    let mut sector_lex = Ordering::Equal;
    let (mut absolute_left, mut absolute_right) = (0i64, 0i64);
    let (mut numerator_left, mut numerator_right) = (0i64, 0i64);
    for (&l, &r) in left.iter().zip(right) {
        // index.rs:420-433 (numeric coordinate; deltas are all false, so delta_order stays Equal)
        let (l, r) = (i64::from(l), i64::from(r));
        denominators_left += usize::from(l > 0);
        denominators_right += usize::from(r > 0);
        if sector_lex == Ordering::Equal {
            sector_lex = (l > 0).cmp(&(r > 0));
        }
        absolute_left += l.abs();
        absolute_right += r.abs();
        numerator_left += if l <= 0 { -l } else { 0 };
        numerator_right += if r <= 0 { -r } else { 0 };
    }
    // index.rs:436-444 (`.then(delta_order)` is Equal without cuts)
    let aggregate = denominators_right
        .cmp(&denominators_left)
        .then(sector_lex.reverse())
        .then(absolute_right.cmp(&absolute_left))
        .then(numerator_right.cmp(&numerator_left));
    if aggregate != Ordering::Equal {
        return aggregate;
    }
    // index.rs:448-453, 456-491
    match ties {
        Some(p) => legacy_ties(left, right, p.iter().copied()),
        None => legacy_ties(left, right, 0..left.len()),
    }
}

/// `compare_coordinate_ties` (`index.rs:456-491`) on numeric keys: denominators by increasing power (the smaller
/// power is harder), then numerators by decreasing power (the larger value, i.e. the smaller |n|, is harder).
fn legacy_ties(left: &[i32], right: &[i32], order: impl Iterator<Item = usize> + Clone) -> Ordering {
    for index in order.clone() {
        if left[index] > 0 {
            let o = left[index].cmp(&right[index]);
            if o != Ordering::Equal {
                return o;
            }
        }
    }
    for index in order {
        if left[index] <= 0 {
            let o = right[index].cmp(&left[index]);
            if o != Ordering::Equal {
                return o;
            }
        }
    }
    Ordering::Equal
}

/// `IntegralOrder::compare` without a program on symbolic patterns (`index.rs:386-491`, uncut), HARDER-first.
/// Patterns were checked to agree on which slots are symbolic.
fn legacy_pattern_harder_first(
    ties: Option<&[usize]>,
    sector: &[bool],
    left: &[PatternSlot],
    right: &[PatternSlot],
) -> Ordering {
    let (mut denominators_left, mut denominators_right) = (0usize, 0usize);
    let mut sector_lex = Ordering::Equal;
    let (mut absolute_left, mut absolute_right) = (0i64, 0i64);
    let (mut numerator_left, mut numerator_right) = (0i64, 0i64);
    for index in 0..left.len() {
        let (l, r) = (left[index].value(), right[index].value());
        if left[index].is_symbolic() {
            // index.rs:403-419 (deltas false)
            if sector[index] {
                absolute_left += l;
                absolute_right += r;
            } else {
                absolute_left -= l;
                absolute_right -= r;
                numerator_left -= l;
                numerator_right -= r;
            }
        } else {
            // index.rs:420-433
            denominators_left += usize::from(l > 0);
            denominators_right += usize::from(r > 0);
            if sector_lex == Ordering::Equal {
                sector_lex = (l > 0).cmp(&(r > 0));
            }
            absolute_left += l.abs();
            absolute_right += r.abs();
            numerator_left += if l <= 0 { -l } else { 0 };
            numerator_right += if r <= 0 { -r } else { 0 };
        }
    }
    let aggregate = denominators_right
        .cmp(&denominators_left)
        .then(sector_lex.reverse())
        .then(absolute_right.cmp(&absolute_left))
        .then(numerator_right.cmp(&numerator_left));
    if aggregate != Ordering::Equal {
        return aggregate;
    }
    let order: Vec<usize> = match ties {
        Some(p) => p.to_vec(),
        None => (0..left.len()).collect(),
    };
    // index.rs:464-476
    for &index in &order {
        let is_denominator = if left[index].is_symbolic() { sector[index] } else { left[index].value() > 0 };
        if is_denominator {
            let o = left[index].value().cmp(&right[index].value());
            if o != Ordering::Equal {
                return o;
            }
        }
    }
    // index.rs:477-489
    for &index in &order {
        let is_numerator = if left[index].is_symbolic() { !sector[index] } else { left[index].value() <= 0 };
        if is_numerator {
            let o = right[index].value().cmp(&left[index].value());
            if o != Ordering::Equal {
                return o;
            }
        }
    }
    Ordering::Equal
}

fn legacy_rank(ties: Option<&[usize]>, key: &[i32]) -> Rank {
    let n = key.len();
    let order: Vec<usize> = match ties {
        Some(p) => p.to_vec(),
        None => (0..n).collect(),
    };
    let mut v = Vec::with_capacity(2 * n + 3);
    v.push(key.iter().filter(|&&x| x > 0).count() as i128);
    v.extend(key.iter().map(|&x| i128::from(x > 0)));
    v.push(key.iter().map(|&x| i128::from(x).abs()).sum());
    v.push(key.iter().map(|&x| if x <= 0 { -i128::from(x) } else { 0 }).sum());
    v.extend(order.iter().filter(|&&s| key[s] > 0).map(|&s| -i128::from(key[s])));
    v.extend(order.iter().filter(|&&s| key[s] <= 0).map(|&s| i128::from(key[s])));
    Rank(v)
}

// ----- program path: rustred-order -----

#[inline]
fn program_cmp(p: &CompiledOrder, a: &[i32], b: &[i32]) -> Ordering {
    let n = a.len();
    let result = if n <= STACK {
        let mut l = [0i64; STACK];
        let mut r = [0i64; STACK];
        for i in 0..n {
            l[i] = i64::from(a[i]);
            r[i] = i64::from(b[i]);
        }
        p.compare(&l[..n], &r[..n])
    } else {
        let l: Vec<i64> = a.iter().map(|&x| i64::from(x)).collect();
        let r: Vec<i64> = b.iter().map(|&x| i64::from(x)).collect();
        p.compare(&l, &r)
    };
    // Arity was checked by the caller; i32 powers are far inside MAX_TWO_SIDED_POWER: no error is possible.
    result.expect("rustred-order compare on concrete i32 keys of the program arity").ordering
}

/// Excess and activity of a concrete power as `compare.rs` (`Coordinates::Concrete`, `View::Powers`) sees it.
fn concrete_view(value: i32, two_sided: bool) -> (bool, i128, i128) {
    let v = i128::from(value);
    if two_sided {
        let e = i128::from(rustred_order::two_sided_excess(i64::from(value)).expect("i32 power in two-sided range"));
        return (true, e, e + 1);
    }
    if value > 0 { (true, v - 1, v) } else { (false, -v, -v) }
}

fn program_rank(d: &OrderDescriptor, key: &[i32]) -> Rank {
    let n = key.len();
    let ts = |a: usize| d.two_sided.get(a).copied().unwrap_or(false);
    let view: Vec<(bool, i128, i128)> = (0..n).map(|a| concrete_view(key[a], ts(a))).collect();
    let mut v = Vec::new();
    let row = |row: &DegreeRow, component: usize| -> i128 {
        (0..n)
            .map(|a| {
                let w = if view[a].0 { row.active[a] } else { row.inactive[a] };
                i128::from(w) * if component == 1 { view[a].1 } else { view[a].2 }
            })
            .sum()
    };
    for r in &d.pre_support_degree_rows {
        v.push(row(r, 2));
    }
    v.push(view.iter().filter(|x| x.0).count() as i128);
    v.push((0..n).filter(|&a| view[a].0).map(|a| i128::from(d.support_weights[a])).sum());
    v.extend(d.support_priority.iter().map(|&a| i128::from(view[a].0)));
    for r in &d.degree_rows {
        v.push(row(r, 1));
    }
    let entry = |a: usize| {
        let dir = if view[a].0 { d.active_direction } else { d.inactive_direction };
        if dir == Direction::Descending { -view[a].1 } else { view[a].1 }
    };
    match d.coordinate_groups {
        CoordinateGroups::Interleaved => v.extend(d.coordinate_priority.iter().map(|&a| entry(a))),
        groups => {
            let first_active = groups == CoordinateGroups::ActiveFirst;
            for pass_active in [first_active, !first_active] {
                v.extend(d.coordinate_priority.iter().filter(|&&a| view[a].0 == pass_active).map(|&a| entry(a)));
            }
        }
    }
    Rank(v)
}

// ----- header order JSON -> descriptor: port of table_import.rs:613-718 -----

/// Port of `propagator::table_import::permuted_order` (`table_import.rs:635-718`): the header's `order` object as a
/// descriptor with every slot moved by `slot_map` (slot `j` becomes slot `slot_map[j]`). Full form (with
/// `degree_rows` or `support_weights`): every field is required. Compact form (neither): `spired_uncut` (or
/// `spired_uncut_two_sided` when a two-sided flag is set) with the recorded `support_priority` and
/// `coordinate_priority`.
pub fn descriptor_from_order_json(order: &serde_json::Value, slot_map: &[usize]) -> Result<OrderDescriptor, String> {
    use serde_json::Value;
    let malformed = |what: String| format!("malformed table: {what}");
    let n = slot_map.len();
    check_permutation("slot map", slot_map, n).map_err(|e| e.to_string())?;
    let u64_list = |value: &Value, field: &str| -> Result<Vec<u64>, String> {
        value
            .as_array()
            .ok_or_else(|| malformed(format!("order field {field}")))?
            .iter()
            .map(|e| e.as_u64().ok_or_else(|| malformed(format!("order field {field}"))))
            .collect()
    };
    let permuted = |values: &[u64]| -> Result<Vec<u64>, String> {
        if values.len() != n {
            return Err(malformed(format!("order list of length {} for arity {n}", values.len())));
        }
        let mut out = values.to_vec();
        for (slot, &value) in values.iter().enumerate() {
            out[slot_map[slot]] = value;
        }
        Ok(out)
    };
    let rows = |field: &str| -> Result<Vec<DegreeRow>, String> {
        order[field]
            .as_array()
            .ok_or_else(|| malformed(format!("order field {field}")))?
            .iter()
            .map(|row| {
                Ok(DegreeRow {
                    active: permuted(&u64_list(&row["active"], field)?)?,
                    inactive: permuted(&u64_list(&row["inactive"], field)?)?,
                })
            })
            .collect()
    };
    let priority = |field: &str| -> Result<Vec<usize>, String> {
        u64_list(&order[field], field)?
            .into_iter()
            .map(|slot| slot_map.get(slot as usize).copied().ok_or_else(|| malformed(format!("order field {field}"))))
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
                Value::Number(x) => Ok(x.as_u64() == Some(1)),
                _ => Err(malformed("order field two_sided".into())),
            })
            .collect::<Result<_, _>>()?,
        None => Vec::new(),
    };
    let permuted_flags = |flags: &[bool]| -> Result<Vec<bool>, String> {
        if flags.len() != n {
            return Err(malformed(format!("two_sided of length {} for arity {n}", flags.len())));
        }
        let mut out = flags.to_vec();
        for (slot, &f) in flags.iter().enumerate() {
            out[slot_map[slot]] = f;
        }
        Ok(out)
    };
    if order.get("degree_rows").is_none() && order.get("support_weights").is_none() {
        let mut descriptor = if two_sided.iter().any(|&b| b) {
            OrderDescriptor::spired_uncut_two_sided(permuted_flags(&two_sided)?)
        } else {
            OrderDescriptor::spired_uncut(n)
        };
        descriptor.support_priority = priority("support_priority")?;
        descriptor.coordinate_priority = priority("coordinate_priority")?;
        return Ok(descriptor);
    }
    Ok(OrderDescriptor {
        pre_support_degree_rows: rows("pre_support_degree_rows")?,
        support_weights: permuted(&u64_list(&order["support_weights"], "support_weights")?)?,
        support_priority: priority("support_priority")?,
        degree_rows: rows("degree_rows")?,
        coordinate_priority: priority("coordinate_priority")?,
        coordinate_groups: match order["coordinate_groups"].as_str() {
            Some("ActiveFirst") => CoordinateGroups::ActiveFirst,
            Some("InactiveFirst") => CoordinateGroups::InactiveFirst,
            Some("Interleaved") => CoordinateGroups::Interleaved,
            _ => return Err(malformed("order field coordinate_groups".into())),
        },
        active_direction: direction("active_direction")?,
        inactive_direction: direction("inactive_direction")?,
        two_sided: if two_sided.iter().any(|&b| b) { permuted_flags(&two_sided)? } else { Vec::new() },
    })
}

// ----- automorphism invariance (requirement R9 (b)) -----

/// One automorphism of a node as a slot map: `(g k)[lines[i]] = k[i]` for the line slots `0..lines.len()`, and
/// likewise `numerators` on the slots after the lines (`None` = numerator map not given).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GroupElement {
    pub lines: Vec<usize>,
    pub numerators: Option<Vec<usize>>,
}

impl GroupElement {
    /// `(g k)`; `None` when the key has a non-zero numerator slot and the numerator map is not given.
    pub fn apply(&self, key: &[i32]) -> Option<Vec<i32>> {
        let nl = self.lines.len();
        let mut out = key.to_vec();
        for (i, &j) in self.lines.iter().enumerate() {
            out[j] = key[i];
        }
        match &self.numerators {
            Some(m) => {
                for (i, &j) in m.iter().enumerate() {
                    out[nl + j] = key[nl + i];
                }
            }
            None => {
                if key[nl..].iter().any(|&x| x != 0) {
                    return None;
                }
            }
        }
        Some(out)
    }

    fn full_permutation(&self, n: usize) -> Vec<usize> {
        let nl = self.lines.len();
        let mut image: Vec<usize> = (0..n).collect();
        image[..nl].copy_from_slice(&self.lines);
        if let Some(m) = &self.numerators {
            for (i, &j) in m.iter().enumerate() {
                image[nl + i] = nl + j;
            }
        }
        image
    }
}

/// Verdict of [`automorphism_invariance`] for one group element.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ElementInvariance {
    pub element: usize,
    /// The transported descriptor equals the comparator's own (canonical bytes): a SUFFICIENT condition for
    /// `cmp(g a, g b) == cmp(a, b)` on all keys. With a total coordinate priority it holds only for elements that fix
    /// every slot whose priority/weights matter, so it is mostly informative for the identity.
    pub descriptor_invariant: bool,
    pub pairs_tested: usize,
    /// Pairs skipped because a numerator slot is non-zero and the numerator map is not given.
    pub pairs_skipped: usize,
    pub pairs_disagreeing: usize,
    pub first_counterexample: Option<(Vec<i32>, Vec<i32>)>,
}

/// Report of [`automorphism_invariance`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct InvarianceReport {
    pub elements: Vec<ElementInvariance>,
}

impl InvarianceReport {
    /// No sampled pair disagrees under any element (the plan's 2.4.0 criterion: `compare(g a, g b) == compare(a, b)`
    /// on the sample).
    pub fn invariant_on_sample(&self) -> bool {
        self.elements.iter().all(|e| e.pairs_disagreeing == 0)
    }
    pub fn total_disagreeing(&self) -> usize {
        self.elements.iter().map(|e| e.pairs_disagreeing).sum()
    }
    pub fn total_tested(&self) -> usize {
        self.elements.iter().map(|e| e.pairs_tested).sum()
    }
}

/// Requirement R9 (b): is the comparator's priority invariant under the node's automorphism group? For every
/// element `g` and every sampled pair `(a, b)` (the plan suggests pairs of one orbit layer), test
/// `cmp(g a, g b) == cmp(a, b)`; also report whether the transported descriptor is identical. The disagreement count
/// ranks candidate orders: an order that is not invariant makes image rules pour back above the key.
pub fn automorphism_invariance(
    comparator: &NodeComparator,
    group: &[GroupElement],
    pairs: &[(Vec<i32>, Vec<i32>)],
) -> Result<InvarianceReport, OrderError> {
    let n = comparator.arity();
    let own = CompiledOrder::compile(comparator.equivalent_descriptor(), Limits::default())?;
    let mut elements = Vec::with_capacity(group.len());
    for (index, g) in group.iter().enumerate() {
        let nl = g.lines.len();
        if nl > n {
            return Err(OrderError::Arity { expected: n, actual: nl });
        }
        check_permutation("group element lines", &g.lines, nl)?;
        if let Some(m) = &g.numerators {
            check_permutation("group element numerators", m, n - nl)?;
        }
        let image = g.full_permutation(n);
        // (g k)[image[i]] = k[i]  =>  (g k)[j] = k[inv[j]]: transport's source_for_target is the inverse image.
        let mut inverse = vec![0; n];
        for (i, &j) in image.iter().enumerate() {
            inverse[j] = i;
        }
        let transported = own.transport(&inverse, Limits::default())?;
        // Without a numerator map the structural check treats the numerator slots as fixed.
        let mut e = ElementInvariance {
            element: index,
            descriptor_invariant: transported.canonical_bytes() == own.canonical_bytes(),
            pairs_tested: 0,
            pairs_skipped: 0,
            pairs_disagreeing: 0,
            first_counterexample: None,
        };
        for (a, b) in pairs {
            if a.len() != n || b.len() != n {
                return Err(OrderError::Arity { expected: n, actual: a.len().max(b.len()) });
            }
            let (Some(ga), Some(gb)) = (g.apply(a), g.apply(b)) else {
                e.pairs_skipped += 1;
                continue;
            };
            e.pairs_tested += 1;
            if comparator.cmp(&ga, &gb) != comparator.cmp(a, b) {
                e.pairs_disagreeing += 1;
                if e.first_counterexample.is_none() {
                    e.first_counterexample = Some((a.clone(), b.clone()));
                }
            }
        }
        elements.push(e);
    }
    Ok(InvarianceReport { elements })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn direction_is_lower_first() {
        let c = NodeComparator::legacy(5, None).unwrap();
        // a subsector key is lower than a top-sector key
        assert_eq!(c.cmp(&[1, 1, 0, 0, 0], &[1, 1, 1, 0, 0]), Ordering::Less);
        assert!(c.lower(&[1, 1, 1, 0, 0], &[2, 1, 1, 0, 0]));
        assert!(!c.lower(&[2, 1, 1, 0, 0], &[1, 1, 1, 0, 0]));
    }
}
