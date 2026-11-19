//! Adapter of kind `regular-basis` (lane wireflow (g); interfaces/regbasis.md): lane regbasis' `rr_regbasis` as a
//! LEGACY DIAGNOSTIC child under the job contract. One job = one CLOSED node's statement (pinned: the effective import
//! spec + the statement directory's statement.jsonl / terminals.json / SHA256SUMS). The WRAPPER (here, in the
//! supervisor, after the launcher reaped the child) re-reads every pinned input (digest = the spec's, else an error
//! OUTCOME 41 naming [`regbasis::INPUT_MISMATCH`]: row 39), checks the export object against the pinned statement
//! ([`regbasis::check_results`]), seals `out/results.jsonl` and writes OUTCOME; the contract classifies (rows 37 / 38 /
//! 39, 15b, signals) and the record is read ONLY through the verified handle (accept mode `diagnostic_records`: never
//! rule or relation evidence, never a verdict).

use super::proof::legacy_wait;
use crate::state::Rung;
use crate::supervisor::worksource::{JobRequest, Settled};
use rustred_campaign_contract::classify::{Accept, WaitStatus};
use rustred_campaign_contract::exit::ExitCode;
use rustred_campaign_contract::fake::FakeScript;
use rustred_campaign_contract::layout::{RESULTS, RunLayout};
use rustred_campaign_contract::outcome::{
    EndKind, ErrorClass, ErrorInfo, ExtraOutput, JobOutcome, Outcome, ProgramInfo, StopInfo,
};
use rustred_campaign_contract::regbasis;
use rustred_campaign_contract::seal::RecordWriter;
use rustred_campaign_contract::spec::{Budgets, InputRef, JobKind, JobSpec};
use rustred_campaign_contract::stop::StopCode;
use serde_json::{Value, json};
use std::io::Write;
use std::path::Path;

fn program_info(spec: &JobSpec) -> ProgramInfo {
    ProgramInfo {
        name: spec.program.name.clone(),
        version: "legacy".into(),
        git_commit: rustred_campaign_contract::UNKNOWN_COMMIT.into(),
        features: vec!["legacy-wrapped".into()],
        exe: None,
    }
}

/// A pinned input (BLAKE3 of the file now).
pub fn input_ref(role: &str, path: &Path) -> Result<InputRef, String> {
    let b = std::fs::read(path).map_err(|e| format!("{}: {e}", path.display()))?;
    Ok(InputRef {
        role: role.into(),
        path: path.display().to_string(),
        blake3: rustred_campaign_contract::blake3_hex(&b),
        upto: None,
    })
}

/// The job's settings (the repair policy's `regbasis` step).
#[derive(Debug, Clone)]
pub struct RegBasisJobConfig {
    pub program: String,
    pub args: Vec<String>,
    pub cap_bytes: u64,
    pub expected_peak_bytes: u64,
    pub heartbeat_s: u32,
    pub priority: i64,
    pub fake: Option<FakeScript>,
}

/// The job request of one node: the effective import spec and the statement directory's three files.
pub fn job_request(
    item: &str,
    job: &str,
    import_spec: &Path,
    statement_dir: &Path,
    cfg: &RegBasisJobConfig,
    depends_on: Vec<String>,
) -> Result<JobRequest, String> {
    let mut inputs = vec![input_ref("import_spec", import_spec)?];
    for (role, name) in regbasis::STATEMENT_FILES {
        inputs.push(input_ref(role, &statement_dir.join(name))?);
    }
    Ok(JobRequest {
        job: job.into(),
        item: item.into(),
        rung: Rung(0),
        kind: JobKind::RegularBasis,
        priority: cfg.priority,
        cap_bytes: cfg.cap_bytes,
        expected_peak_bytes: cfg.expected_peak_bytes.min(cfg.cap_bytes),
        cores: 1,
        heartbeat_s: cfg.heartbeat_s.max(1),
        stop_grace_s: 5,
        program: cfg.program.clone(),
        args: cfg.args.clone(),
        inputs,
        depends_on,
        budgets: Budgets::default(),
        sample_seed: 0,
        fake: cfg.fake.clone(),
    })
}

/// The record (and tower) of a settled attempt, read through the supervisor's verified canonical lines (row 37 only);
/// `None` = nothing to read (rows 38 / 39, refusals, signals: the node gets no regular basis).
pub fn records_of_settled(done: &Settled) -> Result<Option<(Value, Option<Value>)>, String> {
    if done.classification.accept != Accept::DiagnosticRecords || done.classification.row != 37 {
        return Ok(None);
    }
    regbasis::records_of(&done.canonical_lines).map(Some)
}

