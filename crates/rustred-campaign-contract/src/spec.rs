//! The job spec `RUN/job.json` (child-job.md 3.1.2), job kinds (1.2), budgets and work counters (3.3), output
//! options (ruling R-f).
//!
//! Unknown-field policy (fixed by package contract): the job spec and every object inside it reject unknown fields
//! (`deny_unknown_fields`, exit 40): the spec is written by the supervisor of the same contract version, and a
//! misspelt budget must not silently disable a limit. OUTCOME, RESUME and EVENTS readers tolerate unknown fields.

use crate::exit::ExitCode;
use crate::fake::FakeScript;
use crate::resume::ResumeRef;
use crate::{CONTRACT, is_blake3_hex};
use serde::{Deserialize, Serialize};
use std::fmt;
use std::path::{Path, PathBuf};

/// Job kinds (`kind`), table 1.2.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum JobKind {
    Generate,
    GenerateAuto,
    Descent,
    Replay,
    Witness,
    Laporta,
    ProofSpan,
    ProofExact,
    ProofStream,
    TableDump,
    Fake,
}

/// Compliance level (1.1).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Level {
    /// Atomic short job: spec, outputs published at the end, exit table, OUTCOME.
    L1,
    /// Full contract: safe points, heartbeat, RESUME, work budgets.
    L2,
    /// Legacy class (ruling R-d): `table_dump` for eps families, run under a wall cap, accepted only with a sealed
    /// table, no resume.
    Legacy,
}

impl JobKind {
    pub const ALL: [JobKind; 11] = [
        JobKind::Generate,
        JobKind::GenerateAuto,
        JobKind::Descent,
        JobKind::Replay,
        JobKind::Witness,
        JobKind::Laporta,
        JobKind::ProofSpan,
        JobKind::ProofExact,
        JobKind::ProofStream,
        JobKind::TableDump,
        JobKind::Fake,
    ];

    pub const fn level(self) -> Level {
        match self {
            JobKind::Generate | JobKind::Witness | JobKind::Fake => Level::L2,
            JobKind::TableDump => Level::Legacy,
            _ => Level::L1,
        }
    }

    /// The program that runs this kind.
    pub const fn program(self) -> &'static str {
        match self {
            JobKind::Generate
            | JobKind::GenerateAuto
            | JobKind::Descent
            | JobKind::Replay
            | JobKind::Witness
            | JobKind::Laporta => "rr_node",
            JobKind::ProofSpan | JobKind::ProofExact | JobKind::ProofStream => "rr_proof",
            JobKind::TableDump => "table_dump",
            JobKind::Fake => "rr_fake_child",
        }
    }

    /// Kinds whose product is a rule table `out/F.table.jsonl` (the decision table's "S" applies to them; for the
    /// checker kinds S is vacuously true).
    pub const fn produces_table(self) -> bool {
        matches!(
            self,
            JobKind::Generate | JobKind::GenerateAuto | JobKind::TableDump | JobKind::Fake
        )
    }

    /// The sealing rule of the kind's result files (audit A M4): every listed stream must be present in OUTCOME at its
    /// layout path and verify (end record + size + digest + records + canonical digest). `None` = the contract has
    /// no sealing rule for this kind yet: a "success" of it is classified `unverifiable_kind` (row 26), never
    /// accepted. `generate-auto` is legacy and not a campaign kind (7, Q7).
    pub const fn sealed_outputs(self) -> Option<&'static [OutputRule]> {
        match self {
            JobKind::Generate => Some(&GENERATE_OUTPUTS),
            JobKind::TableDump | JobKind::Fake => Some(&TABLE_OUTPUTS),
            JobKind::Witness => Some(&WITNESS_OUTPUTS),
            JobKind::Descent
            | JobKind::Replay
            | JobKind::Laporta
            | JobKind::ProofSpan
            | JobKind::ProofExact
            | JobKind::ProofStream => Some(&RESULT_OUTPUTS),
            JobKind::GenerateAuto => None,
        }
    }

    /// The flags `args` may carry for this kind (audit A M5): a closed ALLOW-list, each with its value shape.
    pub const fn allowed_args(self) -> &'static [ArgRule] {
        match self {
            JobKind::Generate | JobKind::GenerateAuto => &GENERATE_ARGS,
            JobKind::Descent => &DESCENT_ARGS,
            JobKind::Replay => &REPLAY_ARGS,
            JobKind::Witness => &WITNESS_ARGS,
            JobKind::Laporta => &LAPORTA_ARGS,
            JobKind::ProofSpan | JobKind::ProofExact | JobKind::ProofStream => &PROOF_ARGS,
            JobKind::TableDump => &TABLE_DUMP_ARGS,
            JobKind::Fake => &[],
        }
    }

    /// Checker kinds (exit 11 / 12 possible).
    pub const fn is_checker(self) -> bool {
        matches!(
            self,
            JobKind::Descent
                | JobKind::Replay
                | JobKind::Laporta
                | JobKind::ProofSpan
                | JobKind::ProofExact
                | JobKind::ProofStream
        )
    }
}

