//! The checker ENTRY POINTS with a mode switch (DEV-17 REVISED) and the TRIAGE of the `Both` mode.
//!
//! - [`IdentityMode::Independent`] (default): the certificate of record (`rustred_campaign_core::independent_algebra`).
//! - [`IdentityMode::Symbolica`]: the SAME domain logic on [`SymbolicaAlgebra`]; diagnostic (a `replay` record of
//!   this mode is `ReplayLevel::Diagnostic` and never carries the level).
//! - [`IdentityMode::Both`]: the CERTIFICATE run (recorded through the core's `*_traced` entry points, so its record is
//!   the record of record) and the Symbolica run on the same rule, family text, served keys, options, primes and random
//!   stream; then THREE checks, in this order (rows4):
//!   1. the two traces of intermediate quantities ([`Stage`]) are compared: the first difference is a [`Triage::Differ`]
//!      with the outlier named by the [`crate::referee`] (a third implementation sharing nothing with either side) or
//!      an explicit [`UnresolvedClass`] with its reason;
//!   2. when the traces agree, the shared DOMAIN LOGIC is re-decided by the [`oracle`] at every integer key (its own
//!      family reader, IBP derivation, Lee test, applicability, rule vector, membership and controls): a difference is
//!      [`Triage::SharedLogicDiffers`] ("shared logic differs from the oracle"), distinct from the verdicts below;
//!   3. when everything agrees: [`Triage::AgreePass`]; [`Triage::AgreeFail`] (both algebras and the oracle agree the
//!      rule is NOT derived from its recorded seeds, and the one-step probe does not derive it either: not certified by
//!      its seeds); [`Triage::RecordInconsistent`] (the same failure at the recorded seeds, but the probe seeds derive
//!      the identity: the rule holds, its RECORD is inconsistent); [`Triage::AgreeOther`] (same other verdict).
//!
//! In `Both`, anything but an agreement BLOCKS the level of the certificate record and a `Differ` raises a typed
//! [`TriageFinding`].

pub mod oracle;

use std::time::Instant;

use rustred_campaign_core::ibp_rows::{
    PointSpec, ReplayLevel, ReplayPolicy, ReplayRecord, RowsOptions, RowsRule, RowsVerdict,
    Unsupported, Verdict, identity_test, identity_test_traced, identity_test_with, replay_evidence,
    replay_evidence_traced, replay_evidence_with,
};
use rustred_campaign_core::replay_base::algebra::{CheckAlgebra, Recorder, Stage};
use rustred_campaign_core::replay_base::family::Family;
use serde::Serialize;

use crate::backend::SymbolicaAlgebra;
use crate::referee::{Referee, referee_covers};
use crate::wrap::{Event, Planted, Recording, recording};
use oracle::{OracleRun, SharedDiff};

/// Which algebra runs the identity test.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize)]
pub enum IdentityMode {
    /// the certificate of record
    #[default]
    Independent,
    /// Symbolica / numerica (diagnostic)
    Symbolica,
    /// both, with the triage report
    Both,
}

/// Which side a differing quantity is attributed to.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
pub enum Outlier {
    /// the referee agrees with Symbolica: the independent algebra is wrong at that quantity
    Independent,
    /// the referee agrees with the independent algebra: Symbolica is wrong at that quantity
    Symbolica,
    /// see [`Difference::unresolved`] for the class and its reason
    Unresolved,
}

/// Why a difference is not attributed to a side (rows4: every `Unresolved` carries one, printed in `note`).
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub enum UnresolvedClass {
    /// a coefficient is a pole on one side only: the text is a removable 0/0 or has a common factor; Symbolica's
    /// `to_rational_polynomial` cancels it, the independent reader (and the referee) evaluate the text AS WRITTEN. Both
    /// readings are legitimate; the certificate refuses the key (re-export the coefficient without the common factor)
    TextPole,
    /// the independent STRICT grammar refuses a coefficient text that Symbolica reads (e.g. `a/b*c`, audit M1): a
    /// policy difference, not an arithmetic error; refused texts are never guessed (re-export it in the grammar)
    GrammarRefusal,
    /// the referee did not produce a trace (its own refusal, a size bound): the reason is in `note`
    RefereeNotRun,
    /// the referee's trace is not aligned with the two runs before the difference (it decided differently earlier)
    RefereeMisaligned,
    /// the referee reproduces neither value
    RefereeAgreesWithNeither,
}