/// THE WRAPPER of the kind (before the contract classifies; the supervisor settle's dispatch from `legacy::wrap`): a
/// no-op for an unknown wait status and when OUTCOME exists. Legacy exit 0: the pinned inputs re-read (digest = the
/// spec's, else an error OUTCOME 41 naming [`regbasis::INPUT_MISMATCH`]: row 39), the record checked against the pinned
/// statement; checked lines sealed + OUTCOME (row 37), otherwise OUTCOME without a sealed stream (row 38). Legacy exit
/// 2 (the tool's refusal): error OUTCOME 41 (row 39), or 51 when the message names a host resource errno (row 15b).
pub fn wrap(layout: &RunLayout, spec: &JobSpec, spec_blake3: &str, wait: WaitStatus) -> WaitStatus {
    if matches!(wait, WaitStatus::Unknown) {
        return legacy_wait(wait);
    }
    // procfix7 round 19 (landing, legacy::wrap's form): OUTCOME is written only when PROVEN absent (an unreadable one is
    // left to the classification, which reads it: never a second outcome written on a failed stat)
    match crate::fsx::presence_lstat("adapters.outcome", &layout.outcome()) {
        crate::fsx::Presence::Absent => {}
        crate::fsx::Presence::Present(_) | crate::fsx::Presence::Unknown(_) => {
            return legacy_wait(wait);
        }
    }
    let mapped = legacy_wait(wait);
    let write = |j: JobOutcome| {
        if let Err(e) = Outcome::new(j).write_new(&layout.outcome()) {
            let _ = writeln!(std::io::stderr(), "regbasis wrapper: OUTCOME: {e}");
        }
    };
    let input_error = |msg: String| -> WaitStatus {
        let mut j = JobOutcome::for_spec(
            spec,
            spec_blake3,
            program_info(spec),
            ExitCode::Input,
            StopInfo {
                code: Some(StopCode::InputError),
                ..StopInfo::none()
            },
        );
        j.error = Some(ErrorInfo::new(ErrorClass::Input, msg.clone()));
        if rustred_campaign_contract::classify::resource_errno_of(&j).is_some() {
            let mut io = JobOutcome::for_spec(
                spec,
                spec_blake3,
                program_info(spec),
                ExitCode::Io,
                StopInfo {
                    code: Some(StopCode::IoError),
                    ..StopInfo::none()
                },
            );
            io.error = Some(ErrorInfo::new(ErrorClass::Io, msg));
            write(io);
            return WaitStatus::Exited(ExitCode::Io.code());
        }
        write(j);
        WaitStatus::Exited(ExitCode::Input.code())
    };
    // procfix7 round 19 (landing): a file that cannot be READ is an io end (lost, retried after the back-off), never
    // evidence about the record (legacy::wrap's form)
    let io_error = |msg: String| -> WaitStatus {
        let mut io = JobOutcome::for_spec(
            spec,
            spec_blake3,
            program_info(spec),
            ExitCode::Io,
            StopInfo {
                code: Some(StopCode::IoError),
                ..StopInfo::none()
            },
        );
        io.error = Some(ErrorInfo::new(ErrorClass::Io, msg));
        write(io);
        WaitStatus::Exited(ExitCode::Io.code())
    };
    match mapped {
        WaitStatus::Exited(0) => {
            let mut statement = None;
            for i in &spec.inputs {
                match std::fs::read(spec.input_path(i)) {
                    Ok(b) if rustred_campaign_contract::blake3_hex(&b) == i.blake3 => {
                        if i.role == "statement" {
                            statement = Some(b);
                        }
                    }
                    Ok(_) => {
                        return input_error(format!(
                            "{}: the {} input is not the spec's",
                            regbasis::INPUT_MISMATCH,
                            i.role
                        ));
                    }
                    Err(e) => return input_error(format!("{} input: {e}", i.role)),
                }
            }
            let Some(statement) = statement else {
                return input_error("no statement input".into());
            };
            let dir = layout.out_dir().join(regbasis::CHILD_DIR);
            // procfix7 round 19 (landing): a record that cannot be READ is never an empty record (absent = none)
            let record =
                match crate::fsx::read("adapters.outcome", &dir.join(regbasis::RECORD_FILE)) {
                    crate::fsx::Read::Content(b) => b,
                    crate::fsx::Read::Absent => Vec::new(),
                    crate::fsx::Read::Failed(e) => {
                        return io_error(format!("{}: {e}", regbasis::RECORD_FILE));
                    }
                };
            // procfix7 round 16 (int8b-gate review of the tripwire-flagged lines, defect 1): an EXISTING tower.json that
            // cannot be read is never sealed away (the record accepted without the tower the child wrote): an io
            // OUTCOME (ExitCode::Io, retried); only a tower that is not there is None
            let tower = match read_tower(&dir.join(regbasis::TOWER_FILE)) {
                Ok(t) => t,
                Err(e) => return io_error(format!("{}: {e}", regbasis::TOWER_FILE)),
            };
            let sha = rustred_campaign_contract::symcert::sha256_hex(&statement);
            let mut j = JobOutcome::for_spec(
                spec,
                spec_blake3,
                program_info(spec),
                ExitCode::Complete,
                StopInfo::none(),
            );
            match regbasis::check_results(&sha, &record, tower.as_deref()) {
                Ok(lines) => {
                    j.counters.cases = lines.len() as u64;
                    let sealed = (|| -> Result<_, String> {
                        let mut w =
                            RecordWriter::create(&layout.results()).map_err(|e| e.to_string())?;
                        for l in &lines {
                            w.append(l.as_bytes()).map_err(|e| e.to_string())?;
                        }
                        let end =
                            json!({"type": "end", "records": lines.len(), "statement_sha256": sha});
                        w.seal(end.to_string().as_bytes())
                            .map_err(|e| e.to_string())
                    })();
                    match sealed {
                        Ok(s) => j.outputs.extra.push(ExtraOutput {
                            role: "results".into(),
                            path: RunLayout::out_rel(RESULTS),
                            bytes: s.bytes,
                            blake3: s.blake3,
                            records: Some(s.records),
                            canonical_blake3: Some(s.canonical_blake3),
                            end: Some(EndKind::End),
                            sealed: true,
                        }),
                        Err(e) => {
                            let _ = writeln!(
                                std::io::stderr(),
                                "regbasis wrapper: sealing failed: {e}"
                            );
                        }
                    }
                    write(j);
                }
                Err(why) => {
                    // OUTCOME without a sealed stream: row 38 (nothing read)
                    let _ = std::fs::write(layout.sup_dir().join("wrapper-refusal.txt"), &why);
                    write(j);
                }
            }
            mapped
        }
        WaitStatus::Exited(41) => {
            // procfix7 round 19 (landing): a log that cannot be READ leaves the cause unknown: an io end, never input
            // evidence
            let msg = match crate::fsx::read_to_string("adapters.outcome", &layout.stdout_log()) {
                crate::fsx::Read::Content(s) => s
                    .lines()
                    .last()
                    .map(str::to_string)
                    .unwrap_or_else(|| "rr_regbasis refused its input".into()),
                crate::fsx::Read::Absent => "rr_regbasis refused its input".into(),
                crate::fsx::Read::Failed(e) => {
                    return io_error(format!(
                        "rr_regbasis refused its input, its log could not be read: {e}"
                    ));
                }
            };
            input_error(msg)
        }
        w => w,
    }
}

