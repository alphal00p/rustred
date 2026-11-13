//! The script of the fake child `rr_fake_child` (child-job.md section 4, `spec.fake`) and the witness-locus elision
//! policy shared with the engine (ruling R-f).
//!
//! The fake child is a binary of `rustred-campaign`; its script lives here so that a spec with a fake script is
//! validated by the same strict parser as any other spec, and so that its deterministic parts (the record a case
//! writes, loci, the exit code of an `end`) are testable without a process.

use crate::exit::ExitCode;
use crate::outcome::TimeDecisionWhat;
use crate::resume::CaseSpec;
use crate::spec::{OutputOptions, WorkCounter};
use crate::stop::{CaseResult, StopCode};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeMap;

/// The length above which an elidable locus is written as "elided" when elision is ALLOWED (today's engine limit).
pub const WITNESS_LOCUS_ELISION_LIMIT: usize = 4000;
/// The literal written in place of an elided locus.
pub const ELIDED: &str = "elided";

/// Apply the output option to a locus list: with elision allowed, loci longer than
/// [`WITNESS_LOCUS_ELISION_LIMIT`] bytes become [`ELIDED`]; with elision forbidden (the default) every locus is kept in
/// full, whatever its length. Returns the list and the number of elided loci.
pub fn apply_loci_policy(loci: Vec<String>, opts: &OutputOptions) -> (Vec<String>, u64) {
    if !opts.elide_witness_loci {
        return (loci, 0);
    }
    let mut n = 0;
    let v = loci
        .into_iter()
        .map(|l| {
            if l.len() > WITNESS_LOCUS_ELISION_LIMIT {
                n += 1;
                ELIDED.to_string()
            } else {
                l
            }
        })
        .collect();
    (v, n)
}

/// Whether a record's `witness_loci` contains an elided locus (such a record is unusable by the campaign, R-f).
pub fn record_has_elided_loci(record: &Value) -> bool {
    record.get("witness_loci").and_then(Value::as_array).is_some_and(|a| a.iter().any(|x| x.as_str() == Some(ELIDED)))
}

/// Reaction to SIGTERM / SIGINT / SIGHUP.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OnSigterm {
    /// Stop at the next safe point (<= 50 ms).
    #[default]
    Graceful,
    /// Keep working for `slow_ms` before honouring the stop (simulates a long non-safe section).
    Slow,
    /// Signals are ignored (SIG_IGN): only SIGKILL ends the child.
    Ignore,
    /// Default action: the signal kills the process.
    Die,
}

/// Synthetic witness loci for a rule record.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FakeLoci {
    /// Number of loci.
    pub n: u32,
    /// Length of each locus in bytes.
    pub len: u64,
}

impl FakeLoci {
    /// Deterministic locus strings `n<k>*(...)` of exactly `len` bytes (minimum 4).
    pub fn generate(&self) -> Vec<String> {
        (0..self.n)
            .map(|k| {
                let head = format!("n{}*(", k % 20);
                let len = self.len.max(head.len() as u64 + 1) as usize;
                let mut s = String::with_capacity(len);
                s.push_str(&head);
                let body = b"n1+n2-d+";
                let mut i = 0;
                while s.len() < len - 1 {
                    s.push(body[i % body.len()] as char);
                    i += 1;
                }
                s.push(')');
                s
            })
            .collect()
    }
}

/// A rule produced by a case.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FakeRule {
    /// The record written verbatim, except `type` = "rule", `index` = the commit count, and (with jitter) the volatile
    /// keys `seconds_search` / `lift_seconds`; `witness_loci` from `loci` when given.
    pub record: Value,
    #[serde(default)]
    pub loci: Option<FakeLoci>,
}

/// One step of the script.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "op", rename_all = "snake_case", deny_unknown_fields)]
pub enum FakeOp {
    /// Phase `prepare` for `ms`.
    Prepare {
        #[serde(default)]
        ms: u64,
    },
    /// One case: the work counters advance linearly over `ms`; stop flag, STOP file and budgets are checked every
    /// 50 ms; a per-case budget fires exactly at its limit.
    Case {
        ordinal: u64,
        #[serde(default)]
        ms: u64,
        #[serde(default)]
        work: BTreeMap<WorkCounter, u64>,
        result: CaseResult,
        #[serde(default)]
        rule: Option<FakeRule>,
        #[serde(default)]
        children: Vec<CaseSpec>,
    },
    /// Wait for `n` heartbeats (n x heartbeat_s), checking the stop flag.
    Heartbeats { n: u32 },
    /// Record a time decision (event + OUTCOME `time_decisions`, so `time_decided = true`).
    TimeDecision { what: TimeDecisionWhat, limit_s: f64, measured_s: f64, taken: String },
    /// Allocate and touch `mb` MiB, kept until exit.
    Alloc { mb: u64 },
    /// Fork the helper `helpers[index]`.
    Helper { index: usize },
    /// Finish: residual / end record (`end` for stop null, `partial` otherwise), final RESUME, OUTCOME; exit with the
    /// code of the stop, or of `verdict` (11 decided_negative / 12 undecided) for checker-like ends.
    End {
        #[serde(default)]
        stop: Option<StopCode>,
        #[serde(default)]
        verdict: Option<ExitCode>,
    },
    /// Finish with an error OUTCOME (`code` in 40 / 41 / 42 / 51 / 70) and exit with it.
    Fail { code: ExitCode, message: String },
}