/// The first differing intermediate quantity of the two runs.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct Difference {
    /// index in the traces
    pub index: usize,
    pub stage: Stage,
    /// where (cell / row / coefficient ...), from the independent trace
    pub at: String,
    pub independent: String,
    pub symbolica: String,
    pub outlier: Outlier,
    /// the referee's value at that place (when it ran)
    pub referee: Option<String>,
    /// set iff `outlier == Unresolved`: why
    pub unresolved: Option<UnresolvedClass>,
    pub note: String,
}

/// The outcome classes of the triage.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub enum Triage {
    /// both backends pass with identical traces
    AgreePass,
    /// both backends fail at the same place with identical traces, the oracle agrees, and the one-step probe seeds do
    /// not derive the identity either: the rule is not certified by its (recorded or probe) seeds
    AgreeFail {
        point: usize,
        prime: u64,
        column: String,
    },
    /// the same failure at the RECORDED seeds, but the identity is in the span of the rows of the probe seeds (every
    /// seed whose rows reach the target) at the failing key: the rule holds there; its record (seeds) is inconsistent
    RecordInconsistent { point: usize, key: Vec<i64> },
    /// the two backends agree, but the shared domain logic disagrees with the oracle's brute-force decision
    SharedLogicDiffers(SharedDiff),
    /// identical traces and the same non-pass, non-fail verdict
    AgreeOther { verdict: String },
    /// an algebra disagreement (localised)
    Differ(Difference),
    /// the Symbolica backend could not run (no licence, a budget, a text it does not support)
    SymbolicaUnsupported { reason: String },
}

/// The triage report of one rule.
#[derive(Clone, Debug, Serialize)]
pub struct TriageReport {
    pub triage: Triage,
    pub independent_verdict: String,
    pub symbolica_verdict: String,
    pub trace_len_independent: usize,
    pub trace_len_symbolica: usize,
    pub seconds_independent: f64,
    pub seconds_symbolica: f64,
    pub seconds_referee: f64,
    /// the oracle's run (keys and cells re-decided, what it skipped and why); `None` when the traces differed
    pub oracle: Option<OracleRun>,
}

/// A `Differ` outcome of the `Both` mode, raised with the record (it blocks the level).
#[derive(Clone, Debug, Serialize)]
pub struct TriageFinding {
    pub rule_index: Option<i64>,
    pub target: String,
    pub difference: Difference,
}

/// The result of [`identity_test_mode`].
#[derive(Clone, Debug, Serialize)]
pub struct ModeVerdict {
    pub mode: IdentityMode,
    /// the certificate run (modes Independent and Both)
    pub independent: Option<RowsVerdict>,
    /// the diagnostic run (modes Symbolica and Both)
    pub symbolica: Option<RowsVerdict>,
    pub triage: Option<TriageReport>,
}

/// The result of [`replay_evidence_mode`].
#[derive(Clone, Debug, Serialize)]
pub struct ModeRecord {
    pub mode: IdentityMode,
    /// the record of record: the independent run (modes Independent and Both); in Both its level is blocked by a
    /// `Differ` or a Symbolica run that could not be made
    pub independent: Option<ReplayRecord>,
    /// the diagnostic record (level `Diagnostic`)
    pub symbolica: Option<ReplayRecord>,
    pub triage: Option<TriageReport>,
    pub finding: Option<TriageFinding>,
}

/// A backend whose trace can be taken (the recording and planted wrappers).
pub trait Traced: CheckAlgebra {
    fn take_events(&self) -> Vec<Event>;
}

impl<A: CheckAlgebra> Traced for Recording<A> {
    fn take_events(&self) -> Vec<Event> {
        Recording::take_events(self)
    }
}

impl<A: CheckAlgebra> Traced for Planted<A> {
    fn take_events(&self) -> Vec<Event> {
        Planted::take_events(self)
    }
}

fn verdict_text(v: &Verdict) -> String {
    format!("{v:?}")
}

/// One run of the triage: its verdict, its trace, its seconds.
type Run = (RowsVerdict, Vec<Event>, f64);

fn traced_identity<A: Traced>(
    alg: &A,
    family_text: &str,
    rule: &RowsRule,
    points: &[PointSpec],
    opts: &RowsOptions,
) -> Result<Run, String> {
    let t0 = Instant::now();
    let fam = Family::parse_with(alg, family_text)?;
    let v = identity_test_with(alg, &fam, rule, points, opts);
    Ok((v, alg.take_events(), t0.elapsed().as_secs_f64()))
}

