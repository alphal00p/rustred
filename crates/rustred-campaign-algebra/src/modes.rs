//! The checker ENTRY POINTS with a mode switch (DEV-17 REVISED) and the TRIAGE of the `Both` mode.
//!
//! - [`IdentityMode::Independent`] (default): the certificate of record (`rustred_campaign_core::independent_algebra`).
//! - [`IdentityMode::Symbolica`]: the SAME domain logic on [`SymbolicaAlgebra`]; diagnostic (a `replay` record of
//!   this mode is `ReplayLevel::Diagnostic` and never carries the level).
//! - [`IdentityMode::Both`]: both backends on the same rule, family text, served keys, options, primes and random
//!   stream (one OS nonce for both), each RECORDING its trace of intermediate quantities (`Stage`); the
//!   [`TriageReport`] says [`Triage::AgreePass`], [`Triage::AgreeFail`] (the rule is really not certified by its
//!   seeds: failing point / prime / witness column), [`Triage::AgreeOther`] (same non-pass verdict, e.g. both
//!   unsupported), or [`Triage::Differ`] at the FIRST differing quantity (stage, place, both values) with the outlier
//!   when the cheap third opinion ([`crate::referee`]) covers that stage. In `Both`, a `Differ` BLOCKS the level of
//!   the independent record and raises a typed [`TriageFinding`].

use std::time::Instant;

use rustred_campaign_core::ibp_rows::{
    NonceSource, PointSpec, ReplayLevel, ReplayPolicy, ReplayRecord, RowsOptions, RowsRule,
    RowsVerdict, Verdict, identity_test_with, os_nonce, replay_evidence_drawn,
};
use rustred_campaign_core::independent_algebra::Independent;
use rustred_campaign_core::replay_base::algebra::{CheckAlgebra, Stage};
use rustred_campaign_core::replay_base::family::Family;
use serde::Serialize;

use crate::backend::SymbolicaAlgebra;
use crate::referee::{Referee, referee_covers};
use crate::wrap::{Event, Planted, Recording, recording};

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
    /// no third opinion for that stage, the referee could not run, or it agrees with neither
    Unresolved,
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
    pub note: String,
}

/// The outcome classes of the triage.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub enum Triage {
    /// both backends pass with identical traces
    AgreePass,
    /// both backends fail at the same place with identical traces: the rule is not certified by its seeds
    AgreeFail {
        point: usize,
        prime: u64,
        column: String,
    },
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

fn traced_identity<A: Traced>(
    alg: &A,
    family_text: &str,
    rule: &RowsRule,
    points: &[PointSpec],
    opts: &RowsOptions,
) -> Result<(RowsVerdict, Vec<Event>, f64), String> {
    let t0 = Instant::now();
    let fam = Family::parse_with(alg, family_text)?;
    let v = identity_test_with(alg, &fam, rule, points, opts);
    Ok((v, alg.take_events(), t0.elapsed().as_secs_f64()))
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
            if stage == Stage::Coefficient && (iv.starts_with("pole") ^ sv.starts_with("pole")) {
                note = "pole on one side only: Symbolica's to_rational_polynomial cancels common factors of numerator and denominator, the independent reader evaluates the text as written".into();
            }
            let (outlier, referee) = if referee_covers(stage) {
                let t0 = Instant::now();
                let r = referee_trace();
                seconds_referee = t0.elapsed().as_secs_f64();
                match r {
                    Some(rt) => {
                        let same_prefix = rt.len() > index
                            && rt[..index]
                                .iter()
                                .zip(&ind.1[..index])
                                .all(|(x, y)| x.stage == y.stage && x.at == y.at);
                        match rt.get(index).filter(|_| same_prefix) {
                            Some(e) if e.value == iv => (Outlier::Symbolica, Some(e.value.clone())),
                            Some(e) if e.value == sv => {
                                (Outlier::Independent, Some(e.value.clone()))
                            }
                            Some(e) => (Outlier::Unresolved, Some(e.value.clone())),
                            None => (Outlier::Unresolved, None),
                        }
                    }
                    None => (Outlier::Unresolved, None),
                }
            } else {
                if note.is_empty() {
                    note = format!("no third opinion for stage {stage:?}");
                }
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
    }
}

/// The triage of one rule with explicit backends (the entry point of the planted-defect tests): `ind` and `sym`
/// must be recording backends; the referee runs on demand. Returns both verdicts and the report.
pub fn triage_identity<I: Traced, S: Traced>(
    ind: &I,
    sym: &S,
    family_text: &str,
    rule: &RowsRule,
    points: &[PointSpec],
    opts: &RowsOptions,
) -> Result<(RowsVerdict, RowsVerdict, TriageReport), String> {
    let (vi, ti, si) = traced_identity(ind, family_text, rule, points, opts)?;
    let (vs, ts, ss) = traced_identity(sym, family_text, rule, points, opts)?;
    let mut referee = || {
        let r = recording(Referee);
        traced_identity(&r, family_text, rule, points, opts)
            .ok()
            .map(|x| x.1)
    };
    let (triage, sr) = triage_traces((&vi, &ti), (&vs, &ts), &mut referee);
    let report = TriageReport {
        triage,
        independent_verdict: verdict_text(&vi.verdict),
        symbolica_verdict: verdict_text(&vs.verdict),
        trace_len_independent: ti.len(),
        trace_len_symbolica: ts.len(),
        seconds_independent: si,
        seconds_symbolica: ss,
        seconds_referee: sr,
    };
    Ok((vi, vs, report))
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
                independent: Some(identity_test_with(&Independent, &fam, rule, points, opts)),
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
            let ind = recording(Independent);
            let sym = match SymbolicaAlgebra::new() {
                Ok(s) => recording(s),
                Err(reason) => {
                    let t0 = Instant::now();
                    let fam = Family::parse(family_text)?;
                    let v = identity_test_with(&Independent, &fam, rule, points, opts);
                    let rep = unsupported_report(reason, Some(&v), t0.elapsed().as_secs_f64());
                    return Ok(ModeVerdict {
                        mode,
                        independent: Some(v),
                        symbolica: None,
                        triage: Some(rep),
                    });
                }
            };
            let (vi, vs, rep) = triage_identity(&ind, &sym, family_text, rule, points, opts)?;
            Ok(ModeVerdict {
                mode,
                independent: Some(vi),
                symbolica: Some(vs),
                triage: Some(rep),
            })
        }
    }
}

