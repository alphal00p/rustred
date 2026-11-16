//! Rule bodies: the right-hand-side KEY PATTERNS of every rule (no coefficients), stored compactly, instantiated at
//! a key, and classified (positive numerator first, then zero sector, subsector hand-down, top sector).
//!
//! Pattern grammar `I(a_0,..,a_{N-1})`, each argument `n<p>`, `n<p>+<o>`, `n<p>-<o>` (parameter p of the rule
//! point plus offset o) or an integer constant. The rule point is the key for a canonical target and the engine's
//! point map (`find_rule`) for a target pattern with offsets ([`crate::domain::TargetMap`]).
//!
//! Storage: one `u16` entry per slot that differs from the rule point: high byte = slot (6 bits) | kind (2 bits:
//! 0 = add the low byte to the slot, 1 = set the slot to the low byte, 2 = copy parameter `low byte` into the
//! slot), low byte = an `i8` value. A value outside `i8` is a load error (fail closed). Per rule (table, record):
//! the range of its patterns; per pattern: the range of its entries (u32 offsets).

use std::collections::{BTreeSet, HashMap};
use std::io::BufRead;

use crate::cover::{Dispatcher, RuleRef};
use crate::domain::{CaseDomain, TargetMap, TargetSlot, parse_target};
use crate::record::TableFile;

/// One slot of a right-hand-side pattern.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RhsSlot {
    /// `n<param> + offset`.
    Param { param: usize, offset: i32 },
    /// An integer constant.
    Const(i32),
}

/// Parse a right-hand-side pattern.
pub fn parse_rhs_pattern(text: &str, arity: usize) -> Result<Vec<RhsSlot>, String> {
    let inner = text
        .trim()
        .strip_prefix("I(")
        .and_then(|t| t.strip_suffix(')'))
        .ok_or_else(|| format!("bad pattern {text:?}"))?;
    let int = |t: &str| -> Option<i32> {
        let d = t.strip_prefix('-').unwrap_or(t);
        (!d.is_empty() && d.bytes().all(|b| b.is_ascii_digit()))
            .then(|| t.parse().ok())
            .flatten()
    };
    let mut out = Vec::with_capacity(arity);
    for a in inner.split(',') {
        let a = a.trim();
        if let Some(rest) = a.strip_prefix('n') {
            let end = rest.find(['+', '-']).unwrap_or(rest.len());
            let idx = &rest[..end];
            if idx.is_empty() || !idx.bytes().all(|b| b.is_ascii_digit()) {
                return Err(format!("bad pattern argument {a:?}"));
            }
            let param: usize = idx
                .parse()
                .map_err(|_| format!("bad pattern argument {a:?}"))?;
            if param >= arity {
                return Err(format!("pattern parameter n{param} >= arity {arity}"));
            }
            let offset = if end == rest.len() {
                0
            } else {
                let o = &rest[end..];
                let v = match o.strip_prefix('+') {
                    Some(p) if !p.starts_with('-') => int(p),
                    Some(_) => None,
                    None => int(o),
                };
                v.ok_or_else(|| format!("bad pattern argument {a:?}"))?
            };
            out.push(RhsSlot::Param { param, offset });
        } else {
            out.push(RhsSlot::Const(
                int(a).ok_or_else(|| format!("bad pattern argument {a:?}"))?,
            ));
        }
    }
    if out.len() != arity {
        return Err(format!("pattern of arity {}, expected {arity}", out.len()));
    }
    Ok(out)
}

/// Errors while loading or instantiating bodies.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum BodyError {
    /// Unknown table index or record.
    NoRecord { table: usize, detail: String },
    /// A record got two bodies.
    Duplicate { table: usize, record: usize },
    /// Unreadable pattern or line.
    Pattern(String),
    /// An offset or constant outside `i8`, or a slot >= 64.
    Range(String),
    /// The body's target does not match the record's domain (fixed slots / target map).
    TargetMismatch {
        table: usize,
        record: usize,
        detail: String,
    },
    /// The rule point could not be formed at the key (target constant differs, overflow).
    RulePoint { table: usize, record: usize },
}

