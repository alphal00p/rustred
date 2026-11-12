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
    /// Refuse exhausted per-point numeric searches instead of entering the unscreened shared numeric solve.
    /// The unfinished points stay explicit obligations; this does not classify any of them as masters.
    #[serde(default, skip_serializing_if = "is_false")]
    pub strict_numeric_search: bool,
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
    /// Shared case registry of the parallel pieces of ONE node: (directory, piece name). Every rule of this piece is
    /// appended to `<dir>/<piece>.rules.jsonl`; before each case the sibling files are read incrementally (same
    /// presented family, chart and order only): a case contained in a sibling rule's case is CLAIMED by that sibling
    /// and skipped (the sibling closes its exceptional children), and sibling rules act as cover rules for children.
    /// The node is complete only when every piece completes; its table is the union of the pieces' tables.
    ///
    /// DISABLED unless `unsafe_shared_registry` is set (M5 batch 3, coordinator ruling 2026-11-11 17:37Z): pop
    /// pruning transfers ownership of a case by case containment, not by a valid-domain proof; the final join does
    /// not yet keep exception-child dependencies, and the registry key does not bind the full source basis and
    /// certificate provenance. Until that is fixed a node run with `registry` set and the switch off is refused.
    #[serde(default)]
    pub registry: Option<(String, String)>,
    /// Opt-in for the experimental shared case registry (default off; see `registry`).
    #[serde(default)]
    pub unsafe_shared_registry: bool,
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
}

impl SeedScheduleName {
    pub fn schedule(self) -> crate::solver::SeedSchedule {
        match self {
            Self::Spired => crate::solver::SeedSchedule::Spired,
            Self::SymbolicFirst => crate::solver::SeedSchedule::SymbolicFirst,
        }
    }
}

fn yes() -> bool {
    true
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
            strict_numeric_search: false,
            no_witness_guards: false,
            seed_schedule: SeedScheduleName::Spired,
            case_seed_schedule: None,
            case_seed_priority: None,
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
    /// Per-point numeric searches exhausted or were screened out, and strict mode forbids the shared fallback.
    /// Cases are in PRESENTED slots and remain open, including any possible genuine masters.
    NumericSearchRefused {
        cases: Vec<CaseSpec>,
    },
    Error(String),
}

/// Result of [`generate`]. `complete` = the chart closed (no stop, no error).
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct NodeOutcome {
    pub complete: bool,
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
    /// Cases dropped because a sibling piece's rule (shared registry) covers them.
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
    match n {
        5 => run::<5>(
            family_text,
            family_path,
            &presented,
            &fam,
            out_dir,
            opts,
            events,
        ),
        9 => run::<9>(
            family_text,
            family_path,
            &presented,
            &fam,
            out_dir,
            opts,
            events,
        ),
        12 => run::<12>(
            family_text,
            family_path,
            &presented,
            &fam,
            out_dir,
            opts,
            events,
        ),
        14 => run::<14>(
            family_text,
            family_path,
            &presented,
            &fam,
            out_dir,
            opts,
            events,
        ),
        20 => run::<20>(
            family_text,
            family_path,
            &presented,
            &fam,
            out_dir,
            opts,
            events,
        ),
        other => Err(format!(
            "arity {other}: node driver compiled for 5, 9, 12, 14, 20"
        )),
    }
}