/// One sealed output stream of a kind: its role (`table`, or `outputs.extra[].role`), its file in `out/`, and the
/// types its end record may have.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OutputRule {
    pub role: &'static str,
    pub file: &'static str,
    pub end_types: &'static [&'static str],
    /// (lane eng2) an optional output is verified when OUTCOME lists it and may be absent otherwise.
    pub optional: bool,
}

/// End record types of the L1 result files (published once, at the end).
pub const RESULT_END_TYPES: [&str; 1] = ["end"];

const TABLE_OUTPUTS: [OutputRule; 1] = [OutputRule {
    role: "table",
    file: crate::layout::TABLE,
    end_types: &crate::seal::TABLE_END_TYPES,
    optional: false,
}];
/// `generate` (lane eng2): the table and the sealed open list `out/F.subcases.jsonl` (role `subcases`).
const GENERATE_OUTPUTS: [OutputRule; 2] = [
    TABLE_OUTPUTS[0],
    OutputRule {
        role: "subcases",
        file: crate::layout::SUBCASES,
        end_types: &RESULT_END_TYPES,
        optional: true,
    },
];
const WITNESS_OUTPUTS: [OutputRule; 2] = [
    OutputRule {
        role: "witness",
        file: crate::layout::WITNESS,
        end_types: &crate::seal::TABLE_END_TYPES,
        optional: false,
    },
    OutputRule {
        role: "loci",
        file: crate::layout::LOCI,
        end_types: &crate::seal::TABLE_END_TYPES,
        optional: false,
    },
];
const RESULT_OUTPUTS: [OutputRule; 1] = [OutputRule {
    role: "results",
    file: crate::layout::RESULTS,
    end_types: &RESULT_END_TYPES,
    optional: false,
}];

/// The shape of a flag's value.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ArgShape {
    /// No value.
    Flag,
    /// `[0-9]+` (at most 20 digits).
    UInt,
    /// `[0-9]+(.[0-9]+)?`.
    Decimal,
    /// `[A-Za-z0-9._-]+`, not starting with `-`, at most 128 characters.
    Name,
    /// A `Name` or a JSON object (orders).
    JsonOrName,
    /// `[0-9]+` (a chart / mask written as digits).
    Mask,
    /// `[A-Za-z0-9_.:;,+=*()-]+`, not starting with `--`, at most 4096 characters (no `/`: never a path).
    Spec,
    /// `[0-9,-]+` (rule ranges).
    Range,
    /// One of the listed words.
    Choice(&'static [&'static str]),
}

impl ArgShape {
    /// Whether `v` has this shape.
    pub fn accepts(self, v: &str) -> bool {
        let all = |f: fn(u8) -> bool| !v.is_empty() && v.bytes().all(f);
        match self {
            ArgShape::Flag => false,
            ArgShape::UInt => v.len() <= 20 && all(|b| b.is_ascii_digit()),
            ArgShape::Decimal => {
                let (i, f) = v.split_once('.').unwrap_or((v, "1"));
                !i.is_empty()
                    && !f.is_empty()
                    && i.bytes().all(|b| b.is_ascii_digit())
                    && f.bytes().all(|b| b.is_ascii_digit())
            }
            ArgShape::Name => {
                v.len() <= 128
                    && !v.starts_with('-')
                    && all(|b| b.is_ascii_alphanumeric() || matches!(b, b'.' | b'_' | b'-'))
            }
            ArgShape::JsonOrName => {
                ArgShape::Name.accepts(v)
                    || serde_json::from_str::<serde_json::Value>(v).is_ok_and(|x| x.is_object())
            }
            ArgShape::Mask => all(|b| b.is_ascii_digit()),
            ArgShape::Spec => {
                v.len() <= 4096
                    && !v.starts_with("--")
                    && all(|b| {
                        b.is_ascii_alphanumeric()
                            || matches!(
                                b,
                                b'_' | b'.'
                                    | b':'
                                    | b';'
                                    | b','
                                    | b'+'
                                    | b'='
                                    | b'*'
                                    | b'('
                                    | b')'
                                    | b'-'
                            )
                    })
            }
            ArgShape::Range => all(|b| b.is_ascii_digit() || b == b',' || b == b'-'),
            ArgShape::Choice(c) => c.contains(&v),
        }
    }
}

