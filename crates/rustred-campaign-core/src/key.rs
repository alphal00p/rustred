//! Arity-generic integral keys and the node frame.
//!
//! A key is one integral: one `i32` index per slot (`&[i32]`, owned as [`Key`] = `Vec<i32>`). The arity N is a runtime
//! parameter (20 for five-loop nodes; 5, 9, 12, 14 for lower-loop owners). The [`NodeFrame`] says which slots are LINES
//! (propagators of the node's top sector; index >= 1 when present) and which are NUMERATORS (irreducible scalar
//! products; index <= 0 in the chart).
//!
//! Semantics decided here (and used by the domain and coverage modules):
//! - **Sector** of a key = the set of line slots with index >= 1 ([`NodeFrame::sector`]). A line slot with index <= 0
//!   is a REMOVED line: the key belongs to a lower sector (subsector).
//! - **Top sector** ([`NodeFrame::is_top_sector`]): every line slot >= 1. Numerators are not part of this predicate.
//! - **Chart orthant** ([`NodeFrame::in_chart_orthant`]): top sector AND every numerator slot <= 0. A numerator slot with
//!   index > 0 (a POSITIVE NUMERATOR, i.e. the scalar product in the denominator) puts a top-sector key outside the chart.
//!   Such keys arise only as right-hand-side terms; the engine treats them as an error ("ISP in a denominator") unless
//!   the coefficient vanishes. The chart test is separate from the case-domain test (see `domain`).
//! - [`NodeFrame::classify`] names the three situations; dots D and rank r are defined on chart keys only.
//! - **Eps-shifted families** (header `power_shifts`): a slot with shift `k (4-d)/2` (`k` a nonzero integer, |k| <= 64,
//!   exactly the engine replay's `eps_multiple`, `node/replay/family.rs:104-117`) is an EPS AXIS: the integral's power
//!   there is `n_i + k eps`, the key stays the integer vector `n`. Read with [`NodeFrame::from_header_eps_aware`] ONLY;
//!   [`NodeFrame::from_header`] (every consumer not audited for eps families) refuses them with
//!   [`FrameError::Unsupported`], exactly as before. Semantics of an eps axis (engine: `propagator/custom.rs:319-333`
//!   charts, `:789-799` support; replay `family.rs:413-417` zero sectors; `rustred-order` `descriptor.rs:41-47`):
//!   it is a FORCED axis, in the sector of every key whatever the sign of `n_i` (never a removed line); the NODE's
//!   chart orthant leaves it free (any integer); one engine CHART per sign pattern of the eps axes
//!   ([`NodeFrame::charts`], guards `two_sided:n>=1` / `two_sided:n<=0`, [`NodeFrame::chart_guards`]); a key's chart is
//!   [`NodeFrame::chart_of`]. Dots / rank ([`NodeFrame::dots`], [`NodeFrame::rank`], [`NodeFrame::chart_layer`]) are
//!   defined for integer frames only (`None` / empty on an eps frame: the box convention of eps lines is spec data,
//!   BOXDEF B). Coefficients need no shift (the table's texts are functions of d and n). An eps shift on a numerator
//!   slot, or eps axes that differ from the header's two-sided order flags, is refused. Two-sided flags of an
//!   integer frame are carried only. The frame of a table is built FROM ITS HEADER, which reads `mask`,
//!   `denominators`, `power_shifts` and the two-sided flags itself. A frame without a header (the T162 bank has none;
//!   unit tests) needs one of the visibly named escape hatches [`NodeFrame::without_header_from_mask`] /
//!   [`NodeFrame::without_header_lines_first`], which record [`FrameSource::WithoutHeader`] (audit A M5) and have no
//!   eps axes.
//! - Predicates on a key of the wrong length fail closed (`false` / `None`); [`NodeFrame::sector`] and
//!   [`NodeFrame::sector_slots`] panic on it (documented hot paths), their checked variants
//!   [`NodeFrame::try_sector`] / [`NodeFrame::try_sector_slots`] and [`NodeFrame::classify`] return
//!   [`FrameError::Arity`]. Campaign code that handles keys from data uses the checked variants.

use std::fmt;

/// An owned integral key.
pub type Key = Vec<i32>;