/// The node order's [`SectorConfig`] pieces: legacy order, or the compiled tie-priority program.
fn order_program(
    order: &NodeOrder,
    n: usize,
) -> Result<Option<rustred_order::CompiledOrder>, String> {
    match &order.coordinate_priority {
        None => Ok(None),
        Some(priority) => {
            let mut d = rustred_order::OrderDescriptor::spired_uncut(n);
            d.coordinate_priority = priority.clone();
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

fn case_json<const N: usize>(c: &Case<N>, n: usize) -> String {
    serde_json::to_string(&case_spec(c, n)).expect("case json")
}

/// Linear integer expression in `n<i>` (`3*n1-n2+4`) as a coefficient polynomial of the family's index variables.
fn parse_index_poly(
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
                c = &c * &symbolica::domains::integer::Integer::from(f.parse::<i64>().ok()?);
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

fn build_case<const N: usize>(
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
        .map(|t| if t.len() > 4000 { "elided".to_string() } else { t })
        .collect();
    let opt = |v: &Option<Vec<i16>>| v.as_ref().map_or("null".to_string(), |v| format!("{v:?}"));
    let raw = x
        .raw_pivot
        .as_ref()
        .map_or("null".to_string(), |v| format!("{v:?}"));
    writeln!(
        f,
        "{{\"type\":\"rule\",\"chart\":{},\"index\":{index},\"case\":{},\"fixed\":[{}],\"equations\":{},\"target\":{},\"rhs\":[{}],\"exceptions\":[{}],\"dispatch\":{},\"nsources\":{},\"seeds\":{},\"direct_hit\":{},\"seconds_search\":{:.6},\"raw_pivot_encoded\":{raw},\"recenter\":{},\"trace_rows\":{},\"lift_seconds\":{:.6},\"source_order\":{},\"numerical\":{},\"monomials\":{},\"oversized_fallback\":{},\"witness_poles\":{},\"witness_loci\":{}}}",
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
fn covers<const N: usize>(
    rcase: &Case<N>,
    branches: &[Vec<CoefficientPolynomial>],
    case: &Case<N>,
    idx: &[usize; N],
    sector: &[bool; N],
    limits: CaseIntersectionLimits,
) -> bool {
    rcase.contains(case).unwrap_or(false)
        && branches.iter().all(
            |b| matches!(case.intersect_many(b, idx, sector, limits), Ok(r) if r.cases.is_empty()),
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
fn union_covered<const N: usize>(
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

/// Shared case registry of the parallel pieces of one node (see [`NodeOptions::registry`]).
struct Registry {
    dir: PathBuf,
    own: PathBuf,
    file: std::fs::File,
    piece: String,
    key: String,
    offsets: std::collections::BTreeMap<PathBuf, u64>,
}

impl Registry {
    fn open(dir: &str, piece: &str, key: &str) -> Result<Self, String> {
        let dir = PathBuf::from(dir);
        std::fs::create_dir_all(&dir).map_err(|e| format!("registry {}: {e}", dir.display()))?;
        let own = dir.join(format!("{piece}.rules.jsonl"));
        let mut file = std::fs::OpenOptions::new()
            .create(true)
            .write(true)
            .truncate(true)
            .open(&own)
            .map_err(|e| format!("registry {}: {e}", own.display()))?;
        writeln!(
            file,
            "{{\"type\":\"registry_header\",\"piece\":{},\"key\":{}}}",
            q(piece),
            q(key)
        )
        .map_err(|e| e.to_string())?;
        Ok(Self {
            dir,
            own,
            file,
            piece: piece.to_string(),
            key: key.to_string(),
            offsets: Default::default(),
        })
    }

    fn publish(&mut self, index: usize, spec: &CaseSpec, branches: &[Vec<CoefficientPolynomial>]) {
        let ex: Vec<String> = branches.iter().map(|b| strings(b.iter())).collect();
        // one write per line (O_APPEND not needed: one writer per file); readers take complete lines only
        let line = format!(
            "{{\"type\":\"rule\",\"piece\":{},\"key\":{},\"index\":{index},\"fixed\":{},\"equations\":{},\"exceptions\":[{}]}}\n",
            q(&self.piece),
            q(&self.key),
            serde_json::to_string(&spec.fixed).unwrap_or_default(),
            strings(spec.equations.iter()),
            ex.join(",")
        );
        let _ = self.file.write_all(line.as_bytes());
        let _ = self.file.flush();
    }

    /// New complete rule lines of sibling pieces (same key), as JSON values with a `registry_label`.
    fn poll(&mut self) -> Vec<(String, serde_json::Value)> {
        let mut out = Vec::new();
        let Ok(rd) = std::fs::read_dir(&self.dir) else {
            return out;
        };
        let mut files: Vec<PathBuf> = rd
            .filter_map(|e| e.ok().map(|e| e.path()))
            .filter(|p| p.to_string_lossy().ends_with(".rules.jsonl") && *p != self.own)
            .collect();
        files.sort();
        for path in files {
            let off = *self.offsets.get(&path).unwrap_or(&0);
            let Ok(bytes) = std::fs::read(&path) else {
                continue;
            };
            if (bytes.len() as u64) <= off {
                continue;
            }
            let tail = &bytes[off as usize..];
            let Some(last_nl) = tail.iter().rposition(|b| *b == b'\n') else {
                continue;
            };
            let complete = &tail[..=last_nl];
            self.offsets
                .insert(path.clone(), off + complete.len() as u64);
            for line in String::from_utf8_lossy(complete).lines() {
                let Ok(v) = serde_json::from_str::<serde_json::Value>(line) else {
                    continue;
                };
                if v["type"] == "rule" && v["key"].as_str() == Some(self.key.as_str()) {
                    let label = format!(
                        "registry:{}#{}",
                        v["piece"].as_str().unwrap_or("?"),
                        v["index"]
                    );
                    out.push((label, v));
                }
            }
        }
        out
    }
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
            Ok((x, report)) => Guards::Ok(
                x,
                Some(format!("{report:?}")),
                report.route == GuardRouteTaken::Content,
            ),
            Err(e) => Guards::Err(format!("{e:?}")),
        };
    }
    let r = forked(out_dir, tag, secs, || {
        match extract_exceptions(candidate, idx, sector) {
            Ok(_) => "ok".to_string(),
            Err(e) => format!("err {e:?}"),
        }
    });
    if !matches!(r, Forked::Done(..)) {
        return Guards::Timeout(secs);
    }
    match extract_exceptions(candidate, idx, sector) {
        Ok(x) => Guards::Ok(x, None, false),
        Err(e) => Guards::Err(format!("{e:?}")),
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
    // SAFETY: fork in a process whose other threads (if any) hold no locks the child needs; the child only runs
    // `f`, writes one file and _exits.
    let pid = unsafe { fork() };
    if pid < 0 {
        return Forked::Failed("fork failed".into(), 0.0);
    }
    if pid == 0 {
        let s = std::panic::catch_unwind(std::panic::AssertUnwindSafe(f))
            .unwrap_or_else(|_| "{\"child_panic\":true}".to_string());
        let tmp = path.with_extension("part");
        let ok = std::fs::write(&tmp, s.as_bytes()).is_ok() && std::fs::rename(&tmp, &path).is_ok();
        // SAFETY: terminate the forked child without running the parent's destructors.
        unsafe { _exit(if ok { 0 } else { 3 }) }
    }
    FORK_CHILD.store(pid, std::sync::atomic::Ordering::SeqCst);
    loop {
        let mut status = 0i32;
        // SAFETY: own child, WNOHANG.
        let r = unsafe { waitpid(pid, &mut status, 1) };
        if r == pid {
            FORK_CHILD.store(0, std::sync::atomic::Ordering::SeqCst);
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
            // SAFETY: own child.
            unsafe {
                kill(pid, 9);
                waitpid(pid, &mut status, 0);
            }
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
        std::fs::File::create(&table_path).map_err(|e| format!("table: {e}"))?,
    );
    let mut qlog = std::io::BufWriter::new(
        std::fs::File::create(out_dir.join("F.queue.jsonl"))
            .map_err(|e| format!("queue log: {e}"))?,
    );
    let options_json = serde_json::to_string(opts).map_err(|e| e.to_string())?;
    writeln!(
        out,
        "{{\"type\":\"header\",\"schema\":{},\"node_schema\":{},\"source_base\":\"rustred-node\",\"family\":{},\"family_file\":{},\"family_source\":{},\"original_family_source\":{},\"loops\":{},\"externals\":{},\"denominators\":{n},\"parameters\":{},\"power_shifts\":{},\"mask\":{},\"node_order\":{},\"order_name\":{},\"order_is_program\":{},\"order\":{},\"two_sided_axes\":{:?},\"charts\":{},\"numerical_depth\":{},\"max_numerator_rank\":null,\"source_conditions\":{},\"zero_scope\":\"downset\",\"zero_seconds\":{zero_seconds:.3},\"guard_version\":{},\"node_options\":{options_json}}}",
        q(SCHEMA),
        q(NODE_SCHEMA),
        q(fam.name()),
        q(family_path),
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
        if opts.no_witness_guards { 1 } else { GUARD_VERSION },
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
    let config = SectorConfig {
        zero_sectors: Arc::from(zeros.clone()),
        integral_order: order_program(&opts.order, n)?,
        symbolic_exact_backend: exact_backend,
        // the shared numeric solve follows the chosen backend (Astra: it used to default to Sparse regardless)
        numerical_exact_backend: numerical_backend(exact_backend),
        seed_schedule: opts.seed_schedule.schedule(),
        witness_guards: !opts.no_witness_guards,
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
    // certificate file: header, original source rows, basis derivation (replay authority for retained traces)
    let mut cert = std::io::BufWriter::new(
        std::fs::File::create(out_dir.join("F.cert.jsonl")).map_err(|e| format!("cert: {e}"))?,
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
    // cover tables (rules of earlier pieces, FirstValid order)
    let mut cover: Vec<CoverEntry<N>> = Vec::new();
    for (path, upto) in &opts.cover_tables {
        let text = std::fs::read_to_string(path).map_err(|e| format!("cover table {path}: {e}"))?;
        if terminal_ledger.is_some() {
            let header: serde_json::Value = serde_json::from_str(
                text.lines()
                    .next()
                    .ok_or("known terminals: empty cover table")?,
            )
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
        }
        let (mut loaded, mut skipped) = (0usize, 0usize);
        for line in text.lines().filter(|l| l.starts_with("{\"type\":\"rule\"")) {
            let v: serde_json::Value =
                serde_json::from_str(line).map_err(|e| format!("cover table {path}: {e}"))?;
            if v["index"].as_u64().is_none_or(|i| i as usize >= *upto) {
                continue;
            }
            if let Some(ledger) = &terminal_ledger {
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
            }
            let spec = CaseSpec {
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
            };
            let case = build_case::<N>(&spec, &template, &idx, &sector, n)
                .ok()
                .flatten();
            let branches: Option<Vec<Vec<CoefficientPolynomial>>> =
                v["exceptions"].as_array().map(|bs| {
                    bs.iter()
                        .map(|b| {
                            b.as_array()
                                .map(|ps| {
                                    ps.iter()
                                        .filter_map(|p| {
                                            p.as_str()
                                                .and_then(|p| parse_index_poly(p, &template, &idx))
                                        })
                                        .collect()
                                })
                                .unwrap_or_default()
                        })
                        .collect()
                });
            match (case, branches) {
                (Some(c), Some(b)) => {
                    cover.push((format!("{path}#{}", v["index"]), c, b));
                    loaded += 1;
                }
                _ => skipped += 1,
            }
        }
        writeln!(qlog, "{{\"type\":\"cover_table\",\"path\":{},\"upto\":{upto},\"loaded\":{loaded},\"skipped\":{skipped}}}", q(path)).map_err(|e| e.to_string())?;
    }
    // initial cases (ORIGINAL slots -> presented)
    let mut initial: Vec<Case<N>> = Vec::new();
    if opts.initial.is_empty() {
        initial.push(Case::from(
            CoordinateCase::new(std::array::from_fn(|i| if i < n { None } else { Some(0) }))
                .map_err(|e| format!("{e:?}"))?,
        ));
    }
    for spec in &opts.initial {
        let (fixed, eqs) = opts.order.present_case(&spec.fixed, &spec.equations);
        match build_case::<N>(
            &CaseSpec {
                fixed,
                equations: eqs,
            },
            &template,
            &idx,
            &sector,
            n,
        )? {
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
    let key = format!(
        "{:016x}",
        fnv64(&[
            presented,
            &label,
            &descriptor_json(&opts.order, n),
            &nb.to_string()
        ])
    );
    let mut registry = match &opts.registry {
        Some(_) if !opts.unsafe_shared_registry => {
            return Err(
                "the shared case registry is disabled (ownership by case containment is not a \
                        valid-domain proof; M5 batch 3): set unsafe_shared_registry (rr_node \
                        --unsafe-shared-registry) to run it experimentally"
                    .to_string(),
            );
        }
        Some((dir, piece)) => Some(Registry::open(dir, piece, &key)?),
        None => None,
    };
    let mut bank = if opts.refused_bank {
        let mut f = std::io::BufWriter::new(
            std::fs::File::create(out_dir.join("F.refused.jsonl"))
                .map_err(|e| format!("refused bank: {e}"))?,
        );
        writeln!(
            f,
            "{{\"type\":\"bank_header\",\"schema\":\"rustred.node.refused_bank.v1\",\"key\":{},\"chart\":{},\"order_name\":{},\"basis\":{nb},\"accept_monomials\":{},\"fallback_monomials\":{},\"fallback_after\":{}}}",
            q(&key),
            q(&label),
            q(&opts.order.name),
            opts.max_rule_monomials.map_or("null".into(), |v| v.to_string()),
            opts.fallback_rule_monomials.map_or("null".into(), |v| v.to_string()),
            opts.fallback_after
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
        if header["key"].as_str() != Some(key.as_str()) {
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
    let mut deferred_charts: Vec<Case<N>> = Vec::new();
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
    let search = |case: &Case<N>,
                  so: &SourceOrder,
                  seeds: Option<crate::solver::SeedSchedule>,
                  large_gate: bool,
                  options: SearchOptions|
     -> (Result<Searched<N>, SolverError>, Vec<ScreenReport>) {
        // the large-trace "SymbolicFirst rung first" gate applies to the first rung only; later rungs lift such traces
        let screen = screen.map(|mut s| {
            if !large_gate {
                s.symbolic_first_from_rows = None;
            }
            s
        });
        let mut raw: Option<Integral<N>> = None;
        let mut started: Option<Instant> = None;
        let mut lift = 0.0f64;
        let result = super::screen::with_seed_schedule(seeds, || {
            super::screen::with_screen(screen, || {
                let observe = |event: SearchEvent<N>| match event {
                    SearchEvent::ExactStarted {
                        pivot, trace_rows, ..
                    } => {
                        raw = Some(pivot);
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
                if let Some(schedule) = resolved.as_ref().or(fallback) {
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
                        None => solver.solve_case_with_observer(case.clone(), options, observe),
                        Some(order) => solver.solve_case_with_source_order_and_observer(
                            case.clone(),
                            options,
                            &order,
                            observe,
                        ),
                    }
                }
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
    let mut symbolic_cases = 0usize;
    while !pending.is_empty() {
        if opts.max_cases.is_some_and(|m| symbolic_cases >= m) {
            stop = Some(StopReason::MaxCases {
                pending: pending.len(),
            });
            break;
        }
        let current = pending.remove(0);
        if terminal_ledger
            .as_ref()
            .is_some_and(|ledger| ledger.is_point(&current, n))
        {
            writeln!(qlog, "{{\"type\":\"declared_terminal\",\"authority\":\"DECLARED_NON_NATIVE\",\"case\":{}}}", case_json(&current, n)).map_err(|e| e.to_string())?;
            continue;
        }
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
        // shared registry: the sibling pieces' new rules become cover entries; the popped case may be covered by now
        if let Some(reg) = registry.as_mut() {
            for (lbl, v) in reg.poll() {
                if let Some((c, b)) = parse_cover_entry::<N>(&v, &template, &idx, &sector, n) {
                    cover.push((lbl, c, b));
                }
            }
            // a sibling rule whose case CONTAINS this case claims it: the sibling's rule covers it outside its
            // branches and the sibling's own children (rule case ∩ branches, a superset of this case ∩ branches)
            // are the sibling's queue; the node is complete only when every piece is
            if let Some((lbl, _, _)) = cover.iter().find(|(l, c, _)| {
                l.starts_with("registry:") && c.contains(&current).unwrap_or(false)
            }) {
                outcome.registry_covered += 1;
                discarded += 1;
                writeln!(
                    qlog,
                    "{{\"type\":\"covered\",\"by_registry\":{},\"case\":{}}}",
                    q(lbl),
                    case_json(&current, n)
                )
                .map_err(|e| e.to_string())?;
                continue;
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
                            deferred_charts.push(d.chart.clone());
                            writeln!(qlog, "{{\"type\":\"deferred_region\",\"rule\":{index},\"branch\":{bi},\"k\":{di},\"reason\":{},\"chart\":{},\"conjunction\":{}}}", q(&format!("{:?}", d.reason)), case_json(&d.chart, n), strings(d.conjunction.iter())).map_err(|e| e.to_string())?;
                        }
                        children.extend(r.cases);
                    }
                    Err(e) => {
                        deferred_branches += 1;
                        deferred_charts.push(rc.clone());
                        writeln!(qlog, "{{\"type\":\"deferred_branch\",\"rule\":{index},\"branch\":{bi},\"polys\":{},\"error\":{}}}", strings(branch.iter()), q(&format!("{e:?}").chars().take(4000).collect::<String>())).map_err(|e| e.to_string())?;
                    }
                }
            }
            if children.iter().any(|ch| ch == &current) {
                error = Some(format!("NonProgress on {current:?} (banked rule {src})"));
                break;
            }
            if let Some(reg) = registry.as_mut() {
                reg.publish(index, &spec, &branches);
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
        // lift ladder: node source order; the SymbolicFirst seed schedule (same presentation); other source orders
        // (source order, seed schedule override, large-trace gate)
        let mut orders: Vec<(SourceOrder, Option<crate::solver::SeedSchedule>, bool)> =
            vec![(node_source.clone(), None, true)];
        if opts.symbolic_first_rung
            && screen.is_some()
            && opts.seed_schedule != SeedScheduleName::SymbolicFirst
        {
            orders.push((
                node_source.clone(),
                Some(crate::solver::SeedSchedule::SymbolicFirst),
                true,
            ));
            // SymbolicFirst did not help (e.g. it found a larger relation): the node's own relation, lifted
            orders.push((node_source.clone(), None, false));
        }
        for so in &opts.retry_source_orders {
            if !orders.iter().any(|(o, _, _)| o == so) {
                orders.push((so.clone(), None, false));
            }
        }
        let mut found: Option<Searched<N>> = None;
        let mut refusals: Vec<ScreenReport> = Vec::new();
        let mut tried_large: Vec<bool> = Vec::new();
        // size ruling: the smallest fallback rule (accept < size <= fallback limit) with its guards
        let mut fallback: Option<(usize, Searched<N>, ExceptionalConditions, String, usize)> = None;
        let (mut variants_tried, mut since_fallback) = (0usize, 0usize);
        for (k, (so, seeds, large_gate)) in orders.iter().enumerate() {
            // the ungated retry of the node's relation only matters when a large-trace refusal happened
            if !*large_gate
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
                (Some(_), _) => format!("{}+symbolic_first", so.label()),
                (None, false) if *so == node_source => format!("{}+lift_large", so.label()),
                _ => so.label(),
            };
            if k > 0 {
                if seeds.is_some() {
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
            let (result, reports) = search(&current, so, *seeds, *large_gate, symbolic);
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
                if !matches!(r, Forked::Done(..)) {
                    writeln!(qlog, "{{\"type\":\"guard_timeout\",\"rule\":{},\"case\":{},\"rhs\":{},\"timeout\":{secs}}}", rules.len() + adopted_rules, case_json(&current, n), candidate.rhs.len()).map_err(|e| e.to_string())?;
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
        terms += write_rule(&mut out, &label, index, &rule, &extras, n);
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
                if !matches!(r, Forked::Done(..)) {
                    deferred_regions += 1;
                    deferred_charts.push(rule.candidate.case.clone());
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
                        deferred_charts.push(d.chart.clone());
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
                    deferred_charts.push(rule.candidate.case.clone());
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
            reg.publish(
                index,
                &case_spec(&rule.candidate.case, n),
                &rule.exceptions.branches,
            );
        }
        rules.push(rule);
    }
    *WATCH.lock().unwrap() = None;
    // ---- numeric endpoints: one by one under the node's source order and exact backend; exhausted ones in bulk
    let mut residual_keys: Vec<String> = terminal_ledger.as_ref().map_or_else(Vec::new, |l| {
        l.terminals.iter().map(|t| t.key.clone()).collect()
    });
    if error.is_none() && stop.is_none() && !numerical.is_empty() {
        outcome.numerical_cases = numerical.len();
        let options = SearchOptions {
            max_depth: Some(opts.numeric_depth),
            ..symbolic
        };
        let mut exhausted: Vec<CoordinateCase<N>> = Vec::new();
        for point in numerical.clone() {
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
            // every numeric path honours the node's source order (then the retry source orders) and its exact
            // backend; a point the per-point search exhausts or refuses under all of them goes to the shared solve
            let mut numeric_orders: Vec<SourceOrder> = vec![node_source.clone()];
            for so in &opts.retry_source_orders {
                if !numeric_orders.contains(so) {
                    numeric_orders.push(so.clone());
                }
            }
            let mut result: Result<Searched<N>, SolverError> = Err(SolverError::SearchExhausted {
                depth: opts.numeric_depth,
                rows: 0,
            });
            for (k, so) in numeric_orders.iter().enumerate() {
                if k > 0 {
                    writeln!(qlog, "{{\"type\":\"relation_retry\",\"numerical\":true,\"source_order\":{},\"fixed\":{fixed:?}}}", q(&so.label())).map_err(|e| e.to_string())?;
                }
                log_case_seed_schedule(&mut qlog, opts, &case, &top, n)?;
                let (r, reports) = search(&case, so, None, false, options);
                for rep in &reports {
                    outcome.screened_lifts += 1;
                    writeln!(qlog, "{{\"type\":\"lift_screen\",\"numerical\":true,\"source_order\":{},\"report\":{}}}", q(&so.label()), serde_json::to_string(rep).unwrap()).map_err(|e| e.to_string())?;
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
                    let extras = rule_extras(
                        &solver,
                        &rule,
                        searched.raw_pivot,
                        searched.lift_seconds,
                        searched.source_order,
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
                    terms += write_rule(&mut out, &label, index, &rule, &extras, n);
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
        if error.is_none() && !exhausted.is_empty() && opts.strict_numeric_search {
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
        if error.is_none() && stop.is_none() && !exhausted.is_empty() {
            outcome.numeric_bulk_fallback = exhausted.len();
            let visit = node_source.visit(nb);
            match solver.solve_numeric_cases_with_source_order(exhausted, options, visit.as_deref())
            {
                Ok(result) => {
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
                            let nl: Vec<String> = candidate
                                .rhs
                                .iter()
                                .filter(|t| {
                                    solver.ordering().compare(&candidate.target, &t.integral)
                                        != Ordering::Less
                                })
                                .map(|t| t.integral.to_string())
                                .collect();
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
                            terms += write_rule(&mut out, &label, index, &rule, &extras, n);
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
    }
    // deferred regions are obligations: a region is settled when the final rule set covers its chart
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
        let checked = forked(out_dir, "deferred_check", 600, || {
            let flags: Vec<bool> = deferred_charts
                .iter()
                .map(|chart| {
                    let direct = entries
                        .iter()
                        .any(|(c, b)| covers(c, b, chart, &idx, &sector, limits));
                    let mut budget = 400usize;
                    direct || union_covered(chart, &entries, &idx, &sector, limits, 3, &mut budget)
                })
                .collect();
            serde_json::to_string(&flags).unwrap_or_default()
        });
        let flags: Vec<bool> = match checked {
            Forked::Done(text, _) => {
                serde_json::from_str(&text).unwrap_or_else(|_| vec![false; deferred_charts.len()])
            }
            _ => vec![false; deferred_charts.len()],
        };
        let open: Vec<CaseSpec> = deferred_charts
            .iter()
            .zip(flags.iter().chain(std::iter::repeat(&false)))
            .filter(|(_, ok)| !**ok)
            .map(|(c, _)| case_spec(c, n))
            .collect();
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
    let active_partial = terminal_ledger.is_some()
        && (error.is_some() || stop.is_some() || !outcome.not_lower.is_empty());
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
    writeln!(
        out,
        "{{\"type\":\"{chart_record}\",\"chart\":{},\"seconds\":{seconds:.3},\"rules\":{},\"rhs_terms\":{terms},\"branches\":{branches_total},\"residuals\":{},\"symbolic_cases\":{symbolic_cases},\"numerical_cases\":{},\"discarded_cases\":{discarded},\"finite_case_policy\":\"node-queue\",\"deferred_cases\":0,\"deferred_branches\":{deferred_branches},\"deferred_regions\":{deferred_regions}}}",
        q(&label),
        rules.len() + adopted_rules,
        residual_keys.len(),
        outcome.numerical_cases
    )
    .map_err(|e| e.to_string())?;
    let chart_errors = usize::from(error.is_some() || stop.is_some());
    writeln!(
        out,
        "{{\"type\":\"{end_record}\",\"charts\":1,\"rules\":{},\"rhs_terms\":{terms},\"residuals\":{},\"chart_errors\":{chart_errors},\"seconds\":{:.3},\"vmhwm_kb\":{},\"harness\":\"rustred node driver (rr-consolidate)\",\"native_descent_not_lower\":{}{terminal_note}}}",
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
        "{{\"type\":\"{end_record}\",\"error\":{},\"rules\":{},\"residuals\":{},\"seconds\":{seconds:.3}}}",
        error.as_deref().map_or("null".to_string(), q),
        rules.len() + adopted_rules,
        residual_keys.len()
    )
    .map_err(|e| e.to_string())?;
    qlog.flush().map_err(|e| e.to_string())?;
    if let Some(e) = error {
        stop = Some(StopReason::Error(e));
    }
    outcome.complete = stop.is_none() && outcome.not_lower.is_empty();
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
    fn registry_pieces_see_each_others_rules() {
        let reg = tmp("registry");
        let key = "k";
        let mut a = Registry::open(reg.to_str().unwrap(), "a", key).unwrap();
        let mut b = Registry::open(reg.to_str().unwrap(), "b", key).unwrap();
        let mut other = Registry::open(reg.to_str().unwrap(), "x", "other-key").unwrap();
        a.publish(
            0,
            &CaseSpec {
                fixed: vec![Some(1), None],
                equations: vec![],
            },
            &[],
        );
        other.publish(
            0,
            &CaseSpec {
                fixed: vec![None, None],
                equations: vec![],
            },
            &[],
        );
        let seen = b.poll();
        assert_eq!(seen.len(), 1, "only the same key, never its own file");
        assert_eq!(seen[0].0, "registry:a#0");
        assert!(b.poll().is_empty(), "incremental");
        // a partial line is not taken before it is complete
        use std::io::Write as _;
        let mut f = std::fs::OpenOptions::new()
            .append(true)
            .open(reg.join("a.rules.jsonl"))
            .unwrap();
        write!(
            f,
            "{{\"type\":\"rule\",\"piece\":\"a\",\"key\":\"k\",\"index\":1,\"fixed\":[2,null]"
        )
        .unwrap();
        f.flush().unwrap();
        assert!(b.poll().is_empty());
        writeln!(f, ",\"equations\":[],\"exceptions\":[]}}").unwrap();
        f.flush().unwrap();
        assert_eq!(b.poll().len(), 1);
        // two pieces of the sunset chart through the registry: the second sees the first's rules and both close
        let fam_dir = tmp("registry-node");
        let dir = fam_dir.display().to_string();
        let refused = NodeOptions {
            registry: Some((dir.clone(), "p0".into())),
            ..quiet()
        };
        assert!(
            generate(
                SUNSET,
                "sunset.sym",
                &fam_dir.join("p0"),
                &refused,
                &mut |_| {}
            )
            .is_err(),
            "the registry is refused without the experimental switch (M5 batch 3)"
        );
        let p1 = NodeOptions {
            registry: Some((dir.clone(), "p1".into())),
            unsafe_shared_registry: true,
            ..quiet()
        };
        let r1 = generate(SUNSET, "sunset.sym", &fam_dir.join("p1"), &p1, &mut |_| {}).unwrap();
        assert!(r1.complete);
        let p2 = NodeOptions {
            registry: Some((dir, "p2".into())),
            unsafe_shared_registry: true,
            ..quiet()
        };
        let r2 = generate(SUNSET, "sunset.sym", &fam_dir.join("p2"), &p2, &mut |_| {}).unwrap();
        assert!(r2.complete);
        assert!(
            r2.registry_covered > 0 && r2.rules < r1.rules,
            "the second piece leaves the first one's cases to it: {r2:?}"
        );
        let _ = std::fs::remove_dir_all(&reg);
        let _ = std::fs::remove_dir_all(&fam_dir);
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