/// One allowed flag: name, value shape, whether it may repeat, and (for choices) the values allowed under
/// `determinism: strict` (`None` = every value).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ArgRule {
    pub flag: &'static str,
    pub shape: ArgShape,
    pub repeat: bool,
    pub strict_values: Option<&'static [&'static str]>,
}

const fn a(flag: &'static str, shape: ArgShape) -> ArgRule {
    ArgRule {
        flag,
        shape,
        repeat: false,
        strict_values: None,
    }
}

/// `--on-heavy`: `auto` decides by a measured time (3.3.4), so under strict only `refuse` / `proceed`.
pub const ON_HEAVY: ArgRule = ArgRule {
    flag: "--on-heavy",
    shape: ArgShape::Choice(&["auto", "refuse", "sampled", "proceed"]),
    repeat: false,
    strict_values: Some(&["refuse", "proceed"]),
};

const GENERATE_ARGS: [ArgRule; 37] = [
    a("--order", ArgShape::JsonOrName),
    // lane eng2: return the sub-cases instead of descending into them (node-loop.md 5.2)
    a("--descend", ArgShape::Choice(&["open"])),
    a("--chart", ArgShape::Mask),
    ON_HEAVY,
    a("--symmetry", ArgShape::Flag),
    a("--symmetry-rows", ArgShape::Flag),
    a("--symmetry-symbolic", ArgShape::Flag),
    a("--slim-rules", ArgShape::Flag),
    a("--open-on-refusal", ArgShape::Flag),
    a("--no-content-fallback", ArgShape::Flag),
    a("--no-empty-domain-check", ArgShape::Flag),
    a("--no-gsplit", ArgShape::Flag),
    a("--no-refused-bank", ArgShape::Flag),
    a("--no-symbolic-first-rung", ArgShape::Flag),
    a("--no-screen", ArgShape::Flag),
    a("--seed-pose-rung", ArgShape::Flag),
    a("--strict-numeric-search", ArgShape::Flag),
    a("--numeric-depth", ArgShape::UInt),
    a("--numeric-depth-max", ArgShape::UInt),
    a("--native-try", ArgShape::UInt),
    a("--admit-try", ArgShape::UInt),
    a("--max-rule-monomials", ArgShape::UInt),
    a("--fallback-rule-monomials", ArgShape::UInt),
    a("--fallback-after", ArgShape::UInt),
    a("--max-box", ArgShape::UInt),
    a("--max-depth", ArgShape::UInt),
    a("--min-target-block", ArgShape::UInt),
    a("--union-cover-depth", ArgShape::UInt),
    a("--seed-face-floor", ArgShape::UInt),
    a("--seed-pose-min-rows", ArgShape::UInt),
    a("--sampled-from-rows", ArgShape::UInt),
    a("--seed-pose-keep", ArgShape::Decimal),
    a("--seed-centre-offset", ArgShape::Spec),
    a("--seed-schedule", ArgShape::Spec),
    a("--case-seed-schedule", ArgShape::Spec),
    a("--case-seed-rung", ArgShape::Spec),
    a("--backend", ArgShape::Name),
];
const DESCENT_ARGS: [ArgRule; 1] = [a("--order", ArgShape::JsonOrName)];
const REPLAY_ARGS: [ArgRule; 9] = [
    a("--chart", ArgShape::Mask),
    a("--dimensions", ArgShape::Spec),
    a("--sample-attempts", ArgShape::UInt),
    a("--extra-points", ArgShape::UInt),
    a("--loci", ArgShape::UInt),
    a("--box", ArgShape::UInt),
    a("--box-max", ArgShape::UInt),
    a("--fixed-d", ArgShape::Flag),
    a("--single-prime", ArgShape::Flag),
];
const WITNESS_ARGS: [ArgRule; 1] = [a("--rules", ArgShape::Range)];
const LAPORTA_ARGS: [ArgRule; 6] = [
    a("--mask", ArgShape::Mask),
    a("--target", ArgShape::Spec),
    a("--basis", ArgShape::Spec),
    a("--box", ArgShape::Spec),
    a("--dims", ArgShape::Spec),
    a("--primes", ArgShape::UInt),
];
const PROOF_ARGS: [ArgRule; 11] = [
    a("--rule", ArgShape::UInt),
    a("--offset", ArgShape::UInt),
    ArgRule {
        flag: "--key",
        shape: ArgShape::Spec,
        repeat: true,
        strict_values: None,
    },
    a("--drop-zero-sectors", ArgShape::Flag),
    a("--max-primes", ArgShape::UInt),
    a("--max-points", ArgShape::UInt),
    a("--chart", ArgShape::Spec),
    a("--kmod", ArgShape::UInt),
    a("--kexact", ArgShape::UInt),
    a("--rows", ArgShape::Choice(&["engine", "cert"])),
    a("--ignore-exceptions", ArgShape::Flag),
];
const TABLE_DUMP_ARGS: [ArgRule; 10] = [
    a("--mask", ArgShape::Mask),
    a("--order", ArgShape::JsonOrName),
    a("--workers", ArgShape::UInt),
    a("--charts", ArgShape::Mask),
    a("--defer-unsupported", ArgShape::Flag),
    a("--case-gsplit", ArgShape::Flag),
    a("--exact-backend", ArgShape::Name),
    a("--spill-after", ArgShape::UInt),
    a("--numeric-endpoints", ArgShape::Spec),
    a("--source-order", ArgShape::Name),
];

