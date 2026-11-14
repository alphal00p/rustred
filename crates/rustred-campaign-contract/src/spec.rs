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
    /// `rr_node solve-recipe` (coord-recipe-solve.md v1.1; lane adapters): ONE recipe at MANY keys, L1.
    SolveRecipe,
    /// `rr_proof instantiate` (coord-ec2-instantiate.md 3) run as a LEGACY child: no job mode in the child; the
    /// supervisor-side wrapper (interfaces/adapters.md 2.3) seals its output and writes OUTCOME (lane adapters).
    ProofInstantiate,
    /// `rr_proof --mode instance-span` (coord-ec2-instantiate.md 1, 4) at MANY keys of ONE rule record, run as a LEGACY
    /// child like `proof-instantiate`; the supervisor-side wrapper seals its answers (lane closeloop2; closeloop.md 13.1).
    ProofInstanceSpan,
    /// `rr_coef_facts facts` (lane closeloop3; interfaces/closeloop.md 16): EXACT coefficient facts of rule records from
    /// their own coefficient text (Symbolica), for the records `rr_proof instantiate` refuses (elided witness loci). A
    /// LEGACY child like `proof-instantiate` (same request form (A)); the supervisor-side wrapper seals its answers.
    CoefficientFacts,
    /// `rr_node job` kind `node-basis` (line 7, coord-pole-engine.md 4 / 8; child-job.md "Line 7"): the node-local
    /// elimination with the given terminals lowest, at fixed dimensions and primes, L1. A DIAGNOSTIC kind: its sealed
    /// records are never rule or relation evidence (accept mode `diagnostic_records`).
    NodeBasis,
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
    pub const ALL: [JobKind; 16] = [
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
        JobKind::SolveRecipe,
        JobKind::ProofInstantiate,
        JobKind::ProofInstanceSpan,
        JobKind::CoefficientFacts,
        JobKind::NodeBasis,
    ];

    pub const fn level(self) -> Level {
        match self {
            JobKind::Generate | JobKind::Witness | JobKind::Fake => Level::L2,
            JobKind::TableDump
            | JobKind::ProofInstantiate
            | JobKind::ProofInstanceSpan
            | JobKind::CoefficientFacts => Level::Legacy,
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
            | JobKind::Laporta
            | JobKind::SolveRecipe
            | JobKind::NodeBasis => "rr_node",
            JobKind::ProofSpan
            | JobKind::ProofExact
            | JobKind::ProofStream
            | JobKind::ProofInstantiate
            | JobKind::ProofInstanceSpan => "rr_proof",
            JobKind::TableDump => "table_dump",
            JobKind::Fake => "rr_fake_child",
            JobKind::CoefficientFacts => "rr_coef_facts",
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
            | JobKind::ProofStream
            | JobKind::SolveRecipe
            | JobKind::ProofInstantiate
            | JobKind::ProofInstanceSpan
            | JobKind::CoefficientFacts
            | JobKind::NodeBasis => Some(&RESULT_OUTPUTS),
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
            JobKind::SolveRecipe => &SOLVE_RECIPE_ARGS,
            JobKind::NodeBasis => &NODE_BASIS_ARGS,
            // the wrapper builds the legacy argv itself (`instantiate --requests in/requests.jsonl --out ...`); line 7:
            // it passes `--elided-loci refuse|facts` through (coord-ec2-instantiate.md 2d)
            JobKind::ProofInstantiate => &PROOF_INSTANTIATE_ARGS,
            JobKind::ProofInstanceSpan | JobKind::CoefficientFacts => &[],
        }
    }

    /// The input roles of the kind (lane adapters): `None` = no role rule for this kind (every role is passed on);
    /// `Some` = every role must be listed, each `required` role exactly once, each `optional` role at most once.
    pub const fn input_roles(self) -> Option<InputRoles> {
        match self {
            JobKind::SolveRecipe => Some(InputRoles {
                required: &["family", "recipe", "keys"],
                optional: &["expect"],
            }),
            // one job per rule table: the table's digest is part of the attempt's identity
            JobKind::ProofInstantiate => Some(InputRoles {
                required: &["requests", "table"],
                optional: &[],
            }),
            // lane closeloop3: the same request form (A) and one table per job
            JobKind::CoefficientFacts => Some(InputRoles {
                required: &["requests", "table"],
                optional: &[],
            }),
            // line 7: the presented family and the terminal keys (JSONL, put lowest)
            JobKind::NodeBasis => Some(InputRoles {
                required: &["family", "terminals"],
                optional: &[],
            }),
            // one rule record of one table at many keys; rows from the family (engine build) or a cert / ROWS file
            JobKind::ProofInstanceSpan => Some(InputRoles {
                required: &["request", "table", "keys"],
                optional: &["family", "cert", "symmetry_group"],
            }),
            _ => None,
        }
    }

    /// Kinds the fake child `rr_fake_child` can emulate under their own kind name (lane adapters).
    pub const fn fake_emulated(self) -> bool {
        matches!(
            self,
            JobKind::SolveRecipe
                | JobKind::ProofInstantiate
                | JobKind::ProofInstanceSpan
                | JobKind::CoefficientFacts
        )
    }

    /// The kinds run as LEGACY children whose output the supervisor-side wrapper seals (closeloop.md 13.1, 16): the
    /// rr_proof kinds and `coefficient-facts`.
    pub const fn legacy_wrapped(self) -> bool {
        matches!(
            self,
            JobKind::ProofInstantiate | JobKind::ProofInstanceSpan | JobKind::CoefficientFacts
        )
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

/// The input roles a kind accepts ([`JobKind::input_roles`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct InputRoles {
    pub required: &'static [&'static str],
    pub optional: &'static [&'static str],
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
    /// (line 7) An exact rational in the CANONICAL text ([`crate::arith::Rational::parse_canonical`]): `4`, `-3/2`;
    /// lowest terms, denominator > 1 when written; no float, exponent, `+` or leading zero.
    Rational,
    /// (line 7) A comma-separated list of pairwise distinct canonical rationals (`--dims`).
    RationalList,
    /// (line 7) A comma-separated list of pairwise distinct unsigned integers in canonical decimal, with an element rule.
    UIntList(UIntElems),
    /// (line 7, seed minimisation) A centre scan ([`crate::arith::parse_centre_scan`]): comma-separated entries, each
    /// `recipe`, `floor_of_free`, `each_free_slot` or an explicit offset vector `(a;b;..)` of canonical integers.
    CentreScan,
    /// (line 7, seed minimisation target) A case domain as compact JSON ([`crate::arith::parse_case_domain`]):
    /// `{"fixed": [..], "equations": [..], "bounds": {"min": [..], "max": [..]}}`, at least one member.
    CaseDomain,
}