/// A helper process to fork.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct HelperPlan {
    /// The helper sleeps this long, then exits (a bound so that a test never leaks a process forever).
    pub sleep_ms: u64,
    /// Set PR_SET_PDEATHSIG = SIGKILL and check the parent pid (contract 3.4.6).
    #[serde(default = "yes")]
    pub pdeathsig: bool,
    /// SIGTERM / SIGINT / SIGHUP = SIG_IGN in the helper (contract 3.4.6).
    #[serde(default = "yes")]
    pub ignore_sigterm: bool,
    /// Deliberate violation: `setsid()` into its own session and group.
    #[serde(default)]
    pub escape_group: bool,
}

fn yes() -> bool {
    true
}

/// Fault kinds.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FaultKind {
    /// SIGKILL itself.
    SigkillSelf,
    /// `abort()` (signal 6, no OUTCOME).
    Abort,
    /// A panic: the hook writes OUTCOME (stop panic) and exits 70.
    Panic,
    /// Write `bytes` bytes of the next table line without "\n", then SIGKILL itself.
    TornWrite,
    /// The ENOSPC path (3.10): a write to `path` (e.g. /dev/full) or an injected ENOSPC.
    Enospc,
    /// The output-error path: exit 51 with OUTCOME.
    IoError,
    /// Stop progress (no safe point) but keep the heartbeat.
    Hang,
    /// Stop the heartbeat, then hang.
    StopHeartbeat,
    /// Exit with `code` and no OUTCOME (a contract violation).
    Exit,
    /// After the end record is written and the table sealed, SIGKILL itself before OUTCOME.
    KillBeforeOutcome,
    /// Leave the end record out but write an OUTCOME that claims a sealed table.
    OutcomeUnsealed,
}

/// One fault, triggered before op number `at_op` (0-based index into `script`; `at_op = script.len()` = at the end).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Fault {
    pub at_op: usize,
    pub kind: FaultKind,
    #[serde(default)]
    pub bytes: u64,
    #[serde(default)]
    pub code: i32,
    #[serde(default)]
    pub path: Option<String>,
}

/// Memory growth plan.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RssPlan {
    #[serde(default)]
    pub grow_bytes_per_s: u64,
    #[serde(default)]
    pub max_bytes: u64,
}

fn one() -> f64 {
    1.0
}

/// `spec.fake`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FakeScript {
    pub script: Vec<FakeOp>,
    #[serde(default)]
    pub on_sigterm: OnSigterm,
    #[serde(default)]
    pub slow_ms: u64,
    /// Slow start: sleep this long before anything is written (no `job_started` event during it).
    #[serde(default)]
    pub start_delay_ms: u64,
    #[serde(default)]
    pub rss: RssPlan,
    #[serde(default)]
    pub helpers: Vec<HelperPlan>,
    #[serde(default)]
    pub faults: Vec<Fault>,
    /// Scales every `ms` of the script.
    #[serde(default = "one")]
    pub time_scale: f64,
    /// Fill volatile keys with run-dependent values (two runs differ byte-wise, not canonically).
    #[serde(default)]
    pub volatile_jitter: bool,
}