/// Work counters (3.3.1).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WorkCounter {
    Cases,
    Seeds,
    Rows,
    ProbeEvals,
    ReducerRows,
    TraceRows,
    LiftMonomials,
    RhsTerms,
    NumericPoints,
}

impl WorkCounter {
    pub const ALL: [WorkCounter; 9] = [
        WorkCounter::Cases,
        WorkCounter::Seeds,
        WorkCounter::Rows,
        WorkCounter::ProbeEvals,
        WorkCounter::ReducerRows,
        WorkCounter::TraceRows,
        WorkCounter::LiftMonomials,
        WorkCounter::RhsTerms,
        WorkCounter::NumericPoints,
    ];
}

/// Scope of a work limit.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Scope {
    Case,
    Job,
}

/// Counter values (`OUTCOME.job.counters`, heartbeat `counters`, RESUME `counters_at_commit`). `trace_rows_max` is
/// the largest trace of any case; the others are sums.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Counters {
    #[serde(default)]
    pub cases: u64,
    #[serde(default)]
    pub seeds: u64,
    #[serde(default)]
    pub rows: u64,
    #[serde(default)]
    pub probe_evals: u64,
    #[serde(default)]
    pub reducer_rows: u64,
    #[serde(default)]
    pub trace_rows_max: u64,
    #[serde(default)]
    pub lift_monomials: u64,
    #[serde(default)]
    pub rhs_terms: u64,
    #[serde(default)]
    pub numeric_points: u64,
    #[serde(default)]
    pub rules_committed: u64,
}

impl Counters {
    /// The value of one counter (`trace_rows` reads `trace_rows_max`).
    pub fn get(&self, c: WorkCounter) -> u64 {
        match c {
            WorkCounter::Cases => self.cases,
            WorkCounter::Seeds => self.seeds,
            WorkCounter::Rows => self.rows,
            WorkCounter::ProbeEvals => self.probe_evals,
            WorkCounter::ReducerRows => self.reducer_rows,
            WorkCounter::TraceRows => self.trace_rows_max,
            WorkCounter::LiftMonomials => self.lift_monomials,
            WorkCounter::RhsTerms => self.rhs_terms,
            WorkCounter::NumericPoints => self.numeric_points,
        }
    }

    /// Add `n` to one counter (`trace_rows` raises the maximum).
    pub fn add(&mut self, c: WorkCounter, n: u64) {
        match c {
            WorkCounter::Cases => self.cases += n,
            WorkCounter::Seeds => self.seeds += n,
            WorkCounter::Rows => self.rows += n,
            WorkCounter::ProbeEvals => self.probe_evals += n,
            WorkCounter::ReducerRows => self.reducer_rows += n,
            WorkCounter::TraceRows => self.trace_rows_max = self.trace_rows_max.max(n),
            WorkCounter::LiftMonomials => self.lift_monomials += n,
            WorkCounter::RhsTerms => self.rhs_terms += n,
            WorkCounter::NumericPoints => self.numeric_points += n,
        }
    }
}

/// Work limits (`budgets.work`). `None` = no limit. A zero limit is a usage error.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WorkBudgets {
    #[serde(default)]
    pub cases: Option<u64>,
    #[serde(default)]
    pub seeds_per_case: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub seeds_total: Option<u64>,
    #[serde(default)]
    pub rows_per_case: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rows_total: Option<u64>,
    #[serde(default)]
    pub probe_evals_per_case: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reducer_rows_per_case: Option<u64>,
    #[serde(default)]
    pub trace_rows_max: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub lift_monomials_total: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rhs_terms_total: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub numeric_points_total: Option<u64>,
}

/// A budget that fired (`OUTCOME.job.stop.detail`, `budgets.hit`, event `budget_hit`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BudgetHit {
    pub counter: WorkCounter,
    pub scope: Scope,
    pub limit: u64,
    pub value: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub case: Option<serde_json::Value>,
}