/// The CERTIFICATE run, recorded (the core's concrete traced entry points: its verdict is a certificate verdict).
fn certificate_identity(
    family_text: &str,
    rule: &RowsRule,
    points: &[PointSpec],
    opts: &RowsOptions,
) -> Result<Run, String> {
    let t0 = Instant::now();
    let rec = Recorder::new();
    let fam = Family::parse_traced(family_text, &rec)?;
    let v = identity_test_traced(&fam, rule, points, opts, &rec);
    Ok((v, rec.take(), t0.elapsed().as_secs_f64()))
}

fn is_syntax(v: &Verdict) -> bool {
    matches!(v, Verdict::Unsupported(Unsupported::Syntax(_)))
}

/// Compare two traces; the first differing entry (or a length difference).
fn first_difference(a: &[Event], b: &[Event]) -> Option<(usize, Stage, String, String, String)> {
    for (i, (x, y)) in a.iter().zip(b).enumerate() {
        if x.stage != y.stage || x.at != y.at {
            return Some((
                i,
                x.stage,
                format!("{} (symbolica at: {})", x.at, y.at),
                format!("{} = {}", x.at, x.value),
                format!("{} = {}", y.at, y.value),
            ));
        }
        if x.value != y.value {
            return Some((i, x.stage, x.at.clone(), x.value.clone(), y.value.clone()));
        }
    }
    if a.len() != b.len() {
        let i = a.len().min(b.len());
        let pick = |t: &[Event]| {
            t.get(i).map_or("(trace ended)".to_string(), |e| {
                format!("{:?} {} = {}", e.stage, e.at, e.value)
            })
        };
        let stage = a.get(i).or(b.get(i)).map_or(Stage::Verdict, |e| e.stage);
        return Some((i, stage, "trace length".into(), pick(a), pick(b)));
    }
    None
}

/// The triage of two recorded runs (`referee_trace` computes the referee's trace on demand).
pub fn triage_traces(
    ind: (&RowsVerdict, &[Event]),
    sym: (&RowsVerdict, &[Event]),
    referee_trace: &mut dyn FnMut() -> Option<Vec<Event>>,
) -> (Triage, f64) {
    let mut seconds_referee = 0.0;
    let triage = match first_difference(ind.1, sym.1) {
        None => match &ind.0.verdict {
            Verdict::Pass => Triage::AgreePass,
            Verdict::Fail {
                point,
                prime,
                column,
                ..
            } => Triage::AgreeFail {
                point: *point,
                prime: *prime,
                column: column.clone(),
            },
            v => Triage::AgreeOther {
                verdict: verdict_text(v),
            },
        },
        Some((index, stage, at, iv, sv)) => {
            let mut note = String::new();
            let mut unresolved = None;
            if stage == Stage::Coefficient && (iv.contains("pole") ^ sv.contains("pole")) {
                note = "pole on one side only: Symbolica's to_rational_polynomial cancels common factors of numerator and denominator, the independent reader (and the referee) evaluate the text as written".into();
                unresolved = Some(UnresolvedClass::TextPole);
            } else if is_syntax(&ind.0.verdict) != is_syntax(&sym.0.verdict)
                && ind.1.get(index).map(|e| (e.stage, &e.at))
                    != sym.1.get(index).map(|e| (e.stage, &e.at))
            {
                // the traces diverge in PLACE (one side produced no value there: it refused the text), not in value
                note = format!(
                    "one side refuses a text the other reads (independent {}, symbolica {}): the strict grammar of the certificate refuses it; never guessed",
                    ind.0.verdict.kind(),
                    sym.0.verdict.kind()
                );
                unresolved = Some(UnresolvedClass::GrammarRefusal);
            }
            let (outlier, referee) = if unresolved.is_some() {
                (Outlier::Unresolved, None)
            } else if referee_covers(stage) {
                let t0 = Instant::now();
                let r = referee_trace();
                seconds_referee = t0.elapsed().as_secs_f64();
                match r {
                    Some(rt) => {
                        let same_prefix = rt.len() >= index
                            && rt[..index]
                                .iter()
                                .zip(&ind.1[..index])
                                .all(|(x, y)| x.stage == y.stage && x.at == y.at);
                        // the referee's entry at `index` (or the end of its trace) against each side's
                        let same = |t: &[Event]| match (rt.get(index), t.get(index)) {
                            (Some(e), Some(x)) => e == x,
                            (None, None) => true,
                            _ => false,
                        };
                        let shown = rt
                            .get(index)
                            .map_or("(trace ended)".to_string(), |e| e.value.clone());
                        if !same_prefix {
                            unresolved = Some(UnresolvedClass::RefereeMisaligned);
                            note = format!(
                                "the referee's trace ({} entries) is not aligned with the runs before entry {index}",
                                rt.len()
                            );
                            (Outlier::Unresolved, None)
                        } else if same(ind.1) && !same(sym.1) {
                            (Outlier::Symbolica, Some(shown))
                        } else if same(sym.1) && !same(ind.1) {
                            (Outlier::Independent, Some(shown))
                        } else {
                            unresolved = Some(UnresolvedClass::RefereeAgreesWithNeither);
                            note = "the referee reproduces neither value".into();
                            (Outlier::Unresolved, Some(shown))
                        }
                    }
                    None => {
                        unresolved = Some(UnresolvedClass::RefereeNotRun);
                        note = "the referee did not produce a trace (family or rule refused by it)"
                            .into();
                        (Outlier::Unresolved, None)
                    }
                }
            } else {
                unresolved = Some(UnresolvedClass::RefereeNotRun);
                note = format!("the referee does not cover stage {stage:?}");
                (Outlier::Unresolved, None)
            };
            Triage::Differ(Difference {
                index,
                stage,
                at,
                independent: iv,
                symbolica: sv,
                outlier,
                referee,
                unresolved,
                note,
            })
        }
    };
    (triage, seconds_referee)
}