impl std::fmt::Display for BodyError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{self:?}")
    }
}
impl std::error::Error for BodyError {}

/// Counts of a body load.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct BodyLoadStats {
    pub lines: usize,
    pub rules: usize,
    pub patterns: usize,
    pub entries: usize,
}

/// Largest arity of the closure's slot masks (u64 line / numerator / dropped-line masks, 6-bit slot entries): above it
/// the closure refuses with a typed error (re-audit F4), never a silent wrap.
pub const MAX_ARITY: usize = 64;

const KIND_ADD: u16 = 0;
const KIND_SET: u16 = 1;
const KIND_COPY: u16 = 2;
const NONE: u32 = u32::MAX;

#[inline]
fn entry(slot: usize, kind: u16, v: i8) -> u16 {
    ((slot as u16) << 10) | (kind << 8) | (v as u8 as u16)
}
#[inline]
fn entry_parts(e: u16) -> (usize, u16, i8) {
    ((e >> 10) as usize, (e >> 8) & 3, (e & 0xff) as u8 as i8)
}

/// Right-hand-side patterns of the rules of one dispatcher's tables.
#[derive(Clone, Debug)]
pub struct RuleBodies {
    arity: usize,
    table_names: Vec<String>,
    /// per table, per record: first pattern id (NONE = no body)
    rule_first: Vec<Vec<u32>>,
    /// per table, per record: number of patterns
    rule_count: Vec<Vec<u32>>,
    /// per pattern: first entry; one extra sentinel at the end
    pat_start: Vec<u32>,
    ent: Vec<u16>,
    /// per table: label -> record ([`AMBIGUOUS`] if two records carry the label: lookups by label fail closed)
    labels: Vec<HashMap<i64, usize>>,
}

/// Marker of a label carried by more than one record of a table (audit loop-A m4).
const AMBIGUOUS: usize = usize::MAX;

fn label_map(labels: &[i64]) -> HashMap<i64, usize> {
    let mut m = HashMap::with_capacity(labels.len());
    for (r, &l) in labels.iter().enumerate() {
        m.entry(l).and_modify(|x| *x = AMBIGUOUS).or_insert(r);
    }
    m
}

/// Classification of an instantiated right-hand-side key (in this order).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum TermClass {
    /// Every line >= 1, every numerator <= 0.
    Top,
    /// Some numerator slot > 0 (any sector): needs a vanishing coefficient. `slots` = bit mask of the positive
    /// numerator slots; `symbolic` = all of them come from a symbolic slot raised past zero (-d < t <= 0).
    PositiveNumerator {
        slots: u64,
        symbolic: bool,
        subsector: bool,
    },
    /// A removed line in a declared zero sector (`dropped` = mask of line slots <= 0).
    ZeroSector { dropped: u64 },
    /// A removed line: a hand-down to the child sector `dropped`.
    Subsector { dropped: u64 },
}

/// Line mask and optional zero sectors of a node.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Sectors {
    pub lines: Vec<bool>,
    /// Forced (eps, two-sided) axes, bit per slot: a line slot that is in every sector, whatever the sign of its
    /// integer index (never dropped; [`crate::key::NodeFrame::eps_axes`], [`crate::cover::Dispatcher::chart_free`]).
    /// 0 for an integer family.
    pub forced: u64,
    /// Dropped-line masks whose sector is zero (scaleless): such terms vanish.
    pub zero_sectors: Option<BTreeSet<u64>>,
}

