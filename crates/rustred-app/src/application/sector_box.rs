//! `sector-box-reduce`: reduction with the sector-box engine
//! ([`rustred::solver::sector_box`]) at fixed rational values of all
//! parameters but one, with optional closure under a parameter derivative.
//!
//! One request (schema [`SECTOR_BOX_REDUCE_SPEC_SCHEMA`]) names a family in
//! the `rustred.project.toml.v1` format, the targets and the engine options.
//! The modular phase (seeding, elimination, master selection) runs once. With
//! `derivative_slot = s` it is repeated until the demand is closed under
//! `I[a] -> I[a + e_s]` for every master with `a_s > 0`, so that the result
//! also reduces the derivative of every master with respect to the mass on
//! line `s` (the differential equation is assembled by the caller from
//! `d/dm I[a] = a_s I[a + e_s]`). Then, for every value of the dimension
//! in `d_list`, the finite-field lane reconstructs every coefficient exactly
//! in the free parameter, verified at a fresh prime, and writes
//! `solution-<d>.json` (`rustred-harness-solution.v1`). A probabilistic
//! certificate is checked at a prime that is neither a reconstruction nor a
//! verification prime of that solution.
//!
//! With `checkpoint_dir` the modular phase is saved after it completes
//! (session codec, bound to the family, the router, the options and the
//! request) together with the closed target list, and finite-field probes
//! are logged; a restarted request resumes from both. The output is labelled
//! `uncertified-sector-box`: the certificate is probabilistic.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Instant;

use rustred::family::IntegralFamily;
use rustred::sector::CutConstraint;
use rustred::solver::SolverError;
use rustred::solver::sector_box::{
    BoxLaportaSession, BoxLimits, BoxOptions, CensusRouter, ColumnOrder, FfOptions, IdentityRouter,
    MasterCheckMode, ModularReport, SectorRouter, Seeding, SlotPriority, TubeOptions,
};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use symbolica::domains::integer::Integer;
use symbolica::domains::rational::Rational;

use super::AppError;
use super::input::prepare_input;
use super::lowering::lower_project;
use super::options::InputFormat;

pub const SECTOR_BOX_REDUCE_SPEC_SCHEMA: &str = "rustred.sector-box-reduce-spec.v1";
pub const SECTOR_BOX_REDUCE_REPORT_SCHEMA: &str = "rustred.sector-box-reduce-report.v1";
/// Family arities the command dispatches to (the engine is generic in the
/// storage width; each width is compiled once). 3 and 6 are covered by the
/// default tests, 10 and 15 by the ignored four- and five-loop goldens.
pub const SECTOR_BOX_ARITIES: &[usize] = &[3, 6, 10, 15];
/// Default certificate prime: the engine's
/// [`rustred::solver::sector_box::DEFAULT_CERTIFICATE_PRIME`], `2^63 - 25`,
/// above every reconstruction and verification prime (they descend from
/// `2^62`) and distinct from the structure prime `2^61 - 1`.
pub use rustred::solver::sector_box::DEFAULT_CERTIFICATE_PRIME;
const CHECKPOINT_SCHEMA: &str = "rustred.sector-box-reduce-checkpoint.v1";