fn unsupported_report(reason: String, ind: Option<&RowsVerdict>, seconds: f64) -> TriageReport {
    TriageReport {
        triage: Triage::SymbolicaUnsupported { reason },
        independent_verdict: ind.map_or(String::new(), |v| verdict_text(&v.verdict)),
        symbolica_verdict: String::new(),
        trace_len_independent: 0,
        trace_len_symbolica: 0,
        seconds_independent: seconds,
        seconds_symbolica: 0.0,
        seconds_referee: 0.0,
        oracle: None,
    }
}

/// Steps 2 and 3 of the `Both` mode on an agreement of the traces: the shared-logic oracle at every integer key; on an
/// agreed failure the one-step probe (record inconsistent vs not certified by its seeds).
fn oracle_step(
    family_text: &str,
    rule: &RowsRule,
    v: &RowsVerdict,
    triage: Triage,
) -> (Triage, Option<OracleRun>) {
    if matches!(triage, Triage::Differ(_)) {
        return (triage, None);
    }
    let run = oracle::check(family_text, rule, v);
    if let Some(d) = run.diffs.first() {
        return (Triage::SharedLogicDiffers(d.clone()), Some(run));
    }
    let triage = match triage {
        Triage::AgreeFail {
            point,
            prime,
            column,
        } => match oracle::probe(family_text, rule, v) {
            Some(key) => Triage::RecordInconsistent { point, key },
            None => Triage::AgreeFail {
                point,
                prime,
                column,
            },
        },
        t => t,
    };
    (triage, Some(run))
}

/// The triage of two finished runs (`referee` computes the referee's trace on demand).
fn triage_runs(
    family_text: &str,
    rule: &RowsRule,
    ind: &Run,
    sym: &Run,
    referee: &mut dyn FnMut() -> Option<Vec<Event>>,
) -> TriageReport {
    let (triage, sr) = triage_traces((&ind.0, &ind.1), (&sym.0, &sym.1), referee);
    let (triage, oracle) = oracle_step(family_text, rule, &ind.0, triage);
    TriageReport {
        triage,
        independent_verdict: verdict_text(&ind.0.verdict),
        symbolica_verdict: verdict_text(&sym.0.verdict),
        trace_len_independent: ind.1.len(),
        trace_len_symbolica: sym.1.len(),
        seconds_independent: ind.2,
        seconds_symbolica: sym.2,
        seconds_referee: sr,
        oracle,
    }
}

/// The triage of one rule with explicit backends (the entry point of the planted-defect tests): `ind` and `sym`
/// must be recording backends (both runs are diagnostic runs of the generic entry points); the referee runs on demand.
/// Returns both verdicts and the report.
pub fn triage_identity<I: Traced, S: Traced>(
    ind: &I,
    sym: &S,
    family_text: &str,
    rule: &RowsRule,
    points: &[PointSpec],
    opts: &RowsOptions,
) -> Result<(RowsVerdict, RowsVerdict, TriageReport), String> {
    let ri = traced_identity(ind, family_text, rule, points, opts)?;
    let rs = traced_identity(sym, family_text, rule, points, opts)?;
    let mut referee = || {
        let r = recording(Referee);
        traced_identity(&r, family_text, rule, points, opts)
            .ok()
            .map(|x| x.1)
    };
    let report = triage_runs(family_text, rule, &ri, &rs, &mut referee);
    Ok((ri.0, rs.0, report))
}