impl Sectors {
    pub fn new(lines: Vec<bool>) -> Sectors {
        Sectors {
            lines,
            forced: 0,
            zero_sectors: None,
        }
    }
    /// Classify `term`; `const_mask` = slots whose value is a constant of the rule (see [`RuleBodies::instantiate`]).
    #[inline]
    pub fn classify_masked(&self, term: &[i32], const_mask: u64) -> TermClass {
        let mut pos = 0u64;
        let mut dropped = 0u64;
        for (s, (&v, &line)) in term.iter().zip(&self.lines).enumerate() {
            if line {
                if v <= 0 && self.forced >> s & 1 == 0 {
                    dropped |= 1 << s;
                }
            } else if v > 0 {
                pos |= 1 << s;
            }
        }
        if pos != 0 {
            return TermClass::PositiveNumerator {
                slots: pos,
                symbolic: pos & const_mask == 0,
                subsector: dropped != 0,
            };
        }
        if dropped != 0 {
            if self
                .zero_sectors
                .as_ref()
                .is_some_and(|z| z.contains(&dropped))
            {
                return TermClass::ZeroSector { dropped };
            }
            return TermClass::Subsector { dropped };
        }
        TermClass::Top
    }
}

/// Fixed values that equal the rule point (constants there need no entry): only for canonical targets.
fn skip_fixed(dom: &CaseDomain) -> Vec<Option<i32>> {
    match dom.target {
        TargetMap::Canonical => dom.fixed.clone(),
        TargetMap::Pattern(_) => vec![None; dom.arity],
    }
}

/// Rule point of `domain` at `key` (canonical target: the key itself).
pub fn rule_point(domain: &CaseDomain, key: &[i32], pt: &mut Vec<i32>) -> bool {
    pt.clear();
    match &domain.target {
        TargetMap::Canonical => {
            pt.extend_from_slice(key);
            true
        }
        TargetMap::Pattern(t) => {
            let n = domain.arity;
            let mut set = vec![false; n];
            pt.resize(n, 0);
            for (j, s) in t.iter().enumerate() {
                match *s {
                    TargetSlot::Const(c) => {
                        if key[j] as i64 != c {
                            return false;
                        }
                    }
                    TargetSlot::Param { param, offset } => {
                        let Some(v) = (key[j] as i64)
                            .checked_sub(offset)
                            .and_then(|v| i32::try_from(v).ok())
                        else {
                            return false;
                        };
                        if param >= n {
                            return false;
                        }
                        pt[param] = v;
                        set[param] = true;
                    }
                }
            }
            for p in 0..n {
                if !set[p] {
                    pt[p] = domain.fixed[p].unwrap_or(0);
                }
            }
            true
        }
    }
}

impl RuleBodies {
    /// Empty bodies for the tables of `disp`.
    pub fn new(disp: &Dispatcher) -> RuleBodies {
        let t = disp.tables();
        RuleBodies {
            arity: disp.arity(),
            table_names: t.iter().map(|x| x.name.clone()).collect(),
            rule_first: t.iter().map(|x| vec![NONE; x.len()]).collect(),
            rule_count: t.iter().map(|x| vec![0; x.len()]).collect(),
            pat_start: vec![0],
            ent: vec![],
            labels: t.iter().map(|x| label_map(&x.labels)).collect(),
        }
    }

    /// Extend to the tables of a larger dispatcher whose first tables are these (for incremental use).
    pub fn extend_to(&mut self, disp: &Dispatcher) -> Result<(), BodyError> {
        let t = disp.tables();
        if t.len() < self.table_names.len() {
            return Err(BodyError::NoRecord {
                table: t.len(),
                detail: "fewer tables".into(),
            });
        }
        for (i, x) in t.iter().enumerate() {
            if i < self.table_names.len() {
                if x.name != self.table_names[i] || x.len() != self.rule_first[i].len() {
                    return Err(BodyError::NoRecord {
                        table: i,
                        detail: format!("table {} differs", x.name),
                    });
                }
            } else {
                self.table_names.push(x.name.clone());
                self.rule_first.push(vec![NONE; x.len()]);
                self.rule_count.push(vec![0; x.len()]);
                self.labels.push(label_map(&x.labels));
            }
        }
        Ok(())
    }

    pub fn arity(&self) -> usize {
        self.arity
    }

