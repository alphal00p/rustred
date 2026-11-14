//! Node generation under ONE node order: the face-queue loop of the 5L campaign (formerly the external
//! `face_queue_r9` harness) as engine code.
//!
//! One chart of a one-sided family is solved by the FirstValid case queue: each queued case is searched
//! (modular discovery + exact lift of the first winning trace), its strict descent is checked, its exceptional
//! branches are admitted as child cases (G-split refinement, deferring), children covered by earlier rules (own or
//! cover tables) are dropped. Differences to the harness, all driven by the 5L campaign findings:
//!
//! * the order is a [`NodeOrder`] (presentation permutation, tie priority, source visit order), recorded in the
//!   table header; the family is solved in its presented form and the table is in presented slots;
//! * every lift may be pre-screened ([`super::screen`]); a refused lift is retried with other source visit
//!   orders (other candidate relations for the same pivot) and, if all are refused, the node stops with
//!   [`StopReason::LiftRefused`] naming the case, so that the caller can choose another node order;
//! * the fully fixed (numeric endpoint) cases are solved one by one through the same case search with the node's
//!   source order and exact backend (the r9 harness sent them to the shared numeric solve, which ignores both);
//!   only points the per-point search exhausts go to the shared solve, whose residuals are the chart residuals;
//! * every rule records its raw pivot / recenter, trace rows and lift time (replay authority, probe scores).

use std::cmp::Ordering;
use std::io::Write as _;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::Mutex;
use std::time::Instant;

use serde::{Deserialize, Serialize};

use super::order::{NodeOrder, SourceOrder};
use super::screen::{LiftScreen, ScreenReport};
use crate::algebra::CoefficientPolynomial;
use crate::family::IntegralFamily;
use crate::input::{Compiler, Limits, LoweringLimits};
use crate::sector::{Mask, zero};
use crate::solver::{
    Case, CaseIntersectionLimits, CaseRefinement, CaseSeedSchedule, CoordinateCase,
    ExceptionalConditions, GuardRoute, GuardRouteTaken, Integral, RuleCandidate, SearchEvent,
    SearchOptions, SectorConfig, SectorRule, SectorSolveOptions, SectorSolver, SolverError,
    SourceSystem, SymbolicExactBackend, UnsupportedBranchPolicy, extract_exceptions,
    extract_exceptions_routed,
};

/// Table schema of node tables: table_dump.v1 records plus node fields (header `node_order`, rule `recenter`, ...).
pub const SCHEMA: &str = "table_dump.v1";
pub const NODE_SCHEMA: &str = "rustred.node.v1";
/// Guard semantics of a node table (header `guard_version`; absent = 1).
/// 1: rule exceptions = index denominators of the final RHS + activation boundaries only.
/// 2 (guard-fix, 2026-11-12): + the index-containing factors of the source-weight denominators of the lift that
///    produced each rule (and of a direct hit's pivot), so a rule is never used where its witness proves nothing.
///    Tables written with `--no-witness-guards` say 1.
pub const GUARD_VERSION: u32 = 2;

/// Optional per-current-Case priority for an existing finite shell schedule.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum CaseSeedPriority {
    SymbolicIspFirst,
    /// AFFINE FACES (affine-faces, 2026-11-11): on a case with equations the template's shells are searched with
    /// the equation-COUPLED free axes first (free axes whose unit shift is not tangent to the face), so that the
    /// first sources are the neighbours `T + u + e_y` of one layer `l(n) = +-1` next to the face, where the face's
    /// own depth-1 relations are degenerate; cases WITHOUT equations keep the default seed path unchanged.
    /// Use with a `ShiftShells` template of depths 2..2 (notes/affine-faces.md: T210 #2211 3.5M -> 52 monomials).
    AffineCoupledFirst,
}

impl CaseSeedPriority {
    /// Validate direct Rust callers as strictly as the CLI, before preparation.
    pub fn validate_template(
        self,
        schedule: Option<&CaseSeedSchedule>,
        arity: usize,
    ) -> Result<(), String> {
        let schedule =
            schedule.ok_or("case seed priority requires --case-seed-schedule ShiftShells")?;
        schedule.validate(arity).map_err(|e| e.to_string())?;
        if !matches!(schedule, CaseSeedSchedule::ShiftShells { .. }) {
            return Err("case seed priority requires ShiftShells, not Reference".into());
        }
        Ok(())
    }
}

/// An immutable, caller-declared terminal ledger. Reading computes the SHA256 of the actual file;
/// neither a declaration nor its source references prove that an integral is a master.
#[derive(Clone, Debug)]
pub struct KnownTerminalsFile {
    path: String,
    sha256: String,
    ledger: TerminalLedger,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct TerminalLedger {
    schema: String,
    frame: TerminalFrame,
    original_family_source_sha256: String,
    node_order: serde_json::Value,
    chart_presented: String,
    terminals: Vec<TerminalDeclaration>,
}

// The variant names are the ledger file's `frame` values (serde), hence upper case.
#[allow(clippy::upper_case_acronyms)]
#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
enum TerminalFrame {
    ORIGINAL,
    PRESENTED,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct TerminalDeclaration {
    powers: Vec<i64>,
    sources: Vec<TerminalSource>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TerminalSource {
    kind: TerminalSourceKind,
    reference: String,
    sha256: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    note: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
enum TerminalSourceKind {
    #[serde(rename = "old-table-master")]
    OldTableMaster,
    #[serde(rename = "exact-master-map")]
    ExactMasterMap,
    #[serde(rename = "lp-count")]
    LpCount,
}

impl KnownTerminalsFile {
    pub fn read(path: impl AsRef<Path>) -> Result<Self, String> {
        let path = path.as_ref();
        let bytes =
            std::fs::read(path).map_err(|e| format!("known terminals {}: {e}", path.display()))?;
        let ledger: TerminalLedger =
            serde_json::from_slice(&bytes).map_err(|e| format!("known terminals: {e}"))?;
        let valid_hash = |s: &str| s.len() == 64 && s.bytes().all(|b| b.is_ascii_hexdigit());
        if ledger.schema != "rustred.node.known-terminals.v1" || ledger.terminals.is_empty() {
            return Err("known terminals: unsupported schema or empty ledger".into());
        }
        if !valid_hash(&ledger.original_family_source_sha256) {
            return Err("known terminals: malformed family SHA256".into());
        }
        for t in &ledger.terminals {
            if t.sources.is_empty()
                || t.sources
                    .iter()
                    .any(|s| s.reference.trim().is_empty() || !valid_hash(&s.sha256))
            {
                return Err("known terminals: every key requires nonempty typed source references and SHA256".into());
            }
        }
        Ok(Self {
            path: path.display().to_string(),
            sha256: terminal_sha256(&bytes),
            ledger,
        })
    }
}

impl Serialize for KnownTerminalsFile {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.path)
    }
}

impl<'de> Deserialize<'de> for KnownTerminalsFile {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let path = String::deserialize(deserializer)?;
        Self::read(path).map_err(serde::de::Error::custom)
    }
}

/// Residual authority for active ledgers, explicitly separate from native master discovery.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DeclaredTerminalLedger {
    authority: String,
    file: String,
    file_sha256: String,
    input_frame: String,
    chart_presented: String,
    terminals: Vec<DeclaredTerminal>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct DeclaredTerminal {
    key: String,
    powers_presented: Vec<i64>,
    sources: Vec<TerminalSource>,
}

impl KnownTerminalsFile {
    fn bind<const N: usize>(
        &self,
        original: &str,
        opts: &NodeOptions,
        sector: &[bool; N],
        n: usize,
    ) -> Result<DeclaredTerminalLedger, String> {
        let l = &self.ledger;
        if !l
            .original_family_source_sha256
            .eq_ignore_ascii_case(&terminal_sha256(original.as_bytes()))
            || l.node_order != serde_json::to_value(&opts.order).map_err(|e| e.to_string())?
            || l.chart_presented != mask_string(&sector[..n])
        {
            return Err(
                "known terminals: original family, node order or presented chart differs".into(),
            );
        }
        let mut terminals = Vec::new();
        let mut seen = std::collections::BTreeSet::new();
        for t in &l.terminals {
            if t.powers.len() != n {
                return Err("known terminals: key arity differs".into());
            }
            let values: Vec<i64> = match l.frame {
                TerminalFrame::PRESENTED => t.powers.clone(),
                TerminalFrame::ORIGINAL => match &opts.order.slot_permutation {
                    Some(p) => p.iter().map(|&i| t.powers[i]).collect(),
                    None => t.powers.clone(),
                },
            };
            if values.iter().enumerate().any(|(i, &v)| {
                !(crate::solver::Power::MIN as i64..=crate::solver::Power::MAX as i64).contains(&v)
                    || (v > 0) != sector[i]
            }) {
                return Err("known terminals: malformed or off-chart key".into());
            }
            let powers = std::array::from_fn(|i| if i < n { values[i] as i16 } else { 0 });
            let key = Integral::<N>::numeric(powers)
                .map_err(|e| e.to_string())?
                .to_string();
            if !seen.insert(key.clone()) {
                return Err("known terminals: duplicate key".into());
            }
            terminals.push(DeclaredTerminal {
                key,
                powers_presented: values,
                sources: t.sources.clone(),
            });
        }
        Ok(DeclaredTerminalLedger {
            authority: "DECLARED_NON_NATIVE".into(),
            file: self.path.clone(),
            file_sha256: self.sha256.clone(),
            input_frame: match l.frame {
                TerminalFrame::ORIGINAL => "ORIGINAL",
                TerminalFrame::PRESENTED => "PRESENTED",
            }
            .into(),
            chart_presented: l.chart_presented.clone(),
            terminals,
        })
    }
}

impl DeclaredTerminalLedger {
    fn is_point<const N: usize>(&self, case: &Case<N>, n: usize) -> bool {
        case.is_numerical()
            && self.terminals.iter().any(|t| {
                (0..n).all(|i| case.fixed()[i].map(i64::from) == Some(t.powers_presented[i]))
            })
    }

    /// Evaluate the actual target frame, equality domain and complete OR-of-AND exception set.
    /// No RHS inspection is needed: an applicable zero-RHS rule also contradicts a declaration.
    #[allow(clippy::too_many_arguments)]
    fn check_rule<const N: usize>(
        &self,
        target: &Integral<N>,
        case: &Case<N>,
        branches: &[Vec<CoefficientPolynomial>],
        idx: &[usize; N],
        sector: &[bool; N],
        n: usize,
        source: &str,
    ) -> Result<(), String> {
        for t in &self.terminals {
            let mut at = [0i16; N];
            let mut matches = true;
            for i in 0..n {
                let p = target.powers()[i];
                let value = if p.is_symbolic() {
                    t.powers_presented[i] - i64::from(p.value())
                } else {
                    if t.powers_presented[i] != i64::from(p.value()) {
                        matches = false;
                        break;
                    }
                    i64::from(p.value())
                };
                if (value > 0) != sector[i]
                    || case.fixed()[i].is_some_and(|v| i64::from(v) != value)
                {
                    matches = false;
                    break;
                }
                // Coordinates are values, not compact encoded powers. A shifted parameter can exceed Power.
                at[i] = i16::try_from(value)
                    .map_err(|_| "known terminals: target parameter overflow")?;
            }
            if !matches {
                continue;
            }
            let at_zero = |p: &CoefficientPolynomial| {
                let mut p = p.clone();
                for i in 0..n {
                    p = p.replace(idx[i], &symbolica::domains::integer::Integer::from(at[i]));
                }
                p.is_zero()
            };
            if case
                .affine()
                .is_some_and(|a| a.equations().iter().any(|p| !at_zero(p)))
            {
                continue;
            }
            if branches.iter().any(|b| b.iter().all(&at_zero)) {
                continue;
            }
            return Err(format!(
                "known terminals: {source} reduces declared key {}; declaration is not a master proof",
                t.key
            ));
        }
        Ok(())
    }

    fn check_raw_rule<const N: usize>(
        &self,
        v: &serde_json::Value,
        template: &CoefficientPolynomial,
        idx: &[usize; N],
        sector: &[bool; N],
        n: usize,
        source: &str,
    ) -> Result<(), String> {
        let target = super::descent::parse_key::<N>(
            v["target"]
                .as_str()
                .ok_or("known terminals: missing cover target")?,
        )?;
        let fixed: Vec<Option<i64>> = serde_json::from_value(v["fixed"].clone())
            .map_err(|e| format!("known terminals: cover fixed: {e}"))?;
        let equations: Vec<String> = serde_json::from_value(v["equations"].clone())
            .map_err(|e| format!("known terminals: cover equations: {e}"))?;
        let Some(case) = build_case::<N>(&CaseSpec { fixed, equations }, template, idx, sector, n)?
        else {
            return Ok(());
        };
        let bs: Vec<Vec<String>> = serde_json::from_value(v["exceptions"].clone())
            .map_err(|e| format!("known terminals: cover exceptions: {e}"))?;
        let branches = bs
            .into_iter()
            .map(|b| {
                b.into_iter()
                    .map(|p| {
                        parse_index_poly(&p, template, idx)
                            .ok_or_else(|| format!("known terminals: unsupported exception {p}"))
                    })
                    .collect::<Result<Vec<_>, _>>()
            })
            .collect::<Result<Vec<_>, _>>()?;
        self.check_rule(&target, &case, &branches, idx, sector, n, source)
    }

    fn accept_native_residual(&self, key: &str) -> Result<(), String> {
        if self.terminals.iter().any(|t| t.key == key) {
            Ok(())
        } else {
            Err(format!(
                "known terminals: native residual {key} is outside the declared set"
            ))
        }
    }
}

// SHA256 is local to file provenance; no new dependency or trusted self-reported digest.
fn terminal_sha256(bytes: &[u8]) -> String {
    const K: [u32; 64] = [
        0x428a2f98, 0x71374491, 0xb5c0fbcf, 0xe9b5dba5, 0x3956c25b, 0x59f111f1, 0x923f82a4,
        0xab1c5ed5, 0xd807aa98, 0x12835b01, 0x243185be, 0x550c7dc3, 0x72be5d74, 0x80deb1fe,
        0x9bdc06a7, 0xc19bf174, 0xe49b69c1, 0xefbe4786, 0x0fc19dc6, 0x240ca1cc, 0x2de92c6f,
        0x4a7484aa, 0x5cb0a9dc, 0x76f988da, 0x983e5152, 0xa831c66d, 0xb00327c8, 0xbf597fc7,
        0xc6e00bf3, 0xd5a79147, 0x06ca6351, 0x14292967, 0x27b70a85, 0x2e1b2138, 0x4d2c6dfc,
        0x53380d13, 0x650a7354, 0x766a0abb, 0x81c2c92e, 0x92722c85, 0xa2bfe8a1, 0xa81a664b,
        0xc24b8b70, 0xc76c51a3, 0xd192e819, 0xd6990624, 0xf40e3585, 0x106aa070, 0x19a4c116,
        0x1e376c08, 0x2748774c, 0x34b0bcb5, 0x391c0cb3, 0x4ed8aa4a, 0x5b9cca4f, 0x682e6ff3,
        0x748f82ee, 0x78a5636f, 0x84c87814, 0x8cc70208, 0x90befffa, 0xa4506ceb, 0xbef9a3f7,
        0xc67178f2,
    ];
    let mut h: [u32; 8] = [
        0x6a09e667, 0xbb67ae85, 0x3c6ef372, 0xa54ff53a, 0x510e527f, 0x9b05688c, 0x1f83d9ab,
        0x5be0cd19,
    ];
    let mut data = bytes.to_vec();
    data.push(0x80);
    while data.len() % 64 != 56 {
        data.push(0);
    }
    data.extend_from_slice(&(bytes.len() as u64).wrapping_mul(8).to_be_bytes());
    for block in data.as_chunks::<64>().0 {
        let mut w = [0u32; 64];
        for i in 0..16 {
            w[i] = u32::from_be_bytes(block[4 * i..4 * i + 4].try_into().unwrap());
        }
        for i in 16..64 {
            let s0 = w[i - 15].rotate_right(7) ^ w[i - 15].rotate_right(18) ^ (w[i - 15] >> 3);
            let s1 = w[i - 2].rotate_right(17) ^ w[i - 2].rotate_right(19) ^ (w[i - 2] >> 10);
            w[i] = w[i - 16]
                .wrapping_add(s0)
                .wrapping_add(w[i - 7])
                .wrapping_add(s1);
        }
        let [mut a, mut b, mut c, mut d, mut e, mut f, mut g, mut hh] = h;
        for i in 0..64 {
            let t1 = hh
                .wrapping_add(e.rotate_right(6) ^ e.rotate_right(11) ^ e.rotate_right(25))
                .wrapping_add((e & f) ^ (!e & g))
                .wrapping_add(K[i])
                .wrapping_add(w[i]);
            let t2 = (a.rotate_right(2) ^ a.rotate_right(13) ^ a.rotate_right(22))
                .wrapping_add((a & b) ^ (a & c) ^ (b & c));
            hh = g;
            g = f;
            f = e;
            e = d.wrapping_add(t1);
            d = c;
            c = b;
            b = a;
            a = t1.wrapping_add(t2);
        }
        for (v, x) in h.iter_mut().zip([a, b, c, d, e, f, g, hh]) {
            *v = v.wrapping_add(x);
        }
    }
    h.iter().map(|v| format!("{v:08x}")).collect()
}

/// Options of one node (chart) generation.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct NodeOptions {
    pub order: NodeOrder,
    /// Chart mask in PRESENTED slots; `None` = the top chart of the presented family.
    pub chart: Option<String>,
    /// Initial cases in ORIGINAL slots (fixed values, linear equations); empty = the whole chart.
    pub initial: Vec<CaseSpec>,
    pub exact_backend: String,
    pub max_depth: Option<u32>,
    pub max_cases: Option<usize>,
    pub gsplit: bool,
    /// Fork-timed native guard extraction (seconds); on a timeout the exact content route (rr-5l-api) is used
    /// if `guard_content_fallback`, else the node stops with `GuardTimeout`.
    pub native_try: Option<u64>,
    #[serde(default = "yes")]
    pub guard_content_fallback: bool,
    /// Fork-timed admission of nonlinear exceptional branches (seconds); a timeout defers the region.
    pub admit_try: Option<u64>,
    pub screen: Option<LiftScreen>,
    /// Source visit orders tried, in turn, for a case whose lift the screen refused.
    pub retry_source_orders: Vec<SourceOrder>,
    /// Watchdog (supervised children only): after this many seconds on one case, write `F.spill.json` and
    /// terminate the PROCESS with exit status 9.
    pub case_budget_exit: Option<u64>,
    /// Earlier rules (FirstValid order) pruning queued cases: (table path, rules 0..upto).
    pub cover_tables: Vec<(String, usize)>,
    /// Explicit caller-declared terminals, not native master discovery. Empty ledgers are rejected.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub known_terminals: Option<KnownTerminalsFile>,
    pub cand_dump: bool,
    pub numeric_depth: u32,
    /// seed-rung (default None = off): a numeric endpoint whose lift ladder exhausts at `numeric_depth` is searched
    /// again at depths `numeric_depth + 1 ..= numeric_depth_max` (node source order) before it counts as exhausted
    /// (T241: a dotted numerator exceptional point had no relation at depth 2, one at depth 3; the bulk fallback
    /// left it as a false residual).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub numeric_depth_max: Option<u32>,
    /// Refuse exhausted per-point numeric searches instead of entering the unscreened shared numeric solve.
    /// The unfinished points stay explicit obligations; this does not classify any of them as masters.
    #[serde(default, skip_serializing_if = "is_false")]
    pub strict_numeric_search: bool,
    /// Finish symbolic work, then leave every undeclared numeric endpoint as an open obligation.
    /// No per-point search or shared numeric solve runs. Default off preserves the original pipeline.
    #[serde(default, skip_serializing_if = "is_false")]
    pub stop_before_numeric: bool,
    /// guard-fix: switch OFF the witness-pole guards (default ON = every lift's source-weight poles join the
    /// rule's exceptions, `GUARD_VERSION` 2). Off reproduces the old RHS-denominator-only guards: A/B only.
    #[serde(default, skip_serializing_if = "is_false")]
    pub no_witness_guards: bool,
    /// Seed schedule of the whole node (`SymbolicFirst` = p5l-lift-trace's in-shell order); default SpIRed.
    #[serde(default)]
    pub seed_schedule: SeedScheduleName,
    /// Optional finite shells in PRESENTED storage coordinates. None preserves
    /// the historical zero-first search. Independent of the in-shell order above.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub case_seed_schedule: Option<CaseSeedSchedule>,
    /// Stable symbolic ISP / symbolic physical / remaining priority, resolved
    /// against each actual Case in PRESENTED family coordinates. None is static.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub case_seed_priority: Option<CaseSeedPriority>,
    /// seed-pose probe mode (diagnostic, default None): a JSON-lines file of probes {label, case (ORIGINAL slots,
    /// CaseSpec), presented (bool, default false), schedule (CaseSeedSchedule or null = Reference), in_shell
    /// ("Spired" | "SymbolicFirst"), source_order (SourceOrder or null = the node's), max_depth}. Each probe runs the
    /// case search with a refuse-all lift screen: the selected relation (pivot, trace rows, seed shifts of the trace,
    /// RHS terms, coefficient degrees) is measured modularly and never lifted. Results go to `F.poses.jsonl`; no table.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pose_probe: Option<String>,
    /// Automatic SEED-POSE rung (seed-pose agent, default off): before the first search of a case, the default pose
    /// (Reference seeds, node in-shell order) is measured with a refuse-all modular screen; when its relation is
    /// heavier than `seed_pose_keep` (cost RHS x C(deg+v, v)) a small portfolio of seed poses ([`seed_pose_portfolio`])
    /// is measured the same way and the cheapest is searched for real. Order, presentation and lift ladder unchanged.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub seed_pose_rung: bool,
    /// Default-pose cost at or below which the rung keeps the default without probing alternatives.
    #[serde(default = "seed_pose_keep_default")]
    pub seed_pose_keep: f64,
    /// seed-rung (overhead cut): the rung no longer measures every case with a separate refuse-all search. The
    /// node's normal search runs with a seed-pose GATE on its screen ([`super::screen::LiftScreen::pose_gate`]):
    /// only a case whose default relation costs more than this triggers the portfolio (the default's cost is the
    /// gate's own measurement). `None` = the seed-pose agent's behaviour (measure every case first).
    #[serde(
        default = "seed_pose_trigger_default",
        skip_serializing_if = "Option::is_none"
    )]
    pub seed_pose_trigger: Option<f64>,
    /// Smallest trace (rows) the gate measures; smaller traces lift as without the rung.
    #[serde(default = "seed_pose_min_rows_default")]
    pub seed_pose_min_rows: usize,
    /// seed-fixed-dot (inside the rung only): after the portfolio, at most this many FIXED-DOT poses
    /// ([`fixed_dot_poses`]: one L1 shell 3 with one fixed line first, bounded seed count) are measured when no
    /// portfolio pose reached `seed_pose_keep`, led by one free-first shell-3 pose that gates them
    /// ([`FIXED_DOT_GATE_RHS`]). `Some(0)` = off (lead included) = the old rung; `Some(k >= 1)` = on with k line
    /// poses unless [`Self::seed_pose_fixed_dot_lines`] is set; `None` = on, lead only by default.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub seed_pose_fixed_dot: Option<usize>,
    /// Fixed-LINE poses after the lead (opt-in; `None` = [`FIXED_DOT_LINES_DEFAULT`] = 0).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub seed_pose_fixed_dot_lines: Option<usize>,
    /// Keep the fixed-dot poses on cases whose free lines carry a loop momentum (Y9 cyc >= 1; skipped by default).
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub seed_pose_fixed_dot_no_static_skip: bool,
    /// Seed budget of one fixed-dot pose (`None` = [`FIXED_DOT_SEEDS_DEFAULT`]).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub seed_pose_fixed_dot_seeds: Option<u32>,
    /// Measure the fixed-dot poses BEFORE the portfolio on cases with a dotted fixed line (value >= 2).
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub seed_pose_fixed_dot_first: bool,
    /// Lift ladder rung 1 (coordinator ruling 2026-11-11T04:17Z): a case whose lift the screen refused is searched
    /// again with the `SymbolicFirst` seed schedule (same presentation, same seed ball) before other source orders;
    /// the order portfolio (node level, [`super::auto`]) is the next rung.
    #[serde(default = "yes")]
    pub symbolic_first_rung: bool,
    /// Size budget per rule (coefficient monomials over all RHS terms, [`rule_monomials`]): a larger rule is treated
    /// as a refused lift (the ladder tries SymbolicFirst / other source orders, then the node's order portfolio).
    /// Coordinator 06:23Z: T211's composite closed with one 6.63M-monomial rule (2.88 GB table): correct but useless.
    #[serde(default)]
    pub max_rule_monomials: Option<usize>,
    /// Skip cases whose integer domain is empty (fixed values outside the chart's sign range, or linear equations
    /// without an integer solution in the chart's orthant): they need no rule (T211 lesson).
    #[serde(default = "yes")]
    pub empty_domain_check: bool,
    /// Size RULING 2026-11-11T09:53Z: a rule above `max_rule_monomials` (accept limit, 1M) but at most this many
    /// coefficient monomials (10M) is kept as a FALLBACK: the case's ladder tries at most `fallback_after` further
    /// variants for a smaller rule, then the smallest fallback is adopted and flagged oversized in the table and the
    /// outcome. Above this limit a rule is never adopted (it is still banked). `None` = no fallback.
    #[serde(default)]
    pub fallback_rule_monomials: Option<usize>,
    #[serde(default = "three")]
    pub fallback_after: usize,
    /// Side bank of refused candidates (`F.refused.jsonl` + `F.refused_dump.b<k>.json`): every lifted rule that the
    /// size gate refuses is exported WITH its guards, candidate dump and the refusal reason (the gate used to drop
    /// it entirely), screen refusals with their report; a later policy adopts banked rules without recomputation.
    #[serde(default = "yes")]
    pub refused_bank: bool,
    /// Banks of earlier runs (`F.refused.jsonl`, same presented family and order) whose rules are adopted for an
    /// identical case without a search when they satisfy the current policy (size <= `max_rule_monomials`, or <=
    /// `fallback_rule_monomials` after `fallback_after` variants were tried by the banking run).
    #[serde(default)]
    pub adopt_banks: Vec<String>,
    /// Union-aware coverage (Astra T211): a case is dropped when one earlier rule's case contains it and every piece
    /// of its exceptional branches inside the case is covered in turn (by any rule / cover / registry entry), up to
    /// this recursion depth; 0 = only single-rule coverage.
    #[serde(default = "two")]
    pub union_cover_depth: u32,
    /// Proof-bearing shared case registry of the parallel pieces of ONE node: (directory, piece name), see
    /// [`super::registry`]. Every accepted rule of this piece is appended to its own append-only epoch file; the
    /// sibling files with the same registry identity (math key = presented family, chart, order, full certificate;
    /// seed schedules; backend; engine version) are read incrementally. A popped case is DISCHARGED only by a
    /// valid-domain proof against the accepted rules (one rule whose case contains it and whose exceptional branches
    /// miss it, or the union argument); a case merely contained in a sibling rule's case is LEASED (postponed, see
    /// `registry_lease`), never discharged by containment. (Replaces the rr-consolidate registry whose pop pruning
    /// transferred ownership by containment: coordinator ruling 2026-11-11T17:37Z.)
    #[serde(default)]
    pub registry: Option<(String, String)>,
    /// Accepted for compatibility (the M5 batch 3 opt-in of the old registry); without effect.
    #[serde(default)]
    pub unsafe_shared_registry: bool,
    /// With a registry: postpone a case contained in a sibling's rule case until the queue (and the pool) is empty,
    /// wait up to `lease_wait` seconds for the siblings to end, then discharge it by proof or solve it here.
    #[serde(default = "yes", skip_serializing_if = "is_true")]
    pub registry_lease: bool,
    #[serde(default = "lease_wait", skip_serializing_if = "is_default_lease_wait")]
    pub lease_wait: u64,
    /// A case whose every relation is refused (or whose guard extraction times out) becomes an OPEN OBLIGATION of
    /// the run (listed in `F.obligations.json`) and the queue continues; default: the node stops there.
    #[serde(default, skip_serializing_if = "is_false")]
    pub open_on_refusal: bool,
    /// Initial cases in PRESENTED slots (as `F.obligations.json` / queue logs record them), added to `initial`.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub initial_presented: Vec<CaseSpec>,
    /// Obligation pool directory (`POOL.json` + `claims/`): when the queue is empty, the next unclaimed item is
    /// claimed atomically and becomes an initial case of this run (dynamic work distribution of a split node).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pool: Option<String>,
    /// Lift ladder rung after SymbolicFirst: the same case searched with these finite seed shells (ShiftShells; Astra
    /// T162: depth-2 shells select far smaller relations than the mandatory depth-0/1 seeds).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub case_seed_rung: Option<CaseSeedSchedule>,
    /// Drop the RHS and the trace sources of a symbolic rule once it is written (memory of long split pieces: the
    /// queue reads only case and exceptional branches of earlier rules).
    #[serde(default, skip_serializing_if = "is_false")]
    pub slim_rules: bool,
    /// Region-restricted run (t266-regions): the conjunction (PRESENTED `n<i>`, `= 0`) of ONE deferred region whose
    /// chart is the run's initial case. The modular discovery point of every case search is placed on the region
    /// (solver::region_probe), so the first-hit search selects a pivot that does not vanish there, and every
    /// exceptional branch is intersected together with the region (children stay inside it). The run then owns that
    /// region only (`F.obligations.json` `region`), not its whole chart.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub region: Option<Vec<String>>,
    /// sym-gen (2026-11-12, default off; notes/sym-gen.md): the chart's authenticated automorphism group
    /// ([`super::symmetry`]) closes every queued case (symbolic or numeric endpoint) whose keys it maps strictly
    /// lower in the node order by ONE symmetry record `I(a) = s I(g a)` (`"type":"sym"`, a descent step with unit
    /// coefficient and no exceptions), and every case it proves zero (`g a = a`, sign -1) by a `"type":"sym_zero"`
    /// record; such cases are never searched. The group goes into a `"type":"symmetry_group"` record.
    #[serde(default, skip_serializing_if = "is_false")]
    pub symmetry: bool,
    /// sym-gen step 2 (2026-11-12, default off; notes/sym-gen.md): EXACT one-term symmetry rows `I(a) - s I(g a)`
    /// of the chart's authenticated group, added at every seed of the shared numeric bulk solve next to its IBP rows
    /// (same modular reducer, same exact union replay). The exhausted endpoints then end on masters modulo IBP and
    /// symmetry (within the seed depth). Rules whose trace uses such rows carry `"sym_rows":K`; the group goes into a
    /// `"type":"symmetry_group"` record. Independent of `symmetry` (no records, no forms needed).
    #[serde(default, skip_serializing_if = "is_false")]
    pub symmetry_rows: bool,
    /// sym-7master (2026-11-13, default off; notes/sym-7master.md): exact one-term symmetry rows also in the
    /// case search (symbolic cases and the per-point search of numeric endpoints), for the elements that fix every
    /// free slot of the case pointwise
    /// ([`crate::solver::SectorConfig::case_symmetry`]). The rule field `sym_rows` and the `symmetry_group` record
    /// are those of `symmetry_rows` (the CLI flag `--symmetry-symbolic` turns both on).
    #[serde(default, skip_serializing_if = "is_false")]
    pub symmetry_symbolic: bool,
    /// seed-centre (2026-11-13, default None = unchanged): build the seed shells of EVERY case around the case target
    /// plus this offset (PRESENTED slots, length = family arity); target, comparator and pivot test unchanged
    /// ([`crate::solver::SeedRecipe`]). CLI `--seed-centre-offset <slot>:<delta>[,...]`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub seed_centre_offset: Option<Vec<i64>>,
    /// seed-centre: further global centres (the seed set is the union of the shells around every centre; CLI
    /// `--seed-centre-offset 'A;B;..'`, centres separated by ';').
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub seed_centre_extra: Vec<Vec<i64>>,
    /// seed-centre: per-case centre offsets from `--initial-presented` (field `seed_centre_offset` of a case); a case
    /// contained in such an initial case (its exceptional branches included) uses that offset instead of the global one.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub case_seed_centres: Vec<CaseSeedCentre>,
    /// seed-centre: drop every seed whose power on a face line (a sector line symbolic, or numeric at >= 2, in the
    /// case) is below target - k. CLI `--seed-face-floor <k>`; default None = no filter.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub seed_face_floor: Option<u16>,
    /// t266-tight seed layer cap `[D, R, B]` as given by `--seed-layer-cap` (engine line: recorded here so that the
    /// table header's node_options, the queue's seed_recipe record, OUTCOME and every case-search rule's
    /// `seed_setting` carry it; the filter itself is the process-global `solver::SeedLayerCap`). Default None.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub seed_layer_cap: Option<[i32; 3]>,
}

/// seed-centre: an initial case (PRESENTED slots) with its own seed-centre offset.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct CaseSeedCentre {
    pub case: CaseSpec,
    pub seed_centre_offset: Vec<i64>,
    /// Further centres of this case (union of shells).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub extra: Vec<Vec<i64>>,
}

/// seed-centre: parse `A;B;..` (each `<slot>:<delta>[,...]`) into one offset vector per centre.
pub fn parse_seed_centre_offsets(text: &str, arity: usize) -> Result<Vec<Vec<i64>>, String> {
    let out: Vec<Vec<i64>> = text
        .split(';')
        .map(str::trim)
        .filter(|c| !c.is_empty())
        .map(|c| parse_seed_centre_offset(c, arity))
        .collect::<Result<_, _>>()?;
    if out.is_empty() {
        return Err("no centre".into());
    }
    Ok(out)
}

/// seed-centre: parse `<slot>:<delta>[,...]` into an offset vector of length `arity`.
pub fn parse_seed_centre_offset(text: &str, arity: usize) -> Result<Vec<i64>, String> {
    let mut out = vec![0i64; arity];
    for part in text.split(',').map(str::trim).filter(|p| !p.is_empty()) {
        let (axis, delta) = part
            .split_once(':')
            .ok_or_else(|| format!("'{part}': expected <slot>:<delta>"))?;
        let axis: usize = axis
            .trim()
            .parse()
            .map_err(|e| format!("slot '{axis}': {e}"))?;
        let delta: i64 = delta
            .trim()
            .parse()
            .map_err(|e| format!("delta '{delta}': {e}"))?;
        if axis >= arity {
            return Err(format!("slot {axis} >= arity {arity}"));
        }
        if !(-8..=8).contains(&delta) {
            return Err(format!("delta {delta} outside -8..=8"));
        }
        out[axis] += delta;
    }
    Ok(out)
}

/// seed-centre: the recipe of one case (None = unchanged search): the offset of the first per-case entry whose case
/// contains `case`, else the global offset; the floor is global.
fn seed_recipe_for<const N: usize>(
    opts: &NodeOptions,
    centres: &[(Case<N>, Vec<Vec<i64>>)],
    case: &Case<N>,
) -> Option<crate::solver::SeedRecipe> {
    let offsets: Vec<Vec<i64>> = centres
        .iter()
        .find(|(c, _)| c.contains(case).unwrap_or(false))
        .map(|(_, o)| o.clone())
        .or_else(|| {
            opts.seed_centre_offset.as_ref().map(|first| {
                std::iter::once(first.clone())
                    .chain(opts.seed_centre_extra.iter().cloned())
                    .collect()
            })
        })
        .unwrap_or_default();
    let conv =
        |o: &Vec<i64>| -> Vec<i16> { o.iter().map(|&d| i16::try_from(d).unwrap_or(0)).collect() };
    let recipe = crate::solver::SeedRecipe {
        centre_offset: offsets.first().map(conv).unwrap_or_default(),
        extra_centres: offsets.iter().skip(1).map(conv).collect(),
        face_floor: opts.seed_face_floor,
    };
    (!recipe.is_default()).then_some(recipe)
}

/// seed-centre: the per-rule record of the seed setting of `case` (None = default seeds).
fn seed_setting_json<const N: usize>(
    opts: &NodeOptions,
    centres: &[(Case<N>, Vec<Vec<i64>>)],
    case: &Case<N>,
) -> Option<String> {
    let r = seed_recipe_for(opts, centres, case);
    if r.is_none() && opts.seed_layer_cap.is_none() {
        return None;
    }
    let all: Vec<&[i16]> = r.as_ref().map(|r| r.offsets()).unwrap_or_default();
    let mut v = serde_json::json!({"centres": all, "face_floor": r.as_ref().and_then(|r| r.face_floor), "max_depth": opts.max_depth});
    // engine line: the layer cap only when set (the seed-centre form is unchanged without it)
    if let Some(cap) = opts.seed_layer_cap {
        v["layer_cap"] = serde_json::json!(cap);
    }
    Some(v.to_string())
}

fn lease_wait() -> u64 {
    1800
}

// engine line: the registry-line options are serialised only when set, so that a default run's table header
// (node_options) is byte-identical to batch 6's
fn is_default_lease_wait(value: &u64) -> bool {
    *value == lease_wait()
}

fn is_true(value: &bool) -> bool {
    *value
}

fn is_zero(value: &usize) -> bool {
    *value == 0
}

fn seed_pose_keep_default() -> f64 {
    5_000.0
}

/// seed-rung 2026-11-12: default trigger 5e3 (was 5e4) = the regen-flagged queue's recipe value. [M] 5e3 is the only
/// trigger with node-level closure evidence (T205 R1 order: complete, largest 26k, where 1e6 never fires and the node
/// keeps its open region; T267/T241 shipped order, T161/T273 queue attempts complete); its probe overhead is 1.05x
/// (T266 pieces) to 1.7x (T205) of the rung-off wall, 40-61 % of the wall in probes on dotted-corner nodes. 1e6
/// (T266 registry pieces: 2.06x faster) stays an explicit `--seed-pose-trigger` for time-bound piece waves.
fn seed_pose_trigger_default() -> Option<f64> {
    Some(5_000.0)
}

fn seed_pose_min_rows_default() -> usize {
    8
}

fn is_false(value: &bool) -> bool {
    !value
}

fn three() -> usize {
    3
}

fn two() -> u32 {
    2
}

/// Serializable name of a [`crate::solver::SeedSchedule`].
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum SeedScheduleName {
    #[default]
    Spired,
    SymbolicFirst,
    DotsFirst,
}

impl SeedScheduleName {
    pub fn schedule(self) -> crate::solver::SeedSchedule {
        match self {
            Self::Spired => crate::solver::SeedSchedule::Spired,
            Self::SymbolicFirst => crate::solver::SeedSchedule::SymbolicFirst,
            Self::DotsFirst => crate::solver::SeedSchedule::DotsFirst,
        }
    }
}

fn yes() -> bool {
    true
}

impl NodeOptions {
    /// guard-fix D1: the guard semantics this run writes and requires of every reused rule (cover tables, cover
    /// indexes, banks, registry siblings, joins): [`GUARD_VERSION`], or 1 with `--no-witness-guards`.
    pub fn guard_version(&self) -> u32 {
        if self.no_witness_guards {
            1
        } else {
            GUARD_VERSION
        }
    }
}

impl Default for NodeOptions {
    fn default() -> Self {
        Self {
            order: NodeOrder::identity(),
            chart: None,
            initial: Vec::new(),
            exact_backend: "targetonlyfactorized".into(),
            max_depth: None,
            max_cases: None,
            gsplit: true,
            native_try: Some(60),
            guard_content_fallback: true,
            admit_try: Some(120),
            screen: None,
            retry_source_orders: vec![SourceOrder::Reverse],
            case_budget_exit: None,
            cover_tables: Vec::new(),
            known_terminals: None,
            cand_dump: false,
            numeric_depth: 2,
            numeric_depth_max: None,
            strict_numeric_search: false,
            stop_before_numeric: false,
            no_witness_guards: false,
            seed_schedule: SeedScheduleName::Spired,
            case_seed_schedule: None,
            case_seed_priority: None,
            pose_probe: None,
            seed_pose_rung: false,
            seed_pose_keep: seed_pose_keep_default(),
            seed_pose_trigger: seed_pose_trigger_default(),
            seed_pose_min_rows: seed_pose_min_rows_default(),
            seed_pose_fixed_dot: None,
            seed_pose_fixed_dot_lines: None,
            seed_pose_fixed_dot_no_static_skip: false,
            seed_pose_fixed_dot_seeds: None,
            seed_pose_fixed_dot_first: false,
            symbolic_first_rung: true,
            empty_domain_check: true,
            max_rule_monomials: None,
            fallback_rule_monomials: None,
            fallback_after: 3,
            refused_bank: true,
            adopt_banks: Vec::new(),
            union_cover_depth: 2,
            registry: None,
            unsafe_shared_registry: false,
            registry_lease: true,
            lease_wait: 1800,
            open_on_refusal: false,
            initial_presented: Vec::new(),
            pool: None,
            case_seed_rung: None,
            slim_rules: false,
            region: None,
            symmetry: false,
            symmetry_rows: false,
            symmetry_symbolic: false,
            seed_centre_offset: None,
            seed_centre_extra: Vec::new(),
            case_seed_centres: Vec::new(),
            seed_face_floor: None,
            seed_layer_cap: None,
        }
    }
}

/// A case: fixed values (`None` = free) and linear integer equations in `n<i>` (`= 0`).
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct CaseSpec {
    pub fixed: Vec<Option<i64>>,
    #[serde(default)]
    pub equations: Vec<String>,
}

impl CaseSpec {
    /// `1,1,_,0 n0+n2-1;n3` (the face-queue `initial` syntax).
    pub fn parse(spec: &str, equations: &str) -> Result<Self, String> {
        let fixed = spec
            .split(',')
            .map(|v| {
                let v = v.trim();
                if v == "_" {
                    Ok(None)
                } else {
                    v.parse()
                        .map(Some)
                        .map_err(|e| format!("case value {v}: {e}"))
                }
            })
            .collect::<Result<_, String>>()?;
        let equations = equations
            .split(';')
            .map(str::trim)
            .filter(|e| !e.is_empty())
            .map(str::to_string)
            .collect();
        Ok(Self { fixed, equations })
    }
}

/// Why a node stopped before completing its chart.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum StopReason {
    /// Every tried relation for this case (PRESENTED slots) was refused by the lift pre-screen.
    LiftRefused {
        case: CaseSpec,
        reports: Vec<ScreenReport>,
    },
    /// The native guard extraction did not finish within `native_try` seconds.
    GuardTimeout {
        case: CaseSpec,
        rhs: usize,
    },
    MaxCases {
        pending: usize,
    },
    /// Every case closed, but deferred exceptional regions (or failed branch admissions) remain that no rule of
    /// the node covers: they are open obligations (repair R3 / a piece run on their charts), not a closure.
    OpenRegions {
        regions: usize,
        covered: usize,
        charts: Vec<CaseSpec>,
    },
    /// The caller requested a stop before any numeric search. Endpoints remain open, not masters.
    BeforeNumeric {
        pending: usize,
    },
    /// The queue closed, but cases left open by `open_on_refusal` remain (listed in `F.obligations.json`).
    OpenObligations {
        refused: usize,
    },
    /// Per-point numeric searches exhausted or were screened out, and strict mode forbids the shared fallback.
    /// Cases are in PRESENTED slots and remain open, including any possible genuine masters.
    NumericSearchRefused {
        cases: Vec<CaseSpec>,
    },
    Error(String),
    /// child-job (job mode only): a stop request (signal, STOP file, soft memory limit) honoured at a safe point;
    /// `pending` counts the open cases including the interrupted one (RESUME.json lists them).
    Interrupted {
        source: String,
        pending: usize,
    },
    /// child-job `--descend open` (job mode only): the job's own cases are done; `returned` sub-cases, numeric
    /// endpoints and regions were returned to the supervisor (F.subcases.jsonl) instead of being searched.
    Descended {
        returned: usize,
    },
}

/// Result of [`generate`]. `complete` = the chart closed (no stop, no error); `state` says it in words
/// (`complete` / `stopped`) and `open_obligations` counts what `F.obligations.json` lists.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct NodeOutcome {
    pub complete: bool,
    /// seed-centre: the run's seed options (global centres, per-case centres, floor, max depth); absent by default.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub seed_recipe: Option<serde_json::Value>,
    #[serde(default)]
    pub state: String,
    #[serde(default)]
    pub open_obligations: usize,
    /// Math key of the run ([`super::registry::math_key`]).
    #[serde(default)]
    pub math_key: String,
    /// Registry leases taken / resolved by proof / returned to the own queue; pool items claimed.
    #[serde(default)]
    pub leases: usize,
    #[serde(default)]
    pub leases_resolved: usize,
    #[serde(default)]
    pub leases_returned: usize,
    #[serde(default)]
    pub pool_claims: usize,
    /// Cases left open by `open_on_refusal`.
    #[serde(default)]
    pub refused_open: usize,
    pub rules: usize,
    pub rhs_terms: usize,
    pub residuals: usize,
    pub residual_keys: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub terminal_ledger: Option<DeclaredTerminalLedger>,
    pub deferred_regions: usize,
    pub deferred_branches: usize,
    pub symbolic_cases: usize,
    pub numerical_cases: usize,
    pub numeric_bulk_fallback: usize,
    pub screened_lifts: usize,
    pub refused_lifts: usize,
    pub source_order_retries: usize,
    /// Refused lifts retried with the SymbolicFirst seed schedule (ladder rung 1).
    #[serde(default)]
    pub symbolic_first_retries: usize,
    /// Cases skipped because their integer domain is empty.
    #[serde(default)]
    pub empty_domains: usize,
    /// Lifted rules refused by the size budget (each is banked).
    #[serde(default)]
    pub oversized_rules: usize,
    /// Oversized rules adopted as fallbacks (ruling 1M-10M), flagged in the table (`oversized_fallback`).
    #[serde(default)]
    pub oversized_adopted: Vec<OversizedRule>,
    /// Entries written to the side bank `F.refused.jsonl` (refused rules + screen refusals).
    #[serde(default)]
    pub banked: usize,
    /// Rules adopted from earlier banks (`adopt_banks`) without a search.
    #[serde(default)]
    pub bank_adopted: usize,
    /// Cases dropped by union-aware coverage (not covered by any single rule).
    #[serde(default)]
    pub union_covered: usize,
    /// Popped cases discharged by a valid-domain proof against sibling pieces' accepted rules (shared registry).
    #[serde(default)]
    pub registry_covered: usize,
    /// Largest coefficient-monomial count of a rule in the table.
    #[serde(default)]
    pub max_rule_monomials: usize,
    /// Deferred regions / failed branch admissions whose chart no rule (single or union) covers.
    #[serde(default)]
    pub open_deferred: usize,
    /// Cases with a finite integer domain, replaced by their uncovered points (numeric endpoints).
    #[serde(default)]
    pub finite_domains: usize,
    #[serde(default)]
    pub content_routes: usize,
    /// seed-rung: numeric endpoints on which the seed-pose rung measured its portfolio (written only when > 0).
    #[serde(default, skip_serializing_if = "is_zero")]
    pub numeric_seed_pose: usize,
    /// seed-rung: numeric endpoints closed by a deeper search (`numeric_depth_max`; written only when > 0).
    #[serde(default, skip_serializing_if = "is_zero")]
    pub numeric_deeper: usize,
    /// sym-gen: cases closed by symmetry records (symbolic / numeric endpoints) and zero-by-symmetry records.
    #[serde(default, skip_serializing_if = "is_zero")]
    pub sym_rules: usize,
    #[serde(default, skip_serializing_if = "is_zero")]
    pub sym_numeric: usize,
    #[serde(default, skip_serializing_if = "is_zero")]
    pub sym_zeros: usize,
    /// sym-gen step 2: symmetry rows generated / independent in the shared bulk solve, and its rules using them.
    #[serde(default, skip_serializing_if = "is_zero")]
    pub sym_rows: usize,
    #[serde(default, skip_serializing_if = "is_zero")]
    pub sym_rows_independent: usize,
    #[serde(default, skip_serializing_if = "is_zero")]
    pub sym_row_rules: usize,
    pub not_lower: Vec<String>,
    pub stop: Option<StopReason>,
    pub seconds: f64,
    pub vmhwm_kb: u64,
    pub table: String,
    /// First symbolic rule (probe score): trace rows, lift seconds, RHS terms, max coefficient total degree.
    pub first_rule: Option<RuleScore>,
    /// Scores of every symbolic rule, in queue order (probe portfolios).
    #[serde(default)]
    pub rule_scores: Vec<RuleScore>,
}

/// An oversized rule adopted as a fallback (closure report).
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct OversizedRule {
    pub index: usize,
    pub monomials: usize,
    pub rhs_terms: usize,
    pub variant: String,
    pub variants_tried: usize,
    pub case: CaseSpec,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct RuleScore {
    pub trace_rows: usize,
    pub lift_seconds: f64,
    pub search_seconds: f64,
    pub rhs_terms: usize,
    pub max_degree: usize,
    pub direct_hit: bool,
    pub branches: usize,
    /// Coefficient monomials of the rule ([`rule_monomials`]).
    #[serde(default)]
    pub monomials: usize,
}

/// Progress events of [`generate`] (also written to the queue log).
#[derive(Clone, Debug, Serialize)]
pub enum NodeEvent {
    Prepared {
        basis: usize,
        seconds: f64,
    },
    CaseStarted {
        case: CaseSpec,
        pending: usize,
    },
    LiftScreened {
        report: ScreenReport,
    },
    RelationRetry {
        case: CaseSpec,
        source_order: String,
    },
    RuleFound {
        index: usize,
        rhs: usize,
        trace_rows: usize,
        lift_seconds: f64,
        branches: usize,
    },
    NumericalPoint {
        fixed: Vec<i64>,
        outcome: String,
    },
    Stopped {
        reason: String,
    },
}

pub(crate) fn q(s: &str) -> String {
    serde_json::to_string(s).expect("json string")
}

fn strings<T: std::fmt::Display>(items: impl IntoIterator<Item = T>) -> String {
    let parts: Vec<String> = items.into_iter().map(|x| q(&x.to_string())).collect();
    format!("[{}]", parts.join(","))
}

fn mask_string(bits: &[bool]) -> String {
    bits.iter().map(|&b| if b { '1' } else { '0' }).collect()
}

pub fn vmhwm_kb() -> u64 {
    std::fs::read_to_string("/proc/self/status")
        .ok()
        .and_then(|s| {
            s.lines()
                .find(|l| l.starts_with("VmHWM"))
                .and_then(|l| l.split_whitespace().nth(1)?.parse().ok())
        })
        .unwrap_or(0)
}

/// Decode the complete existing native schedule, refusing invalid shells or
/// an incomplete coordinate permutation before any search work.
pub fn parse_case_seed_schedule(text: &str, arity: usize) -> Result<CaseSeedSchedule, String> {
    let schedule: CaseSeedSchedule = serde_json::from_str(text).map_err(|e| e.to_string())?;
    schedule.validate(arity).map_err(|e| e.to_string())?;
    Ok(schedule)
}

/// The schedule of one current case under `priority`; `None` = the default seed path (only
/// [`CaseSeedPriority::AffineCoupledFirst`] on a case without equations).
fn resolve_case_seed_schedule<const N: usize>(
    priority: CaseSeedPriority,
    template: Option<&CaseSeedSchedule>,
    case: &Case<N>,
    physical: &[bool; N],
) -> Result<Option<CaseSeedSchedule>, SolverError> {
    if priority == CaseSeedPriority::AffineCoupledFirst {
        priority
            .validate_template(template, N)
            .map_err(SolverError::InvalidInput)?;
        let Some(affine) = case.affine() else {
            return Ok(None);
        };
        let Some(CaseSeedSchedule::ShiftShells {
            first_depth,
            last_depth,
            axis_priority,
            positive_first,
        }) = template
        else {
            return Err(SolverError::InvalidInput(
                "case seed priority requires ShiftShells".into(),
            ));
        };
        let coupled = |axis: usize| {
            case.fixed()[axis].is_none() && {
                let mut unit = [0i16; N];
                unit[axis] = 1;
                !affine.is_tangent(&unit)
            }
        };
        let axis_priority: Vec<usize> = axis_priority
            .iter()
            .copied()
            .filter(|&a| coupled(a))
            .chain(axis_priority.iter().copied().filter(|&a| !coupled(a)))
            .collect();
        let resolved = CaseSeedSchedule::ShiftShells {
            first_depth: *first_depth,
            last_depth: *last_depth,
            axis_priority,
            positive_first: *positive_first,
        };
        resolved.validate(N)?;
        return Ok(Some(resolved));
    }
    resolve_symbolic_isp_first(priority, template, case, physical).map(Some)
}

fn resolve_symbolic_isp_first<const N: usize>(
    priority: CaseSeedPriority,
    template: Option<&CaseSeedSchedule>,
    case: &Case<N>,
    physical: &[bool; N],
) -> Result<CaseSeedSchedule, SolverError> {
    priority
        .validate_template(template, N)
        .map_err(SolverError::InvalidInput)?;
    let Some(CaseSeedSchedule::ShiftShells {
        first_depth,
        last_depth,
        axis_priority,
        positive_first,
    }) = template
    else {
        return Err(SolverError::InvalidInput(
            "case seed priority requires ShiftShells".into(),
        ));
    };
    let group = |axis: usize| match (case.fixed()[axis].is_none(), physical[axis]) {
        (true, false) => 0,
        (true, true) => 1,
        _ => 2,
    };
    let axis_priority = (0..3)
        .flat_map(|g| {
            axis_priority
                .iter()
                .copied()
                .filter(move |&axis| group(axis) == g)
        })
        .collect();
    let resolved = CaseSeedSchedule::ShiftShells {
        first_depth: *first_depth,
        last_depth: *last_depth,
        axis_priority,
        positive_first: *positive_first,
    };
    resolved.validate(N)?;
    Ok(resolved)
}

/// Record the same pure resolution outside the search closure so qlog is not
/// captured by it. Static/default paths emit no additional bytes.
fn log_case_seed_schedule<const N: usize>(
    out: &mut impl std::io::Write,
    opts: &NodeOptions,
    case: &Case<N>,
    physical: &[bool; N],
    n: usize,
) -> Result<(), String> {
    let Some(priority) = opts.case_seed_priority else {
        return Ok(());
    };
    let Some(resolved) =
        resolve_case_seed_schedule(priority, opts.case_seed_schedule.as_ref(), case, physical)
            .map_err(|e| e.to_string())?
    else {
        return Ok(());
    };
    writeln!(
        out,
        "{{\"type\":\"case_seed_schedule_resolved\",\"frame\":\"presented\",\"case\":{},\"policy\":{},\"physical_mask\":{},\"schedule\":{}}}",
        case_json(case, n),
        serde_json::to_string(&priority).map_err(|e| e.to_string())?,
        q(&mask_string(&physical[..n])),
        serde_json::to_string(&resolved).map_err(|e| e.to_string())?,
    ).map_err(|e| e.to_string())?;
    out.flush().map_err(|e| e.to_string())
}

pub fn parse_backend(name: &str) -> Result<SymbolicExactBackend, String> {
    Ok(match name {
        "sparse" => SymbolicExactBackend::Sparse,
        "factorized" => SymbolicExactBackend::SparseFactorized,
        "targetonly" => SymbolicExactBackend::SparseTargetOnly,
        "targetonlyfactorized" => SymbolicExactBackend::SparseTargetOnlyFactorized,
        #[cfg(feature = "reconstruction")]
        "sampled" => SymbolicExactBackend::Sampled {
            threads: 1,
            seed: 0x5eed,
            min_rows: 600,
        },
        #[cfg(feature = "reconstruction")]
        "sampled-strict" => SymbolicExactBackend::SampledStrict {
            threads: 1,
            seed: 0x5eed,
        },
        other => return Err(format!("unknown exact backend {other}")),
    })
}

/// Exact backend of the shared numeric solve for a symbolic backend: the factorized variants keep factorized
/// coefficients, the others (and the sampled reconstruction, which has no shared-numeric form) use Sparse.
pub fn numerical_backend(b: SymbolicExactBackend) -> crate::solver::NumericalExactBackend {
    match b {
        SymbolicExactBackend::SparseFactorized
        | SymbolicExactBackend::SparseTargetOnlyFactorized => {
            crate::solver::NumericalExactBackend::SparseFactorized
        }
        _ => crate::solver::NumericalExactBackend::Sparse,
    }
}

/// Compile a compact family text.
pub fn compile_family(text: &str) -> Result<IntegralFamily, String> {
    Ok(Compiler::new(Limits::default())
        .map_err(|e| format!("compiler: {e:?}"))?
        .compile_compact(text, None)
        .map_err(|e| format!("compile family: {e:?}"))?
        .into_lowered(LoweringLimits::default())
        .map_err(|e| format!("lower family: {e:?}"))?
        .into_family())
}

/// THE list of arities `generate` is compiled for (audit-engjob m9: one source for its dispatch below and for what
/// `rr_node version --json` reports); `$m!` is applied to the list.
macro_rules! generate_arities {
    ($m:ident) => {
        $m!(5, 9, 12, 14, 20)
    };
}

/// The arities of [`generate`] (from `generate_arities!`).
pub const GENERATE_ARITIES: &[usize] = {
    macro_rules! list {
        ($($a:literal),*) => {
            &[$($a),*]
        };
    }
    generate_arities!(list)
};

/// Generate one chart of `family_text` (ORIGINAL presentation) under `opts.order` into `out_dir`
/// (`F.family.sym` = presented family, `F.table.jsonl`, `F.queue.jsonl`, `F.cert.jsonl`, optional candidate dumps).
pub fn generate(
    family_text: &str,
    family_path: &str,
    out_dir: &Path,
    opts: &NodeOptions,
    events: &mut dyn FnMut(&NodeEvent),
) -> Result<NodeOutcome, String> {
    if opts.known_terminals.is_some() && opts.registry.is_some() {
        return Err("known terminals: the legacy shared registry lacks actual target authority; ledger + registry is unsupported".into());
    }
    let presented = opts.order.present_family(family_text)?;
    let fam = compile_family(&presented)?;
    std::fs::create_dir_all(out_dir).map_err(|e| format!("{}: {e}", out_dir.display()))?;
    std::fs::write(out_dir.join("F.family.sym"), &presented)
        .map_err(|e| format!("family copy: {e}"))?;
    let n = fam.denominator_count();
    opts.order.validate(n)?;
    macro_rules! dispatch {
        ($($a:literal),*) => {
            match n {
                $($a => run::<$a>(family_text, family_path, &presented, &fam, out_dir, opts, events),)*
                other => Err(format!(
                    "arity {other}: node driver compiled for {}",
                    GENERATE_ARITIES.iter().map(|a| a.to_string()).collect::<Vec<_>>().join(", ")
                )),
            }
        };
    }
    generate_arities!(dispatch)
}

/// The node order's [`SectorConfig`] pieces: legacy order, or the compiled tie-priority program.
pub(super) fn order_program(
    order: &NodeOrder,
    n: usize,
) -> Result<Option<rustred_order::CompiledOrder>, String> {
    match &order.coordinate_priority {
        None => Ok(None),
        Some(priority) => {
            let mut d = rustred_order::OrderDescriptor::spired_uncut(n);
            d.coordinate_priority = priority.clone();
            // batch 6 (helper Z12): preconditions of the descent argument, asserted at generation time
            if !d.pre_support_degree_rows.is_empty()
                || d.coordinate_groups != rustred_order::CoordinateGroups::ActiveFirst
            {
                return Err(
                    "order program: the descent argument needs no pre-support degree rows and ActiveFirst coordinate groups"
                        .into(),
                );
            }
            rustred_order::CompiledOrder::compile(d, Default::default())
                .map(Some)
                .map_err(|e| format!("order program: {e:?}"))
        }
    }
}

fn descriptor_json(order: &NodeOrder, n: usize) -> String {
    let mut d = rustred_order::OrderDescriptor::spired_uncut(n);
    if let Some(p) = &order.coordinate_priority {
        d.coordinate_priority = p.clone();
    }
    format!(
        "{{\"support_priority\":{:?},\"coordinate_priority\":{:?},\"two_sided\":{:?}}}",
        d.support_priority, d.coordinate_priority, d.two_sided
    )
}

pub(crate) fn case_spec<const N: usize>(c: &Case<N>, n: usize) -> CaseSpec {
    let fixed = c.fixed()[..n].iter().map(|v| v.map(i64::from)).collect();
    let equations = match c {
        Case::Coordinate(_) => Vec::new(),
        Case::Affine(a) => a.equations().iter().map(|e| e.to_string()).collect(),
    };
    CaseSpec { fixed, equations }
}

/// `RUSTRED_NODE_PROGRESS=1`: print the search's discovery milestones on stderr (diagnostics, default off; t178).
fn discovery_progress_log() -> bool {
    static ON: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *ON.get_or_init(|| std::env::var("RUSTRED_NODE_PROGRESS").as_deref() == Ok("1"))
}

fn case_json<const N: usize>(c: &Case<N>, n: usize) -> String {
    serde_json::to_string(&case_spec(c, n)).expect("case json")
}

/// Linear integer expression in `n<i>` (`3*n1-n2+4`) as a coefficient polynomial of the family's index variables.
pub(super) fn parse_index_poly(
    expr: &str,
    template: &CoefficientPolynomial,
    idx: &[usize],
) -> Option<CoefficientPolynomial> {
    let vars = template.variables();
    let s: String = expr.chars().filter(|c| !c.is_whitespace()).collect();
    if s.is_empty() {
        return None;
    }
    let mut acc = template.zero();
    let mut monos: Vec<(bool, String)> = Vec::new();
    let mut cur = String::new();
    let mut neg = false;
    for (k, ch) in s.chars().enumerate() {
        if (ch == '+' || ch == '-') && k > 0 && !cur.ends_with('^') {
            monos.push((neg, std::mem::take(&mut cur)));
            neg = ch == '-';
        } else if (ch == '+' || ch == '-') && k == 0 {
            neg = ch == '-';
        } else {
            cur.push(ch);
        }
    }
    monos.push((neg, cur));
    for (neg, body) in monos {
        let mut c = symbolica::domains::integer::Integer::one();
        let mut m = template.one();
        for f in body.split('*') {
            if f.chars().all(|x| x.is_ascii_digit()) && !f.is_empty() {
                // arbitrary precision: deferred-region polynomials carry coefficients beyond i64 (t266-regions)
                c = &c * &f.parse::<symbolica::domains::integer::Integer>().ok()?;
            } else {
                let (name, e) = match f.split_once('^') {
                    Some((a, b)) => (a, b.parse::<u32>().ok()?),
                    None => (f, 1),
                };
                let i: usize = name.strip_prefix('n')?.parse().ok()?;
                let v = template.variable(&vars[*idx.get(i)?]).ok()?;
                for _ in 0..e {
                    m = &m * &v;
                }
            }
        }
        if neg {
            c = -c;
        }
        m = m.mul_coeff(c);
        acc = &acc + &m;
    }
    Some(acc)
}

pub(super) fn build_case<const N: usize>(
    spec: &CaseSpec,
    template: &CoefficientPolynomial,
    idx: &[usize; N],
    sector: &[bool; N],
    n: usize,
) -> Result<Option<Case<N>>, String> {
    if spec.fixed.len() != n {
        return Err(format!("case arity {} != {n}", spec.fixed.len()));
    }
    let mut fixed = [Some(0i16); N];
    for (i, v) in spec.fixed.iter().enumerate() {
        fixed[i] = v
            .map(|x| i16::try_from(x).map_err(|_| format!("case value {x}")))
            .transpose()?;
    }
    let face = Case::from(CoordinateCase::new(fixed).map_err(|e| format!("case: {e:?}"))?);
    let polys: Vec<CoefficientPolynomial> = spec
        .equations
        .iter()
        .map(|e| parse_index_poly(e, template, idx).ok_or_else(|| format!("equation {e}")))
        .collect::<Result<_, _>>()?;
    face.intersect(&polys, idx, sector)
        .map_err(|e| format!("admit case: {e:?}"))
}

fn max_total_degree(p: &CoefficientPolynomial) -> usize {
    (0..p.nterms())
        .map(|i| p.exponents(i).iter().map(|e| *e as usize).sum::<usize>())
        .max()
        .unwrap_or(0)
}

/// Max total degree over numerators and denominators of a rule's RHS coefficients.
/// Size of a rule: coefficient monomials summed over its RHS terms (numerator + denominator).
pub fn rule_monomials<const N: usize>(c: &RuleCandidate<N>) -> usize {
    c.rhs
        .iter()
        .map(|t| t.coefficient.numerator.nterms() + t.coefficient.denominator.nterms())
        .sum()
}

pub fn rule_degree<const N: usize>(c: &RuleCandidate<N>) -> usize {
    c.rhs
        .iter()
        .map(|t| {
            max_total_degree(&t.coefficient.numerator)
                .max(max_total_degree(&t.coefficient.denominator))
        })
        .max()
        .unwrap_or(0)
}

fn power_byte(p: crate::solver::Power) -> u8 {
    (p.value() as u8 & 0x7f) | if p.is_symbolic() { 0x80 } else { 0 }
}

struct RuleExtras {
    raw_pivot: Option<Vec<u8>>,
    recenter: Option<Vec<i16>>,
    trace_rows: usize,
    lift_seconds: f64,
    source_order: String,
    numerical: bool,
    monomials: usize,
    oversized_fallback: bool,
    /// guard-fix D1: guard semantics of this rule (2 = witness guards, 1 = RHS denominators only)
    guard_version: u32,
    /// seed-centre: the seed setting this rule's case search used (None = default seeds: field absent).
    seed_setting: Option<String>,
}

fn write_rule<const N: usize>(
    f: &mut impl std::io::Write,
    chart: &str,
    index: usize,
    rule: &SectorRule<N>,
    x: &RuleExtras,
    n: usize,
) -> usize {
    let c = &rule.candidate;
    let rhs: Vec<String> = c
        .rhs
        .iter()
        .map(|t| {
            format!(
                "[{},{}]",
                q(&t.integral.to_string()),
                q(&t.coefficient.to_string())
            )
        })
        .collect();
    let exceptions: Vec<String> = rule
        .exceptions
        .branches
        .iter()
        .map(|b| strings(b.iter()))
        .collect();
    let spec = case_spec(&c.case, n);
    let fixed: Vec<String> = spec
        .fixed
        .iter()
        .map(|v| v.map_or("null".to_string(), |x| x.to_string()))
        .collect();
    let mut seeds: Vec<String> = c
        .sources
        .iter()
        .map(|s| s.seed.integral.to_string())
        .collect();
    seeds.sort();
    seeds.dedup();
    // guard-fix: the index-containing witness poles (canonical frame, unfactored, primitive up to sign), for audits
    // and replay hardening; their factors are already among `exceptions`. Very long ones are elided.
    let witness_loci: Vec<String> = c
        .hidden_poles
        .iter()
        .map(|p| p.to_string())
        .filter(|t| {
            t.as_bytes()
                .windows(2)
                .any(|w| w[0] == b'n' && w[1].is_ascii_digit())
        })
        .map(|t| {
            if t.len() > 4000 && super::childjob::elide_witness_loci() {
                "elided".to_string()
            } else {
                t
            }
        })
        .collect();
    // N2 (helper y3b): the d-only (parameter-only) factors of the witness poles, never index guards (rules hold in
    // Q(d)); recorded so that a specialisation of d (d = 4, integer-d checks) knows which values to avoid. A witness
    // pole too long for the record (> 4000 characters, as witness_loci) is not factored here: "elided".
    let mut witness_d_poles: Vec<String> = Vec::new();
    let has_index = |t: &str| {
        t.as_bytes()
            .windows(2)
            .any(|w| w[0] == b'n' && w[1].is_ascii_digit())
    };
    let short: Vec<_> = c
        .hidden_poles
        .iter()
        .filter(|p| {
            let long = p.to_string().len() > 4000 && super::childjob::elide_witness_loci();
            if long {
                witness_d_poles.push("elided".to_string());
            }
            !long
        })
        .cloned()
        .collect();
    witness_d_poles.extend(
        crate::solver::exception::primitive_factors(&short)
            .iter()
            .map(|f| f.to_string())
            .filter(|t| !has_index(t)),
    );
    witness_d_poles.sort();
    witness_d_poles.dedup();
    let opt = |v: &Option<Vec<i16>>| v.as_ref().map_or("null".to_string(), |v| format!("{v:?}"));
    // sym-gen step 2: number of exact symmetry rows in the rule's trace (field absent when 0: off tables unchanged)
    let sym_rows = c
        .sources
        .iter()
        .filter(|s| s.basis_row == crate::solver::SYMMETRY_ROW)
        .count();
    let mut sym_field = if sym_rows > 0 {
        format!(",\"sym_rows\":{sym_rows}")
    } else {
        String::new()
    };
    if let Some(setting) = &x.seed_setting {
        sym_field.push_str(&format!(",\"seed_setting\":{setting}"));
    }
    let raw = x
        .raw_pivot
        .as_ref()
        .map_or("null".to_string(), |v| format!("{v:?}"));
    writeln!(
        f,
        "{{\"type\":\"rule\",\"chart\":{},\"index\":{index},\"case\":{},\"fixed\":[{}],\"equations\":{},\"target\":{},\"rhs\":[{}],\"exceptions\":[{}],\"dispatch\":{},\"nsources\":{},\"seeds\":{},\"direct_hit\":{},\"seconds_search\":{:.6},\"raw_pivot_encoded\":{raw},\"recenter\":{},\"trace_rows\":{},\"lift_seconds\":{:.6},\"source_order\":{},\"numerical\":{},\"monomials\":{},\"oversized_fallback\":{},\"witness_poles\":{},\"witness_loci\":{},\"witness_d_poles\":{},\"guard_version\":{}{sym_field}}}",
        q(chart),
        q(&format!("{:?}", c.case)),
        fixed.join(","),
        strings(spec.equations.iter()),
        q(&c.target.to_string()),
        rhs.join(","),
        exceptions.join(","),
        q(&format!("{:?}", rule.dispatch_policy)),
        c.sources.len(),
        strings(seeds.iter()),
        c.stats.direct_hit,
        c.stats.elapsed.as_secs_f64(),
        opt(&x.recenter),
        x.trace_rows,
        x.lift_seconds,
        q(&x.source_order),
        x.numerical,
        x.monomials,
        x.oversized_fallback,
        c.hidden_poles.len(),
        strings(witness_loci.iter()),
        strings(witness_d_poles.iter()),
        x.guard_version,
    )
    .expect("write rule");
    c.rhs.len()
}

fn write_cert_header<const N: usize>(
    f: &mut impl std::io::Write,
    fam: &IntegralFamily,
    sources: &SourceSystem<N>,
) {
    let (loops, externals) = (fam.loop_count(), fam.external_count());
    writeln!(
        f,
        "{{\"type\":\"cert_header\",\"schema\":{},\"family\":{},\"loops\":{loops},\"externals\":{externals},\"source_rows\":{},\"conditions\":{}}}",
        q(SCHEMA),
        q(fam.name()),
        sources.rows().len(),
        strings(sources.conditions().iter())
    )
    .expect("cert header");
    let contraction: Vec<usize> = (loops..loops + externals).rev().chain(0..loops).collect();
    for (m, row) in sources.rows().iter().enumerate() {
        let (loop_id, momentum) = (m / contraction.len(), contraction[m % contraction.len()]);
        let terms: Vec<String> = row
            .iter()
            .map(|t| {
                format!(
                    "[{},{}]",
                    q(&t.integral.to_string()),
                    q(&t.coefficient.to_string())
                )
            })
            .collect();
        writeln!(f, "{{\"type\":\"source_row\",\"m\":{m},\"differentiated_loop\":{loop_id},\"contraction_momentum\":{momentum},\"terms\":[{}]}}", terms.join(",")).expect("source row");
    }
}

/// Covering check of the harness (`rule_covers`): the rule's case contains `case` and no exceptional branch meets it.
pub(super) fn covers<const N: usize>(
    rcase: &Case<N>,
    branches: &[Vec<CoefficientPolynomial>],
    case: &Case<N>,
    idx: &[usize; N],
    sector: &[bool; N],
    limits: CaseIntersectionLimits,
) -> bool {
    rcase.contains(case).unwrap_or(false)
        && branches.iter().all(
            // guard-fix (Y3c/Y3d): a deferred piece is NOT an empty intersection (fail closed, as union_covered)
            |b| matches!(case.intersect_many(b, idx, sector, limits), Ok(r) if r.cases.is_empty() && r.deferred.is_empty()),
        )
}

fn prune_subsumed<const N: usize>(cases: &mut Vec<Case<N>>, candidate: &Case<N>) -> bool {
    if cases
        .iter()
        .any(|queued| queued.contains(candidate).unwrap_or(false))
    {
        return false;
    }
    cases.retain(|c| !candidate.contains(c).unwrap_or(false));
    true
}

/// A cover entry: label, rule case, exceptional branches (rules of cover tables, banks, sibling pieces).
type CoverEntry<const N: usize> = (String, Case<N>, Vec<Vec<CoefficientPolynomial>>);

/// Case and exceptional branches of a table rule line (`fixed`, `equations`, `exceptions`).
fn parse_cover_entry<const N: usize>(
    v: &serde_json::Value,
    template: &CoefficientPolynomial,
    idx: &[usize; N],
    sector: &[bool; N],
    n: usize,
) -> Option<(Case<N>, Vec<Vec<CoefficientPolynomial>>)> {
    let spec = rule_line_case(v);
    let case = build_case::<N>(&spec, template, idx, sector, n)
        .ok()
        .flatten()?;
    let mut branches = Vec::new();
    for b in v["exceptions"].as_array()? {
        let mut polys = Vec::new();
        for p in b.as_array()? {
            polys.push(parse_index_poly(p.as_str()?, template, idx)?);
        }
        branches.push(polys);
    }
    Some((case, branches))
}

fn rule_line_case(v: &serde_json::Value) -> CaseSpec {
    CaseSpec {
        fixed: v["fixed"]
            .as_array()
            .map(|a| a.iter().map(|x| x.as_i64()).collect())
            .unwrap_or_default(),
        equations: v["equations"]
            .as_array()
            .map(|a| {
                a.iter()
                    .filter_map(|x| x.as_str().map(str::to_string))
                    .collect()
            })
            .unwrap_or_default(),
    }
}

/// FNV-1a, 64 bit: a stable key of (presented family, chart, order) for banks and registries.
fn fnv64(parts: &[&str]) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for p in parts {
        for b in p.bytes().chain(std::iter::once(0u8)) {
            h ^= u64::from(b);
            h = h.wrapping_mul(0x0100_0000_01b3);
        }
    }
    h
}

/// Union-aware coverage: `case` is covered when an entry's case contains it and every piece of
/// `case ∩ branch` (over the entry's exceptional branches) is covered by some entry, directly or (depth-bounded)
/// through the same argument. Sound: the entry's rule is valid on its case outside its branches, and the
/// intersection pieces over-cover `case ∩ branch` (deferred pieces make the check fail).
#[allow(clippy::too_many_arguments)]
pub(super) fn union_covered<const N: usize>(
    case: &Case<N>,
    entries: &[(&Case<N>, &[Vec<CoefficientPolynomial>])],
    idx: &[usize; N],
    sector: &[bool; N],
    limits: CaseIntersectionLimits,
    depth: u32,
    budget: &mut usize,
) -> bool {
    if depth == 0 {
        return false;
    }
    let mut tried = 0usize;
    for (rc, branches) in entries.iter().rev() {
        if *budget == 0 {
            return false;
        }
        if !rc.contains(case).unwrap_or(false) {
            continue;
        }
        tried += 1;
        if tried > 6 {
            break;
        }
        let mut all = true;
        'branches: for b in branches.iter() {
            *budget = budget.saturating_sub(1);
            match case.intersect_many(b, idx, sector, limits) {
                Ok(r) if r.deferred.is_empty() => {
                    for ch in &r.cases {
                        if ch == case {
                            all = false;
                            break 'branches;
                        }
                        let direct = entries
                            .iter()
                            .any(|(c2, b2)| covers(c2, b2, ch, idx, sector, limits));
                        if !direct
                            && !union_covered(ch, entries, idx, sector, limits, depth - 1, budget)
                        {
                            all = false;
                            break 'branches;
                        }
                    }
                }
                _ => {
                    all = false;
                    break 'branches;
                }
            }
        }
        if all {
            return true;
        }
    }
    false
}

/// Guards of a candidate: native, fork-timed native try with the exact content-route fallback, or native only.
enum Guards {
    Ok(ExceptionalConditions, Option<String>, bool),
    Timeout(u64),
    Err(String),
}

#[allow(clippy::too_many_arguments)]
fn guards_of<const N: usize>(
    candidate: &RuleCandidate<N>,
    idx: &[usize; N],
    sector: &[bool; N],
    opts: &NodeOptions,
    out_dir: &Path,
    tag: &str,
    allow_content: bool,
) -> Guards {
    let Some(secs) = opts.native_try else {
        return match extract_exceptions(candidate, idx, sector) {
            Ok(x) => Guards::Ok(x, None, false),
            Err(e) => Guards::Err(format!("{e:?}")),
        };
    };
    if opts.guard_content_fallback && allow_content {
        let route = GuardRoute::NativeThenContent {
            native_timeout: std::time::Duration::from_secs(secs),
        };
        return match extract_exceptions_routed(candidate, idx, sector, &[false; N], route) {
            Ok((x, report)) => {
                note_guard_report(&report, secs, None);
                Guards::Ok(
                    x,
                    Some(format!("{report:?}")),
                    report.route == GuardRouteTaken::Content,
                )
            }
            Err(e) => Guards::Err(format!("{e:?}")),
        };
    }
    let r = forked(out_dir, tag, secs, || {
        match extract_exceptions(candidate, idx, sector) {
            Ok(_) => "ok".to_string(),
            Err(e) => format!("err {e:?}"),
        }
    });
    note_forked(
        &r,
        super::childjob::TimeWhat::GuardTimeout,
        secs,
        None,
        "no guards (side bank)",
    );
    if !matches!(r, Forked::Done(..)) {
        return Guards::Timeout(secs);
    }
    match extract_exceptions(candidate, idx, sector) {
        Ok(x) => Guards::Ok(x, None, false),
        Err(e) => Guards::Err(format!("{e:?}")),
    }
}

/// child-job (audit-engjob M5): a fork-timed try that the CLOCK decided (`Forked::Timeout`: `what`), or that a
/// helper failure decided (`Forked::Failed`: fork unavailable or the helper died), is recorded as a time decision.
/// No-op in legacy mode and when the try finished.
fn note_forked(
    r: &Forked,
    what: super::childjob::TimeWhat,
    limit: u64,
    rule: Option<usize>,
    taken: &str,
) {
    match r {
        Forked::Done(..) => {}
        Forked::Timeout(t) => super::childjob::time_decision(what, rule, limit as f64, *t, taken),
        Forked::Failed(_, t) => super::childjob::time_decision(
            super::childjob::TimeWhat::ForkUnavailable,
            rule,
            limit as f64,
            *t,
            taken,
        ),
    }
}

/// child-job (audit-engjob M5): the timed native guard try of the content-fallback route timed out (content route
/// taken): a time decision. (A helper that could not be forked or was killed by a signal is recorded by
/// `solver::exception`'s timed try itself; a native error is deterministic and not recorded.)
fn note_guard_report(report: &crate::solver::GuardReport, limit: u64, rule: Option<usize>) {
    if report.native_try == crate::solver::NativeGuardTry::TimedOut {
        super::childjob::time_decision(
            super::childjob::TimeWhat::GuardRoute,
            rule,
            limit as f64,
            report.try_seconds,
            "content route",
        );
    }
}

// ---------------------------------------------------------------------------------------------------------------
// process helpers (fork-timed tries and the case-budget watchdog), as in the campaign harness
unsafe extern "C" {
    pub(crate) fn fork() -> i32;
    pub(crate) fn waitpid(pid: i32, status: *mut i32, options: i32) -> i32;
    pub(crate) fn kill(pid: i32, sig: i32) -> i32;
    pub(crate) fn _exit(code: i32) -> !;
}

static FORK_CHILD: std::sync::atomic::AtomicI32 = std::sync::atomic::AtomicI32::new(0);

pub enum Forked {
    Done(String, f64),
    Timeout(f64),
    Failed(String, f64),
}

/// Run `f` in a forked child (its String result passed through a file in `dir`); kill it after `secs`.
/// The caller must be single-threaded apart from idle helper threads (RAYON_NUM_THREADS=1 in campaign runs).
pub fn forked<F: FnOnce() -> String>(dir: &Path, tag: &str, secs: u64, f: F) -> Forked {
    let path = dir.join(format!("rr_fork_{}_{}.out", std::process::id(), tag));
    let _ = std::fs::remove_file(&path);
    let t = Instant::now();
    let parent = super::childjob::proc::pid();
    // SAFETY: fork in a process whose other threads (if any) hold no locks the child needs; the child only runs
    // `f`, writes one file and _exits.
    let pid = unsafe { fork() };
    if pid < 0 {
        return Forked::Failed("fork failed".into(), 0.0);
    }
    if pid == 0 {
        super::childjob::proc::after_fork_child(parent);
        let s = std::panic::catch_unwind(std::panic::AssertUnwindSafe(f))
            .unwrap_or_else(|_| "{\"child_panic\":true}".to_string());
        let tmp = path.with_extension("part");
        let ok = std::fs::write(&tmp, s.as_bytes()).is_ok() && std::fs::rename(&tmp, &path).is_ok();
        // SAFETY: terminate the forked child without running the parent's destructors.
        unsafe { _exit(if ok { 0 } else { 3 }) }
    }
    FORK_CHILD.store(pid, std::sync::atomic::Ordering::SeqCst);
    let registered = super::childjob::proc::register_helper(pid);
    loop {
        // the child is reaped only after it left the helper registry (audit-engjob m4: no pid-reuse window for the
        // job watchdog's kill_helpers)
        if super::childjob::proc::helper_ended(pid) {
            FORK_CHILD.store(0, std::sync::atomic::Ordering::SeqCst);
            let status = super::childjob::proc::reap_helper(pid, registered).unwrap_or(-1);
            let used = t.elapsed().as_secs_f64();
            return match std::fs::read_to_string(&path) {
                Ok(s) => {
                    let _ = std::fs::remove_file(&path);
                    Forked::Done(s, used)
                }
                Err(_) => Forked::Failed(format!("child exited, status {status}, no output"), used),
            };
        }
        if t.elapsed().as_secs() >= secs {
            super::childjob::proc::kill_reap_helper(pid, registered);
            let _ = std::fs::remove_file(&path);
            let _ = std::fs::remove_file(path.with_extension("part"));
            FORK_CHILD.store(0, std::sync::atomic::Ordering::SeqCst);
            return Forked::Timeout(t.elapsed().as_secs_f64());
        }
        std::thread::sleep(std::time::Duration::from_millis(100));
    }
}

struct WatchState {
    started: Option<Instant>,
    current: String,
    pending: Vec<String>,
    numerical: Vec<String>,
    rules_done: usize,
}
static WATCH: Mutex<Option<WatchState>> = Mutex::new(None);

fn start_watchdog(budget: u64, spill: PathBuf) {
    std::thread::spawn(move || {
        loop {
            std::thread::sleep(std::time::Duration::from_millis(500));
            let g = WATCH.lock().unwrap();
            let Some(s) = g.as_ref() else { continue };
            let Some(t) = s.started else { continue };
            if t.elapsed().as_secs() < budget {
                continue;
            }
            let child = FORK_CHILD.load(std::sync::atomic::Ordering::SeqCst);
            if child > 0 {
                // SAFETY: our own forked child.
                unsafe {
                    kill(child, 9);
                }
            }
            let body = format!(
                "{{\"type\":\"spill\",\"reason\":\"case_budget\",\"budget\":{budget},\"case_seconds\":{:.3},\"rules_done\":{},\"current\":{},\"pending\":[{}],\"numerical\":[{}]}}\n",
                t.elapsed().as_secs_f64(),
                s.rules_done,
                s.current,
                s.pending.join(","),
                s.numerical.join(",")
            );
            let tmp = spill.with_extension("part");
            let _ = std::fs::write(&tmp, body.as_bytes());
            let _ = std::fs::rename(&tmp, &spill);
            eprintln!("SPILL case budget {budget} s exceeded; exiting 9");
            std::process::exit(9);
        }
    });
}

// ---------------------------------------------------------------------------------------------------------------

/// Cost proxy of a measured relation: RHS terms x dense coefficient box C(deg + v, v) (deg = max(num, den) total
/// degree, v = active variables); a direct hit costs its RHS; a non-converged degree probe is infinite.
pub fn relation_cost(m: &super::screen::ScreenMeasure) -> f64 {
    m.cost()
}

/// The seed-pose portfolio of a case (seed-pose agent, R3a): label, case seed schedule, in-shell order.
/// Free axes = the case's unfixed coordinates (affine-coupled ones included), in stored order; `phys` = line mask.
pub fn seed_pose_portfolio<const N: usize>(
    case: &Case<N>,
    phys: &[bool; N],
) -> Vec<(String, CaseSeedSchedule, SeedScheduleName)> {
    let fixed = case.fixed();
    let free: Vec<usize> = (0..N).filter(|&i| fixed[i].is_none()).collect();
    // free ISPs, free lines, boundary (line 1 / ISP 0), interior
    let class = |i: usize| match fixed[i] {
        None if !phys[i] => 0,
        None => 1,
        Some(v) if (phys[i] && v == 1) || (!phys[i] && v == 0) => 2,
        Some(_) => 3,
    };
    // free axes first (stored order), then the fixed ones (stored order)
    let mut free_first: Vec<usize> = (0..N).collect();
    free_first.sort_by_key(|&i| fixed[i].is_some());
    let _ = class;
    let natural: Vec<usize> = (0..N).collect();
    let _ = free;
    use SeedScheduleName::{DotsFirst, Spired, SymbolicFirst};
    let sym = |d: u32, prio: &Vec<usize>, pf: bool| CaseSeedSchedule::SymbolicShells {
        symbolic_depth: d,
        then_full: true,
        last_depth: 3,
        axis_priority: prio.clone(),
        positive_first: pf,
    };
    let shell2 = |prio: &Vec<usize>, pf: bool| CaseSeedSchedule::ShiftShells {
        first_depth: 2,
        last_depth: 2,
        axis_priority: prio.clone(),
        positive_first: pf,
    };
    // [M] seed-pose sweep (19 hard 5L cases of the shipped legacy order + 41 cases of Astra's measured poses, ~45
    // poses each): with the default, this portfolio is within 10x of the best measured pose (Astra's hand poses
    // included) on 19/19 + 39/41 cases (the default alone: 3/19 + 21/41).
    let reversed: Vec<usize> = (0..N).rev().collect();
    let _ = (&natural, sym(2, &natural, false));
    let mut poses: Vec<(String, CaseSeedSchedule, SeedScheduleName)> = Vec::new();
    // seed-rung: the AFFINE pose (affine-faces R2a, AffineCoupledFirst): on a case with equations, shell 2 with the
    // equation-COUPLED free axes first (unit shift not tangent to the face), then the other free axes, then the fixed
    // ones; first in the portfolio of an affine case (T210 #2211 3.5M -> 52 monomials, T255 faces 23-104).
    if let Some(affine) = case.affine() {
        let coupled = |axis: usize| {
            fixed[axis].is_none() && {
                let mut unit = [0i16; N];
                unit[axis] = 1;
                !affine.is_tangent(&unit)
            }
        };
        let coupled_first: Vec<usize> = free_first
            .iter()
            .copied()
            .filter(|&a| coupled(a))
            .chain(free_first.iter().copied().filter(|&a| !coupled(a)))
            .collect();
        if coupled_first.iter().any(|&a| coupled(a)) {
            poses.push((
                "shell2-coupled+|spired".into(),
                shell2(&coupled_first, true),
                Spired,
            ));
        }
    }
    poses.extend([
        ("ref|sf".into(), CaseSeedSchedule::Reference, SymbolicFirst),
        ("ref|dots".into(), CaseSeedSchedule::Reference, DotsFirst),
        ("sym1-rev+|spired".into(), sym(1, &reversed, true), Spired),
        (
            "shell2-ff+|spired".into(),
            shell2(&free_first, true),
            Spired,
        ),
    ]);
    poses
}

/// Fixed-LINE poses (one fixed line first) measured by the rung after the free-first shell-3 lead by default:
/// none (opt-in with `--seed-pose-fixed-dot-lines K`). [M] seed-fixed-dot ab5: in the rung the lead was chosen in
/// all 5 cases where the gate opened; the line poses never beat it and cost 140-250 s there (T230 #11, T225 G04).
pub const FIXED_DOT_LINES_DEFAULT: usize = 0;
/// Seed budget of one fixed-dot pose (Astra's T265 pose: 2,504 sector-valid seeds in its shell; the relation was
/// selected after ~460).
pub const FIXED_DOT_SEEDS_DEFAULT: u32 = 1_500;
/// The L1 shell of a fixed-dot pose (Astra T265: shell 3 = one fixed dot + an L1-2 move of the free axes).
pub const FIXED_DOT_DEPTH: u32 = 3;
/// The fixed-line poses run only when the free-first shell-3 lead measured a relation of at most this many RHS
/// terms. [M] seed-fixed-dot probes (19 cases): every case where a shell-3 pose gave a small relation had a lead
/// of <= 313 RHS (T265 goal 313, T230 #11 286, T265 #17 242, T225 G04 124, T165 #11 34, T225 G01 20, T266 R45 16,
/// T165 #14 11); on every other case the lead had >= 544 RHS (or none) and no fixed line gave a usable relation
/// (T163 544 ... T183 4,376; T228 none), while the fixed-line poses cost 140-3,300 s of probes there.
pub const FIXED_DOT_GATE_RHS: usize = 400;

/// Fixed-dot configuration of the rung.
#[derive(Clone, Debug)]
pub struct FixedDotConfig {
    /// The fixed-dot extension of the rung at all (`--seed-pose-fixed-dot 0` = off = the old rung).
    pub enabled: bool,
    /// Fixed-line poses after the lead ([`FIXED_DOT_LINES_DEFAULT`]).
    pub lines: usize,
    pub max_seeds: u32,
    pub first: bool,
    /// The node order's final tie priority over the presented slots (None = legacy = identity).
    pub priority: Option<Vec<usize>>,
    /// Skip the fixed-dot poses on a case whose free lines carry a loop momentum ([`FixedDotStatic::FreeCycle`]).
    pub static_skip: bool,
    /// Loop momentum (up to scale) of every presented denominator ([`fixed_dot_line_momenta`]); None = unknown.
    pub momenta: Option<Arc<Vec<Option<Vec<i64>>>>>,
}

impl FixedDotConfig {
    pub fn from_options(opts: &NodeOptions) -> Self {
        Self {
            enabled: opts.seed_pose_fixed_dot != Some(0),
            // an explicit `--seed-pose-fixed-dot K` (K >= 1) keeps its earlier meaning: K line poses
            lines: opts
                .seed_pose_fixed_dot_lines
                .or(opts.seed_pose_fixed_dot.filter(|&k| k > 0))
                .unwrap_or(FIXED_DOT_LINES_DEFAULT),
            max_seeds: opts
                .seed_pose_fixed_dot_seeds
                .unwrap_or(FIXED_DOT_SEEDS_DEFAULT),
            first: opts.seed_pose_fixed_dot_first,
            priority: opts.order.coordinate_priority.clone(),
            static_skip: !opts.seed_pose_fixed_dot_no_static_skip,
            momenta: None,
        }
    }

    pub fn with_momenta(mut self, momenta: Option<Arc<Vec<Option<Vec<i64>>>>>) -> Self {
        self.momenta = momenta;
        self
    }
}

/// The loop momentum of every denominator of `fam`, up to a nonzero factor, read off its loop-loop quadratic
/// part (`(sum_i c_i k_i + p)^2` has `c_i^2` on `k_i^2` and `2 c_i c_j` on `k_i.k_j`, so any row `j` with
/// `c_j != 0` of the symmetric matrix `2 c c^T` is `2 c_j c`). None for a denominator without a quadratic part
/// or with a non-integer coefficient.
pub fn fixed_dot_line_momenta(fam: &IntegralFamily) -> Vec<Option<Vec<i64>>> {
    use crate::family::ScalarProductCoordinate;
    let l = fam.loop_count();
    let integer = |c: &crate::algebra::Coefficient| -> Option<i64> {
        let s = c.to_string();
        s.trim()
            .trim_start_matches('(')
            .trim_end_matches(')')
            .trim()
            .parse::<i64>()
            .ok()
    };
    fam.denominators()
        .iter()
        .map(|d| {
            let mut m = vec![vec![0i64; l]; l];
            for (coef, coord) in d.coefficients().iter().zip(fam.coordinates()) {
                if let ScalarProductCoordinate::LoopLoop { left, right } = *coord {
                    let v = integer(coef)?;
                    if left == right {
                        m[left][left] = v.checked_mul(2)?;
                    } else {
                        m[left][right] = v;
                        m[right][left] = v;
                    }
                }
            }
            let j = (0..l).find(|&j| m[j][j] != 0)?;
            Some(m[j].clone())
        })
        .collect()
}

/// Rank of integer vectors over Q (computed modulo the prime 2^61 - 1: exact for the small entries of
/// line momenta).
#[allow(clippy::needless_range_loop)]
fn rank_mod_p(rows: &[Vec<i64>]) -> usize {
    const P: i128 = (1 << 61) - 1;
    let pow = |mut b: i128, mut e: i128| {
        let mut r = 1i128;
        b = b.rem_euclid(P);
        while e > 0 {
            if e & 1 == 1 {
                r = r * b % P;
            }
            b = b * b % P;
            e >>= 1;
        }
        r
    };
    let mut m: Vec<Vec<i128>> = rows
        .iter()
        .map(|r| r.iter().map(|&x| i128::from(x).rem_euclid(P)).collect())
        .collect();
    let cols = m.first().map_or(0, Vec::len);
    let mut rank = 0;
    for c in 0..cols {
        let Some(p) = (rank..m.len()).find(|&i| m[i][c] != 0) else {
            continue;
        };
        m.swap(rank, p);
        let inv = pow(m[rank][c], P - 2);
        for i in 0..m.len() {
            if i != rank && m[i][c] != 0 {
                let f = m[i][c] * inv % P;
                for k in 0..cols {
                    m[i][k] = (m[i][k] - f * m[rank][k] % P).rem_euclid(P);
                }
            }
        }
        rank += 1;
    }
    rank
}

/// Helper Y9's static fixed-dot test of a case (2026-11-12T20:50Z), with RustRed's own comparator.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FixedDotStatic {
    /// `cyc = L - rank{momenta of the fixed lines} >= 1`: the free lines carry a loop momentum; on-face seeds
    /// close the case, fixed-dot seeds are unnecessary (Y9: 10/11 cases).
    FreeCycle(usize),
    /// cyc = 0 and every minimal exposed set of fixed lines has a dot transfer {a: +1, f: -1} (a exposed, f a free
    /// line) that is NOT below the target under the node order: fixed dots needed (Y9: 19/21).
    Needed,
    /// cyc = 0 and some minimal exposed set has all its transfers below the target: undecided (Y9: 8 of 18 on-face).
    Undecided,
    /// No momenta (non-integer or linear denominators) or no fixed line.
    Unknown,
}

impl FixedDotStatic {
    pub fn label(self) -> String {
        match self {
            Self::FreeCycle(c) => format!("free_cycle{c}"),
            Self::Needed => "needed".into(),
            Self::Undecided => "undecided".into(),
            Self::Unknown => "unknown".into(),
        }
    }
}

/// Detail of [`fixed_dot_static`]: the verdict, cyc, the minimal exposure mu and its exposed sets, and the fixed
/// lines that have a dot transfer NOT below the target ("rising" lines).
#[derive(Clone, Debug)]
pub struct FixedDotStaticReport {
    pub verdict: FixedDotStatic,
    pub cyc: Option<usize>,
    pub mu: Option<usize>,
    pub exposed: Vec<Vec<usize>>,
    pub rising: Vec<usize>,
}

impl FixedDotStaticReport {
    pub fn to_json(&self) -> String {
        format!(
            "{{\"verdict\":\"{}\",\"cyc\":{},\"mu\":{},\"exposed\":{},\"rising\":{}}}",
            self.verdict.label(),
            self.cyc.map_or("null".into(), |c| c.to_string()),
            self.mu.map_or("null".into(), |c| c.to_string()),
            serde_json::to_string(&self.exposed).unwrap(),
            serde_json::to_string(&self.rising).unwrap(),
        )
    }
}

/// Y9's static test: `cyc = L - rank{alpha_a : a fixed line}`; for cyc = 0 the exposed sets E(c) = {fixed lines a
/// with alpha_a . c != 0} of the loop directions c in {-1,0,1}^L of minimal size, and for each E whether every
/// transfer {a: +1, f: -1} (a in E, f a free line) is below the target under `order` (RustRed's comparator:
/// `compare(target, term) == Less`).
pub fn fixed_dot_static<const N: usize>(
    case: &Case<N>,
    phys: &[bool; N],
    momenta: &[Option<Vec<i64>>],
    order: &crate::solver::IntegralOrder<N>,
) -> FixedDotStaticReport {
    let unknown = FixedDotStaticReport {
        verdict: FixedDotStatic::Unknown,
        cyc: None,
        mu: None,
        exposed: Vec::new(),
        rising: Vec::new(),
    };
    let fixed = case.fixed();
    let lines: Vec<usize> = (0..N)
        .filter(|&i| phys[i] && matches!(fixed[i], Some(v) if v >= 1))
        .collect();
    // free LINES (the transfers lower one); a case whose free axes are all ISPs has none: then no transfer exists
    // and the verdict is at most Undecided
    let free: Vec<usize> = (0..N).filter(|&i| phys[i] && fixed[i].is_none()).collect();
    if lines.is_empty() {
        return unknown;
    }
    let Some(alpha): Option<Vec<&Vec<i64>>> = lines
        .iter()
        .map(|&a| momenta.get(a).and_then(Option::as_ref))
        .collect()
    else {
        return unknown;
    };
    let l = alpha[0].len();
    let cyc = l - rank_mod_p(&alpha.iter().map(|v| (*v).clone()).collect::<Vec<_>>());
    if cyc >= 1 {
        return FixedDotStaticReport {
            verdict: FixedDotStatic::FreeCycle(cyc),
            cyc: Some(cyc),
            ..unknown
        };
    }
    // minimal exposed sets over c in {-1,0,1}^L \ {0}
    let mut best: Option<usize> = None;
    let mut sets: Vec<Vec<usize>> = Vec::new();
    let total = 3usize.pow(l as u32);
    for code in 1..total {
        let mut x = code;
        let c: Vec<i64> = (0..l)
            .map(|_| {
                let d = (x % 3) as i64 - 1;
                x /= 3;
                d
            })
            .collect();
        if c.iter().all(|&v| v == 0) {
            continue;
        }
        let e: Vec<usize> = lines
            .iter()
            .zip(&alpha)
            .filter(|(_, v)| v.iter().zip(&c).map(|(a, b)| a * b).sum::<i64>() != 0)
            .map(|(&a, _)| a)
            .collect();
        match best {
            Some(b) if e.len() > b => {}
            Some(b) if e.len() == b => {
                if !sets.contains(&e) {
                    sets.push(e);
                }
            }
            _ => {
                best = Some(e.len());
                sets = vec![e];
            }
        }
    }
    let target = case.integral();
    let below = |a: usize, f: usize| -> Option<bool> {
        let mut powers = *target.powers();
        powers[a] = crate::solver::Power::new(false, fixed[a]? + 1).ok()?;
        powers[f] = powers[f].shifted(-1).ok()?;
        let term = Integral::new(powers);
        Some(order.compare(&target, &term) == Ordering::Less)
    };
    let mut rising: Vec<usize> = lines
        .iter()
        .copied()
        .filter(|&a| free.iter().any(|&f| below(a, f) == Some(false)))
        .collect();
    rising.sort_unstable();
    let some_all_below = free.is_empty()
        || sets.iter().any(|e| {
            e.iter()
                .all(|&a| free.iter().all(|&f| below(a, f) == Some(true)))
        });
    FixedDotStaticReport {
        verdict: if some_all_below {
            FixedDotStatic::Undecided
        } else {
            FixedDotStatic::Needed
        },
        cyc: Some(0),
        mu: best,
        exposed: sets,
        rising,
    }
}

/// The fixed lines of a case (value >= 1; ISPs never lead a fixed-dot pose) in the order the fixed-dot poses try
/// them: the LEAST significant line of the node order's tie priority first (`priority` = most significant first;
/// None = the legacy identity). [M] seed-fixed-dot probes: on the 3 cases where a fixed-dot pose wins (T265 goal
/// under pred_n13_last, T265 #17 and T230 #11 under the legacy order) the best line is among the first 3 of this
/// order (lines 13 / 13 / 9); dotted lines first was refuted (T265 #17 dotted line 2: 531 RHS vs 96-142).
pub fn fixed_dot_axes<const N: usize>(
    case: &Case<N>,
    phys: &[bool; N],
    priority: Option<&[usize]>,
) -> Vec<usize> {
    let fixed = case.fixed();
    let rank = |i: usize| priority.map_or(i, |p| p.iter().position(|&a| a == i).unwrap_or(i));
    let mut lines: Vec<usize> = (0..N)
        .filter(|&i| phys[i] && matches!(fixed[i], Some(v) if v >= 1))
        .collect();
    lines.sort_by_key(|&i| std::cmp::Reverse(rank(i)));
    lines
}

/// seed-fixed-dot poses of a case: the free-first shell-3 lead, then for each fixed line `a` of
/// [`fixed_dot_axes`] (at most `k`; 0 = the lead only), one shell
/// L1 = 3 ([`CaseSeedSchedule::FixedDotShell`]) with axis priority [a, free axes (equation-coupled first), the
/// other fixed lines, the remaining axes], positive first, at most `max_seeds` seeds, SpIRed in-shell order.
/// Source seeds leave the target face by a dot on `a`; the selected pivot is still on the case.
pub fn fixed_dot_poses<const N: usize>(
    case: &Case<N>,
    phys: &[bool; N],
    k: usize,
    max_seeds: u32,
    priority: Option<&[usize]>,
) -> Vec<(String, CaseSeedSchedule, SeedScheduleName)> {
    let fixed = case.fixed();
    let free: Vec<usize> = (0..N).filter(|&i| fixed[i].is_none()).collect();
    let coupled = |axis: usize| {
        case.affine().is_some_and(|affine| {
            let mut unit = [0i16; N];
            unit[axis] = 1;
            !affine.is_tangent(&unit)
        })
    };
    let free: Vec<usize> = free
        .iter()
        .copied()
        .filter(|&a| coupled(a))
        .chain(free.iter().copied().filter(|&a| !coupled(a)))
        .collect();
    let lines = fixed_dot_axes(case, phys, priority);
    if free.is_empty() {
        return Vec::new();
    }
    // [M] seed-fixed-dot probe4: shell 3 ALONE with the free axes first (no fixed line first) is the cheapest pose
    // (7-45 s modular) and already gives the small relation on T266 R45 (16 RHS), T165 #11 (34), T225 r38 (20);
    // it leads, then one pose per fixed line (at most k).
    let mut ff = free.clone();
    ff.extend(lines.iter().copied());
    ff.extend((0..N).filter(|i| !free.contains(i) && !lines.contains(i)));
    let lead = (
        format!("shell{FIXED_DOT_DEPTH}-ff+|spired"),
        CaseSeedSchedule::FixedDotShell {
            depth: FIXED_DOT_DEPTH,
            axis_priority: ff,
            positive_first: true,
            max_seeds,
        },
        SeedScheduleName::Spired,
    );
    std::iter::once(lead)
        .chain(lines.iter().take(k).map(|&a| {
            let mut prio = vec![a];
            prio.extend(free.iter().copied());
            prio.extend(lines.iter().copied().filter(|&b| b != a));
            let rest: Vec<usize> = (0..N).filter(|i| !prio.contains(i)).collect();
            prio.extend(rest);
            (
                format!("fixdot{a}-shell{FIXED_DOT_DEPTH}+|spired"),
                CaseSeedSchedule::FixedDotShell {
                    depth: FIXED_DOT_DEPTH,
                    axis_priority: prio,
                    positive_first: true,
                    max_seeds,
                },
                SeedScheduleName::Spired,
            )
        }))
        .collect()
}

/// Result of the seed-pose rung.
pub struct SeedPoseChoice {
    pub label: String,
    pub schedule: CaseSeedSchedule,
    pub in_shell: SeedScheduleName,
    pub cost: f64,
    pub default_cost: f64,
    pub probes: usize,
    pub seconds: f64,
}

/// Measure one pose with a refuse-all screen: Some(cost) of its selected relation, None when no relation.
fn measure_pose<const N: usize>(
    solver: &SectorSolver<N>,
    case: &Case<N>,
    options: SearchOptions,
    visit: Option<&[usize]>,
    schedule: &CaseSeedSchedule,
    in_shell: SeedScheduleName,
) -> Option<f64> {
    measure_pose_rhs(solver, case, options, visit, schedule, in_shell).map(|(cost, _)| cost)
}

/// [`measure_pose`] with the RHS count of the measured relation: Some((cost, rhs terms)).
fn measure_pose_rhs<const N: usize>(
    solver: &SectorSolver<N>,
    case: &Case<N>,
    options: SearchOptions,
    visit: Option<&[usize]>,
    schedule: &CaseSeedSchedule,
    in_shell: SeedScheduleName,
) -> Option<(f64, usize)> {
    let screen = LiftScreen {
        min_rows: 0,
        min_target_block: 0,
        max_points: 64,
        max_total_degree: 0,
        max_rhs_terms: 0,
        sampled_from_rows: None,
        on_heavy: super::screen::HeavyAction::Refuse,
        symbolic_first_from_rows: None,
        ..Default::default()
    };
    let _ = super::screen::take_reports();
    let result = super::screen::with_seed_schedule(Some(in_shell.schedule()), || {
        super::screen::with_screen(Some(screen), || {
            solver.solve_case_with_schedule_and_observer(
                case.clone(),
                options,
                visit,
                schedule,
                |_| {},
            )
        })
    });
    let reports = super::screen::take_reports();
    let _ = super::screen::take_trace();
    match result {
        Ok(candidate) => Some((candidate.rhs.len() as f64, candidate.rhs.len())),
        Err(_) => reports
            .last()
            .map(|r| (relation_cost(&r.measure), r.measure.rhs_terms)),
    }
}

/// The automatic seed-pose rung: keep the default when it is cheap, else the cheapest measured portfolio pose.
#[allow(clippy::too_many_arguments)]
pub fn choose_seed_pose<const N: usize>(
    solver: &SectorSolver<N>,
    case: &Case<N>,
    options: SearchOptions,
    visit: Option<&[usize]>,
    phys: &[bool; N],
    node_in_shell: SeedScheduleName,
    keep: f64,
    known_default: Option<f64>,
    fixed_dot: FixedDotConfig,
) -> Option<SeedPoseChoice> {
    let t = Instant::now();
    // seed-rung: the gated normal search already measured the default relation (no second search for it)
    let default_cost = known_default.unwrap_or_else(|| {
        measure_pose(
            solver,
            case,
            options,
            visit,
            &CaseSeedSchedule::Reference,
            node_in_shell,
        )
        .unwrap_or(f64::INFINITY)
    });
    let mut best = SeedPoseChoice {
        label: "default".into(),
        schedule: CaseSeedSchedule::Reference,
        in_shell: node_in_shell,
        cost: default_cost,
        default_cost,
        probes: usize::from(known_default.is_none()),
        seconds: 0.0,
    };
    if default_cost > keep {
        // seed-fixed-dot: the fixed-dot poses follow the portfolio (or precede it with `first` on a case with a
        // dotted fixed line); every pose is measured the same way, the cheapest wins, early stop at `keep`.
        // Y9 static test first: a case whose free lines carry a loop momentum skips them (static_skip).
        let report = fixed_dot.momenta.as_deref().map_or_else(
            || FixedDotStaticReport {
                verdict: FixedDotStatic::Unknown,
                cyc: None,
                mu: None,
                exposed: Vec::new(),
                rising: Vec::new(),
            },
            |m| fixed_dot_static(case, phys, m, solver.ordering()),
        );
        let skip_static =
            fixed_dot.static_skip && matches!(report.verdict, FixedDotStatic::FreeCycle(_));
        if fixed_dot.enabled {
            eprintln!(
                "EVENT FixedDotStatic {} skip={skip_static}",
                report.to_json()
            );
        }
        let dots = if fixed_dot.enabled && !skip_static {
            fixed_dot_poses(
                case,
                phys,
                fixed_dot.lines,
                fixed_dot.max_seeds,
                fixed_dot.priority.as_deref(),
            )
        } else {
            Vec::new()
        };
        let dotted = case
            .fixed()
            .iter()
            .zip(phys.iter())
            .any(|(v, &p)| p && matches!(*v, Some(x) if x >= 2));
        let portfolio = seed_pose_portfolio(case, phys);
        let poses: Vec<_> = if fixed_dot.first && dotted {
            dots.into_iter().chain(portfolio).collect()
        } else {
            portfolio.into_iter().chain(dots).collect()
        };
        // the free-first shell-3 lead gates the fixed-line poses (FIXED_DOT_GATE_RHS)
        let mut skip_lines = false;
        for (label, schedule, in_shell) in poses {
            if skip_lines && label.starts_with("fixdot") {
                continue;
            }
            best.probes += 1;
            let measured = measure_pose_rhs(solver, case, options, visit, &schedule, in_shell);
            if label.starts_with(&format!("shell{FIXED_DOT_DEPTH}-ff")) {
                skip_lines = measured.is_none_or(|(_, rhs)| rhs > FIXED_DOT_GATE_RHS);
            }
            if let Some((cost, _)) = measured {
                if cost < best.cost {
                    best.label = label;
                    best.schedule = schedule;
                    best.in_shell = in_shell;
                    best.cost = cost;
                }
            }
            if best.cost <= keep {
                break;
            }
        }
    }
    best.seconds = t.elapsed().as_secs_f64();
    Some(best)
}

/// One seed-pose probe (see [`NodeOptions::pose_probe`]).
#[derive(Clone, Debug, Deserialize)]
struct PoseProbe {
    #[serde(default)]
    label: String,
    case: CaseSpec,
    #[serde(default)]
    presented: bool,
    #[serde(default)]
    schedule: Option<CaseSeedSchedule>,
    #[serde(default)]
    in_shell: SeedScheduleName,
    #[serde(default)]
    source_order: Option<SourceOrder>,
    #[serde(default)]
    max_depth: Option<u32>,
    /// Probe point seed of the modular discovery (default 0 = the engine's).
    #[serde(default)]
    sample_seed: u64,
    /// seed-fixed-dot: write only the static fixed-dot test of the case (no search).
    #[serde(default)]
    static_only: bool,
}

/// seed-pose: run every probe of `path` with a refuse-all screen; write `F.poses.jsonl`.
#[allow(clippy::too_many_arguments)]
fn pose_probe_run<const N: usize>(
    path: &str,
    solver: &SectorSolver<N>,
    opts: &NodeOptions,
    template: &CoefficientPolynomial,
    idx: &[usize; N],
    sector: &[bool; N],
    n: usize,
    nb: usize,
    out_dir: &Path,
    top: &[bool; N],
    momenta: &[Option<Vec<i64>>],
) -> Result<NodeOutcome, String> {
    let text = std::fs::read_to_string(path).map_err(|e| format!("pose probes {path}: {e}"))?;
    let mut out = std::io::BufWriter::new(
        std::fs::File::create(out_dir.join("F.poses.jsonl")).map_err(|e| format!("poses: {e}"))?,
    );
    let screen = LiftScreen {
        min_rows: 0,
        min_target_block: 0,
        max_points: opts.screen.map_or(80, |s| s.max_points),
        max_total_degree: 0,
        max_rhs_terms: 0,
        sampled_from_rows: None,
        on_heavy: super::screen::HeavyAction::Refuse,
        symbolic_first_from_rows: None,
        ..opts.screen.unwrap_or_default()
    };
    let mut symbolic = SectorSolveOptions::default().symbolic;
    let mut done = 0usize;
    for (line_no, line) in text.lines().enumerate() {
        if line.trim().is_empty() {
            continue;
        }
        let probe: PoseProbe = serde_json::from_str(line)
            .map_err(|e| format!("pose probe line {}: {e}", line_no + 1))?;
        let (fixed, equations) = if probe.presented {
            (probe.case.fixed.clone(), probe.case.equations.clone())
        } else {
            opts.order
                .present_case(&probe.case.fixed, &probe.case.equations)
        };
        let spec = CaseSpec { fixed, equations };
        let Some(case) = build_case::<N>(&spec, template, idx, sector, n)? else {
            writeln!(
                out,
                "{{\"label\":{},\"line\":{},\"result\":\"empty_case\"}}",
                q(&probe.label),
                line_no + 1
            )
            .map_err(|e| e.to_string())?;
            continue;
        };
        // seed-fixed-dot: Y9's static test of the case under the node order (no search with `static_only`)
        let fd_static = fixed_dot_static(&case, top, momenta, solver.ordering()).to_json();
        if probe.static_only {
            writeln!(
                out,
                "{{\"label\":{},\"line\":{},\"case\":{},\"result\":\"static\",\"fixed_dot_static\":{fd_static}}}",
                q(&probe.label),
                line_no + 1,
                serde_json::to_string(&spec).unwrap(),
            )
            .map_err(|e| e.to_string())?;
            done += 1;
            continue;
        }
        symbolic.max_depth = probe.max_depth.or(opts.max_depth);
        symbolic.sample_seed = probe.sample_seed;
        let schedule = probe.schedule.clone().unwrap_or_default();
        let so = probe
            .source_order
            .clone()
            .unwrap_or_else(|| opts.order.source_order.clone());
        let visit = so.visit(nb);
        let mut pivot: Option<Integral<N>> = None;
        let mut trace_rows = 0usize;
        let mut discovery = None;
        let (mut depth, mut seeds, mut rows) = (0u32, 0usize, 0usize);
        let t = Instant::now();
        let _ = super::screen::take_reports();
        let _ = super::screen::take_trace();
        let result = super::screen::with_seed_schedule(Some(probe.in_shell.schedule()), || {
            super::screen::with_screen(Some(screen), || {
                solver.solve_case_with_schedule_and_observer(
                    case.clone(),
                    symbolic,
                    visit.as_deref(),
                    &schedule,
                    |event: SearchEvent<N>| match event {
                        SearchEvent::ExactStarted {
                            pivot: p,
                            trace_rows: r,
                            discovery: d,
                        } => {
                            pivot = Some(p);
                            trace_rows = r;
                            discovery = Some(d);
                        }
                        SearchEvent::DiscoveryProgress {
                            depth: d,
                            seeds: s,
                            rows: r,
                            ..
                        } => {
                            depth = d;
                            seeds = s;
                            rows = r;
                        }
                        _ => {}
                    },
                )
            })
        });
        let seconds = t.elapsed().as_secs_f64();
        let reports = super::screen::take_reports();
        let trace = super::screen::take_trace();
        let (kind, direct_rhs, err) = match &result {
            Ok(c) => ("direct", Some(c.rhs.len()), String::new()),
            Err(SolverError::SearchExhausted { .. }) => ("exhausted", None, String::new()),
            Err(e) => {
                let s = format!("{e:?}");
                if s.contains("lift pre-screen refused") {
                    ("relation", None, String::new())
                } else {
                    ("error", None, s)
                }
            }
        };
        let pivot_json = pivot.map(|p| {
            // numeric power -> number; symbolic power -> "s<shift>" (displacement from the case's free index)
            let powers: Vec<String> = (0..n)
                .map(|i| {
                    if p[i].is_symbolic() {
                        format!("\"s{}\"", p[i].value())
                    } else {
                        p[i].value().to_string()
                    }
                })
                .collect();
            format!("[{}]", powers.join(","))
        });
        let trace_json = trace.map(|t| {
            serde_json::to_string(
                &t.iter()
                    .map(|(row, shift)| (row, &shift[..n]))
                    .collect::<Vec<_>>(),
            )
            .unwrap()
        });
        writeln!(
            out,
            "{{\"label\":{},\"line\":{},\"case\":{},\"schedule\":{},\"in_shell\":{},\"source_order\":{},\"result\":{},\"error\":{},\"pivot\":{},\"trace_rows\":{},\"direct_rhs\":{},\"last_depth\":{depth},\"seeds\":{seeds},\"rows\":{rows},\"discovery\":{},\"measure\":{},\"trace\":{},\"seconds\":{seconds:.4},\"fixed_dot_static\":{fd_static}}}",
            q(&probe.label),
            line_no + 1,
            serde_json::to_string(&spec).unwrap(),
            serde_json::to_string(&probe.schedule).unwrap(),
            serde_json::to_string(&probe.in_shell).unwrap(),
            q(&so.label()),
            q(kind),
            q(&err),
            pivot_json.unwrap_or_else(|| "null".into()),
            if kind == "direct" { 1 } else { trace_rows },
            direct_rhs.map_or("null".into(), |r| r.to_string()),
            discovery.map_or("null".into(), |d| format!("{{\"rows_seen\":{},\"independent_rows\":{},\"columns\":{}}}", d.rows_seen, d.independent_rows, d.columns)),
            reports.last().map_or("null".into(), |r| serde_json::to_string(&r.measure).unwrap()),
            trace_json.unwrap_or_else(|| "null".into()),
        )
        .map_err(|e| e.to_string())?;
        out.flush().map_err(|e| e.to_string())?;
        done += 1;
    }
    let _ = top;
    eprintln!("pose probes: {done} done");
    Ok(NodeOutcome {
        symbolic_cases: done,
        ..Default::default()
    })
}

#[allow(clippy::too_many_arguments)]
fn run<const N: usize>(
    original_text: &str,
    family_path: &str,
    presented: &str,
    fam: &IntegralFamily,
    out_dir: &Path,
    opts: &NodeOptions,
    events: &mut dyn FnMut(&NodeEvent),
) -> Result<NodeOutcome, String> {
    let n = fam.denominator_count();
    if let Some(schedule) = &opts.case_seed_schedule {
        schedule
            .validate(N)
            .map_err(|e| format!("case seed schedule: {e}"))?;
    }
    if let Some(priority) = opts.case_seed_priority {
        priority.validate_template(opts.case_seed_schedule.as_ref(), N)?;
    }
    let t_total = Instant::now();
    if fam
        .power_shifts()
        .iter()
        .any(|s| s.to_string().trim() != "0")
    {
        return Err("two-sided (eps) families are generated by the engine's two-sided queue (rr-5l-api), not by the node driver".into());
    }
    let sources = SourceSystem::<N>::from_family_with_capacity(fam, true)
        .map_err(|e| format!("sources: {e:?}"))?;
    // seed-fixed-dot: line momenta of the presented family for the rung's static test (Y9)
    let fd_momenta = Arc::new(fixed_dot_line_momenta(fam));
    let flags = super::order::physical_slots(presented);
    if flags.len() != n {
        return Err("family propagator lines do not match the compiled arity".into());
    }
    let top: [bool; N] = std::array::from_fn(|i| i < n && flags[i]);
    let base: [bool; N] = match &opts.chart {
        Some(m) => {
            let b: Vec<bool> = m.chars().map(|c| c == '1').collect();
            if b.len() != n {
                return Err(format!("chart {m}: arity"));
            }
            std::array::from_fn(|i| i < n && b[i])
        }
        None => top,
    };
    let label = mask_string(&base[..n]);
    let terminal_ledger = opts
        .known_terminals
        .as_ref()
        .map(|file| file.bind(original_text, opts, &base, n))
        .transpose()?;
    let analyzer =
        zero::Analyzer::try_unrestricted(fam).map_err(|e| format!("zero analyzer: {e:?}"))?;
    let scope: Vec<usize> = (0..n).filter(|&i| top[i]).collect();
    let t0 = Instant::now();
    let mut zeros: Vec<[bool; N]> = Vec::new();
    for bits in 0u64..(1u64 << scope.len()) {
        let mut active = [false; N];
        for (j, &i) in scope.iter().enumerate() {
            active[i] = bits >> j & 1 == 1;
        }
        let m = Mask::try_new(active.iter().copied()).map_err(|e| format!("mask: {e:?}"))?;
        if let zero::Decision::ProvedZero(_) = analyzer
            .analyze(&m)
            .map_err(|e| format!("zero analysis: {e:?}"))?
        {
            zeros.push(active);
        }
    }
    let zero_seconds = t0.elapsed().as_secs_f64();
    let table_path = out_dir.join("F.table.jsonl");
    let mut out = std::io::BufWriter::new(
        super::childjob::create_stream(&table_path, super::childjob::Stream::Table)
            .map_err(|e| format!("table: {e}"))?,
    );
    let mut qlog = std::io::BufWriter::new(
        super::childjob::create_stream(
            &out_dir.join("F.queue.jsonl"),
            super::childjob::Stream::Queue,
        )
        .map_err(|e| format!("queue log: {e}"))?,
    );
    let options_json = serde_json::to_string(opts).map_err(|e| e.to_string())?;
    writeln!(
        out,
        "{{\"type\":\"header\",\"schema\":{},\"node_schema\":{},\"source_base\":\"rustred-node\",\"family\":{},\"family_file\":{}{},\"family_source\":{},\"original_family_source\":{},\"loops\":{},\"externals\":{},\"denominators\":{n},\"parameters\":{},\"power_shifts\":{},\"mask\":{},\"node_order\":{},\"order_name\":{},\"order_is_program\":{},\"order\":{},\"two_sided_axes\":{:?},\"charts\":{},\"numerical_depth\":{},\"max_numerator_rank\":null,\"source_conditions\":{},\"zero_scope\":\"downset\",\"zero_seconds\":{zero_seconds:.3},\"guard_version\":{},\"node_options\":{options_json}}}",
        q(SCHEMA),
        q(NODE_SCHEMA),
        q(fam.name()),
        q(family_path),
        super::childjob::header_family_digest(),
        q(presented),
        q(original_text),
        fam.loop_count(),
        fam.external_count(),
        strings(fam.coefficient_context().parameter_names().iter()),
        strings(fam.power_shifts().iter()),
        q(&mask_string(&top[..n])),
        opts.order.to_json(),
        q(&opts.order.name),
        opts.order.coordinate_priority.is_some(),
        descriptor_json(&opts.order, n),
        vec![false; n],
        strings([label.clone()].iter()),
        opts.numeric_depth,
        strings(sources.conditions().iter()),
        opts.guard_version(),
    )
    .map_err(|e| e.to_string())?;
    if let Some(ledger) = &terminal_ledger {
        writeln!(
            out,
            "{}",
            serde_json::json!({"type":"terminal_ledger","provenance":ledger})
        )
        .map_err(|e| e.to_string())?;
        out.flush().map_err(|e| e.to_string())?;
    }
    writeln!(
        out,
        "{{\"type\":\"zero_sectors\",\"scope\":\"downset\",\"count\":{},\"masks\":{}}}",
        zeros.len(),
        strings(zeros.iter().map(|z| mask_string(&z[..n])))
    )
    .map_err(|e| e.to_string())?;
    let guards: Vec<String> = (0..n)
        .map(|i| {
            if base[i] {
                "n>=1".to_string()
            } else {
                "n<=0".to_string()
            }
        })
        .collect();
    let mut outcome = NodeOutcome {
        table: table_path.display().to_string(),
        terminal_ledger: terminal_ledger.clone(),
        ..Default::default()
    };
    if zeros.contains(&base) {
        if terminal_ledger.is_some() {
            return Err(
                "known terminals: a proved-zero chart cannot declare terminal masters".into(),
            );
        }
        writeln!(
            out,
            "{{\"type\":\"chart\",\"chart\":{},\"guards\":{},\"zero\":true}}",
            q(&label),
            strings(guards.iter())
        )
        .map_err(|e| e.to_string())?;
        writeln!(out, "{{\"type\":\"end\",\"charts\":1,\"rules\":0,\"rhs_terms\":0,\"residuals\":0,\"chart_errors\":0,\"seconds\":{:.3},\"vmhwm_kb\":{},\"zero_chart\":true,\"native_descent_not_lower\":[]}}", t_total.elapsed().as_secs_f64(), vmhwm_kb()).map_err(|e| e.to_string())?;
        out.flush().map_err(|e| e.to_string())?;
        outcome.complete = true;
        return Ok(outcome);
    }
    writeln!(
        out,
        "{{\"type\":\"chart\",\"chart\":{},\"guards\":{},\"zero\":false}}",
        q(&label),
        strings(guards.iter())
    )
    .map_err(|e| e.to_string())?;
    out.flush().map_err(|e| e.to_string())?;
    let exact_backend = parse_backend(&opts.exact_backend)?;
    // sym-7master: the chart group, computed before the solver when the symbolic case search gets symmetry rows
    let early_group: Option<super::symmetry::NodeGroup> = if opts.symmetry_symbolic {
        let chart: Vec<bool> = base[..n].to_vec();
        Some(super::symmetry::node_group(presented, &chart)?)
    } else {
        None
    };
    let case_symmetry: Option<Arc<[Vec<Option<(usize, bool)>>]>> = early_group.as_ref().map(|g| {
        g.elements
            .iter()
            .map(|e| e.action.clone())
            .collect::<Vec<_>>()
            .into()
    });
    let config = SectorConfig {
        zero_sectors: Arc::from(zeros.clone()),
        integral_order: order_program(&opts.order, n)?,
        symbolic_exact_backend: exact_backend,
        // the shared numeric solve follows the chosen backend (Astra: it used to default to Sparse regardless)
        numerical_exact_backend: numerical_backend(exact_backend),
        seed_schedule: opts.seed_schedule.schedule(),
        witness_guards: !opts.no_witness_guards,
        case_symmetry,
        ..Default::default()
    };
    let t1 = Instant::now();
    let (solver, derivation) = SectorSolver::with_basis_derivation(&sources, base, config)
        .map_err(|e| format!("solver: {e:?}"))?;
    let nb = solver.basis().len();
    events(&NodeEvent::Prepared {
        basis: nb,
        seconds: t1.elapsed().as_secs_f64(),
    });
    let idx: [usize; N] = *sources.index_variables();
    let sector: [bool; N] = *solver.ordering().sector();
    let template = sources
        .rows()
        .iter()
        .flat_map(|r| r.iter())
        .next()
        .ok_or("empty source system")?
        .coefficient
        .clone();
    // region-restricted run: the region's polynomials in the source system's coefficient map
    let region_polys: Vec<CoefficientPolynomial> = match &opts.region {
        None => Vec::new(),
        Some(conj) => conj
            .iter()
            .map(|e| {
                parse_index_poly(e, &template, &idx)
                    .ok_or_else(|| format!("--region polynomial {e}"))
            })
            .collect::<Result<_, _>>()?,
    };
    crate::solver::region_probe::set_region(
        (!region_polys.is_empty()).then(|| region_polys.clone()),
    );
    if let Some(conj) = &opts.region {
        writeln!(
            qlog,
            "{{\"type\":\"region_run\",\"conjunction\":{}}}",
            strings(conj.iter())
        )
        .map_err(|e| e.to_string())?;
    }
    // certificate file: header, original source rows, basis derivation (replay authority for retained traces)
    let mut cert = std::io::BufWriter::new(
        super::childjob::create_stream(
            &out_dir.join("F.cert.jsonl"),
            super::childjob::Stream::Cert,
        )
        .map_err(|e| format!("cert: {e}"))?,
    );
    write_cert_header(&mut cert, fam, &sources);
    for (k, row) in derivation.rows.iter().enumerate() {
        let entries: Vec<String> = row
            .iter()
            .map(|(m, p)| format!("[{m},{}]", q(&p.to_string())))
            .collect();
        writeln!(
            cert,
            "{{\"type\":\"basis_row\",\"chart\":{},\"k\":{k},\"sources\":[{}]}}",
            q(&label),
            entries.join(",")
        )
        .map_err(|e| e.to_string())?;
    }
    cert.flush().map_err(|e| e.to_string())?;
    // sym-gen: the chart's automorphism group and the node order as linear forms on the chart (cross-checked)
    let symmetry: Option<(super::symmetry::NodeGroup, super::symmetry::ChartForms)> = if opts
        .symmetry
    {
        let chart: Vec<bool> = base[..n].to_vec();
        let t = Instant::now();
        let group = super::symmetry::node_group(presented, &chart)?;
        let forms = super::symmetry::ChartForms::from_order(solver.ordering(), &chart).ok_or(
            "symmetry: the node order is not modelled on this chart (cuts / two-sided axes)",
        )?;
        let bad = forms.mismatches(solver.ordering(), &chart, &group, 20_000);
        if bad > 0 {
            return Err(format!(
                "symmetry: the chart forms disagree with the node order on {bad} of 20000 sampled pairs"
            ));
        }
        writeln!(
            out,
            "{{\"type\":\"symmetry_group\",\"chart\":{},\"seconds\":{:.3},\"forms_checked\":20000,\"group\":{}}}",
            q(&label),
            t.elapsed().as_secs_f64(),
            group.to_json()
        )
        .map_err(|e| e.to_string())?;
        out.flush().map_err(|e| e.to_string())?;
        Some((group, forms))
    } else {
        None
    };
    // sym-gen step 2: the group for exact symmetry rows in the shared bulk solve (no order forms needed)
    let row_group: Option<super::symmetry::NodeGroup> = if !opts.symmetry_rows
        && !opts.symmetry_symbolic
    {
        None
    } else if let Some((g, _)) = symmetry.as_ref() {
        Some(g.clone())
    } else {
        let chart: Vec<bool> = base[..n].to_vec();
        let t = Instant::now();
        let group = match early_group.clone() {
            Some(g) => g,
            None => super::symmetry::node_group(presented, &chart)?,
        };
        writeln!(
            out,
            "{{\"type\":\"symmetry_group\",\"chart\":{},\"seconds\":{:.3},\"rows\":true,\"group\":{}}}",
            q(&label),
            t.elapsed().as_secs_f64(),
            group.to_json()
        )
        .map_err(|e| e.to_string())?;
        out.flush().map_err(|e| e.to_string())?;
        Some(group)
    };
    // a symmetry / zero record for a case (fixed values) the group decides; `None` when it decides nothing
    let sym_record = |spec: &CaseSpec, index: usize, numerical: bool| -> Option<(String, bool)> {
        use super::symmetry::{CaseVerdict, case_verdict, image_pattern, least_image};
        let (group, forms) = symmetry.as_ref()?;
        let fixed = &spec.fixed;
        let verdict = if numerical {
            let key: Vec<i64> = fixed.iter().map(|v| v.unwrap_or(0)).collect();
            match least_image(group, forms, &key) {
                Some((e, _, _)) => CaseVerdict::Lower { element: e },
                None => case_verdict(group, forms, fixed),
            }
        } else {
            case_verdict(group, forms, fixed)
        };
        let (kind, element) = match verdict {
            CaseVerdict::Lower { element } => ("sym", element),
            CaseVerdict::Zero { element } => ("sym_zero", element),
            CaseVerdict::None => return None,
        };
        let e = &group.elements[element];
        let target = format!(
            "I({})",
            fixed
                .iter()
                .enumerate()
                .map(|(i, v)| v.map_or(format!("n{i}"), |x| x.to_string()))
                .collect::<Vec<_>>()
                .join(",")
        );
        let flipped_free =
            (0..fixed.len()).any(|i| fixed[i].is_none() && matches!(e.action[i], Some((_, true))));
        let sign = if flipped_free {
            "key".to_string()
        } else {
            let odd = (0..fixed.len()).fold(false, |odd, i| match (e.action[i], fixed[i]) {
                (Some((_, true)), Some(v)) => odd ^ (v % 2 != 0),
                _ => odd,
            });
            if odd { "-".into() } else { "+".into() }
        };
        let fixed_json: Vec<String> = fixed
            .iter()
            .map(|v| v.map_or("null".to_string(), |x| x.to_string()))
            .collect();
        Some((
            format!(
                "{{\"type\":{},\"chart\":{},\"index\":{index},\"fixed\":[{}],\"equations\":{},\"target\":{},\"image\":{},\"element\":{element},\"action\":{},\"matrix\":{:?},\"sign\":{},\"numerical\":{numerical}}}",
                q(kind),
                q(&label),
                fixed_json.join(","),
                strings(spec.equations.iter()),
                q(&target),
                q(&image_pattern(e, fixed)),
                e.action_json(),
                e.matrix,
                q(&sign)
            ),
            kind == "sym_zero",
        ))
    };
    let limits = CaseIntersectionLimits::default();
    let mut symbolic = SectorSolveOptions::default().symbolic;
    if opts.max_depth.is_some() {
        symbolic.max_depth = opts.max_depth;
    }
    let node_source = opts.order.source_order.clone();
    writeln!(
        qlog,
        "{{\"type\":\"options\",\"node_order\":{},\"basis\":{nb},\"options\":{options_json}}}",
        opts.order.to_json()
    )
    .map_err(|e| e.to_string())?;
    // cover tables (rules of earlier pieces, FirstValid order): streamed rule shapes (case + exceptions only; the
    // coefficients are skipped) or a compact cover index (`rr_node cover-index`) bound to its table
    let mut cover: Vec<CoverEntry<N>> = Vec::new();
    for (path, upto) in &opts.cover_tables {
        let (shapes, head) = super::registry::read_rule_shapes(path, *upto)?;
        // guard-fix D1: cover material of another guard semantics is REFUSED (a pre-fix rule lacks the witness-pole
        // branches, so as a cover it would suppress the child cases this run must open). Header (absent = 1; a cover
        // index carries its table's), and every rule line that carries its own stamp.
        {
            let have = head["guard_version"].as_u64().unwrap_or(1);
            if have != opts.guard_version() as u64 {
                return Err(format!(
                    "cover {path}: guard_version {have} (absent = 1) differs from this run's {}: rule material of other guard semantics is refused (guard-fix D1: a pre-fix rule lacks its witness-pole branches and would suppress child cases); regenerate it with this binary (a cover index: rr_node cover-index), or run with matching --no-witness-guards",
                    opts.guard_version()
                ));
            }
            if let Some(sh) = shapes.iter().find(|sh| {
                sh.guard_version
                    .is_some_and(|g| g != opts.guard_version() as u64)
            }) {
                return Err(format!(
                    "cover {path}: rule {:?} carries guard_version {:?}, this run requires {} (guard-fix D1)",
                    sh.index,
                    sh.guard_version,
                    opts.guard_version()
                ));
            }
        }
        if let Some(ledger) = &terminal_ledger {
            // Astra's declared-terminal check (41299cce) on the shape path: the shapes drop the rule targets, so an
            // active ledger re-streams the FULL table (header + every selected rule's actual target and RHS) before
            // any cover entry is inserted. A compact cover index has no targets: refused (T266-MERGE-NOTE).
            if head["type"] == "cover_index" {
                return Err(format!(
                    "known terminals: cover {path} is a compact cover index (no rule targets); pass the full table"
                ));
            }
            use std::io::BufRead as _;
            let f = std::fs::File::open(path).map_err(|e| format!("cover table {path}: {e}"))?;
            let mut rd = std::io::BufReader::with_capacity(1 << 20, f);
            let mut line = String::new();
            rd.read_line(&mut line)
                .map_err(|e| format!("cover table {path}: {e}"))?;
            let header: serde_json::Value = serde_json::from_str(line.trim_end())
                .map_err(|e| format!("known terminals: cover header: {e}"))?;
            let order: serde_json::Value = serde_json::from_str(&descriptor_json(&opts.order, n))
                .map_err(|e| e.to_string())?;
            let conditions: Vec<String> = sources
                .conditions()
                .iter()
                .map(ToString::to_string)
                .collect();
            if header["family_source"] != presented
                || header["order"] != order
                || header["source_conditions"] != serde_json::json!(conditions)
                || !header["charts"]
                    .as_array()
                    .is_some_and(|charts| charts.iter().any(|c| c == &label))
            {
                return Err(format!(
                    "known terminals: cover {path} family/order/chart/source conditions differ"
                ));
            }
            let mut checked = 0usize;
            loop {
                line.clear();
                if rd
                    .read_line(&mut line)
                    .map_err(|e| format!("cover table {path}: {e}"))?
                    == 0
                {
                    break;
                }
                if !line.starts_with("{\"type\":\"rule\"") {
                    continue;
                }
                let v: serde_json::Value = serde_json::from_str(line.trim_end())
                    .map_err(|e| format!("cover table {path}: {e}"))?;
                if v["index"].as_u64().is_none_or(|i| i as usize >= *upto) {
                    continue;
                }
                if v["chart"] != label {
                    return Err(format!("known terminals: cover {path} rule chart differs"));
                }
                ledger.check_raw_rule(
                    &v,
                    &template,
                    &idx,
                    &sector,
                    n,
                    &format!("cover {path}#{}", v["index"]),
                )?;
                checked += 1;
            }
            if checked != shapes.len() {
                return Err(format!(
                    "known terminals: cover {path}: {checked} rules checked but {} shapes loaded",
                    shapes.len()
                ));
            }
        }
        let (mut loaded, mut skipped) = (0usize, 0usize);
        for sh in shapes {
            let spec = CaseSpec {
                fixed: sh.fixed,
                equations: sh.equations,
            };
            match super::registry::shape_entry::<N>(
                &spec,
                &sh.exceptions,
                &template,
                &idx,
                &sector,
                n,
            ) {
                Some((c, b)) => {
                    cover.push((format!("{path}#{}", sh.index.unwrap_or(0)), c, b));
                    loaded += 1;
                }
                None => skipped += 1,
            }
        }
        writeln!(qlog, "{{\"type\":\"cover_table\",\"path\":{},\"upto\":{upto},\"loaded\":{loaded},\"skipped\":{skipped},\"index\":{}}}", q(path), head["type"] == "cover_index").map_err(|e| e.to_string())?;
    }
    if let Some(path) = &opts.pose_probe {
        return pose_probe_run::<N>(
            path,
            &solver,
            opts,
            &template,
            &idx,
            &sector,
            n,
            nb,
            out_dir,
            &top,
            &fd_momenta,
        );
    }
    // initial cases (ORIGINAL slots -> presented)
    let mut initial: Vec<Case<N>> = Vec::new();
    if opts.initial.is_empty() && opts.initial_presented.is_empty() && opts.pool.is_none() {
        initial.push(Case::from(
            CoordinateCase::new(std::array::from_fn(|i| if i < n { None } else { Some(0) }))
                .map_err(|e| format!("{e:?}"))?,
        ));
    }
    let presented_initial: Vec<CaseSpec> = opts
        .initial
        .iter()
        .map(|spec| {
            let (fixed, equations) = opts.order.present_case(&spec.fixed, &spec.equations);
            CaseSpec { fixed, equations }
        })
        .chain(opts.initial_presented.iter().cloned())
        .collect();
    // what this run owns (F.obligations.json): its initial cases, pool claims added below
    let mut owned: Vec<CaseSpec> = if presented_initial.is_empty() && opts.pool.is_none() {
        vec![CaseSpec {
            fixed: vec![None; n],
            equations: vec![],
        }]
    } else {
        presented_initial.clone()
    };
    // seed-centre: per-case centre offsets (cases in PRESENTED slots) and the global offset, checked once
    let mut seed_centres: Vec<(Case<N>, Vec<Vec<i64>>)> = Vec::new();
    for o in opts
        .seed_centre_offset
        .iter()
        .chain(&opts.seed_centre_extra)
    {
        if o.len() != n {
            return Err(format!("seed_centre_offset arity {} != {n}", o.len()));
        }
    }
    for entry in &opts.case_seed_centres {
        let all: Vec<Vec<i64>> = std::iter::once(entry.seed_centre_offset.clone())
            .chain(entry.extra.iter().cloned())
            .collect();
        if let Some(o) = all.iter().find(|o| o.len() != n) {
            return Err(format!("case seed_centre_offset arity {} != {n}", o.len()));
        }
        if let Some(c) = build_case::<N>(&entry.case, &template, &idx, &sector, n)? {
            seed_centres.push((c, all));
        }
    }
    if opts.seed_centre_offset.is_some()
        || !seed_centres.is_empty()
        || opts.seed_face_floor.is_some()
        || opts.seed_layer_cap.is_some()
    {
        let mut rec = serde_json::json!({
            "seed_centre_offset": opts.seed_centre_offset,
            "seed_centre_extra": opts.seed_centre_extra,
            "case_seed_centres": opts.case_seed_centres,
            "seed_face_floor": opts.seed_face_floor,
            "max_depth": opts.max_depth,
        });
        if let Some(cap) = opts.seed_layer_cap {
            rec["seed_layer_cap"] = serde_json::json!(cap);
        }
        outcome.seed_recipe = Some(rec);
        let cap_field = opts.seed_layer_cap.map_or(String::new(), |c| {
            format!(",\"seed_layer_cap\":{}", serde_json::json!(c))
        });
        writeln!(
            qlog,
            "{{\"type\":\"seed_recipe\",\"seed_centre_offset\":{},\"seed_centre_extra\":{},\"case_seed_centres\":{},\"seed_face_floor\":{}{cap_field}}}",
            serde_json::to_string(&opts.seed_centre_offset).unwrap(),
            serde_json::to_string(&opts.seed_centre_extra).unwrap(),
            serde_json::to_string(&opts.case_seed_centres).unwrap(),
            serde_json::to_string(&opts.seed_face_floor).unwrap()
        )
        .map_err(|e| e.to_string())?;
    }
    for spec in &presented_initial {
        match build_case::<N>(spec, &template, &idx, &sector, n)? {
            Some(c) => {
                writeln!(
                    qlog,
                    "{{\"type\":\"initial\",\"case\":{}}}",
                    case_json(&c, n)
                )
                .map_err(|e| e.to_string())?;
                if prune_subsumed(&mut initial, &c) {
                    initial.push(c);
                }
            }
            None => writeln!(
                qlog,
                "{{\"type\":\"initial_empty\",\"case\":{}}}",
                serde_json::to_string(spec).unwrap()
            )
            .map_err(|e| e.to_string())?,
        }
    }
    // stable key of (presented family, chart, integral order, basis) for the side bank and the shared registry: the
    // source visit order only steers the search, so rules found under another one are valid here (same basis rows)
    // guard-fix D1: a guard_version other than 1 is part of the key (v1 keys stay the legacy key), so banks and
    // shared-registry pieces of different guard semantics never match
    let gv_part = format!("guard_version={}", opts.guard_version());
    let mut key_parts: Vec<&str> = vec![presented, &label];
    let descriptor = descriptor_json(&opts.order, n);
    let nb_text = nb.to_string();
    key_parts.push(&descriptor);
    key_parts.push(&nb_text);
    if opts.guard_version() != 1 {
        key_parts.push(&gv_part);
    }
    let key = format!("{:016x}", fnv64(&key_parts));
    // full keys: math key (presented family, chart, order, full certificate = source rows + basis derivation) and the
    // registry identity (math key + seed schedules + backend + engine version)
    let cert_text =
        std::fs::read_to_string(out_dir.join("F.cert.jsonl")).map_err(|e| format!("cert: {e}"))?;
    let math_key = super::registry::math_key(
        presented,
        &label,
        &descriptor_json(&opts.order, n),
        &super::registry::cert_hash(&cert_text),
    );
    drop(cert_text);
    outcome.math_key = math_key.clone();
    let provenance = serde_json::json!({
        "seed_schedule": opts.seed_schedule,
        "case_seed_schedule": serde_json::to_value(&opts.case_seed_schedule).unwrap_or_default(),
        "case_seed_priority": opts.case_seed_priority,
        "case_seed_rung": serde_json::to_value(&opts.case_seed_rung).unwrap_or_default(),
        "exact_backend": opts.exact_backend,
        "engine": format!("rustred {} node {}", env!("CARGO_PKG_VERSION"), super::registry::REGISTRY_SCHEMA),
    });
    // guard-fix D1 (engine line): registry pieces of other guard semantics never match. The default guard version
    // (GUARD_VERSION) leaves the identity as on the sym-symbolic line (pieces stay joinable); any other version is
    // part of the identity.
    let mut provenance = provenance;
    if opts.guard_version() != GUARD_VERSION {
        provenance["guard_version"] = serde_json::json!(opts.guard_version());
    }
    // seed-centre: in the registry identity only when set (default identities unchanged)
    if let Some(o) = &opts.seed_centre_offset {
        provenance["seed_centre_offset"] = serde_json::json!(o);
    }
    if !opts.seed_centre_extra.is_empty() {
        provenance["seed_centre_extra"] = serde_json::json!(opts.seed_centre_extra);
    }
    if !opts.case_seed_centres.is_empty() {
        provenance["case_seed_centres"] = serde_json::json!(opts.case_seed_centres);
    }
    if let Some(k) = opts.seed_face_floor {
        provenance["seed_face_floor"] = serde_json::json!(k);
    }
    // engine line: the t266-tight layer cap, only when set
    if let Some(cap) = opts.seed_layer_cap {
        provenance["seed_layer_cap"] = serde_json::json!(cap);
    }
    let identity =
        super::registry::hash_parts(&[math_key.as_bytes(), provenance.to_string().as_bytes()]);
    writeln!(qlog, "{{\"type\":\"keys\",\"math_key\":{},\"identity\":{},\"bank_key\":{},\"provenance\":{provenance}}}", q(&math_key), q(&identity), q(&key)).map_err(|e| e.to_string())?;
    let mut registry = match &opts.registry {
        Some((dir, piece)) => Some(super::registry::Registry::open(
            dir,
            piece,
            &identity,
            &math_key,
            provenance.clone(),
        )?),
        None => None,
    };
    let mut pool = match &opts.pool {
        Some(dir) => Some(super::registry::PoolClaimer::open(
            dir,
            registry.as_ref().map_or("solo", |r| r.piece.as_str()),
            &math_key,
        )?),
        None => None,
    };
    let mut pool_claims: Vec<String> = Vec::new();
    let mut bank = if opts.refused_bank {
        let mut f = std::io::BufWriter::new(
            std::fs::File::create(out_dir.join("F.refused.jsonl"))
                .map_err(|e| format!("refused bank: {e}"))?,
        );
        writeln!(
            f,
            "{{\"type\":\"bank_header\",\"schema\":\"rustred.node.refused_bank.v1\",\"key\":{},\"math_key\":{},\"chart\":{},\"order_name\":{},\"basis\":{nb},\"accept_monomials\":{},\"fallback_monomials\":{},\"fallback_after\":{},\"guard_version\":{}}}",
            q(&key),
            q(&math_key),
            q(&label),
            q(&opts.order.name),
            opts.max_rule_monomials.map_or("null".into(), |v| v.to_string()),
            opts.fallback_rule_monomials.map_or("null".into(), |v| v.to_string()),
            opts.fallback_after,
            opts.guard_version()
        )
        .map_err(|e| e.to_string())?;
        f.flush().map_err(|e| e.to_string())?;
        Some(f)
    } else {
        None
    };
    // banked rules of earlier runs adoptable under the current policy: (case, rule line, source label, monomials, dump)
    let mut adoptable: Vec<(
        CaseSpec,
        serde_json::Value,
        String,
        usize,
        Option<PathBuf>,
        usize,
    )> = Vec::new();
    for path in &opts.adopt_banks {
        let text = std::fs::read_to_string(path).map_err(|e| format!("bank {path}: {e}"))?;
        let mut lines = text.lines();
        let header: serde_json::Value = lines
            .next()
            .and_then(|l| serde_json::from_str(l).ok())
            .unwrap_or_default();
        // guard-fix D1: a bank of another guard semantics is never adopted (header absent = 1)
        let bank_gv = header["guard_version"].as_u64().unwrap_or(1);
        if bank_gv != opts.guard_version() as u64 {
            writeln!(qlog, "{{\"type\":\"bank_skipped\",\"path\":{},\"reason\":\"guard_version {bank_gv} (absent = 1) differs from this run's {}\"}}", q(path), opts.guard_version()).map_err(|e| e.to_string())?;
            continue;
        }
        let full_differs = header["math_key"]
            .as_str()
            .is_some_and(|k| k != math_key.as_str());
        if header["key"].as_str() != Some(key.as_str()) || full_differs {
            writeln!(qlog, "{{\"type\":\"bank_skipped\",\"path\":{},\"reason\":\"key differs (other family presentation, chart or order)\"}}", q(path)).map_err(|e| e.to_string())?;
            continue;
        }
        let (mut loaded, mut refused) = (0usize, 0usize);
        for line in lines {
            let Ok(v) = serde_json::from_str::<serde_json::Value>(line) else {
                continue;
            };
            if v["type"] != "refused" || v["rule"].is_null() {
                continue;
            }
            // guard-fix D1: a banked rule line stamped with another guard version is never adopted
            if v["rule"]["guard_version"]
                .as_u64()
                .is_some_and(|g| g != opts.guard_version() as u64)
            {
                refused += 1;
                continue;
            }
            let size = v["monomials"].as_u64().unwrap_or(u64::MAX) as usize;
            let tried = v["variants_tried"].as_u64().unwrap_or(0) as usize;
            let ok = opts.max_rule_monomials.is_none_or(|a| size <= a)
                || (opts.fallback_rule_monomials.is_some_and(|f| size <= f)
                    && tried >= opts.fallback_after);
            if !ok {
                refused += 1;
                continue;
            }
            let dump = Path::new(path)
                .parent()
                .map(|d| d.join(format!("F.refused_dump.b{}.json", v["bank_index"])));
            adoptable.push((
                rule_line_case(&v["rule"]),
                v["rule"].clone(),
                format!("{path}#{}", v["bank_index"]),
                size,
                dump,
                tried,
            ));
            loaded += 1;
        }
        writeln!(qlog, "{{\"type\":\"bank_loaded\",\"path\":{},\"adoptable\":{loaded},\"not_under_policy\":{refused}}}", q(path)).map_err(|e| e.to_string())?;
    }
    let mut pending: Vec<Case<N>> = Vec::new();
    let mut numerical: Vec<CoordinateCase<N>> = Vec::new();
    let mut rules: Vec<SectorRule<N>> = Vec::new();
    // rules adopted from earlier banks (written to the table, kept as cover entries, not as SectorRule values)
    let mut adopted_rules = 0usize;
    // charts of deferred regions / failed branch admissions: open obligations unless covered at the end
    // with their full fields (rule, branch, reason, conjunction, exclusions, dependents, parent) for F.obligations.json
    let mut deferred_charts: Vec<(Case<N>, serde_json::Value)> = Vec::new();
    let mut discarded = 0usize;
    #[allow(clippy::too_many_arguments)]
    fn enqueue<const N: usize>(
        case: Case<N>,
        from: Option<usize>,
        pending: &mut Vec<Case<N>>,
        numerical: &mut Vec<CoordinateCase<N>>,
        rules: &[SectorRule<N>],
        cover: &[CoverEntry<N>],
        idx: &[usize; N],
        sector: &[bool; N],
        limits: CaseIntersectionLimits,
        qlog: &mut impl std::io::Write,
        discarded: &mut usize,
        n: usize,
        union_depth: u32,
        union_count: &mut usize,
        terminal_ledger: Option<&DeclaredTerminalLedger>,
    ) {
        let from = from.map_or("null".into(), |x| x.to_string());
        if terminal_ledger.is_some_and(|ledger| ledger.is_point(&case, n)) {
            let _ = writeln!(
                qlog,
                "{{\"type\":\"declared_terminal\",\"authority\":\"DECLARED_NON_NATIVE\",\"from_rule\":{from},\"case\":{}}}",
                case_json(&case, n)
            );
            return;
        }
        for (label, rc, branches) in cover {
            if covers(rc, branches, &case, idx, sector, limits) {
                let _ = writeln!(
                    qlog,
                    "{{\"type\":\"covered\",\"from_rule\":{from},\"by_cover\":{},\"case\":{}}}",
                    q(label),
                    case_json(&case, n)
                );
                *discarded += 1;
                return;
            }
        }
        for (k, rule) in rules.iter().enumerate() {
            if covers(
                &rule.candidate.case,
                &rule.exceptions.branches,
                &case,
                idx,
                sector,
                limits,
            ) {
                let _ = writeln!(
                    qlog,
                    "{{\"type\":\"covered\",\"from_rule\":{from},\"by_rule\":{k},\"case\":{}}}",
                    case_json(&case, n)
                );
                *discarded += 1;
                return;
            }
        }
        if union_depth > 0 {
            let entries: Vec<(&Case<N>, &[Vec<CoefficientPolynomial>])> = cover
                .iter()
                .map(|(_, c, b)| (c, b.as_slice()))
                .chain(
                    rules
                        .iter()
                        .map(|r| (&r.candidate.case, r.exceptions.branches.as_slice())),
                )
                .collect();
            let mut budget = 200usize;
            if union_covered(
                &case,
                &entries,
                idx,
                sector,
                limits,
                union_depth,
                &mut budget,
            ) {
                let _ = writeln!(
                    qlog,
                    "{{\"type\":\"union_covered\",\"from_rule\":{from},\"case\":{}}}",
                    case_json(&case, n)
                );
                *discarded += 1;
                *union_count += 1;
                return;
            }
        }
        if !prune_subsumed(pending, &case) {
            let _ = writeln!(
                qlog,
                "{{\"type\":\"subsumed_by_pending\",\"from_rule\":{from},\"case\":{}}}",
                case_json(&case, n)
            );
            *discarded += 1;
            return;
        }
        if case.is_numerical() {
            let fixed = *case.face();
            if numerical.contains(&fixed) {
                *discarded += 1;
                return;
            }
            let _ = writeln!(
                qlog,
                "{{\"type\":\"enqueue_numerical\",\"from_rule\":{from},\"case\":{}}}",
                case_json(&case, n)
            );
            numerical.push(fixed);
            return;
        }
        match pending.binary_search_by(|queued| queued.queue_cmp(&case)) {
            Ok(_) => *discarded += 1,
            Err(position) => {
                let _ = writeln!(
                    qlog,
                    "{{\"type\":\"enqueue\",\"from_rule\":{from},\"case\":{}}}",
                    case_json(&case, n)
                );
                pending.insert(position, case);
            }
        }
    }
    initial.sort_unstable_by(Case::queue_cmp);
    for case in initial {
        enqueue(
            case,
            None,
            &mut pending,
            &mut numerical,
            &rules,
            &cover,
            &idx,
            &sector,
            limits,
            &mut qlog,
            &mut discarded,
            n,
            opts.union_cover_depth,
            &mut outcome.union_covered,
            terminal_ledger.as_ref(),
        );
    }
    qlog.flush().map_err(|e| e.to_string())?;
    if let Some(budget) = opts.case_budget_exit {
        start_watchdog(budget, out_dir.join("F.spill.json"));
    }
    let t2 = Instant::now();
    let mut error: Option<String> = None;
    let mut stop: Option<StopReason> = None;
    let (mut terms, mut deferred_regions, mut deferred_branches, mut branches_total) =
        (0usize, 0usize, 0usize, 0usize);
    let rung = opts.symbolic_first_rung && opts.seed_schedule != SeedScheduleName::SymbolicFirst;
    let screen = opts.screen.map(|mut s| {
        if !rung {
            s.symbolic_first_from_rows = None;
        }
        s
    });
    // ---- one case search with the node's source order; the screen may refuse the lift
    struct Searched<const N: usize> {
        candidate: RuleCandidate<N>,
        raw_pivot: Option<Integral<N>>,
        lift_seconds: f64,
        source_order: String,
    }
    // seed-pose rung firings (portfolio measured), read per numeric endpoint
    let rung_fired = std::cell::Cell::new(0usize);
    let search = |case: &Case<N>,
                  so: &SourceOrder,
                  seeds: Option<crate::solver::SeedSchedule>,
                  large_gate: bool,
                  exhausted_rung: bool,
                  shell_rung: bool,
                  options: SearchOptions|
     -> (Result<Searched<N>, SolverError>, Vec<ScreenReport>) {
        // the large-trace "SymbolicFirst rung first" gate applies to the first rung only; later rungs lift such traces
        let screen = screen.map(|mut s| {
            if !large_gate {
                s.symbolic_first_from_rows = None;
            }
            s
        });
        // one search: in-shell seed override, explicit case seed pose (seed-pose rung), screen
        let attempt = |seeds: Option<crate::solver::SeedSchedule>,
                       pose: Option<&CaseSeedSchedule>,
                       screen: Option<LiftScreen>|
         -> (Result<Searched<N>, SolverError>, Vec<ScreenReport>) {
            let mut raw: Option<Integral<N>> = None;
            let mut started: Option<Instant> = None;
            let mut lift = 0.0f64;
            let recipe = seed_recipe_for(opts, &seed_centres, case);
            let result = crate::solver::with_seed_recipe(recipe, || {
                super::screen::with_seed_schedule(seeds, || {
                    super::screen::with_screen(screen, || {
                        let observe = |event: SearchEvent<N>| match event {
                            SearchEvent::ExactStarted {
                                pivot, trace_rows, ..
                            } => {
                                raw = Some(pivot);
                                super::childjob::exact_started(trace_rows);
                                started = Some(Instant::now());
                                eprintln!(
                                    "EVENT {:.3}s ExactStarted rows={trace_rows} pivot={pivot}",
                                    t2.elapsed().as_secs_f64()
                                );
                            }
                            SearchEvent::CanonicalizationStarted { .. } => {
                                if let Some(s) = started {
                                    lift = s.elapsed().as_secs_f64();
                                }
                            }
                            // t178 diagnostics (default off): discovery milestones on stderr
                            SearchEvent::DiscoveryProgress {
                                depth,
                                seeds,
                                rows,
                                discovery,
                            } if discovery_progress_log() => {
                                eprintln!(
                                    "EVENT {:.3}s DiscoveryProgress depth={depth} seeds={seeds} rows={rows} independent={} columns={}",
                                    t2.elapsed().as_secs_f64(),
                                    discovery.map_or(0, |d| d.independent_rows),
                                    discovery.map_or(0, |d| d.columns)
                                );
                            }
                            _ => {}
                        };
                        let resolved = opts
                            .case_seed_priority
                            .map(|priority| {
                                resolve_case_seed_schedule(
                                    priority,
                                    opts.case_seed_schedule.as_ref(),
                                    case,
                                    &top,
                                )
                            })
                            .transpose()?
                            .flatten();
                        // a priority that declines a case (AffineCoupledFirst on a coordinate case) leaves the default path
                        let fallback = if opts.case_seed_priority.is_some() {
                            None
                        } else {
                            opts.case_seed_schedule.as_ref()
                        };
                        // rr-registry's --case-seed-rung (the depth-2 seed-shell rung) applies on its rung only
                        let rung_schedule = if shell_rung {
                            opts.case_seed_rung.as_ref()
                        } else {
                            None
                        };
                        if let Some(schedule) =
                            pose.or(rung_schedule).or(resolved.as_ref()).or(fallback)
                        {
                            let visit = so.visit(nb);
                            solver.solve_case_with_schedule_and_observer(
                                case.clone(),
                                options,
                                visit.as_deref(),
                                schedule,
                                observe,
                            )
                        } else {
                            match so.visit(nb) {
                                None => {
                                    solver.solve_case_with_observer(case.clone(), options, observe)
                                }
                                Some(order) => solver.solve_case_with_source_order_and_observer(
                                    case.clone(),
                                    options,
                                    &order,
                                    observe,
                                ),
                            }
                        }
                    })
                })
            });
            let reports = super::screen::take_reports();
            (
                result.map(|candidate| Searched {
                    candidate,
                    raw_pivot: raw,
                    lift_seconds: lift,
                    source_order: so.label(),
                }),
                reports,
            )
        };
        // automatic seed-pose rung (first variant of a case only; default off)
        if opts.seed_pose_rung && seeds.is_none() && large_gate {
            let visit = so.visit(nb);
            let rung_choice = |known: Option<f64>| {
                choose_seed_pose(
                    &solver,
                    case,
                    options,
                    visit.as_deref(),
                    &top,
                    opts.seed_schedule,
                    opts.seed_pose_keep,
                    known,
                    FixedDotConfig::from_options(opts).with_momenta(Some(fd_momenta.clone())),
                )
            };
            let report = |choice: &SeedPoseChoice, gated: bool| {
                rung_fired.set(rung_fired.get() + 1);
                eprintln!(
                    "EVENT {:.3}s SeedPose chosen={} cost={:.3e} default_cost={:.3e} probes={} probe_seconds={:.2} gated={gated}",
                    t2.elapsed().as_secs_f64(),
                    choice.label,
                    choice.cost,
                    choice.default_cost,
                    choice.probes,
                    choice.seconds
                );
            };
            let Some(trigger) = opts.seed_pose_trigger else {
                // seed-pose agent's rung: measure the default with a separate refuse-all search on every case
                if let Some(choice) = rung_choice(None) {
                    report(&choice, false);
                    if choice.label != "default" {
                        return attempt(
                            Some(choice.in_shell.schedule()),
                            Some(&choice.schedule),
                            screen,
                        );
                    }
                }
                return attempt(seeds, None, screen);
            };
            // seed-rung: the normal search with the seed-pose gate on its screen; only a default relation costing more
            // than the trigger is refused by the gate (its measure is the default's cost) and goes to the portfolio
            let gated = screen.unwrap_or(LiftScreen {
                min_rows: usize::MAX,
                min_target_block: usize::MAX,
                on_heavy: super::screen::HeavyAction::Proceed,
                symbolic_first_from_rows: None,
                sampled_from_rows: None,
                ..LiftScreen::default()
            });
            let gated = LiftScreen {
                pose_gate: Some(trigger.max(0.0) as u64),
                pose_gate_min_rows: opts.seed_pose_min_rows,
                ..gated
            };
            let (result, mut reports) = attempt(seeds, None, Some(gated));
            let gate_cost = reports.iter().find_map(|r| match &r.decision {
                super::screen::ScreenDecision::Refuse(why)
                    if why.starts_with(super::screen::POSE_GATE_REFUSAL) =>
                {
                    Some(r.measure.cost())
                }
                _ => None,
            });
            // seed-rung (numeric endpoints, `exhausted_rung`): a default search that found no relation within the
            // depth is an infinitely costly default: the portfolio poses may seed the missing equations
            let gate_cost = gate_cost.or_else(|| {
                (exhausted_rung && matches!(result, Err(SolverError::SearchExhausted { .. })))
                    .then_some(f64::INFINITY)
            });
            let Some(default_cost) = gate_cost else {
                return (result, reports);
            };
            // the gate's refusal is not a screen verdict on the case: drop it from the reports
            reports.retain(|r| {
                !matches!(&r.decision, super::screen::ScreenDecision::Refuse(why) if why.starts_with(super::screen::POSE_GATE_REFUSAL))
            });
            let (result, more) = match rung_choice(Some(default_cost)) {
                Some(choice) if choice.label != "default" => {
                    report(&choice, true);
                    attempt(
                        Some(choice.in_shell.schedule()),
                        Some(&choice.schedule),
                        screen,
                    )
                }
                choice => {
                    if let Some(c) = choice {
                        report(&c, true);
                    }
                    attempt(seeds, None, screen)
                }
            };
            reports.extend(more);
            return (result, reports);
        }
        attempt(seeds, None, screen)
    };
    // lift ladder of one case: node source order; the SymbolicFirst seed schedule (same presentation); other source
    // orders (source order, seed schedule override, large-trace gate). The seed-pose rung fires on the first rung
    // (large_gate = true). Symbolic cases always use it; numeric endpoints use it when the seed-pose rung is on.
    let lift_ladder = || {
        let mut orders: Vec<(SourceOrder, Option<crate::solver::SeedSchedule>, bool, bool)> =
            vec![(node_source.clone(), None, true, false)];
        if opts.symbolic_first_rung
            && screen.is_some()
            && opts.seed_schedule != SeedScheduleName::SymbolicFirst
        {
            orders.push((
                node_source.clone(),
                Some(crate::solver::SeedSchedule::SymbolicFirst),
                true,
                false,
            ));
        }
        // rr-registry ladder rung: the configured finite seed shells (e.g. depth-2 ShiftShells), same case and order
        if opts.case_seed_rung.is_some() {
            orders.push((node_source.clone(), None, true, true));
        }
        if opts.symbolic_first_rung
            && screen.is_some()
            && opts.seed_schedule != SeedScheduleName::SymbolicFirst
        {
            // SymbolicFirst did not help (e.g. it found a larger relation): the node's own relation, lifted
            orders.push((node_source.clone(), None, false, false));
        }
        for so in &opts.retry_source_orders {
            if !orders.iter().any(|(o, _, _, _)| o == so) {
                orders.push((so.clone(), None, false, false));
            }
        }
        orders
    };
    let mut symbolic_cases = 0usize;
    // child-job resume of an interrupted NUMERIC stage (audit-engjob C1): (endpoints of the whole stage, endpoints the
    // per-point search already exhausted, waiting for the shared solve)
    let mut resumed_numeric: Option<(usize, Vec<CoordinateCase<N>>)> = None;
    // child-job (job mode only): a resumed attempt continues from RESUME.json: its queue and counters, and the
    // committed rules of the copied table prefix as cover entries (rule indices continue after them)
    if let Some(r) = super::childjob::resumed() {
        super::childjob::resume_arm(&mut out, &mut qlog)?;
        let built = |c: &CaseSpec| build_case::<N>(c, &template, &idx, &sector, n);
        pending = r
            .cases
            .iter()
            .map(built)
            .collect::<Result<Vec<_>, _>>()?
            .into_iter()
            .flatten()
            .collect();
        // audit-engjob m8: a numeric endpoint of the wrong arity, out of range or not a valid point is refused (41),
        // never dropped or truncated
        let point = |f: &Vec<i64>| {
            numeric_point::<N>(f, n).unwrap_or_else(|e| {
                super::childjob::fail_input(&format!("resume: numeric endpoint {f:?}: {e}"))
            })
        };
        numerical = r.numerical.iter().map(point).collect();
        resumed_numeric = r
            .numeric_stage
            .as_ref()
            .map(|(total, ex)| (*total, ex.iter().map(point).collect()));
        deferred_charts = r
            .deferred
            .iter()
            .map(|(c, d)| Ok(built(c)?.map(|c| (c, d.clone()))))
            .collect::<Result<Vec<_>, String>>()?
            .into_iter()
            .flatten()
            .collect();
        for v in &r.rules {
            if let Some((c, b)) = parse_cover_entry::<N>(v, &template, &idx, &sector, n) {
                cover.push((format!("resume#{}", v["index"]), c, b));
            }
        }
        adopted_rules = r.rules.len();
        symbolic_cases = r.symbolic_cases;
        [
            terms,
            branches_total,
            discarded,
            deferred_regions,
            deferred_branches,
        ] = r.counters;
        // audit-engjob M1: not-lower rules of the committed prefix are reported again (stop `not_lower` at the end)
        outcome.not_lower = r.not_lower.clone();
    }
    // registry leases (postponed sibling-contained cases, with the lessor's label), cases returned from a lease (never
    // leased again), cases left open by `open_on_refusal`, and the case in flight when the loop breaks
    let mut leased: Vec<(Case<N>, String)> = Vec::new();
    let mut returned: Vec<Case<N>> = Vec::new();
    let mut refused_open: Vec<(Case<N>, serde_json::Value)> = Vec::new();
    // child-job resume: the cases a previous attempt left open by `open_on_refusal`
    if let Some(r) = super::childjob::resumed() {
        for (c, d) in &r.refused_open {
            if let Some(c) = build_case::<N>(c, &template, &idx, &sector, n)? {
                refused_open.push((c, d.clone()));
            }
        }
        outcome.refused_open = refused_open.len();
    }
    let mut in_flight: Option<Case<N>>;
    loop {
        in_flight = None;
        if pending.is_empty() {
            // (a) the next pool item becomes an initial case of this run
            if let Some(p) = pool.as_mut() {
                if let Some(item) = p.claim() {
                    outcome.pool_claims += 1;
                    pool_claims.push(item.id.clone());
                    owned.push(item.case.clone());
                    writeln!(
                        qlog,
                        "{{\"type\":\"pool_claim\",\"id\":{},\"case\":{}}}",
                        q(&item.id),
                        serde_json::to_string(&item.case).unwrap_or_default()
                    )
                    .map_err(|e| e.to_string())?;
                    if let Some(c) = build_case::<N>(&item.case, &template, &idx, &sector, n)? {
                        enqueue(
                            c,
                            None,
                            &mut pending,
                            &mut numerical,
                            &rules,
                            &cover,
                            &idx,
                            &sector,
                            limits,
                            &mut qlog,
                            &mut discarded,
                            n,
                            opts.union_cover_depth,
                            &mut outcome.union_covered,
                            terminal_ledger.as_ref(),
                        );
                    }
                    qlog.flush().map_err(|e| e.to_string())?;
                    continue;
                }
            }
            // (b) leases: wait (bounded) for the lessors to end, then discharge each by a valid-domain proof against
            // every accepted rule, or take it back into the own queue
            if leased.is_empty() {
                break;
            }
            if let Some(reg) = registry.as_mut() {
                let t = Instant::now();
                loop {
                    let (new, foreign) = reg.poll();
                    for f in foreign {
                        writeln!(
                            qlog,
                            "{{\"type\":\"registry_foreign\",\"file\":{}}}",
                            q(&f.display().to_string())
                        )
                        .map_err(|e| e.to_string())?;
                    }
                    for (lbl, sp, ex) in new {
                        if let Some((c, b)) =
                            super::registry::shape_entry::<N>(&sp, &ex, &template, &idx, &sector, n)
                        {
                            cover.push((lbl, c, b));
                        }
                    }
                    if reg.siblings_ended() || t.elapsed().as_secs() >= opts.lease_wait {
                        break;
                    }
                    std::thread::sleep(std::time::Duration::from_secs(10));
                }
            }
            let mut back: Vec<Case<N>> = Vec::new();
            {
                let entries: Vec<(&Case<N>, &[Vec<CoefficientPolynomial>])> = cover
                    .iter()
                    .map(|(_, c, b)| (c, b.as_slice()))
                    .chain(
                        rules
                            .iter()
                            .map(|r| (&r.candidate.case, r.exceptions.branches.as_slice())),
                    )
                    .collect();
                for (c, lessor) in leased.drain(..) {
                    let direct = entries
                        .iter()
                        .any(|(rc, b)| covers(rc, b, &c, &idx, &sector, limits));
                    let mut budget = 400usize;
                    let proved = direct
                        || union_covered(
                            &c,
                            &entries,
                            &idx,
                            &sector,
                            limits,
                            opts.union_cover_depth.max(3),
                            &mut budget,
                        );
                    let kind = if proved {
                        "lease_resolved"
                    } else {
                        "lease_returned"
                    };
                    writeln!(
                        qlog,
                        "{{\"type\":{},\"lessor\":{},\"proof\":{},\"case\":{}}}",
                        q(kind),
                        q(&lessor),
                        q(if direct {
                            "direct"
                        } else if proved {
                            "union"
                        } else {
                            "none"
                        }),
                        case_json(&c, n)
                    )
                    .map_err(|e| e.to_string())?;
                    if let Some(reg) = registry.as_mut() {
                        reg.record(serde_json::json!({"type": kind, "lessor": lessor, "case": case_spec(&c, n)}));
                    }
                    if proved {
                        outcome.leases_resolved += 1;
                        discarded += 1;
                    } else {
                        outcome.leases_returned += 1;
                        back.push(c);
                    }
                }
            }
            for c in back {
                returned.push(c.clone());
                enqueue(
                    c,
                    None,
                    &mut pending,
                    &mut numerical,
                    &rules,
                    &cover,
                    &idx,
                    &sector,
                    limits,
                    &mut qlog,
                    &mut discarded,
                    n,
                    opts.union_cover_depth,
                    &mut outcome.union_covered,
                    terminal_ledger.as_ref(),
                );
            }
            qlog.flush().map_err(|e| e.to_string())?;
            continue;
        }
        if opts.max_cases.is_some_and(|m| symbolic_cases >= m) {
            stop = Some(StopReason::MaxCases {
                pending: pending.len(),
            });
            break;
        }
        let current = pending.remove(0);
        // child-job (job mode only): commit point of the previous case, safe point, start of this one
        if super::childjob::active() {
            match super::childjob::case_boundary(
                &mut out,
                &mut qlog,
                symbolic_cases,
                [
                    terms,
                    branches_total,
                    discarded,
                    deferred_regions,
                    deferred_branches,
                ],
                case_spec(&current, n),
                false,
                &mut || {
                    super::childjob::queue_lists(
                        &pending,
                        &numerical,
                        &deferred_charts,
                        &refused_open,
                        None,
                        n,
                    )
                },
            )? {
                super::childjob::Boundary::Stop(s) => {
                    // the popped case stays open: `current` in RESUME and F.subcases, and in F.obligations
                    in_flight = Some(current.clone());
                    stop = Some(s);
                    break;
                }
                super::childjob::Boundary::Skip => continue,
                super::childjob::Boundary::Go => {}
            }
        }
        // Astra known terminals: a declared literal is never searched (checked before it becomes in flight)
        if terminal_ledger
            .as_ref()
            .is_some_and(|ledger| ledger.is_point(&current, n))
        {
            writeln!(qlog, "{{\"type\":\"declared_terminal\",\"authority\":\"DECLARED_NON_NATIVE\",\"case\":{}}}", case_json(&current, n)).map_err(|e| e.to_string())?;
            continue;
        }
        in_flight = Some(current.clone());
        if opts.case_budget_exit.is_some() {
            let fixed_json = |f: &CoordinateCase<N>| {
                serde_json::to_string(&CaseSpec {
                    fixed: f.fixed()[..n].iter().map(|v| v.map(i64::from)).collect(),
                    equations: vec![],
                })
                .unwrap()
            };
            *WATCH.lock().unwrap() = Some(WatchState {
                started: Some(Instant::now()),
                current: case_json(&current, n),
                pending: pending.iter().map(|c| case_json(c, n)).collect(),
                numerical: numerical.iter().map(fixed_json).collect(),
                rules_done: rules.len() + adopted_rules,
            });
        }
        let spec = case_spec(&current, n);
        // shared registry: sibling pieces' accepted rules become cover entries; the popped case is discharged only by
        // a valid-domain proof (one rule, or the union argument); mere containment in a sibling rule's case leases it
        if let Some(reg) = registry.as_mut() {
            let (new, foreign) = reg.poll();
            for f in foreign {
                writeln!(
                    qlog,
                    "{{\"type\":\"registry_foreign\",\"file\":{}}}",
                    q(&f.display().to_string())
                )
                .map_err(|e| e.to_string())?;
            }
            for (lbl, sp, ex) in new {
                if let Some((c, b)) =
                    super::registry::shape_entry::<N>(&sp, &ex, &template, &idx, &sector, n)
                {
                    cover.push((lbl, c, b));
                }
            }
            let direct = cover
                .iter()
                .find(|(_, c, b)| covers(c, b, &current, &idx, &sector, limits))
                .map(|(l, _, _)| l.clone())
                .or_else(|| {
                    rules
                        .iter()
                        .position(|r| {
                            covers(
                                &r.candidate.case,
                                &r.exceptions.branches,
                                &current,
                                &idx,
                                &sector,
                                limits,
                            )
                        })
                        .map(|k| format!("rule#{k}"))
                });
            let proof = match direct {
                Some(l) => Some(("direct", l)),
                None => {
                    let entries: Vec<(&Case<N>, &[Vec<CoefficientPolynomial>])> = cover
                        .iter()
                        .map(|(_, c, b)| (c, b.as_slice()))
                        .chain(
                            rules
                                .iter()
                                .map(|r| (&r.candidate.case, r.exceptions.branches.as_slice())),
                        )
                        .collect();
                    let mut budget = 200usize;
                    union_covered(
                        &current,
                        &entries,
                        &idx,
                        &sector,
                        limits,
                        opts.union_cover_depth.max(1),
                        &mut budget,
                    )
                    .then(|| ("union", String::new()))
                }
            };
            if let Some((how, lbl)) = proof {
                outcome.registry_covered += 1;
                discarded += 1;
                writeln!(
                    qlog,
                    "{{\"type\":\"covered\",\"by_registry\":{},\"proof\":{},\"case\":{}}}",
                    q(&lbl),
                    q(how),
                    case_json(&current, n)
                )
                .map_err(|e| e.to_string())?;
                continue;
            }
            if opts.registry_lease && !returned.contains(&current) {
                if let Some((lbl, _, _)) = cover.iter().find(|(l, c, _)| {
                    l.starts_with("registry:") && c.contains(&current).unwrap_or(false)
                }) {
                    outcome.leases += 1;
                    writeln!(
                        qlog,
                        "{{\"type\":\"lease\",\"lessor\":{},\"case\":{}}}",
                        q(lbl),
                        case_json(&current, n)
                    )
                    .map_err(|e| e.to_string())?;
                    reg.record(serde_json::json!({"type": "lease", "lessor": lbl, "case": spec}));
                    leased.push((current, lbl.clone()));
                    continue;
                }
            }
        }
        // a banked rule of an earlier run for this very case (same key): adopt it without a search
        if let Some(pos) = adoptable.iter().position(|e| e.0 == spec) {
            let (_, mut line, src, size, dump, tried) = adoptable.swap_remove(pos);
            let index = rules.len() + adopted_rules;
            let over = opts.max_rule_monomials.is_some_and(|a| size > a);
            line["index"] = index.into();
            line["chart"] = label.clone().into();
            line["adopted_from"] = src.clone().into();
            line["monomials"] = size.into();
            line["oversized_fallback"] = over.into();
            let Some((rc, branches)) = parse_cover_entry::<N>(&line, &template, &idx, &sector, n)
            else {
                error = Some(format!("banked rule {src} does not parse"));
                break;
            };
            let rhs_terms = line["rhs"].as_array().map_or(0, |a| a.len());
            if let Some(ledger) = &terminal_ledger {
                if let Err(e) = ledger.check_raw_rule(
                    &line,
                    &template,
                    &idx,
                    &sector,
                    n,
                    &format!("bank {src}"),
                ) {
                    error = Some(e);
                    break;
                }
            }
            writeln!(
                out,
                "{}",
                serde_json::to_string(&line).map_err(|e| e.to_string())?
            )
            .map_err(|e| e.to_string())?;
            out.flush().map_err(|e| e.to_string())?;
            terms += rhs_terms;
            branches_total += branches.len();
            if opts.cand_dump {
                if let Some(text) = dump.as_ref().and_then(|d| std::fs::read_to_string(d).ok()) {
                    let mut dv: serde_json::Value =
                        serde_json::from_str(&text).map_err(|e| format!("bank dump: {e}"))?;
                    dv["rule_index"] = index.into();
                    std::fs::write(
                        out_dir.join(format!("F.cand_dump.r{index}.json")),
                        serde_json::to_string(&dv).unwrap_or_default() + "\n",
                    )
                    .map_err(|e| format!("cand dump: {e}"))?;
                }
            }
            outcome.bank_adopted += 1;
            outcome.max_rule_monomials = outcome.max_rule_monomials.max(size);
            if over {
                outcome.oversized_adopted.push(OversizedRule {
                    index,
                    monomials: size,
                    rhs_terms,
                    variant: format!("bank {src}"),
                    variants_tried: tried,
                    case: spec.clone(),
                });
            }
            writeln!(qlog, "{{\"type\":\"bank_adopted\",\"index\":{index},\"from\":{},\"monomials\":{size},\"case\":{}}}", q(&src), case_json(&current, n)).map_err(|e| e.to_string())?;
            let mut children: Vec<Case<N>> = Vec::new();
            for (bi, branch) in branches.iter().enumerate() {
                let admitted = if opts.gsplit {
                    rc.intersect_many_refined(
                        branch,
                        &idx,
                        &sector,
                        limits,
                        None,
                        UnsupportedBranchPolicy::Defer,
                        CaseRefinement::all(),
                    )
                } else {
                    rc.intersect_many(branch, &idx, &sector, limits)
                };
                match admitted {
                    Ok(r) => {
                        for (di, d) in r.deferred.iter().enumerate() {
                            deferred_regions += 1;
                            deferred_charts
                                .push((d.chart.clone(), region_detail(index, bi, di, d, n)));
                            writeln!(qlog, "{{\"type\":\"deferred_region\",\"rule\":{index},\"branch\":{bi},\"k\":{di},\"reason\":{},\"chart\":{},\"conjunction\":{}}}", q(&format!("{:?}", d.reason)), case_json(&d.chart, n), strings(d.conjunction.iter())).map_err(|e| e.to_string())?;
                        }
                        children.extend(r.cases);
                    }
                    Err(e) => {
                        deferred_branches += 1;
                        deferred_charts.push((rc.clone(), serde_json::json!({"kind": "branch", "rule": index, "branch": bi, "polys": branch.iter().map(|p| p.to_string()).collect::<Vec<_>>(), "error": format!("{e:?}").chars().take(2000).collect::<String>()})));
                        writeln!(qlog, "{{\"type\":\"deferred_branch\",\"rule\":{index},\"branch\":{bi},\"polys\":{},\"error\":{}}}", strings(branch.iter()), q(&format!("{e:?}").chars().take(4000).collect::<String>())).map_err(|e| e.to_string())?;
                    }
                }
            }
            if children.iter().any(|ch| ch == &current) {
                error = Some(format!("NonProgress on {current:?} (banked rule {src})"));
                break;
            }
            if let Some(reg) = registry.as_mut() {
                reg.publish_rule(
                    index,
                    &spec,
                    branches
                        .iter()
                        .map(|b| b.iter().map(|p| p.to_string()).collect())
                        .collect(),
                );
            }
            cover.push((format!("bank:{src}"), rc, branches));
            adopted_rules += 1;
            symbolic_cases += 1;
            for ch in children {
                enqueue(
                    ch,
                    Some(index),
                    &mut pending,
                    &mut numerical,
                    &rules,
                    &cover,
                    &idx,
                    &sector,
                    limits,
                    &mut qlog,
                    &mut discarded,
                    n,
                    opts.union_cover_depth,
                    &mut outcome.union_covered,
                    terminal_ledger.as_ref(),
                );
            }
            qlog.flush().map_err(|e| e.to_string())?;
            continue;
        }
        // sym-gen: a case the chart's group maps strictly lower (or proves zero) is closed by one symmetry record
        if let Some((line, zero)) = sym_record(&spec, rules.len() + adopted_rules, false) {
            writeln!(out, "{line}").map_err(|e| e.to_string())?;
            out.flush().map_err(|e| e.to_string())?;
            writeln!(
                qlog,
                "{{\"type\":\"sym_closed\",\"index\":{},\"zero\":{zero},\"case\":{}}}",
                rules.len() + adopted_rules,
                case_json(&current, n)
            )
            .map_err(|e| e.to_string())?;
            if zero {
                outcome.sym_zeros += 1;
            } else {
                outcome.sym_rules += 1;
            }
            cover.push((
                format!("sym:{}", rules.len() + adopted_rules),
                current.clone(),
                Vec::new(),
            ));
            adopted_rules += 1;
            continue;
        }
        // integer domain before any lift: empty -> no rule needed; finite -> point cases (numeric endpoints), each
        // dropped when an earlier rule covers it (union-aware coverage on finite domains)
        if opts.empty_domain_check && !current.is_numerical() {
            let physical: Vec<bool> = base[..n].to_vec();
            match super::domain::integer_domain(
                &spec.fixed,
                &spec.equations,
                &physical,
                64,
                1 << 16,
            ) {
                super::domain::IntegerDomain::Empty => {
                    outcome.empty_domains += 1;
                    writeln!(
                        qlog,
                        "{{\"type\":\"empty_domain\",\"case\":{}}}",
                        case_json(&current, n)
                    )
                    .map_err(|e| e.to_string())?;
                    continue;
                }
                super::domain::IntegerDomain::Finite(points) => {
                    let mut open = 0usize;
                    for p in &points {
                        let ps = CaseSpec {
                            fixed: p.iter().map(|v| Some(*v)).collect(),
                            equations: vec![],
                        };
                        let Some(pc) = build_case::<N>(&ps, &template, &idx, &sector, n)? else {
                            continue;
                        };
                        let covered = cover
                            .iter()
                            .any(|(_, rc, b)| covers(rc, b, &pc, &idx, &sector, limits))
                            || rules.iter().any(|r| {
                                covers(
                                    &r.candidate.case,
                                    &r.exceptions.branches,
                                    &pc,
                                    &idx,
                                    &sector,
                                    limits,
                                )
                            });
                        if !covered {
                            open += 1;
                            enqueue(
                                pc,
                                None,
                                &mut pending,
                                &mut numerical,
                                &rules,
                                &cover,
                                &idx,
                                &sector,
                                limits,
                                &mut qlog,
                                &mut discarded,
                                n,
                                opts.union_cover_depth,
                                &mut outcome.union_covered,
                                terminal_ledger.as_ref(),
                            );
                        }
                    }
                    outcome.finite_domains += 1;
                    writeln!(qlog, "{{\"type\":\"finite_domain\",\"points\":{},\"uncovered\":{open},\"case\":{}}}", points.len(), case_json(&current, n)).map_err(|e| e.to_string())?;
                    continue;
                }
                _ => {}
            }
        }
        events(&NodeEvent::CaseStarted {
            case: spec.clone(),
            pending: pending.len(),
        });
        writeln!(
            qlog,
            "{{\"type\":\"case_started\",\"t\":{:.3},\"pending\":{},\"case\":{}}}",
            t2.elapsed().as_secs_f64(),
            pending.len(),
            case_json(&current, n)
        )
        .map_err(|e| e.to_string())?;
        qlog.flush().map_err(|e| e.to_string())?;
        let orders = lift_ladder();
        let mut found: Option<Searched<N>> = None;
        let mut refusals: Vec<ScreenReport> = Vec::new();
        let mut tried_large: Vec<bool> = Vec::new();
        // size ruling: the smallest fallback rule (accept < size <= fallback limit) with its guards
        let mut fallback: Option<(usize, Searched<N>, ExceptionalConditions, String, usize)> = None;
        let (mut variants_tried, mut since_fallback) = (0usize, 0usize);
        for (k, (so, seeds, large_gate, shell_rung)) in orders.iter().enumerate() {
            // the ungated retry of the node's relation only matters when a large-trace refusal happened
            if !*large_gate
                && !*shell_rung
                && seeds.is_none()
                && *so == node_source
                && !tried_large.iter().any(|x| *x)
            {
                continue;
            }
            if fallback.is_some() {
                if since_fallback >= opts.fallback_after {
                    break;
                }
                since_fallback += 1;
            }
            variants_tried += 1;
            let vlabel = match (seeds, large_gate) {
                _ if *shell_rung => format!("{}+seed_shells", so.label()),
                (Some(_), _) => format!("{}+symbolic_first", so.label()),
                (None, false) if *so == node_source => format!("{}+lift_large", so.label()),
                _ => so.label(),
            };
            if k > 0 {
                if seeds.is_some() || *shell_rung {
                    outcome.symbolic_first_retries += 1;
                } else {
                    outcome.source_order_retries += 1;
                }
                events(&NodeEvent::RelationRetry {
                    case: spec.clone(),
                    source_order: vlabel.clone(),
                });
                writeln!(
                    qlog,
                    "{{\"type\":\"relation_retry\",\"source_order\":{},\"case\":{}}}",
                    q(&vlabel),
                    case_json(&current, n)
                )
                .map_err(|e| e.to_string())?;
            }
            log_case_seed_schedule(&mut qlog, opts, &current, &top, n)?;
            let (result, reports) = search(
                &current,
                so,
                *seeds,
                *large_gate,
                false,
                *shell_rung,
                symbolic,
            );
            tried_large.push(reports.iter().any(|r| matches!(&r.decision, super::screen::ScreenDecision::Refuse(m) if m.contains("SymbolicFirst rung first"))));
            for r in &reports {
                outcome.screened_lifts += 1;
                events(&NodeEvent::LiftScreened { report: r.clone() });
                writeln!(
                    qlog,
                    "{{\"type\":\"lift_screen\",\"source_order\":{},\"report\":{}}}",
                    q(&vlabel),
                    serde_json::to_string(r).unwrap()
                )
                .map_err(|e| e.to_string())?;
                if let (Some(b), super::screen::ScreenDecision::Refuse(why)) =
                    (bank.as_mut(), &r.decision)
                {
                    // a screen refusal has no rule to keep: bank the report (case, variant, measure) for later policies
                    writeln!(b, "{{\"type\":\"screen_refused\",\"variant\":{},\"reason\":{},\"case\":{},\"report\":{}}}", q(&vlabel), q(why), serde_json::to_string(&spec).unwrap(), serde_json::to_string(r).unwrap()).map_err(|e| e.to_string())?;
                    b.flush().map_err(|e| e.to_string())?;
                    outcome.banked += 1;
                }
            }
            let refused = reports
                .iter()
                .any(|r| matches!(r.decision, super::screen::ScreenDecision::Refuse(_)));
            match result {
                Ok(s)
                    if opts
                        .max_rule_monomials
                        .is_some_and(|b| rule_monomials(&s.candidate) > b) =>
                {
                    let size = rule_monomials(&s.candidate);
                    let rhs_len = s.candidate.rhs.len();
                    let accept = opts.max_rule_monomials.unwrap_or(0);
                    outcome.refused_lifts += 1;
                    outcome.oversized_rules += 1;
                    let in_fallback_range = opts.fallback_rule_monomials.is_some_and(|f| size <= f);
                    let keep = in_fallback_range && fallback.as_ref().is_none_or(|f| size < f.0);
                    let reason = if in_fallback_range {
                        format!(
                            "rule size {size} monomials > accept {accept}; fallback (<= {})",
                            opts.fallback_rule_monomials.unwrap_or(0)
                        )
                    } else {
                        format!(
                            "rule size {size} monomials > fallback limit {}; never adopted",
                            opts.fallback_rule_monomials
                                .map_or("none".to_string(), |f| f.to_string())
                        )
                    };
                    writeln!(qlog, "{{\"type\":\"rule_oversized\",\"source_order\":{},\"monomials\":{size},\"rhs\":{rhs_len},\"fallback\":{keep},\"case\":{}}}", q(&vlabel), case_json(&current, n)).map_err(|e| e.to_string())?;
                    refusals.push(ScreenReport {
                        measure: super::screen::ScreenMeasure {
                            rows: s.candidate.sources.len(),
                            rhs_terms: rhs_len,
                            converged: true,
                            note: format!("lifted rule has {size} coefficient monomials"),
                            ..Default::default()
                        },
                        decision: super::screen::ScreenDecision::Refuse(reason.clone()),
                        heavy: Some("rule size budget".into()),
                    });
                    // guards + table line + candidate dump go to the side bank (the gate used to drop the rule here)
                    let guards = if bank.is_some() || keep {
                        out.flush().map_err(|e| e.to_string())?;
                        qlog.flush().map_err(|e| e.to_string())?;
                        guards_of(
                            &s.candidate,
                            &idx,
                            &sector,
                            opts,
                            out_dir,
                            &format!("bank_g{}", outcome.banked),
                            keep,
                        )
                    } else {
                        Guards::Err("not banked".into())
                    };
                    let (guard_status, exceptions) = match guards {
                        Guards::Ok(x, report, content) => {
                            if content {
                                outcome.content_routes += 1;
                            }
                            (
                                report.map_or("ok".to_string(), |r| format!("ok {r}")),
                                Some(x),
                            )
                        }
                        Guards::Timeout(t) => {
                            (format!("native guard extraction timeout ({t} s)"), None)
                        }
                        Guards::Err(e) => (
                            format!("error {}", e.chars().take(400).collect::<String>()),
                            None,
                        ),
                    };
                    let Searched {
                        mut candidate,
                        raw_pivot,
                        lift_seconds,
                        source_order,
                    } = s;
                    if let Some(b) = bank.as_mut() {
                        let bank_index = outcome.banked;
                        let rule_json = match &exceptions {
                            Some(x) => {
                                let rule = SectorRule {
                                    candidate,
                                    exceptions: x.clone(),
                                    dispatch_policy: Default::default(),
                                };
                                let mut extras = rule_extras(
                                    &solver,
                                    &rule,
                                    raw_pivot,
                                    lift_seconds,
                                    source_order.clone(),
                                    false,
                                );
                                extras.seed_setting =
                                    seed_setting_json(opts, &seed_centres, &rule.candidate.case);
                                extras.oversized_fallback = true;
                                let mut buf: Vec<u8> = Vec::new();
                                write_rule(&mut buf, &label, bank_index, &rule, &extras, n);
                                candidate = rule.candidate;
                                String::from_utf8_lossy(&buf).trim_end().to_string()
                            }
                            None => "null".to_string(),
                        };
                        writeln!(
                            b,
                            "{{\"type\":\"refused\",\"bank_index\":{bank_index},\"reason\":{},\"monomials\":{size},\"rhs\":{rhs_len},\"variant\":{},\"variants_tried\":{variants_tried},\"guards\":{},\"case\":{},\"rule\":{rule_json}}}",
                            q(&reason),
                            q(&vlabel),
                            q(&guard_status),
                            serde_json::to_string(&spec).unwrap()
                        )
                        .map_err(|e| e.to_string())?;
                        b.flush().map_err(|e| e.to_string())?;
                        write_cand_dump_named(
                            &out_dir.join(format!("F.refused_dump.b{bank_index}.json")),
                            bank_index,
                            &candidate,
                            n,
                        )?;
                        outcome.banked += 1;
                    }
                    if let (true, Some(x)) = (keep, exceptions) {
                        fallback = Some((
                            size,
                            Searched {
                                candidate,
                                raw_pivot,
                                lift_seconds,
                                source_order,
                            },
                            x,
                            vlabel.clone(),
                            variants_tried,
                        ));
                        since_fallback = 0;
                    }
                    continue;
                }
                Ok(s) => {
                    found = Some(s);
                    break;
                }
                Err(_) if refused => {
                    outcome.refused_lifts += 1;
                    refusals.extend(reports);
                    continue;
                }
                // no relation within the search depth under this variant: the next rung may find one; if none
                // does, the case is refused like a screened lift (an open obligation with `open_on_refusal`)
                Err(SolverError::SearchExhausted { depth, rows }) if opts.open_on_refusal => {
                    outcome.refused_lifts += 1;
                    writeln!(
                        qlog,
                        "{{\"type\":\"search_exhausted\",\"source_order\":{},\"depth\":{depth},\"rows\":{rows},\"case\":{}}}",
                        q(&vlabel),
                        case_json(&current, n)
                    )
                    .map_err(|e| e.to_string())?;
                    refusals.push(ScreenReport {
                        measure: super::screen::ScreenMeasure {
                            rows,
                            note: format!("search exhausted at depth {depth} ({vlabel})"),
                            ..Default::default()
                        },
                        decision: super::screen::ScreenDecision::Refuse(format!(
                            "search exhausted at depth {depth}"
                        )),
                        heavy: None,
                    });
                    continue;
                }
                // t266-3: in a region-restricted run the discovery point lies on the region (a special locus), where
                // the rows selected modularly can fail to reproduce the target pivot exactly; that is a failure of
                // this variant's point, not of the run: try the next variant (another seed/pose), and refuse the case
                // (an open obligation) when none succeeds, instead of stopping the whole run
                Err(SolverError::ExactReplay(msg))
                    if opts.region.is_some() && opts.open_on_refusal =>
                {
                    outcome.refused_lifts += 1;
                    writeln!(
                        qlog,
                        "{{\"type\":\"region_exact_replay_failed\",\"source_order\":{},\"error\":{},\"case\":{}}}",
                        q(&vlabel),
                        q(&msg.chars().take(400).collect::<String>()),
                        case_json(&current, n)
                    )
                    .map_err(|e| e.to_string())?;
                    refusals.push(ScreenReport {
                        measure: super::screen::ScreenMeasure {
                            note: format!(
                                "exact replay failed at the region point ({vlabel}): {msg}"
                            ),
                            ..Default::default()
                        },
                        decision: super::screen::ScreenDecision::Refuse(
                            "exact replay failed at the region point".into(),
                        ),
                        heavy: None,
                    });
                    continue;
                }
                Err(e) => {
                    error = Some(format!("search error on {current:?}: {e:?}"));
                    writeln!(
                        qlog,
                        "{{\"type\":\"search_error\",\"case\":{},\"error\":{}}}",
                        case_json(&current, n),
                        q(&format!("{e:?}").chars().take(4000).collect::<String>())
                    )
                    .map_err(|e| e.to_string())?;
                    break;
                }
            }
        }
        if error.is_some() {
            break;
        }
        // closure first, size second: no acceptable rule, but a fallback within the ruling's 1M-10M band
        let mut pre_exceptions: Option<ExceptionalConditions> = None;
        let mut oversized_flag: Option<OversizedRule> = None;
        if found.is_none() {
            if let Some((size, searched, x, variant, tried)) = fallback.take() {
                writeln!(qlog, "{{\"type\":\"fallback_adopted\",\"monomials\":{size},\"variant\":{},\"variants_tried\":{variants_tried},\"case\":{}}}", q(&variant), case_json(&current, n)).map_err(|e| e.to_string())?;
                oversized_flag = Some(OversizedRule {
                    index: 0,
                    monomials: size,
                    rhs_terms: searched.candidate.rhs.len(),
                    variant,
                    variants_tried: variants_tried.max(tried),
                    case: spec.clone(),
                });
                pre_exceptions = Some(x);
                found = Some(searched);
            }
        }
        let Some(searched) = found else {
            writeln!(
                qlog,
                "{{\"type\":\"lift_refused\",\"case\":{}}}",
                case_json(&current, n)
            )
            .map_err(|e| e.to_string())?;
            if opts.open_on_refusal {
                outcome.refused_open += 1;
                refused_open.push((
                    current.clone(),
                    serde_json::json!({"reason": "lift refused", "reports": serde_json::to_value(&refusals).unwrap_or_default()}),
                ));
                continue;
            }
            stop = Some(StopReason::LiftRefused {
                case: spec.clone(),
                reports: refusals,
            });
            break;
        };
        symbolic_cases += 1;
        let candidate = searched.candidate;
        let nl: Vec<String> = candidate
            .rhs
            .iter()
            .filter(|t| solver.ordering().compare(&candidate.target, &t.integral) != Ordering::Less)
            .map(|t| t.integral.to_string())
            .collect();
        if !nl.is_empty() {
            error = Some(format!(
                "winning rule is not strictly descending on {current:?}"
            ));
            super::childjob::note_not_lower(&nl);
            outcome.not_lower.extend(nl);
            break;
        }
        // guard extraction: native, or a fork-timed native try that falls back to the exact content route
        // (rr-5l-api GuardRoute::NativeThenContent); without the fallback a native timeout stops the node
        let te = Instant::now();
        let exceptions: ExceptionalConditions = if let Some(x) = pre_exceptions.take() {
            x
        } else if let Some(secs) = opts.native_try {
            out.flush().map_err(|e| e.to_string())?;
            qlog.flush().map_err(|e| e.to_string())?;
            if opts.guard_content_fallback {
                let route = GuardRoute::NativeThenContent {
                    native_timeout: std::time::Duration::from_secs(secs),
                };
                match extract_exceptions_routed(&candidate, &idx, &sector, &[false; N], route) {
                    Ok((x, report)) => {
                        note_guard_report(&report, secs, Some(rules.len() + adopted_rules));
                        if report.route == GuardRouteTaken::Content {
                            outcome.content_routes += 1;
                        }
                        writeln!(
                            qlog,
                            "{{\"type\":\"guard\",\"rule\":{},\"report\":{}}}",
                            rules.len() + adopted_rules,
                            q(&format!("{report:?}"))
                        )
                        .map_err(|e| e.to_string())?;
                        x
                    }
                    Err(e) => {
                        error = Some(format!("exception extraction on {current:?}: {e:?}"));
                        break;
                    }
                }
            } else {
                let r = forked(
                    out_dir,
                    &format!("guard_r{}", rules.len() + adopted_rules),
                    secs,
                    || match extract_exceptions(&candidate, &idx, &sector) {
                        Ok(_) => "ok".to_string(),
                        Err(e) => format!("err {e:?}"),
                    },
                );
                note_forked(
                    &r,
                    super::childjob::TimeWhat::GuardTimeout,
                    secs,
                    Some(rules.len() + adopted_rules),
                    if opts.open_on_refusal {
                        "case left open (open-on-refusal)"
                    } else {
                        "node stopped (guard_timeout)"
                    },
                );
                if !matches!(r, Forked::Done(..)) {
                    writeln!(qlog, "{{\"type\":\"guard_timeout\",\"rule\":{},\"case\":{},\"rhs\":{},\"timeout\":{secs}}}", rules.len() + adopted_rules, case_json(&current, n), candidate.rhs.len()).map_err(|e| e.to_string())?;
                    if opts.open_on_refusal {
                        outcome.refused_open += 1;
                        refused_open.push((
                            current.clone(),
                            serde_json::json!({"reason": format!("guard extraction timeout ({secs} s)"), "rhs": candidate.rhs.len()}),
                        ));
                        continue;
                    }
                    stop = Some(StopReason::GuardTimeout {
                        case: spec.clone(),
                        rhs: candidate.rhs.len(),
                    });
                    break;
                }
                match extract_exceptions(&candidate, &idx, &sector) {
                    Ok(x) => x,
                    Err(e) => {
                        error = Some(format!("exception extraction on {current:?}: {e:?}"));
                        break;
                    }
                }
            }
        } else {
            match extract_exceptions(&candidate, &idx, &sector) {
                Ok(x) => x,
                Err(e) => {
                    error = Some(format!("exception extraction on {current:?}: {e:?}"));
                    break;
                }
            }
        };
        let rule = SectorRule {
            candidate,
            exceptions,
            dispatch_policy: Default::default(),
        };
        let index = rules.len() + adopted_rules;
        let mut extras = rule_extras(
            &solver,
            &rule,
            searched.raw_pivot,
            searched.lift_seconds,
            searched.source_order.clone(),
            false,
        );
        extras.seed_setting = seed_setting_json(opts, &seed_centres, &rule.candidate.case);
        outcome.max_rule_monomials = outcome.max_rule_monomials.max(extras.monomials);
        if let Some(mut flag) = oversized_flag.take() {
            extras.oversized_fallback = true;
            flag.index = index;
            outcome.oversized_adopted.push(flag);
        }
        if outcome.rule_scores.len() < 64 {
            let score = RuleScore {
                trace_rows: extras.trace_rows,
                lift_seconds: extras.lift_seconds,
                search_seconds: rule.candidate.stats.elapsed.as_secs_f64(),
                rhs_terms: rule.candidate.rhs.len(),
                max_degree: rule_degree(&rule.candidate),
                direct_hit: rule.candidate.stats.direct_hit,
                branches: rule.exceptions.branches.len(),
                monomials: extras.monomials,
            };
            if outcome.first_rule.is_none() {
                outcome.first_rule = Some(score.clone());
            }
            outcome.rule_scores.push(score);
        }
        if let Some(ledger) = &terminal_ledger {
            if let Err(e) = ledger.check_rule(
                &rule.candidate.target,
                &rule.candidate.case,
                &rule.exceptions.branches,
                &idx,
                &sector,
                n,
                &format!("new rule {index}"),
            ) {
                error = Some(e);
                break;
            }
        }
        let rhs_written = write_rule(&mut out, &label, index, &rule, &extras, n);
        terms += rhs_written;
        super::childjob::rule_written(rhs_written, extras.trace_rows, extras.monomials);
        out.flush().map_err(|e| e.to_string())?;
        branches_total += rule.exceptions.branches.len();
        if opts.cand_dump {
            write_cand_dump(out_dir, index, &rule.candidate, n)?;
        }
        let branch_strings: Vec<String> = rule
            .exceptions
            .branches
            .iter()
            .map(|b| strings(b.iter()))
            .collect();
        writeln!(
            qlog,
            "{{\"type\":\"rule\",\"index\":{index},\"case\":{},\"rhs\":{},\"sources\":{},\"trace_rows\":{},\"lift_seconds\":{:.3},\"search_seconds\":{:.3},\"exception_seconds\":{:.3},\"source_order\":{},\"branches\":[{}]}}",
            case_json(&rule.candidate.case, n),
            rule.candidate.rhs.len(),
            rule.candidate.sources.len(),
            extras.trace_rows,
            extras.lift_seconds,
            rule.candidate.stats.elapsed.as_secs_f64(),
            te.elapsed().as_secs_f64(),
            q(&extras.source_order),
            branch_strings.join(",")
        )
        .map_err(|e| e.to_string())?;
        events(&NodeEvent::RuleFound {
            index,
            rhs: rule.candidate.rhs.len(),
            trace_rows: extras.trace_rows,
            lift_seconds: extras.lift_seconds,
            branches: rule.exceptions.branches.len(),
        });
        // exceptional children
        let mut children: Vec<Case<N>> = Vec::new();
        for (bi, branch) in rule.exceptions.branches.iter().enumerate() {
            // region-restricted run: the child lives on branch AND region; record whether the branch vanishes at
            // the region point the search used (false = this branch cannot contain the region's component there)
            let restricted: Vec<CoefficientPolynomial>;
            let branch: &Vec<CoefficientPolynomial> = if region_polys.is_empty() {
                branch
            } else {
                let at = crate::solver::region_probe::vanishes_at_last(branch);
                writeln!(qlog, "{{\"type\":\"region_branch\",\"rule\":{index},\"branch\":{bi},\"vanishes_at_region_point\":{}}}", at.map_or("null".to_string(), |b| b.to_string())).map_err(|e| e.to_string())?;
                restricted = branch.iter().chain(region_polys.iter()).cloned().collect();
                &restricted
            };
            let nonlinear = branch.iter().any(|p| max_total_degree(p) >= 2);
            if let Some(secs) = opts.admit_try.filter(|_| opts.gsplit && nonlinear) {
                out.flush().map_err(|e| e.to_string())?;
                qlog.flush().map_err(|e| e.to_string())?;
                let case_ref = &rule.candidate.case;
                let r = forked(
                    out_dir,
                    &format!("admit_r{index}_b{bi}"),
                    secs,
                    || match case_ref.intersect_many_refined(
                        branch,
                        &idx,
                        &sector,
                        limits,
                        None,
                        UnsupportedBranchPolicy::Defer,
                        CaseRefinement::all(),
                    ) {
                        Ok(_) => "ok".to_string(),
                        Err(e) => format!("err {e:?}"),
                    },
                );
                note_forked(
                    &r,
                    super::childjob::TimeWhat::Admission,
                    secs,
                    Some(index),
                    "deferred region",
                );
                if !matches!(r, Forked::Done(..)) {
                    deferred_regions += 1;
                    deferred_charts.push((rule.candidate.case.clone(), serde_json::json!({"kind": "region", "rule": index, "branch": bi, "reason": format!("AdmissionTimeout({secs} s)"), "conjunction": branch.iter().map(|p| p.to_string()).collect::<Vec<_>>(), "exclusions": [], "parent": case_spec(&rule.candidate.case, n)})));
                    writeln!(qlog, "{{\"type\":\"deferred_region\",\"rule\":{index},\"branch\":{bi},\"reason\":\"AdmissionTimeout({secs} s)\",\"chart\":{},\"conjunction\":{}}}", case_json(&rule.candidate.case, n), strings(branch.iter())).map_err(|e| e.to_string())?;
                    continue;
                }
            }
            let admitted = if opts.gsplit {
                rule.candidate.case.intersect_many_refined(
                    branch,
                    &idx,
                    &sector,
                    limits,
                    None,
                    UnsupportedBranchPolicy::Defer,
                    CaseRefinement::all(),
                )
            } else {
                rule.candidate
                    .case
                    .intersect_many(branch, &idx, &sector, limits)
            };
            match admitted {
                Ok(r) => {
                    for (di, d) in r.deferred.iter().enumerate() {
                        deferred_regions += 1;
                        deferred_charts.push((d.chart.clone(), region_detail(index, bi, di, d, n)));
                        writeln!(qlog, "{{\"type\":\"deferred_region\",\"rule\":{index},\"branch\":{bi},\"k\":{di},\"reason\":{},\"chart\":{},\"conjunction\":{}}}", q(&format!("{:?}", d.reason)), case_json(&d.chart, n), strings(d.conjunction.iter())).map_err(|e| e.to_string())?;
                    }
                    if r.cases.is_empty() && r.deferred.is_empty() {
                        discarded += 1;
                    }
                    for ch in &r.cases {
                        writeln!(
                            qlog,
                            "{{\"type\":\"child\",\"rule\":{index},\"branch\":{bi},\"case\":{}}}",
                            case_json(ch, n)
                        )
                        .map_err(|e| e.to_string())?;
                    }
                    children.extend(r.cases);
                }
                Err(e) => {
                    deferred_branches += 1;
                    deferred_charts.push((rule.candidate.case.clone(), serde_json::json!({"kind": "branch", "rule": index, "branch": bi, "polys": branch.iter().map(|p| p.to_string()).collect::<Vec<_>>(), "error": format!("{e:?}").chars().take(2000).collect::<String>()})));
                    writeln!(qlog, "{{\"type\":\"deferred_branch\",\"rule\":{index},\"branch\":{bi},\"polys\":{},\"error\":{}}}", strings(branch.iter()), q(&format!("{e:?}").chars().take(4000).collect::<String>())).map_err(|e| e.to_string())?;
                }
            }
        }
        if children.iter().any(|ch| ch == &current) {
            error = Some(format!("NonProgress on {current:?}"));
            rules.push(rule);
            break;
        }
        for ch in children {
            enqueue(
                ch,
                Some(index),
                &mut pending,
                &mut numerical,
                &rules,
                &cover,
                &idx,
                &sector,
                limits,
                &mut qlog,
                &mut discarded,
                n,
                opts.union_cover_depth,
                &mut outcome.union_covered,
                terminal_ledger.as_ref(),
            );
        }
        qlog.flush().map_err(|e| e.to_string())?;
        if let Some(reg) = registry.as_mut() {
            reg.publish_rule(
                index,
                &case_spec(&rule.candidate.case, n),
                rule.exceptions
                    .branches
                    .iter()
                    .map(|b| b.iter().map(|p| p.to_string()).collect())
                    .collect(),
            );
        }
        let mut rule = rule;
        if opts.slim_rules {
            // the table holds the rule; the queue only needs its case and exceptional branches
            rule.candidate.rhs = Vec::new();
            rule.candidate.sources = Vec::new();
        }
        rules.push(rule);
    }
    // child-job (job mode only): commit point at the end of the symbolic queue
    if super::childjob::active() {
        let s = super::childjob::queue_done(
            &mut out,
            &mut qlog,
            symbolic_cases,
            [
                terms,
                branches_total,
                discarded,
                deferred_regions,
                deferred_branches,
            ],
            &mut || {
                super::childjob::queue_lists(
                    &pending,
                    &numerical,
                    &deferred_charts,
                    &refused_open,
                    resumed_numeric.as_ref().map(|(t, e)| (*t, e.as_slice())),
                    n,
                )
            },
        )?;
        if stop.is_none() && error.is_none() {
            stop = s;
        }
    }
    *WATCH.lock().unwrap() = None;
    // ---- numeric endpoints: one by one under the node's source order and exact backend; exhausted ones in bulk
    // declared inventory first (Astra known terminals); native residuals are added (and checked) below
    let mut residual_keys: Vec<String> = terminal_ledger.as_ref().map_or_else(Vec::new, |l| {
        l.terminals.iter().map(|t| t.key.clone()).collect()
    });
    if opts.stop_before_numeric && error.is_none() && stop.is_none() {
        let pending = numerical
            .iter()
            .filter(|p| {
                !terminal_ledger
                    .as_ref()
                    .is_some_and(|ledger| ledger.is_point(&Case::from(**p), n))
            })
            .count();
        if pending > 0 {
            writeln!(
                qlog,
                "{{\"type\":\"stop_before_numeric\",\"pending\":{pending},\"searched\":0}}"
            )
            .map_err(|e| e.to_string())?;
            stop = Some(StopReason::BeforeNumeric { pending });
        }
    }
    // numeric endpoints are open obligations unless the numeric phase ran to its end (a stop before the numeric
    // phase leaves them open)
    let numeric_ran = error.is_none() && stop.is_none();
    // endpoints exhausted by the per-point search (bulk-solved after the loop) and the endpoint count of the whole
    // stage; a child-job attempt resumed inside the numeric stage starts from its RESUME (audit-engjob C1)
    let (numeric_total, mut exhausted): (usize, Vec<CoordinateCase<N>>) = resumed_numeric
        .take()
        .unwrap_or_else(|| (numerical.len(), Vec::new()));
    if !opts.stop_before_numeric && numeric_ran && (!numerical.is_empty() || !exhausted.is_empty())
    {
        outcome.numerical_cases = numeric_total;
        let options = SearchOptions {
            max_depth: Some(opts.numeric_depth),
            ..symbolic
        };
        for (point_index, point) in numerical.clone().into_iter().enumerate() {
            // child-job (job mode only): safe point and commit point before each numeric endpoint
            if super::childjob::active() {
                match super::childjob::case_boundary(
                    &mut out,
                    &mut qlog,
                    symbolic_cases,
                    [
                        terms,
                        branches_total,
                        discarded,
                        deferred_regions,
                        deferred_branches,
                    ],
                    case_spec(&Case::from(point), n),
                    true,
                    &mut || {
                        super::childjob::queue_lists(
                            &[],
                            &numerical[point_index + 1..],
                            &deferred_charts,
                            &refused_open,
                            Some((numeric_total, &exhausted)),
                            n,
                        )
                    },
                )? {
                    super::childjob::Boundary::Stop(s) => {
                        stop = Some(s);
                        break;
                    }
                    super::childjob::Boundary::Skip => continue,
                    super::childjob::Boundary::Go => {}
                }
            }
            if terminal_ledger
                .as_ref()
                .is_some_and(|ledger| ledger.is_point(&Case::from(point), n))
            {
                writeln!(qlog, "{{\"type\":\"declared_terminal\",\"authority\":\"DECLARED_NON_NATIVE\",\"key\":{}}}", q(&point.integral().to_string())).map_err(|e| e.to_string())?;
                continue;
            }
            let case = Case::from(point);
            let fixed: Vec<i64> = point.fixed()[..n]
                .iter()
                .map(|v| v.map_or(0, i64::from))
                .collect();
            // seed-rung: each numeric endpoint is its own case for the case-budget watchdog (it used to be off for
            // the whole numerical stage: T183 hung, T270 was killed there)
            if opts.case_budget_exit.is_some() {
                let fixed_json = |f: &CoordinateCase<N>| case_json(&Case::from(*f), n);
                *WATCH.lock().unwrap() = Some(WatchState {
                    started: Some(Instant::now()),
                    current: case_json(&case, n),
                    pending: Vec::new(),
                    numerical: numerical[point_index + 1..]
                        .iter()
                        .map(fixed_json)
                        .collect(),
                    rules_done: rules.len() + adopted_rules,
                });
            }
            // a numeric case may already be covered by a rule found later than its enqueueing (or by a cover /
            // bank / sibling-piece rule)
            if let Some(k) = rules.iter().position(|r| {
                covers(
                    &r.candidate.case,
                    &r.exceptions.branches,
                    &case,
                    &idx,
                    &sector,
                    limits,
                )
            }) {
                writeln!(
                    qlog,
                    "{{\"type\":\"numerical_covered\",\"by_rule\":{k},\"fixed\":{fixed:?}}}"
                )
                .map_err(|e| e.to_string())?;
                continue;
            }
            if let Some((lbl, _, _)) = cover
                .iter()
                .find(|(_, c, b)| covers(c, b, &case, &idx, &sector, limits))
            {
                writeln!(
                    qlog,
                    "{{\"type\":\"numerical_covered\",\"by_cover\":{},\"fixed\":{fixed:?}}}",
                    q(lbl)
                )
                .map_err(|e| e.to_string())?;
                continue;
            }
            // sym-gen: a point the chart's group maps lower (its orbit minimum) or proves zero: one symmetry record
            if let Some((line, zero)) =
                sym_record(&case_spec(&case, n), rules.len() + adopted_rules, true)
            {
                writeln!(out, "{line}").map_err(|e| e.to_string())?;
                out.flush().map_err(|e| e.to_string())?;
                writeln!(
                    qlog,
                    "{{\"type\":\"sym_closed\",\"index\":{},\"zero\":{zero},\"numerical\":true,\"fixed\":{fixed:?}}}",
                    rules.len() + adopted_rules
                )
                .map_err(|e| e.to_string())?;
                if zero {
                    outcome.sym_zeros += 1;
                } else {
                    outcome.sym_numeric += 1;
                }
                cover.push((
                    format!("sym:{}", rules.len() + adopted_rules),
                    case.clone(),
                    Vec::new(),
                ));
                adopted_rules += 1;
                continue;
            }
            // every numeric path honours the node's source order (then the retry source orders) and its exact
            // backend; a point the per-point search exhausts or refuses under all of them goes to the shared solve.
            // seed-rung: with the seed-pose rung on, a numeric endpoint runs the symbolic cases' lift ladder (first
            // rung large-gated: the seed-pose rung and the SymbolicFirst gate fire, then SymbolicFirst, the lifted
            // node relation, the retry source orders); with the rung off the legacy numeric ladder (no gate) is kept.
            let numeric_orders: Vec<(
                SourceOrder,
                Option<crate::solver::SeedSchedule>,
                bool,
                bool,
            )> = if opts.seed_pose_rung {
                lift_ladder()
            } else {
                let mut v: Vec<SourceOrder> = vec![node_source.clone()];
                for so in &opts.retry_source_orders {
                    if !v.contains(so) {
                        v.push(so.clone());
                    }
                }
                v.into_iter().map(|so| (so, None, false, false)).collect()
            };
            let mut tried_large = false;
            let fired_before = rung_fired.get();
            let mut result: Result<Searched<N>, SolverError> = Err(SolverError::SearchExhausted {
                depth: opts.numeric_depth,
                rows: 0,
            });
            for (k, (so, seeds, large_gate, shell_rung)) in numeric_orders.iter().enumerate() {
                // as for symbolic cases: the ungated retry of the node's relation only after a large-trace refusal
                if k > 0 && !*large_gate && seeds.is_none() && *so == node_source && !tried_large {
                    continue;
                }
                let vlabel = match (seeds, large_gate) {
                    (Some(_), _) => format!("{}+symbolic_first", so.label()),
                    (None, false) if k > 0 && *so == node_source => {
                        format!("{}+lift_large", so.label())
                    }
                    _ => so.label(),
                };
                if k > 0 {
                    writeln!(qlog, "{{\"type\":\"relation_retry\",\"numerical\":true,\"source_order\":{},\"fixed\":{fixed:?}}}", q(&vlabel)).map_err(|e| e.to_string())?;
                }
                log_case_seed_schedule(&mut qlog, opts, &case, &top, n)?;
                let (r, reports) =
                    search(&case, so, *seeds, *large_gate, true, *shell_rung, options);
                tried_large |= reports.iter().any(|r| matches!(&r.decision, super::screen::ScreenDecision::Refuse(m) if m.contains("SymbolicFirst rung first")));
                for rep in &reports {
                    outcome.screened_lifts += 1;
                    writeln!(qlog, "{{\"type\":\"lift_screen\",\"numerical\":true,\"source_order\":{},\"report\":{}}}", q(&vlabel), serde_json::to_string(rep).unwrap()).map_err(|e| e.to_string())?;
                }
                let refused = reports
                    .iter()
                    .any(|r| matches!(r.decision, super::screen::ScreenDecision::Refuse(_)));
                match r {
                    Ok(x) => {
                        result = Ok(x);
                        break;
                    }
                    Err(SolverError::SearchExhausted { .. }) => continue,
                    Err(_) if refused => continue,
                    Err(e) => {
                        result = Err(e);
                        break;
                    }
                }
            }
            // seed-rung: deeper per-point searches before the exhausted route (default off)
            if matches!(result, Err(SolverError::SearchExhausted { .. })) {
                for depth in opts.numeric_depth + 1..=opts.numeric_depth_max.unwrap_or(0) {
                    writeln!(qlog, "{{\"type\":\"relation_retry\",\"numerical\":true,\"depth\":{depth},\"source_order\":{},\"fixed\":{fixed:?}}}", q(&node_source.label())).map_err(|e| e.to_string())?;
                    let deeper = SearchOptions {
                        max_depth: Some(depth),
                        ..options
                    };
                    let (r, reports) =
                        search(&case, &node_source, None, false, false, false, deeper);
                    for rep in &reports {
                        outcome.screened_lifts += 1;
                        writeln!(qlog, "{{\"type\":\"lift_screen\",\"numerical\":true,\"depth\":{depth},\"source_order\":{},\"report\":{}}}", q(&node_source.label()), serde_json::to_string(rep).unwrap()).map_err(|e| e.to_string())?;
                    }
                    let refused = reports
                        .iter()
                        .any(|r| matches!(r.decision, super::screen::ScreenDecision::Refuse(_)));
                    match r {
                        Ok(x) => {
                            outcome.numeric_deeper += 1;
                            result = Ok(x);
                            break;
                        }
                        Err(SolverError::SearchExhausted { .. }) => continue,
                        Err(_) if refused => continue,
                        Err(e) => {
                            result = Err(e);
                            break;
                        }
                    }
                }
            }
            if rung_fired.get() > fired_before {
                outcome.numeric_seed_pose += 1;
            }
            match result {
                Ok(searched) => {
                    let candidate = searched.candidate;
                    let nl: Vec<String> = candidate
                        .rhs
                        .iter()
                        .filter(|t| {
                            solver.ordering().compare(&candidate.target, &t.integral)
                                != Ordering::Less
                        })
                        .map(|t| t.integral.to_string())
                        .collect();
                    super::childjob::note_not_lower(&nl);
                    outcome.not_lower.extend(nl);
                    let exceptions = extract_exceptions(&candidate, &idx, &sector)
                        .map_err(|e| format!("numerical exceptions: {e:?}"))?;
                    if !exceptions.branches.is_empty() {
                        error = Some("a fully fixed rule has index exceptions".into());
                        break;
                    }
                    let rule = SectorRule {
                        candidate,
                        exceptions,
                        dispatch_policy: Default::default(),
                    };
                    let index = rules.len() + adopted_rules;
                    let mut extras = rule_extras(
                        &solver,
                        &rule,
                        searched.raw_pivot,
                        searched.lift_seconds,
                        searched.source_order,
                        true,
                    );
                    extras.seed_setting =
                        seed_setting_json(opts, &seed_centres, &rule.candidate.case);
                    if let Some(ledger) = &terminal_ledger {
                        if let Err(e) = ledger.check_rule(
                            &rule.candidate.target,
                            &rule.candidate.case,
                            &rule.exceptions.branches,
                            &idx,
                            &sector,
                            n,
                            &format!("new rule {index}"),
                        ) {
                            error = Some(e);
                            break;
                        }
                    }
                    let rhs_written = write_rule(&mut out, &label, index, &rule, &extras, n);
                    terms += rhs_written;
                    super::childjob::rule_written(rhs_written, extras.trace_rows, extras.monomials);
                    out.flush().map_err(|e| e.to_string())?;
                    if opts.cand_dump {
                        write_cand_dump(out_dir, index, &rule.candidate, n)?;
                    }
                    writeln!(qlog, "{{\"type\":\"rule\",\"index\":{index},\"numerical\":true,\"fixed\":{fixed:?},\"rhs\":{},\"trace_rows\":{},\"lift_seconds\":{:.3},\"source_order\":{}}}", rule.candidate.rhs.len(), extras.trace_rows, extras.lift_seconds, q(&extras.source_order)).map_err(|e| e.to_string())?;
                    events(&NodeEvent::NumericalPoint {
                        fixed,
                        outcome: format!("rule {index}"),
                    });
                    rules.push(rule);
                }
                Err(SolverError::SearchExhausted { .. }) => {
                    events(&NodeEvent::NumericalPoint {
                        fixed: fixed.clone(),
                        outcome: if opts.strict_numeric_search {
                            "exhausted; strict numeric search refuses shared solve".into()
                        } else {
                            "exhausted; shared solve".into()
                        },
                    });
                    writeln!(
                        qlog,
                        "{{\"type\":\"numerical_exhausted\",\"fixed\":{fixed:?}}}"
                    )
                    .map_err(|e| e.to_string())?;
                    exhausted.push(point);
                }
                Err(e) => {
                    error = Some(format!("numerical point {fixed:?}: {e:?}"));
                    break;
                }
            }
        }
        // child-job (job mode only): safe point (d) after the per-point loop, before the shared solve (audit-engjob C1)
        if super::childjob::active() && error.is_none() && stop.is_none() {
            if let Some(s) = super::childjob::numeric_done(
                &mut out,
                &mut qlog,
                symbolic_cases,
                [
                    terms,
                    branches_total,
                    discarded,
                    deferred_regions,
                    deferred_branches,
                ],
                !exhausted.is_empty(),
                &mut || {
                    super::childjob::queue_lists(
                        &[],
                        &[],
                        &deferred_charts,
                        &refused_open,
                        Some((numeric_total, &exhausted)),
                        n,
                    )
                },
            )? {
                stop = Some(s);
            }
        }
        // audit-engjob M2: an interrupted stage never becomes a strict-search refusal
        if error.is_none() && stop.is_none() && !exhausted.is_empty() && opts.strict_numeric_search
        {
            let cases: Vec<CaseSpec> = exhausted
                .iter()
                .map(|point| case_spec(&Case::from(*point), n))
                .collect();
            writeln!(
                qlog,
                "{{\"type\":\"numeric_shared_refused\",\"cases\":{}}}",
                serde_json::to_string(&cases).map_err(|e| e.to_string())?
            )
            .map_err(|e| e.to_string())?;
            stop = Some(StopReason::NumericSearchRefused { cases });
        }
        // seed-rung: the shared bulk solve of the exhausted points is one more case for the watchdog (spill: the
        // first exhausted point is `current`, the others are `numerical`); off again once the numeric stage ends
        if opts.case_budget_exit.is_some() {
            *WATCH.lock().unwrap() = (error.is_none() && stop.is_none() && !exhausted.is_empty())
                .then(|| WatchState {
                    started: Some(Instant::now()),
                    current: case_json(&Case::from(exhausted[0]), n),
                    pending: Vec::new(),
                    numerical: exhausted[1..]
                        .iter()
                        .map(|f| case_json(&Case::from(*f), n))
                        .collect(),
                    rules_done: rules.len() + adopted_rules,
                });
        }
        if error.is_none() && stop.is_none() && !exhausted.is_empty() {
            outcome.numeric_bulk_fallback = exhausted.len();
            let visit = node_source.visit(nb);
            // sym-gen step 2: exact images of a chart key (every chart line >= 1; subsector seeds get no rows)
            let chart_lines: Vec<bool> = base[..n].to_vec();
            let images = |a: &Integral<N>| -> Vec<(Integral<N>, bool)> {
                let Some(group) = row_group.as_ref() else {
                    return Vec::new();
                };
                let p = a.powers();
                if (0..n).any(|i| chart_lines[i] && p[i].value() < 1)
                    || p[..n].iter().any(|x| x.is_symbolic())
                {
                    return Vec::new();
                }
                let key: Vec<i64> = p[..n].iter().map(|x| i64::from(x.value())).collect();
                group
                    .elements
                    .iter()
                    .filter_map(|e| e.image(&key))
                    .filter_map(|(img, negative)| {
                        let mut powers = *p;
                        for i in 0..n {
                            powers[i] = crate::solver::Power::new(false, img[i] as i16).ok()?;
                        }
                        Some((Integral::new(powers), negative))
                    })
                    .collect()
            };
            let images_dyn: Option<&dyn Fn(&Integral<N>) -> Vec<(Integral<N>, bool)>> = row_group
                .as_ref()
                .map(|_| &images as &dyn Fn(&Integral<N>) -> Vec<(Integral<N>, bool)>);
            match solver.solve_numeric_cases_with_symmetry(
                exhausted.clone(),
                options,
                visit.as_deref(),
                images_dyn,
            ) {
                Ok(result) => {
                    outcome.sym_rows = result.stats.symmetry_rows;
                    outcome.sym_rows_independent = result.stats.symmetry_rows_independent;
                    if row_group.is_some() {
                        writeln!(qlog, "{{\"type\":\"sym_rows\",\"rows\":{},\"independent\":{},\"seeds\":{},\"ibp_rows\":{},\"rules\":{},\"residuals\":{}}}", result.stats.symmetry_rows, result.stats.symmetry_rows_independent, result.stats.seeds, result.stats.rows, result.rules.len(), result.residuals.len()).map_err(|e| e.to_string())?;
                    }
                    // Check the entire returned batch before exporting any shared-solve rule.
                    if let Some(ledger) = &terminal_ledger {
                        for candidate in &result.rules {
                            let exceptions =
                                extract_exceptions(candidate, &idx, &sector).map_err(|e| {
                                    format!("known terminals: numerical exceptions: {e:?}")
                                })?;
                            if let Err(e) = ledger.check_rule(
                                &candidate.target,
                                &candidate.case,
                                &exceptions.branches,
                                &idx,
                                &sector,
                                n,
                                "shared numeric result",
                            ) {
                                error = Some(e);
                                break;
                            }
                        }
                        for residual in &result.residuals {
                            if let Err(e) =
                                ledger.accept_native_residual(&residual.integral().to_string())
                            {
                                error = Some(e);
                                break;
                            }
                        }
                    }
                    if error.is_none() {
                        for candidate in result.rules {
                            if candidate
                                .sources
                                .iter()
                                .any(|s| s.basis_row == crate::solver::SYMMETRY_ROW)
                            {
                                outcome.sym_row_rules += 1;
                            }
                            let nl: Vec<String> = candidate
                                .rhs
                                .iter()
                                .filter(|t| {
                                    solver.ordering().compare(&candidate.target, &t.integral)
                                        != Ordering::Less
                                })
                                .map(|t| t.integral.to_string())
                                .collect();
                            super::childjob::note_not_lower(&nl);
                            outcome.not_lower.extend(nl);
                            let exceptions = extract_exceptions(&candidate, &idx, &sector)
                                .map_err(|e| format!("numerical exceptions: {e:?}"))?;
                            let rule = SectorRule {
                                candidate,
                                exceptions,
                                dispatch_policy: Default::default(),
                            };
                            let index = rules.len() + adopted_rules;
                            let extras = rule_extras(
                                &solver,
                                &rule,
                                None,
                                0.0,
                                format!("shared:{}", node_source.label()),
                                true,
                            );
                            if let Some(ledger) = &terminal_ledger {
                                if let Err(e) = ledger.check_rule(
                                    &rule.candidate.target,
                                    &rule.candidate.case,
                                    &rule.exceptions.branches,
                                    &idx,
                                    &sector,
                                    n,
                                    &format!("new rule {index}"),
                                ) {
                                    error = Some(e);
                                    break;
                                }
                            }
                            let rhs_written =
                                write_rule(&mut out, &label, index, &rule, &extras, n);
                            terms += rhs_written;
                            super::childjob::rule_written(
                                rhs_written,
                                extras.trace_rows,
                                extras.monomials,
                            );
                            if opts.cand_dump {
                                write_cand_dump(out_dir, index, &rule.candidate, n)?;
                            }
                            rules.push(rule);
                        }
                        for r in &result.residuals {
                            let key = r.integral().to_string();
                            if let Some(ledger) = &terminal_ledger {
                                if let Err(e) = ledger.accept_native_residual(&key) {
                                    error = Some(e);
                                    break;
                                }
                            } else {
                                residual_keys.push(key);
                            }
                        }
                    }
                }
                Err(e) => error = Some(format!("numerical (shared): {e:?}")),
            }
        }
        *WATCH.lock().unwrap() = None;
    }
    // the numeric phase settled its endpoints only if it ended without an error and without a strict refusal;
    // otherwise every numeric endpoint stays an obligation (re-listing ones that got a rule is redundant, not unsound)
    let numeric_ran = numeric_ran && error.is_none() && stop.is_none();
    // deferred regions are obligations: a region is settled when the final rule set covers its chart
    let mut regions_open: Option<Vec<usize>> = None;
    if error.is_none() && stop.is_none() && !deferred_charts.is_empty() {
        let entries: Vec<(&Case<N>, &[Vec<CoefficientPolynomial>])> = cover
            .iter()
            .map(|(_, c, b)| (c, b.as_slice()))
            .chain(
                rules
                    .iter()
                    .map(|r| (&r.candidate.case, r.exceptions.branches.as_slice())),
            )
            .collect();
        // the covering check intersects charts with exceptional branches, which is where these regions came from:
        // run it in a forked child with a time limit; a timeout leaves every region open (never a silent closure)
        out.flush().map_err(|e| e.to_string())?;
        qlog.flush().map_err(|e| e.to_string())?;
        super::childjob::deferred_check_started();
        let checked = forked(out_dir, "deferred_check", 600, || {
            let flags: Vec<bool> = deferred_charts
                .iter()
                .map(|(chart, _)| {
                    let direct = entries
                        .iter()
                        .any(|(c, b)| covers(c, b, chart, &idx, &sector, limits));
                    let mut budget = 400usize;
                    direct || union_covered(chart, &entries, &idx, &sector, limits, 3, &mut budget)
                })
                .collect();
            serde_json::to_string(&flags).unwrap_or_default()
        });
        note_forked(
            &checked,
            super::childjob::TimeWhat::DeferredCheck,
            600,
            None,
            "every region left open",
        );
        let flags: Vec<bool> = match checked {
            Forked::Done(text, _) => {
                serde_json::from_str(&text).unwrap_or_else(|_| vec![false; deferred_charts.len()])
            }
            _ => vec![false; deferred_charts.len()],
        };
        let open_idx: Vec<usize> = (0..deferred_charts.len())
            .filter(|&k| !flags.get(k).copied().unwrap_or(false))
            .collect();
        let open: Vec<CaseSpec> = open_idx
            .iter()
            .map(|&k| case_spec(&deferred_charts[k].0, n))
            .collect();
        regions_open = Some(open_idx);
        writeln!(
            qlog,
            "{{\"type\":\"deferred_check\",\"regions\":{},\"open\":{}}}",
            deferred_charts.len(),
            open.len()
        )
        .map_err(|e| e.to_string())?;
        outcome.open_deferred = open.len();
        if !open.is_empty() {
            stop = Some(StopReason::OpenRegions {
                regions: deferred_charts.len(),
                covered: deferred_charts.len() - open.len(),
                charts: open,
            });
        }
    }
    if stop.is_none() && !refused_open.is_empty() {
        stop = Some(StopReason::OpenObligations {
            refused: refused_open.len(),
        });
    }
    // ---- F.obligations.json: what this run owns and what stays open (every stop writes the full list)
    {
        use super::registry::{OBLIGATIONS_SCHEMA, Obligation, Obligations};
        let mut open: Vec<Obligation> = Vec::new();
        let ob = |kind: &str, c: &Case<N>, detail: serde_json::Value| Obligation {
            kind: kind.into(),
            case: case_spec(c, n),
            detail,
        };
        let stopped_by: Option<StopReason> = stop
            .clone()
            .or_else(|| error.clone().map(StopReason::Error));
        if stopped_by.is_some() {
            if let Some(c) = &in_flight {
                let detail = match &stopped_by {
                    Some(s) => {
                        serde_json::json!({"stop": serde_json::to_value(s).unwrap_or_default()})
                    }
                    None => serde_json::Value::Null,
                };
                open.push(ob("current", c, detail));
            }
        }
        for (c, d) in &refused_open {
            open.push(ob("refused", c, d.clone()));
        }
        for c in &pending {
            open.push(ob("pending", c, serde_json::Value::Null));
        }
        for (c, lessor) in &leased {
            open.push(ob("leased", c, serde_json::json!({"lessor": lessor})));
        }
        if !numeric_ran {
            // declared terminals (known-terminals ledger) are never obligations
            for p in numerical.iter().filter(|p| {
                !terminal_ledger
                    .as_ref()
                    .is_some_and(|ledger| ledger.is_point(&Case::from(**p), n))
            }) {
                open.push(ob("numerical", &Case::from(*p), serde_json::Value::Null));
            }
            // a resumed numeric stage: its exhausted endpoints are not in `numerical` any more
            for p in exhausted.iter().filter(|p| !numerical.contains(p)) {
                open.push(ob("numerical", &Case::from(*p), serde_json::Value::Null));
            }
        }
        let region_list: Vec<usize> = match &regions_open {
            Some(v) => v.clone(),
            None => (0..deferred_charts.len()).collect(),
        };
        for k in region_list {
            let (c, d) = &deferred_charts[k];
            let kind = d["kind"].as_str().unwrap_or("region").to_string();
            open.push(ob(&kind, c, d.clone()));
        }
        let complete = stopped_by.is_none() && outcome.not_lower.is_empty() && open.is_empty();
        outcome.open_obligations = open.len();
        outcome.state = if complete {
            "complete".into()
        } else {
            "stopped".into()
        };
        let record = Obligations {
            schema: OBLIGATIONS_SCHEMA.into(),
            math_key: math_key.clone(),
            identity: Some(identity.clone()),
            state: outcome.state.clone(),
            stop: stopped_by
                .as_ref()
                .map(|s| serde_json::to_string(s).unwrap_or_default())
                .or_else(|| {
                    (!outcome.not_lower.is_empty())
                        .then(|| "rules not strictly descending".to_string())
                }),
            chart: label.clone(),
            initial: owned.clone(),
            pool_claims: pool_claims.clone(),
            open,
            rules: rules.len() + adopted_rules,
            residual_keys: residual_keys.clone(),
            source: out_dir.display().to_string(),
            region: opts.region.clone(),
        };
        record.write(&out_dir.join("F.obligations.json"))?;
        if let Some(reg) = registry.as_mut() {
            reg.record(serde_json::json!({"type": "end", "state": outcome.state, "open": outcome.open_obligations,
                "rules": rules.len() + adopted_rules}));
        }
    }
    for key in &residual_keys {
        if let Some(ledger) = &terminal_ledger {
            let terminal = ledger
                .terminals
                .iter()
                .find(|t| &t.key == key)
                .ok_or("known terminals: residual set differs")?;
            writeln!(out, "{}", serde_json::json!({"type":"residual", "chart":label, "key":key,
                "authority":"DECLARED_NON_NATIVE", "file":ledger.file, "file_sha256":ledger.file_sha256,
                "sources":terminal.sources})).map_err(|e| e.to_string())?;
        } else {
            writeln!(
                out,
                "{{\"type\":\"residual\",\"chart\":{},\"key\":{}}}",
                q(&label),
                q(key)
            )
            .map_err(|e| e.to_string())?;
        }
    }
    let seconds = t2.elapsed().as_secs_f64();
    if let Some(e) = &error {
        writeln!(
            out,
            "{{\"type\":\"chart_error\",\"chart\":{},\"seconds\":{seconds:.3},\"error\":{}}}",
            q(&label),
            q(&e.chars().take(4000).collect::<String>())
        )
        .map_err(|e| e.to_string())?;
    }
    if let Some(s) = &stop {
        let reason = serde_json::to_string(s).unwrap_or_default();
        writeln!(out, "{{\"type\":\"chart_error\",\"chart\":{},\"seconds\":{seconds:.3},\"error\":{},\"stop\":{reason}}}", q(&label), q("node stopped (see stop)")).map_err(|e| e.to_string())?;
        events(&NodeEvent::Stopped { reason });
    }
    let active_partial = ((terminal_ledger.is_some() || opts.stop_before_numeric)
        && (error.is_some() || stop.is_some() || !outcome.not_lower.is_empty()))
        || super::childjob::partial_end(
            error.is_some() || stop.is_some() || !outcome.not_lower.is_empty(),
        );
    let chart_record = if active_partial {
        "chart_partial"
    } else {
        "chart_end"
    };
    let end_record = if active_partial { "partial" } else { "end" };
    let terminal_note = if let Some(ledger) = &terminal_ledger {
        format!(
            ",\"terminal_ledger\":{},\"terminal_ledger_provenance\":{}",
            q("terminal ledger declared (non-native)"),
            serde_json::to_string(ledger).map_err(|e| e.to_string())?
        )
    } else {
        String::new()
    };
    let mut sym_fields = if opts.symmetry {
        format!(
            ",\"sym_rules\":{},\"sym_numeric\":{},\"sym_zeros\":{}",
            outcome.sym_rules, outcome.sym_numeric, outcome.sym_zeros
        )
    } else {
        String::new()
    };
    if opts.symmetry_rows || opts.symmetry_symbolic {
        sym_fields.push_str(&format!(
            ",\"sym_rows\":{},\"sym_rows_independent\":{},\"sym_row_rules\":{}",
            outcome.sym_rows, outcome.sym_rows_independent, outcome.sym_row_rules
        ));
    }
    writeln!(
        out,
        "{{\"type\":\"{chart_record}\",\"chart\":{},\"seconds\":{seconds:.3},\"rules\":{},\"rhs_terms\":{terms},\"branches\":{branches_total},\"residuals\":{},\"symbolic_cases\":{symbolic_cases},\"numerical_cases\":{},\"discarded_cases\":{discarded},\"finite_case_policy\":\"node-queue\",\"deferred_cases\":0,\"deferred_branches\":{deferred_branches},\"deferred_regions\":{deferred_regions}{sym_fields}}}",
        q(&label),
        rules.len() + adopted_rules,
        residual_keys.len(),
        outcome.numerical_cases
    )
    .map_err(|e| e.to_string())?;
    let chart_errors = usize::from(error.is_some() || stop.is_some());
    // engine line: the registry line's end-record fields (state, open obligations) and harness label only when a
    // registry-line option is in use; a default run's end record is batch 6's (OUTCOME.json and F.obligations.json
    // carry the state in every run)
    let registry_line = opts.registry.is_some()
        || opts.pool.is_some()
        || opts.open_on_refusal
        || !opts.initial_presented.is_empty();
    let (harness, registry_fields) = if registry_line {
        (
            "rr-registry",
            format!(
                ",\"state\":{},\"open_obligations\":{}",
                q(&outcome.state),
                outcome.open_obligations
            ),
        )
    } else {
        ("rr-consolidate", String::new())
    };
    writeln!(
        out,
        "{{\"type\":\"{end_record}\",\"charts\":1,\"rules\":{},\"rhs_terms\":{terms},\"residuals\":{},\"chart_errors\":{chart_errors},\"seconds\":{:.3},\"vmhwm_kb\":{},\"harness\":\"rustred node driver ({harness})\"{registry_fields},\"native_descent_not_lower\":{}{terminal_note}}}",
        rules.len() + adopted_rules,
        residual_keys.len(),
        t_total.elapsed().as_secs_f64(),
        vmhwm_kb(),
        strings(outcome.not_lower.iter())
    )
    .map_err(|e| e.to_string())?;
    out.flush().map_err(|e| e.to_string())?;
    let cert_record = if active_partial {
        "cert_partial"
    } else {
        "cert_end"
    };
    writeln!(cert, "{{\"type\":\"{cert_record}\",\"ok\":0,\"failed\":0,\"skipped\":{},\"weights\":0,\"rules_with_weight_poles\":0,\"seconds\":{:.3},\"vmhwm_kb\":{}}}", rules.len() + adopted_rules, t_total.elapsed().as_secs_f64(), vmhwm_kb()).map_err(|e| e.to_string())?;
    cert.flush().map_err(|e| e.to_string())?;
    writeln!(
        qlog,
        "{{\"type\":\"{end_record}\",\"state\":{},\"open_obligations\":{},\"error\":{},\"rules\":{},\"residuals\":{},\"seconds\":{seconds:.3}}}",
        q(&outcome.state),
        outcome.open_obligations,
        error.as_deref().map_or("null".to_string(), q),
        rules.len() + adopted_rules,
        residual_keys.len()
    )
    .map_err(|e| e.to_string())?;
    qlog.flush().map_err(|e| e.to_string())?;
    if let Some(e) = error {
        stop = Some(StopReason::Error(e));
    }
    outcome.complete = outcome.state == "complete";
    outcome.rules = rules.len() + adopted_rules;
    outcome.rhs_terms = terms;
    outcome.residuals = residual_keys.len();
    outcome.residual_keys = residual_keys;
    outcome.deferred_regions = deferred_regions;
    outcome.deferred_branches = deferred_branches;
    outcome.symbolic_cases = symbolic_cases;
    outcome.stop = stop;
    outcome.seconds = t_total.elapsed().as_secs_f64();
    outcome.vmhwm_kb = vmhwm_kb();
    Ok(outcome)
}

/// A numeric endpoint (all `n` presented slots fixed) from a RESUME list: the arity must be `n`, every value must fit
/// the engine's power type, and the point must be a valid coordinate case (child-job resume, audit-engjob m8).
fn numeric_point<const N: usize>(f: &[i64], n: usize) -> Result<CoordinateCase<N>, String> {
    if f.len() != n {
        return Err(format!("arity {} != {n}", f.len()));
    }
    let mut fixed = [Some(0i16); N];
    for (slot, &v) in fixed.iter_mut().zip(f) {
        *slot = Some(i16::try_from(v).map_err(|_| format!("value {v} out of range"))?);
    }
    CoordinateCase::new(fixed).map_err(|e| format!("{e:?}"))
}

/// Full fields of a deferred region (the queue log keeps chart and conjunction only).
fn region_detail<const N: usize>(
    rule: usize,
    branch: usize,
    k: usize,
    d: &crate::solver::DeferredRegion<N>,
    n: usize,
) -> serde_json::Value {
    let polys = |ps: &[CoefficientPolynomial]| ps.iter().map(|p| p.to_string()).collect::<Vec<_>>();
    let dependents: Vec<serde_json::Value> = d
        .dependents
        .iter()
        .map(|g| serde_json::json!({"axis": g.axis, "numerator": g.numerator.to_string(), "denominator": g.denominator.to_string()}))
        .collect();
    serde_json::json!({
        "kind": "region", "rule": rule, "branch": branch, "k": k, "reason": format!("{:?}", d.reason),
        "conjunction": polys(&d.conjunction), "exclusions": polys(&d.exclusions), "dependents": dependents,
        "parent": case_spec(&d.parent, n),
    })
}

fn rule_extras<const N: usize>(
    solver: &SectorSolver<N>,
    rule: &SectorRule<N>,
    raw: Option<Integral<N>>,
    lift_seconds: f64,
    source_order: String,
    numerical: bool,
) -> RuleExtras {
    let c = &rule.candidate;
    let n = solver.ordering().physical_arity();
    let (raw_pivot, recenter) = if c.case.is_numerical() {
        (None, Some(vec![0i16; n]))
    } else if let Some(p) = raw {
        // canonicalize: target = raw shifted by -raw offsets on symbolic axes; recenter = target - raw offset
        let rec: Vec<i16> = (0..n)
            .map(|i| {
                if p.powers()[i].is_symbolic() {
                    -p.powers()[i].value()
                } else {
                    0
                }
            })
            .collect();
        (
            Some(p.powers()[..n].iter().map(|x| power_byte(*x)).collect()),
            Some(rec),
        )
    } else if c.stats.direct_hit {
        // a direct hit has no ExactStarted event: the recenter of its one-row frame is exact and cheap
        match solver.rule_frame(rule) {
            Ok(frame) => {
                let rec: Vec<i16> = frame.recenter[..n].to_vec();
                let raw: Vec<u8> = (0..n)
                    .map(|i| {
                        let t = c.target.powers()[i];
                        if t.is_symbolic() {
                            power_byte(
                                crate::solver::Power::new(true, t.value() - rec[i]).unwrap_or(t),
                            )
                        } else {
                            power_byte(t)
                        }
                    })
                    .collect();
                (Some(raw), Some(rec))
            }
            Err(_) => (None, None),
        }
    } else {
        (None, None)
    };
    RuleExtras {
        raw_pivot,
        recenter,
        trace_rows: c.stats.exact_trace_rows,
        lift_seconds,
        source_order,
        numerical,
        monomials: rule_monomials(c),
        oversized_fallback: false,
        guard_version: if solver.witness_guards() {
            GUARD_VERSION
        } else {
            1
        },
        seed_setting: None,
    }
}

fn write_cand_dump<const N: usize>(
    dir: &Path,
    index: usize,
    c: &RuleCandidate<N>,
    n: usize,
) -> Result<(), String> {
    write_cand_dump_named(&dir.join(format!("F.cand_dump.r{index}.json")), index, c, n)
}

fn write_cand_dump_named<const N: usize>(
    path: &Path,
    index: usize,
    c: &RuleCandidate<N>,
    n: usize,
) -> Result<(), String> {
    let pivots: Vec<String> = c
        .sources
        .iter()
        .map(|s| {
            format!(
                "{{\"basis_row\":{},\"seed\":{}}}",
                s.basis_row,
                q(&format!("{:?}", s.seed))
            )
        })
        .collect();
    let spec = case_spec(&c.case, n);
    let body = format!(
        "{{\"schema\":\"xstuck.candidate_dump.v1\",\"producer\":\"rustred node driver\",\"rule_index\":{index},\"case\":{},\"case_fixed\":{},\"target\":{},\"rhs_count\":{},\"pivots\":[{}]}}\n",
        serde_json::to_string(&spec).unwrap(),
        serde_json::to_string(&spec.fixed).unwrap(),
        q(&c.target.to_string()),
        c.rhs.len(),
        pivots.join(",")
    );
    std::fs::write(path, body).map_err(|e| format!("cand dump: {e}"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::node::order::NodeOrder;
    use crate::node::screen::{HeavyAction, LiftScreen};

    pub(crate) const SUNSET: &str = "I(\n  name(S),\n  loops(k1,k2),\n  externals(q),\n  dimension(d),\n  prop(D1,(k1)^2,1),\n  prop(D2,(k2)^2,1),\n  prop(D3,(k1+k2-q)^2,1),\n  prop(D4,(k1+q)^2,0),\n  prop(D5,(k2+q)^2,0),\n  gram(q,q,1)\n)\n";

    fn tmp(tag: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("rr-node-test-{tag}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        d
    }

    fn quiet() -> NodeOptions {
        NodeOptions {
            native_try: None,
            admit_try: None,
            ..NodeOptions::default()
        }
    }

    fn known_file(tag: &str, opts: &NodeOptions, points: Vec<Vec<i64>>) -> KnownTerminalsFile {
        let dir = tmp(tag);
        std::fs::create_dir_all(&dir).unwrap();
        let file = dir.join("terminals.json");
        let value = serde_json::json!({
            "schema":"rustred.node.known-terminals.v1", "frame":"ORIGINAL",
            "original_family_source_sha256":terminal_sha256(SUNSET.as_bytes()),
            "node_order":opts.order, "chart_presented":"11100",
            "terminals":points.into_iter().map(|powers| serde_json::json!({"powers":powers,
                "sources":[{"kind":"old-table-master","reference":"genuine-test-control", "sha256":"a".repeat(64)}]})).collect::<Vec<_>>()
        });
        std::fs::write(&file, serde_json::to_vec(&value).unwrap()).unwrap();
        KnownTerminalsFile::read(file).unwrap()
    }

    #[test]
    fn stop_before_numeric_is_opt_in_and_preserves_option_bytes() {
        let legacy = serde_json::to_value(quiet()).unwrap();
        assert!(legacy.get("stop_before_numeric").is_none());
        let read: NodeOptions = serde_json::from_value(legacy.clone()).unwrap();
        assert!(!read.stop_before_numeric);
        assert_eq!(serde_json::to_value(read).unwrap(), legacy);
        let mut explicit_off = legacy.clone();
        explicit_off["stop_before_numeric"] = serde_json::json!(false);
        let read: NodeOptions = serde_json::from_value(explicit_off).unwrap();
        assert_eq!(serde_json::to_value(read).unwrap(), legacy);
        let on = NodeOptions {
            stop_before_numeric: true,
            ..quiet()
        };
        assert_eq!(
            serde_json::to_value(on).unwrap()["stop_before_numeric"],
            true
        );
    }

    #[test]
    fn stop_before_numeric_leaves_every_endpoint_open_without_search() {
        let dir = tmp("before-numeric-points");
        let opts = NodeOptions {
            initial: [1, 2]
                .into_iter()
                .map(|dot| CaseSpec {
                    fixed: vec![Some(dot), Some(1), Some(1), Some(0), Some(0)],
                    equations: vec![],
                })
                .collect(),
            stop_before_numeric: true,
            ..quiet()
        };
        let mut searched = 0;
        let outcome = generate(SUNSET, "sunset.sym", &dir, &opts, &mut |e| {
            if matches!(e, NodeEvent::CaseStarted { .. }) {
                searched += 1;
            }
        })
        .unwrap();
        assert_eq!(searched, 0);
        assert_eq!(outcome.numerical_cases, 0);
        assert_eq!(outcome.numeric_bulk_fallback, 0);
        assert_eq!(outcome.rules, 0);
        assert!(outcome.residual_keys.is_empty());
        assert!(!outcome.complete);
        assert!(matches!(
            outcome.stop,
            Some(StopReason::BeforeNumeric { pending: 2 })
        ));
        // (batch-5 line: no F.obligations.json / open_obligations; the stop reason carries the open count)
        let table = std::fs::read_to_string(&outcome.table).unwrap();
        let records: Vec<serde_json::Value> = table
            .lines()
            .map(|l| serde_json::from_str(l).unwrap())
            .collect();
        assert!(records.iter().any(|r| r["type"] == "partial"));
        assert!(
            !records
                .iter()
                .any(|r| r["type"] == "end" || r["type"] == "residual")
        );
    }

    #[test]
    fn stop_before_numeric_handles_last_symbolic_leaf_without_retainer() {
        let dir = tmp("before-numeric-last-leaf");
        let opts = NodeOptions {
            initial: vec![CaseSpec {
                fixed: vec![None, Some(1), Some(1), Some(0), Some(0)],
                equations: vec![],
            }],
            stop_before_numeric: true,
            max_cases: Some(1),
            ..quiet()
        };
        let outcome = generate(SUNSET, "sunset.sym", &dir, &opts, &mut |_| {}).unwrap();
        assert_eq!(outcome.symbolic_cases, 1);
        assert!(outcome.rules > 0);
        assert_eq!(outcome.numerical_cases, 0);
        assert_eq!(outcome.numeric_bulk_fallback, 0);
        assert!(outcome.residual_keys.is_empty());
        assert!(matches!(
            outcome.stop,
            Some(StopReason::BeforeNumeric { pending: 1 })
        ));
        // (batch-5 line: no F.obligations.json / open_obligations; the stop reason carries the open count)
    }

    #[test]
    fn stop_before_numeric_keeps_declared_terminals_out_of_open_endpoints() {
        let dir = tmp("before-numeric-ledger");
        let mut opts = NodeOptions {
            initial: vec![CaseSpec {
                fixed: vec![Some(1), Some(1), Some(1), Some(0), Some(0)],
                equations: vec![],
            }],
            stop_before_numeric: true,
            ..quiet()
        };
        opts.known_terminals = Some(known_file(
            "before-numeric-known",
            &opts,
            vec![vec![1, 1, 1, 0, 0]],
        ));
        let mut searched = 0;
        let outcome = generate(SUNSET, "sunset.sym", &dir, &opts, &mut |e| {
            if matches!(e, NodeEvent::CaseStarted { .. }) {
                searched += 1;
            }
        })
        .unwrap();
        assert_eq!(searched, 0);
        assert_eq!(outcome.numerical_cases, 0);
        // (batch-5 line: no F.obligations.json / open_obligations; the stop reason carries the open count)
        assert!(outcome.complete);
        assert_eq!(outcome.residual_keys, vec!["I(1,1,1,0,0)"]);
        let table = std::fs::read_to_string(&outcome.table).unwrap();
        assert!(table.contains("DECLARED_NON_NATIVE"));
        assert!(table.contains("terminal ledger declared (non-native)"));
    }

    #[test]
    fn known_terminals_sha256_vectors_and_default_bytes() {
        assert_eq!(
            terminal_sha256(b""),
            "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
        );
        assert_eq!(
            terminal_sha256(b"abc"),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
        assert_eq!(
            terminal_sha256(b"abcdbcdecdefdefgefghfghighijhijkijkljklmklmnlmnomnopnopq"),
            "248d6a61d20638b8e5c026930c3e6039a33ce45964ff2167f6ecedd419db06c1"
        );
        let old = serde_json::to_value(NodeOptions::default()).unwrap();
        assert!(old.get("known_terminals").is_none());
        let round: NodeOptions = serde_json::from_value(old.clone()).unwrap();
        assert!(round.known_terminals.is_none());
        assert_eq!(old, serde_json::to_value(round).unwrap());
        assert!(
            serde_json::to_value(NodeOutcome::default())
                .unwrap()
                .get("terminal_ledger")
                .is_none()
        );
    }

    #[test]
    fn known_terminals_bind_once_and_refuse_malformed_inputs() {
        let opts = NodeOptions {
            order: NodeOrder {
                name: "swapped".into(),
                slot_permutation: Some(vec![0, 1, 2, 4, 3]),
                ..NodeOrder::identity()
            },
            ..quiet()
        };
        let file = known_file("known-bind", &opts, vec![vec![1, 1, 1, 0, -2]]);
        let sector = [true, true, true, false, false];
        let bound = file.bind::<5>(SUNSET, &opts, &sector, 5).unwrap();
        assert_eq!(bound.terminals[0].powers_presented, vec![1, 1, 1, -2, 0]);
        assert_eq!(
            bound.file_sha256,
            terminal_sha256(&std::fs::read(&file.path).unwrap())
        );
        assert!(file.bind::<5>("different", &opts, &sector, 5).is_err());
        assert!(file.bind::<5>(SUNSET, &quiet(), &sector, 5).is_err());
        assert!(file.bind::<5>(SUNSET, &opts, &[true; 5], 5).is_err());
        let raw: serde_json::Value =
            serde_json::from_slice(&std::fs::read(&file.path).unwrap()).unwrap();
        for bad in [
            serde_json::json!({"terminals":[]}),
            serde_json::json!({"frame":"GUESSED"}),
            serde_json::json!({"terminals":[{"powers":[1,1,1,0,0],"sources":[]}]}),
            serde_json::json!({"original_family_source_sha256":"not-a-hash"}),
        ] {
            let mut changed = raw.clone();
            for (k, v) in bad.as_object().unwrap() {
                changed[k] = v.clone();
            }
            std::fs::write(&file.path, serde_json::to_vec(&changed).unwrap()).unwrap();
            assert!(KnownTerminalsFile::read(&file.path).is_err());
        }
        for points in [
            vec![vec![1, 1, 1, 0]],
            vec![vec![1, 0, 1, 0, 0]],
            vec![vec![1, 1, 1, 0, 0]; 2],
        ] {
            let file = known_file("known-bad-key", &opts, points);
            assert!(file.bind::<5>(SUNSET, &opts, &sector, 5).is_err());
        }
    }

    #[test]
    fn known_terminals_real_point_is_never_searched_and_remains_declared() {
        let mut opts = NodeOptions {
            initial: vec![CaseSpec {
                fixed: vec![Some(1), Some(1), Some(1), Some(0), Some(0)],
                equations: vec![],
            }],
            strict_numeric_search: true,
            numeric_depth: 0,
            ..quiet()
        };
        opts.known_terminals = Some(known_file("known-master", &opts, vec![vec![1, 1, 1, 0, 0]]));
        let mut searched = 0;
        let dir = tmp("known-skip");
        let outcome = generate(SUNSET, "sunset.sym", &dir, &opts, &mut |event| {
            if matches!(
                event,
                NodeEvent::NumericalPoint { .. } | NodeEvent::CaseStarted { .. }
            ) {
                searched += 1;
            }
        })
        .unwrap();
        assert!(outcome.complete, "{outcome:?}");
        assert_eq!(
            (
                searched,
                outcome.symbolic_cases,
                outcome.numerical_cases,
                outcome.numeric_bulk_fallback,
                outcome.rules,
                outcome.residuals
            ),
            (0, 0, 0, 0, 0, 1)
        );
        let table = std::fs::read_to_string(&outcome.table).unwrap();
        assert!(table.contains("terminal ledger declared (non-native)"));
        let residual: serde_json::Value = table
            .lines()
            .filter_map(|l| serde_json::from_str(l).ok())
            .find(|v: &serde_json::Value| v["type"] == "residual")
            .unwrap();
        assert_eq!(residual["authority"], "DECLARED_NON_NATIVE");
        assert_eq!(
            residual["file_sha256"],
            outcome.terminal_ledger.as_ref().unwrap().file_sha256
        );
    }

    #[test]
    fn known_terminals_legacy_registry_refuses_before_terminal_skip() {
        let mut opts = quiet();
        opts.known_terminals = Some(known_file(
            "known-registry-ledger",
            &opts,
            vec![vec![1, 1, 1, 0, 0]],
        ));
        opts.registry = Some((tmp("known-registry").display().to_string(), "test".into()));
        opts.unsafe_shared_registry = true;
        opts.initial = vec![CaseSpec {
            fixed: vec![Some(1), Some(1), Some(1), Some(0), Some(0)],
            equations: vec![],
        }];
        let mut events = 0;
        let out = tmp("known-registry-reject");
        let error = generate(SUNSET, "sunset.sym", &out, &opts, &mut |_| {
            events += 1;
        })
        .unwrap_err();
        assert!(error.contains("lacks actual target authority"), "{error}");
        assert_eq!(events, 0);
        assert!(!out.exists());
    }

    #[test]
    fn known_terminals_target_frame_full_guards_and_empty_rhs_conflicts() {
        let opts = quiet();
        let ledger = known_file("known-target", &opts, vec![vec![1, 1, 1, 0, 0]])
            .bind::<5>(SUNSET, &opts, &[true, true, true, false, false], 5)
            .unwrap();
        let family = compile_family(SUNSET).unwrap();
        let sources = SourceSystem::<5>::from_family_with_capacity(&family, true).unwrap();
        let idx = *sources.index_variables();
        let template = &sources.rows()[0].first().unwrap().coefficient;
        let sector = [true, true, true, false, false];
        let face = Case::from(CoordinateCase::<5>::generic());
        let shifted = super::super::descent::parse_key::<5>("I(n0-1,n1,n2,n3,n4)").unwrap();
        // Applies at n0=2, not n0=1. Guard extraction and target recentering cannot be interchanged.
        let guard = parse_index_poly("n0-2", template, &idx).unwrap();
        assert!(
            ledger
                .check_rule(
                    &shifted,
                    &face,
                    &[vec![guard.clone()]],
                    &idx,
                    &sector,
                    5,
                    "protected"
                )
                .is_ok()
        );
        assert!(
            ledger
                .check_rule(&shifted, &face, &[], &idx, &sector, 5, "even zero RHS")
                .unwrap_err()
                .contains("reduces declared key")
        );
        let other = parse_index_poly("n1-2", template, &idx).unwrap();
        assert!(
            ledger
                .check_rule(
                    &shifted,
                    &face,
                    &[vec![guard, other]],
                    &idx,
                    &sector,
                    5,
                    "AND guard"
                )
                .is_err()
        );
        let raw = serde_json::json!({"target":"I(n0-1,n1,n2,n3,n4)","fixed":[null,null,null,null,null],"equations":[],"exceptions":[],"rhs":[]});
        assert!(
            ledger
                .check_raw_rule(&raw, template, &idx, &sector, 5, "adopted zero RHS")
                .is_err()
        );
        let affine = build_case::<5>(
            &CaseSpec {
                fixed: vec![None; 5],
                equations: vec!["n0-3".into()],
            },
            template,
            &idx,
            &sector,
            5,
        )
        .unwrap()
        .unwrap();
        assert!(
            ledger
                .check_rule(
                    &shifted,
                    &affine,
                    &[],
                    &idx,
                    &sector,
                    5,
                    "disjoint equality"
                )
                .is_ok()
        );
        assert!(ledger.accept_native_residual("I(2,1,1,0,0)").is_err());
    }

    #[test]
    fn known_terminals_initial_cover_and_future_numeric_rule_are_errors() {
        let cover_dir = tmp("known-cover-source");
        let cover = generate(SUNSET, "sunset.sym", &cover_dir, &quiet(), &mut |_| {}).unwrap();
        assert!(cover.complete && cover.rules > 0);
        let mut opts = NodeOptions {
            initial: vec![CaseSpec {
                fixed: vec![Some(2), Some(1), Some(1), Some(0), Some(0)],
                equations: vec![],
            }],
            cover_tables: vec![(cover.table.clone(), cover.rules)],
            ..quiet()
        };
        opts.known_terminals = Some(known_file(
            "known-cover-ledger",
            &opts,
            vec![vec![2, 1, 1, 0, 0]],
        ));
        let error = generate(
            SUNSET,
            "sunset.sym",
            &tmp("known-cover-reject"),
            &opts,
            &mut |_| {},
        )
        .unwrap_err();
        assert!(error.contains("reduces declared key"), "{error}");
        // The declared point is not an initial target. A newly discovered generic rule must still protect it.
        opts.cover_tables.clear();
        opts.initial.clear();
        let outcome = generate(
            SUNSET,
            "sunset.sym",
            &tmp("known-future-reject"),
            &opts,
            &mut |_| {},
        )
        .unwrap();
        assert!(!outcome.complete);
        assert!(
            matches!(&outcome.stop,Some(StopReason::Error(e)) if e.contains("reduces declared key")),
            "{outcome:?}"
        );
        let table = std::fs::read_to_string(&outcome.table).unwrap();
        assert!(table.contains("\"type\":\"partial\""));
        assert!(!table.contains("\"type\":\"end\""));
    }

    #[test]
    fn known_terminals_refuse_a_compact_cover_index() {
        // gf-merge control (T266-MERGE-NOTE): a cover index keeps no rule targets, so an active ledger cannot check
        // it and must refuse it; the full table of the same run is accepted by the ledger check.
        let cover = generate(
            SUNSET,
            "sunset.sym",
            &tmp("known-index-source"),
            &quiet(),
            &mut |_| {},
        )
        .unwrap();
        assert!(cover.complete && cover.rules > 0);
        let index = format!("{}.cover.jsonl", tmp("known-index").display());
        super::super::registry::write_cover_index(&cover.table, cover.rules, &index).unwrap();
        let mut opts = NodeOptions {
            initial: vec![CaseSpec {
                fixed: vec![Some(1), Some(1), Some(1), Some(0), Some(0)],
                equations: vec![],
            }],
            cover_tables: vec![(index, cover.rules)],
            ..quiet()
        };
        opts.known_terminals = Some(known_file(
            "known-index-ledger",
            &opts,
            vec![vec![1, 1, 1, 0, 0]],
        ));
        let error = generate(
            SUNSET,
            "sunset.sym",
            &tmp("known-index-reject"),
            &opts,
            &mut |_| {},
        )
        .unwrap_err();
        assert!(error.contains("compact cover index"), "{error}");
        opts.cover_tables = vec![(cover.table.clone(), cover.rules)];
        let outcome = generate(
            SUNSET,
            "sunset.sym",
            &tmp("known-index-full"),
            &opts,
            &mut |_| {},
        )
        .unwrap();
        assert!(outcome.complete, "{outcome:?}");
    }

    #[test]
    fn known_terminals_native_extra_master_refuses_without_fake_end() {
        let mut opts = NodeOptions {
            initial: vec![CaseSpec {
                fixed: vec![Some(1), Some(1), Some(1), Some(0), Some(0)],
                equations: vec![],
            }],
            numeric_depth: 0,
            ..quiet()
        };
        // A deliberately wrong declared inventory must not absorb a native residual from another point.
        opts.known_terminals = Some(known_file(
            "known-extra-ledger",
            &opts,
            vec![vec![2, 1, 1, 0, 0]],
        ));
        let result = generate(
            SUNSET,
            "sunset.sym",
            &tmp("known-extra-reject"),
            &opts,
            &mut |_| {},
        )
        .unwrap();
        assert!(!result.complete);
        assert!(
            matches!(&result.stop,Some(StopReason::Error(e)) if e.contains("outside the declared set")),
            "{result:?}"
        );
        assert_eq!(result.residual_keys, vec!["I(2,1,1,0,0)"]);
    }

    #[test]
    fn sunset_chart_closes_records_order_and_replays() {
        for order in [
            NodeOrder::identity(),
            NodeOrder {
                name: "tie_isp_rev".into(),
                coordinate_priority: Some(vec![0, 1, 2, 4, 3]),
                ..NodeOrder::identity()
            },
        ] {
            let dir = tmp(&format!("sunset-{}", order.name));
            let opts = NodeOptions {
                order: order.clone(),
                ..quiet()
            };
            let out = generate(SUNSET, "sunset.sym", &dir, &opts, &mut |_| {}).unwrap();
            assert!(out.complete, "{:?}", out.stop);
            assert!(out.rules > 0);
            assert_eq!(
                out.residuals, 1,
                "the massless sunset has one master in its top sector"
            );
            // the order is part of the artefact
            let header: serde_json::Value = serde_json::from_str(
                std::fs::read_to_string(&out.table)
                    .unwrap()
                    .lines()
                    .next()
                    .unwrap(),
            )
            .unwrap();
            assert_eq!(crate::node::descent::header_order(&header).unwrap(), order);
            let (_, rep) = crate::node::descent::check_table_file(&out.table, None).unwrap();
            assert!(rep.passed() && rep.rules == out.rules);
            // independent replay of the whole table: every symbolic rule carries its raw pivot authority
            let args = crate::node::replay::ReplayArgs {
                family: None,
                table: PathBuf::from(&out.table),
                cert: Some(dir.join("F.cert.jsonl")),
                cand_dump_dir: None,
                bindings: None,
                rules: None,
                chart: None,
                out: dir.join("replay"),
                dimensions: crate::node::replay::DIMS
                    .iter()
                    .map(|s| s.to_string())
                    .collect(),
                sample_attempts: 128,
                extra_points: 0,
                loci: None,
                box_size: 0,
                box_max: 0,
                dump_weights: false,
                random_d: false,
                single_prime: false,
            };
            let (status, doc) = crate::node::replay::run(&args).unwrap();
            assert_eq!(
                status,
                "PASS_RS2_RS3_SAMPLED",
                "{}",
                serde_json::to_string(&doc["rules"])
                    .unwrap()
                    .chars()
                    .take(2000)
                    .collect::<String>()
            );
            let _ = std::fs::remove_dir_all(&dir);
        }
    }

    #[test]
    fn replay_rejects_a_mutated_table_coefficient() {
        let dir = tmp("mutant");
        let out = generate(SUNSET, "sunset.sym", &dir, &quiet(), &mut |_| {}).unwrap();
        let text = std::fs::read_to_string(&out.table).unwrap();
        // corrupt the first coefficient of the first symbolic rule with at least one RHS term: c -> (c)+1
        let mut done = false;
        let lines: Vec<String> = text
            .lines()
            .map(|l| {
                if done || !l.starts_with("{\"type\":\"rule\"") {
                    return l.to_string();
                }
                let mut v: serde_json::Value = serde_json::from_str(l).unwrap();
                if v["rhs"].as_array().is_none_or(|a| a.is_empty()) {
                    return l.to_string();
                }
                let c = v["rhs"][0][1].as_str().unwrap().to_string();
                v["rhs"][0][1] = serde_json::json!(format!("({c})+1"));
                done = true;
                serde_json::to_string(&v).unwrap()
            })
            .collect();
        assert!(done);
        let bad = dir.join("F.bad.table.jsonl");
        std::fs::write(&bad, lines.join("\n") + "\n").unwrap();
        let args = crate::node::replay::ReplayArgs {
            family: None,
            table: bad,
            cert: None,
            cand_dump_dir: None,
            bindings: None,
            rules: None,
            chart: None,
            out: dir.join("replay-bad"),
            dimensions: crate::node::replay::DIMS
                .iter()
                .map(|s| s.to_string())
                .collect(),
            sample_attempts: 128,
            extra_points: 0,
            loci: None,
            box_size: 0,
            box_max: 0,
            dump_weights: false,
            random_d: false,
            single_prime: false,
        };
        let (status, _) = crate::node::replay::run(&args).unwrap();
        assert_eq!(status, "FAIL");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn screen_refusal_stops_with_the_case_or_reports_nothing() {
        let dir = tmp("screen");
        let screen = LiftScreen {
            min_rows: 1,
            max_total_degree: 0,
            max_rhs_terms: 0,
            on_heavy: HeavyAction::Refuse,
            ..LiftScreen::default()
        };
        let opts = NodeOptions {
            screen: Some(screen),
            ..quiet()
        };
        let out = generate(SUNSET, "sunset.sym", &dir, &opts, &mut |_| {}).unwrap();
        if out.screened_lifts > 0 {
            assert!(out.refused_lifts > 0);
            assert!(
                matches!(out.stop, Some(StopReason::LiftRefused { .. })),
                "{:?}",
                out.stop
            );
            assert!(
                out.symbolic_first_retries + out.source_order_retries > 0,
                "a refused lift is retried (SymbolicFirst, other source orders) first"
            );
        } else {
            assert!(out.complete);
        }
        // a permissive screen changes nothing
        let dir2 = tmp("screen-ok");
        let opts = NodeOptions {
            screen: Some(LiftScreen {
                min_rows: 1,
                ..LiftScreen::default()
            }),
            ..quiet()
        };
        let ok = generate(SUNSET, "sunset.sym", &dir2, &opts, &mut |_| {}).unwrap();
        assert!(ok.complete);
        let plain = generate(SUNSET, "sunset.sym", &tmp("plain"), &quiet(), &mut |_| {}).unwrap();
        assert_eq!((ok.rules, ok.residuals), (plain.rules, plain.residuals));
        let _ = std::fs::remove_dir_all(&dir);
        let _ = std::fs::remove_dir_all(&dir2);
    }

    fn replay_status(dir: &Path, table: &str) -> String {
        let args = crate::node::replay::ReplayArgs {
            family: None,
            table: PathBuf::from(table),
            cert: Some(dir.join("F.cert.jsonl")),
            cand_dump_dir: None,
            bindings: None,
            rules: None,
            chart: None,
            out: dir.join("replay"),
            dimensions: crate::node::replay::DIMS
                .iter()
                .map(|s| s.to_string())
                .collect(),
            sample_attempts: 2048,
            extra_points: 0,
            loci: None,
            box_size: 0,
            box_max: 0,
            dump_weights: false,
            random_d: false,
            single_prime: false,
        };
        crate::node::replay::run(&args).unwrap().0
    }

    #[test]
    fn refused_rules_are_banked_adopted_as_fallbacks_and_reused() {
        // (1) every rule with RHS terms is oversized and no fallback band: the node stops, but nothing is lost
        let d1 = tmp("bank-refuse");
        let o1 = NodeOptions {
            max_rule_monomials: Some(0),
            fallback_rule_monomials: None,
            cand_dump: true,
            ..quiet()
        };
        let r1 = generate(SUNSET, "sunset.sym", &d1, &o1, &mut |_| {}).unwrap();
        assert!(
            matches!(r1.stop, Some(StopReason::LiftRefused { .. })),
            "{:?}",
            r1.stop
        );
        assert!(r1.oversized_rules > 0 && r1.banked >= r1.oversized_rules);
        let bank = std::fs::read_to_string(d1.join("F.refused.jsonl")).unwrap();
        let refused: Vec<serde_json::Value> = bank
            .lines()
            .filter_map(|l| serde_json::from_str(l).ok())
            .filter(|v: &serde_json::Value| v["type"] == "refused")
            .collect();
        assert!(!refused.is_empty());
        for v in &refused {
            assert!(
                v["reason"].as_str().unwrap().contains("never adopted"),
                "{v}"
            );
            assert!(v["guards"].as_str().unwrap().starts_with("ok"), "{v}");
            assert_eq!(
                v["rule"]["type"], "rule",
                "the banked rule carries its full table line"
            );
            assert!(
                d1.join(format!("F.refused_dump.b{}.json", v["bank_index"]))
                    .exists()
            );
        }
        // (2) with the fallback band the node closes; adopted fallbacks are flagged in the table and the outcome
        let d2 = tmp("bank-fallback");
        let o2 = NodeOptions {
            max_rule_monomials: Some(0),
            fallback_rule_monomials: Some(usize::MAX),
            fallback_after: 3,
            ..quiet()
        };
        let r2 = generate(SUNSET, "sunset.sym", &d2, &o2, &mut |_| {}).unwrap();
        assert!(r2.complete, "{:?}", r2.stop);
        assert!(!r2.oversized_adopted.is_empty());
        let table = std::fs::read_to_string(&r2.table).unwrap();
        let flagged = table
            .lines()
            .filter(|l| {
                l.starts_with("{\"type\":\"rule\"") && l.contains("\"oversized_fallback\":true")
            })
            .count();
        assert_eq!(flagged, r2.oversized_adopted.len());
        assert_eq!(replay_status(&d2, &r2.table), "PASS_RS2_RS3_SAMPLED");
        let plain = generate(
            SUNSET,
            "sunset.sym",
            &tmp("bank-plain"),
            &quiet(),
            &mut |_| {},
        )
        .unwrap();
        assert_eq!(r2.residuals, plain.residuals);
        // (3) a later policy (no size limit) adopts the banked rules of run (2) without searching them again
        let d3 = tmp("bank-adopt");
        let o3 = NodeOptions {
            max_rule_monomials: None,
            adopt_banks: vec![d2.join("F.refused.jsonl").display().to_string()],
            ..quiet()
        };
        let r3 = generate(SUNSET, "sunset.sym", &d3, &o3, &mut |_| {}).unwrap();
        assert!(r3.complete, "{:?}", r3.stop);
        assert!(r3.bank_adopted > 0, "{r3:?}");
        assert_eq!(r3.residuals, plain.residuals);
        assert_eq!(replay_status(&d3, &r3.table), "PASS_RS2_RS3_SAMPLED");
        // a bank of another presentation is skipped
        let o4 = NodeOptions {
            order: NodeOrder {
                name: "isp_rev_all".into(),
                slot_permutation: Some(vec![0, 1, 2, 4, 3]),
                ..NodeOrder::identity()
            },
            ..o3.clone()
        };
        let r4 = generate(SUNSET, "sunset.sym", &tmp("bank-other"), &o4, &mut |_| {}).unwrap();
        assert_eq!(r4.bank_adopted, 0);
        for d in [d1, d2, d3] {
            let _ = std::fs::remove_dir_all(&d);
        }
    }

    fn strip_guard_stamps(table: &str, out: &Path) -> String {
        // a "pre-fix" copy of a table: no guard_version in the header or any rule line (absent = 1)
        let text = std::fs::read_to_string(table).unwrap();
        let mut lines = Vec::new();
        for l in text.lines() {
            let mut v: serde_json::Value = serde_json::from_str(l).unwrap();
            if let Some(o) = v.as_object_mut() {
                o.remove("guard_version");
            }
            lines.push(v.to_string());
        }
        std::fs::create_dir_all(out).unwrap();
        let p = out.join("F.table.jsonl");
        std::fs::write(&p, lines.join("\n") + "\n").unwrap();
        p.display().to_string()
    }

    #[test]
    fn guard_fix_d1_cover_tables_of_other_guard_versions_are_refused() {
        let root = tmp("d1-cover");
        let v1o = NodeOptions {
            no_witness_guards: true,
            ..quiet()
        };
        let v1 = generate(SUNSET, "sunset.sym", &root.join("v1"), &v1o, &mut |_| {}).unwrap();
        let v2 = generate(
            SUNSET,
            "sunset.sym",
            &root.join("v2"),
            &quiet(),
            &mut |_| {},
        )
        .unwrap();
        assert!(v1.complete && v2.complete && v1.rules > 0 && v2.rules > 0);
        let head = |t: &str| -> serde_json::Value {
            serde_json::from_str(std::fs::read_to_string(t).unwrap().lines().next().unwrap())
                .unwrap()
        };
        assert_eq!(head(&v1.table)["guard_version"], 1);
        assert_eq!(head(&v2.table)["guard_version"], GUARD_VERSION);
        // every rule line carries its own stamp
        for (t, g) in [(&v1.table, 1u64), (&v2.table, GUARD_VERSION as u64)] {
            for l in std::fs::read_to_string(t).unwrap().lines() {
                if l.starts_with("{\"type\":\"rule\"") {
                    let v: serde_json::Value = serde_json::from_str(l).unwrap();
                    assert_eq!(v["guard_version"].as_u64(), Some(g), "{l}");
                }
            }
        }
        let sub = vec![CaseSpec {
            fixed: vec![Some(1), Some(1), None, Some(0), Some(0)],
            equations: vec![],
        }];
        let with_cover = |o: &NodeOptions, cover: &str, rules: usize| NodeOptions {
            initial: sub.clone(),
            cover_tables: vec![(cover.to_string(), rules)],
            ..o.clone()
        };
        // a v2 run refuses the v1 table, its cover index, and a stamp-free (pre-fix) table
        let e = generate(
            SUNSET,
            "sunset.sym",
            &root.join("r1"),
            &with_cover(&quiet(), &v1.table, v1.rules),
            &mut |_| {},
        )
        .unwrap_err();
        assert!(e.contains("guard_version 1") && e.contains("D1"), "{e}");
        let pre = strip_guard_stamps(&v2.table, &root.join("pre"));
        let e = generate(
            SUNSET,
            "sunset.sym",
            &root.join("r3"),
            &with_cover(&quiet(), &pre, v2.rules),
            &mut |_| {},
        )
        .unwrap_err();
        assert!(e.contains("absent = 1"), "{e}");
        // a header saying 2 over v1 rule lines (a laundered table) is refused by the rule stamps
        let laundered = root.join("laundered.table.jsonl");
        let mut lines: Vec<String> = std::fs::read_to_string(&v1.table)
            .unwrap()
            .lines()
            .map(String::from)
            .collect();
        let mut h: serde_json::Value = serde_json::from_str(&lines[0]).unwrap();
        h["guard_version"] = serde_json::json!(GUARD_VERSION);
        lines[0] = h.to_string();
        std::fs::write(&laundered, lines.join("\n") + "\n").unwrap();
        let e = generate(
            SUNSET,
            "sunset.sym",
            &root.join("r4"),
            &with_cover(&quiet(), laundered.to_str().unwrap(), v1.rules),
            &mut |_| {},
        )
        .unwrap_err();
        assert!(e.contains("carries guard_version"), "{e}");
        // matching versions are accepted (v2 over v2, v1 over v1)
        let ok2 = generate(
            SUNSET,
            "sunset.sym",
            &root.join("r5"),
            &with_cover(&quiet(), &v2.table, v2.rules),
            &mut |_| {},
        )
        .unwrap();
        assert!(ok2.complete, "{ok2:?}");
        let ok1 = generate(
            SUNSET,
            "sunset.sym",
            &root.join("r6"),
            &with_cover(&v1o, &v1.table, v1.rules),
            &mut |_| {},
        )
        .unwrap();
        assert!(ok1.complete, "{ok1:?}");
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn guard_fix_d1_banks_of_other_guard_versions_are_never_adopted() {
        let root = tmp("d1-bank");
        let bank_opts = |no_witness_guards: bool| NodeOptions {
            max_rule_monomials: Some(0),
            fallback_rule_monomials: Some(usize::MAX),
            fallback_after: 3,
            no_witness_guards,
            ..quiet()
        };
        let b1 = generate(
            SUNSET,
            "sunset.sym",
            &root.join("b1"),
            &bank_opts(true),
            &mut |_| {},
        )
        .unwrap();
        let b2 = generate(
            SUNSET,
            "sunset.sym",
            &root.join("b2"),
            &bank_opts(false),
            &mut |_| {},
        )
        .unwrap();
        assert!(b1.complete && b2.complete);
        let bank_head = |d: &Path| -> serde_json::Value {
            serde_json::from_str(
                std::fs::read_to_string(d.join("F.refused.jsonl"))
                    .unwrap()
                    .lines()
                    .next()
                    .unwrap(),
            )
            .unwrap()
        };
        assert_eq!(bank_head(&root.join("b1"))["guard_version"], 1);
        assert_eq!(bank_head(&root.join("b2"))["guard_version"], GUARD_VERSION);
        assert_ne!(
            bank_head(&root.join("b1"))["key"],
            bank_head(&root.join("b2"))["key"]
        );
        let adopt = |banks: &str, no_witness_guards: bool, tag: &str| {
            let o = NodeOptions {
                max_rule_monomials: None,
                adopt_banks: vec![
                    root.join(banks)
                        .join("F.refused.jsonl")
                        .display()
                        .to_string(),
                ],
                no_witness_guards,
                ..quiet()
            };
            generate(SUNSET, "sunset.sym", &root.join(tag), &o, &mut |_| {}).unwrap()
        };
        assert_eq!(
            adopt("b1", false, "a12").bank_adopted,
            0,
            "v2 never adopts a v1 bank"
        );
        assert!(
            adopt("b2", false, "a22").bank_adopted > 0,
            "v2 adopts a v2 bank"
        );
        assert_eq!(
            adopt("b2", true, "a21").bank_adopted,
            0,
            "v1 never adopts a v2 bank"
        );
        assert!(
            adopt("b1", true, "a11").bank_adopted > 0,
            "v1 adopts a v1 bank"
        );
        let q = std::fs::read_to_string(root.join("a12/F.queue.jsonl")).unwrap();
        assert!(
            q.contains("bank_skipped") && q.contains("guard_version 1"),
            "{q}"
        );
        // a header-less-version (pre-fix) bank: absent = 1, refused by a v2 run
        let pre = root.join("pre");
        std::fs::create_dir_all(&pre).unwrap();
        let text = std::fs::read_to_string(root.join("b2/F.refused.jsonl")).unwrap();
        let mut out = Vec::new();
        for l in text.lines() {
            let mut v: serde_json::Value = serde_json::from_str(l).unwrap();
            if let Some(o) = v.as_object_mut() {
                o.remove("guard_version");
            }
            if let Some(o) = v.get_mut("rule").and_then(|r| r.as_object_mut()) {
                o.remove("guard_version");
            }
            out.push(v.to_string());
        }
        std::fs::write(pre.join("F.refused.jsonl"), out.join("\n") + "\n").unwrap();
        assert_eq!(adopt("pre", false, "apre").bank_adopted, 0);
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn union_of_rules_covers_a_case_no_single_rule_covers() {
        let fam = compile_family(SUNSET).unwrap();
        let sources = SourceSystem::<5>::from_family_with_capacity(&fam, true).unwrap();
        let idx: [usize; 5] = *sources.index_variables();
        let template = sources
            .rows()
            .iter()
            .flat_map(|r| r.iter())
            .next()
            .unwrap()
            .coefficient
            .clone();
        let sector = [true, true, true, false, false];
        let case = |fixed: Vec<Option<i64>>| {
            build_case::<5>(
                &CaseSpec {
                    fixed,
                    equations: vec![],
                },
                &template,
                &idx,
                &sector,
                5,
            )
            .unwrap()
            .unwrap()
        };
        let limits = CaseIntersectionLimits::default();
        // A: the whole chart, except where n0 = 1; B: the n0 = 1 face, no exceptions
        let a = case(vec![None, None, None, None, None]);
        let a_branches = vec![vec![parse_index_poly("n0-1", &template, &idx).unwrap()]];
        let b = case(vec![Some(1), None, None, None, None]);
        let c = case(vec![None, Some(2), None, None, None]);
        assert!(
            !covers(&a, &a_branches, &c, &idx, &sector, limits),
            "A alone does not cover C (its n0 = 1 branch meets C)"
        );
        assert!(!covers(&b, &[], &c, &idx, &sector, limits));
        let both: Vec<(&Case<5>, &[Vec<CoefficientPolynomial>])> =
            vec![(&a, a_branches.as_slice()), (&b, &[])];
        let mut budget = 100;
        assert!(union_covered(
            &c,
            &both,
            &idx,
            &sector,
            limits,
            2,
            &mut budget
        ));
        let only_a: Vec<(&Case<5>, &[Vec<CoefficientPolynomial>])> =
            vec![(&a, a_branches.as_slice())];
        let mut budget = 100;
        assert!(!union_covered(
            &c,
            &only_a,
            &idx,
            &sector,
            limits,
            2,
            &mut budget
        ));
        let mut budget = 100;
        assert!(
            !union_covered(&c, &both, &idx, &sector, limits, 0, &mut budget),
            "depth 0 = off"
        );
    }

    #[test]
    fn registry_binds_identity_and_is_append_only() {
        use crate::node::registry::Registry;
        let reg = tmp("registry");
        let d = reg.to_str().unwrap();
        let mut a = Registry::open(d, "a", "id1", "mk", serde_json::json!({})).unwrap();
        let mut b = Registry::open(d, "b", "id1", "mk", serde_json::json!({})).unwrap();
        let mut other = Registry::open(d, "x", "id2", "mk", serde_json::json!({})).unwrap();
        let spec = |f: Vec<Option<i64>>| CaseSpec {
            fixed: f,
            equations: vec![],
        };
        a.publish_rule(0, &spec(vec![Some(1), None]), vec![vec!["n1-2".into()]]);
        other.publish_rule(0, &spec(vec![None, None]), vec![]);
        let (seen, foreign) = b.poll();
        assert_eq!(seen.len(), 1, "only the same identity, never its own file");
        assert!(seen[0].0.starts_with("registry:a."));
        assert_eq!(
            seen[0].2,
            vec![vec!["n1-2".to_string()]],
            "full exceptions travel"
        );
        assert_eq!(foreign.len(), 1, "the other identity is reported foreign");
        assert!(b.poll().0.is_empty(), "incremental");
        assert!(!b.siblings_ended());
        a.record(serde_json::json!({"type": "end", "state": "stopped"}));
        let _ = b.poll();
        assert!(b.siblings_ended());
        // a second epoch of piece a is a NEW file: the first one's records survive (never truncated)
        let _a2 = Registry::open(d, "a", "id1", "mk", serde_json::json!({})).unwrap();
        let files: Vec<_> = std::fs::read_dir(&reg)
            .unwrap()
            .filter_map(|e| e.ok())
            .filter(|e| e.file_name().to_string_lossy().starts_with("a."))
            .collect();
        assert_eq!(files.len(), 2);
        let first = std::fs::read_to_string(a_path(&reg, &a)).unwrap();
        assert!(first.contains("\"type\":\"rule\""));
        let _ = std::fs::remove_dir_all(&reg);
    }

    fn a_path(dir: &Path, r: &crate::node::registry::Registry) -> PathBuf {
        dir.join(format!("{}.reg.jsonl", r.label()))
    }

    fn obligations(dir: &Path) -> crate::node::registry::Obligations {
        crate::node::registry::Obligations::read(&dir.join("F.obligations.json")).unwrap()
    }

    fn join_of(
        dirs: &[&Path],
        out: &Path,
        expect: Option<usize>,
    ) -> crate::node::registry::JoinReport {
        let opts = crate::node::registry::JoinOptions {
            runs: dirs
                .iter()
                .map(|d| crate::node::registry::JoinRun {
                    dir: d.to_path_buf(),
                    obligations: d.join("F.obligations.json"),
                    cover_index: None,
                })
                .collect(),
            out: out.to_path_buf(),
            expect_residuals: expect,
            union_depth: 3,
            write_table: true,
            ..Default::default()
        };
        crate::node::registry::join(&opts).unwrap()
    }

    #[test]
    fn containment_in_a_sibling_rule_never_certifies_a_case() {
        // p1 stops after its first rule (whose exceptional children stay open); p2 runs the whole chart with the
        // registry: p1's rule case CONTAINS p2's first case but its branches meet it, so p2 may only lease it and
        // must take it back (no proof) and close it itself. The old registry discharged it by containment.
        let root = tmp("contain");
        let regdir = root.join("reg").display().to_string();
        let p1 = NodeOptions {
            registry: Some((regdir.clone(), "p1".into())),
            max_cases: Some(1),
            ..quiet()
        };
        let r1 = generate(SUNSET, "sunset.sym", &root.join("p1"), &p1, &mut |_| {}).unwrap();
        assert!(!r1.complete && r1.state == "stopped", "{r1:?}");
        let o1 = obligations(&root.join("p1"));
        assert_eq!(o1.state, "stopped");
        assert!(
            !o1.open.is_empty(),
            "a stopped run lists its open obligations"
        );
        assert_eq!(r1.open_obligations, o1.open.len());
        let qlog = std::fs::read_to_string(root.join("p1/F.queue.jsonl")).unwrap();
        assert!(
            qlog.contains("\"state\":\"stopped\""),
            "the queue end record says stopped"
        );
        let p2 = NodeOptions {
            registry: Some((regdir, "p2".into())),
            lease_wait: 0,
            ..quiet()
        };
        let r2 = generate(SUNSET, "sunset.sym", &root.join("p2"), &p2, &mut |_| {}).unwrap();
        assert!(r2.complete && r2.state == "complete", "{r2:?}");
        assert!(
            r2.leases >= 1,
            "the contained case is leased, not discharged: {r2:?}"
        );
        assert!(
            r2.leases_returned >= 1,
            "no proof: the lease comes back: {r2:?}"
        );
        let q2 = std::fs::read_to_string(root.join("p2/F.queue.jsonl")).unwrap();
        assert!(
            !q2.lines()
                .any(|l| l.contains("\"by_registry\"") && l.contains("\"proof\":\"none\"")),
            "no discharge without a proof"
        );
        let plain = generate(
            SUNSET,
            "sunset.sym",
            &root.join("plain"),
            &quiet(),
            &mut |_| {},
        )
        .unwrap();
        assert_eq!(r2.rules, plain.rules, "p2 solved the whole chart itself");
        // join: p1's open obligations lie in p2's initial case (an edge p1 -> p2), p2 is complete: closed
        let j = join_of(
            &[&root.join("p1"), &root.join("p2")],
            &root.join("join"),
            Some(1),
        );
        assert_eq!(j.verdict, "closed", "{j:?}");
        assert!(j.edges.len() <= 1, "{j:?}");
        // p1 alone stays open, with the obligations listed
        let j1 = join_of(&[&root.join("p1")], &root.join("join1"), None);
        assert_eq!(j1.verdict, "open", "{j1:?}");
        assert_eq!(j1.unresolved.len(), o1.open.len());
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn join_uses_a_covered_region_certificate_only_when_every_case_is_owned_or_covered() {
        // a stopped run with one deferred region obligation (chart = one of its open cases, a stand-in conjunction);
        // a COVERED certificate {chart, conjunction, cases} discharges it iff each listed case is empty, owned by a
        // COMPLETE other run or covered by one rule; a malformed or unowned certificate is not used
        let root = tmp("covered");
        let stopped = NodeOptions {
            max_cases: Some(1),
            ..quiet()
        };
        generate(
            SUNSET,
            "sunset.sym",
            &root.join("p1"),
            &stopped,
            &mut |_| {},
        )
        .unwrap();
        generate(
            SUNSET,
            "sunset.sym",
            &root.join("full"),
            &quiet(),
            &mut |_| {},
        )
        .unwrap();
        let mut o1 = obligations(&root.join("p1"));
        let chart = o1.open[0].case.clone();
        let conj = serde_json::json!([["n1*n2-3"]]);
        o1.open.push(crate::node::registry::Obligation {
            kind: "region".into(),
            case: chart.clone(),
            detail: serde_json::json!({"kind": "region", "conjunction": conj}),
        });
        o1.write(&root.join("p1/F.obligations.json")).unwrap();
        let join_with = |cases: serde_json::Value, runs: &[&Path], tag: &str| {
            let cert = root.join(format!("{tag}.json"));
            std::fs::write(
                &cert,
                serde_json::json!([{"chart": chart, "conjunction": conj, "status": "COVERED",
                    "method": "test", "cases": cases}])
                .to_string(),
            )
            .unwrap();
            let opts = crate::node::registry::JoinOptions {
                runs: runs
                    .iter()
                    .map(|d| crate::node::registry::JoinRun {
                        dir: d.to_path_buf(),
                        obligations: d.join("F.obligations.json"),
                        cover_index: None,
                    })
                    .collect(),
                out: root.join(tag),
                expect_residuals: None,
                region_proofs: vec![cert],
                union_depth: 3,
                write_table: false,
            };
            crate::node::registry::join(&opts).unwrap()
        };
        let (p1, full) = (root.join("p1"), root.join("full"));
        // (a) the listed case is owned by the complete run: discharged by the certificate, with an edge
        let j = join_with(serde_json::json!([chart]), &[&p1, &full], "a");
        assert_eq!(j.verdict, "closed", "{j:?}");
        assert_eq!(j.discharged.get("region_covered:test"), Some(&1), "{j:?}");
        // (b) a malformed piece (wrong arity): certificate not used; the region falls through to the ownership step
        let j = join_with(
            serde_json::json!([{"fixed": [null], "equations": []}]),
            &[&p1, &full],
            "b",
        );
        assert!(!j.discharged.contains_key("region_covered:test"), "{j:?}");
        // (c) no complete owner and no single covering rule: the certificate cannot discharge, the region stays open
        let j = join_with(
            serde_json::json!([{"fixed": [null, null, null, null, null], "equations": []}]),
            &[&p1],
            "c",
        );
        assert_eq!(j.verdict, "open", "{j:?}");
        assert!(!j.discharged.contains_key("region_covered:test"), "{j:?}");
        assert!(
            j.unresolved
                .iter()
                .any(|u| u["obligation"]["kind"] == "region"),
            "{j:?}"
        );
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn join_rejects_a_two_cycle_of_stopped_runs() {
        // two stopped runs of the whole chart: each one's open obligations lie in the other's initial case, so each
        // "discharges" the other: an unresolved strongly connected component (cf. Astra's T212 2-cycle); rejected
        let root = tmp("cycle");
        let stopped = NodeOptions {
            max_cases: Some(1),
            ..quiet()
        };
        generate(SUNSET, "sunset.sym", &root.join("a"), &stopped, &mut |_| {}).unwrap();
        generate(SUNSET, "sunset.sym", &root.join("b"), &stopped, &mut |_| {}).unwrap();
        let j = join_of(
            &[&root.join("a"), &root.join("b")],
            &root.join("join"),
            None,
        );
        assert_eq!(j.verdict, "rejected", "{j:?}");
        assert_eq!(j.cycles.len(), 1);
        assert_eq!(
            crate::node::registry::sccs(3, &[(0, 1), (1, 0), (1, 2)])
                .iter()
                .filter(|c| c.len() > 1)
                .count(),
            1
        );
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn join_rejects_runs_under_different_orders() {
        // T212 lesson: cheap rules of two orders joined into one table can cycle; the math key binds the order
        let root = tmp("orders");
        let tie = NodeOrder {
            name: "tie_isp_rev".into(),
            coordinate_priority: Some(vec![0, 1, 2, 4, 3]),
            ..NodeOrder::identity()
        };
        generate(
            SUNSET,
            "sunset.sym",
            &root.join("id"),
            &quiet(),
            &mut |_| {},
        )
        .unwrap();
        let o = NodeOptions {
            order: tie,
            ..quiet()
        };
        generate(SUNSET, "sunset.sym", &root.join("tie"), &o, &mut |_| {}).unwrap();
        let j = join_of(
            &[&root.join("id"), &root.join("tie")],
            &root.join("join"),
            None,
        );
        assert_eq!(j.verdict, "rejected", "{j:?}");
        assert!(j.reasons.iter().any(|r| r.contains("math key")));
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn pool_split_closes_and_reconstruction_matches_the_native_record() {
        use crate::node::registry::{POOL_SCHEMA, Pool, PoolItem, obligations_from_queue};
        let root = tmp("pool");
        let p0 = NodeOptions {
            max_cases: Some(2),
            ..quiet()
        };
        let r0 = generate(SUNSET, "sunset.sym", &root.join("p0"), &p0, &mut |_| {}).unwrap();
        assert!(!r0.complete);
        let native = obligations(&root.join("p0"));
        let rebuilt =
            obligations_from_queue(SUNSET, &NodeOrder::identity(), &root.join("p0")).unwrap();
        assert_eq!(rebuilt.math_key, native.math_key);
        let cases = |o: &crate::node::registry::Obligations, kinds: &[&str]| {
            let mut v: Vec<String> = o
                .open
                .iter()
                .filter(|x| kinds.contains(&x.kind.as_str()))
                .map(|x| serde_json::to_string(&x.case).unwrap())
                .collect();
            v.sort();
            v
        };
        assert_eq!(
            cases(&rebuilt, &["pending", "current"]),
            cases(&native, &["pending", "current"])
        );
        assert_eq!(
            cases(&rebuilt, &["numerical"]),
            cases(&native, &["numerical"])
        );
        // the pool: every open obligation of p0; two pieces claim from it with the registry and p0 as cover
        let items: Vec<PoolItem> = native
            .open
            .iter()
            .enumerate()
            .map(|(k, o)| PoolItem {
                id: format!("o{k:03}"),
                case: o.case.clone(),
            })
            .collect();
        let pool_dir = root.join("pool");
        std::fs::create_dir_all(&pool_dir).unwrap();
        let pool = Pool {
            schema: POOL_SCHEMA.into(),
            math_key: Some(native.math_key.clone()),
            items,
        };
        std::fs::write(
            pool_dir.join("POOL.json"),
            serde_json::to_string(&pool).unwrap(),
        )
        .unwrap();
        let cover = (
            root.join("p0/F.table.jsonl").display().to_string(),
            usize::MAX,
        );
        // a compact cover index of p0's table loads like the table
        let index = root.join("p0.cover.jsonl").display().to_string();
        let k = crate::node::registry::write_cover_index(&cover.0, usize::MAX, &index).unwrap();
        assert_eq!(k, r0.rules);
        let mut dirs = vec![root.join("p0")];
        for (piece, cov) in [("q1", cover.clone()), ("q2", (index.clone(), usize::MAX))] {
            let o = NodeOptions {
                pool: Some(pool_dir.display().to_string()),
                registry: Some((root.join("reg").display().to_string(), piece.into())),
                cover_tables: vec![cov],
                lease_wait: 0,
                ..quiet()
            };
            let r = generate(SUNSET, "sunset.sym", &root.join(piece), &o, &mut |_| {}).unwrap();
            assert!(r.complete, "{piece}: {r:?}");
            dirs.push(root.join(piece));
        }
        let claimed = obligations(&root.join("q1")).pool_claims.len()
            + obligations(&root.join("q2")).pool_claims.len();
        assert_eq!(
            claimed,
            native.open.len(),
            "every pool item claimed exactly once"
        );
        let refs: Vec<&Path> = dirs.iter().map(|d| d.as_path()).collect();
        let j = join_of(&refs, &root.join("join"), Some(1));
        assert_eq!(j.verdict, "closed", "{j:?}");
        assert!(j.cycles.is_empty());
        let (_, rep) =
            crate::node::descent::check_table_file(j.table.as_ref().unwrap(), None).unwrap();
        assert!(rep.passed() && rep.rules == j.rules, "{rep:?}");
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn refused_cases_stay_open_obligations_and_the_queue_continues() {
        let dir = tmp("open-refusal");
        let screen = LiftScreen {
            min_rows: 1,
            max_total_degree: 0,
            max_rhs_terms: 0,
            on_heavy: HeavyAction::Refuse,
            ..LiftScreen::default()
        };
        let opts = NodeOptions {
            screen: Some(screen),
            open_on_refusal: true,
            ..quiet()
        };
        let out = generate(SUNSET, "sunset.sym", &dir, &opts, &mut |_| {}).unwrap();
        if out.screened_lifts > 0 {
            assert!(out.refused_open > 0 && !out.complete && out.state == "stopped");
            assert!(
                matches!(out.stop, Some(StopReason::OpenObligations { .. })),
                "{:?}",
                out.stop
            );
            let o = obligations(&dir);
            assert_eq!(
                o.open.iter().filter(|x| x.kind == "refused").count(),
                out.refused_open
            );
            assert!(
                o.open
                    .iter()
                    .all(|x| x.kind != "refused" || x.detail["reports"].is_array())
            );
        }
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn case_seed_priority_default_static_and_validation() {
        let encoded = serde_json::to_value(NodeOptions::default()).unwrap();
        assert!(encoded.get("case_seed_priority").is_none());
        let restored: NodeOptions = serde_json::from_value(encoded).unwrap();
        assert!(restored.case_seed_priority.is_none());
        let mut output = Vec::new();
        log_case_seed_schedule(
            &mut output,
            &NodeOptions::default(),
            &Case::<2>::generic(),
            &[true, false],
            2,
        )
        .unwrap();
        assert!(output.is_empty(), "None must not alter legacy queue bytes");
        let policy = CaseSeedPriority::SymbolicIspFirst;
        assert!(policy.validate_template(None, 2).is_err());
        assert!(
            policy
                .validate_template(Some(&CaseSeedSchedule::Reference), 2)
                .is_err()
        );
        for (first_depth, last_depth, axis_priority) in [
            (2, 1, vec![0, 1]),
            (1, 2, vec![0, 0]),
            (1, 2, vec![0, 2]),
            (1, 2, vec![0]),
        ] {
            let invalid = CaseSeedSchedule::ShiftShells {
                first_depth,
                last_depth,
                axis_priority,
                positive_first: true,
            };
            assert!(policy.validate_template(Some(&invalid), 2).is_err());
        }
    }

    #[test]
    fn case_seed_priority_explicit_coordinate_and_numeric_equivalence() {
        let template = CaseSeedSchedule::ShiftShells {
            first_depth: 2,
            last_depth: 2,
            axis_priority: vec![4, 1, 5, 0, 3, 2],
            positive_first: true,
        };
        let physical = [true, true, true, false, false, false];
        let case =
            Case::from(CoordinateCase::new([None, Some(1), None, None, Some(0), None]).unwrap());
        let resolved = resolve_case_seed_schedule(
            CaseSeedPriority::SymbolicIspFirst,
            Some(&template),
            &case,
            &physical,
        )
        .unwrap()
        .unwrap();
        let explicit = CaseSeedSchedule::ShiftShells {
            first_depth: 2,
            last_depth: 2,
            axis_priority: vec![5, 3, 0, 2, 4, 1],
            positive_first: true,
        };
        assert_eq!(
            resolved, explicit,
            "the unchanged solver receives the exact explicit descriptor"
        );
        let numeric = Case::from(
            CoordinateCase::new([Some(1), Some(1), Some(1), Some(0), Some(0), Some(0)]).unwrap(),
        );
        assert_eq!(
            resolve_case_seed_schedule(
                CaseSeedPriority::SymbolicIspFirst,
                Some(&template),
                &numeric,
                &physical
            )
            .unwrap(),
            Some(template.clone())
        );
        let opts = NodeOptions {
            case_seed_schedule: Some(template),
            case_seed_priority: Some(CaseSeedPriority::SymbolicIspFirst),
            ..NodeOptions::default()
        };
        let mut bytes = Vec::new();
        log_case_seed_schedule(&mut bytes, &opts, &case, &physical, 6).unwrap();
        let record: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(record["schedule"], serde_json::to_value(explicit).unwrap());
        assert_eq!(record["frame"], "presented");
        assert_eq!(record["physical_mask"], "111000");
    }

    #[test]
    fn case_seed_priority_affine_unfixed_axes_remain_symbolic() {
        let c = crate::algebra::CoefficientContext::try_new(["n0", "n1", "n2", "n3", "n4", "n5"])
            .unwrap();
        let physical = [true, true, true, false, false, false];
        let spec = CaseSpec {
            fixed: vec![None, Some(1), None, None, Some(0), None],
            equations: vec!["n3-n5".into()],
        };
        let affine = build_case::<6>(&spec, &c.one().numerator, &[0, 1, 2, 3, 4, 5], &physical, 6)
            .unwrap()
            .unwrap();
        assert!(affine.affine().is_some());
        assert!(affine.fixed()[3].is_none() && affine.fixed()[5].is_none());
        let template = CaseSeedSchedule::ShiftShells {
            first_depth: 1,
            last_depth: 2,
            axis_priority: vec![4, 1, 5, 0, 3, 2],
            positive_first: false,
        };
        let actual = resolve_case_seed_schedule(
            CaseSeedPriority::SymbolicIspFirst,
            Some(&template),
            &affine,
            &physical,
        )
        .unwrap()
        .unwrap();
        assert_eq!(
            actual,
            CaseSeedSchedule::ShiftShells {
                first_depth: 1,
                last_depth: 2,
                axis_priority: vec![5, 3, 0, 2, 4, 1],
                positive_first: false,
            }
        );
    }

    #[test]
    fn seed_pose_portfolio_puts_the_coupled_axes_pose_first_on_affine_cases() {
        let c = crate::algebra::CoefficientContext::try_new(["n0", "n1", "n2", "n3", "n4", "n5"])
            .unwrap();
        let physical = [true, true, true, false, false, false];
        let spec = CaseSpec {
            fixed: vec![None, Some(1), None, None, Some(0), None],
            equations: vec!["n3-n5".into()],
        };
        let affine = build_case::<6>(&spec, &c.one().numerator, &[0, 1, 2, 3, 4, 5], &physical, 6)
            .unwrap()
            .unwrap();
        let p = seed_pose_portfolio(&affine, &physical);
        assert_eq!(p.len(), 5);
        assert_eq!(p[0].0, "shell2-coupled+|spired");
        assert_eq!(
            p[0].1,
            CaseSeedSchedule::ShiftShells {
                first_depth: 2,
                last_depth: 2,
                axis_priority: vec![3, 5, 0, 2, 1, 4],
                positive_first: true,
            }
        );
        let coordinate =
            Case::from(CoordinateCase::new([None, Some(1), None, None, Some(0), None]).unwrap());
        let q = seed_pose_portfolio(&coordinate, &physical);
        assert_eq!(q.len(), 4);
        assert_eq!(q[0].0, "ref|sf");
    }

    #[test]
    fn fixed_dot_poses_lead_with_one_fixed_line_least_significant_first() {
        let c = crate::algebra::CoefficientContext::try_new(["n0", "n1", "n2", "n3", "n4", "n5"])
            .unwrap();
        // lines 0..3, ISPs 4, 5; line 1 dotted (2), line 2 boundary (1), ISP 4 fixed 0; free lines 0 = 3 coupled
        // (an equation between a line and an ISP would empty the case: lines >= 1, ISPs <= 0)
        let physical = [true, true, true, true, false, false];
        let spec = CaseSpec {
            fixed: vec![None, Some(2), Some(1), None, Some(0), None],
            equations: vec!["n3-n0".into()],
        };
        let affine = build_case::<6>(&spec, &c.one().numerator, &[0, 1, 2, 3, 4, 5], &physical, 6)
            .unwrap()
            .unwrap();
        // least significant first: legacy identity -> line 2 before line 1; priority [2, 1, ..] -> 1 before 2
        assert_eq!(fixed_dot_axes(&affine, &physical, None), vec![2, 1]);
        let prio = [2, 1, 0, 3, 4, 5];
        assert_eq!(fixed_dot_axes(&affine, &physical, Some(&prio)), vec![1, 2]);
        let p = fixed_dot_poses(&affine, &physical, 4, 100, Some(&prio));
        assert_eq!(p.len(), 3);
        assert_eq!(p[0].0, "shell3-ff+|spired");
        assert_eq!(
            p[0].1,
            CaseSeedSchedule::FixedDotShell {
                depth: 3,
                axis_priority: vec![0, 3, 5, 1, 2, 4],
                positive_first: true,
                max_seeds: 100,
            }
        );
        let p = p[1..].to_vec();
        assert_eq!(p[0].0, "fixdot1-shell3+|spired");
        assert_eq!(
            p[0].1,
            CaseSeedSchedule::FixedDotShell {
                depth: 3,
                axis_priority: vec![1, 0, 3, 5, 2, 4],
                positive_first: true,
                max_seeds: 100,
            }
        );
        assert_eq!(p[0].2, SeedScheduleName::Spired);
        assert_eq!(fixed_dot_poses(&affine, &physical, 1, 100, None).len(), 2);
        // k = 0: the lead only (line poses are opt-in)
        assert_eq!(fixed_dot_poses(&affine, &physical, 0, 100, None).len(), 1);
        // no free axis: nothing to search
        let point = Case::from(
            CoordinateCase::new([Some(1), Some(2), Some(1), Some(1), Some(0), Some(0)]).unwrap(),
        );
        assert!(fixed_dot_poses(&point, &physical, 4, 100, None).is_empty());
        // the options default serialises nothing new (table headers of rung-off runs stay byte-identical)
        let o = NodeOptions::default();
        let json = serde_json::to_string(&o).unwrap();
        assert!(!json.contains("fixed_dot"));
        let cfg = FixedDotConfig::from_options(&o);
        assert_eq!(
            (
                cfg.enabled,
                cfg.lines,
                cfg.max_seeds,
                cfg.first,
                cfg.priority.is_none(),
                cfg.static_skip
            ),
            (
                true,
                FIXED_DOT_LINES_DEFAULT,
                FIXED_DOT_SEEDS_DEFAULT,
                false,
                true,
                true
            )
        );
        assert_eq!(FIXED_DOT_LINES_DEFAULT, 0);
        // --seed-pose-fixed-dot 0 = off; K >= 1 keeps its earlier meaning (K line poses); -lines wins
        let off = FixedDotConfig::from_options(&NodeOptions {
            seed_pose_fixed_dot: Some(0),
            ..NodeOptions::default()
        });
        assert!(!off.enabled);
        let four = FixedDotConfig::from_options(&NodeOptions {
            seed_pose_fixed_dot: Some(4),
            ..NodeOptions::default()
        });
        assert!(four.enabled && four.lines == 4);
        let two = FixedDotConfig::from_options(&NodeOptions {
            seed_pose_fixed_dot: Some(4),
            seed_pose_fixed_dot_lines: Some(2),
            ..NodeOptions::default()
        });
        assert_eq!(two.lines, 2);
    }

    #[test]
    fn fixed_dot_static_counts_free_cycles_and_exposed_sets() {
        let fam = compile_family(SUNSET).unwrap();
        let m = fixed_dot_line_momenta(&fam);
        assert_eq!(
            m,
            vec![
                Some(vec![2, 0]),
                Some(vec![0, 2]),
                Some(vec![2, 2]),
                Some(vec![2, 0]),
                Some(vec![0, 2])
            ]
        );
        let physical = [true, true, true, false, false, false];
        let order = crate::solver::IntegralOrder::<6>::new(physical, [false; 6]);
        // free line 0 only: fixed momenta k2, k1 + k2 span both loops -> cyc 0, exposed sets {1} and {2}
        let one = Case::from(
            CoordinateCase::new([None, Some(1), Some(1), Some(0), Some(0), Some(0)]).unwrap(),
        );
        let r = fixed_dot_static(&one, &physical, &m, &order);
        assert_eq!(r.cyc, Some(0));
        assert_eq!(r.mu, Some(1));
        assert!(r.exposed.contains(&vec![1]) && r.exposed.contains(&vec![2]));
        assert!(matches!(
            r.verdict,
            FixedDotStatic::Needed | FixedDotStatic::Undecided
        ));
        // free lines 0, 1: the fixed line k1 + k2 leaves the direction k1 - k2 free -> cyc 1
        let two = Case::from(
            CoordinateCase::new([None, None, Some(1), Some(0), Some(0), Some(0)]).unwrap(),
        );
        let r = fixed_dot_static(&two, &physical, &m, &order);
        assert_eq!(r.verdict, FixedDotStatic::FreeCycle(1));
        assert!(r.to_json().contains("\"verdict\":\"free_cycle1\""));
        // every line fixed, only an ISP free: cyc 0 and no transfer -> undecided
        let isp = Case::from(
            CoordinateCase::new([Some(1), Some(1), Some(1), None, Some(0), Some(0)]).unwrap(),
        );
        let r = fixed_dot_static(&isp, &physical, &m, &order);
        assert_eq!((r.verdict, r.cyc), (FixedDotStatic::Undecided, Some(0)));
        // no momenta: unknown
        let r = fixed_dot_static(&two, &physical, &[None, None, None, None, None], &order);
        assert_eq!(r.verdict, FixedDotStatic::Unknown);
        assert_eq!(rank_mod_p(&[vec![1, 2], vec![2, 4], vec![0, 1]]), 2);
    }

    #[test]
    fn affine_coupled_first_orders_coupled_axes_and_declines_coordinate_cases() {
        let c = crate::algebra::CoefficientContext::try_new(["n0", "n1", "n2", "n3", "n4", "n5"])
            .unwrap();
        let physical = [true, true, true, false, false, false];
        let spec = CaseSpec {
            fixed: vec![None, Some(1), None, None, Some(0), None],
            equations: vec!["n3-n5".into()],
        };
        let affine = build_case::<6>(&spec, &c.one().numerator, &[0, 1, 2, 3, 4, 5], &physical, 6)
            .unwrap()
            .unwrap();
        let template = CaseSeedSchedule::ShiftShells {
            first_depth: 2,
            last_depth: 2,
            axis_priority: vec![4, 1, 5, 0, 3, 2],
            positive_first: false,
        };
        let p = CaseSeedPriority::AffineCoupledFirst;
        assert_eq!(
            resolve_case_seed_schedule(p, Some(&template), &affine, &physical).unwrap(),
            Some(CaseSeedSchedule::ShiftShells {
                first_depth: 2,
                last_depth: 2,
                axis_priority: vec![5, 3, 4, 1, 0, 2],
                positive_first: false,
            })
        );
        let coordinate =
            Case::from(CoordinateCase::new([None, Some(1), None, None, Some(0), None]).unwrap());
        assert_eq!(
            resolve_case_seed_schedule(p, Some(&template), &coordinate, &physical).unwrap(),
            None
        );
        // a declined case writes no resolution record
        let opts = NodeOptions {
            case_seed_schedule: Some(template),
            case_seed_priority: Some(p),
            ..NodeOptions::default()
        };
        let mut bytes = Vec::new();
        log_case_seed_schedule(&mut bytes, &opts, &coordinate, &physical, 6).unwrap();
        assert!(bytes.is_empty());
        assert_eq!(
            serde_json::to_string(&p).unwrap(),
            "\"affine-coupled-first\""
        );
    }

    #[test]
    fn seed_centre_options_default_off_and_parse() {
        // defaults serialize exactly as before (no new keys in the options record / registry identity)
        let v = serde_json::to_value(NodeOptions::default()).unwrap();
        for key in [
            "seed_centre_offset",
            "seed_centre_extra",
            "case_seed_centres",
            "seed_face_floor",
        ] {
            assert!(v.get(key).is_none(), "{key}");
        }
        let restored: NodeOptions = serde_json::from_value(v).unwrap();
        assert!(restored.seed_centre_offset.is_none() && restored.case_seed_centres.is_empty());
        assert_eq!(parse_seed_centre_offset("0:-1", 3).unwrap(), vec![-1, 0, 0]);
        assert_eq!(
            parse_seed_centre_offset("0:-1, 2:1", 3).unwrap(),
            vec![-1, 0, 1]
        );
        for bad in ["3:-1", "0", "x:1", "0:99"] {
            assert!(parse_seed_centre_offset(bad, 3).is_err(), "{bad}");
        }
        // a CaseSpec with the per-case field still parses (the field is read separately by the CLI)
        let spec: CaseSpec = serde_json::from_str(
            r#"{"fixed":[null,1,0],"equations":[],"seed_centre_offset":[-1,0,0]}"#,
        )
        .unwrap();
        assert_eq!(spec.fixed, vec![None, Some(1), Some(0)]);
        // per-case offset wins for contained cases, else the global one; no recipe without both and a floor
        let face = Case::<3>::from(CoordinateCase::new([None, Some(1), Some(0)]).unwrap());
        let point = Case::<3>::from(CoordinateCase::new([Some(3), Some(1), Some(0)]).unwrap());
        let other = Case::<3>::from(CoordinateCase::new([None, Some(2), Some(0)]).unwrap());
        let mut opts = NodeOptions::default();
        assert!(seed_recipe_for(&opts, &[], &face).is_none());
        opts.seed_centre_offset = Some(vec![0, 0, 0]);
        assert!(seed_recipe_for(&opts, &[], &face).is_none());
        opts.seed_centre_offset = Some(vec![0, 1, 0]);
        let centres = vec![(face.clone(), vec![vec![-1, 0, 0], vec![0, 0, 0]])];
        let r = seed_recipe_for(&opts, &centres, &point).unwrap();
        assert_eq!(
            (r.centre_offset.clone(), r.extra_centres.clone()),
            (vec![-1, 0, 0], vec![vec![0, 0, 0]])
        );
        assert_eq!(
            parse_seed_centre_offsets("0:-1;2:-1", 3).unwrap(),
            vec![vec![-1, 0, 0], vec![0, 0, -1]]
        );
        assert!(parse_seed_centre_offsets(";", 3).is_err());
        assert_eq!(
            seed_recipe_for(&opts, &centres, &other)
                .unwrap()
                .centre_offset,
            vec![0, 1, 0]
        );
        opts.seed_centre_offset = None;
        opts.seed_face_floor = Some(1);
        let r = seed_recipe_for(&opts, &[], &other).unwrap();
        assert_eq!(
            (r.centre_offset.iter().all(|&d| d == 0), r.face_floor),
            (true, Some(1))
        );
    }

    #[test]
    fn scheduled_node_descriptor_defaults_and_validation_are_explicit() {
        let old_options = serde_json::to_value(NodeOptions::default()).unwrap();
        assert!(old_options.get("case_seed_schedule").is_none());
        let restored: NodeOptions = serde_json::from_value(old_options).unwrap();
        assert!(restored.case_seed_schedule.is_none());
        let text = r#"{"ShiftShells":{"first_depth":2,"last_depth":2,"axis_priority":[1,0],"positive_first":true}}"#;
        let native = parse_case_seed_schedule(text, 2).unwrap();
        assert_eq!(
            serde_json::to_value(&native).unwrap(),
            serde_json::from_str::<serde_json::Value>(text).unwrap()
        );
        for invalid in [
            text.replace("[1,0]", "[0]"),
            text.replace("[1,0]", "[0,0]"),
            text.replace("[1,0]", "[0,2]"),
            text.replace("\"first_depth\":2", "\"first_depth\":3"),
            text.replace(
                "\"positive_first\":true",
                "\"positive_first\":true,\"extra\":1",
            ),
        ] {
            assert!(parse_case_seed_schedule(&invalid, 2).is_err(), "{invalid}");
        }
    }

    #[test]
    fn seed_pose_rung_fires_on_numeric_endpoints_only_when_on() {
        // a fully fixed non-master point is a numeric endpoint with a per-point relation
        let base = NodeOptions {
            initial: vec![CaseSpec {
                fixed: vec![Some(2), Some(1), Some(1), Some(0), Some(0)],
                equations: vec![],
            }],
            ..quiet()
        };
        let off_dir = tmp("numeric-rung-off");
        let off = generate(SUNSET, "sunset.sym", &off_dir, &base, &mut |_| {}).unwrap();
        assert!(off.complete, "{off:?}");
        assert_eq!(off.numerical_cases, 1);
        assert_eq!(off.numeric_seed_pose, 0);
        assert!(
            serde_json::to_value(&off)
                .unwrap()
                .get("numeric_seed_pose")
                .is_none(),
            "rung off: OUTCOME bytes unchanged"
        );
        // trigger 0 / min rows 0 / keep < 0: the gate refuses every relation, the portfolio is always measured
        let on_dir = tmp("numeric-rung-on");
        let on = generate(
            SUNSET,
            "sunset.sym",
            &on_dir,
            &NodeOptions {
                seed_pose_rung: true,
                seed_pose_trigger: Some(0.0),
                seed_pose_min_rows: 0,
                seed_pose_keep: -1.0,
                ..base.clone()
            },
            &mut |_| {},
        )
        .unwrap();
        assert!(on.complete, "{on:?}");
        assert_eq!(on.numerical_cases, 1);
        assert_eq!(
            on.numeric_seed_pose, 1,
            "the rung fired on the numeric endpoint"
        );
        assert_eq!(on.rules, off.rules);
        assert_eq!(on.residuals, off.residuals);
        assert_eq!(on.numeric_bulk_fallback, off.numeric_bulk_fallback);
        // the rung only sees large-gated rungs: with the rung on but an unreachable trigger nothing fires
        let quiet_dir = tmp("numeric-rung-quiet");
        let calm = generate(
            SUNSET,
            "sunset.sym",
            &quiet_dir,
            &NodeOptions {
                seed_pose_rung: true,
                seed_pose_trigger: Some(1e300),
                ..base.clone()
            },
            &mut |_| {},
        )
        .unwrap();
        assert!(calm.complete);
        assert_eq!(calm.numeric_seed_pose, 0);
        assert_eq!(
            std::fs::read_to_string(&calm.table)
                .unwrap()
                .lines()
                .filter(|l| l.contains("\"type\":\"rule\""))
                .map(|l| {
                    let mut v: serde_json::Value = serde_json::from_str(l).unwrap();
                    v.as_object_mut().unwrap().remove("lift_seconds");
                    v.as_object_mut().unwrap().remove("seconds_search");
                    v
                })
                .collect::<Vec<_>>(),
            std::fs::read_to_string(&off.table)
                .unwrap()
                .lines()
                .filter(|l| l.contains("\"type\":\"rule\""))
                .map(|l| {
                    let mut v: serde_json::Value = serde_json::from_str(l).unwrap();
                    v.as_object_mut().unwrap().remove("lift_seconds");
                    v.as_object_mut().unwrap().remove("seconds_search");
                    v
                })
                .collect::<Vec<_>>(),
            "an untriggered rung leaves the numeric rules as without the rung"
        );
        // an EXHAUSTED numeric endpoint (depth 0: no relation) is an infinitely costly default: the portfolio is
        // measured on it even with an unreachable trigger; nothing found -> the legacy exhausted route, unchanged
        let ex_base = NodeOptions {
            initial: vec![CaseSpec {
                fixed: vec![Some(1), Some(1), Some(1), Some(0), Some(0)],
                equations: vec![],
            }],
            numeric_depth: 0,
            ..quiet()
        };
        let ex_dir = tmp("numeric-rung-exhausted");
        let ex = generate(
            SUNSET,
            "sunset.sym",
            &ex_dir,
            &NodeOptions {
                seed_pose_rung: true,
                seed_pose_trigger: Some(1e300),
                ..ex_base.clone()
            },
            &mut |_| {},
        )
        .unwrap();
        assert_eq!(ex.numeric_seed_pose, 1, "{ex:?}");
        assert_eq!(ex.numeric_bulk_fallback, 1);
        assert_eq!(ex.residuals, 1);
        let ex_strict_dir = tmp("numeric-rung-exhausted-strict");
        let ex_strict = generate(
            SUNSET,
            "sunset.sym",
            &ex_strict_dir,
            &NodeOptions {
                seed_pose_rung: true,
                strict_numeric_search: true,
                ..ex_base
            },
            &mut |_| {},
        )
        .unwrap();
        assert!(matches!(
            ex_strict.stop,
            Some(StopReason::NumericSearchRefused { .. })
        ));
        // depth escalation (default off): a non-master point exhausted at depth 0 gets its relation at depth <= 2
        let deep_base = NodeOptions {
            numeric_depth: 0,
            ..base.clone()
        };
        let shallow_dir = tmp("numeric-depth-shallow");
        let shallow =
            generate(SUNSET, "sunset.sym", &shallow_dir, &deep_base, &mut |_| {}).unwrap();
        assert_eq!(shallow.numeric_bulk_fallback, 1, "{shallow:?}");
        assert_eq!(shallow.numeric_deeper, 0);
        let deep_dir = tmp("numeric-depth-deeper");
        let deep = generate(
            SUNSET,
            "sunset.sym",
            &deep_dir,
            &NodeOptions {
                numeric_depth_max: Some(2),
                ..deep_base
            },
            &mut |_| {},
        )
        .unwrap();
        assert!(deep.complete, "{deep:?}");
        assert_eq!(deep.numeric_deeper, 1);
        assert_eq!(deep.numeric_bulk_fallback, 0);
        assert_eq!(deep.rules, 1);
        assert!(
            serde_json::to_value(NodeOptions::default())
                .unwrap()
                .get("numeric_depth_max")
                .is_none()
        );
        for d in [
            off_dir,
            on_dir,
            quiet_dir,
            ex_dir,
            ex_strict_dir,
            shallow_dir,
            deep_dir,
        ] {
            let _ = std::fs::remove_dir_all(d);
        }
    }

    #[test]
    fn strict_numeric_search_defaults_preserve_legacy_options() {
        let opts = NodeOptions::default();
        let mut legacy = serde_json::to_value(&opts).unwrap();
        assert!(!opts.strict_numeric_search);
        assert!(legacy.get("strict_numeric_search").is_none());
        let read: NodeOptions = serde_json::from_value(legacy.clone()).unwrap();
        assert!(!read.strict_numeric_search);
        let strict = NodeOptions {
            strict_numeric_search: true,
            ..opts
        };
        assert_eq!(
            serde_json::to_value(&strict).unwrap()["strict_numeric_search"],
            true
        );
        legacy["strict_numeric_search"] = serde_json::json!(false);
        assert_eq!(
            serde_json::to_value(serde_json::from_value::<NodeOptions>(legacy).unwrap()).unwrap(),
            serde_json::to_value(NodeOptions::default()).unwrap()
        );
    }

    #[test]
    fn symmetry_rows_in_the_shared_solve_identify_symmetric_endpoints_and_replay() {
        // sym-gen step 2: the three single-dot sunset endpoints form one orbit of the chart group (line permutations,
        // exact on ISP-free keys). At depth 0 the per-point search has no relation; the shared solve with exact
        // symmetry rows leaves at most one of them, every rule replays on IBP + verified symmetry rows.
        let points: Vec<CaseSpec> = [[2, 1, 1], [1, 2, 1], [1, 1, 2]]
            .iter()
            .map(|p| CaseSpec {
                fixed: vec![Some(p[0]), Some(p[1]), Some(p[2]), Some(0), Some(0)],
                equations: vec![],
            })
            .collect();
        let base = NodeOptions {
            initial: points,
            numeric_depth: 0,
            ..quiet()
        };
        let off_dir = tmp("symrows-off");
        let off = generate(SUNSET, "sunset.sym", &off_dir, &base, &mut |_| {}).unwrap();
        assert!(off.complete, "{off:?}");
        assert_eq!(off.numeric_bulk_fallback, 3);
        assert_eq!(off.sym_rows, 0);
        let off_table = std::fs::read_to_string(&off.table).unwrap();
        assert!(!off_table.contains("symmetry_group") && !off_table.contains("sym_rows"));
        let on_dir = tmp("symrows-on");
        let on = generate(
            SUNSET,
            "sunset.sym",
            &on_dir,
            &NodeOptions {
                symmetry_rows: true,
                ..base.clone()
            },
            &mut |_| {},
        )
        .unwrap();
        assert!(on.complete, "{on:?}");
        assert!(on.sym_rows > 0 && on.sym_rows_independent > 0, "{on:?}");
        assert!(on.residuals <= off.residuals, "{on:?} {off:?}");
        let dotted = on.residual_keys.iter().filter(|k| k.contains('2')).count();
        assert!(dotted <= 1, "{:?}", on.residual_keys);
        assert!(on.sym_row_rules > 0, "{on:?}");
        assert!(on.not_lower.is_empty());
        let table = std::fs::read_to_string(&on.table).unwrap();
        assert!(table.contains("\"type\":\"symmetry_group\"") && table.contains("\"sym_rows\":"));
        assert_eq!(replay_status(&on_dir, &on.table), "PASS_RS2_RS3_SAMPLED");
        let _ = std::fs::remove_dir_all(off_dir);
        let _ = std::fs::remove_dir_all(on_dir);
    }

    #[test]
    fn symmetry_symbolic_whole_sunset_chart_replays_and_defaults_off() {
        // sym-7master: the flag adds case-stabiliser symmetry rows to the symbolic case search (and turns on the
        // bulk-solve rows). The whole sunset chart must still close, never with more residuals than IBP only, and
        // every rule (symbolic ones included) must replay on IBP + the verified symmetry rows. Off: no field.
        assert!(
            serde_json::to_value(NodeOptions::default())
                .unwrap()
                .get("symmetry_symbolic")
                .is_none()
        );
        let off_dir = tmp("symsym-off");
        let off = generate(SUNSET, "sunset.sym", &off_dir, &quiet(), &mut |_| {}).unwrap();
        assert!(off.complete, "{off:?}");
        let on_dir = tmp("symsym-on");
        let on = generate(
            SUNSET,
            "sunset.sym",
            &on_dir,
            &NodeOptions {
                symmetry_symbolic: true,
                ..quiet()
            },
            &mut |_| {},
        )
        .unwrap();
        assert!(on.complete, "{on:?}");
        assert!(on.residuals <= off.residuals, "{on:?} {off:?}");
        assert!(on.not_lower.is_empty());
        let table = std::fs::read_to_string(&on.table).unwrap();
        assert!(table.contains("\"type\":\"symmetry_group\""));
        assert_eq!(replay_status(&on_dir, &on.table), "PASS_RS2_RS3_SAMPLED");
        let _ = std::fs::remove_dir_all(off_dir);
        let _ = std::fs::remove_dir_all(on_dir);
    }

    #[test]
    fn strict_numeric_search_refuses_real_exhausted_dispatch_without_master_claim() {
        let fixed = vec![Some(1), Some(1), Some(1), Some(0), Some(0)];
        let base = NodeOptions {
            initial: vec![CaseSpec {
                fixed: fixed.clone(),
                equations: vec![],
            }],
            numeric_depth: 0,
            ..quiet()
        };
        // A depth-zero per-point search has no relation. The legacy shared solver may still classify residuals.
        let legacy_dir = tmp("numeric-legacy");
        let legacy = generate(SUNSET, "sunset.sym", &legacy_dir, &base, &mut |_| {}).unwrap();
        assert!(legacy.complete, "{legacy:?}");
        assert_eq!(legacy.numeric_bulk_fallback, 1);
        assert_eq!(legacy.rules, 0);
        assert_eq!(legacy.residuals, 1);

        let strict_dir = tmp("numeric-strict");
        let strict = generate(
            SUNSET,
            "sunset.sym",
            &strict_dir,
            &NodeOptions {
                strict_numeric_search: true,
                ..base
            },
            &mut |_| {},
        )
        .unwrap();
        assert!(!strict.complete);
        assert_eq!(strict.numeric_bulk_fallback, 0);
        assert_eq!(strict.rules, 0);
        assert_eq!(strict.residuals, 0);
        match &strict.stop {
            Some(StopReason::NumericSearchRefused { cases }) => {
                assert_eq!(cases.len(), 1);
                assert_eq!(cases[0].fixed, fixed);
                assert!(cases[0].equations.is_empty());
            }
            other => panic!("expected retained open numeric case, got {other:?}"),
        }
        let queue = std::fs::read_to_string(strict_dir.join("F.queue.jsonl")).unwrap();
        assert!(queue.contains("\"type\":\"numeric_shared_refused\""));
        // the refused endpoint is not lost: it stays an open obligation of the run (re-pooled by the next wave)
        let o = obligations(&strict_dir);
        assert_eq!(o.state, "stopped");
        assert!(
            o.open
                .iter()
                .any(|x| x.kind == "numerical" && x.case.fixed == fixed),
            "{:?}",
            o.open
        );
        let table = std::fs::read_to_string(&strict.table).unwrap();
        assert!(!table.contains("\"type\":\"residual\""));
        let _ = std::fs::remove_dir_all(legacy_dir);
        let _ = std::fs::remove_dir_all(strict_dir);
    }

    #[test]
    fn numeric_backend_follows_the_symbolic_backend() {
        assert_eq!(
            numerical_backend(SymbolicExactBackend::SparseTargetOnlyFactorized),
            crate::solver::NumericalExactBackend::SparseFactorized
        );
        assert_eq!(
            numerical_backend(SymbolicExactBackend::SparseFactorized),
            crate::solver::NumericalExactBackend::SparseFactorized
        );
        assert_eq!(
            numerical_backend(SymbolicExactBackend::Sparse),
            crate::solver::NumericalExactBackend::Sparse
        );
        assert_eq!(
            numerical_backend(SymbolicExactBackend::SparseTargetOnly),
            crate::solver::NumericalExactBackend::Sparse
        );
    }

    #[test]
    fn presented_family_is_solved_and_case_specs_parse() {
        let dir = tmp("perm");
        let order = NodeOrder {
            name: "isp_rev_all".into(),
            slot_permutation: Some(vec![0, 1, 2, 4, 3]),
            ..NodeOrder::identity()
        };
        let out = generate(
            SUNSET,
            "sunset.sym",
            &dir,
            &NodeOptions { order, ..quiet() },
            &mut |_| {},
        )
        .unwrap();
        assert!(out.complete);
        let presented = std::fs::read_to_string(dir.join("F.family.sym")).unwrap();
        assert!(presented.contains("prop(D4,(k2+q)^2,0)"));
        assert_eq!(
            CaseSpec::parse("1,_,0", "n1-2").unwrap(),
            CaseSpec {
                fixed: vec![Some(1), None, Some(0)],
                equations: vec!["n1-2".into()]
            }
        );
        let _ = std::fs::remove_dir_all(&dir);
    }
}