/// The A2 identity test of one rule in the given mode (the family is given as TEXT: each backend parses it).
pub fn identity_test_mode(
    mode: IdentityMode,
    family_text: &str,
    rule: &RowsRule,
    points: &[PointSpec],
    opts: &RowsOptions,
) -> Result<ModeVerdict, String> {
    match mode {
        IdentityMode::Independent => {
            let fam = Family::parse(family_text)?;
            Ok(ModeVerdict {
                mode,
                independent: Some(identity_test(&fam, rule, points, opts)),
                symbolica: None,
                triage: None,
            })
        }
        IdentityMode::Symbolica => {
            let s = SymbolicaAlgebra::new()?;
            let fam = Family::parse_with(&s, family_text)?;
            Ok(ModeVerdict {
                mode,
                independent: None,
                symbolica: Some(identity_test_with(&s, &fam, rule, points, opts)),
                triage: None,
            })
        }
        IdentityMode::Both => {
            let ri = certificate_identity(family_text, rule, points, opts)?;
            let sym = match SymbolicaAlgebra::new() {
                Ok(s) => recording(s),
                Err(reason) => {
                    let rep = unsupported_report(reason, Some(&ri.0), ri.2);
                    return Ok(ModeVerdict {
                        mode,
                        independent: Some(ri.0),
                        symbolica: None,
                        triage: Some(rep),
                    });
                }
            };
            let rs = traced_identity(&sym, family_text, rule, points, opts)?;
            let mut referee = || {
                let r = recording(Referee);
                traced_identity(&r, family_text, rule, points, opts)
                    .ok()
                    .map(|x| x.1)
            };
            let rep = triage_runs(family_text, rule, &ri, &rs, &mut referee);
            Ok(ModeVerdict {
                mode,
                independent: Some(ri.0),
                symbolica: Some(rs.0),
                triage: Some(rep),
            })
        }
    }
}

/// A diagnostic `replay` run of a recording backend, its trace and seconds.
fn traced_replay<A: Traced>(
    alg: &A,
    family_text: &str,
    rule: &RowsRule,
    served_keys: &[Vec<i64>],
    policy: &ReplayPolicy,
    opts: &RowsOptions,
) -> Result<(ReplayRecord, Vec<Event>, f64), String> {
    let t0 = Instant::now();
    let fam = Family::parse_with(alg, family_text)?;
    let r = replay_evidence_with(alg, &fam, rule, served_keys, policy, opts);
    Ok((r, alg.take_events(), t0.elapsed().as_secs_f64()))
}

/// The `replay` evidence of one rule in the given mode. In `Both` the certificate run is recorded through the core's
/// traced entry point (its record is the record of record); the Symbolica and referee runs use the SAME nonce (the
/// certificate run's: derived from the beacon, or its OS draw without one), so all random streams coincide; anything
/// but an agreement blocks the level of the certificate record (`Differ` raises a [`TriageFinding`]).
pub fn replay_evidence_mode(
    mode: IdentityMode,
    family_text: &str,
    rule: &RowsRule,
    served_keys: &[Vec<i64>],
    policy: &ReplayPolicy,
    opts: &RowsOptions,
) -> Result<ModeRecord, String> {
    match mode {
        IdentityMode::Independent => {
            let fam = Family::parse(family_text)?;
            Ok(ModeRecord {
                mode,
                independent: Some(replay_evidence(&fam, rule, served_keys, policy, opts)),
                symbolica: None,
                triage: None,
                finding: None,
            })
        }
        IdentityMode::Symbolica => {
            let s = SymbolicaAlgebra::new()?;
            let fam = Family::parse_with(&s, family_text)?;
            let r = replay_evidence_with(&s, &fam, rule, served_keys, policy, opts);
            Ok(ModeRecord {
                mode,
                independent: None,
                symbolica: Some(r),
                triage: None,
                finding: None,
            })
        }
        IdentityMode::Both => {
            let t0 = Instant::now();
            let rec = Recorder::new();
            let fam = Family::parse_traced(family_text, &rec)?;
            let ri = replay_evidence_traced(&fam, rule, served_keys, policy, opts, &rec);
            let ind = (ri, rec.take(), t0.elapsed().as_secs_f64());
            match SymbolicaAlgebra::new() {
                Ok(s) => replay_triage_with(
                    ind,
                    &recording(s),
                    family_text,
                    rule,
                    served_keys,
                    policy,
                    opts,
                ),
                Err(reason) => {
                    let (mut ri, _, secs) = ind;
                    block(
                        &mut ri,
                        format!("triage: Symbolica run not made ({reason})"),
                    );
                    Ok(ModeRecord {
                        mode,
                        triage: Some(unsupported_report(reason, Some(&ri.identity), secs)),
                        independent: Some(ri),
                        symbolica: None,
                        finding: None,
                    })
                }
            }
        }
    }
}

