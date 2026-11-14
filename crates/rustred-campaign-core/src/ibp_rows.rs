//! A2 "IDENTITY-TESTED" (plan 2.5.2, WP-I2, design comment A6): an independent modular test that a rule
//! `target = sum_k c_k(n, d) I(rhs_k)` lies in the span of the RAW IBP identities of its family, generated from the
//! family text at the rule's RECORDED seeds. Pure logic on the copied engine replay code ([`crate::replay_base`]); no
//! engine, no Symbolica, no files, no clock in any verdict field (R22: callable in-process).
//!
//! Semantics follow the engine's replay (`replay_cells` of `rustred-core/src/node/replay/mod.rs` at b9f06de7,
//! re-implemented here): columns = every key of the seeds' rows plus the target and the RHS keys; columns in a
//! scaleless sector (Lee's criterion on U + F, numerators with index > 0 count as propagators) are dropped from rows
//! and from the rule vector; the rule vector is `e_target - sum c_k e_rhs_k`; a PASS cell must also reject the +1
//! mutant of its first nonzero RHS coefficient and keep every sector's lowest numerator-free column out of the span.
//! Differences from the replay: primes from the verify range (never P61 / P2), random d per cell recorded, generic
//! points (random field values on the symbolic slots, the checker's C1) besides integer keys, and verdicts typed
//! for the ledger (interfaces/ibp-rows.md section 2). "Fail" = not in the span of the rows of its recorded seeds at
//! that point at both primes (not derived from its seeds there); never "the rule is false".

use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::io::{BufRead, Write};
use std::time::Instant;

use serde::Serialize;
use serde_json::Value;

use crate::poly::Poly;
use crate::replay_base::arith::{self, Q};
use crate::replay_base::coeff::{self, Ctx};
use crate::replay_base::expr::Expr;
use crate::replay_base::family::{Family, family_header_check};
use crate::replay_base::primes::{self, SplitMix64};
use crate::replay_base::record::{self, Pat};
use crate::replay_base::rows::{self, Support};
use crate::replay_base::span::{Elim, MEMORY_GUARD};
use crate::replay_base::symrows::{self, Provenance, SymRowMode, VerifiedGroup};

/// Version of the row convention and verdict semantics (bump on any change of either).
pub const ROWS_VERSION: &str = "rustred.campaign.ibp_rows.v2";

/// A rule record that cannot even be read as a rule (missing / malformed required fields).
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub enum RowsError {
    Record(String),
    Io(String),
}

impl std::fmt::Display for RowsError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            RowsError::Record(s) => write!(f, "rule record: {s}"),
            RowsError::Io(s) => write!(f, "io: {s}"),
        }
    }
}

impl std::error::Error for RowsError {}

/// The A2 view of one rule record (interfaces/ibp-rows.md section 3).
#[derive(Clone, Debug)]
pub struct RowsRule {
    pub index: Option<i64>,
    pub arity: usize,
    pub chart_text: String,
    /// `true` = line slot (chart '1')
    pub chart: Vec<bool>,
    pub fixed: Vec<Option<i64>>,
    pub equations: Vec<String>,
    pub exceptions: Vec<Vec<String>>,
    pub target_text: String,
    pub target: Pat,
    /// (key pattern, coefficient text)
    pub rhs: Vec<(Pat, String)>,
    pub seeds: Vec<Pat>,
    pub recenter: Vec<i64>,
    /// the raw `sym_rows` value when the record declares symmetry rows (non-null, nonzero, not false)
    pub sym_rows: Option<String>,
    /// a key / seed pattern outside the engine's `parse_integral` grammar (the rule is then Unsupported)
    pub syntax_error: Option<String>,
    /// witness loci (polynomials in n, d), without the literal "elided"
    pub witness_loci: Vec<String>,
    /// the record's witness loci contain the literal "elided" (R24 ruling (6): UNUSABLE for `replay`)
    pub witness_elided: bool,
    /// `None` = field absent (incomplete record); `Some` = present (possibly empty)
    pub witness_present: bool,
    /// the engine node driver's raw pivot (Power bytes), when recorded: authority for the recenter
    pub raw_pivot: Option<Vec<i64>>,
    /// the rule's `guard_version` (engine guard-fix D1: 2 = witness guards on; absent = unknown here)
    pub guard_version: Option<u64>,
}

fn take_str(v: &mut Value, key: &str) -> Result<String, RowsError> {
    match v.get_mut(key).map(Value::take) {
        Some(Value::String(s)) => Ok(s),
        _ => Err(RowsError::Record(format!(
            "field `{key}` missing or not a string"
        ))),
    }
}

/// An array field, or a JSON string holding the array (the checker accepts both for `rhs` and `seeds`).
fn take_array(v: &mut Value, key: &str) -> Result<Option<Vec<Value>>, RowsError> {
    match v.get_mut(key).map(Value::take) {
        None | Some(Value::Null) => Ok(None),
        Some(Value::Array(a)) => Ok(Some(a)),
        Some(Value::String(s)) => match serde_json::from_str::<Value>(&s) {
            Ok(Value::Array(a)) => Ok(Some(a)),
            _ => Err(RowsError::Record(format!(
                "field `{key}` is a string but not a JSON array"
            ))),
        },
        Some(_) => Err(RowsError::Record(format!("field `{key}` is not an array"))),
    }
}

fn strings(a: Vec<Value>, key: &str) -> Result<Vec<String>, RowsError> {
    a.into_iter()
        .map(|x| match x {
            Value::String(s) => Ok(s),
            _ => Err(RowsError::Record(format!("`{key}` holds a non-string"))),
        })
        .collect()
}

impl RowsRule {
    /// Read one rule record (a `table_dump.v1` rule object or a bare export rule). Strings are moved out of `v`.
    pub fn from_value(mut v: Value, arity: usize) -> Result<RowsRule, RowsError> {
        if !v.is_object() {
            return Err(RowsError::Record("rule is not an object".into()));
        }
        let index = v.get("index").and_then(Value::as_i64);
        let chart_text = take_str(&mut v, "chart")?;
        if chart_text.len() != arity || !chart_text.bytes().all(|b| b == b'0' || b == b'1') {
            return Err(RowsError::Record(format!(
                "chart {chart_text:?} is not a 0/1 string of length {arity}"
            )));
        }
        let chart = chart_text.bytes().map(|b| b == b'1').collect();
        let fixed: Vec<Option<i64>> = match v.get("fixed") {
            Some(Value::Array(a)) if a.len() == arity => a
                .iter()
                .map(|x| match x {
                    Value::Null => Ok(None),
                    x => x.as_i64().map(Some).ok_or_else(|| {
                        RowsError::Record("fixed entry is not an integer or null".into())
                    }),
                })
                .collect::<Result<_, _>>()?,
            _ => {
                return Err(RowsError::Record(format!(
                    "`fixed` must be an array of length {arity}"
                )));
            }
        };
        let equations = strings(
            take_array(&mut v, "equations")?.unwrap_or_default(),
            "equations",
        )?;
        let exceptions = take_array(&mut v, "exceptions")?
            .unwrap_or_default()
            .into_iter()
            .map(|b| match b {
                Value::Array(a) => strings(a, "exceptions"),
                _ => Err(RowsError::Record("exception branch is not an array".into())),
            })
            .collect::<Result<Vec<_>, _>>()?;
        let target_text = take_str(&mut v, "target")?;
        let rhs_raw =
            take_array(&mut v, "rhs")?.ok_or_else(|| RowsError::Record("`rhs` missing".into()))?;
        let mut rhs_text = Vec::with_capacity(rhs_raw.len());
        for t in rhs_raw {
            match t {
                Value::Array(mut pair) if pair.len() == 2 => {
                    let c = pair.pop().unwrap();
                    let k = pair.pop().unwrap();
                    match (k, c) {
                        (Value::String(k), Value::String(c)) => rhs_text.push((k, c)),
                        _ => {
                            return Err(RowsError::Record(
                                "rhs term is not [key string, coefficient string]".into(),
                            ));
                        }
                    }
                }
                _ => return Err(RowsError::Record("rhs term is not a pair".into())),
            }
        }
        let seed_text = strings(take_array(&mut v, "seeds")?.unwrap_or_default(), "seeds")?;
        let recenter = match v.get("recenter") {
            None | Some(Value::Null) => vec![0; arity],
            Some(Value::Array(a)) if a.len() == arity => a
                .iter()
                .map(|x| {
                    x.as_i64()
                        .ok_or_else(|| RowsError::Record("recenter entry not an integer".into()))
                })
                .collect::<Result<_, _>>()?,
            Some(_) => {
                return Err(RowsError::Record(format!(
                    "`recenter` must be an array of length {arity}"
                )));
            }
        };
        let (witness_present, mut witness_elided, mut witness_loci) = (
            // audit B m3: only a LIST counts as present (a string or number is a malformed record)
            v.get("witness_loci").is_some_and(Value::is_array),
            false,
            Vec::new(),
        );
        if let Some(Value::Array(a)) = v.get("witness_loci") {
            for x in a {
                match x.as_str() {
                    Some(t) if t.trim() == "elided" => witness_elided = true,
                    Some(t) => witness_loci.push(t.to_string()),
                    None => return Err(RowsError::Record("witness locus is not a string".into())),
                }
            }
        }
        let raw_pivot = match v.get("raw_pivot_encoded") {
            None | Some(Value::Null) => None,
            Some(Value::Array(a)) if a.len() == arity => Some(
                a.iter()
                    .map(|x| {
                        x.as_i64()
                            .ok_or_else(|| RowsError::Record("raw pivot entry".into()))
                    })
                    .collect::<Result<_, _>>()?,
            ),
            Some(_) => {
                return Err(RowsError::Record(format!(
                    "`raw_pivot_encoded` must have length {arity}"
                )));
            }
        };
        let guard_version = v.get("guard_version").and_then(Value::as_u64);
        let sym_rows = match v.get("sym_rows") {
            None | Some(Value::Null) | Some(Value::Bool(false)) => None,
            Some(x) if x.as_i64() == Some(0) || x.as_u64() == Some(0) => None,
            Some(x) => Some(x.to_string()),
        };
        // keys: the engine's parse_integral grammar; a refusal makes the rule Unsupported (never guessed)
        let mut syntax_error = None;
        let mut parse = |t: &str| -> Pat {
            match record::parse_integral(t, arity) {
                Ok(p) => p,
                Err(e) => {
                    syntax_error.get_or_insert(e);
                    Vec::new()
                }
            }
        };
        let target = parse(&target_text);
        let seeds: Vec<Pat> = seed_text.iter().map(|s| parse(s)).collect();
        let rhs: Vec<(Pat, String)> = rhs_text.into_iter().map(|(k, c)| (parse(&k), c)).collect();
        Ok(RowsRule {
            index,
            arity,
            chart_text,
            chart,
            fixed,
            equations,
            exceptions,
            target_text,
            target,
            rhs,
            seeds,
            recenter,
            sym_rows,
            syntax_error,
            witness_loci,
            witness_elided,
            witness_present,
            raw_pivot,
            guard_version,
        })
    }

    /// All rules of one JSONL line: a `table_dump.v1` rule line, an export wrapper `{"rules": [...]}`, or a bare
    /// record. Header / chart / residual lines give an empty list.
    pub fn from_json_line(line: &str, arity: usize) -> Result<Vec<RowsRule>, RowsError> {
        let v: Value =
            serde_json::from_str(line).map_err(|e| RowsError::Record(format!("json: {e}")))?;
        Self::from_line_value(v, arity).map(|(r, _)| r)
    }

    fn from_line_value(
        mut v: Value,
        arity: usize,
    ) -> Result<(Vec<RowsRule>, LineContext), RowsError> {
        let mut ctx = LineContext {
            run: v.get("run").and_then(Value::as_str).map(str::to_string),
            gap_points: Vec::new(),
        };
        if let Some(Value::Array(g)) = v.get("gap_points") {
            for p in g {
                let k: Option<Vec<i64>> = p
                    .as_array()
                    .map(|a| a.iter().filter_map(Value::as_i64).collect());
                if let Some(k) = k.filter(|k| k.len() == arity) {
                    ctx.gap_points.push(k);
                }
            }
        }
        if let Some(Value::Array(rules)) = v.get_mut("rules").map(Value::take) {
            let rs = rules
                .into_iter()
                .map(|r| RowsRule::from_value(r, arity))
                .collect::<Result<Vec<_>, _>>()?;
            return Ok((rs, ctx));
        }
        match v.get("type").and_then(Value::as_str) {
            Some("rule") => Ok((vec![RowsRule::from_value(v, arity)?], ctx)),
            Some(_) => Ok((Vec::new(), ctx)),
            None if v.get("target").is_some() && v.get("rhs").is_some() => {
                Ok((vec![RowsRule::from_value(v, arity)?], ctx))
            }
            None => Ok((Vec::new(), ctx)),
        }
    }

    /// Default stream id: FNV-1a of `chart|index|target`.
    pub fn default_id(&self) -> u64 {
        let mut h: u64 = 0xcbf2_9ce4_8422_2325;
        for b in format!("{}|{:?}|{}", self.chart_text, self.index, self.target_text).bytes() {
            h ^= b as u64;
            h = h.wrapping_mul(0x0100_0000_01b3);
        }
        h
    }

    /// Total number of coefficient bytes (cost accounting).
    pub fn coefficient_bytes(&self) -> usize {
        self.rhs.iter().map(|(_, c)| c.len()).sum()
    }
}

/// Context of an input line (export wrapper fields the point plan may use).
#[derive(Clone, Debug, Default)]
pub struct LineContext {
    pub run: Option<String>,
    pub gap_points: Vec<Vec<i64>>,
}

/// A point at which the identity is tested.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub enum PointSpec {
    /// an integer key in the target frame (a served key)
    Key(Vec<i64>),
    /// random field values on the symbolic slots (one draw per prime)
    Generic,
}