/// Errors about keys and frames.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum FrameError {
    /// A key whose length differs from the frame arity.
    Arity { expected: usize, got: usize },
    /// A feature of the frame that integer-index evaluation does not support (e.g. a non-zero power shift).
    Unsupported(String),
    /// A malformed frame description (mask, shifts, ...).
    Malformed(String),
}

impl fmt::Display for FrameError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            FrameError::Arity { expected, got } => {
                write!(f, "key of arity {got}, frame arity {expected}")
            }
            FrameError::Unsupported(s) => write!(f, "unsupported: {s}"),
            FrameError::Malformed(s) => write!(f, "malformed frame: {s}"),
        }
    }
}
impl std::error::Error for FrameError {}

/// Where a key lies relative to the frame.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum KeyClass {
    /// Every line >= 1 and every numerator <= 0.
    Chart,
    /// Every line >= 1, but these numerator slots are > 0.
    TopPositiveNumerator { slots: Vec<usize> },
    /// These line slots are <= 0 (removed lines); `positive_numerators` lists numerator slots > 0, if any.
    Subsector {
        removed_lines: Vec<usize>,
        positive_numerators: Vec<usize>,
    },
}

/// How a frame was built.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum FrameSource {
    /// From a table header by [`NodeFrame::from_header`]: shifts and flags read from the header.
    Header,
    /// Escape hatch without a header: no power shifts are known; the caller asserts the source has none.
    WithoutHeader,
}

/// The frame of a node (or lower-loop owner): arity, line/numerator slots, power shifts.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NodeFrame {
    is_line: Vec<bool>,
    /// Header `power_shifts`, verbatim (one string per slot; "0" = no shift). Empty = none given.
    power_shifts: Vec<String>,
    /// Per slot the integer `k` of the power shift `k (4-d)/2` (0 = no shift); one entry per slot.
    kshift: Vec<i64>,
    /// Slots flagged two-sided by the header `order.two_sided` (carried only).
    two_sided: Vec<usize>,
    source: FrameSource,
}

fn mask_bits(mask: &str) -> Result<Vec<bool>, FrameError> {
    mask.chars()
        .map(|c| match c {
            '1' => Ok(true),
            '0' => Ok(false),
            _ => Err(FrameError::Malformed(format!("mask character {c:?}"))),
        })
        .collect()
}