impl WorkBudgets {
    /// Every set limit as (counter, scope, limit).
    pub fn limits(&self) -> Vec<(WorkCounter, Scope, u64)> {
        use Scope::*;
        use WorkCounter::*;
        let all = [
            (Cases, Job, self.cases),
            (Seeds, Case, self.seeds_per_case),
            (Seeds, Job, self.seeds_total),
            (Rows, Case, self.rows_per_case),
            (Rows, Job, self.rows_total),
            (ProbeEvals, Case, self.probe_evals_per_case),
            (ReducerRows, Case, self.reducer_rows_per_case),
            (TraceRows, Case, self.trace_rows_max),
            (LiftMonomials, Job, self.lift_monomials_total),
            (RhsTerms, Job, self.rhs_terms_total),
            (NumericPoints, Job, self.numeric_points_total),
        ];
        all.into_iter()
            .filter_map(|(c, s, l)| l.map(|l| (c, s, l)))
            .collect()
    }

    /// The first WORK limit reached (value >= limit) by the counters of the current case or of the job, in the order
    /// of [`WorkBudgets::limits`]; a hit ends the job with stop `budget_work` (exit 20). The `cases` limit is NOT part
    /// of it: it is the old `--max-cases`, checked before a case starts ([`WorkBudgets::cases_exhausted`]), and ends
    /// the job with stop `max_cases` (exit 10).
    pub fn first_hit(&self, case: &Counters, job: &Counters) -> Option<BudgetHit> {
        self.limits()
            .into_iter()
            .filter(|&(c, _, _)| c != WorkCounter::Cases)
            .find_map(|(c, s, l)| {
                let v = match s {
                    Scope::Case => case.get(c),
                    Scope::Job => job.get(c),
                };
                (v >= l).then_some(BudgetHit {
                    counter: c,
                    scope: s,
                    limit: l,
                    value: v,
                    case: None,
                })
            })
    }
}

impl WorkBudgets {
    /// Whether the `cases` limit (old `--max-cases`) forbids starting another case: the job then ends with stop
    /// `max_cases`.
    pub fn cases_exhausted(&self, job: &Counters) -> bool {
        self.cases.is_some_and(|l| job.cases >= l)
    }
}

/// Wall budgets in seconds (`budgets.wall`).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WallBudgets {
    #[serde(default)]
    pub case_s: Option<u64>,
    #[serde(default)]
    pub job_s: Option<u64>,
}

/// `budgets`.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Budgets {
    #[serde(default)]
    pub work: WorkBudgets,
    #[serde(default)]
    pub wall: WallBudgets,
    #[serde(default)]
    pub mem_soft_bytes: Option<u64>,
}

/// `on_case_budget` (3.3.2).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OnCaseBudget {
    #[default]
    Stop,
    Open,
}

/// `determinism` (3.3.4). `strict` = no clock decides anything except the wall budgets (ruling R-g: a goal).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Determinism {
    #[default]
    Recorded,
    Strict,
}

/// `seed` (3.2.2).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SeedSpec {
    #[serde(default)]
    pub sample_seed: u64,
}

/// `output` (ruling R-f). `elide_witness_loci = false` (the default, and the campaign's setting) FORBIDS elision:
/// the child writes every witness locus in full whatever its length, even when that exceeds a size limit that would
/// otherwise apply (today 4,000 characters per locus); records may then be very long lines.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct OutputOptions {
    #[serde(default)]
    pub elide_witness_loci: bool,
}

/// `program`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProgramRef {
    pub name: String,
    pub blake3: String,
    #[serde(default)]
    pub features_required: Vec<String>,
}

/// One entry of `inputs`. A relative `path` is relative to `run_dir`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct InputRef {
    pub role: String,
    pub path: String,
    pub blake3: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub upto: Option<u64>,
}

fn d_heartbeat() -> u32 {
    10
}
fn d_grace() -> u32 {
    20
}
fn d_resume_every() -> u32 {
    30
}

/// The job spec.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct JobSpec {
    pub schema: String,
    pub job_id: String,
    pub attempt: u32,
    pub kind: JobKind,
    pub run_dir: String,
    pub program: ProgramRef,
    #[serde(default)]
    pub inputs: Vec<InputRef>,
    #[serde(default)]
    pub args: Vec<String>,
    #[serde(default)]
    pub seed: SeedSpec,
    #[serde(default)]
    pub budgets: Budgets,
    #[serde(default)]
    pub on_case_budget: OnCaseBudget,
    #[serde(default)]
    pub determinism: Determinism,
    #[serde(default)]
    pub output: OutputOptions,
    #[serde(default = "d_heartbeat")]
    pub heartbeat_s: u32,
    #[serde(default = "d_grace")]
    pub stop_grace_s: u32,
    /// 0 = rewrite RESUME.json at every commit point.
    #[serde(default = "d_resume_every")]
    pub resume_every_s: u32,
    #[serde(default)]
    pub resume: Option<ResumeRef>,
    #[serde(default)]
    pub env_allow: Vec<String>,
    #[serde(default)]
    pub fake: Option<FakeScript>,
}