/// JSON request of `rustred sector-box-reduce`. Field names shared with
/// `litered-reduce` (`targets`, `preferred`, `seed`, `max_primes`,
/// `workers`) have the same meaning.
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct SectorBoxReduceSpec {
    pub schema: String,
    /// The family, a `rustred.project.toml.v1` file (`[family]` table; the
    /// top-level `parameters` list fixes the parameter order).
    pub family: PathBuf,
    /// Directory of the solutions (`solution-<d>.json`) and the report.
    pub out_dir: PathBuf,
    /// Integrals to reduce, in the family's denominator order.
    pub targets: Vec<Vec<i64>>,
    /// Preferred masters, forced to the easy end of the column order. One
    /// whose route onto its representative sector has several terms is
    /// dropped from the preferred set and reported (list it in `targets` to
    /// reduce it).
    #[serde(default)]
    pub preferred: Vec<Vec<i64>>,
    /// Expected master count per class (Kira id of the representative
    /// sector, as a decimal string): master check and growth trigger.
    #[serde(default)]
    pub expected: BTreeMap<String, usize>,
    /// Close the demand under `I[a] -> I[a + e_slot]` for masters with
    /// `a_slot > 0` (derivative in the mass of that line).
    #[serde(default)]
    pub derivative_slot: Option<usize>,
    /// Maximal closure rounds after the first modular phase (default 8); an
    /// unclosed demand after them is an error.
    #[serde(default)]
    pub closure_rounds: Option<usize>,
    /// Values of the family dimension, `"p/q"`; one solution each.
    #[serde(default)]
    pub d_list: Vec<String>,
    /// Further fixed parameters, name -> `"p/q"`.
    #[serde(default)]
    pub fixed: BTreeMap<String, String>,
    /// The free parameter; default: the only parameter not fixed.
    #[serde(default)]
    pub free: Option<String>,
    /// `"census-top"` (default; sector-symmetry census of the subsectors of
    /// `top_sector`), `"census"` (all slots) or `"identity"`.
    #[serde(default)]
    pub router: Option<String>,
    /// Top sector (Kira id); default: the sector of the first target.
    #[serde(default)]
    pub top_sector: Option<u64>,
    /// Default `{"kind": "staircase", "s_pad": 0, "d_pad": 0, "max_growth": 3}`.
    #[serde(default)]
    pub seeding: Option<SeedingSpec>,
    #[serde(default)]
    pub truncate: Option<i16>,
    #[serde(default)]
    pub stability: Option<[u8; 3]>,
    /// `"report"` (default), `"grow"` or `"strict"`.
    #[serde(default)]
    pub master_check: Option<String>,
    #[serde(default)]
    pub settle_on_expected: bool,
    /// `"lines-rank-dots"` (default), `"lines-dots-rank"` or `"numeric"`.
    #[serde(default)]
    pub order: Option<String>,
    /// `"top"` (default; lines of `top_sector` first) or `"natural"`.
    #[serde(default)]
    pub slot_priority: Option<String>,
    #[serde(default)]
    pub tubes: Option<TubeSpec>,
    /// Lee-Pomeransky gate per sector (default true, as `BoxOptions`).
    #[serde(default)]
    pub lp_gate: Option<bool>,
    #[serde(default)]
    pub workers: Option<usize>,
    /// Seed of the modular sample.
    #[serde(default)]
    pub seed: Option<u64>,
    /// Finite-field lane: maximal primes per function, points per bunch,
    /// probe-point seed.
    #[serde(default)]
    pub max_primes: Option<usize>,
    #[serde(default)]
    pub ff_bunch: Option<usize>,
    #[serde(default)]
    pub ff_seed: Option<u64>,
    /// Default [`DEFAULT_CERTIFICATE_PRIME`].
    #[serde(default)]
    pub certificate_prime: Option<u64>,
    /// Also certify exactly over Q(free parameter) (can be expensive).
    #[serde(default)]
    pub exact_certificate: bool,
    #[serde(default)]
    pub checkpoint_dir: Option<PathBuf>,
    /// Opt-in caps; none by default.
    #[serde(default)]
    pub limits: LimitsSpec,
    /// Stop after the modular phase (masters and selection only).
    #[serde(default)]
    pub modular_only: bool,
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize)]
#[serde(tag = "kind", rename_all = "kebab-case", deny_unknown_fields)]
pub enum SeedingSpec {
    Staircase {
        s_pad: u8,
        d_pad: u8,
        max_growth: u8,
    },
    Demand {
        extra_r: u8,
        extra_s: u8,
        max_growth: u8,
    },
    KiraBox {
        r: u8,
        s: u8,
        d: u8,
    },
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct TubeSpec {
    pub min_rank: u8,
    pub halo: u8,
    #[serde(default)]
    pub subsector_halo: Option<u8>,
}

#[derive(Clone, Copy, Debug, Default, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct LimitsSpec {
    pub max_rows: Option<usize>,
    pub max_columns: Option<usize>,
    pub max_u_nnz: Option<usize>,
    pub max_l_entries: Option<usize>,
    pub max_selected: Option<usize>,
    pub max_bytes: Option<usize>,
}