impl NodeFrame {
    /// The frame of a table, from its header (both JSONL formats), for consumers NOT audited for eps families:
    /// `mask` (required, `'1'`/`'0'` only), `denominators` (required, must equal the mask length), `power_shifts`
    /// (absent or `null` = none; otherwise one entry per slot and every entry exactly the JSON string `"0"`, as the
    /// engine's `mastermap::Table::load`: a numeric `0`, a padded `"0 "`, an eps shift or any other value is
    /// [`FrameError::Unsupported`]), and the two-sided flags ([`crate::record::TableHeader::two_sided`]).
    pub fn from_header(h: &crate::record::TableHeader) -> Result<Self, FrameError> {
        Self::header_frame(h, false)
    }
    /// [`Self::from_header`] for consumers audited for eps-shifted families: a `power_shifts` entry other than `"0"`
    /// must be a JSON string whose value is `k (4-d)/2` with `k` a nonzero integer, |k| <= 64 (exact affine reading in
    /// d, `replay_base::family::eps_multiple`, as the engine replay), on a LINE slot; the eps axes must equal the
    /// header's two-sided order flags (`order.two_sided`: the comparator's two-sided axes). Anything else is
    /// [`FrameError::Unsupported`] (never read as a zero shift).
    pub fn from_header_eps_aware(h: &crate::record::TableHeader) -> Result<Self, FrameError> {
        Self::header_frame(h, true)
    }
    fn header_frame(h: &crate::record::TableHeader, eps_aware: bool) -> Result<Self, FrameError> {
        let mask = h
            .mask
            .as_deref()
            .ok_or_else(|| FrameError::Malformed("header without mask".into()))?;
        let is_line = mask_bits(mask)?;
        let n = is_line.len();
        match h.denominators {
            Some(d) if d == n => {}
            Some(d) => {
                return Err(FrameError::Malformed(format!(
                    "header denominators {d}, mask of length {n}"
                )));
            }
            None => return Err(FrameError::Malformed("header without denominators".into())),
        }
        let mut power_shifts = vec![];
        let mut kshift = vec![0i64; n];
        match h.raw.get("power_shifts") {
            None | Some(serde_json::Value::Null) => {}
            Some(serde_json::Value::Array(a)) => {
                if a.len() != n {
                    return Err(FrameError::Malformed(format!(
                        "power_shifts of length {}, arity {n}",
                        a.len()
                    )));
                }
                for (i, x) in a.iter().enumerate() {
                    match x.as_str() {
                        Some("0") => power_shifts.push("0".to_string()),
                        Some(s) if eps_aware => {
                            let k = crate::replay_base::family::eps_multiple(
                                &crate::independent_algebra::Independent,
                                s,
                                n,
                            )
                            .map_err(|e| {
                                FrameError::Unsupported(format!(
                                    "power shift {x} of slot {i}: {e} (only nonzero integer multiples of eps = (4-d)/2 \
                                     are eps axes)"
                                ))
                            })?;
                            if !is_line[i] {
                                return Err(FrameError::Unsupported(format!(
                                    "eps shift {x} on the numerator slot {i} (an eps axis must be a line)"
                                )));
                            }
                            kshift[i] = k;
                            power_shifts.push(s.to_string());
                        }
                        _ => {
                            return Err(FrameError::Unsupported(format!(
                                "power shift {x} of slot {i}: eps-shifted family refused by an integer-only consumer \
                                 (frame from NodeFrame::from_header; the engine's master-map loader accepts only the \
                                 string \"0\")"
                            )));
                        }
                    }
                }
            }
            Some(o) => return Err(FrameError::Malformed(format!("power_shifts {o}"))),
        }
        if !h.two_sided.is_empty() && h.two_sided.len() != n {
            return Err(FrameError::Malformed(format!(
                "order.two_sided of length {}, arity {n}",
                h.two_sided.len()
            )));
        }
        let two_sided: Vec<usize> = (0..h.two_sided.len()).filter(|&i| h.two_sided[i]).collect();
        let eps_axes: Vec<usize> = (0..n).filter(|&i| kshift[i] != 0).collect();
        if !eps_axes.is_empty() && two_sided != eps_axes {
            return Err(FrameError::Unsupported(format!(
                "eps axes {eps_axes:?} differ from the two-sided order axes {two_sided:?} (the comparator would not \
                 treat the eps axes as forced)"
            )));
        }
        Ok(NodeFrame {
            is_line,
            power_shifts,
            kshift,
            two_sided,
            source: FrameSource::Header,
        })
    }
    /// ESCAPE HATCH (no header): line slots first, `0..n_lines` lines, `n_lines..arity` numerators. No power shifts
    /// are known: the caller asserts the source has none. Panics if `n_lines > arity` (constant arguments).
    pub fn without_header_lines_first(arity: usize, n_lines: usize) -> Self {
        assert!(n_lines <= arity, "n_lines {n_lines} > arity {arity}");
        NodeFrame {
            is_line: (0..arity).map(|i| i < n_lines).collect(),
            power_shifts: vec![],
            kshift: vec![0; arity],
            two_sided: vec![],
            source: FrameSource::WithoutHeader,
        }
    }
    /// ESCAPE HATCH (no header): frame from a mask string (`'1'` = line, `'0'` = numerator). No power shifts are
    /// known: the caller asserts the source has none.
    pub fn without_header_from_mask(mask: &str) -> Result<Self, FrameError> {
        let is_line = mask_bits(mask)?;
        Ok(NodeFrame {
            kshift: vec![0; is_line.len()],
            is_line,
            power_shifts: vec![],
            two_sided: vec![],
            source: FrameSource::WithoutHeader,
        })
    }
    pub fn source(&self) -> &FrameSource {
        &self.source
    }
    /// Header `power_shifts`, verbatim (`"0"` or an eps shift; empty when the header gave none or the frame has no
    /// header). Two frames with the same shifts written differently compare unequal here (strict); [`Self::kshift`]
    /// is the exact value.
    pub fn power_shifts(&self) -> &[String] {
        &self.power_shifts
    }
    /// Per slot the integer `k` of the power shift `k (4-d)/2` (0 = no shift).
    pub fn kshift(&self) -> &[i64] {
        &self.kshift
    }
    /// The eps axes (slots with a nonzero shift), ascending.
    pub fn eps_axes(&self) -> Vec<usize> {
        (0..self.arity()).filter(|&i| self.kshift[i] != 0).collect()
    }
    /// Whether slot `slot` is an eps axis (forced, two-sided).
    pub fn is_eps_axis(&self, slot: usize) -> bool {
        self.kshift.get(slot).is_some_and(|&k| k != 0)
    }
    /// Whether the frame has an eps axis.
    pub fn has_eps_axes(&self) -> bool {
        self.kshift.iter().any(|&k| k != 0)
    }
    /// The engine charts of the frame (`propagator/custom.rs:319-333`, `table_dump.rs:997-1010`): every integer line
    /// `true`, every numerator `false`, each eps axis either `true` (`n >= 1`) or `false` (`n <= 0`): 2^(eps axes)
    /// masks, ascending as bit strings. An integer frame has the one chart [`Self::mask`].
    pub fn charts(&self) -> Vec<Vec<bool>> {
        let axes = self.eps_axes();
        let mut out: Vec<Vec<bool>> = (0..1u64 << axes.len())
            .map(|bits| {
                let mut c = self.is_line.clone();
                for (j, &a) in axes.iter().enumerate() {
                    c[a] = bits >> j & 1 == 1;
                }
                c
            })
            .collect();
        out.sort();
        out
    }
    /// The chart of a key ([`Self::charts`]): the mask with each eps axis set to `key >= 1`. `None` for a key of the
    /// wrong length. Says nothing about whether the key is in the node's chart orthant ([`Self::in_chart_orthant`]).
    pub fn chart_of(&self, key: &[i32]) -> Option<Vec<bool>> {
        (key.len() == self.arity()).then(|| {
            (0..self.arity())
                .map(|i| {
                    if self.is_eps_axis(i) {
                        key[i] >= 1
                    } else {
                        self.is_line[i]
                    }
                })
                .collect()
        })
    }
    /// The engine's guard strings of a chart (`table_dump.rs:1149-1156`): `n>=1` / `n<=0`, prefixed `two_sided:` on
    /// an eps axis. `None` if `chart` is not one of [`Self::charts`].
    pub fn chart_guards(&self, chart: &[bool]) -> Option<Vec<String>> {
        if chart.len() != self.arity()
            || (0..self.arity()).any(|i| !self.is_eps_axis(i) && chart[i] != self.is_line[i])
        {
            return None;
        }
        Some(
            (0..self.arity())
                .map(|i| {
                    let sign = if chart[i] { "n>=1" } else { "n<=0" };
                    if self.is_eps_axis(i) {
                        format!("two_sided:{sign}")
                    } else {
                        sign.to_string()
                    }
                })
                .collect(),
        )
    }
    /// Slots flagged two-sided by the header (carried only).
    pub fn two_sided(&self) -> &[usize] {
        &self.two_sided
    }
    /// The same line mask (equality of the slot structure, whatever the source).
    pub fn same_mask(&self, o: &NodeFrame) -> bool {
        self.is_line == o.is_line
    }
    /// The line mask as booleans.
    pub fn mask(&self) -> &[bool] {
        &self.is_line
    }
    pub fn arity(&self) -> usize {
        self.is_line.len()
    }
    pub fn n_lines(&self) -> usize {
        self.is_line.iter().filter(|x| **x).count()
    }
    pub fn is_line(&self, slot: usize) -> bool {
        self.is_line[slot]
    }
    /// The chart mask string (`'1'` line, `'0'` numerator).
    pub fn mask_string(&self) -> String {
        self.is_line
            .iter()
            .map(|b| if *b { '1' } else { '0' })
            .collect()
    }
    /// True if every power shift is zero (or none is given): no eps axis.
    pub fn is_integer_only(&self) -> bool {
        !self.has_eps_axes()
    }
    /// `Err(Unsupported)` when the frame has an eps axis (a consumer that is integer-index only).
    pub fn require_integer_only(&self) -> Result<(), FrameError> {
        self.require_integer_only_for("this consumer")
    }
    /// [`Self::require_integer_only`] naming the refusing `feature` in the error.
    pub fn require_integer_only_for(&self, feature: &str) -> Result<(), FrameError> {
        if self.is_integer_only() {
            Ok(())
        } else {
            Err(FrameError::Unsupported(format!(
                "eps-shifted family (power shifts {:?}, eps axes {:?}): {feature} is integer-index only",
                self.power_shifts,
                self.eps_axes()
            )))
        }
    }
    pub fn check_arity(&self, key: &[i32]) -> Result<(), FrameError> {
        if key.len() == self.arity() {
            Ok(())
        } else {
            Err(FrameError::Arity {
                expected: self.arity(),
                got: key.len(),
            })
        }
    }
    /// The sector of a key: bit i set iff slot i is a line with index >= 1 or an eps axis (forced, whatever its
    /// index) (slots >= 64 are not representable: use [`NodeFrame::sector_slots`]).
    /// Panics if the key length differs from the arity.
    pub fn sector(&self, key: &[i32]) -> u64 {
        assert!(self.arity() <= 64, "sector bitmask needs arity <= 64");
        assert_eq!(key.len(), self.arity(), "NodeFrame::sector: key arity");
        let mut m = 0u64;
        for (i, &x) in key.iter().enumerate() {
            if self.in_sector(i, x) {
                m |= 1 << i;
            }
        }
        m
    }
    /// [`Self::sector`] with typed errors instead of panics (re-audit RB m5): a key of the wrong length is
    /// [`FrameError::Arity`], an arity above 64 is [`FrameError::Unsupported`].
    pub fn try_sector(&self, key: &[i32]) -> Result<u64, FrameError> {
        if self.arity() > 64 {
            return Err(FrameError::Unsupported(format!(
                "sector bitmask needs arity <= 64, frame arity {}",
                self.arity()
            )));
        }
        self.check_arity(key)?;
        Ok(self.sector(key))
    }
    /// [`Self::sector_slots`] with a typed error instead of a panic (re-audit RB m5).
    pub fn try_sector_slots(&self, key: &[i32]) -> Result<Vec<usize>, FrameError> {
        self.check_arity(key)?;
        Ok(self.sector_slots(key))
    }
    /// Line slots with index >= 1, and every eps axis. Panics if the key length differs from the arity.
    pub fn sector_slots(&self, key: &[i32]) -> Vec<usize> {
        assert_eq!(
            key.len(),
            self.arity(),
            "NodeFrame::sector_slots: key arity"
        );
        key.iter()
            .enumerate()
            .filter(|(i, x)| self.in_sector(*i, **x))
            .map(|(i, _)| i)
            .collect()
    }
    /// Slot `i` with index `x` is in the key's sector: an eps axis always, a line iff `x >= 1`.
    fn in_sector(&self, i: usize, x: i32) -> bool {
        self.is_eps_axis(i) || (self.is_line[i] && x >= 1)
    }
    /// Slot `i` with index `x` satisfies the node's chart orthant: an eps axis always (two-sided), a line iff
    /// `x >= 1`, a numerator iff `x <= 0`.
    fn in_orthant(&self, i: usize, x: i32) -> bool {
        if self.is_eps_axis(i) {
            true
        } else if self.is_line[i] {
            x >= 1
        } else {
            x <= 0
        }
    }
    /// Every line slot >= 1 except eps axes (forced: any index); numerators unrestricted. `false` for a key of the
    /// wrong length.
    pub fn is_top_sector(&self, key: &[i32]) -> bool {
        key.len() == self.arity()
            && key
                .iter()
                .enumerate()
                .all(|(i, &x)| !self.is_line[i] || self.in_sector(i, x))
    }
    /// Top sector and every numerator slot <= 0 (eps axes: any index; the union of [`Self::charts`]). `false` for a
    /// key of the wrong length.
    pub fn in_chart_orthant(&self, key: &[i32]) -> bool {
        key.len() == self.arity() && key.iter().enumerate().all(|(i, &x)| self.in_orthant(i, x))
    }
    /// First slot violating the chart orthant, if any. A key of the wrong length violates at slot
    /// `min(len, arity)`.
    pub fn chart_violation(&self, key: &[i32]) -> Option<usize> {
        if key.len() != self.arity() {
            return Some(key.len().min(self.arity()));
        }
        key.iter()
            .enumerate()
            .position(|(i, &x)| !self.in_orthant(i, x))
    }
    pub fn classify(&self, key: &[i32]) -> Result<KeyClass, FrameError> {
        self.check_arity(key)?;
        let removed: Vec<usize> = (0..key.len())
            .filter(|&i| self.is_line[i] && !self.in_sector(i, key[i]))
            .collect();
        let pos: Vec<usize> = (0..key.len())
            .filter(|&i| !self.is_line[i] && key[i] > 0)
            .collect();
        Ok(if !removed.is_empty() {
            KeyClass::Subsector {
                removed_lines: removed,
                positive_numerators: pos,
            }
        } else if !pos.is_empty() {
            KeyClass::TopPositiveNumerator { slots: pos }
        } else {
            KeyClass::Chart
        })
    }
    /// Dots D = sum over lines of (n - 1); `None` outside the chart orthant and on an eps frame (the box
    /// convention of eps lines is spec data).
    pub fn dots(&self, key: &[i32]) -> Option<i64> {
        (self.is_integer_only() && self.in_chart_orthant(key)).then(|| {
            key.iter()
                .enumerate()
                .filter(|(i, _)| self.is_line[*i])
                .map(|(_, &x)| x as i64 - 1)
                .sum()
        })
    }
    /// Rank r = sum over numerators of |n|; `None` outside the chart orthant and on an eps frame.
    pub fn rank(&self, key: &[i32]) -> Option<i64> {
        (self.is_integer_only() && self.in_chart_orthant(key)).then(|| {
            key.iter()
                .enumerate()
                .filter(|(i, _)| !self.is_line[*i])
                .map(|(_, &x)| -(x as i64))
                .sum()
        })
    }