/// Flags carried by typed fields; present in `args` => exit 40 (3.1.2).
pub const TYPED_FIELD_FLAGS: [&str; 8] = [
    "--family",
    "--out",
    "--table",
    "--initial",
    "--initial-presented",
    "--cover",
    "--case-budget",
    "--max-cases",
];

/// Flags that write outside the run directory; present in `args` => exit 42 (3.1.3).
pub const REFUSED_FLAGS: [&str; 4] = [
    "--registry",
    "--pool",
    "--unsafe-shared-registry",
    "--lease-wait",
];

/// Environment variables refused in job mode unless listed in `env_allow` (3.2.2; 2.5: every variable that changes
/// results, audit A M5).
pub const GATED_ENV: [&str; 6] = [
    "RUSTRED_PROBE_POINT",
    "RUSTRED_TRACE_DUMP",
    "RUSTRED_TRACE_DUMP_MIN_ROWS",
    "RUSTRED_TRACE_DUMP_EXIT",
    "RR_REPLAY_RECENTER_SEARCH",
    "RR_REPLAY_RECENTER_LIMIT",
];

/// A spec that cannot be run. [`SpecError::exit_code`] says 40 or 42.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SpecError {
    /// Not JSON, wrong types, missing or unknown fields.
    Parse(String),
    Schema(String),
    BadField {
        field: &'static str,
        why: String,
    },
    /// A flag carried by a typed field appears in `args` (40).
    TypedFlagInArgs(String),
    /// A flag writing outside RUN appears in `args` (42).
    RefusedFlag(String),
    /// `run_dir` is not the directory of the spec file (42).
    RunDirMismatch {
        run_dir: String,
        spec_dir: String,
    },
    /// `kind` and `fake` disagree.
    FakeMismatch(String),
    /// An `args` token that is not on the kind's allow-list, a `--flag=value` spelling, a positional, a repeated
    /// flag or a value of the wrong shape (40).
    ArgNotAllowed(String),
}

impl SpecError {
    pub fn exit_code(&self) -> ExitCode {
        match self {
            SpecError::RefusedFlag(_) | SpecError::RunDirMismatch { .. } => ExitCode::Refused,
            _ => ExitCode::Usage,
        }
    }
}

impl fmt::Display for SpecError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SpecError::Parse(m) => write!(f, "job spec does not parse: {m}"),
            SpecError::Schema(s) => write!(f, "job spec schema {s:?} is not {CONTRACT:?}"),
            SpecError::BadField { field, why } => write!(f, "job spec field {field}: {why}"),
            SpecError::TypedFlagInArgs(a) => {
                write!(f, "args carries {a}, which is a typed field of the spec")
            }
            SpecError::RefusedFlag(a) => write!(
                f,
                "args carries {a}, which writes outside the run directory"
            ),
            SpecError::RunDirMismatch { run_dir, spec_dir } => {
                write!(
                    f,
                    "run_dir {run_dir} is not the directory of the spec file ({spec_dir})"
                )
            }
            SpecError::FakeMismatch(m) => write!(f, "job spec kind/fake: {m}"),
            SpecError::ArgNotAllowed(m) => write!(f, "args: {m}"),
        }
    }
}
impl std::error::Error for SpecError {}

fn bad(field: &'static str, why: impl Into<String>) -> SpecError {
    SpecError::BadField {
        field,
        why: why.into(),
    }
}

fn flag_name(a: &str) -> &str {
    a.split_once('=').map_or(a, |(f, _)| f)
}

fn valid_env_name(s: &str) -> bool {
    !s.is_empty()
        && !s.as_bytes()[0].is_ascii_digit()
        && s.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_')
}

impl JobSpec {
    /// Parse and validate a spec file's bytes (no file-system checks; see [`JobSpec::check_location`]).
    pub fn from_json(bytes: &[u8]) -> Result<JobSpec, SpecError> {
        let s: JobSpec =
            serde_json::from_slice(bytes).map_err(|e| SpecError::Parse(e.to_string()))?;
        s.validate()?;
        Ok(s)
    }