/// The `replay` evidence of one rule in the given mode. In `Both` the two runs share ONE OS nonce (so the
/// independent record keeps `NonceSource::Os`), their identity traces are triaged, and anything but an agreement
/// blocks the level of the independent record (`Differ` raises a [`TriageFinding`]).
pub fn replay_evidence_mode(
    mode: IdentityMode,
    family_text: &str,
    rule: &RowsRule,
    served_keys: &[Vec<i64>],
    policy: &ReplayPolicy,
    opts: &RowsOptions,
) -> Result<ModeRecord, String> {
    let drawn = match opts.nonce {
        Some(x) => (x, NonceSource::Caller),
        None => os_nonce(),
    };
    match mode {
        IdentityMode::Independent => {
            let fam = Family::parse(family_text)?;
            let r =
                replay_evidence_drawn(&Independent, &fam, rule, served_keys, policy, opts, drawn);
            Ok(ModeRecord {
                mode,
                independent: Some(r),
                symbolica: None,
                triage: None,
                finding: None,
            })
        }
        IdentityMode::Symbolica => {
            let s = SymbolicaAlgebra::new()?;
            let fam = Family::parse_with(&s, family_text)?;
            let r = replay_evidence_drawn(&s, &fam, rule, served_keys, policy, opts, drawn);
            Ok(ModeRecord {
                mode,
                independent: None,
                symbolica: Some(r),
                triage: None,
                finding: None,
            })
        }
        IdentityMode::Both => match SymbolicaAlgebra::new() {
            Ok(s) => replay_evidence_triage(
                &recording(Independent),
                &recording(s),
                family_text,
                rule,
                served_keys,
                policy,
                opts,
                drawn,
            ),
            Err(reason) => {
                let t0 = Instant::now();
                let fam = Family::parse(family_text)?;
                let mut ri = replay_evidence_drawn(
                    &Independent,
                    &fam,
                    rule,
                    served_keys,
                    policy,
                    opts,
                    drawn,
                );
                block(
                    &mut ri,
                    format!("triage: Symbolica run not made ({reason})"),
                );
                Ok(ModeRecord {
                    mode,
                    triage: Some(unsupported_report(
                        reason,
                        Some(&ri.identity),
                        t0.elapsed().as_secs_f64(),
                    )),
                    independent: Some(ri),
                    symbolica: None,
                    finding: None,
                })
            }
        },
    }
}

/// The `Both` mode of [`replay_evidence_mode`] with explicit recording backends (the planted-defect tests inject
/// theirs): one nonce for both runs; the identity traces are triaged; anything but an agreement blocks the level of
/// the record of `ind` (a `Differ` also raises a [`TriageFinding`]).
#[allow(clippy::too_many_arguments)]
pub fn replay_evidence_triage<I: Traced, S: Traced>(
    ind: &I,
    sym: &S,
    family_text: &str,
    rule: &RowsRule,
    served_keys: &[Vec<i64>],
    policy: &ReplayPolicy,
    opts: &RowsOptions,
    drawn: (u64, NonceSource),
) -> Result<ModeRecord, String> {
    let t0 = Instant::now();
    let fam_i = Family::parse_with(ind, family_text)?;
    let mut ri = replay_evidence_drawn(ind, &fam_i, rule, served_keys, policy, opts, drawn);
    let ti = ind.take_events();
    let si = t0.elapsed().as_secs_f64();
    let t1 = Instant::now();
    let fam_s = Family::parse_with(sym, family_text)?;
    let rs = replay_evidence_drawn(sym, &fam_s, rule, served_keys, policy, opts, drawn);
    let ts = sym.take_events();
    let ss = t1.elapsed().as_secs_f64();
    let mut referee = || {
        let r = recording(Referee);
        let fam = Family::parse_with(&r, family_text).ok()?;
        let _ = replay_evidence_drawn(&r, &fam, rule, served_keys, policy, opts, drawn);
        Some(r.take_events())
    };
    let (triage, sr) = triage_traces((&ri.identity, &ti), (&rs.identity, &ts), &mut referee);
    let finding = match &triage {
        Triage::Differ(d) => Some(TriageFinding {
            rule_index: rule.index,
            target: rule.target_text.clone(),
            difference: d.clone(),
        }),
        _ => None,
    };
    match &triage {
        Triage::AgreePass | Triage::AgreeFail { .. } | Triage::AgreeOther { .. } => {}
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
    let report = TriageReport {
        triage,
        independent_verdict: verdict_text(&ri.identity.verdict),
        symbolica_verdict: verdict_text(&rs.identity.verdict),
        trace_len_independent: ti.len(),
        trace_len_symbolica: ts.len(),
        seconds_independent: si,
        seconds_symbolica: ss,
        seconds_referee: sr,
    };
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
