mod campaign;
mod candidate_inspect;
mod candidates;
mod derive;
mod entry_domain;
mod family_close;
mod family_solve;
mod owner_domains;
mod owner_guarded;
mod owner_match;
mod resource_limits;
mod routed;
pub(crate) use entry_domain::EntryDomainPlanArgs;
pub(crate) use owner_domains::OwnerDomainScanArgs;
pub(crate) use owner_guarded::OwnerGuardedApplyArgs;
pub(crate) use owner_match::OwnerDomainMatchArgs;
pub(crate) use routed::RoutedCampaignArgs;

pub(crate) use candidate_inspect::CandidateInspectArgs;
pub(crate) use candidates::{CertifyCandidatesArgs, FamilyCandidatesArgs};
use resource_limits::ResourceLimitsArgs;

use std::ffi::OsString;
use std::fmt;
use std::path::PathBuf;

use crate::{ClosingFamilySelector, InputFormat, RelationSelection};

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum StreamPath {
    Stdio,
    File(PathBuf),
}

impl StreamPath {
    fn parse(value: OsString) -> Result<Self, ArgError> {
        if value == "-" {
            Ok(Self::Stdio)
        } else if value.is_empty() {
            Err(ArgError::EmptyPath)
        } else {
            Ok(Self::File(PathBuf::from(value)))
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct DeriveArgs {
    pub(crate) input: StreamPath,
    pub(crate) output: StreamPath,
    pub(crate) input_format: InputFormat,
    pub(crate) relations: RelationSelection,
    pub(crate) n_cores: usize,
    pub(crate) force: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct FamilySolveArgs {
    pub(crate) input: StreamPath,
    pub(crate) output: StreamPath,
    pub(crate) input_format: InputFormat,
    pub(crate) sectors: Option<Vec<Vec<bool>>>,
    pub(crate) n_cores: usize,
    pub(crate) force: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct FamilyCloseArgs {
    pub(crate) input: StreamPath,
    pub(crate) output: StreamPath,
    pub(crate) input_format: InputFormat,
    pub(crate) permutation: Option<Vec<usize>>,
    pub(crate) nonpositive_indices: Vec<usize>,
    pub(crate) resources: ResourceLimitsArgs,
    pub(crate) n_cores: usize,
    pub(crate) progress: bool,
    pub(crate) force: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct CampaignPlanArgs {
    pub(crate) input: StreamPath,
    pub(crate) output: StreamPath,
    pub(crate) input_format: InputFormat,
    pub(crate) root_id: Option<String>,
    pub(crate) force: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct CampaignPreflightArgs {
    pub(crate) profile: StreamPath,
    pub(crate) output: StreamPath,
    pub(crate) n_cores: usize,
    pub(crate) max_memory_bytes: u64,
    pub(crate) force: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct CampaignGenerateArgs {
    pub(crate) family: ClosingFamilySelector,
    pub(crate) output: StreamPath,
    pub(crate) force: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct CampaignInspectArgs {
    pub(crate) artifact: StreamPath,
    pub(crate) resources: ResourceLimitsArgs,
    pub(crate) output: StreamPath,
    pub(crate) force: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct CampaignReduceArgs {
    pub(crate) artifact: StreamPath,
    pub(crate) resources: ResourceLimitsArgs,
    pub(crate) target_powers: Vec<i64>,
    pub(crate) max_rule_applications: usize,
    pub(crate) output: StreamPath,
    pub(crate) force: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct FoundryCampaignRunArgs {
    pub(crate) config: StreamPath,
    pub(crate) output: StreamPath,
    pub(crate) measurements_output: Option<StreamPath>,
    pub(crate) no_progress: bool,
    pub(crate) color: ColorPolicy,
    pub(crate) force: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct FoundryWaveCampaignRunArgs {
    pub(crate) config: StreamPath,
    pub(crate) output: StreamPath,
    pub(crate) measurements_output: Option<StreamPath>,
    pub(crate) artifact_output: Option<StreamPath>,
    pub(crate) n_cores: usize,
    pub(crate) no_progress: bool,
    pub(crate) color: ColorPolicy,
    pub(crate) force: bool,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) enum ColorPolicy {
    #[default]
    Auto,
    Always,
    Never,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum Command {
    EntryDomainPlan(EntryDomainPlanArgs),
    OwnerGuardedApply(OwnerGuardedApplyArgs),
    OwnerDomainMatch(OwnerDomainMatchArgs),
    OwnerDomainScan(OwnerDomainScanArgs),
    RoutedCampaign(RoutedCampaignArgs),
    Derive(DeriveArgs),
    FamilySolve(FamilySolveArgs),
    FamilyClose(FamilyCloseArgs),
    FamilyCandidates(FamilyCandidatesArgs),
    CandidateInspect(CandidateInspectArgs),
    CertifyCandidates(CertifyCandidatesArgs),
    CampaignPlan(CampaignPlanArgs),
    CampaignPreflight(CampaignPreflightArgs),
    CampaignGenerate(CampaignGenerateArgs),
    CampaignInspect(CampaignInspectArgs),
    CampaignReduce(CampaignReduceArgs),
    CampaignShards(Vec<OsString>),
    CampaignMonitor(Vec<OsString>),
    PreparationMonitor(Vec<OsString>),
    WalkVerifyClosure(super::walk_verify::WalkVerifyClosureArgs),
    WalkRescuePlan(super::walk_rescue::WalkRescuePlanArgs),
    WalkInventory(super::walk_inventory::WalkInventoryArgs),
    WalkMasterReduce(super::master_reduction::MasterArgs),
    MasterInspect(super::master_reduction::MasterInspectArgs),
    ArtifactInspect(super::artifact_inspect::InspectArgs),
    FoundryCampaignRun(FoundryCampaignRunArgs),
    FoundryWaveCampaignRun(FoundryWaveCampaignRunArgs),
    Help,
    Version,
    WalkSemanticsVersion,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ArgError {
    NonUtf8Option(OsString),
    MissingCommand,
    MissingSubcommand(&'static str),
    UnknownCommand(String),
    UnknownSubcommand {
        command: &'static str,
        subcommand: String,
    },
    UnknownOption(String),
    DuplicateOption(&'static str),
    MissingValue(&'static str),
    MissingRequiredOption(&'static str),
    UnexpectedArgument(String),
    InvalidValue {
        option: &'static str,
        value: String,
        expected: &'static str,
    },
    EmptyPath,
    InvalidCombination(&'static str),
}

impl fmt::Display for ArgError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NonUtf8Option(value) => {
                write!(formatter, "command-line option is not UTF-8: {value:?}")
            }
            Self::MissingCommand => formatter.write_str(
                "missing command; expected `derive`, `family-solve`, `family-close`, `family-candidates`, `certify-candidates`, or `campaign`",
            ),
            Self::MissingSubcommand(command) => write!(
                formatter,
                "missing {command} subcommand; expected `plan`, `preflight`, `run`, `run-waves`, `generate`, `inspect`, or `reduce`"
            ),
            Self::UnknownCommand(command) => write!(formatter, "unknown command {command:?}"),
            Self::UnknownSubcommand {
                command,
                subcommand,
            } => write!(
                formatter,
                "unknown {command} subcommand {subcommand:?}; expected `plan`, `preflight`, `run`, `run-waves`, `generate`, `inspect`, or `reduce`"
            ),
            Self::UnknownOption(option) => write!(formatter, "unknown option {option:?}"),
            Self::DuplicateOption(option) => {
                write!(formatter, "option {option} was supplied twice")
            }
            Self::MissingValue(option) => write!(formatter, "option {option} needs a value"),
            Self::MissingRequiredOption(option) => {
                write!(formatter, "required option {option} was not supplied")
            }
            Self::UnexpectedArgument(argument) => {
                write!(formatter, "unexpected positional argument {argument:?}")
            }
            Self::InvalidValue {
                option,
                value,
                expected,
            } => write!(
                formatter,
                "invalid value {value:?} for {option}; expected {expected}"
            ),
            Self::EmptyPath => formatter.write_str("an input or output path cannot be empty"),
            Self::InvalidCombination(detail) => formatter.write_str(detail),
        }
    }
}

impl std::error::Error for ArgError {}

pub(crate) fn parse_args(
    arguments: impl IntoIterator<Item = OsString>,
) -> Result<Command, ArgError> {
    let mut arguments = arguments.into_iter();
    let _program = arguments.next();
    let Some(command) = arguments.next() else {
        return Err(ArgError::MissingCommand);
    };
    let command = command.into_string().map_err(ArgError::NonUtf8Option)?;
    match command.as_str() {
        "--help" | "-h" | "help" => {
            reject_trailing(arguments)?;
            Ok(Command::Help)
        }
        "--version" | "-V" => {
            reject_trailing(arguments)?;
            Ok(Command::Version)
        }
        "walk-semantics-version" => {
            reject_trailing(arguments)?;
            Ok(Command::WalkSemanticsVersion)
        }
        "derive" => derive::parse(arguments),
        "entry-domain-plan" => entry_domain::parse(arguments),
        "family-solve" => family_solve::parse(arguments),
        "family-close" => family_close::parse(arguments),
        "family-candidates" => candidates::parse_generation(arguments),
        "candidate-inspect" => candidate_inspect::parse(arguments),
        "preparation-monitor" => Ok(Command::PreparationMonitor(arguments.collect())),
        "certify-candidates" => candidates::parse_certification(arguments),
        "campaign" => campaign::parse(arguments),
        "routed-campaign" => routed::parse(arguments),
        "owner-domain-scan" => owner_domains::parse(arguments),
        "owner-domain-match" => owner_match::parse(arguments),
        "owner-guarded-apply" => owner_guarded::parse(arguments),
        "walk-verify-closure" => super::walk_verify::parse(arguments),
        "walk-rescue-plan" => super::walk_rescue::parse(arguments),
        "walk-inventory" => super::walk_inventory::parse(arguments),
        "walk-master-reduce" => super::master_reduction::parse(arguments),
        "walk-publish" => super::master_reduction::parse_publish(arguments),
        "master-inspect" => super::master_reduction::parse_inspect(arguments),
        "artifact-inspect" => super::artifact_inspect::parse(arguments),
        _ => Err(ArgError::UnknownCommand(command)),
    }
}

fn reject_trailing(arguments: impl IntoIterator<Item = OsString>) -> Result<(), ArgError> {
    if let Some(argument) = arguments.into_iter().next() {
        let argument = argument.into_string().map_err(ArgError::NonUtf8Option)?;
        Err(ArgError::UnexpectedArgument(argument))
    } else {
        Ok(())
    }
}

pub(super) fn next_value(
    arguments: &mut impl Iterator<Item = OsString>,
    option: &'static str,
) -> Result<OsString, ArgError> {
    arguments.next().ok_or(ArgError::MissingValue(option))
}

pub(super) fn next_utf8_value(
    arguments: &mut impl Iterator<Item = OsString>,
    option: &'static str,
) -> Result<String, ArgError> {
    next_value(arguments, option)?
        .into_string()
        .map_err(ArgError::NonUtf8Option)
}

pub(super) fn parse_positive_integer(
    option: &'static str,
    value: String,
) -> Result<usize, ArgError> {
    value
        .bytes()
        .all(|byte| byte.is_ascii_digit())
        .then(|| value.parse::<usize>().ok())
        .flatten()
        .filter(|value| *value > 0)
        .ok_or(ArgError::InvalidValue {
            option,
            value,
            expected: "a positive integer",
        })
}

fn parse_nonnegative_integer(option: &'static str, value: String) -> Result<usize, ArgError> {
    value
        .bytes()
        .all(|byte| byte.is_ascii_digit())
        .then(|| value.parse::<usize>().ok())
        .flatten()
        .ok_or(ArgError::InvalidValue {
            option,
            value,
            expected: "a nonnegative integer fitting this platform",
        })
}

fn set_once<T>(slot: &mut Option<T>, option: &'static str, value: T) -> Result<(), ArgError> {
    if slot.is_some() {
        Err(ArgError::DuplicateOption(option))
    } else {
        *slot = Some(value);
        Ok(())
    }
}

pub(crate) const HELP: &str = "\
RustRed: pure-Rust parametric IBP/LI derivation with Symbolica

USAGE:
    rustred entry-domain-plan --input SPEC.json --output PLAN.json [--force]
    rustred owner-guarded-apply --manifest SELECTION.json --queries QUERIES.json --output RESULT.json [--owner-base DIR] [--work-limits LIMITS.json] [--events EVENTS.jsonl] [--stop-file PATH] [--no-progress] [--max-queries N] [--max-report-events N] [--max-report-bytes N] [--max-expression-bytes N]
    rustred owner-domain-match --manifest SELECTION.json --queries QUERIES.json --output RESULT.json [--owner-base DIR] [--events EVENTS.jsonl] [--stop-file PATH] [--no-progress] [--max-queries N] [--max-query-bytes N] [--max-total-pieces N] [--max-rules-per-query N] [--max-terminal-checks-per-query N] [--max-predicates-per-query N] [--max-pieces-per-query N] [--max-cells-per-query N] [--max-split-operations-per-query N] [--max-coordinate-cells-per-query N] [--max-bounded-refinement-cells-per-query N] [--bounded-refinement-axes inactive-only|finite-axes] [--max-guard-univariate-degree N] [--follow-successors [--workers N] [--max-domains N] [--max-frontiers N] [--frontier-policy record|stop] [--max-successor-events N] [--max-containment-checks N|unlimited] [--transfer-unreserved-lookahead H [--reuse-initial-d-bands] [--g2-residual-anchors off|union [--g2-activate-on-resume]]] [--max-rhs-cells-per-query N] [--max-term-visits-per-query N] [--max-native-operations-per-query N] [--max-rhs-events-per-query N] [--max-shift-groups-per-query N] [--route-domain-overcover [--max-route-masks-per-query N]] [--resume DIR [--amend-queries AMENDMENT.json]...]]
    rustred owner-domain-scan --manifest SELECTION.json (--max-numerator-rank R | --unbounded-rank) --output RESULT.json [--owner-base DIR] [--events EVENTS.jsonl] [--stop-file PATH] [--max-rules-per-owner N] [--max-terms-per-owner N] [--max-regions-per-owner N] [--max-total-regions N] [--max-summary-groups N] [--factor-census [--factor-census-numerators] [--factor-census-max-terms N]]
    rustred routed-campaign --manifest SELECTION.json --targets TARGETS.csv --output RESULT.json [--entry-domains DOMAINS.json] [--events EVENTS.jsonl] [--owner-base DIR] [--workers 1..50] [--stop-file PATH] [--expansion-limits LIMITS.json] [--max-nodes N] [--max-input-targets N] [--max-transport-operations N] [--max-transport-endpoints N] [--max-coalescing-additions N] [--max-rule-applications N]
    rustred derive [OPTIONS]
    rustred family-solve [OPTIONS]
    rustred family-close [OPTIONS]
    rustred family-candidates [OPTIONS]
    rustred candidate-inspect --input BUNDLE [--options OPTIONS_JSON] [--output FILE]
    rustred preparation-monitor --snapshot PATH [--once | --json]
    rustred certify-candidates [OPTIONS]
    rustred campaign plan [OPTIONS]
    rustred campaign preflight [OPTIONS]
    rustred campaign run [OPTIONS]
    rustred campaign run-waves [OPTIONS]
    rustred campaign generate [OPTIONS]
    rustred campaign inspect [OPTIONS]
    rustred campaign reduce [OPTIONS]
    rustred walk-semantics-version
    rustred walk-verify-closure --command WALK_ARGV.json [--checkpoint DIR] [--result RESULT.json | --no-result] [--output REPORT.json] [--threads N] [--reinspect all|none|sample:N[:SEED]] [--brute-force-max-points N] [--brute-force-point-budget N] [--require-closure] [--reference-levers off|as-run] [--union-sample COUNT[:SEED]] [--mutate KIND] [--certification-scope auto|all-roots|physics-queries] [--max-violations N] [--force]
    rustred walk-rescue-plan --command WALK_ARGV.json [--checkpoint DIR] [--helper-id-prefix TEXT] [--rescue-helpers QUERIES.json] [--max-repeats N] [--rescue-scope class|tainted] [--amendment-output AMENDMENT.json] [--output PLAN.json] [--force]
    rustred walk-inventory (--campaign-directory DIR | --command WALK_ARGV.json) [--checkpoint DIR] [--threads N] [--normalize-terminals] [--rules-start N] [--terminals-start N] [--normalized-terminals-start N] [--page-size N] [--output REPORT.json] [--force]
    rustred walk-master-reduce --command WALK_ARGV.json --checkpoint DIR --directory DIR [--resume | --previous-artifact DIR] [--seed-depth N] [--threads N] [--events PATH] [--stop-file PATH] [--checkpoint-interval-seconds N]
    rustred master-inspect --artifact DIR [--format auto|table|json]
    rustred artifact-inspect (--campaign-directory DIR | --artifact DIR) [--format auto|table|json] [--threads N]
    rustred walk-publish --command WALK_ARGV.json --checkpoint DIR --directory OUTPUT [--resume] [--previous-artifact DIR] [--threads N] [--events FILE] [--stop-file FILE]
    rustred walk-master-reduce --artifact SOURCE --directory OUTPUT [--resume] [--seed-depth N] [--containing-sector-depth N] [--saved-rule-assistance] [--circuit-symmetry-assistance] [--finite-feedback | --no-finite-feedback] [--normalization-profile conservative|standard] [--collection-artifact PEER (repeatable)] [--threads N] [--events FILE] [--stop-file FILE]

DERIVE OPTIONS:
    --input <PATH|->             Read from PATH, or standard input with - [default: -]
    --output <PATH|->            Write TOML to PATH, or standard output with - [default: -]
    --input-format <FORMAT>      auto, toml, or symbolica [default: auto]
    --relations <SELECTION>      all, ordinary, or li [default: all]
    --n-cores <COUNT>            Maximum worker cores for parallel stages [default: 1]
    --force                      Atomically replace an existing output file

FAMILY-CLOSE OPTIONS:
    --input <PATH|->             Read external unit-mass vacuum family input [default: -]
    --output <PATH|->            Write complete durable artifact bytes [default: -]
    --input-format <FORMAT>      auto, toml, or symbolica [default: auto]
    --permutation <N,N,...>      Optional zero-based coordinate priority permutation
    --nonpositive-indices <N,N,...>  Coordinates restricted to nonpositive powers
    --max-domain-bound-endpoint-cells <N>  Publication endpoint budget [core default]
    --max-predicate-consistency-work <N>   Publication consistency budget [core default]
    --max-predicate-atoms <N>              Publication atom limit [default: 32; maximum: 256]
    --n-cores <COUNT>            Maximum worker cores [default: 1]
    --progress                  Also emit plain progress when stderr is redirected
    --force                      Atomically replace an existing output file

FAMILY-CANDIDATES OPTIONS:
    --input <PATH|->             Read supplied family text [default: -]
    --output <PATH|->            Write UNCERTIFIED candidate bundle [default: -]
    --report-output <PATH|->     Optional separate phase-timing TOML report
    --input-format <FORMAT>      auto, toml, or symbolica [default: auto]
    --permutation <N,N,...>      Optional zero-based coordinate priority permutation
    --discovery-strategy <PATH>  JSON file selecting finite source/sector discovery scheduling
    --integral-order <PATH>      JSON file selecting a persisted uncut mathematical order
                                Incompatible with --permutation; separate from source scheduling
    --selected-sectors <MASKS>   Generate only comma-separated nonzero binary masks, not their downsets
    --nonpositive-indices <N,N,...>  Coordinates restricted to nonpositive powers
    --n-cores <COUNT>            Maximum worker cores [default: 1]
    --exact-backend <BACKEND>    sparse, sparse-factorized, sparse-target-factorized, or semi-numerical [default: sparse]
    --numerical-depth <DEPTH>    Search depth for fully fixed cases; zero keeps initial seeds [default: 2]
    --max-numerator-rank <R>     Experimental entry sum(max(-n_i,0)) bound; positive powers unbounded
    --finite-case-policy <MODE>  search (default) or retain-rank-finite (requires R)
    --finite-max-visited-points <N>      Per-sector finite enumeration budget (default 1000000)
    --finite-max-retained-terminals <N> Per-sector retained terminal limit (default 1000000)
    --case-max-work-items <N>            Shared per-conjunction exact geometry work (default 4096)
    --case-max-terms-per-conjunction <N> Exact geometry term budget (default 100000)
    --case-max-normalizations <N>        Per-conjunction normalization budget (default 1024)
    --case-max-factorizations <N>        Per-conjunction factorization budget (default 4096)
    --bundle-max-bytes <N>       Native bundle byte budget (default 268435456; hard maximum 1073741824)
    --bundle-max-entries <N>     Native collection-entry budget (default 1000000)
    --bundle-max-coefficient-bytes <N>       Per-coefficient byte budget (default 16777216)
    --bundle-max-total-coefficient-bytes <N> Total coefficient-table byte budget (default 134217728)
    --checkpoint-dir <PATH>     Dedicated trusted-local native sector checkpoint directory
    --resume                    Reuse a matching checkpoint; requires --checkpoint-dir
    --checkpoint-max-bytes <N>  Positive total checkpoint payload budget [default: 1073741824]
    --progress                  Also emit plain progress when stderr is redirected
    --progress-json <PATH>      Atomically publish bounded JSON observations at 1 Hz, independent of TTY
    --force                      Atomically replace existing output files

PREPARATION-MONITOR OPTIONS:
    --snapshot <PATH>          Read a rustred.preparation-progress.v1 supervisor snapshot
    --once                     Print every parent once, without terminal control codes
    --json                     Print the validated snapshot once for another consumer
                               Read-only; signals to the campaign still stop work
                               Generation progress/checkpoints are not family closure

CERTIFY-CANDIDATES OPTIONS:
    --input <PATH|->             Read saved uncertified candidate bundle [default: -]
    --output <PATH|->            Write independently certified artifact [default: -]
    --report-output <PATH|->     Optional separate phase-timing TOML report
    --max-domain-bound-endpoint-cells <N>  Certification endpoint budget [core default]
    --max-predicate-consistency-work <N>   Certification consistency budget [core default]
    --max-predicate-atoms <N>              Certification atom limit [default: 32; maximum: 256]
    --max-negative-index-degree <N>        Request rank-scoped certification (currently fail-closed; maximum 30)
    --max-total-excess-degree <D>          Entry sum of dots plus negative powers; unsigned 64-bit [default: unrestricted]
                                          Descendant bounds are independently proved and may exceed D;
                                          mutually exclusive with --max-negative-index-degree
    --force                      Atomically replace existing output files

FAMILY-SOLVE OPTIONS:
    --input <PATH|->             Read arbitrary family Project TOML from PATH or stdin
    --output <PATH|->            Write diagnostic TOML to PATH or stdout
    --input-format <FORMAT>      auto, toml, or symbolica [default: auto]
    --sectors <BITS,BITS,...>    Optional comma-separated sector bit strings; otherwise enumerate safely
    --n-cores <COUNT>            Maximum worker cores [default: 1]
    --force                      Atomically replace an existing output file

CAMPAIGN PLAN OPTIONS:
    --input <PATH|->             Read from PATH, or standard input with - [default: -]
    --output <PATH|->            Write TOML to PATH, or standard output with - [default: -]
    --input-format <FORMAT>      auto, toml, or symbolica [default: auto]
    --root-id <ID>               Root ID for one raw Symbolica campaign input
    --force                      Atomically replace an existing output file

CAMPAIGN PREFLIGHT OPTIONS:
    --profile <PATH|->           Read a physical resource profile from PATH or -
    --output <PATH|->            Write TOML to PATH, or standard output with - [default: -]
    --n-cores <COUNT>            Requested execution-width ceiling [default: 1]
    --max-memory <SIZE>          Operational memory limit (B/KiB/MiB/GiB/TiB)
    --force                      Atomically replace an existing output file

FOUNDRY CAMPAIGN RUN OPTIONS:
    --config <PATH|->            Read strict versioned campaign TOML from PATH or stdin
    --output <PATH|->            Write deterministic diagnostic TOML to PATH or stdout
    --measurements-output <PATH|->
                                 Optionally write nonsemantic timing TOML separately
    --no-progress                Disable the interactive stderr dashboard
    --color <WHEN>               auto, always, or never [default: auto]
    --force                      Atomically replace existing output files

FOUNDRY WAVE CAMPAIGN RUN OPTIONS:
    --config <PATH|->            Read strict versioned campaign TOML from PATH or stdin
    --output <PATH|->            Write deterministic diagnostic TOML to PATH or stdout
    --measurements-output <PATH|->
                                 Optionally write nonsemantic timing TOML separately
    --artifact-output <PATH|->   Write canonical durable K6 bytes after successful closure
    --n-cores <COUNT>            Sibling workers within one atomic wave [default: 1]
    --no-progress                Disable the interactive stderr dashboard
    --color <WHEN>               auto, always, or never [default: auto]
    --force                      Atomically replace existing output files

CAMPAIGN GENERATE OPTIONS:
    --family <SELECTOR>          unit-mass-vacuum-k1 or unit-mass-vacuum-k3
    --output <PATH|->            Write durable artifact bytes to PATH or stdout [default: -]
    --force                      Atomically replace an existing output file

CAMPAIGN INSPECT OPTIONS:
    --artifact <PATH|->          Read durable artifact bytes from PATH or standard input
    --max-domain-bound-endpoint-cells <N>  Cold-replay endpoint budget [core default]
    --max-predicate-consistency-work <N>   Cold-replay consistency budget [core default]
    --max-predicate-atoms <N>              Cold-replay atom limit [default: 32; maximum: 256]
    --output <PATH|->            Write TOML to PATH, or standard output with - [default: -]
    --force                      Atomically replace an existing output file

CAMPAIGN REDUCE OPTIONS:
    --artifact <PATH|->          Read durable artifact bytes from PATH or standard input
    --powers <N,...>             Signed integer target powers in denominator order
    --max-rule-applications <N>  Per-request recurrence ceiling [default: 1000000]
    --max-domain-bound-endpoint-cells <N>  Cold-replay endpoint budget [core default]
    --max-predicate-consistency-work <N>   Cold-replay consistency budget [core default]
    --max-predicate-atoms <N>              Cold-replay atom limit [default: 32; maximum: 256]
    --output <PATH|->            Write TOML to PATH, or standard output with - [default: -]
    --force                      Atomically replace an existing output file

GENERAL OPTIONS:
    -h, --help                   Print this help
    -V, --version                Print the RustRed version

`derive` generates fully parametric identities. Any concrete target carried by
the input is validated and reported as not processed; it is never reduced by
this command.

`family-close` enumerates the complete sector census of a caller-supplied
unshifted unit-mass vacuum family (1..16 coordinates, sole parameter d,
denominator constant terms -1), proves zero sectors, solves every admitted sector,
and publishes only after exact replay, strict descent and complete coverage.
Use --nonpositive-indices to explicitly keep selected numerator coordinates
nonpositive. By default every sector is admitted. Target powers never infer scope.
Failure writes no artifact. Use `campaign inspect` and `campaign reduce` on
the resulting bytes; no family name selects an implementation.
TTY progress overwrites one stderr status field; redirected stderr is quiet
unless --progress is supplied. Artifact stdout is never mixed with progress.
Publication and cold-load resource budgets are caller policy, not artifact
evidence. Zero is a valid restrictive budget. Reapply any chosen budgets for
inspection and reduction; artifacts never raise their loader's limits.
TOML resource reports use decimal strings to preserve the platform integer range.

`family-candidates` prepares sources, solves the supplied root, and saves
formulas with an explicit uncertified status. It does not replay provenance,
prove global coverage, or emit a closing artifact. `certify-candidates`
independently reconstructs and replays that bundle through the existing complete
publication pipeline without running search again; incomplete inputs fail.
Finite row/job discovery can be selected with `--discovery-strategy <JSON_FILE>`;
version 1 selects discovery order; version 2 also requires an explicit bounded
rule-selection portfolio. Its one or two alternate source recipes, quality-key
order/version, per-trial budgets and deterministic trigger are checkpoint-bound.
The first valid baseline remains mandatory; optional trials cannot override exact
rule/exception eligibility. Shift-excursion scores are heuristics, not rank bounds.
Neither discovery version changes the mathematical integral order.
`--selected-sectors 011,111` selects exact generation jobs in the original family
axes. The full zero census is retained; omitted nonzero sectors remain uncovered.
`--integral-order <JSON_FILE>` selects a persisted uncut mathematical order
(support priorities, nonnegative weighted excess rows and coordinate ties).
It is distinct from discovery scheduling and cannot be combined with --permutation.
Neither option grants closure authority.
Optional --report-output records phase timings separately from bundle/artifact
bytes. Data is written before its report; destinations must differ.
Optional --checkpoint-dir stores completed sectors without certifying them.
Use --resume with the same source/root/selection/order/discovery/backend/depth/rank; worker count may
change. Input, final bundle and report paths must be outside that dedicated
directory. Its byte budget includes retained/staging payloads and pending writes,
not peak RAM or filesystem overhead; final bundle limits remain unchanged.
Optional --max-numerator-rank declares candidate generation/application entry
scope, not certified coverage or a finite dot bound. It is distinct from search
depth, per-coordinate limits and tensor momentum rank. Omission preserves the
existing unrestricted scope. Rank-scoped candidate bundles cannot currently be
certified, including by the separate total-excess certification path.

`campaign plan` authenticates and interns only the supplied campaign roots.
It does not discover dependencies, derive relations, prove closure, or publish
rules. It deliberately has no --n-cores or --max-memory option.

`campaign preflight` checks a topology-neutral physical resource profile and
reports a ready width or typed memory-capacity pause. It never starts a frontier
or constructs a worker pool.

`campaign run` executes one bounded foundry diagnostic. Its deterministic
report is not a closing artifact; optional timings are emitted only through the
separate measurement sidecar. On a terminal, its stderr dashboard is refreshed
in place; `cap ETA` estimates only time to the configured task ceiling, never
time to mathematical closure. Redirected stderr is quiet by default. The V2
configuration uses disjoint `autonomous` and `external-hints-only` modes;
autonomous requests cannot carry caller-authored search hints.

`campaign run-waves` executes the `full-rank-atomic-waves` itinerary. Its
successful result can still be incomplete; same-rank siblings publish only as
a complete atomic wave. After complete publication, `--artifact-output`
writes bytes only after exact installation and one successful cold reload. Its
terminal-only dashboard aggregates detached sibling telemetry and is quiet
when stderr is redirected.

`owner-guarded-apply` inspects explicitly selected saved candidates on their own
guarded one-hop domains. Exit 0 means diagnostic inspections/rendering finished;
complements and RHS problems can remain. It does NOT mean first-priority
applicability, integer feasibility, recursive coverage or closure. Native
expression text is display-only, not an artifact codec. Strict --work-limits
JSON forwards bounded native allowances; report/event/expression limits bound
retained diagnostics. No algebra or queue policy is implemented in Python.

`entry-domain-plan` counts finite starting targets without running a solver.
Its JSON schema is rustred.entry-domain.json.v1, with unique same-arity 0/1
sector strings and exactly one explicit budget or profile object
{name: renormalizable_marginal_feynman, loops: N}. The profile is a caller
physics assumption, not automatic graph authentication. A positive
max_positive_layers_per_sector is required (at most 1000000); counts are exact
decimal strings or the command fails. max_preview_targets defaults to 0 and
is a bounded global prefix (at most 10000 targets / 1000000 coordinates), never
closure or descendant coverage. Input is bounded to 1 MiB, 10000 sectors and
1024 axes. Use '-' for stdin/stdout; existing files require --force.

`owner-domain-match` classifies explicit parametric boxes against saved owners.
Each query JSON supplies its own rank cap (or null) and unbounded upper bounds.
Input defaults to at most 256 queries and 1 MiB of serialized UTF-8. Explicit
--max-queries and --max-query-bytes independently admit larger input; both must
be positive platform-sized integers. They do not change per-query work,
descendant domains, event limits, rank scope or native memory budgets.
Exit 0 means exact local classification, which may include gaps or invalid source
conditions; inspect all_queries_locally_applicable separately. Unresolved,
cancelled or resource-limited classification exits nonzero with a partial report.
Without --follow-successors, no RHS expansion is performed. No mode claims IBP
generation or recursive family closure. Outputs are fresh paths; stop-file
cancellation and compact heartbeat events remain active.
Opt-in --max-bounded-refinement-cells-per-query (default 0) retries unresolved
guards and selected native algebra preflight refusals on exact bounded
inactive-coordinate faces. --bounded-refinement-axes inactive-only is the default;
finite-axes also admits positive axes with explicitly finite box upper bounds.
Unbounded positive axes stay symbolic; numerator rank never bounds positive axes.
The axis policy does not grant any faces: the separate allowance must be positive.
the full split is admitted before any face, and insufficient allowance preserves
the unresolved piece or original typed refusal. Refinement is local
classification, not routing overcover, source generation or family closure.
Cancellation and backend faults are not
reclassified as unresolved geometry.
--max-guard-univariate-degree sets a positive native guard-degree allowance
(default 16) for either mode; it does not alter rank scope or guard semantics.
--follow-successors instead emits a distinct shared symbolic worklist result,
reusing pending containing domains in the same immutable owner snapshot. Native
RHS inspection follows installed literal owners; routes remain explicit frontiers.
Adding --route-domain-overcover follows admitted maps using conservative full
orthants without numerator polynomial expansion. Full roots retain the actual
incoming rank R; losing k active denominators tightens a bounded image to R-k.
Unbounded ranks stay unbounded; the saved entry rank never clips a successor.
Full mapped roots go directly to owner application; strict subsupports reenter
routing. Unchecked source conditions and missing maps remain explicit obligations.
This mode does not identify every covered point as actually reached.
The mode accepts --workers (default 1) and positive --max-domains,
--max-frontiers and --max-successor-events allowances.
--unbounded-work removes cumulative work-count stops for a production walk;
it requires --follow-successors and rejects explicit diagnostic work caps.
It does not remove bounded worker buffers or optional algebra/scratch safeguards.
Use the Python campaign supervisor to protect aggregate process and host RAM.
--checkpoint DIR saves resumable Ordered work state periodically and on a
cooperative stop; --resume DIR restores it under matching immutable inputs and
policies. These flags are mutually exclusive and require --follow-successors.
--checkpoint-interval-seconds N sets the positive save interval (default 3600).
--frontier-policy record|stop decides what a walk does at an explicit frontier
(default record: keep walking; frontiers stay explicit and block every closure
claim either way). stop saves the checkpoint and stops cooperatively (exit 4,
stop_reason frontier_policy) at the first frontier this session commits;
frontiers restored by --resume do not fire again, so a resume continues to the
next new frontier. stop requires --checkpoint or --resume and is bound into
the checkpoint request; record leaves existing checkpoint bindings unchanged.
A resumed unfinished inspection may replay its verified published prefix;
completed logical work is retained. A checkpoint is not a closure certificate.
--apply-subdivision-axis AXIS --apply-subdivision-cut CUT opts into two exact
physical parts of initial Apply boxes spanning the cut (zero-based coordinates).
Both nonnegative options are required, with Ordered successor publication.
The logical parent completes only after both parts; descendants remain shared
ordinary work and are not recursively subdivided by this policy.
--apply-cell-refinement-max-cardinality N optionally refines a selected Apply
cell's single finite varying axis into singleton cells before RHS application.
N must be positive; omission leaves this policy off. It requires --follow-successors
and applies within native inspections, including descendants, without adding workers.
The cardinality threshold controls when to refine, not source scope or cumulative
work; it is unchanged by --unbounded-work or physical-part budget sharing.
All source cases, coupled bounds and descendants remain obligations. The complete
policy is bound into checkpoints; changing it requires a new campaign.
application_refinement_steps/cells report per-shift-group application work, not
extra matched pieces, physical workers or completed logical inspections.
--inspection-workers N optionally partitions --workers W into N inspectors,
W-1-N admission helpers and one coordinator (W>1); W=1 only accepts N=1.
It requires --follow-successors, leaves the default split unchanged when omitted,
and reserves compute slots rather than guaranteeing activity. With a finite
containment comparison cap, N must be W-1 (or 1 inline): no admission helpers.
Aggregate containment comparisons are unlimited by default; --max-containment-checks N sets a positive
finite diagnostic cap, and --max-containment-checks unlimited selects the default.
This leaves all native work, storage and resource limits unchanged.
--transfer-unreserved-lookahead H opts into delegating unreserved contained
domains to later containing domains. With ordered publication, H is a fixed
positive logical dispatch lookahead; ready publication uses outstanding-work
credits instead. It requires --follow-successors and unlimited containment checks.
The default inspects all scheduled domains. Delegation is not native completion;
unresolved representatives and frontiers remain incomplete.
--reuse-initial-d-bands opts into exact partial reuse of an initially admitted
same-owner D band. It requires --follow-successors and
--transfer-unreserved-lookahead H (hence unlimited containment checks).
Only the disjoint residual is inspected again; the original anchor obligation
remains tracked. This does not clip descendants or establish coverage by itself.
--g2-residual-anchors union (default off) inspects a newly dispatched Apply
domain only on the D band that the union of merged same-owner anchors (Native
records, G2' residual records, initial-D-band slices merged strictly before
the plan's snapshot) does not cover point by point; every anchor gets a
dependency edge and a responsibility link. It requires
--transfer-unreserved-lookahead H, is bound into the checkpoint and excludes
physical Apply subdivision. Ordered walks use the snapshot id + 1 - H.
--g2-activate-on-resume (with --resume and --g2-residual-anchors union) switches
a checkpoint written without G2' to G2': a recorded binding amendment; the G2'
log is back-filled from the ledger and the record order, and every stream with
accepted events finishes as a whole inspection.
--finite-replay-initial-domain (experimental v2, default off) attempts exact
discharge of whole initial ID0: a singleton or an exactly equivalent finite
zero-lower A/R/D envelope. No clipped or sampled region is admitted. It requires
--follow-successors, --publication-policy epoch and a fresh --checkpoint;
runtime resume/amendments are not supported in this first slice. Cold
owner-domain-walk-verify repeats the exact trace from the retained typed recipe.
The caller-thread kernel creates no nested worker pool. Optional finite caps:
--finite-replay-max-nodes N (default 1000000),
--finite-replay-max-rule-applications N (1000000),
--finite-replay-max-transport-calls N (1000000),
--finite-replay-max-transport-operations N (64000000),
--finite-replay-max-transport-endpoints N (4000000),
--finite-replay-max-coalescing-additions N (16000000),
--finite-replay-max-positive-layers N (64),
--finite-replay-max-seed-points N (1024),
--finite-replay-max-seed-bytes N (1048576, retained seed buffer only).
Enumeration is complete or declines. Cold re-counts and replays all seeds;
no new terminals or cross-attempt cache are introduced. Trace node/transport
aggregate slots are configured by this opt-in policy; the kernel intersects
with admitted reduction/per-formula limits. Zero
is allowed, and --unbounded-work does not remove them. Budget/frontier misses
fall back to ordinary symbolic inspection; cancellation and hard native errors
do not. No new rules or masters are generated by this option.
Separate admitted reducer aggregate limits can be set explicitly:
--reduction-max-rule-applications N (default 1000000),
--reduction-max-pending-frames N (1000000),
--reduction-max-coalescing-additions N (16000000).
Zero is allowed; --unbounded-work leaves these limits unchanged. Finite replay
uses the MINIMUM of its additional allowance and these admitted limits, not a
replacement. These flags do not alter per-formula algebra/transport limits.
--finite-replay-budget-preflight writes requested/admitted/effective numeric
policy to --output and exits before owner/native preparation or checkpoint
creation. It requires the same finite-replay/fresh-walk flags, without --events
or --stop-file. It reads the manifest/query text but does not validate inputs,
owners, coverage or closure. Actual online/cold work retains the same budget
summary and typed resource refusals for reconciliation.
--publication-policy ordered|owner-batched|ready|epoch requires --follow-successors and
defaults to ordered. The opt-in owner-batched mode shares immutable saved rules
but uses separate phase/owner admission and publication queues. Bounded native
chunks are delivered to destination queues; all descendants remain required.
Its v4 receipt uses composite (bucket, local id) identities. Diagnostic IDs,
cover fragmentation and capped prefixes may differ between worker budgets;
this does not change saved rules or concrete reductions. Chunk synchronization
and owner imbalance can still limit parallel speedup.
The opt-in ready policy uses one shared admission queue and publishes ready
sources, including sources of the same owner, without a global FIFO barrier.
It requires --transfer-unreserved-lookahead H: H bounds outstanding native
responsibilities, not an ID window behind the oldest unfinished source.
It supports checkpoint/resume but cannot be combined with physical subdivision.
Its IDs, covers and work counts may depend on scheduling; saved rules and
obligation requirements are unchanged. Use a new campaign for a changed policy;
never attach an ordered checkpoint to a ready run.
The opt-in epoch policy (walk semantics 4) commits whole native
inspections in bulk merges by one coordinator: Lockstep epochs of the 16 lowest
pending IDs (B = 16, independent of the worker count), every successor resolved
in the merge (canonical minimum-ID index semantics, verified containment), IDs
assigned at merge, full reverse retirement with verified transfers. Results are
identical across worker counts for a fixed B; the diagnostic-only environment
override RUSTRED_EPOCH_LOCKSTEP_B changes the walk and is recorded in the result.
It requires --transfer-unreserved-lookahead H (the value is unused), refuses a
finite containment cap and physical subdivision, defaults --frontier-policy to
stop (no checkpoint needed). With --checkpoint DIR it saves resumable CP6
generations; --resume accepts only this CP6 format, not CP5 or old private
snapshots. CP6 terminal output is checkpoint-only, even on a drained worklist:
finalization is not evaluated and the summary does not claim scoped closure.
Use walk-verify-closure --no-result on the checkpoint for independent
reinspection; omitting --result alone still auto-selects a nearby result.json.
CP6 supports G2 union anchors and rescue amendments on resume. G2 activation
on resume is refused; start a fresh Union campaign. Resume validates the current
CP6 schema, walk semantics and request binding. Without a checkpoint the legacy
memory-only epoch path still emits a full, non-resumable result.
--epoch-inspector-lookup all-miss|snapshot is an explicit CP6 Epoch comparison
control (default all-miss). It requires --follow-successors, --publication-policy
epoch and --checkpoint or --resume. Snapshot performs inspector-side lookup
against a pinned immutable committed view; it does not grant closure authority.
The mode is bound to the checkpoint request and cannot change on resume.
--epoch-rolling opts into bounded rolling publication/inspection overlap for a
checkpoint-enabled Epoch walk. The choice is frozen on resume; omitted retains
the lockstep control. It changes scheduling, not the required mathematical scope.
--epoch-dispatch fifo|adaptive selects pending-job order (default fifo).
--epoch-publication-order oldest-prefix|oldest-ready selects rolling publication
order (default oldest-prefix); --epoch-cut-size N and --epoch-window N tune
bounded batching (1..4096). These require rolling Epoch with checkpoint/resume.
Resume must retain saved publication order and cut; an omitted window inherits
the saved window. They do not change exact inspection or publication checks.
--epoch-result-escrow-jobs E with --epoch-result-escrow-bytes N allows bounded
extra logical reservations behind completed whole results, for rolling
oldest-prefix CP6 only. Default E=0 keeps the ordinary path. Base window + E
must not exceed 4096. N is a returned-result admission threshold, not a strict
RSS cap; running results may overshoot it. No threads or mathematical scope are
added. These fields are frozen on resume; scalar schema5 requires fresh state.
Adaptive requires --epoch-rolling and persists its observations and fairness
state on restart. It does not change algebraic pivots or discard obligations.
--epoch-preparation-workers N reserves P2 helpers within --workers (zero runs
serially). If --inspection-workers is also supplied, both must exactly fill
the non-coordinator budget. Omitted retains the existing helper complement.
--epoch-preparation-max-obligations N and --epoch-preparation-max-retirements N
bound retained per-cut logical scratch counts (default u32::MAX), not RAM bytes,
rank or cumulative work. Exhaustion stops before publication, never truncates.
These options require Epoch successor walking. Logical scratch limits are frozen
on resume; the helper count may change within a valid worker partition.
--route-joint-source-support-pruning enables a conservative shared-numerator
degree bound for simultaneous propagator pinches. It is off by default and is
part of the immutable checkpoint policy; it does not clip descendants.

`walk-semantics-version` prints one JSON line with this executable's
walk_semantics_version, checkpoint_format and checkpoint_schema (the legacy
lanes' CP5 identity), plus per_policy walk semantics, checkpoint_formats and
the resumable epoch_checkpoint identity,
then exits 0.
It reads no file and runs no algebra. A paused walk checkpoint resumes on a
different executable digest only when the saved manifest carries the same
format, schema and walk semantics version.

`--amend-queries FILE` (repeatable, only with --resume) applies the resume-time
frontier rescue: an append-only, digest-chained amendment (schema
rustred.owner-domain-walk-amendment.json.v1: sequence, parent digest, query
rows) that adds protected queries, typically bounded helpers covering the
physics region of a frontier-bearing helper. The first amendment chains from
the checkpoint's request binding, each later one from the previous file's
blake3 digest; every recorded amendment must be supplied again, in order.
An amended walk quarantines, at every resume, every node that reaches a
frontier (no later lookup resolves into it) and reports per-query
certification through any closed containing input root.

`walk-rescue-plan` reads one saved walk generation (named like
walk-verify-closure by its argv, with every recorded --amend-queries),
classifies its frontiers (known rescues: guard obstructions on rank- or
positive-power-unbounded nodes) and writes the next amendment for the physics
queries (explicit immutable query_roles declaration) that the frontier taint blocks. Exit 0
for verdicts rescue, no_amendment_needed and no_frontier; exit 1 when the
owner must decide (unknown_frontier_class, rescue_exhausted).

`walk-verify-closure` is an offline oracle over one saved CP5 walk
generation, named by the walk's own owner-domain-match argv (a JSON list, or
an object with a `command` list). It checks the checkpoint's request binding,
record/domain parity, the F8 seal rule against the saved seal flags, exact
alias and partial-anchor inclusion with their dependency edges (a partial
record must be Apply, anchored on an earlier initial Native record, with the
recorded residual equal to its D < cut slice and Q inside anchor u residual,
decided exactly), re-derives
dependency closure from the saved edges, binds the published result.json
(default: next to the command file; `--no-result` skips) record by record to
the generation, and (unless `--reinspect none`) re-inspects natives with the
walker's native visitor under the run's request and every walk-level reuse
lever off (`--reference-levers off`, the default, also turns the native
shortcuts off: Route joint source-support pruning; `as-run` keeps them):
frontier, error, event and Apply successor counts must match (counts are
informational when a lever on in the run is off in the reference), and
every admitted domain must be contained in a recorded target of its parent or
along that target's alias chain. Small cells are also checked by lattice-point
enumeration; `--union-sample COUNT[:SEED]` also validates the exact
multi-target cover predicate against enumeration on COUNT sampled saved cells
(earlier same-owner natives as real covers, 2-3-way splits). The report
(JSON) separates helper roots from physics queries.
Successor generation is reproduced by the same visitor, not derived
independently; F10 independently checks the graph bookkeeping.
`--require-closure` requires every root closed AND independently verified
(every native of its cone re-inspected). Exit status: 0 PASS (no violation,
every native re-inspected), 1 FAIL, 9 INCOMPLETE (no violation, but
re-inspection was partial or none: never a certificate). A gate asserts
`verdict == PASS` and `roots_independently_verified == roots_total`
(examples/python/assert_oracle_pass.py). `--mutate` injects
one defect in memory and must FAIL (`alias-chain-detour` is a positive
control and must PASS).

`walk-inventory` re-inspects a saved CP6 campaign without modifying it or
generating rules. Choose --campaign-directory (the published active run) or
--command (the saved native argv). It separately reports installed rule and
residual counts, then unique rules and terminal keys observed in the inspected
domain cover. These conservative covers can contain points not reached by a
particular concrete reduction. --normalize-terminals applies existing exact
structural identities to that observed terminal set, not to all stored
residuals; normalized terminals are not asserted independent or minimal masters.
Totals are unpaginated; rule and terminal lists use --rules-start,
--terminals-start, --normalized-terminals-start and --page-size (1..1000,
default 25). Each list has its own offset. JSON goes to stdout
unless --output is given. Existing reports require --force; campaign inputs
and checkpoint data cannot be overwritten even with --force. SIGINT/SIGTERM
cancel inspection cooperatively and never stop or alter the producer campaign.

Independent starting-owner campaigns (opt-in, Linux):
  rustred campaign shards --config CONFIG.json --directory DIR
  rustred campaign shards --directory DIR --resume
  rustred campaign monitor --directory DIR [--once|--json]
Independent shards retain all saved rules/routes, with separate queues and
checkpoints under one aggregate compute/RAM budget. Completion requires every
shard; output shares native payloads once. The Rust dashboard reports measured
CPU, per-shard work and checkpoint status; shard counts are not a closure ETA.
Streamed aggregate events and retained frontier storage have separate budgets.
--max-rhs-cells-per-query counts all refined
RHS cells (not only pinches); --max-term-visits-per-query and
--max-native-operations-per-query adjust native inspection work budgets.
--max-rhs-events-per-query and --max-shift-groups-per-query set distinct native
per-domain allowances (default 1000000 each); the aggregate event allowance
does not change them. --max-sign-splits-per-query independently controls the
native sign-partition work allowance (default 1000000).
Worker-local work may precede canonical publication;
attempted work and committed events are reported separately.
--max-route-masks-per-query requires --route-domain-overcover and bounds each
symbolic routing call. Unresolved work or work-limit exhaustion is incomplete;
an exhausted local worklist is not a family-closure claim.

`campaign generate` writes a deterministic durable artifact encoding.
`campaign inspect` loads and authenticates durable artifact bytes once, then
writes their canonical metadata as TOML.

`campaign reduce` loads and applies the supplied artifact and emits exact
typed-master coefficients at unit mass together with the separate power of
`mass_squared` required by dimensional homogeneity.
";