    /// Set the body of (table, record) from pattern strings. `fixed` (the record's fixed slots) lets constants equal
    /// to the rule point be skipped.
    pub fn set<S: AsRef<str>>(
        &mut self,
        disp: &Dispatcher,
        table: usize,
        record: usize,
        rhs: &[S],
    ) -> Result<(), BodyError> {
        let dom = disp
            .tables()
            .get(table)
            .and_then(|t| t.domains.get(record))
            .ok_or_else(|| BodyError::NoRecord {
                table,
                detail: format!("record {record}"),
            })?;
        let fixed = skip_fixed(dom);
        self.set_with_fixed(table, record, &fixed, rhs)
    }

    fn set_with_fixed<S: AsRef<str>>(
        &mut self,
        table: usize,
        record: usize,
        fixed: &[Option<i32>],
        rhs: &[S],
    ) -> Result<(), BodyError> {
        let n = self.arity;
        if n > MAX_ARITY {
            return Err(BodyError::Range(format!("arity {n} above {MAX_ARITY}")));
        }
        let first = self
            .rule_first
            .get(table)
            .and_then(|t| t.get(record))
            .copied()
            .ok_or_else(|| BodyError::NoRecord {
                table,
                detail: format!("record {record}"),
            })?;
        if first != NONE {
            return Err(BodyError::Duplicate { table, record });
        }
        let start = (self.pat_start.len() - 1) as u32;
        let i8v = |v: i32, what: &str| -> Result<i8, BodyError> {
            i8::try_from(v).map_err(|_| BodyError::Range(format!("{what} {v} outside i8")))
        };
        for p in rhs {
            let pat = parse_rhs_pattern(p.as_ref(), n).map_err(BodyError::Pattern)?;
            for (s, a) in pat.iter().enumerate() {
                match *a {
                    RhsSlot::Param { param, offset } if param == s => {
                        if offset != 0 {
                            self.ent.push(entry(s, KIND_ADD, i8v(offset, "offset")?));
                        }
                    }
                    RhsSlot::Param { param, offset } => {
                        self.ent
                            .push(entry(s, KIND_COPY, i8v(param as i32, "param")?));
                        if offset != 0 {
                            self.ent.push(entry(s, KIND_ADD, i8v(offset, "offset")?));
                        }
                    }
                    RhsSlot::Const(c) => {
                        if fixed[s] != Some(c) {
                            self.ent.push(entry(s, KIND_SET, i8v(c, "constant")?));
                        }
                    }
                }
            }
            if self.ent.len() >= u32::MAX as usize {
                return Err(BodyError::Range("more than 2^32 entries".into()));
            }
            self.pat_start.push(self.ent.len() as u32);
        }
        self.rule_first[table][record] = start;
        self.rule_count[table][record] = rhs.len() as u32;
        Ok(())
    }

    /// Check that a body's target agrees with the record's domain: constants = fixed slots, parameters as the
    /// domain's target map (canonical: `n<j>` at slot j).
    fn check_target(&self, dom: &CaseDomain, target: &str) -> Result<(), String> {
        let t = parse_target(target, self.arity)?;
        for (j, s) in t.iter().enumerate() {
            match (*s, dom.fixed[j]) {
                (TargetSlot::Const(c), Some(v)) if c == v as i64 => {}
                (TargetSlot::Const(c), f) => {
                    return Err(format!("slot {j}: constant {c}, fixed {f:?}"));
                }
                (TargetSlot::Param { .. }, Some(v)) => {
                    return Err(format!("slot {j}: symbolic target on fixed slot ({v})"));
                }
                (TargetSlot::Param { param, offset }, None) => match &dom.target {
                    TargetMap::Canonical if param == j && offset == 0 => {}
                    TargetMap::Pattern(p) if p[j] == *s => {}
                    _ => {
                        return Err(format!(
                            "slot {j}: target n{param}{offset:+} vs domain target"
                        ));
                    }
                },
            }
        }
        Ok(())
    }