/// The `Both` mode of [`replay_evidence_mode`] with explicit recording backends (the planted-defect tests inject
/// theirs; both runs are then diagnostic, so the "independent" record never carries the level): the second and the
/// referee run use the nonce of the first; the identity traces are triaged; anything but an agreement blocks the level
/// of the record of `ind` (a `Differ` also raises a [`TriageFinding`]).
pub fn replay_evidence_triage<I: Traced, S: Traced>(
    ind: &I,
    sym: &S,
    family_text: &str,
    rule: &RowsRule,
    served_keys: &[Vec<i64>],
    policy: &ReplayPolicy,
    opts: &RowsOptions,
) -> Result<ModeRecord, String> {
    let first = traced_replay(ind, family_text, rule, served_keys, policy, opts)?;
    replay_triage_with(first, sym, family_text, rule, served_keys, policy, opts)
}

fn replay_triage_with<S: Traced>(
    ind: (ReplayRecord, Vec<Event>, f64),
    sym: &S,
    family_text: &str,
    rule: &RowsRule,
    served_keys: &[Vec<i64>],
    policy: &ReplayPolicy,
    opts: &RowsOptions,
) -> Result<ModeRecord, String> {
    let (mut ri, ti, si) = ind;
    // the same random streams: the nonce of the first run (a caller nonce for the diagnostic runs)
    let same = RowsOptions {
        nonce: Some(ri.nonce),
        ..opts.clone()
    };
    let (rs, ts, ss) = traced_replay(sym, family_text, rule, served_keys, policy, &same)?;
    let mut referee = || {
        traced_replay(
            &recording(Referee),
            family_text,
            rule,
            served_keys,
            policy,
            &same,
        )
        .ok()
        .map(|x| x.1)
    };
    let run_i: Run = (ri.identity.clone(), ti, si);
    let run_s: Run = (rs.identity.clone(), ts, ss);
    let report = triage_runs(family_text, rule, &run_i, &run_s, &mut referee);
    let finding = match &report.triage {
        Triage::Differ(d) => Some(TriageFinding {
            rule_index: rule.index,
            target: rule.target_text.clone(),
            difference: d.clone(),
        }),
        _ => None,
    };
    match &report.triage {
        Triage::AgreePass | Triage::AgreeFail { .. } | Triage::AgreeOther { .. } => {}
        Triage::RecordInconsistent { key, .. } => block(
            &mut ri,
            format!(
                "triage RecordInconsistent: the identity holds at {key:?} but not from its recorded seeds"
            ),
        ),
        Triage::SharedLogicDiffers(d) => block(
            &mut ri,
            format!(
                "triage: shared logic differs from the oracle at {:?} ({})",
                d.key, d.what
            ),
        ),
        Triage::Differ(d) => block(
            &mut ri,
            format!(
                "triage Differ at {:?} {} (outlier {:?})",
                d.stage, d.at, d.outlier
            ),
        ),
        Triage::SymbolicaUnsupported { reason } => {
            block(&mut ri, format!("triage: Symbolica unsupported ({reason})"))
        }
    }
    Ok(ModeRecord {
        mode: IdentityMode::Both,
        independent: Some(ri),
        symbolica: Some(rs),
        triage: Some(report),
        finding,
    })
}

/// Block the level of a record (the triage could not confirm it).
fn block(r: &mut ReplayRecord, reason: String) {
    r.level = match std::mem::replace(&mut r.level, ReplayLevel::Established) {
        ReplayLevel::NotEstablished { mut reasons } => {
            reasons.push(reason);
            ReplayLevel::NotEstablished { reasons }
        }
        ReplayLevel::Established => ReplayLevel::NotEstablished {
            reasons: vec![reason],
        },
        other => ReplayLevel::NotEstablished {
            reasons: vec![format!("{other:?}"), reason],
        },
    };
}