/// Outcome: the report, with `status` `"ok"`, `"modular-only"`,
/// `"master-check-failed"` or `"certificate-failed"` (the last two are the
/// engine's verdicts; errors are returned as [`AppError`]).
#[derive(Clone, Debug)]
pub struct SectorBoxReduceOutcome {
    pub report: Value,
    pub status: &'static str,
}

fn parse_rational(text: &str) -> Result<Rational, AppError> {
    let parse = |part: &str| {
        part.trim()
            .parse::<i64>()
            .map_err(|_| AppError::input(format!("invalid rational {text:?}")))
    };
    let (numerator, denominator) = match text.split_once('/') {
        Some((n, d)) => (parse(n)?, parse(d)?),
        None => (parse(text)?, 1),
    };
    if denominator == 0 {
        return Err(AppError::input(format!("zero denominator in {text:?}")));
    }
    Ok(Rational::from((
        Integer::from(numerator),
        Integer::from(denominator),
    )))
}

fn load_family(path: &Path) -> Result<Arc<IntegralFamily>, AppError> {
    let source = std::fs::read_to_string(path)
        .map_err(|e| AppError::input(format!("cannot read family {}: {e}", path.display())))?;
    super::validate_ingress(&source)?;
    let prepared = prepare_input(&source, InputFormat::Toml)?;
    let (_, _, _, lowered) = lower_project(prepared)?.into_parts();
    Ok(Arc::new(lowered.into_family()))
}

fn powers<const N: usize>(values: &[i64], arity: usize) -> Result<[i16; N], AppError> {
    if values.len() != arity {
        return Err(AppError::input(format!(
            "integral {values:?} has {} indices, the family has {arity} denominators",
            values.len()
        )));
    }
    let mut out = [0i16; N];
    for (slot, &value) in values.iter().enumerate() {
        out[slot] = i16::try_from(value)
            .map_err(|_| AppError::input(format!("index {value} of {values:?} out of range")))?;
    }
    Ok(out)
}

fn sector_id(values: &[i16]) -> u64 {
    values
        .iter()
        .enumerate()
        .filter(|(_, value)| **value > 0)
        .map(|(slot, _)| 1u64 << slot)
        .sum()
}

fn engine(context: &str) -> impl Fn(SolverError) -> AppError + '_ {
    move |error| AppError::execution(format!("{context}: {error}"))
}