    /// Load `{"i": key_i, "target": .., "rhs": [..]}` lines (a first header line is skipped) for `table`; the
    /// record is the one with label `key_i - label_shift`.
    pub fn load_keys_jsonl(
        &mut self,
        disp: &Dispatcher,
        table: usize,
        r: impl BufRead,
        label_shift: i64,
    ) -> Result<BodyLoadStats, BodyError> {
        #[derive(serde::Deserialize)]
        struct Line<'a> {
            i: i64,
            #[serde(borrow, default)]
            target: Option<&'a str>,
            #[serde(borrow)]
            rhs: Vec<&'a str>,
        }
        let tab = disp
            .tables()
            .get(table)
            .ok_or_else(|| BodyError::NoRecord {
                table,
                detail: "no such table".into(),
            })?;
        if tab.eps_chart().is_some() {
            // a keys file names a record by `i` alone; an eps table numbers its records per chart
            return Err(BodyError::NoRecord {
                table,
                detail: "keys-jsonl bodies name records by index alone: refused for an eps-shifted family".into(),
            });
        }
        let mut st = BodyLoadStats::default();
        let e0 = self.ent.len();
        for (ln, line) in r.lines().enumerate() {
            let line = line.map_err(|e| BodyError::Pattern(format!("line {ln}: {e}")))?;
            let t = line.trim();
            if t.is_empty() || (ln == 0 && t.contains("\"header\"")) {
                continue;
            }
            st.lines += 1;
            let l: Line = serde_json::from_str(t)
                .map_err(|e| BodyError::Pattern(format!("line {ln}: {e}")))?;
            let label = l.i - label_shift;
            let record = *self.labels[table]
                .get(&label)
                .ok_or_else(|| BodyError::NoRecord {
                    table,
                    detail: format!("line {ln}: no record with label {label}"),
                })?;
            if record == AMBIGUOUS {
                return Err(BodyError::NoRecord {
                    table,
                    detail: format!("line {ln}: label {label} carried by several records"),
                });
            }
            let dom = &tab.domains[record];
            if let Some(tg) = l.target {
                self.check_target(dom, tg)
                    .map_err(|detail| BodyError::TargetMismatch {
                        table,
                        record,
                        detail,
                    })?;
            }
            let fixed = skip_fixed(dom);
            self.set_with_fixed(table, record, &fixed, &l.rhs)?;
            st.rules += 1;
            st.patterns += l.rhs.len();
        }
        st.entries = self.ent.len() - e0;
        Ok(st)
    }

    /// Load the `rhs_keys` of a table file's records (read with `RhsMode::KeysOnly`); records are matched to the
    /// table by label (`i`, else `index`). Records without `rhs_keys` are skipped (counted out of `rules`).
    pub fn load_table_records(
        &mut self,
        disp: &Dispatcher,
        table: usize,
        tf: &TableFile,
    ) -> Result<BodyLoadStats, BodyError> {
        let mut st = BodyLoadStats::default();
        let e0 = self.ent.len();
        for rec in &tf.rules {
            st.lines += 1;
            let Some(rhs) = &rec.rhs_keys else { continue };
            // eps tables: the label is (chart, index) packed (Table::record_label), never the raw index alone
            let Some(label) = disp.tables()[table].record_label(rec) else {
                continue;
            };
            let Some(&record) = self.labels.get(table).and_then(|m| m.get(&label)) else {
                continue;
            };
            if record == AMBIGUOUS {
                return Err(BodyError::NoRecord {
                    table,
                    detail: format!("label {label} carried by several records"),
                });
            }
            let dom = &disp.tables()[table].domains[record];
            if let Some(tg) = &rec.target {
                self.check_target(dom, tg)
                    .map_err(|detail| BodyError::TargetMismatch {
                        table,
                        record,
                        detail,
                    })?;
            }
            let fixed = skip_fixed(dom);
            self.set_with_fixed(table, record, &fixed, rhs)?;
            st.rules += 1;
            st.patterns += rhs.len();
        }
        st.entries = self.ent.len() - e0;
        Ok(st)
    }

    /// Copy the body of (from_table, from_record) to (table, record) (e.g. T178 R0 <- R0_try1, byte-identical cases).
    pub fn alias(&mut self, from: (usize, usize), to: (usize, usize)) -> Result<(), BodyError> {
        let f = self.rule_first[from.0][from.1];
        if f == NONE {
            return Err(BodyError::NoRecord {
                table: from.0,
                detail: format!("record {} has no body", from.1),
            });
        }
        if self.rule_first[to.0][to.1] != NONE {
            return Err(BodyError::Duplicate {
                table: to.0,
                record: to.1,
            });
        }
        self.rule_first[to.0][to.1] = f;
        self.rule_count[to.0][to.1] = self.rule_count[from.0][from.1];
        Ok(())
    }

    pub fn has(&self, table: usize, record: usize) -> bool {
        self.rule_first
            .get(table)
            .and_then(|t| t.get(record))
            .is_some_and(|&f| f != NONE)
    }
    pub fn n_terms(&self, table: usize, record: usize) -> usize {
        if self.has(table, record) {
            self.rule_count[table][record] as usize
        } else {
            0
        }
    }
    /// Records of `table` with a body.
    pub fn count_with_body(&self, table: usize) -> usize {
        self.rule_first[table]
            .iter()
            .filter(|&&f| f != NONE)
            .count()
    }

    /// Entries of pattern `p`.
    #[inline]
    fn pattern(&self, p: usize) -> &[u16] {
        &self.ent[self.pat_start[p] as usize..self.pat_start[p + 1] as usize]
    }

    /// Apply pattern entries to the rule point; returns the mask of slots whose value is a CONSTANT of the rule: set by
    /// a constant, or derived (copied, possibly with an offset) from a rule-point parameter in `const_params` (a fixed
    /// slot of the case; re-audit N2). Every other slot comes from a symbolic parameter.
    #[inline]
    fn apply(entries: &[u16], pt: &[i32], out: &mut [i32], const_params: u64) -> u64 {
        out.copy_from_slice(pt);
        let mut cm = const_params;
        for &e in entries {
            let (s, k, v) = entry_parts(e);
            match k {
                KIND_ADD => out[s] += v as i32,
                KIND_SET => {
                    out[s] = v as i32;
                    cm |= 1 << s;
                }
                _ => {
                    out[s] = pt[v as usize];
                    if const_params >> v as usize & 1 == 1 {
                        cm |= 1 << s;
                    } else {
                        cm &= !(1 << s);
                    }
                }
            }
        }
        cm
    }

    /// Rule-point parameters that are constants of the rule (re-audit N2): canonical target -> the case's fixed slots;
    /// target pattern -> the parameters no target slot sets (they take the fixed value or 0).
    fn const_params(dom: &CaseDomain) -> u64 {
        match &dom.target {
            TargetMap::Canonical => dom
                .fixed
                .iter()
                .enumerate()
                .filter(|(_, f)| f.is_some())
                .fold(0u64, |m, (s, _)| m | 1 << s),
            TargetMap::Pattern(t) => {
                let mut m = if dom.arity >= 64 {
                    u64::MAX
                } else {
                    (1u64 << dom.arity) - 1
                };
                for s in t {
                    if let TargetSlot::Param { param, .. } = *s
                        && param < 64
                    {
                        m &= !(1u64 << param);
                    }
                }
                m
            }
        }
    }

    /// Instantiate the body of `rule` at `key` (point map of its domain): appends `n_terms * arity` values to
    /// `out` and one constant-slot mask per term to `cmask`. Returns the number of terms; `None` if the record has
    /// no body.
    pub fn instantiate(
        &self,
        disp: &Dispatcher,
        rule: RuleRef,
        key: &[i32],
        out: &mut Vec<i32>,
        cmask: &mut Vec<u64>,
    ) -> Result<Option<usize>, BodyError> {
        if !self.has(rule.table, rule.record) {
            return Ok(None);
        }
        let dom = &disp.tables()[rule.table].domains[rule.record];
        let mut pt = Vec::with_capacity(self.arity);
        if !rule_point(dom, key, &mut pt) {
            return Err(BodyError::RulePoint {
                table: rule.table,
                record: rule.record,
            });
        }
        let f = self.rule_first[rule.table][rule.record] as usize;
        let c = self.rule_count[rule.table][rule.record] as usize;
        let n = self.arity;
        let cp = Self::const_params(dom);
        let base = out.len();
        out.resize(base + c * n, 0);
        for j in 0..c {
            let m = Self::apply(
                self.pattern(f + j),
                &pt,
                &mut out[base + j * n..base + (j + 1) * n],
                cp,
            );
            cmask.push(m);
        }
        Ok(Some(c))
    }

    /// Feed the body of (table, record) into `h`: the number of patterns, then every pattern's compact entries
    /// (slot, kind, value: a faithful encoding given the record's domain); `none` without a body.
    pub fn digest_record(&self, table: usize, record: usize, h: &mut super::sha256::Sha256) {
        if !self.has(table, record) {
            h.update(b"none;");
            return;
        }
        let f = self.rule_first[table][record] as usize;
        let c = self.rule_count[table][record] as usize;
        let mut buf: Vec<u8> = Vec::with_capacity(256);
        buf.extend_from_slice(&(c as u64).to_le_bytes());
        for j in 0..c {
            let p = self.pattern(f + j);
            buf.extend_from_slice(&(p.len() as u32).to_le_bytes());
            for &e in p {
                buf.extend_from_slice(&e.to_le_bytes());
            }
            if buf.len() >= 4096 {
                h.update(&buf);
                buf.clear();
            }
        }
        h.update(&buf);
    }

    pub fn memory_bytes(&self) -> usize {
        self.ent.capacity() * 2
            + self.pat_start.capacity() * 4
            + self
                .rule_first
                .iter()
                .map(|v| v.capacity() * 8)
                .sum::<usize>()
            + self.labels.iter().map(|m| m.capacity() * 24).sum::<usize>()
    }
    pub fn n_patterns(&self) -> usize {
        self.pat_start.len() - 1
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn parse_patterns() {
        let p = parse_rhs_pattern("I(n0-1,1,n2+1,n1)", 4).unwrap();
        assert_eq!(
            p,
            vec![
                RhsSlot::Param {
                    param: 0,
                    offset: -1
                },
                RhsSlot::Const(1),
                RhsSlot::Param {
                    param: 2,
                    offset: 1
                },
                RhsSlot::Param {
                    param: 1,
                    offset: 0
                }
            ]
        );
        assert!(parse_rhs_pattern("I(n0,1)", 3).is_err());
        assert!(parse_rhs_pattern("I(n7,1,1)", 3).is_err());
        assert!(parse_rhs_pattern("I(n0+-1,1,1)", 3).is_err());
    }
    #[test]
    fn classify_order() {
        let s = Sectors {
            lines: vec![true, true, false, false],
            forced: 0,
            zero_sectors: Some([2u64].into()),
        };
        assert_eq!(s.classify_masked(&[1, 2, 0, -1], 0), TermClass::Top);
        assert_eq!(
            s.classify_masked(&[0, 2, 1, -1], 0),
            TermClass::PositiveNumerator {
                slots: 4,
                symbolic: true,
                subsector: true
            }
        );
        assert_eq!(
            s.classify_masked(&[1, 2, 1, -1], 4),
            TermClass::PositiveNumerator {
                slots: 4,
                symbolic: false,
                subsector: false
            }
        );
        assert_eq!(
            s.classify_masked(&[1, 0, 0, -1], 0),
            TermClass::ZeroSector { dropped: 2 }
        );
        assert_eq!(
            s.classify_masked(&[0, 1, 0, -1], 0),
            TermClass::Subsector { dropped: 1 }
        );
    }
}