    /// Compact JSON + "\n" (what the supervisor writes to `job.json`).
    pub fn to_json_line(&self) -> Vec<u8> {
        let mut v = serde_json::to_vec(self).expect("JobSpec serialises");
        v.push(b'\n');
        v
    }

    /// Structural validation (everything that does not need the file system). Order of checks is fixed so that the
    /// error for a given spec is stable.
    pub fn validate(&self) -> Result<(), SpecError> {
        if self.schema != CONTRACT {
            return Err(SpecError::Schema(self.schema.clone()));
        }
        if self.job_id.is_empty()
            || self.job_id.len() > 128
            || !self
                .job_id
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_' || b == b'.')
            || self.job_id.starts_with('.')
        {
            return Err(bad(
                "job_id",
                "must be 1-128 characters of [A-Za-z0-9._-], not starting with '.'",
            ));
        }
        if self.attempt == 0 {
            return Err(bad("attempt", "attempts count from 1"));
        }
        if !Path::new(&self.run_dir).is_absolute() {
            return Err(bad("run_dir", "must be an absolute path"));
        }
        if self.program.name.is_empty() {
            return Err(bad("program.name", "empty"));
        }
        if !is_blake3_hex(&self.program.blake3) {
            return Err(bad(
                "program.blake3",
                "not a 64-character lower-case hex BLAKE3 digest",
            ));
        }
        for i in &self.inputs {
            if i.role.is_empty() {
                return Err(bad("inputs.role", "empty"));
            }
            if i.path.is_empty() {
                return Err(bad("inputs.path", "empty"));
            }
            if !is_blake3_hex(&i.blake3) {
                return Err(bad(
                    "inputs.blake3",
                    format!("input {:?}: not a BLAKE3 hex digest", i.role),
                ));
            }
        }
        self.check_args()?;
        let w = &self.budgets.work;
        if w.limits().iter().any(|&(_, _, l)| l == 0) {
            return Err(bad(
                "budgets.work",
                "a zero limit is not allowed (omit the limit instead)",
            ));
        }
        if self.budgets.wall.case_s == Some(0) || self.budgets.wall.job_s == Some(0) {
            return Err(bad("budgets.wall", "a zero limit is not allowed"));
        }
        if self.budgets.mem_soft_bytes == Some(0) {
            return Err(bad("budgets.mem_soft_bytes", "a zero limit is not allowed"));
        }
        if self.heartbeat_s == 0 {
            return Err(bad("heartbeat_s", "minimum 1"));
        }
        if self.stop_grace_s == 0 {
            return Err(bad("stop_grace_s", "minimum 1"));
        }
        if let Some(r) = &self.resume {
            r.validate().map_err(|why| bad("resume", why))?;
        }
        for e in &self.env_allow {
            if !valid_env_name(e) {
                return Err(bad(
                    "env_allow",
                    format!("{e:?} is not an environment variable name"),
                ));
            }
        }
        match (self.kind, &self.fake) {
            (JobKind::Fake, None) => {
                return Err(SpecError::FakeMismatch(
                    "kind fake needs a fake script".into(),
                ));
            }
            (JobKind::Fake, Some(f)) => {
                if !self.args.is_empty() {
                    return Err(SpecError::FakeMismatch("kind fake takes no args".into()));
                }
                f.validate().map_err(SpecError::FakeMismatch)?;
            }
            (_, Some(_)) => {
                return Err(SpecError::FakeMismatch(
                    "a fake script on a non-fake kind".into(),
                ));
            }
            _ => {}
        }
        Ok(())
    }

    /// `run_dir` must be the directory of the spec file (3.1.3, exit 42). Both paths are canonicalised when they
    /// exist, so symlinked parents compare equal.
    pub fn check_location(&self, spec_path: &Path) -> Result<(), SpecError> {
        let dir = spec_path.parent().unwrap_or(Path::new("."));
        let canon = |p: &Path| std::fs::canonicalize(p).unwrap_or_else(|_| p.to_path_buf());
        let (a, b) = (canon(Path::new(&self.run_dir)), canon(dir));
        if a != b {
            return Err(SpecError::RunDirMismatch {
                run_dir: self.run_dir.clone(),
                spec_dir: b.display().to_string(),
            });
        }
        Ok(())
    }

    /// The absolute path of an input (relative paths are relative to `run_dir`).
    pub fn input_path(&self, i: &InputRef) -> PathBuf {
        let p = Path::new(&i.path);
        if p.is_absolute() {
            p.to_path_buf()
        } else {
            Path::new(&self.run_dir).join(p)
        }
    }