    /// All chart keys with dots exactly `dots` and rank exactly `rank`, line powers at most `max_line_power`
    /// (`None` = unbounded, i.e. bounded by `dots`). Lexicographically sorted. Empty on an eps frame (dots and rank
    /// are defined for integer frames only).
    pub fn chart_layer(&self, dots: u32, rank: u32, max_line_power: Option<i32>) -> Vec<Key> {
        if !self.is_integer_only() {
            return vec![];
        }
        let lines: Vec<usize> = (0..self.arity()).filter(|&i| self.is_line[i]).collect();
        let nums: Vec<usize> = (0..self.arity()).filter(|&i| !self.is_line[i]).collect();
        let cap = max_line_power.map_or(dots, |p| (p - 1).max(0) as u32);
        let dl = compositions(dots, lines.len(), cap);
        let nl = compositions(rank, nums.len(), rank);
        let mut out = Vec::with_capacity(dl.len() * nl.len());
        for a in &dl {
            for b in &nl {
                let mut k = vec![0i32; self.arity()];
                for (j, &s) in lines.iter().enumerate() {
                    k[s] = 1 + a[j] as i32;
                }
                for (j, &s) in nums.iter().enumerate() {
                    k[s] = -(b[j] as i32);
                }
                out.push(k);
            }
        }
        out.sort();
        out
    }
}