/// The options of the request; validates every enumerated field.
fn box_options(
    spec: &SectorBoxReduceSpec,
    top: u64,
    expected: &BTreeMap<u64, usize>,
) -> Result<BoxOptions, AppError> {
    let seeding = match spec.seeding.unwrap_or(SeedingSpec::Staircase {
        s_pad: 0,
        d_pad: 0,
        max_growth: 3,
    }) {
        SeedingSpec::Staircase {
            s_pad,
            d_pad,
            max_growth,
        } => Seeding::Staircase {
            s_pad,
            d_pad,
            max_growth,
        },
        SeedingSpec::Demand {
            extra_r,
            extra_s,
            max_growth,
        } => Seeding::Demand {
            extra_r,
            extra_s,
            max_growth,
        },
        SeedingSpec::KiraBox { r, s, d } => Seeding::KiraBox { r, s, d },
    };
    let master_check = match spec.master_check.as_deref() {
        None | Some("report") => MasterCheckMode::Report,
        Some("grow") => MasterCheckMode::Grow,
        Some("strict") => MasterCheckMode::Strict,
        Some(other) => return Err(AppError::input(format!("unknown master_check {other:?}"))),
    };
    let order = match spec.order.as_deref() {
        None | Some("lines-rank-dots") => ColumnOrder::LinesRankDots,
        Some("lines-dots-rank") => ColumnOrder::LinesDotsRank,
        Some("numeric") => ColumnOrder::Numeric,
        Some(other) => return Err(AppError::input(format!("unknown order {other:?}"))),
    };
    let slot_priority = match spec.slot_priority.as_deref() {
        None | Some("top") => SlotPriority::TopSectorFirst(top),
        Some("natural") => SlotPriority::Natural,
        Some(other) => return Err(AppError::input(format!("unknown slot_priority {other:?}"))),
    };
    let mut sample = BoxOptions::default().sample;
    if let Some(seed) = spec.seed {
        sample.seed = seed;
    }
    let workers = spec.workers.unwrap_or(1);
    if workers == 0 {
        return Err(AppError::input("workers must be positive"));
    }
    // No hidden caps: every limit is off unless the request sets it.
    let cap = |value: Option<usize>| value.unwrap_or(usize::MAX);
    let limits = BoxLimits {
        max_rows: cap(spec.limits.max_rows),
        max_columns: cap(spec.limits.max_columns),
        max_u_nnz: cap(spec.limits.max_u_nnz),
        max_l_entries: cap(spec.limits.max_l_entries),
        max_selected: cap(spec.limits.max_selected),
        max_bytes: cap(spec.limits.max_bytes),
        wall: None,
    };
    Ok(BoxOptions {
        seeding,
        order,
        sample,
        stability: spec.stability.map(|[r, s, d]| (r, s, d)),
        workers,
        lp_gate: spec.lp_gate.unwrap_or(true),
        limits,
        truncate: spec.truncate,
        slot_priority,
        master_check,
        expected_counts: (!expected.is_empty()).then(|| Arc::new(expected.clone())),
        tubes: spec.tubes.map(|tubes| TubeOptions {
            min_rank: tubes.min_rank,
            halo: tubes.halo,
            subsector_halo: tubes.subsector_halo,
        }),
        settle_on_expected: spec.settle_on_expected,
        ..Default::default()
    })
}

/// Run one `sector-box-reduce` request.
pub fn sector_box_reduce(spec: &SectorBoxReduceSpec) -> Result<SectorBoxReduceOutcome, AppError> {
    if spec.schema != SECTOR_BOX_REDUCE_SPEC_SCHEMA {
        return Err(AppError::input(format!(
            "sector-box-reduce spec schema must be {SECTOR_BOX_REDUCE_SPEC_SCHEMA}"
        )));
    }
    if spec.targets.is_empty() {
        return Err(AppError::input(
            "sector-box-reduce needs at least one target",
        ));
    }
    if spec.d_list.is_empty() && !spec.modular_only {
        return Err(AppError::input(
            "d_list is empty: give at least one dimension value or set modular_only",
        ));
    }
    let family = load_family(&spec.family)?;
    let arity = family.denominator_count();
    macro_rules! dispatch {
        ($($n:literal),*) => {
            match arity {
                $($n => run::<$n>(spec, family),)*
                _ => Err(AppError::input(format!(
                    "sector-box-reduce supports family arities {SECTOR_BOX_ARITIES:?}, not {arity}"
                ))),
            }
        };
    }
    dispatch!(3, 6, 10, 15)
}

struct Report {
    fields: serde_json::Map<String, Value>,
    path: PathBuf,
    started: Instant,
}

impl Report {
    fn set(&mut self, key: &str, value: Value) {
        self.fields.insert(key.into(), value);
    }

    /// Written atomically after every phase, so a killed run leaves the last
    /// completed phase on disk.
    fn write(&mut self) -> Result<(), AppError> {
        self.set("wall_seconds", json!(self.started.elapsed().as_secs_f64()));
        let text = serde_json::to_string_pretty(&Value::Object(self.fields.clone()))
            .map_err(|e| AppError::execution(e.to_string()))?;
        write_atomic(&self.path, format!("{text}\n").as_bytes())
    }
}

fn write_atomic(path: &Path, bytes: &[u8]) -> Result<(), AppError> {
    let part = path.with_extension("part");
    std::fs::write(&part, bytes)
        .and_then(|()| std::fs::rename(&part, path))
        .map_err(|e| AppError::execution(format!("cannot write {}: {e}", path.display())))
}

/// Failure of the modular phase: the strict master check (a verdict) or
/// any other error.
enum PhaseError {
    MasterCheck(String),
    Other(AppError),
}