    /// The allow-list check of `args` (audit A M5, B F8): two-token `--flag value` form only (any `--flag=value` is
    /// refused, so no spelling escapes the value check); every flag on the kind's list, with a value of its shape;
    /// no positional; no repeat unless the flag repeats; under `determinism: strict` only the strict values of a
    /// choice, and a `generate` job must name `--on-heavy` explicitly (its default `auto` is a clock decision).
    pub fn check_args(&self) -> Result<(), SpecError> {
        let rules = self.kind.allowed_args();
        let mut seen: Vec<&str> = Vec::new();
        let mut on_heavy = false;
        let mut k = 0;
        while k < self.args.len() {
            let tok = self.args[k].as_str();
            let f = flag_name(tok);
            if REFUSED_FLAGS.contains(&f) {
                return Err(SpecError::RefusedFlag(f.to_string()));
            }
            if TYPED_FIELD_FLAGS.contains(&f) {
                return Err(SpecError::TypedFlagInArgs(f.to_string()));
            }
            if !tok.starts_with("--") {
                return Err(SpecError::ArgNotAllowed(format!("positional {tok:?}")));
            }
            if tok.contains('=') {
                return Err(SpecError::ArgNotAllowed(format!(
                    "{tok:?}: write `{f} VALUE` as two tokens"
                )));
            }
            let Some(r) = rules.iter().find(|r| r.flag == tok) else {
                return Err(SpecError::ArgNotAllowed(format!(
                    "{tok} is not allowed for kind {:?}",
                    self.kind
                )));
            };
            if seen.contains(&r.flag) && !r.repeat {
                return Err(SpecError::ArgNotAllowed(format!("{tok} repeated")));
            }
            seen.push(r.flag);
            if r.flag == ON_HEAVY.flag {
                on_heavy = true;
            }
            if r.shape == ArgShape::Flag {
                k += 1;
                continue;
            }
            let Some(v) = self.args.get(k + 1) else {
                return Err(SpecError::ArgNotAllowed(format!("{tok} needs a value")));
            };
            if !r.shape.accepts(v) {
                return Err(SpecError::ArgNotAllowed(format!(
                    "{tok}: value {v:?} is not of shape {:?}",
                    r.shape
                )));
            }
            if self.determinism == Determinism::Strict
                && let Some(ok) = r.strict_values
                && !ok.contains(&v.as_str())
            {
                return Err(bad(
                    "args",
                    format!("{tok} {v} is a clock decision, refused under determinism strict"),
                ));
            }
            k += 2;
        }
        if self.determinism == Determinism::Strict
            && matches!(self.kind, JobKind::Generate | JobKind::GenerateAuto)
            && !on_heavy
        {
            return Err(bad(
                "args",
                "determinism strict needs an explicit --on-heavy refuse|proceed (the default auto is a clock decision)",
            ));
        }
        Ok(())
    }

    /// The identity of a JOB across attempts (audit A M7 / C1): the spec as JSON without `attempt`, `run_dir`,
    /// `resume`, and, for the fake kind, without the test-stand behaviour that does not change what is computed
    /// (`on_sigterm`, `slow_ms`, `start_delay_ms`, `faults`, `helpers`, `rss`, `time_scale` and the `ms` of every op).
    pub fn job_identity(&self) -> serde_json::Value {
        let mut v = serde_json::to_value(self).expect("JobSpec serialises");
        if let Some(m) = v.as_object_mut() {
            for k in ["attempt", "run_dir", "resume"] {
                m.remove(k);
            }
            if let Some(f) = m.get_mut("fake").and_then(|f| f.as_object_mut()) {
                for k in [
                    "on_sigterm",
                    "slow_ms",
                    "start_delay_ms",
                    "faults",
                    "helpers",
                    "rss",
                    "time_scale",
                ] {
                    f.remove(k);
                }
                if let Some(ops) = f.get_mut("script").and_then(|s| s.as_array_mut()) {
                    for op in ops {
                        if let Some(o) = op.as_object_mut() {
                            o.remove("ms");
                        }
                    }
                }
            }
        }
        v
    }

    /// Whether `prev` (the spec of an earlier attempt) is the same job: same identity ([`JobSpec::job_identity`]).
    pub fn same_job(&self, prev: &JobSpec) -> bool {
        self.job_identity() == prev.job_identity()
    }

    /// Whether the campaign may use witness loci of this job's records (R-f: elided loci are unusable).
    pub fn elision_forbidden(&self) -> bool {
        !self.output.elide_witness_loci
    }
}

/// `spec_blake3`: the BLAKE3 of the `job.json` bytes as written (the ledger identity of the job attempt).
pub fn spec_blake3(spec_file_bytes: &[u8]) -> String {
    crate::blake3_hex(spec_file_bytes)
}