/// All vectors of `parts` non-negative integers <= `cap` summing to `total`.
pub fn compositions(total: u32, parts: usize, cap: u32) -> Vec<Vec<u32>> {
    fn rec(total: u32, parts: usize, cap: u32, cur: &mut Vec<u32>, out: &mut Vec<Vec<u32>>) {
        if parts == 0 {
            if total == 0 {
                out.push(cur.clone());
            }
            return;
        }
        for a in 0..=total.min(cap) {
            cur.push(a);
            rec(total - a, parts - 1, cap, cur, out);
            cur.pop();
        }
    }
    let mut out = vec![];
    rec(total, parts, cap, &mut Vec::with_capacity(parts), &mut out);
    out
}

/// Parse a key written `I(1,2,-1,...)` or as a JSON-like list `[1,2,-1,...]`.
pub fn parse_key(s: &str) -> Result<Key, String> {
    let t = s.trim();
    let inner = t
        .strip_prefix("I(")
        .and_then(|x| x.strip_suffix(')'))
        .or_else(|| t.strip_prefix('[').and_then(|x| x.strip_suffix(']')))
        .ok_or_else(|| format!("not a key: {s}"))?;
    if inner.trim().is_empty() {
        return Ok(vec![]);
    }
    inner
        .split(',')
        .map(|x| {
            x.trim()
                .parse::<i32>()
                .map_err(|e| format!("bad key entry {x:?}: {e}"))
        })
        .collect()
}

/// Format a key as `I(a,b,...)`.
pub fn key_text(k: &[i32]) -> String {
    format!(
        "I({})",
        k.iter().map(i32::to_string).collect::<Vec<_>>().join(",")
    )
}