/// The final modular report, the closed target list and the per-round log.
type ModularPhase<const N: usize> = (ModularReport<N>, Vec<[i16; N]>, Vec<Value>);

/// The modular phase with the derivative closure.
fn modular_phase<const N: usize>(
    session: &mut BoxLaportaSession<N>,
    targets: &[[i16; N]],
    preferred: &[[i16; N]],
    derivative_slot: Option<usize>,
    closure_rounds: usize,
) -> Result<ModularPhase<N>, PhaseError> {
    let clock = Instant::now();
    let mut final_targets = targets.to_vec();
    let mut log = Vec::new();
    let mut round = 0usize;
    loop {
        let report = session
            .extend(&final_targets, preferred)
            .map_err(|error| match error {
                SolverError::MasterCheck(message) => PhaseError::MasterCheck(message),
                other => PhaseError::Other(engine("modular phase")(other)),
            })?;
        let Some(slot) = derivative_slot else {
            return Ok((report, final_targets, log));
        };
        let known: BTreeSet<[i16; N]> = final_targets.iter().copied().collect();
        let mut added = 0usize;
        for master in &report.masters {
            if master[slot] > 0 {
                let mut seed = *master;
                seed[slot] += 1;
                if !known.contains(&seed) {
                    final_targets.push(seed);
                    added += 1;
                }
            }
        }
        log.push(
            json!({"round": round, "masters": report.masters.len(), "added": added,
                        "targets": final_targets.len(), "seconds": clock.elapsed().as_secs_f64()}),
        );
        if added == 0 {
            return Ok((report, final_targets, log));
        }
        if round == closure_rounds {
            return Err(PhaseError::Other(AppError::execution(format!(
                "the derivative closure is not closed after {closure_rounds} rounds ({added} new seeds in the last)"
            ))));
        }
        round += 1;
    }
}

fn modular_json<const N: usize>(report: &ModularReport<N>, seconds: f64) -> Value {
    let check = &report.master_check;
    json!({
        "seconds": seconds,
        "masters": report.masters.iter().map(|m| m.to_vec()).collect::<Vec<_>>(),
        "master_check": {
            "passed": check.passed(),
            "extra": check.extra.iter().map(|m| m.to_vec()).collect::<Vec<_>>(),
            "unneeded": check.unneeded.iter().map(|m| m.to_vec()).collect::<Vec<_>>(),
            "missing": check.missing, "excess": check.excess, "per_class": check.per_class,
        },
        "demand": report.demand, "zero_demand": report.zero_demand,
        "selected_rows": report.selected_rows, "rounds": report.rounds, "growth": report.growth,
        "stable": report.stable, "truncate": report.truncate,
        "seeds": report.seeds, "hull_seeds": report.hull_seeds,
        "pivot_hash": report.pivot_hash, "selected_hash": report.selected_hash,
        "master_hash": report.master_hash,
    })
}

/// File name of the solution at `d = value`: `/` -> `_`, `-` -> `m`.
pub fn solution_file_name(value: &str) -> String {
    format!(
        "solution-{}.json",
        value.trim().replace('/', "_").replace('-', "m")
    )
}