/// The table a rule comes from (audit C2): its header binds the family (engine `family_check`) and states the guard
/// version. `from_header` recognises a `table_dump.v1` header (`"type":"header"`) and an export `cert_header`.
#[derive(Clone, Debug)]
pub struct TableBinding {
    pub header: Value,
    /// header `guard_version` (absent = 1 in the engine's semantics)
    pub guard_version: Option<u64>,
}

impl TableBinding {
    pub fn from_header(v: &Value) -> Option<TableBinding> {
        match v.get("type").and_then(Value::as_str) {
            Some("header") | Some("cert_header") => Some(TableBinding {
                header: v.clone(),
                guard_version: v.get("guard_version").and_then(Value::as_u64),
            }),
            _ => None,
        }
    }
}

/// Which rules get symmetry rows (interfaces/ibp-rows.md 10.4).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Default)]
pub enum SymUse {
    /// exactly the rules that declare `sym_rows` (or `generated_with_symmetry_rows`): the engine's rule
    #[default]
    Declared,
    /// never (the control "without symmetry rows", also for declaring rules)
    Without,
    /// every rule (diagnostic)
    Always,
}

/// The symmetry rows used by a verdict (recorded so the evidence can name the group and the elements used).
#[derive(Clone, Debug, Serialize)]
pub struct SymmetryUse {
    pub group_digest: String,
    pub provenance: Provenance,
    pub mode: SymRowMode,
    pub elements: usize,
    /// symmetry rows generated over all cells
    pub rows_total: usize,
    /// element indices whose rows carry nonzero weight in some cell's span solution (union over cells)
    pub support: Vec<usize>,
}

#[derive(Clone, Debug)]
pub struct RowsOptions {
    /// a VERIFIED symmetry group of the rule's chart (`symrows::verify_group`); `None` = no symmetry rows possible
    pub symmetry: Option<std::sync::Arc<VerifiedGroup>>,
    pub sym_mode: SymRowMode,
    pub sym_use: SymUse,
    /// bound on the terms of one expanded (multi-term) symmetry row
    pub sym_max_terms: usize,
    /// nonce bound into the stream seed (`replay_evidence` draws one from the OS when `None`; `identity_test` uses 0)
    pub nonce: Option<u64>,
    /// the rule's table header (family binding, guard version); `None` = no header known (recorded as such)
    pub table: Option<TableBinding>,
    pub seed: u64,
    pub rule_id: Option<u64>,
    pub primes: Option<[u64; 2]>,
    pub lift_primes: Vec<u64>,
    pub generated_with_symmetry_rows: bool,
    pub max_entries: usize,
    /// guard on the estimated heap of the rows BEFORE they are built (audit B M3), default 1.2 GB
    pub max_row_bytes: usize,
    pub shell_diagnostic: bool,
    pub d_attempts: u32,
    pub controls: bool,
}