impl FakeScript {
    pub fn validate(&self) -> Result<(), String> {
        if self.script.is_empty() {
            return Err("empty fake script".into());
        }
        if !(self.time_scale > 0.0 && self.time_scale.is_finite()) {
            return Err("time_scale must be positive".into());
        }
        let mut last = None;
        for op in &self.script {
            match op {
                FakeOp::Case { ordinal, rule, result, .. } => {
                    if last.is_some_and(|l| *ordinal <= l) {
                        return Err("case ordinals must increase".into());
                    }
                    last = Some(*ordinal);
                    if (*result == CaseResult::Rule) != rule.is_some() {
                        return Err(format!("case {ordinal}: a rule exactly when result is rule"));
                    }
                    if let Some(r) = rule {
                        if !r.record.is_object() {
                            return Err(format!("case {ordinal}: rule.record must be an object"));
                        }
                    }
                }
                FakeOp::Helper { index } if *index >= self.helpers.len() => {
                    return Err(format!("helper index {index} out of range"));
                }
                FakeOp::End { stop, verdict } => {
                    if let Some(v) = verdict {
                        if !matches!(v, ExitCode::DecidedNegative | ExitCode::Undecided) || stop.is_some() {
                            return Err("end.verdict is 11 or 12, with stop null".into());
                        }
                    }
                }
                FakeOp::Fail { code, .. } => {
                    if !matches!(code, ExitCode::Usage | ExitCode::Input | ExitCode::Refused | ExitCode::Io | ExitCode::Internal) {
                        return Err("fail.code is one of 40, 41, 42, 51, 70".into());
                    }
                }
                _ => {}
            }
        }
        for f in &self.faults {
            if f.at_op > self.script.len() {
                return Err(format!("fault at_op {} beyond the script", f.at_op));
            }
        }
        Ok(())
    }

    /// Exit code of an `end` op.
    pub fn end_exit(stop: Option<StopCode>, verdict: Option<ExitCode>) -> ExitCode {
        match (stop, verdict) {
            (_, Some(v)) => v,
            (None, None) => ExitCode::Complete,
            (Some(s), None) => s.exit_code(),
        }
    }
}

/// The rule record a fake case writes (deterministic part): `record` with `type` = "rule", `index` = `index`, and
/// `witness_loci` from `loci` under the output option; `seconds_search` / `lift_seconds` are set to `jitter` (0.0
/// when no jitter). Returns the compact JSON line and the number of elided loci.
pub fn fake_rule_record(rule: &FakeRule, index: u64, opts: &OutputOptions, jitter: Option<(f64, f64)>) -> (Vec<u8>, u64) {
    let mut m = rule.record.as_object().cloned().unwrap_or_default();
    m.insert("type".into(), Value::from("rule"));
    m.insert("index".into(), Value::from(index));
    let (a, b) = jitter.unwrap_or((0.0, 0.0));
    m.insert("seconds_search".into(), Value::from(a));
    m.insert("lift_seconds".into(), Value::from(b));
    let mut elided = 0;
    if let Some(l) = rule.loci {
        let (v, n) = apply_loci_policy(l.generate(), opts);
        elided = n;
        m.insert("witness_loci".into(), Value::from(v));
    }
    (serde_json::to_vec(&Value::Object(m)).expect("record serialises"), elided)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn loci_policy() {
        let l = FakeLoci { n: 3, len: 5000 }.generate();
        assert!(l.iter().all(|s| s.len() == 5000));
        let (full, n) = apply_loci_policy(l.clone(), &OutputOptions { elide_witness_loci: false });
        assert_eq!((full.clone(), n), (l.clone(), 0));
        let (el, n) = apply_loci_policy(l, &OutputOptions { elide_witness_loci: true });
        assert_eq!(n, 3);
        assert!(el.iter().all(|s| s == ELIDED));
        let short = FakeLoci { n: 2, len: 10 }.generate();
        assert_eq!(apply_loci_policy(short.clone(), &OutputOptions { elide_witness_loci: true }), (short, 0));
    }

    #[test]
    fn end_exit_codes() {
        assert_eq!(FakeScript::end_exit(None, None), ExitCode::Complete);
        assert_eq!(FakeScript::end_exit(Some(StopCode::MaxCases), None), ExitCode::Incomplete);
        assert_eq!(FakeScript::end_exit(None, Some(ExitCode::Undecided)), ExitCode::Undecided);
    }

    #[test]
    fn rule_record_is_deterministic_and_canonical_without_jitter() {
        let r = FakeRule { record: serde_json::json!({"chart":"C","target":"I(1)","rhs":[["I(0)","1"]]}), loci: Some(FakeLoci { n: 1, len: 5000 }) };
        let (a, n) = fake_rule_record(&r, 4, &OutputOptions::default(), None);
        let (b, _) = fake_rule_record(&r, 4, &OutputOptions::default(), Some((1.5, 2.25)));
        assert_eq!(n, 0);
        assert_ne!(a, b);
        assert_eq!(crate::canonical_line(&a).unwrap(), crate::canonical_line(&b).unwrap());
        assert!(!record_has_elided_loci(&serde_json::from_slice(&a).unwrap()));
        let (c, n) = fake_rule_record(&r, 4, &OutputOptions { elide_witness_loci: true }, None);
        assert_eq!(n, 1);
        assert!(record_has_elided_loci(&serde_json::from_slice(&c).unwrap()));
    }
}