fn run<const N: usize>(
    spec: &SectorBoxReduceSpec,
    family: Arc<IntegralFamily>,
) -> Result<SectorBoxReduceOutcome, AppError> {
    let arity = family.denominator_count();
    std::fs::create_dir_all(&spec.out_dir)
        .map_err(|e| AppError::input(format!("cannot create {}: {e}", spec.out_dir.display())))?;
    let mut out = Report {
        fields: serde_json::Map::new(),
        path: spec.out_dir.join("report.json"),
        started: Instant::now(),
    };
    out.set("schema", json!(SECTOR_BOX_REDUCE_REPORT_SCHEMA));
    out.set("status", json!("running"));
    out.set("label", json!("uncertified-sector-box"));
    out.write()?;

    let targets: Vec<[i16; N]> = spec
        .targets
        .iter()
        .map(|t| powers::<N>(t, arity))
        .collect::<Result<_, _>>()?;
    let preferred: Vec<[i16; N]> = spec
        .preferred
        .iter()
        .map(|t| powers::<N>(t, arity))
        .collect::<Result<_, _>>()?;
    let mut expected = BTreeMap::new();
    for (class, count) in &spec.expected {
        let id = class
            .parse::<u64>()
            .map_err(|_| AppError::input(format!("expected class {class:?} is not a sector id")))?;
        expected.insert(id, *count);
    }
    if let Some(slot) = spec.derivative_slot
        && slot >= arity
    {
        return Err(AppError::input(format!(
            "derivative_slot {slot} is not a slot of the {arity}-denominator family"
        )));
    }
    // Parameters: the dimension takes the values of `d_list`, `fixed` the
    // others, and exactly one stays free.
    let context = family.coefficient_context();
    let names = context.parameter_names().to_vec();
    let dimension = names
        .iter()
        .find(|name| context.parameter(name).as_ref() == Some(family.dimension()))
        .cloned()
        .ok_or_else(|| AppError::input("the family dimension must be a parameter of its own"))?;
    let mut fixed_rest = Vec::new();
    for (name, value) in &spec.fixed {
        if !names.contains(name) || *name == dimension {
            return Err(AppError::input(format!(
                "fixed parameter {name:?} is not a free family parameter"
            )));
        }
        fixed_rest.push((name.clone(), parse_rational(value)?));
    }
    let free: Vec<&String> = names
        .iter()
        .filter(|name| **name != dimension && !spec.fixed.contains_key(*name))
        .collect();
    match (&spec.free, free.as_slice()) {
        _ if spec.modular_only => {}
        (Some(wanted), [only]) if wanted == *only => {}
        (None, [_]) => {}
        _ => {
            return Err(AppError::input(format!(
                "exactly one parameter must stay free (requested {:?}, left free {free:?})",
                spec.free
            )));
        }
    }
    let d_values: Vec<(String, Rational)> = spec
        .d_list
        .iter()
        .map(|value| Ok((value.clone(), parse_rational(value)?)))
        .collect::<Result<_, AppError>>()?;
    let top = spec
        .top_sector
        .unwrap_or_else(|| sector_id(&targets[0][..arity]));
    if top >= 1u64 << arity {
        return Err(AppError::input(format!(
            "top_sector {top} is not a sector of the family"
        )));
    }

    // Router.
    let clock = Instant::now();
    let census_mask: [bool; N] = std::array::from_fn(|slot| top >> slot & 1 == 1);
    let router: Arc<dyn SectorRouter<N>> = match spec.router.as_deref() {
        None | Some("census-top") => Arc::new(
            CensusRouter::discover(family.clone(), &census_mask).map_err(engine("census"))?,
        ),
        Some("census") => {
            Arc::new(CensusRouter::discover(family.clone(), &[true; N]).map_err(engine("census"))?)
        }
        Some("identity") => IdentityRouter::shared(&family).map_err(engine("router"))?,
        Some(other) => return Err(AppError::input(format!("unknown router {other:?}"))),
    };
    let router_fingerprint: String = router
        .fingerprint()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect();
    let family_block = json!({"name": family.name(), "arity": arity,
        "fingerprint": family.fingerprint(), "router_fingerprint": router_fingerprint});
    out.set("family", family_block.clone());
    out.set("census_seconds", json!(clock.elapsed().as_secs_f64()));
    // A preferred integral must route onto one integral of its representative
    // sector; one with a multi-term route is demanded as a target only.
    let (mut kept, mut demand_only) = (Vec::new(), Vec::new());
    for integral in &preferred {
        match router.route(integral) {
            Ok(route) if route.len() <= 1 => kept.push(*integral),
            Ok(route) => demand_only
                .push(json!({"integral": &integral[..arity], "route_terms": route.len()})),
            Err(error) => {
                return Err(AppError::input(format!(
                    "preferred {:?}: {error}",
                    &integral[..arity]
                )));
            }
        }
    }
    out.set("preferred_dropped", json!(demand_only));
    let options = box_options(spec, top, &expected)?;

    // Checkpoint binding: everything that determines the modular phase.
    let mut binding = serde_json::to_value(spec).map_err(|e| AppError::execution(e.to_string()))?;
    if let Value::Object(map) = &mut binding {
        for key in [
            "out_dir",
            "d_list",
            "fixed",
            "free",
            "workers",
            "max_primes",
            "ff_bunch",
            "ff_seed",
            "certificate_prime",
            "exact_certificate",
            "checkpoint_dir",
            "limits",
            "modular_only",
            "family",
        ] {
            map.remove(key);
        }
        map.insert("family_block".into(), family_block.clone());
    }
    let binding = blake3::hash(binding.to_string().as_bytes())
        .to_hex()
        .to_string();
    let checkpoint = spec.checkpoint_dir.clone();
    if let Some(dir) = &checkpoint {
        std::fs::create_dir_all(dir)
            .map_err(|e| AppError::input(format!("cannot create {}: {e}", dir.display())))?;
    }
    let restored = checkpoint.as_ref().and_then(|dir| {
        let saved: Value =
            serde_json::from_slice(&std::fs::read(dir.join("modular.json")).ok()?).ok()?;
        if saved["schema"] != CHECKPOINT_SCHEMA || saved["binding"] != binding.as_str() {
            return None;
        }
        let bytes = std::fs::read(dir.join("session.bin")).ok()?;
        let session = BoxLaportaSession::<N>::from_native_bytes(
            &bytes,
            family.clone(),
            router.clone(),
            options.clone(),
        )
        .ok()?;
        let final_targets: Vec<[i16; N]> = saved["final_targets"]
            .as_array()?
            .iter()
            .map(|t| {
                let values: Vec<i64> = serde_json::from_value(t.clone()).ok()?;
                powers::<N>(&values, arity).ok()
            })
            .collect::<Option<_>>()?;
        Some((session, final_targets, saved["modular"].clone()))
    });
    out.set("restored_from_checkpoint", json!(restored.is_some()));
    let (mut session, final_targets, modular) = match restored {
        Some(restored) => restored,
        None => {
            let mut session = BoxLaportaSession::<N>::new(
                family.clone(),
                &CutConstraint::none(arity).map_err(|e| AppError::input(e.to_string()))?,
                router.clone(),
                options.clone(),
            )
            .map_err(|e| AppError::input(format!("options: {e}")))?;
            let clock = Instant::now();
            let (report, final_targets, closure) = match modular_phase(
                &mut session,
                &targets,
                &kept,
                spec.derivative_slot,
                spec.closure_rounds.unwrap_or(8),
            ) {
                Ok(phase) => phase,
                Err(PhaseError::MasterCheck(message)) => {
                    out.set("status", json!("master-check-failed"));
                    out.set("error", json!(message));
                    out.write()?;
                    return Ok(SectorBoxReduceOutcome {
                        report: Value::Object(out.fields),
                        status: "master-check-failed",
                    });
                }
                Err(PhaseError::Other(error)) => return Err(error),
            };
            let mut modular = modular_json(&report, clock.elapsed().as_secs_f64());
            modular["closure"] = json!(closure);
            if let Some(dir) = &checkpoint {
                let bytes = session.to_native_bytes().map_err(engine("checkpoint"))?;
                write_atomic(&dir.join("session.bin"), &bytes)?;
                let saved = json!({"schema": CHECKPOINT_SCHEMA, "binding": binding,
                    "final_targets": final_targets.iter().map(|t| t[..arity].to_vec()).collect::<Vec<_>>(),
                    "modular": modular});
                write_atomic(&dir.join("modular.json"), saved.to_string().as_bytes())?;
            }
            (session, final_targets, modular)
        }
    };
    out.set("targets", json!(final_targets.len()));
    out.set("modular", modular);
    if spec.modular_only {
        out.set("status", json!("modular-only"));
        out.write()?;
        return Ok(SectorBoxReduceOutcome {
            report: Value::Object(out.fields),
            status: "modular-only",
        });
    }
    out.write()?;

    // Finite-field lane, one solution per dimension value.
    let mut ff = FfOptions::default();
    if let Some(bunch) = spec.ff_bunch {
        ff.bunch = bunch;
    }
    if let Some(max_primes) = spec.max_primes {
        ff.max_primes = max_primes;
    }
    if let Some(seed) = spec.ff_seed {
        ff.seed = seed;
    }
    if let Some(dir) = &checkpoint {
        let probes = dir.join("ff-probes");
        std::fs::create_dir_all(&probes)
            .map_err(|e| AppError::input(format!("cannot create {}: {e}", probes.display())))?;
        ff.checkpoint_dir = Some(probes);
    }
    let certificate_prime = spec.certificate_prime.unwrap_or(DEFAULT_CERTIFICATE_PRIME);
    let mut per_d = Vec::new();
    let mut status = "ok";
    for (value, d_value) in &d_values {
        let clock = Instant::now();
        let mut fixed = vec![(dimension.clone(), d_value.clone())];
        fixed.extend(fixed_rest.iter().cloned());
        let solution = session
            .materialize_ff(&final_targets, &fixed, &ff)
            .map_err(engine(&format!(
                "finite-field lane at {dimension} = {value}"
            )))?;
        let reconstruction = &solution.report.reconstruction;
        let primes: Vec<u64> = reconstruction.primes.iter().map(|p| p.prime).collect();
        let verification: Vec<u64> = reconstruction
            .verifications
            .iter()
            .map(|v| v.prime)
            .collect();
        if let Some(prime) = verification.iter().find(|prime| primes.contains(prime)) {
            return Err(AppError::execution(format!(
                "verification prime {prime} is also a reconstruction prime"
            )));
        }
        let name = solution_file_name(value);
        let ff_seconds = clock.elapsed().as_secs_f64();
        // The certificate prime must be independent of the solution. The
        // certificates that pass are written into the solution file.
        let mut passed = Vec::new();
        let certificate = if primes.contains(&certificate_prime)
            || verification.contains(&certificate_prime)
        {
            json!({"prime": certificate_prime, "ok": false,
                   "detail": "the certificate prime is a reconstruction or verification prime of this solution"})
        } else {
            let clock = Instant::now();
            let result = session.certify(&solution, Some(certificate_prime), 0x5345_4354);
            let entry = json!({"prime": certificate_prime, "ok": result.is_ok(),
                   "seconds": clock.elapsed().as_secs_f64(), "detail": format!("{result:?}")});
            passed.extend(result.ok());
            entry
        };
        let mut ok = certificate["ok"] == true;
        let mut certificates = json!({"probabilistic": certificate});
        if spec.exact_certificate {
            let clock = Instant::now();
            let result = session.certify(&solution, None, 0);
            ok &= result.is_ok();
            certificates["exact"] = json!({"ok": result.is_ok(),
                "seconds": clock.elapsed().as_secs_f64(), "detail": format!("{result:?}")});
            passed.extend(result.ok());
        }
        let extra = json!({"producer": "rustred sector-box-reduce", "d_fixed": value});
        let text = solution
            .to_harness_json(&session.harness_family(), &extra, &passed)
            .map_err(engine(&format!("solution file at {dimension} = {value}")))?;
        write_atomic(&spec.out_dir.join(&name), text.as_bytes())?;
        if !ok {
            status = "certificate-failed";
        }
        per_d.push(json!({
            "d": value, "file": name, "blake3": blake3::hash(text.as_bytes()).to_hex().to_string(),
            "ff_seconds": ff_seconds, "free": solution.free,
            "entries": solution.report.entries, "nonzero": solution.report.nonzero_entries,
            "max_degrees": [reconstruction.max_numerator_degree, reconstruction.max_denominator_degree],
            "resumed_probes": solution.report.resumed_probes,
            "reconstruction_primes": primes, "verification_primes": verification,
            "selected_hash": solution.provenance.selected_hash,
            "master_hash": solution.provenance.master_hash,
            "certificates": certificates, "ok": ok,
        }));
        out.set("per_d", json!(per_d));
        out.write()?;
    }
    out.set("status", json!(status));
    out.write()?;
    Ok(SectorBoxReduceOutcome {
        report: Value::Object(out.fields),
        status,
    })
}