impl Default for RowsOptions {
    fn default() -> Self {
        RowsOptions {
            symmetry: None,
            sym_mode: SymRowMode::Engine,
            sym_use: SymUse::Declared,
            sym_max_terms: 200_000,
            nonce: None,
            table: None,
            seed: 20_261_113,
            rule_id: None,
            primes: None,
            lift_primes: Vec::new(),
            generated_with_symmetry_rows: false,
            max_entries: 60_000_000,
            max_row_bytes: 1_200_000_000,
            shell_diagnostic: false,
            d_attempts: 8,
            controls: true,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub enum Unsupported {
    /// the rule declares `sym_rows` or was generated with symmetry rows: UNKNOWN (needs symmetry rows), D-34
    NeedsSymmetryRows { declared: Option<String> },
    /// a key, seed or coefficient outside the grammar (refused, never guessed)
    Syntax(String),
    /// a case equation that is not affine (generic points need it solved)
    NonlinearEquation(String),
    /// the record has no seeds (nothing to build rows from)
    NoSeeds,
    /// a prime that may not decide (range, engine stream, lift prime)
    BadPrime(String),
    /// arity mismatch or a malformed record
    Record(String),
    /// the family text is outside what the replay models, or does not match the rule's table (audit C2)
    Family(String),
    /// the record has an empty exception branch (an empty AND = excepted everywhere; the engine refuses it)
    EmptyExceptionBranch { branch: usize },
    /// the rule's target lies in a zero (scaleless) sector at the point (the engine refuses it)
    TargetInZeroSector,
    /// the symmetry group is for another chart / arity / family
    SymGroupMismatch(String),
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub enum NotApplicableReason {
    Arity { expected: usize, got: usize },
    TargetConstant { slot: usize },
    FixedSlot { slot: usize },
    Equation(usize),
    OnException { branch: usize },
    ChartMismatch { slot: usize },
    InconsistentEquations,
}

/// The rule-level verdict (interfaces/ibp-rows.md section 2).
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub enum Verdict {
    Pass,
    Fail {
        point: usize,
        prime: u64,
        d: u64,
        column: String,
    },
    SeedsInsufficient {
        point: usize,
    },
    Pole {
        point: usize,
        coefficient: usize,
    },
    Unsupported(Unsupported),
    MemoryGuard {
        entries: usize,
    },
    NotApplicable {
        reasons: Vec<NotApplicableReason>,
    },
    Undecided {
        reason: String,
    },
}

impl Verdict {
    pub fn kind(&self) -> &'static str {
        match self {
            Verdict::Pass => "pass",
            Verdict::Fail { .. } => "fail",
            Verdict::SeedsInsufficient { .. } => "seeds_insufficient",
            Verdict::Pole { .. } => "pole",
            Verdict::Unsupported(Unsupported::NeedsSymmetryRows { .. }) => {
                "unsupported_needs_symmetry_rows"
            }
            Verdict::Unsupported(_) => "unsupported",
            Verdict::MemoryGuard { .. } => "memory_guard",
            Verdict::NotApplicable { .. } => "not_applicable",
            Verdict::Undecided { .. } => "undecided",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub enum CellStatus {
    Pass,
    Fail,
    /// refused at this cell (target in a zero sector)
    Refused(Unsupported),
    Pole {
        coefficient: usize,
    },
    MemoryGuard {
        entries: usize,
    },
    Undecided {
        reason: String,
    },
}

/// One (point, prime) evaluation.
#[derive(Clone, Debug, Serialize)]
pub struct CellReport {
    pub prime: u64,
    pub d: u64,
    /// residues of n0..n{N-1} at a generic point
    pub generic_values: Option<Vec<u64>>,
    pub redraws: u32,
    pub status: CellStatus,
    /// generated rows (30 per distinct seed key for T162)
    pub rows: usize,
    /// non-zero-sector columns met by the rows (the checker's `ncols`)
    pub row_columns: usize,
    pub universe_columns: usize,
    pub zero_columns: usize,
    pub rank: usize,
    pub in_span: bool,
    /// first remainder column when not in the span (the witness)
    pub witness: Option<String>,
    pub mutation_rejected: Option<bool>,
    pub corners_tested: usize,
    pub corners_in_span: Vec<String>,
    /// stored elimination entries (pivot rows + dependency lists)
    pub entries: usize,
    /// symmetry rows generated in this cell, those with nonzero weight in the span solution, and their elements
    pub sym_rows: usize,
    pub sym_rows_weighted: usize,
    pub sym_support: Vec<usize>,
    /// owed control with symmetry rows (engine replay): the chart corner in the span of the rows projected onto the
    /// chart's top-sector columns (`Some(false)` = control passed; `None` = not run: no symmetry rows)
    pub corner_projected_in_span: Option<bool>,
    /// generic point only: columns dropped as scaleless under the generic convention that are NOT scaleless on a
    /// boundary face (a symbolic non-line slot at positive offset becomes active there; audit A C1 / audit B). A
    /// generic-point pass says nothing on those faces.
    pub boundary_columns: usize,
    /// Schwartz-Zippel degree of this cell's false-PASS event
    pub sz_degree: u64,
    pub seconds: f64,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub enum PointStatus {
    Pass,
    Fail,
    SeedsInsufficient,
    Pole { coefficient: usize },
    MemoryGuard { entries: usize },
    NotApplicable(NotApplicableReason),
    Unsupported(Unsupported),
    Undecided { reason: String },
}

#[derive(Clone, Debug, Serialize)]
pub struct PointReport {
    pub spec: PointSpec,
    /// the rule point (values of n0..n{N-1}) for an integer key
    pub rule_point: Option<Vec<i64>>,
    pub status: PointStatus,
    pub cells: Vec<CellReport>,
    /// cells of the shell retry (diagnostic) when run
    pub shell: Vec<CellReport>,
}

#[derive(Clone, Debug, Default, Serialize)]
pub struct Cost {
    pub rows: usize,
    pub row_columns_max: usize,
    pub rank_max: usize,
    pub entries_peak: usize,
    /// rough upper estimate of the peak heap of one cell (entries, rows, column keys)
    pub peak_bytes_estimate: usize,
    pub coefficient_bytes: usize,
    pub monomials: usize,
    pub seconds: f64,
}

#[derive(Clone, Debug, Serialize)]
pub struct RowsVerdict {
    pub version: &'static str,
    /// sha256 of the family text used and its name (audit C2 / M4)
    pub family_digest: String,
    pub family_name: Option<String>,
    /// the table header was given and matched (`false` = no binding known)
    pub family_bound_to_table: bool,
    pub verdict: Verdict,
    pub rule_index: Option<i64>,
    pub target: String,
    pub rule_id: u64,
    /// the stream seed actually used (derived from `master_seed` and the rule digest)
    pub seed: u64,
    pub master_seed: u64,
    /// the nonce bound into `seed` (0 = none given)
    pub nonce: u64,
    pub primes: [u64; 2],
    pub points: Vec<PointReport>,
    pub points_passed: usize,
    /// product over the passed cells of deg / (p - 2) (false-PASS probability bound); 1.0 when nothing passed
    /// (may underflow to 0.0: `sz_bound_log10` is the same bound as a base-10 logarithm)
    pub sz_bound: f64,
    pub sz_bound_log10: f64,
    pub sz_bound_per_cell_max: f64,
    pub meets_a2_policy: bool,
    /// the symmetry rows used (group digest, provenance, mode, support); `None` = IBP rows only
    pub symmetry: Option<SymmetryUse>,
    pub cost: Cost,
}

// ------------------------------------------------------------------ columns ------------------------------------------

/// A column: (symbolic-slot mask, values). Integer keys have mask 0; at a generic point a slot with its bit set holds
/// the offset to the slot's random value, the others hold constants.
type ColKey = (u64, Vec<i64>);

fn col_text(k: &ColKey) -> String {
    let parts: Vec<String> =
        k.1.iter()
            .enumerate()
            .map(|(i, &v)| {
                if k.0 >> i & 1 == 1 {
                    match v {
                        0 => format!("n{i}"),
                        v if v > 0 => format!("n{i}+{v}"),
                        v => format!("n{i}{v}"),
                    }
                } else {
                    v.to_string()
                }
            })
            .collect();
    format!("I({})", parts.join(","))
}

/// Active slots of a column (integer: index > 0; generic symbolic slot: the chart's line flag, as the checker).
fn active(k: &ColKey, chart: &[bool]) -> Vec<bool> {
    k.1.iter()
        .enumerate()
        .map(|(i, &v)| if k.0 >> i & 1 == 1 { chart[i] } else { v > 0 })
        .collect()
}

/// Pivot preference (only affects fill-in): more active slots, then more excess, = more complex = pivot first.
fn col_weight(k: &ColKey, chart: &[bool]) -> (usize, i64) {
    let act = active(k, chart);
    let mut excess = 0i64;
    for (i, &v) in k.1.iter().enumerate() {
        excess += if k.0 >> i & 1 == 1 {
            if chart[i] { v } else { -v }
        } else if v > 0 {
            v - 1
        } else {
            -v
        };
    }
    (act.iter().filter(|&&a| a).count(), excess)
}

// ------------------------------------------------------------------ coefficients -------------------------------------

#[derive(Clone, Copy, Debug, Default)]
struct DegStats {
    max_num_deg: u64,
    sum_den_deg: u64,
    max_num_degd: u64,
    sum_den_degd: u64,
    monomials: usize,
}

struct CoeffEval {
    /// per context: c_k = num_k / den_k, or the first k with den_k = 0
    values: Vec<Result<Vec<u64>, usize>>,
    deg: DegStats,
}

fn eval_coeffs(rule: &RowsRule, ctxs: &[Ctx]) -> Result<CoeffEval, String> {
    let m = ctxs.len();
    let mut vals: Vec<Result<Vec<u64>, usize>> = (0..m)
        .map(|_| Ok(Vec::with_capacity(rule.rhs.len())))
        .collect();
    let mut deg = DegStats::default();
    let (mut num, mut den) = (vec![0u64; m], vec![0u64; m]);
    for (k, (_, text)) in rule.rhs.iter().enumerate() {
        let st = coeff::eval_ratio(text, rule.arity, ctxs, &mut num, &mut den)
            .map_err(|e| format!("rhs[{k}] coefficient: {e}"))?;
        deg.max_num_deg = deg.max_num_deg.max(st.num.deg as u64);
        deg.max_num_degd = deg.max_num_degd.max(st.num.degd as u64);
        deg.sum_den_deg += st.den.deg as u64;
        deg.sum_den_degd += st.den.degd as u64;
        deg.monomials += st.num.terms + st.den.terms;
        // audit M1: the copied engine expression reader must agree (first context; texts up to CROSS_CHECK_BYTES)
        if let Some(c0) = ctxs.first() {
            coeff::cross_check_engine(text, rule.arity, c0, num[0], den[0])
                .map_err(|e| format!("rhs[{k}] coefficient: {e}"))?;
        }
        for (j, c) in ctxs.iter().enumerate() {
            if let Ok(v) = &mut vals[j] {
                match arith::inv(c.p, den[j]) {
                    Some(iv) => v.push(arith::mul(c.p, num[j], iv)),
                    None => vals[j] = Err(k),
                }
            }
        }
    }
    Ok(CoeffEval { values: vals, deg })
}

/// d-degree of one coefficient's denominator (syntactic).
fn den_degd(rule: &RowsRule, k: usize, p: u64) -> Result<u32, String> {
    let c = Ctx {
        p,
        vals: vec![1; rule.arity + 1],
    };
    let (mut n, mut d) = ([0u64], [0u64]);
    let st = coeff::eval_ratio(
        &rule.rhs[k].1,
        rule.arity,
        std::slice::from_ref(&c),
        &mut n,
        &mut d,
    )
    .map_err(|e| e.to_string())?;
    Ok(st.den.degd)
}

// ------------------------------------------------------------------ draws ---------------------------------------------

const PURPOSE_D: u64 = 1;
const PURPOSE_POLE: u64 = 3;
const PURPOSE_SHELL: u64 = 4;

/// d uniform in [2, p), never 37/7 or -11/5 mod p (the engine's fixed dimensions).
fn draw_d(s: &mut SplitMix64, p: u64) -> u64 {
    let bad: Vec<u64> = [Q { n: 37, d: 7 }, Q { n: -11, d: 5 }]
        .iter()
        .filter_map(|q| q.modp(p))
        .collect();
    loop {
        let d = s.range(2, p);
        if !bad.contains(&d) {
            return d;
        }
    }
}

// ------------------------------------------------------------------ generic points -----------------------------------

/// Exact solution of the affine case equations: dependent slot -> (constant, [(free slot, coefficient)]).
type Solved = BTreeMap<usize, (Q, Vec<(usize, Q)>)>;

fn solve_equations(rule: &RowsRule) -> Result<Result<Solved, NotApplicableReason>, Unsupported> {
    let n = rule.arity;
    // rows over [slot coefficients .. , const]; fixed slots substituted
    let mut rows: Vec<Vec<Q>> = Vec::new();
    let qerr = |e: String| Unsupported::NonlinearEquation(e);
    for e in &rule.equations {
        let lin = Expr::parse(e, n)
            .and_then(|x| x.linear())
            .map_err(|err| Unsupported::NonlinearEquation(format!("{e}: {err}")))?;
        if !lin[n + 1].is_zero() {
            return Err(Unsupported::NonlinearEquation(format!("{e}: contains d")));
        }
        let mut row = vec![Q::ZERO; n + 1];
        let mut c = lin[0];
        for i in 0..n {
            let a = lin[i + 1];
            if a.is_zero() {
                continue;
            }
            match rule.fixed[i] {
                Some(v) => {
                    c = c
                        .add(a.mul(Q::int(v as i128)).map_err(qerr)?)
                        .map_err(qerr)?
                }
                None => row[i] = a,
            }
        }
        row[n] = c;
        rows.push(row);
    }
    // Gauss-Jordan
    let mut pivots: Vec<usize> = Vec::new();
    let mut r = 0;
    for col in 0..n {
        let Some(pr) = (r..rows.len()).find(|&i| !rows[i][col].is_zero()) else {
            continue;
        };
        rows.swap(r, pr);
        let f = rows[r][col];
        rows[r] = rows[r]
            .iter()
            .map(|x| x.div(f))
            .collect::<Result<_, _>>()
            .map_err(qerr)?;
        for i in 0..rows.len() {
            if i != r && !rows[i][col].is_zero() {
                let g = rows[i][col];
                let pivot = rows[r].clone();
                rows[i] = rows[i]
                    .iter()
                    .zip(&pivot)
                    .map(|(x, y)| x.sub(g.mul(*y)?))
                    .collect::<Result<_, _>>()
                    .map_err(qerr)?;
            }
        }
        pivots.push(col);
        r += 1;
    }
    for row in &rows[r..] {
        if !row[n].is_zero() {
            return Ok(Err(NotApplicableReason::InconsistentEquations));
        }
    }
    let mut out = Solved::new();
    for (i, &pc) in pivots.iter().enumerate() {
        // x_pc + sum_{j != pc} a_j x_j + c = 0  ->  x_pc = -c - sum a_j x_j
        let deps: Vec<(usize, Q)> = (0..n)
            .filter(|&j| j != pc && !rows[i][j].is_zero())
            .map(|j| (j, rows[i][j].neg()))
            .collect();
        out.insert(pc, (rows[i][n].neg(), deps));
    }
    Ok(Ok(out))
}

fn generic_values(
    rule: &RowsRule,
    solved: &Solved,
    p: u64,
    s: &mut SplitMix64,
) -> Result<Vec<u64>, String> {
    let n = rule.arity;
    let mut x = vec![0u64; n];
    for (i, xi) in x.iter_mut().enumerate() {
        *xi = match rule.fixed[i] {
            Some(v) => arith::from_i128(p, v as i128),
            None if solved.contains_key(&i) => 0,
            None => s.range(2, p),
        };
    }
    for (&i, (c, deps)) in solved {
        let mut v = c.modp(p).ok_or("equation denominator divisible by p")?;
        for (j, a) in deps {
            let av = a.modp(p).ok_or("equation denominator divisible by p")?;
            v = arith::add(p, v, arith::mul(p, av, x[*j]));
        }
        x[i] = v;
    }
    Ok(x)
}

// ------------------------------------------------------------------ applicability ------------------------------------

fn rule_point(
    rule: &RowsRule,
    key: &[i64],
) -> Result<Result<Vec<i64>, NotApplicableReason>, Unsupported> {
    let n = rule.arity;
    if key.len() != n {
        return Ok(Err(NotApplicableReason::Arity {
            expected: n,
            got: key.len(),
        }));
    }
    let mut pt: Vec<i64> = rule.fixed.iter().map(|f| f.unwrap_or(0)).collect();
    for (j, &(ax, off)) in rule.target.iter().enumerate() {
        match ax {
            Some(a) => pt[a] = key[j] - off,
            None if key[j] != off => {
                return Ok(Err(NotApplicableReason::TargetConstant { slot: j }));
            }
            None => {}
        }
    }
    for (j, f) in rule.fixed.iter().enumerate() {
        if let Some(v) = f
            && key[j] != *v
        {
            return Ok(Err(NotApplicableReason::FixedSlot { slot: j }));
        }
    }
    let to32: Vec<i32> = match pt
        .iter()
        .map(|&x| i32::try_from(x))
        .collect::<Result<Vec<_>, _>>()
    {
        Ok(v) => v,
        Err(_) => return Err(Unsupported::Record("rule point outside i32".into())),
    };
    let zero = |s: &str| -> Result<bool, Unsupported> {
        let p = Poly::parse(s, n, false).map_err(|e| Unsupported::Syntax(format!("{s}: {e}")))?;
        p.is_zero_at(&to32)
            .map_err(|e| Unsupported::Syntax(format!("{s}: {e}")))
    };
    for (i, e) in rule.equations.iter().enumerate() {
        if !zero(e)? {
            return Ok(Err(NotApplicableReason::Equation(i)));
        }
    }
    for (b, branch) in rule.exceptions.iter().enumerate() {
        let mut all = !branch.is_empty();
        for lit in branch {
            if !zero(lit)? {
                all = false;
                break;
            }
        }
        if all {
            return Ok(Err(NotApplicableReason::OnException { branch: b }));
        }
    }
    for (j, &line) in rule.chart.iter().enumerate() {
        if (line && key[j] < 1) || (!line && key[j] > 0) {
            return Ok(Err(NotApplicableReason::ChartMismatch { slot: j }));
        }
    }
    Ok(Ok(pt))
}

// ------------------------------------------------------------------ one cell -----------------------------------------

struct CellCtx<'a> {
    family: &'a Family,
    rule: &'a RowsRule,
    opts: &'a RowsOptions,
    sym: Option<&'a VerifiedGroup>,
}

struct System {
    rows: Vec<BTreeMap<ColKey, u64>>,
    /// per row: the symmetry group element that produced it (`None` = an IBP row)
    labels: Vec<Option<usize>>,
}

/// The symmetry rows at one seed column key, modulo p, deduplicated across the cell (`seen`).
fn add_sym_rows(
    cx: &CellCtx,
    seed: &ColKey,
    p: u64,
    out: &mut System,
    seen: &mut BTreeSet<Vec<(ColKey, (i128, i128))>>,
) -> Result<(), String> {
    let Some(g) = cx.sym else { return Ok(()) };
    for r in symrows::rows_at(g, seed, cx.opts.sym_mode, cx.opts.sym_max_terms)? {
        let canon: Vec<(ColKey, (i128, i128))> = r
            .terms
            .iter()
            .map(|(k, c)| (k.clone(), (c.n, c.d)))
            .collect();
        if !seen.insert(canon) {
            continue;
        }
        let mut row: BTreeMap<ColKey, u64> = BTreeMap::new();
        for (k, c) in r.terms {
            let v = c
                .modp(p)
                .ok_or("symmetry row coefficient denominator divisible by p")?;
            if v != 0 {
                row.insert(k, v);
            }
        }
        out.rows.push(row);
        out.labels.push(Some(r.element));
    }
    Ok(())
}

/// Rows at a generic point: seed pattern -> base residues and column offsets; family.rows_at_mod.
fn generic_rows(cx: &CellCtx, seeds: &[Pat], x: &[u64], d: u64, p: u64) -> Result<System, String> {
    let n = cx.rule.arity;
    let rc = &cx.rule.recenter;
    let mut seen: BTreeSet<ColKey> = BTreeSet::new();
    let mut seen_sym = BTreeSet::new();
    let mut out = System {
        rows: Vec::new(),
        labels: Vec::new(),
    };
    for s in seeds {
        let mut mask = 0u64;
        let mut offs = vec![0i64; n];
        let mut base = vec![0u64; n];
        for (i, &(ax, off)) in s.iter().enumerate() {
            // audit m6: a slot FIXED by the case is a constant at the generic point (its zero-sector status is
            // read from its integer value, not from the chart bit)
            let ax = ax.filter(|_| cx.rule.fixed[i].is_none());
            let off = match (s[i].0, cx.rule.fixed[i]) {
                (Some(_), Some(v)) => v + off + rc[i],
                _ => off,
            };
            match ax {
                Some(a) => {
                    mask |= 1 << i;
                    offs[i] = off + rc[i];
                    base[i] = arith::add(p, x[a], arith::from_i128(p, (off + rc[i]) as i128));
                }
                None => {
                    offs[i] = off;
                    base[i] = arith::from_i128(p, off as i128);
                }
            }
        }
        if !seen.insert((mask, offs.clone())) {
            continue;
        }
        for r in cx.family.rows_at_mod(&base, d, p)? {
            let mut row = BTreeMap::new();
            for (delta, v) in r {
                let k: Vec<i64> = offs.iter().zip(&delta).map(|(a, b)| a + b).collect();
                let e = row.entry((mask, k)).or_insert(0u64);
                *e = arith::add(p, *e, v);
            }
            row.retain(|_, v| *v != 0);
            out.rows.push(row);
            out.labels.push(None);
        }
        add_sym_rows(cx, &(mask, offs), p, &mut out, &mut seen_sym)?;
    }
    Ok(out)
}

/// Rows at an integer rule point through the COPIED `rows_for_p`.
fn integer_rows(cx: &CellCtx, seeds: &[Pat], pt: &[i64], d: u64, p: u64) -> Result<System, String> {
    let built = rows::rows_for_p(
        p,
        &Support::Exported(seeds.to_vec()),
        pt,
        &cx.rule.recenter,
        d,
        cx.family,
    )?;
    let mut out = System {
        labels: vec![None; built.rows.len()],
        rows: built
            .rows
            .into_iter()
            .map(|r| r.into_iter().map(|(k, v)| ((0u64, k), v)).collect())
            .collect(),
    };
    if cx.sym.is_some() {
        // the same seed keys as rows_for_p (pattern at the point + recenter on symbolic slots), in order, once each
        let mut seen_y: BTreeSet<Vec<i64>> = BTreeSet::new();
        let mut seen_sym = BTreeSet::new();
        for pat in seeds {
            let y: Vec<i64> = pat
                .iter()
                .enumerate()
                .map(|(i, (ax, off))| ax.map_or(*off, |a| pt[a] + off + cx.rule.recenter[i]))
                .collect();
            if seen_y.insert(y.clone()) {
                add_sym_rows(cx, &(0, y), p, &mut out, &mut seen_sym)?;
            }
        }
    }
    Ok(out)
}

fn pattern_col(pat: &Pat, generic: bool, pt: &[i64], fixed: &[Option<i64>]) -> ColKey {
    if generic {
        let mut mask = 0u64;
        let v = pat
            .iter()
            .enumerate()
            .map(
                |(i, &(ax, off))| match (ax, fixed.get(i).copied().flatten()) {
                    // audit m6: a fixed slot is a constant
                    (Some(_), Some(f)) => f + off,
                    (Some(_), None) => {
                        mask |= 1 << i;
                        off
                    }
                    (None, _) => off,
                },
            )
            .collect();
        (mask, v)
    } else {
        (0, record::key_at(pat, pt))
    }
}

struct SolveOut {
    status: CellStatus,
    rows: usize,
    row_columns: usize,
    universe_columns: usize,
    zero_columns: usize,
    rank: usize,
    in_span: bool,
    witness: Option<String>,
    mutation_rejected: Option<bool>,
    corners_tested: usize,
    corners_in_span: Vec<String>,
    entries: usize,
    bytes: usize,
    boundary_columns: usize,
    sym_rows: usize,
    sym_rows_weighted: usize,
    sym_support: Vec<usize>,
    corner_projected_in_span: Option<bool>,
}

/// Columns, zero sectors, elimination, span test and controls of one cell.
fn solve_system(
    cx: &CellCtx,
    sys: System,
    target: ColKey,
    rhs: Vec<ColKey>,
    values: &[u64],
    p: u64,
) -> SolveOut {
    let chart = &cx.rule.chart;
    let n = cx.rule.arity;
    let mut universe: BTreeSet<&ColKey> = BTreeSet::new();
    for r in &sys.rows {
        universe.extend(r.keys());
    }
    universe.insert(&target);
    universe.extend(rhs.iter());
    let mut zero_cache: HashMap<Vec<bool>, bool> = HashMap::new();
    let mut is_zero = |k: &ColKey| -> bool {
        let a = active(k, chart);
        if let Some(z) = zero_cache.get(&a) {
            return *z;
        }
        let z = cx.family.zero_mask(&a);
        zero_cache.insert(a, z);
        z
    };
    let mut nonzero: Vec<&ColKey> = Vec::new();
    let mut zero_columns = 0;
    let mut boundary_columns = 0;
    for k in &universe {
        if is_zero(k) {
            zero_columns += 1;
            // symbolic non-line slots at positive offset: active on the face where n_i + off >= 1
            if (0..n).any(|i| k.0 >> i & 1 == 1 && !chart[i] && k.1[i] > 0) {
                let mut a = active(k, chart);
                for (i, ai) in a.iter_mut().enumerate() {
                    if k.0 >> i & 1 == 1 && !chart[i] && k.1[i] > 0 {
                        *ai = true;
                    }
                }
                if !cx.family.zero_mask(&a) {
                    boundary_columns += 1;
                }
            }
        } else {
            nonzero.push(k);
        }
    }
    let universe_columns = universe.len();
    let target_zero = is_zero(&target);
    // pivot order: most complex first (index 0); ties by key
    nonzero.sort_by(|a, b| {
        col_weight(b, chart)
            .cmp(&col_weight(a, chart))
            .then_with(|| a.cmp(b))
    });
    let ids: HashMap<&ColKey, usize> = nonzero.iter().enumerate().map(|(i, k)| (*k, i)).collect();
    let enc = |m: &BTreeMap<ColKey, u64>| -> Vec<(usize, u64)> {
        let mut es: Vec<(usize, u64)> = m
            .iter()
            .filter(|(_, v)| **v % p != 0)
            .filter_map(|(k, v)| ids.get(k).map(|&i| (i, *v)))
            .collect();
        es.sort_unstable();
        es
    };
    let enc_rows: Vec<Vec<(usize, u64)>> = sys.rows.iter().map(enc).collect();
    let mut used = vec![false; nonzero.len()];
    let mut row_entries = 0usize;
    for r in &enc_rows {
        row_entries += r.len();
        for &(c, _) in r {
            used[c] = true;
        }
    }
    let row_columns = used.iter().filter(|&&u| u).count();
    let mut desired: BTreeMap<ColKey, u64> = BTreeMap::new();
    desired.insert(target.clone(), 1);
    for (k, v) in rhs.iter().zip(values) {
        let e = desired.entry(k.clone()).or_insert(0);
        *e = arith::sub(p, *e, *v);
    }
    let enc_desired = enc(&desired);
    let bytes_cols = nonzero.len() * (n * 8 + 64);
    let mut out = SolveOut {
        status: CellStatus::Undecided {
            reason: String::new(),
        },
        rows: sys.rows.len(),
        row_columns,
        universe_columns,
        zero_columns,
        rank: 0,
        in_span: false,
        witness: None,
        mutation_rejected: None,
        corners_tested: 0,
        corners_in_span: Vec::new(),
        entries: 0,
        bytes: row_entries * 16 + bytes_cols,
        boundary_columns,
        sym_rows: sys.labels.iter().filter(|l| l.is_some()).count(),
        sym_rows_weighted: 0,
        sym_support: Vec::new(),
        corner_projected_in_span: None,
    };
    if target_zero {
        out.status = CellStatus::Refused(Unsupported::TargetInZeroSector);
        return out;
    }
    let elim = match Elim::new_guarded(&enc_rows, nonzero.len(), p, cx.opts.max_entries) {
        Ok(e) => e,
        Err(e) if e.starts_with(MEMORY_GUARD) => {
            out.status = CellStatus::MemoryGuard {
                entries: cx.opts.max_entries,
            };
            out.entries = cx.opts.max_entries;
            return out;
        }
        Err(e) => {
            out.status = CellStatus::Undecided { reason: e };
            return out;
        }
    };
    out.entries = elim.entries();
    out.bytes += out.entries * 16;
    out.rank = elim.rank();
    let sp = match elim.solve(&enc_desired) {
        Ok(s) => s,
        Err(e) => {
            out.status = CellStatus::Undecided { reason: e };
            return out;
        }
    };
    out.in_span = sp.in_span;
    // the certificate's symmetry support: elements whose rows carry nonzero weight
    let mut support: BTreeSet<usize> = BTreeSet::new();
    for (i, w) in &sp.weights {
        if *w != 0
            && let Some(Some(e)) = sys.labels.get(*i)
        {
            out.sym_rows_weighted += 1;
            support.insert(*e);
        }
    }
    out.sym_support = support.into_iter().collect();
    if !sp.in_span {
        out.witness = sp.remainder.first().map(|&(c, _)| col_text(nonzero[c]));
        out.status = CellStatus::Fail;
        return out;
    }
    if !cx.opts.controls {
        out.status = CellStatus::Pass;
        return out;
    }
    // control 1 (engine Y24): the +1 mutant of the first nonzero non-scaleless RHS coefficient must NOT be in the span
    let mutable = rhs
        .iter()
        .zip(values)
        .position(|(k, v)| *v != 0 && ids.contains_key(k));
    let Some(mi) = mutable else {
        out.status = CellStatus::Undecided {
            reason: "no nonzero non-scaleless RHS coefficient for the +1 mutation control".into(),
        };
        return out;
    };
    let mut mutant = desired.clone();
    let e = mutant.entry(rhs[mi].clone()).or_insert(0);
    *e = arith::sub(p, *e, 1);
    let rejected = match elim.solve(&enc(&mutant)) {
        Ok(s) => !s.in_span,
        Err(e) => {
            out.status = CellStatus::Undecided { reason: e };
            return out;
        }
    };
    out.mutation_rejected = Some(rejected);
    // control 2 (engine Y24): per sector touched, its lowest numerator-free column is not in the span
    let mut probe: BTreeMap<Vec<bool>, (i64, usize)> = BTreeMap::new();
    for (c, k) in nonzero.iter().enumerate() {
        let corner = k.1.iter().enumerate().all(|(i, &v)| {
            if k.0 >> i & 1 == 1 {
                chart[i]
            } else {
                v >= 1 || v == 0
            }
        });
        if !corner {
            continue;
        }
        let weight: i64 = k.1.iter().sum();
        let e = probe.entry(active(k, chart)).or_insert((weight, c));
        if weight < e.0 {
            *e = (weight, c);
        }
    }
    for (_, c) in probe.values() {
        out.corners_tested += 1;
        match elim.solve(&[(*c, 1)]) {
            Ok(s) if s.in_span => out.corners_in_span.push(col_text(nonzero[*c])),
            Ok(_) => {}
            Err(e) => {
                out.status = CellStatus::Undecided { reason: e };
                return out;
            }
        }
    }
    // owed control with symmetry rows (engine replay/mod.rs:1653-1686 at 385df34c): the chart corner must not be in
    // the span, neither exactly nor with the rows projected onto the chart's top-sector columns
    if out.sym_rows > 0 {
        let corner: ColKey = (0, chart.iter().map(|&l| i64::from(l)).collect());
        let mut in_proj = false;
        if let Some(&cc) = ids.get(&corner) {
            let exact = match elim.solve(&[(cc, 1)]) {
                Ok(s) => s.in_span,
                Err(e) => {
                    out.status = CellStatus::Undecided { reason: e };
                    return out;
                }
            };
            let top = |k: &ColKey| (0..n).all(|i| !chart[i] || (k.0 >> i & 1 == 1) || k.1[i] >= 1);
            let proj: Vec<Vec<(usize, u64)>> = enc_rows
                .iter()
                .map(|r| {
                    r.iter()
                        .copied()
                        .filter(|(c, _)| top(nonzero[*c]))
                        .collect()
                })
                .collect();
            match Elim::new_guarded(&proj, nonzero.len(), p, cx.opts.max_entries)
                .and_then(|e2| e2.solve(&[(cc, 1)]))
            {
                Ok(s) => in_proj = exact || s.in_span,
                Err(e) if e.starts_with(MEMORY_GUARD) => {
                    out.status = CellStatus::MemoryGuard {
                        entries: cx.opts.max_entries,
                    };
                    return out;
                }
                Err(e) => {
                    out.status = CellStatus::Undecided { reason: e };
                    return out;
                }
            }
        }
        out.corner_projected_in_span = Some(in_proj);
        if in_proj {
            out.status = CellStatus::Undecided {
                reason: "control failed: the chart corner is in the span of IBP + symmetry rows (exactly or modulo subsectors)".into(),
            };
            return out;
        }
    }
    out.status = if !rejected {
        CellStatus::Undecided {
            reason: "control failed: the +1 mutant is also in the span".into(),
        }
    } else if !out.corners_in_span.is_empty() {
        CellStatus::Undecided {
            reason: "control failed: a nonzero corner integral is in the span".into(),
        }
    } else {
        CellStatus::Pass
    };
    out
}

fn empty_cell(p: u64, d: u64, status: CellStatus) -> CellReport {
    CellReport {
        prime: p,
        d,
        generic_values: None,
        redraws: 0,
        status,
        rows: 0,
        row_columns: 0,
        universe_columns: 0,
        zero_columns: 0,
        rank: 0,
        in_span: false,
        witness: None,
        mutation_rejected: None,
        corners_tested: 0,
        corners_in_span: Vec::new(),
        entries: 0,
        sym_rows: 0,
        sym_rows_weighted: 0,
        sym_support: Vec::new(),
        corner_projected_in_span: None,
        boundary_columns: 0,
        sz_degree: 0,
        seconds: 0.0,
    }
}

fn fill_cell(c: &mut CellReport, s: SolveOut) -> usize {
    c.status = s.status;
    c.rows = s.rows;
    c.row_columns = s.row_columns;
    c.universe_columns = s.universe_columns;
    c.zero_columns = s.zero_columns;
    c.rank = s.rank;
    c.in_span = s.in_span;
    c.witness = s.witness;
    c.mutation_rejected = s.mutation_rejected;
    c.corners_tested = s.corners_tested;
    c.corners_in_span = s.corners_in_span;
    c.entries = s.entries;
    c.boundary_columns = s.boundary_columns;
    c.sym_rows = s.sym_rows;
    c.sym_rows_weighted = s.sym_rows_weighted;
    c.sym_support = s.sym_support;
    c.corner_projected_in_span = s.corner_projected_in_span;
    s.bytes
}

struct CellRun {
    cell: CellReport,
    deg: DegStats,
    bytes: usize,
}

#[allow(clippy::too_many_arguments)]
fn integer_cell(
    cx: &CellCtx,
    seeds: &[Pat],
    pt: &[i64],
    p: u64,
    stream: &[u64],
) -> Result<CellRun, Unsupported> {
    let t0 = Instant::now();
    let rule = cx.rule;
    let mut s = SplitMix64::derive(cx.opts.seed, stream);
    let base: Vec<u64> = pt.iter().map(|&x| arith::from_i128(p, x as i128)).collect();
    let mut redraws = 0;
    let mut deg = DegStats::default();
    let (d, values) = loop {
        if redraws >= cx.opts.d_attempts {
            let mut c = empty_cell(
                p,
                0,
                CellStatus::Undecided {
                    reason: format!("{redraws} d draws hit a zero denominator"),
                },
            );
            c.redraws = redraws;
            return Ok(CellRun {
                cell: c,
                deg,
                bytes: 0,
            });
        }
        let d = draw_d(&mut s, p);
        let mut vals = base.clone();
        vals.push(d);
        let ev = eval_coeffs(rule, &[Ctx { p, vals }]).map_err(Unsupported::Syntax)?;
        deg = ev.deg;
        match ev.values.into_iter().next().unwrap() {
            Ok(v) => break (d, v),
            Err(k) => {
                // identically zero in d at this key? (deg_d + 1 further values)
                let degd = den_degd(rule, k, p).map_err(Unsupported::Syntax)?;
                let mut ps =
                    SplitMix64::derive(cx.opts.seed, &[stream, &[PURPOSE_POLE, k as u64]].concat());
                let mut ds: BTreeSet<u64> = BTreeSet::new();
                while ds.len() < degd as usize + 1 {
                    ds.insert(draw_d(&mut ps, p));
                }
                let ctxs: Vec<Ctx> = ds
                    .iter()
                    .map(|&dd| {
                        let mut v = base.clone();
                        v.push(dd);
                        Ctx { p, vals: v }
                    })
                    .collect();
                let (mut nn, mut dn) = (vec![0; ctxs.len()], vec![0; ctxs.len()]);
                coeff::eval_ratio(&rule.rhs[k].1, rule.arity, &ctxs, &mut nn, &mut dn)
                    .map_err(|e| Unsupported::Syntax(e.to_string()))?;
                if dn.iter().all(|&x| x == 0) {
                    let mut c = empty_cell(p, d, CellStatus::Pole { coefficient: k });
                    c.redraws = redraws;
                    c.seconds = t0.elapsed().as_secs_f64();
                    return Ok(CellRun {
                        cell: c,
                        deg,
                        bytes: 0,
                    });
                }
                redraws += 1;
            }
        }
    };
    let target = pattern_col(&rule.target, false, pt, &rule.fixed);
    let rhs: Vec<ColKey> = rule
        .rhs
        .iter()
        .map(|(k, _)| pattern_col(k, false, pt, &rule.fixed))
        .collect();
    let mut c = empty_cell(p, d, CellStatus::Pass);
    c.redraws = redraws;
    let bytes = match integer_rows(cx, seeds, pt, d, p) {
        Ok(sys) => fill_cell(&mut c, solve_system(cx, sys, target, rhs, &values, p)),
        Err(e) => {
            c.status = CellStatus::Undecided { reason: e };
            0
        }
    };
    // audit m1: bound the GENERIC rank by min(rows, columns), not the rank seen at this point
    c.sz_degree = c.rows.min(c.row_columns) as u64 + deg.max_num_degd + deg.sum_den_degd;
    c.seconds = t0.elapsed().as_secs_f64();
    Ok(CellRun {
        cell: c,
        deg,
        bytes,
    })
}

fn generic_cell(
    cx: &CellCtx,
    seeds: &[Pat],
    solved: &Solved,
    p: u64,
    stream: &[u64],
) -> Result<CellRun, Unsupported> {
    let t0 = Instant::now();
    let rule = cx.rule;
    let mut s = SplitMix64::derive(cx.opts.seed, stream);
    let mut redraws = 0;
    let mut deg = DegStats::default();
    let mut last_k = 0;
    let (x, d, values) = loop {
        if redraws >= cx.opts.d_attempts {
            let mut c = empty_cell(
                p,
                0,
                CellStatus::Pole {
                    coefficient: last_k,
                },
            );
            c.redraws = redraws;
            return Ok(CellRun {
                cell: c,
                deg,
                bytes: 0,
            });
        }
        let x = generic_values(rule, solved, p, &mut s).map_err(Unsupported::Record)?;
        let d = draw_d(&mut s, p);
        let mut vals = x.clone();
        vals.push(d);
        let ev = eval_coeffs(rule, &[Ctx { p, vals }]).map_err(Unsupported::Syntax)?;
        deg = ev.deg;
        match ev.values.into_iter().next().unwrap() {
            Ok(v) => break (x, d, v),
            Err(k) => {
                last_k = k;
                redraws += 1;
            }
        }
    };
    let target = pattern_col(&rule.target, true, &[], &rule.fixed);
    let rhs: Vec<ColKey> = rule
        .rhs
        .iter()
        .map(|(k, _)| pattern_col(k, true, &[], &rule.fixed))
        .collect();
    let mut c = empty_cell(p, d, CellStatus::Pass);
    c.redraws = redraws;
    c.generic_values = Some(x.clone());
    let bytes = match generic_rows(cx, seeds, &x, d, p) {
        Ok(sys) => fill_cell(&mut c, solve_system(cx, sys, target, rhs, &values, p)),
        Err(e) => {
            c.status = CellStatus::Undecided { reason: e };
            0
        }
    };
    c.sz_degree = c.rows.min(c.row_columns) as u64 + deg.max_num_deg + deg.sum_den_deg;
    c.seconds = t0.elapsed().as_secs_f64();
    Ok(CellRun {
        cell: c,
        deg,
        bytes,
    })
}

/// Seeds plus every seed shifted by +-1 in one slot (the checker's `shell`; a diagnostic, never an acceptance).
fn shell(seeds: &[Pat]) -> Vec<Pat> {
    let mut out: BTreeSet<Pat> = seeds.iter().cloned().collect();
    for s in seeds {
        for i in 0..s.len() {
            for e in [1i64, -1] {
                let mut t = s.clone();
                t[i].1 += e;
                out.insert(t);
            }
        }
    }
    out.into_iter().collect()
}

fn combine(cells: &[CellReport]) -> PointStatus {
    if let Some(u) = cells.iter().find_map(|c| match &c.status {
        CellStatus::Refused(u) => Some(u.clone()),
        _ => None,
    }) {
        return PointStatus::Unsupported(u);
    }
    if let Some(e) = cells.iter().find_map(|c| match c.status {
        CellStatus::MemoryGuard { entries } => Some(entries),
        _ => None,
    }) {
        return PointStatus::MemoryGuard { entries: e };
    }
    let poles: Vec<usize> = cells
        .iter()
        .filter_map(|c| match c.status {
            CellStatus::Pole { coefficient } => Some(coefficient),
            _ => None,
        })
        .collect();
    if !poles.is_empty() {
        return if poles.len() == cells.len() {
            PointStatus::Pole {
                coefficient: poles[0],
            }
        } else {
            PointStatus::Undecided {
                reason: "pole at one prime only".into(),
            }
        };
    }
    if let Some(r) = cells.iter().find_map(|c| match &c.status {
        CellStatus::Undecided { reason } => Some(reason.clone()),
        _ => None,
    }) {
        return PointStatus::Undecided { reason: r };
    }
    let fails = cells
        .iter()
        .filter(|c| c.status == CellStatus::Fail)
        .count();
    if fails == 0 {
        PointStatus::Pass
    } else if fails == cells.len() {
        PointStatus::Fail
    } else {
        PointStatus::Undecided {
            reason: "mixed: in the span at one prime, not at the other".into(),
        }
    }
}

fn unsupported_verdict(
    family: &Family,
    rule: &RowsRule,
    rule_id: u64,
    opts: &RowsOptions,
    primes: [u64; 2],
    u: Unsupported,
) -> RowsVerdict {
    RowsVerdict {
        version: ROWS_VERSION,
        family_digest: family.source_digest.clone(),
        family_name: family.name.clone(),
        family_bound_to_table: false,
        verdict: Verdict::Unsupported(u),
        rule_index: rule.index,
        target: rule.target_text.clone(),
        rule_id,
        seed: opts.seed,
        master_seed: opts.seed,
        nonce: 0,
        primes,
        points: Vec::new(),
        points_passed: 0,
        sz_bound: 1.0,
        sz_bound_log10: 0.0,
        sz_bound_per_cell_max: 1.0,
        meets_a2_policy: false,
        symmetry: None,
        cost: Cost {
            coefficient_bytes: rule.coefficient_bytes(),
            ..Cost::default()
        },
    }
}

/// The random stream seed of a rule (audit B M1): sha256(master seed, rule digest), first 8 bytes. Primes, d values
/// and generic points all derive from it, so a coefficient tailored to the recorded draws changes the draws.
/// The stream seed binds the master seed, the rule digest, the family digest, the digest of the points / served keys
/// and a nonce (audit B M1: reproducible from the record, not predictable before the rule exists when the nonce is
/// drawn at run time, which `replay_evidence` does unless `RowsOptions.nonce` is given).
pub fn stream_seed(
    master: u64,
    rule: &RowsRule,
    family_digest: &str,
    keys_digest: &str,
    nonce: u64,
) -> u64 {
    let h = crate::replay_base::sha256::sha256_hex(
        format!(
            "{master}\x00{}\x00{family_digest}\x00{keys_digest}\x00{nonce}",
            rule_digest(rule)
        )
        .as_bytes(),
    );
    u64::from_str_radix(&h[..16], 16).unwrap_or(master)
}

/// A run-time nonce from the OS (recorded in the evidence record).
pub fn os_nonce() -> u64 {
    use std::io::Read;
    let mut b = [0u8; 8];
    if std::fs::File::open("/dev/urandom")
        .and_then(|mut f| f.read_exact(&mut b))
        .is_ok()
    {
        return u64::from_le_bytes(b);
    }
    let t = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_nanos() as u64);
    t ^ (u64::from(std::process::id()) << 32)
}

fn points_digest(points: &[PointSpec]) -> String {
    let keys: Vec<Vec<i64>> = points
        .iter()
        .filter_map(|p| match p {
            PointSpec::Key(k) => Some(k.clone()),
            PointSpec::Generic => None,
        })
        .collect();
    let generic = points.len() - keys.len();
    format!("{}g{generic}", key_set_digest(&keys))
}

/// Estimated heap of the rows of `seeds` distinct seed keys (audit B M3: guard BEFORE building the rows).
fn rows_bytes_estimate(family: &Family, seeds: usize) -> usize {
    seeds
        .saturating_mul(family.n_ops())
        .saturating_mul(2 * family.n + 2)
        .saturating_mul(family.n * 8 + 64)
}

/// THE A2 test of one rule at the given points (each at both primes). See the module doc and interfaces/ibp-rows.md.
pub fn identity_test(
    family: &Family,
    rule: &RowsRule,
    points: &[PointSpec],
    opts: &RowsOptions,
) -> RowsVerdict {
    let nonce = opts.nonce.unwrap_or(0);
    let derived_opts = RowsOptions {
        seed: stream_seed(
            opts.seed,
            rule,
            &family.source_digest,
            &points_digest(points),
            nonce,
        ),
        ..opts.clone()
    };
    let mut v = identity_test_derived(family, rule, points, &derived_opts);
    v.master_seed = opts.seed;
    v.nonce = nonce;
    v
}

fn identity_test_derived(
    family: &Family,
    rule: &RowsRule,
    points: &[PointSpec],
    opts: &RowsOptions,
) -> RowsVerdict {
    let t0 = Instant::now();
    let rule_id = opts.rule_id.unwrap_or_else(|| rule.default_id());
    let primes = match opts.primes {
        Some(pp) => pp,
        None => primes::verify_primes(opts.seed, &opts.lift_primes),
    };
    if let Some(e) = primes
        .iter()
        .find_map(|&p| primes::check_verify_prime(p, &opts.lift_primes).err())
        .or_else(|| (primes[0] == primes[1]).then(|| "the two primes are equal".to_string()))
    {
        return unsupported_verdict(
            family,
            rule,
            rule_id,
            opts,
            primes,
            Unsupported::BadPrime(e),
        );
    }
    if family.n != rule.arity {
        let u = Unsupported::Record(format!(
            "family has {} slots, rule {}",
            family.n, rule.arity
        ));
        return unsupported_verdict(family, rule, rule_id, opts, primes, u);
    }
    // audit C2: the family must be inside what the replay models and match the rule's table header
    if let Err(e) = &family.check {
        return unsupported_verdict(
            family,
            rule,
            rule_id,
            opts,
            primes,
            Unsupported::Family(e.clone()),
        );
    }
    if let Some(t) = &opts.table
        && let Err(e) = family_header_check(&t.header, family)
    {
        return unsupported_verdict(family, rule, rule_id, opts, primes, Unsupported::Family(e));
    }
    // audit m3: an empty exception branch (the engine refuses it)
    if let Some(b) = rule.exceptions.iter().position(Vec::is_empty) {
        let u = Unsupported::EmptyExceptionBranch { branch: b };
        return unsupported_verdict(family, rule, rule_id, opts, primes, u);
    }
    // symmetry rows (section 10): only where the engine would add them (declared), or as the control / diagnostic
    let declared = rule.sym_rows.is_some() || opts.generated_with_symmetry_rows;
    let use_sym = match opts.sym_use {
        SymUse::Declared => declared,
        SymUse::Without => false,
        SymUse::Always => true,
    };
    let sym: Option<&VerifiedGroup> = if use_sym {
        match opts.symmetry.as_deref() {
            None => {
                let u = Unsupported::NeedsSymmetryRows {
                    declared: rule.sym_rows.clone(),
                };
                return unsupported_verdict(family, rule, rule_id, opts, primes, u);
            }
            Some(g) => {
                let why = if g.group.arity != rule.arity || g.group.chart != rule.chart {
                    Some(format!(
                        "group chart {} vs rule chart {}",
                        g.group.chart_text(),
                        rule.chart_text
                    ))
                } else if g.family_digest != family.source_digest {
                    Some("group verified on another family".to_string())
                } else {
                    None
                };
                if let Some(w) = why {
                    return unsupported_verdict(
                        family,
                        rule,
                        rule_id,
                        opts,
                        primes,
                        Unsupported::SymGroupMismatch(w),
                    );
                }
                Some(g)
            }
        }
    } else {
        None
    };
    if let Some(e) = &rule.syntax_error {
        return unsupported_verdict(
            family,
            rule,
            rule_id,
            opts,
            primes,
            Unsupported::Syntax(e.clone()),
        );
    }
    if rule.seeds.is_empty() {
        return unsupported_verdict(family, rule, rule_id, opts, primes, Unsupported::NoSeeds);
    }
    if rule.arity > 64 {
        let u = Unsupported::Record(format!("arity {} > 64 (column masks are u64)", rule.arity));
        return unsupported_verdict(family, rule, rule_id, opts, primes, u);
    }
    if let Err(e) = check_recenter_authority(rule) {
        return unsupported_verdict(family, rule, rule_id, opts, primes, Unsupported::Record(e));
    }
    let distinct_seeds = rule.seeds.iter().collect::<BTreeSet<_>>().len();
    let est = rows_bytes_estimate(family, distinct_seeds);
    if est > opts.max_row_bytes {
        let mut v = unsupported_verdict(family, rule, rule_id, opts, primes, Unsupported::NoSeeds);
        v.verdict = Verdict::MemoryGuard { entries: est };
        return v;
    }
    let cx = CellCtx {
        family,
        rule,
        opts,
        sym,
    };
    let mut cost = Cost {
        coefficient_bytes: rule.coefficient_bytes(),
        ..Cost::default()
    };
    let mut reports: Vec<PointReport> = Vec::new();
    let solved_res: Option<Result<Result<Solved, NotApplicableReason>, Unsupported>> = points
        .contains(&PointSpec::Generic)
        .then(|| solve_equations(rule));
    let mut unsupported: Option<Unsupported> = None;
    let run = |pi: usize,
               seeds: &[Pat],
               purpose: u64,
               pt: Option<&Vec<i64>>|
     -> Result<Vec<CellRun>, Unsupported> {
        let mut runs = Vec::new();
        for (qi, &p) in primes.iter().enumerate() {
            let stream = [rule_id, pi as u64, qi as u64, purpose];
            runs.push(match pt {
                Some(pt) => integer_cell(&cx, seeds, pt, p, &stream)?,
                None => match &solved_res {
                    Some(Ok(Ok(solved))) => generic_cell(&cx, seeds, solved, p, &stream)?,
                    _ => {
                        return Err(Unsupported::Record(
                            "generic point without solved case equations".into(),
                        ));
                    }
                },
            });
        }
        Ok(runs)
    };
    for (pi, spec) in points.iter().enumerate() {
        let mut rep = PointReport {
            spec: spec.clone(),
            rule_point: None,
            status: PointStatus::Pass,
            cells: Vec::new(),
            shell: Vec::new(),
        };
        // applicability / equations
        let early = match spec {
            PointSpec::Key(k) => match rule_point(rule, k) {
                Err(u) => Some(PointStatus::Unsupported(u)),
                Ok(Err(r)) => Some(PointStatus::NotApplicable(r)),
                Ok(Ok(pt)) => {
                    rep.rule_point = Some(pt);
                    None
                }
            },
            PointSpec::Generic => match &solved_res {
                Some(Err(u)) => Some(PointStatus::Unsupported(u.clone())),
                Some(Ok(Err(r))) => Some(PointStatus::NotApplicable(r.clone())),
                _ => None,
            },
        };
        if let Some(st) = early {
            if let PointStatus::Unsupported(u) = &st {
                unsupported.get_or_insert(u.clone());
            }
            rep.status = st;
            reports.push(rep);
            continue;
        }
        let runs = match run(pi, &rule.seeds, PURPOSE_D, rep.rule_point.as_ref()) {
            Ok(r) => r,
            Err(u) => {
                unsupported.get_or_insert(u.clone());
                rep.status = PointStatus::Unsupported(u);
                reports.push(rep);
                continue;
            }
        };
        for r in runs {
            cost.monomials = cost.monomials.max(r.deg.monomials);
            cost.peak_bytes_estimate = cost.peak_bytes_estimate.max(r.bytes);
            rep.cells.push(r.cell);
        }
        rep.status = combine(&rep.cells);
        if rep.status == PointStatus::Fail && opts.shell_diagnostic {
            let sh = shell(&rule.seeds);
            if let Ok(runs) = run(pi, &sh, PURPOSE_SHELL, rep.rule_point.as_ref()) {
                for r in runs {
                    cost.peak_bytes_estimate = cost.peak_bytes_estimate.max(r.bytes);
                    rep.shell.push(r.cell);
                }
                if combine(&rep.shell) == PointStatus::Pass {
                    rep.status = PointStatus::SeedsInsufficient;
                }
            }
        }
        reports.push(rep);
    }
    // aggregate
    for c in reports
        .iter()
        .flat_map(|r| r.cells.iter().chain(r.shell.iter()))
    {
        cost.rows += c.rows;
        cost.row_columns_max = cost.row_columns_max.max(c.row_columns);
        cost.rank_max = cost.rank_max.max(c.rank);
        cost.entries_peak = cost.entries_peak.max(c.entries);
    }
    // a point refused by a cell (e.g. target in a zero sector) is Unsupported like an early refusal
    if unsupported.is_none() {
        unsupported = reports.iter().find_map(|r| match &r.status {
            PointStatus::Unsupported(u) => Some(u.clone()),
            _ => None,
        });
    }
    let first = |f: &dyn Fn(&PointStatus) -> bool| reports.iter().position(|r| f(&r.status));
    // audit m2: a Fail (a stored disproof) is reported before an Unsupported point elsewhere
    let verdict = if let Some(i) = first(&|s| *s == PointStatus::Fail) {
        let c = reports[i]
            .cells
            .iter()
            .find(|c| c.status == CellStatus::Fail)
            .expect("failed cell");
        Verdict::Fail {
            point: i,
            prime: c.prime,
            d: c.d,
            column: c.witness.clone().unwrap_or_default(),
        }
    } else if let Some(u) = unsupported {
        Verdict::Unsupported(u)
    } else if let Some(i) = first(&|s| *s == PointStatus::SeedsInsufficient) {
        Verdict::SeedsInsufficient { point: i }
    } else if let Some(i) = first(&|s| matches!(s, PointStatus::MemoryGuard { .. })) {
        match reports[i].status {
            PointStatus::MemoryGuard { entries } => Verdict::MemoryGuard { entries },
            _ => unreachable!(),
        }
    } else if let Some(i) = first(&|s| matches!(s, PointStatus::Pole { .. })) {
        match reports[i].status {
            PointStatus::Pole { coefficient } => Verdict::Pole {
                point: i,
                coefficient,
            },
            _ => unreachable!(),
        }
    } else if let Some(i) = first(&|s| matches!(s, PointStatus::Undecided { .. })) {
        match &reports[i].status {
            PointStatus::Undecided { reason } => Verdict::Undecided {
                reason: format!("point {i}: {reason}"),
            },
            _ => unreachable!(),
        }
    } else if reports.iter().any(|r| r.status == PointStatus::Pass) {
        Verdict::Pass
    } else {
        Verdict::NotApplicable {
            reasons: reports
                .iter()
                .filter_map(|r| match &r.status {
                    PointStatus::NotApplicable(x) => Some(x.clone()),
                    _ => None,
                })
                .collect(),
        }
    };
    let points_passed = reports
        .iter()
        .filter(|r| r.status == PointStatus::Pass)
        .count();
    let (mut sz, mut szlog, mut szmax) = (1.0f64, 0.0f64, 0.0f64);
    if verdict == Verdict::Pass {
        for c in reports
            .iter()
            .filter(|r| r.status == PointStatus::Pass)
            .flat_map(|r| &r.cells)
        {
            let b = c.sz_degree as f64 / (c.prime - 2) as f64;
            sz *= b;
            szlog += b.log10();
            szmax = szmax.max(b);
        }
    } else {
        szmax = 1.0;
    }
    cost.seconds = t0.elapsed().as_secs_f64();
    let symmetry = sym.map(|g| {
        let mut support: BTreeSet<usize> = BTreeSet::new();
        let mut rows_total = 0;
        for c in reports.iter().flat_map(|r| r.cells.iter()) {
            rows_total += c.sym_rows;
            support.extend(c.sym_support.iter().copied());
        }
        SymmetryUse {
            group_digest: g.digest.clone(),
            provenance: g.provenance.clone(),
            mode: opts.sym_mode,
            elements: g.group.elements.len(),
            rows_total,
            support: support.into_iter().collect(),
        }
    });
    RowsVerdict {
        symmetry,
        version: ROWS_VERSION,
        family_digest: family.source_digest.clone(),
        family_name: family.name.clone(),
        family_bound_to_table: opts.table.is_some(),
        // a pass resting only on generic points with boundary-sensitive columns does not meet A2 (audit A C1 / B)
        meets_a2_policy: verdict == Verdict::Pass
            && points_passed >= 2
            && (reports
                .iter()
                .all(|r| r.cells.iter().all(|c| c.boundary_columns == 0))
                || reports
                    .iter()
                    .any(|r| matches!(r.spec, PointSpec::Key(_)) && r.status == PointStatus::Pass)),
        verdict,
        rule_index: rule.index,
        target: rule.target_text.clone(),
        rule_id,
        seed: opts.seed,
        master_seed: opts.seed,
        nonce: 0,
        primes,
        points: reports,
        points_passed,
        sz_bound: sz,
        sz_bound_log10: szlog,
        sz_bound_per_cell_max: szmax,
        cost,
    }
}

/// The engine replay's recenter authority (`recenter_for`, raw-pivot branch): when the record carries the node
/// driver's raw pivot, the recenter it implies (target offset minus raw offset on symbolic slots) must equal the
/// record's `recenter`; a disagreement refuses the rule (the seeds would be placed differently).
pub fn check_recenter_authority(rule: &RowsRule) -> Result<(), String> {
    let Some(raw) = &rule.raw_pivot else {
        return Ok(());
    };
    for (j, (&b, &(ax, toff))) in raw.iter().zip(&rule.target).enumerate() {
        let (sym, off) = record::power_decode(b)?;
        if sym != ax.is_some() {
            return Err(format!(
                "raw pivot symbolic pattern disagrees with target at slot {j}"
            ));
        }
        let derived = if sym {
            toff - off
        } else if off != toff {
            return Err(format!(
                "raw pivot numeric coordinate disagrees with target at slot {j}"
            ));
        } else {
            0
        };
        if derived != rule.recenter[j] {
            return Err(format!("raw pivot and recenter disagree at slot {j}"));
        }
    }
    Ok(())
}

// ------------------------------------------------------------------ evidence level `replay` -----------------------

/// Schema of the [`ReplayRecord`] (the artefact the ledger cites for evidence level `replay`).
pub const REPLAY_SCHEMA: &str = "rustred.campaign.replay_evidence.v2";

/// Which points establish `replay` (interfaces/ibp-rows.md section 9, corrected after audit A).
#[derive(Clone, Debug, Serialize)]
pub struct ReplayPolicy {
    /// random points on the symbolic slots (skipped for a rule without symbolic slots)
    pub generic_points: usize,
    /// served keys at which the identity is replayed: 0 = EVERY served key (the `replay` level); any other value
    /// replays a deterministic sample and can only give the level `ReplaySampled`, which the campaign never accepts
    /// (audit C1: a pass at sampled keys says nothing at the others, e.g. on boundary faces)
    pub max_identity_keys: usize,
    /// window of the search for an integer point on a witness locus (excess 0..=w per free slot)
    pub loci_window: i64,
    /// cap of that search (beyond it: a deterministic random sample of this many candidates)
    pub loci_max_candidates: usize,
    /// where the served key set comes from (a file path or ledger id), recorded with its digest (audit M4)
    pub served_keys_source: Option<String>,
}

impl Default for ReplayPolicy {
    fn default() -> Self {
        ReplayPolicy {
            generic_points: 2,
            max_identity_keys: 0,
            loci_window: 4,
            loci_max_candidates: 200_000,
            served_keys_source: None,
        }
    }
}

#[derive(Clone, Debug, Serialize)]
pub struct PoleHit {
    pub key: Vec<i64>,
    pub coefficient: usize,
}

/// Pole check WITHOUT factoring (R38): every RHS denominator evaluated at every served key, modulo both primes, at a
/// random d; a zero is re-evaluated at deg_d + 1 further d values: identically zero in d at both primes = a pole.
#[derive(Clone, Debug, Default, Serialize)]
pub struct PoleScan {
    pub keys_given: usize,
    pub keys_applicable: usize,
    pub keys_not_applicable: usize,
    /// the served keys that are NOT applicable (outside the case / chart / on an exception), with the reason
    pub not_applicable: Vec<(Vec<i64>, NotApplicableReason)>,
    pub poles: Vec<PoleHit>,
    /// identically zero at one prime only, or unreadable
    pub undecided: Vec<(Vec<i64>, String)>,
}

#[derive(Clone, Debug, Serialize)]
pub enum LocusStatus {
    /// the locus is one of the rule's single-literal exception branches (guarded by construction)
    Guarded,
    /// the locus contains d: its index part is carried by the exceptions (not sampled, as the engine replay)
    Parameter,
    /// proved free of integer points in the case (affine locus, gcd of the coefficients does not divide the constant)
    NoIntegerPoint,
    /// no applicable integer point found in the search window: UNTESTED, blocks the level (audit M3)
    NoPoint {
        candidates: usize,
    },
    /// served keys lie on the locus; they are replayed with every other served key
    TestedAtServed {
        keys: usize,
    },
    /// replayed at a searched integer point on the locus (index into the identity points)
    Tested {
        key: Vec<i64>,
        point: usize,
        status: PointStatus,
    },
    Unreadable(String),
}

#[derive(Clone, Debug, Serialize)]
pub struct LocusReport {
    pub locus: String,
    pub status: LocusStatus,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub enum ReplayLevel {
    /// every check passed at EVERY served key (plus the generic points)
    Established,
    /// every check passed, but the identity was replayed at a SAMPLE of the served keys only: never accepted
    ReplaySampled {
        replayed: usize,
        applicable: usize,
    },
    NotEstablished {
        reasons: Vec<String>,
    },
}

/// Witness-guard completeness of the record (audit M3).
#[derive(Clone, Debug, Serialize)]
pub struct GuardStatus {
    /// the rule's `guard_version`, else the table header's (absent = 1 in the engine's semantics)
    pub guard_version: Option<u64>,
    /// an empty witness-locus list is complete only with witness guards on (guard_version >= 2)
    pub witness_list_complete: bool,
}

/// The artefact of evidence level `replay` for one rule (a third party can re-run it from the recorded inputs).
#[derive(Clone, Debug, Serialize)]
pub struct ReplayRecord {
    pub schema: &'static str,
    pub tool_version: &'static str,
    /// sha256 of the family text and its name; whether a table header bound it (audit C2 / M4)
    pub family_digest: String,
    pub family_name: Option<String>,
    pub family_bound_to_table: bool,
    /// sha256 of the rule's canonical content (see [`rule_digest`])
    pub rule_digest: String,
    pub rule_index: Option<i64>,
    pub target: String,
    /// stream seed (derived from `master_seed` and the rule digest)
    pub seed: u64,
    pub master_seed: u64,
    /// the run-time nonce bound into the stream seed (re-run with `RowsOptions.nonce = Some(nonce)`)
    pub nonce: u64,
    pub primes: [u64; 2],
    pub policy: ReplayPolicy,
    pub served_keys: usize,
    /// sha256 of the served key set (sorted, deduplicated, one key per line, comma-separated) and its source
    pub served_keys_digest: String,
    pub served_keys_source: Option<String>,
    /// served keys at which the identity was replayed (all applicable ones for `Established`)
    pub served_keys_replayed: usize,
    /// how the served set is covered: "explicit" (every served key replayed); strata are a documented optimisation
    pub strata: &'static str,
    /// identity replay at the generic points, the served keys and the loci points, both primes
    pub identity: RowsVerdict,
    pub pole_scan: PoleScan,
    pub loci: Vec<LocusReport>,
    pub guards: GuardStatus,
    /// the symmetry group used (digest, provenance, mode, support) when the rule declares symmetry rows (section 10.5)
    pub symmetry_group: Option<SymmetryUse>,
    pub witness_elided: bool,
    pub witness_present: bool,
    pub level: ReplayLevel,
    /// checks of the level that live in other modules and are recorded beside this record
    pub checked_elsewhere: &'static str,
}

fn pat_text(p: &Pat) -> String {
    let parts: Vec<String> = p
        .iter()
        .enumerate()
        .map(|(i, &(ax, off))| match (ax, off) {
            (Some(_), 0) => format!("n{i}"),
            (Some(_), o) if o > 0 => format!("n{i}+{o}"),
            (Some(_), o) => format!("n{i}{o}"),
            (None, c) => c.to_string(),
        })
        .collect();
    format!("I({})", parts.join(","))
}

/// sha256 over the rule's canonical content: index, chart, fixed, equations, exceptions, target, rhs (key pattern text,
/// coefficient text verbatim), seeds, recenter, raw pivot, witness loci, guard version, sym_rows; fields separated by
/// NUL / SOH bytes.
pub fn rule_digest(rule: &RowsRule) -> String {
    let mut h = crate::replay_base::sha256::Sha256::new();
    let mut put = |tag: &str, v: &str| {
        h.update(tag.as_bytes());
        h.update(b"\x01");
        h.update(v.as_bytes());
        h.update(b"\x00");
    };
    put("version", ROWS_VERSION);
    put("index", &format!("{:?}", rule.index));
    put("chart", &rule.chart_text);
    put("fixed", &format!("{:?}", rule.fixed));
    put("equations", &format!("{:?}", rule.equations));
    put("exceptions", &format!("{:?}", rule.exceptions));
    put("target", &pat_text(&rule.target));
    for (k, c) in &rule.rhs {
        put("rhs_key", &pat_text(k));
        put("rhs_coef", c);
    }
    for sd in &rule.seeds {
        put("seed", &pat_text(sd));
    }
    put("recenter", &format!("{:?}", rule.recenter));
    put("raw_pivot", &format!("{:?}", rule.raw_pivot));
    put(
        "witness",
        &format!("{:?}/{}", rule.witness_loci, rule.witness_elided),
    );
    put("guard_version", &format!("{:?}", rule.guard_version));
    put("sym_rows", &format!("{:?}", rule.sym_rows));
    h.hex()
}

/// sha256 of a key set: sorted, deduplicated, one key per line, entries comma-separated.
pub fn key_set_digest(keys: &[Vec<i64>]) -> String {
    let mut ks: Vec<&Vec<i64>> = keys.iter().collect();
    ks.sort();
    ks.dedup();
    let text: String = ks
        .iter()
        .map(|k| k.iter().map(i64::to_string).collect::<Vec<_>>().join(",") + "\n")
        .collect();
    crate::replay_base::sha256::sha256_hex(text.as_bytes())
}

/// Identically zero in d at the rule point `pt` modulo `p` (deg_d + 1 distinct values).
fn den_identically_zero(
    rule: &RowsRule,
    k: usize,
    pt: &[i64],
    p: u64,
    s: &mut SplitMix64,
) -> Result<bool, String> {
    let degd = den_degd(rule, k, p)?;
    let base: Vec<u64> = pt.iter().map(|&x| arith::from_i128(p, x as i128)).collect();
    let mut ds: BTreeSet<u64> = BTreeSet::new();
    while ds.len() < degd as usize + 1 {
        ds.insert(draw_d(s, p));
    }
    let ctxs: Vec<Ctx> = ds
        .iter()
        .map(|&d| {
            let mut v = base.clone();
            v.push(d);
            Ctx { p, vals: v }
        })
        .collect();
    let (mut nn, mut dn) = (vec![0; ctxs.len()], vec![0; ctxs.len()]);
    coeff::eval_ratio(&rule.rhs[k].1, rule.arity, &ctxs, &mut nn, &mut dn)
        .map_err(|e| e.to_string())?;
    Ok(dn.iter().all(|&x| x == 0))
}

/// The pole check at one rule point: Ok(None) = no pole, Ok(Some(k)) = coefficient k has a pole at both primes,
/// Err = undecided (one prime only / unreadable).
fn pole_at(
    rule: &RowsRule,
    pt: &[i64],
    primes: [u64; 2],
    seed: u64,
    stream: &[u64],
) -> Result<Option<usize>, String> {
    let mut s = SplitMix64::derive(seed, stream);
    let ctxs: Vec<Ctx> = primes
        .iter()
        .map(|&p| {
            let mut v: Vec<u64> = pt.iter().map(|&x| arith::from_i128(p, x as i128)).collect();
            v.push(draw_d(&mut s, p));
            Ctx { p, vals: v }
        })
        .collect();
    let (mut num, mut den) = ([0u64; 2], [0u64; 2]);
    for (k, (_, text)) in rule.rhs.iter().enumerate() {
        coeff::eval_ratio(text, rule.arity, &ctxs, &mut num, &mut den)
            .map_err(|e| format!("rhs[{k}]: {e}"))?;
        if den.contains(&0) {
            let z0 = den_identically_zero(rule, k, pt, primes[0], &mut s)?;
            let z1 = den_identically_zero(rule, k, pt, primes[1], &mut s)?;
            match (z0, z1) {
                (true, true) => return Ok(Some(k)),
                (false, false) => {}
                _ => {
                    return Err(format!(
                        "rhs[{k}]: denominator identically zero in d at one prime only"
                    ));
                }
            }
        }
    }
    Ok(None)
}

/// Equal up to a nonzero constant factor (exact: primitive normal forms of the d-free polynomials).
fn same_poly(a: &str, b: &str, n: usize) -> bool {
    match (Poly::parse(a, n, false), Poly::parse(b, n, false)) {
        (Ok(x), Ok(y)) => x.primitive_normal() == y.primitive_normal(),
        _ => false,
    }
}

/// Proof that an AFFINE d-free locus has no integer point in the case: after substituting the fixed slots, the gcd
/// of the integer coefficients of the free slots does not divide the constant. Dependent (equation-solved) slots in
/// the locus: no proof (false).
fn locus_has_no_integer_point(rule: &RowsRule, text: &str, solved: Option<&Solved>) -> bool {
    let n = rule.arity;
    let Ok(lin) = Expr::parse(text, n).and_then(|e| e.linear()) else {
        return false;
    };
    let mut c = lin[0];
    let mut coefs: Vec<Q> = Vec::new();
    for i in 0..n {
        let a = lin[i + 1];
        if a.is_zero() {
            continue;
        }
        match rule.fixed[i] {
            Some(v) => match a.mul(Q::int(v as i128)).and_then(|x| c.add(x)) {
                Ok(x) => c = x,
                Err(_) => return false,
            },
            None if solved.is_some_and(|s| s.contains_key(&i)) => return false,
            None => coefs.push(a),
        }
    }
    // scale to integers
    let mut l: i128 = c.d;
    for q in &coefs {
        let g = gcd_i128(l, q.d);
        l = match (l / g).checked_mul(q.d) {
            Some(x) => x,
            None => return false,
        };
    }
    let ci = match c.n.checked_mul(l / c.d) {
        Some(x) => x,
        None => return false,
    };
    let mut g: i128 = 0;
    for q in &coefs {
        match q.n.checked_mul(l / q.d) {
            Some(x) => g = gcd_i128(g, x),
            None => return false,
        }
    }
    if g == 0 {
        return ci != 0; // constant nonzero locus: never zero
    }
    ci % g != 0
}

fn gcd_i128(mut a: i128, mut b: i128) -> i128 {
    a = a.abs();
    b = b.abs();
    while b != 0 {
        (a, b) = (b, a % b);
    }
    a.max(1)
}

/// An applicable integer key on the locus `poly = 0` in the window (deterministic), and the number of candidates seen.
/// The case equations are honoured (audit M3): slots solved by affine equations are computed from the free ones and
/// must be integers.
fn locus_point(
    rule: &RowsRule,
    poly: &Poly,
    policy: &ReplayPolicy,
    seed: u64,
    solved: Option<&Solved>,
) -> (Option<Vec<i64>>, usize) {
    let free: Vec<usize> = (0..rule.arity)
        .filter(|&i| rule.fixed[i].is_none() && !solved.is_some_and(|s| s.contains_key(&i)))
        .collect();
    let w = policy.loci_window.max(0);
    let span = (w + 1) as u128;
    let total = span.checked_pow(free.len() as u32).unwrap_or(u128::MAX);
    let sample = total > policy.loci_max_candidates as u128;
    let count = if sample {
        policy.loci_max_candidates as u128
    } else {
        total
    };
    let mut s = SplitMix64::derive(seed, &[0x4c4f4349]); // "LOCI"
    let mut seen = 0usize;
    'cand: for idx in 0..count {
        let mut pt: Vec<i64> = rule.fixed.iter().map(|f| f.unwrap_or(0)).collect();
        let mut r = idx;
        for &i in &free {
            let e = if sample {
                s.range(0, span as u64) as i64
            } else {
                let e = (r % span) as i64;
                r /= span;
                e
            };
            pt[i] = if rule.chart[i] { 1 + e } else { -e };
        }
        if let Some(sol) = solved {
            for (&i, (c, deps)) in sol {
                let mut v = *c;
                for (j, a) in deps {
                    match a.mul(Q::int(pt[*j] as i128)).and_then(|x| v.add(x)) {
                        Ok(x) => v = x,
                        Err(_) => continue 'cand,
                    }
                }
                if v.d != 1 {
                    continue 'cand;
                }
                match i64::try_from(v.n) {
                    Ok(x) => pt[i] = x,
                    Err(_) => continue 'cand,
                }
            }
        }
        seen += 1;
        let Ok(p32) = pt
            .iter()
            .map(|&x| i32::try_from(x))
            .collect::<Result<Vec<_>, _>>()
        else {
            continue;
        };
        if !matches!(poly.is_zero_at(&p32), Ok(true)) {
            continue;
        }
        let key = record::key_at(&rule.target, &pt);
        if let Ok(Ok(_)) = rule_point(rule, &key) {
            return (Some(key), seen);
        }
    }
    (None, seen)
}

/// EVIDENCE LEVEL `replay` of one rule ON ITS SERVED KEY SET (R40 U-12 / R41 P-3, corrected after audit A): the
/// identity replayed at EVERY served key (and the generic points) at two verify-range primes (never a lift prime),
/// the pole check at every served key, every served key applicable, every unguarded witness locus tested (at a served
/// key on it or a searched point; untested = not established), witness guards on when the locus list is empty, the
/// family bound by digest (and to the table header when given). Descent under the node comparator and the guard
/// (domain / exception) checks are recorded beside it by the accept module (`checked_elsewhere`).
pub fn replay_evidence(
    family: &Family,
    rule: &RowsRule,
    served_keys: &[Vec<i64>],
    policy: &ReplayPolicy,
    opts: &RowsOptions,
) -> ReplayRecord {
    let master_seed = opts.seed;
    let nonce = opts.nonce.unwrap_or_else(os_nonce);
    let mut dedup0: Vec<Vec<i64>> = served_keys.to_vec();
    dedup0.sort();
    dedup0.dedup();
    let derived_opts = RowsOptions {
        seed: stream_seed(
            opts.seed,
            rule,
            &family.source_digest,
            &key_set_digest(&dedup0),
            nonce,
        ),
        nonce: Some(nonce),
        ..opts.clone()
    };
    let opts = &derived_opts;
    let rule_id = opts.rule_id.unwrap_or_else(|| rule.default_id());
    let primes = opts
        .primes
        .unwrap_or_else(|| primes::verify_primes(opts.seed, &opts.lift_primes));
    let mut reasons: Vec<String> = Vec::new();
    // a point rule with no served key given: its own key is the served set
    let own: Vec<Vec<i64>>;
    let served_keys: &[Vec<i64>] =
        if served_keys.is_empty() && rule.fixed.iter().all(Option::is_some) {
            let pt: Vec<i64> = rule.fixed.iter().map(|f| f.unwrap_or(0)).collect();
            own = vec![record::key_at(&rule.target, &pt)];
            &own
        } else {
            served_keys
        };
    // audit B M2: duplicate served keys count once
    let mut dedup: Vec<Vec<i64>> = served_keys.to_vec();
    dedup.sort();
    dedup.dedup();
    let served_keys: &[Vec<i64>] = &dedup;
    // pole scan and applicability at every served key
    let mut ps = PoleScan {
        keys_given: served_keys.len(),
        ..PoleScan::default()
    };
    let mut applicable: Vec<(&Vec<i64>, Vec<i64>)> = Vec::new();
    for (i, k) in served_keys.iter().enumerate() {
        match rule_point(rule, k) {
            Ok(Ok(pt)) => {
                ps.keys_applicable += 1;
                if rule.syntax_error.is_none() {
                    match pole_at(
                        rule,
                        &pt,
                        primes,
                        opts.seed,
                        &[rule_id, i as u64, PURPOSE_POLE, 0x504f4c45],
                    ) {
                        Ok(None) => {}
                        Ok(Some(c)) => ps.poles.push(PoleHit {
                            key: k.clone(),
                            coefficient: c,
                        }),
                        Err(e) => ps.undecided.push((k.clone(), e)),
                    }
                }
                applicable.push((k, pt));
            }
            Ok(Err(r)) => {
                ps.keys_not_applicable += 1;
                ps.not_applicable.push((k.clone(), r));
            }
            Err(u) => ps.undecided.push((k.clone(), format!("{u:?}"))),
        }
    }
    // points: generic, served keys (all, or a sample -> ReplaySampled), loci
    let mut points: Vec<PointSpec> = Vec::new();
    if rule.fixed.iter().any(Option::is_none) {
        points.extend(std::iter::repeat_n(
            PointSpec::Generic,
            policy.generic_points,
        ));
    }
    let sampled = policy.max_identity_keys != 0 && applicable.len() > policy.max_identity_keys;
    let take: Vec<&Vec<i64>> = if !sampled {
        applicable.iter().map(|(k, _)| *k).collect()
    } else {
        let mut s = SplitMix64::derive(opts.seed, &[rule_id, 0x4b455953]); // "KEYS"
        let mut idx: BTreeSet<usize> = BTreeSet::new();
        while idx.len() < policy.max_identity_keys {
            idx.insert(s.range(0, applicable.len() as u64) as usize);
        }
        idx.into_iter().map(|i| applicable[i].0).collect()
    };
    let served_keys_replayed = take.len();
    points.extend(take.iter().map(|k| PointSpec::Key((*k).clone())));
    let solved_owned = solve_equations(rule).ok().and_then(Result::ok);
    let solved = solved_owned.as_ref();
    let mut loci: Vec<LocusReport> = Vec::new();
    let mut tested: Vec<(usize, usize)> = Vec::new(); // (loci index, point index)
    for text in &rule.witness_loci {
        let status = match Expr::parse(text, rule.arity) {
            Err(e) => LocusStatus::Unreadable(e),
            Ok(e) if e.has_d() => LocusStatus::Parameter,
            Ok(_)
                if rule
                    .exceptions
                    .iter()
                    .any(|b| b.len() == 1 && same_poly(&b[0], text, rule.arity)) =>
            {
                LocusStatus::Guarded
            }
            Ok(_) => match Poly::parse(text, rule.arity, false) {
                Err(e) => LocusStatus::Unreadable(e.to_string()),
                Ok(poly) => {
                    // served keys on the locus (all replayed when !sampled)
                    let on: Vec<&Vec<i64>> = applicable
                        .iter()
                        .filter(|(_, pt)| {
                            pt.iter()
                                .map(|&x| i32::try_from(x))
                                .collect::<Result<Vec<_>, _>>()
                                .is_ok_and(|p32| matches!(poly.is_zero_at(&p32), Ok(true)))
                        })
                        .map(|(k, _)| *k)
                        .collect();
                    if !on.is_empty() && !sampled {
                        LocusStatus::TestedAtServed { keys: on.len() }
                    } else if locus_has_no_integer_point(rule, text, solved) {
                        LocusStatus::NoIntegerPoint
                    } else {
                        match locus_point(rule, &poly, policy, opts.seed ^ rule_id, solved) {
                            (Some(key), _) => {
                                tested.push((loci.len(), points.len()));
                                points.push(PointSpec::Key(key.clone()));
                                LocusStatus::Tested {
                                    key,
                                    point: points.len() - 1,
                                    status: PointStatus::Pass,
                                }
                            }
                            (None, n) => LocusStatus::NoPoint { candidates: n },
                        }
                    }
                }
            },
        };
        loci.push(LocusReport {
            locus: text.clone(),
            status,
        });
    }
    // the identity uses the record's primes and nonce (its d values derive from its own stream seed)
    let id_opts = RowsOptions {
        seed: master_seed,
        primes: Some(primes),
        nonce: Some(nonce),
        ..opts.clone()
    };
    let identity = identity_test(family, rule, &points, &id_opts);
    for (li, pi) in tested {
        let name = loci[li].locus.clone();
        if let LocusStatus::Tested { status, .. } = &mut loci[li].status {
            *status = identity.points.get(pi).map(|p| p.status.clone()).unwrap_or(
                PointStatus::Undecided {
                    reason: "not run".into(),
                },
            );
            if *status != PointStatus::Pass {
                reasons.push(format!("witness locus {name}: {status:?}"));
            }
        }
    }
    if identity.verdict != Verdict::Pass {
        reasons.push(format!("identity: {}", identity.verdict.kind()));
    } else if identity.points_passed < 2 && rule.fixed.iter().any(Option::is_none) {
        // symbolic rules: >= 2 points (the generic points); a POINT rule is decided at its one key at both primes
        reasons.push(format!(
            "identity passed at {} point(s), 2 required",
            identity.points_passed
        ));
    }
    // audit M2: nothing served = nothing established; every served key must be applicable
    if ps.keys_applicable == 0 {
        reasons.push("NothingServed: no served key is applicable to the rule".into());
    }
    if ps.keys_not_applicable > 0 {
        reasons.push(format!(
            "{} served key(s) not applicable to the rule (first: {:?})",
            ps.keys_not_applicable,
            ps.not_applicable.first()
        ));
    }
    if !sampled && identity.verdict == Verdict::Pass {
        // every served key point must itself have passed (not merely the rule overall)
        let bad = identity
            .points
            .iter()
            .filter(|p| matches!(p.spec, PointSpec::Key(_)) && p.status != PointStatus::Pass)
            .count();
        if bad > 0 {
            reasons.push(format!("{bad} replayed key(s) did not pass"));
        }
    }
    if !ps.poles.is_empty() {
        reasons.push(format!(
            "{} served key(s) on a coefficient pole",
            ps.poles.len()
        ));
    }
    if !ps.undecided.is_empty() {
        reasons.push(format!(
            "{} served key(s) with an undecided pole check",
            ps.undecided.len()
        ));
    }
    if rule.witness_elided {
        reasons.push("witness loci elided (unusable record)".into());
    }
    if !rule.witness_present {
        reasons.push("witness loci absent (incomplete record)".into());
    }
    // audit M3: an empty witness list is complete only with witness guards on (guard_version >= 2); point rules
    // (no free slot) are evaluated at their one key and need no loci
    let guard_version = rule
        .guard_version
        .or_else(|| opts.table.as_ref().and_then(|t| t.guard_version));
    let free = rule.fixed.iter().any(Option::is_none);
    let witness_list_complete =
        !free || !rule.witness_loci.is_empty() || guard_version.unwrap_or(1) >= 2;
    if !witness_list_complete {
        reasons.push(
            "no witness guards: empty witness-locus list with guard_version 1 or absent (Unsupported(no witness guards))"
                .into(),
        );
    }
    for l in &loci {
        match &l.status {
            LocusStatus::Unreadable(e) => {
                reasons.push(format!("witness locus {} unreadable: {e}", l.locus))
            }
            LocusStatus::NoPoint { candidates } => reasons.push(format!(
                "witness locus {} UNTESTED: no point in {candidates} candidates",
                l.locus
            )),
            _ => {}
        }
    }
    let level = if !reasons.is_empty() {
        ReplayLevel::NotEstablished { reasons }
    } else if sampled {
        ReplayLevel::ReplaySampled {
            replayed: served_keys_replayed,
            applicable: applicable.len(),
        }
    } else {
        ReplayLevel::Established
    };
    ReplayRecord {
        schema: REPLAY_SCHEMA,
        tool_version: ROWS_VERSION,
        family_digest: family.source_digest.clone(),
        family_name: family.name.clone(),
        family_bound_to_table: opts.table.is_some(),
        rule_digest: rule_digest(rule),
        rule_index: rule.index,
        target: rule.target_text.clone(),
        seed: opts.seed,
        master_seed,
        nonce,
        primes,
        policy: policy.clone(),
        served_keys: served_keys.len(),
        served_keys_digest: key_set_digest(served_keys),
        served_keys_source: policy.served_keys_source.clone(),
        served_keys_replayed,
        strata: "explicit: every served key replayed (strata not implemented; ibp-rows.md 9.4)",
        symmetry_group: identity.symmetry.clone(),
        identity,
        pole_scan: ps,
        loci,
        guards: GuardStatus {
            guard_version,
            witness_list_complete,
        },
        witness_elided: rule.witness_elided,
        witness_present: rule.witness_present,
        level,
        checked_elsewhere: "descent under the node comparator (order/accept), domain and exception guards at the served keys (domain/accept)",
    }
}

// ------------------------------------------------------------------ batch --------------------------------------------

/// Counts and cost of a batch run.
#[derive(Clone, Debug, Default, Serialize)]
pub struct BatchSummary {
    pub lines: usize,
    pub rules: usize,
    pub by_verdict: BTreeMap<String, usize>,
    pub rows: usize,
    pub entries_peak: usize,
    pub peak_bytes_estimate: usize,
    pub seconds: f64,
}

/// One output line of the batch.
#[derive(Clone, Debug, Serialize)]
pub struct BatchLine<'a> {
    pub line_no: usize,
    pub run: Option<&'a str>,
    pub rule_in_line: usize,
    pub result: &'a RowsVerdict,
}

/// Stream a JSONL file of rule records (table_dump lines or export wrapper lines; header and other lines skipped),
/// test each rule at the points `plan` names (it sees the line context: run, gap_points), write one JSON line per
/// rule to `out` in input order. Unreadable rule records give an `Unsupported(Record)` line; I/O errors stop the run.
pub fn identity_test_batch<R: BufRead, W: Write>(
    family: &Family,
    mut input: R,
    plan: &dyn Fn(&RowsRule, &LineContext) -> Vec<PointSpec>,
    opts: &RowsOptions,
    mut out: W,
) -> Result<BatchSummary, RowsError> {
    let t0 = Instant::now();
    let mut sum = BatchSummary::default();
    let mut buf: Vec<u8> = Vec::new();
    let mut line_no = 0usize;
    let mut table: Option<TableBinding> = opts.table.clone();
    loop {
        buf.clear();
        let n = input
            .read_until(b'\n', &mut buf)
            .map_err(|e| RowsError::Io(e.to_string()))?;
        if n == 0 {
            break;
        }
        line_no += 1;
        sum.lines += 1;
        let text =
            std::str::from_utf8(&buf).map_err(|e| RowsError::Io(format!("line {line_no}: {e}")))?;
        if text.trim().is_empty() {
            continue;
        }
        // audit C2: a table header binds the family for the following rules; an export line binds its own
        // `cert_header` (the caller's `opts.table` is the default)
        let mut line_table: Option<TableBinding> = None;
        let parsed = serde_json::from_str::<Value>(text)
            .map_err(|e| RowsError::Record(format!("json: {e}")))
            .and_then(|v| {
                if let Some(t) = TableBinding::from_header(&v) {
                    table = Some(t);
                }
                line_table = v
                    .get("cert")
                    .and_then(Value::as_array)
                    .and_then(|c| c.iter().find_map(TableBinding::from_header));
                RowsRule::from_line_value(v, family.n)
            });
        let lopts_owned;
        let lopts: &RowsOptions = match line_table.or_else(|| table.clone()) {
            Some(t) => {
                lopts_owned = RowsOptions {
                    table: Some(t),
                    ..opts.clone()
                };
                &lopts_owned
            }
            None => opts,
        };
        let mut emit = |v: &RowsVerdict, run: Option<&str>, k: usize| -> Result<(), RowsError> {
            sum.rules += 1;
            *sum.by_verdict
                .entry(v.verdict.kind().to_string())
                .or_insert(0) += 1;
            sum.rows += v.cost.rows;
            sum.entries_peak = sum.entries_peak.max(v.cost.entries_peak);
            sum.peak_bytes_estimate = sum.peak_bytes_estimate.max(v.cost.peak_bytes_estimate);
            let line = BatchLine {
                line_no,
                run,
                rule_in_line: k,
                result: v,
            };
            serde_json::to_writer(&mut out, &line).map_err(|e| RowsError::Io(e.to_string()))?;
            out.write_all(b"\n")
                .map_err(|e| RowsError::Io(e.to_string()))
        };
        match parsed {
            Ok((rules, ctx)) => {
                for (k, r) in rules.iter().enumerate() {
                    let pts = plan(r, &ctx);
                    let v = identity_test(family, r, &pts, lopts);
                    emit(&v, ctx.run.as_deref(), k)?;
                }
            }
            Err(e) => {
                let v = RowsVerdict {
                    version: ROWS_VERSION,
                    family_digest: family.source_digest.clone(),
                    family_name: family.name.clone(),
                    family_bound_to_table: false,
                    verdict: Verdict::Unsupported(Unsupported::Record(e.to_string())),
                    rule_index: None,
                    target: String::new(),
                    rule_id: 0,
                    seed: opts.seed,
                    master_seed: opts.seed,
                    nonce: 0,
                    primes: opts
                        .primes
                        .unwrap_or_else(|| primes::verify_primes(opts.seed, &opts.lift_primes)),
                    points: Vec::new(),
                    points_passed: 0,
                    sz_bound: 1.0,
                    sz_bound_log10: 0.0,
                    sz_bound_per_cell_max: 1.0,
                    meets_a2_policy: false,
                    symmetry: None,
                    cost: Cost::default(),
                };
                emit(&v, None, 0)?;
            }
        }
    }
    sum.seconds = t0.elapsed().as_secs_f64();
    Ok(sum)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// No deciding call may use the replay's P61 / P2 (plan 3.1 (e)): this file never names them.
    #[test]
    fn never_decides_at_the_replay_primes() {
        let src = include_str!("ibp_rows.rs");
        let body = &src[..src.find("#[cfg(test)]").unwrap()];
        for forbidden in [
            "arith::P,",
            "arith::P)",
            "arith::P;",
            "arith::P2",
            "arith::P ",
        ] {
            assert!(!body.contains(forbidden), "ibp_rows.rs uses {forbidden}");
        }
    }

    #[test]
    fn column_text_round_trip() {
        let k: ColKey = (0b101, vec![0, 3, -2]);
        assert_eq!(col_text(&k), "I(n0,3,n2-2)");
        let pat = record::parse_integral("I(n0,3,n2-2)", 3).unwrap();
        assert_eq!(pattern_col(&pat, true, &[], &[None, None, None]), k);
    }
}