/// The element rule of an [`ArgShape::UIntList`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UIntElems {
    /// Any `u64`.
    Any,
    /// Every element in [2^40, 2^62) and PRIME ([`crate::arith::check_line7_prime`]): `--probe-primes`, node-basis
    /// `--primes`.
    Line7Primes,
}

impl ArgShape {
    /// Whether `v` has this shape, with the reason when not.
    pub fn check(self, v: &str) -> Result<(), String> {
        use crate::arith;
        match self {
            ArgShape::Rational => arith::Rational::parse_canonical(v).map(|_| ()),
            ArgShape::RationalList => arith::parse_rational_list(v).map(|_| ()),
            ArgShape::UIntList(UIntElems::Any) => arith::parse_uint_list(v).map(|_| ()),
            ArgShape::UIntList(UIntElems::Line7Primes) => arith::parse_prime_list(v).map(|_| ()),
            ArgShape::CentreScan => arith::parse_centre_scan(v).map(|_| ()),
            ArgShape::CaseDomain => arith::parse_case_domain(v).map(|_| ()),
            other if other.accepts(v) => Ok(()),
            other => Err(format!("value {v:?} is not of shape {other:?}")),
        }
    }

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
            ArgShape::Rational
            | ArgShape::RationalList
            | ArgShape::UIntList(_)
            | ArgShape::CentreScan
            | ArgShape::CaseDomain => self.check(v).is_ok(),
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

const GENERATE_ARGS: [ArgRule; 43] = [
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
    // lane adapters (EC-1 v2 section 9 request): the lift pre-screen `MIN_ROWS,MAX_POINTS,MAX_DEG,MAX_RHS`
    a("--screen", ArgShape::Spec),
    a("--seed-pose-min-rows", ArgShape::UInt),
    a("--sampled-from-rows", ArgShape::UInt),
    a("--seed-pose-keep", ArgShape::Decimal),
    a("--seed-centre-offset", ArgShape::Spec),
    // lane engbase (engine line, coord-ec1-seed-centre.md v2): t266-tight seed layer cap `D,R[,B]` (needs --max-depth)
    a("--seed-layer-cap", ArgShape::Spec),
    a("--seed-schedule", ArgShape::Spec),
    a("--case-seed-schedule", ArgShape::Spec),
    a("--case-seed-rung", ArgShape::Spec),
    a("--backend", ArgShape::Name),
    // line 7, seed minimisation (coord-seed-minimise.md 6 / 7, replacing section 5's `--seed-centre floor_of_free`):
    // the deterministic centre scan; the minimisation mode; its WORK budget (a count of re-solves, never a clock)
    a("--centre-scan", ArgShape::CentreScan),
    a("--minimise-rules", ArgShape::Choice(&["support", "greedy"])),
    a("--minimise-max-solves", ArgShape::UInt),
    // coord-seed-minimise.md 7: the target domain a candidate's exception branches must not intersect
    a("--minimise-target", ArgShape::CaseDomain),
];
const DESCENT_ARGS: [ArgRule; 1] = [a("--order", ArgShape::JsonOrName)];
/// `solve-recipe` (lane adapters; coord-recipe-solve.md v1.1 job mode): the node order (required), worker threads
/// (results do not depend on them), the deterministic per-key trace-row limit (verdict BUDGET_WORK) and the per-key
/// wall budget in seconds (verdict BUDGET_WALL, time-decided). Files (family, recipe, keys, expect) are inputs.
pub const SOLVE_RECIPE_ARGS: [ArgRule; 7] = [
    a("--order", ArgShape::JsonOrName),
    a("--threads", ArgShape::UInt),
    a("--max-trace-rows", ArgShape::UInt),
    a("--key-budget-s", ArgShape::UInt),
    // line 7 (coord-pole-engine.md 8): probe mode (modular only, `probe` records, never relations) ...
    a("--probe-d", ArgShape::Rational),
    a("--probe-primes", ArgShape::UIntList(UIntElems::Line7Primes)),
    // ... and the m1 work cap of one key (`BUDGET_WORK` with counter `m1_work_rows`)
    a("--max-m1-rows", ArgShape::UInt),
];
/// `node-basis` (line 7, coord-pole-engine.md 8; contract owner's ruling: `--primes` is a LIST of primes under the
/// line-7 prime rule, `--dims` a list of rationals).
pub const NODE_BASIS_ARGS: [ArgRule; 5] = [
    a("--mask", ArgShape::Mask),
    a("--box", ArgShape::Spec),
    a("--dims", ArgShape::RationalList),
    a("--primes", ArgShape::UIntList(UIntElems::Line7Primes)),
    a("--symmetry", ArgShape::Choice(&["on", "off"])),
];
/// `proof-instantiate` (line 7, coord-ec2-instantiate.md 2d): `--elided-loci refuse` (the default when absent: the
/// R-f refusal) or `facts` (records with elided witness loci answer coefficient facts only, never regularity).
pub const PROOF_INSTANTIATE_ARGS: [ArgRule; 1] =
    [a("--elided-loci", ArgShape::Choice(&["refuse", "facts"]))];
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

/// The probe parameters of a solve-recipe job run with `--probe-d` ([`JobSpec::probe_mode`]).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProbeArgs {
    pub d0: crate::arith::Rational,
    /// In argument order (records list `per_prime` in this order).
    pub primes: Vec<u64>,
}