/// procfix7 round 16 (int8b-gate review, defect 1): the OPTIONAL tower of the child's export: not there (NotFound) =
/// None; any other read error is an error, never "no tower".
fn read_tower(path: &Path) -> std::io::Result<Option<Vec<u8>>> {
    // procfix7 round 19 (landing): through the fsx rule (Absent = NotFound only)
    match crate::fsx::read("adapters.outcome", path) {
        crate::fsx::Read::Content(b) => Ok(Some(b)),
        crate::fsx::Read::Absent => Ok(None),
        crate::fsx::Read::Failed(e) => Err(e),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// procfix7 round 16 (int8b-gate review, defect 1): a tower that exists but cannot be read (here: a directory, EISDIR)
    /// is an error, never None (the record would be sealed and accepted without it).
    #[test]
    fn a_tower_that_exists_but_cannot_be_read_is_an_error_never_none() {
        let d = std::env::temp_dir().join(format!("rr-regbasis-tower-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        assert!(read_tower(&d.join("absent.json")).unwrap().is_none());
        std::fs::write(d.join("tower.json"), b"{}").unwrap();
        assert_eq!(
            read_tower(&d.join("tower.json")).unwrap().as_deref(),
            Some(&b"{}"[..])
        );
        std::fs::create_dir(d.join("unreadable.json")).unwrap();
        assert!(read_tower(&d.join("unreadable.json")).is_err());
        let _ = std::fs::remove_dir_all(&d);
    }
}