/// `proof-instantiate --elided-loci` (coord-ec2-instantiate.md 2d).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ElidedLoci {
    /// R-f fail closed: a record with elided witness loci is refused (the whole legacy job, exit 2 -> 41).
    #[default]
    Refuse,
    /// Records with elided witness loci answer exact COEFFICIENT facts only; never regularity evidence.
    Facts,
}

/// The fake child's program name: a spec of an emulated kind ([`JobKind::fake_emulated`]) may carry a fake script only
/// with this program name (lane adapters).
pub const FAKE_PROGRAM: &str = "rr_fake_child";

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
        crate::canon::check_document(bytes).map_err(|e| SpecError::Parse(e.to_string()))?;
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
        self.check_input_roles()?;
        self.check_kind_constraints()?;
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
            // lane adapters: the fake child emulates a kind (today solve-recipe) when the spec names it as the program;
            // a real engine refuses that program name at preflight step 6 (42), so a fake script never reaches it
            (k, Some(f)) if k.fake_emulated() && self.program.name == FAKE_PROGRAM => {
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

    /// The kind's input-role rule ([`JobKind::input_roles`], lane adapters; exit 40).
    pub fn check_input_roles(&self) -> Result<(), SpecError> {
        let Some(r) = self.kind.input_roles() else {
            return Ok(());
        };
        for i in &self.inputs {
            if !r.required.contains(&i.role.as_str()) && !r.optional.contains(&i.role.as_str()) {
                return Err(bad(
                    "inputs",
                    format!("role {:?} is not an input of kind {:?}", i.role, self.kind),
                ));
            }
        }
        for role in r.required.iter().chain(r.optional) {
            let n = self.inputs.iter().filter(|i| i.role == *role).count();
            if n > 1 {
                return Err(bad("inputs", format!("role {role:?} listed {n} times")));
            }
            if n == 0 && r.required.contains(role) {
                return Err(bad("inputs", format!("role {role:?} is required")));
            }
        }
        Ok(())
    }

    /// Constraints of the L1 / legacy kinds added by lane adapters (exit 40; interfaces/adapters.md 1.1).
    pub fn check_kind_constraints(&self) -> Result<(), SpecError> {
        if !matches!(
            self.kind,
            JobKind::SolveRecipe
                | JobKind::ProofInstantiate
                | JobKind::ProofInstanceSpan
                | JobKind::CoefficientFacts
                | JobKind::NodeBasis
        ) {
            return Ok(());
        }
        let k = self.kind;
        if self.resume.is_some() {
            return Err(bad(
                "resume",
                format!("kind {k:?} has no resume (rerun from scratch)"),
            ));
        }
        if !self.budgets.work.limits().is_empty() {
            return Err(bad(
                "budgets.work",
                format!("kind {k:?} has no work counters (solve-recipe: --max-trace-rows)"),
            ));
        }
        if self.budgets.mem_soft_bytes.is_some() {
            return Err(bad(
                "budgets.mem_soft_bytes",
                format!("not implemented by kind {k:?}"),
            ));
        }
        if self.budgets.wall.job_s.is_some() {
            return Err(bad(
                "budgets.wall.job_s",
                format!("kind {k:?} is bounded by the supervisor's wall cap"),
            ));
        }
        if self.seed.sample_seed != 0 {
            return Err(bad(
                "seed.sample_seed",
                format!("kind {k:?} fixes the sample seed 0"),
            ));
        }
        if self.on_case_budget != OnCaseBudget::Stop {
            return Err(bad("on_case_budget", format!("kind {k:?}: stop only")));
        }
        let key_budget = self.args.iter().any(|a| a == "--key-budget-s");
        if (k.legacy_wrapped() || k == JobKind::NodeBasis) && self.budgets.wall.case_s.is_some() {
            return Err(bad(
                "budgets.wall.case_s",
                format!("kind {k:?} has no per-case budget"),
            ));
        }
        if key_budget && self.budgets.wall.case_s.is_some() {
            return Err(bad(
                "budgets.wall.case_s",
                "the per-key wall budget is given twice (--key-budget-s and budgets.wall.case_s)",
            ));
        }
        if self.determinism == Determinism::Strict
            && (key_budget || self.budgets.wall.case_s.is_some())
        {
            return Err(bad(
                "determinism",
                "strict: a per-key wall budget is a clock decision",
            ));
        }
        if k == JobKind::SolveRecipe && !self.args.iter().any(|a| a == "--order") {
            return Err(bad("args", "kind SolveRecipe needs --order"));
        }
        // line 7
        if self.arg_value("--probe-primes").is_some() && self.arg_value("--probe-d").is_none() {
            return Err(bad("args", "--probe-primes needs --probe-d (probe mode)"));
        }
        if self.max_m1_rows() == Some(0) {
            return Err(bad(
                "args",
                "--max-m1-rows 0: a zero limit is not allowed (omit it instead)",
            ));
        }
        Ok(())
    }

    /// The value of a two-token flag of `args` (`None` when absent or the flag has no value). The allow-list check
    /// ([`JobSpec::check_args`]) guarantees one occurrence and a value of the flag's shape.
    pub fn arg_value(&self, flag: &str) -> Option<&str> {
        let i = self.args.iter().position(|a| a == flag)?;
        self.args.get(i + 1).map(String::as_str)
    }

    /// Kind solve-recipe, line 7: the probe mode of the job (`--probe-d` given), with the primes it runs (the given
    /// `--probe-primes`, else [`crate::arith::PROBE_DEFAULT_PRIMES`]); `None` = a solve (relation) job. `Err` = a
    /// value that does not parse (a spec that failed validation).
    pub fn probe_mode(&self) -> Result<Option<ProbeArgs>, String> {
        if self.kind != JobKind::SolveRecipe {
            return Ok(None);
        }
        let Some(d) = self.arg_value("--probe-d") else {
            return Ok(None);
        };
        let d0 = crate::arith::Rational::parse_canonical(d)?;
        let primes = match self.arg_value("--probe-primes") {
            Some(p) => crate::arith::parse_prime_list(p)?,
            None => crate::arith::PROBE_DEFAULT_PRIMES.to_vec(),
        };
        Ok(Some(ProbeArgs { d0, primes }))
    }

    /// Kind solve-recipe, line 7: the `--max-m1-rows` cap, if set.
    pub fn max_m1_rows(&self) -> Option<u64> {
        self.arg_value("--max-m1-rows").and_then(|v| v.parse().ok())
    }

    /// Kind proof-instantiate, line 7: the elided-loci mode (`--elided-loci`, default [`ElidedLoci::Refuse`]).
    pub fn elided_loci(&self) -> ElidedLoci {
        match self.arg_value("--elided-loci") {
            Some("facts") if self.kind == JobKind::ProofInstantiate => ElidedLoci::Facts,
            _ => ElidedLoci::Refuse,
        }
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
            if let Err(why) = r.shape.check(v) {
                return Err(SpecError::ArgNotAllowed(format!(
                    "{tok}: {why} (shape {:?})",
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
        // line 7 (seed minimisation): the budget belongs to a mode, and a zero budget is not a limit
        if self.arg_value("--minimise-target").is_some()
            && self.arg_value("--minimise-rules").is_none()
        {
            return Err(bad("args", "--minimise-target needs --minimise-rules"));
        }
        if self.arg_value("--minimise-max-solves").is_some() {
            if self.arg_value("--minimise-rules").is_none() {
                return Err(bad("args", "--minimise-max-solves needs --minimise-rules"));
            }
            if self
                .arg_value("--minimise-max-solves")
                .and_then(|v| v.parse::<u64>().ok())
                == Some(0)
            {
                return Err(bad(
                    "args",
                    "--minimise-max-solves 0: a zero limit is not allowed (omit it instead)",
                ));
            }
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

/// BLAKE3 key-derivation context of [`spec_blake3`] (domain separation by digest kind, lane contractfix2, audit
/// ac2-m7: the spec digest of some bytes is never their raw file digest).
pub const CONTEXT_SPEC: &str = "rustred.child-job.v1 2026-11-14 job spec digest";

/// `spec_blake3`: the BLAKE3 of the `job.json` bytes as written, in the [`CONTEXT_SPEC`] domain (the ledger identity
/// of the job attempt). NOT the plain BLAKE3 of the file (`b3sum job.json` differs): compute it only with this
/// function.
pub fn spec_blake3(spec_file_bytes: &[u8]) -> String {
    let mut h = blake3::Hasher::new_derive_key(CONTEXT_SPEC);
    h.update(spec_file_bytes);
    h.finalize().to_hex().to_string()
}
