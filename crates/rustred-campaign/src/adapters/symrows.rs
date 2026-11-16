//! Adapter of kind `proof-symrows` (lane symcert; interfaces/proof-symrows-job.md): rr_proof `--mode instance-span
//! --symrows FILE [--symrows-only]` as a LEGACY child under the job contract. ONE job per relation: the relation's
//! files (core `symcert::build_files`: point record + key + symmetry rows + optional zero-sector census) and the pinned
//! request (relation digest, record SHA-256 / BLAKE3, group, elements, catalogue records, kernel sha256) are written
//! into the attempt's `in/`; after the launcher reaped the child the WRAPPER ([`wrap`], also dispatched from
//! `legacy::wrap` for the supervisor's settle) re-reads the pinned inputs, checks the kernel's record against the
//! request (record SHA-256 recomputed here: the contract has no SHA-256) and seals ONE certificate; the contract then
//! classifies (rows 40-43, 15b, signals) and re-checks the sealed certificate against the request (S of the kind). The
//! certificate is read ONLY through the verified handle.
//!
//! Generic (L8): keys are integer vectors of any length; programs, caps and families are data.

use super::job::{
    AdapterError, Ended, JobAdapter, JobPlan, Prepared, Program, classify_attempt, create_run,
    sha256_hex, write_input, write_spec,
};
use super::proof::legacy_wait;
use rustred_campaign_contract::classify::symcert::{
    self as sc, CatalogueUse, Certificate, SymrowsRequest, ZeroCensus,
};
use rustred_campaign_contract::classify::{Classification, WaitStatus};
use rustred_campaign_contract::exit::ExitCode;
use rustred_campaign_contract::fake::FakeScript;
use rustred_campaign_contract::layout::{RESULTS, RunLayout};
use rustred_campaign_contract::outcome::{
    EndKind, ErrorClass, ErrorInfo, ExtraOutput, JobOutcome, Outcome, ProgramInfo, StopInfo,
};
use rustred_campaign_contract::seal::RecordWriter;
use rustred_campaign_contract::spec::{
    Budgets, Determinism, JobKind, JobSpec, OnCaseBudget, OutputOptions, ProgramRef, SeedSpec,
};
use rustred_campaign_contract::stop::StopCode;
use rustred_campaign_core::symcert::{JobFiles, SymRelation, build_files};
use serde_json::json;
use std::io::Write;
use std::path::Path;

/// The rung tag of proof-symrows job ids.
pub const RUNG: &str = "sc";

/// One relation as a job request: the relation, its files and the request (every digest filled in).
#[derive(Clone, Debug)]
pub struct SymrowsJobRequest {
    pub relation: SymRelation,
    pub files: JobFiles,
    pub request: SymrowsRequest,
}

impl SymrowsJobRequest {
    /// Build the files and the request of a relation. `family_source` = the presented family of the keys' frame;
    /// `kernel_sha256` = SHA-256 of the kernel binary that will run.
    pub fn new(
        rel: SymRelation,
        family_source: &str,
        kernel_sha256: &str,
        max_primes: Option<u32>,
    ) -> Result<SymrowsJobRequest, String> {
        let files = build_files(&rel, family_source)?;
        let relation_digest = sc::relation_digest(&rel.key, &rel.rhs)?;
        let census = match (&rel.zero_sectors, &files.census_line) {
            (Some(z), Some(l)) => Some(ZeroCensus {
                producer: z.producer.clone(),
                masks: files.census_masks,
                blake3: rustred_campaign_contract::blake3_hex(l),
            }),
            _ => None,
        };
        let request = SymrowsRequest {
            schema: sc::REQUEST_SCHEMA.into(),
            relation_digest,
            rule: 0,
            offset: files.offset,
            record_sha256: sha256_hex(&files.record_line),
            record_blake3: rustred_campaign_contract::blake3_hex(&files.record_line),
            key: rel.key.clone(),
            producer: rel.producer.clone(),
            producer_record_sha256: rel.producer_record_sha256.clone(),
            group_digest: rel.group_digest.clone(),
            elements: files.elements.clone(),
            symrows_lines: rel.rows.len() as u64,
            symrows_only: !files.needs_ibp_rows,
            rows: if files.needs_ibp_rows {
                "engine".into()
            } else {
                "none".into()
            },
            drop_zero_sectors: census.is_some(),
            zero_census: census,
            catalogue: rel
                .catalogue
                .iter()
                .map(|c| CatalogueUse {
                    source: c.source.clone(),
                    label: c.label.clone(),
                    record_sha256: c.record_sha256.clone(),
                })
                .collect(),
            kernel_sha256: kernel_sha256.to_string(),
            family_blake3: rustred_campaign_contract::blake3_hex(family_source.as_bytes()),
            family_sha256: sha256_hex(family_source.as_bytes()),
            relation_sha256: sha256_hex(&sc::relation_canonical(&rel.key, &rel.rhs)?),
            max_primes,
        };
        request.check()?;
        Ok(SymrowsJobRequest {
            relation: rel,
            files,
            request,
        })
    }

    /// The request bytes (the pinned `request` input).
    pub fn request_bytes(&self) -> Vec<u8> {
        serde_json::to_vec(&self.request).expect("request serialises")
    }
}

/// The adapter (data).
#[derive(Debug, Clone)]
pub struct SymrowsAdapter {
    pub program: Program,
    pub licence_env: Option<String>,
    /// Test stand: the fake child emulates the kind.
    pub fake: Option<FakeScript>,
}

/// The output of one attempt.
#[derive(Debug, Clone)]
pub struct SymrowsOutcome {
    pub job_id: String,
    pub classification: Classification,
    /// The checked certificate (row 40).
    pub certificate: Option<Certificate>,
    /// The kernel's refusal message (row 41: SYMROW_REFUSED ...; also rows 13 / 42).
    pub refusal: Option<String>,
    /// Why no certificate was read (every row but 40).
    pub unread: Option<String>,
}

fn program_info(spec: &JobSpec) -> ProgramInfo {
    ProgramInfo {
        name: spec.program.name.clone(),
        version: "legacy".into(),
        git_commit: rustred_campaign_contract::UNKNOWN_COMMIT.into(),
        features: vec!["legacy-wrapped".into()],
        exe: None,
    }
}

/// The checked certificate line of an exit-0 attempt (pinned inputs re-read, the kernel record checked).
fn checked_certificate(layout: &RunLayout, spec: &JobSpec) -> Result<Vec<u8>, String> {
    let r = sc::request(spec)?;
    let table = sc::pinned(spec, "table")?;
    let line = sc::table_relation(&table, &r)?;
    if sha256_hex(&line) != r.record_sha256 {
        return Err("the table record's SHA-256 is not the request's".into());
    }
    // the import-side digests (SHA-256; the contract checks their echo, the values are checked here)
    let (key, rhs) = sc::relation_of_record(&line)?;
    if sha256_hex(&sc::relation_canonical(&key, &rhs)?) != r.relation_sha256 {
        return Err("the record's relation SHA-256 is not the request's".into());
    }
    let head: serde_json::Value = serde_json::from_slice(sc::line_at(&table, 0)?)
        .map_err(|e| format!("table header: {e}"))?;
    if head["family_source"]
        .as_str()
        .map(|f| sha256_hex(f.as_bytes()))
        .as_deref()
        != Some(r.family_sha256.as_str())
    {
        return Err("the table header's family SHA-256 is not the request's".into());
    }
    let symrows = sc::pinned(spec, "symrows")?;
    let n = symrows
        .split(|&c| c == b'\n')
        .filter(|l| !l.iter().all(u8::is_ascii_whitespace))
        .count() as u64;
    if n != r.symrows_lines {
        return Err(format!(
            "{n} symrows lines, the request says {}",
            r.symrows_lines
        ));
    }
    let raw = std::fs::read(layout.out_dir().join(sc::CHILD_OUT))
        .map_err(|e| format!("out/{}: {e}", sc::CHILD_OUT))?;
    let entry = sc::check_span_output(&r, &raw)?;
    let sym_b3 = rustred_campaign_contract::blake3_hex(&symrows);
    sc::certificate_line(&r, &sym_b3, &entry)
}

/// THE WRAPPER of the kind (before the contract classifies; also dispatched from `legacy::wrap`): a no-op for an
/// unknown wait status and when OUTCOME exists. Legacy exit 0: the certificate checked and sealed + OUTCOME (row 40),
/// otherwise OUTCOME without a sealed stream (row 43). Legacy exit 2 (rr_proof's refusal): error OUTCOME 41 with the
/// kernel's message (rows 41 / 42 / 13), or 51 when it names a host resource errno (row 15b). Anything else stays.
pub fn wrap(layout: &RunLayout, spec: &JobSpec, spec_blake3: &str, wait: WaitStatus) -> WaitStatus {
    if matches!(wait, WaitStatus::Unknown) || layout.outcome().exists() {
        return legacy_wait(wait);
    }
    let mapped = legacy_wait(wait);
    let write = |j: JobOutcome| {
        if let Err(e) = Outcome::new(j).write_new(&layout.outcome()) {
            let _ = writeln!(std::io::stderr(), "symrows wrapper: OUTCOME: {e}");
        }
    };
    match mapped {
        WaitStatus::Exited(0) => {
            let mut j = JobOutcome::for_spec(
                spec,
                spec_blake3,
                program_info(spec),
                ExitCode::Complete,
                StopInfo::none(),
            );
            match checked_certificate(layout, spec) {
                Ok(line) => {
                    j.counters.cases = 1;
                    let req_b3 = spec
                        .inputs
                        .iter()
                        .find(|i| i.role == "request")
                        .map(|i| i.blake3.clone());
                    let sealed = (|| -> Result<_, String> {
                        let mut w =
                            RecordWriter::create(&layout.results()).map_err(|e| e.to_string())?;
                        w.append(&line).map_err(|e| e.to_string())?;
                        let end = json!({"type": "end", "records": 1, "request_blake3": req_b3});
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
                            let _ = writeln!(std::io::stderr(), "symrows wrapper: sealing: {e}");
                        }
                    }
                    write(j);
                }
                Err(why) => {
                    let _ = std::fs::write(layout.sup_dir().join("wrapper-refusal.txt"), &why);
                    write(j);
                }
            }
            mapped
        }
        WaitStatus::Exited(41) => {
            let msg = std::fs::read_to_string(layout.stdout_log())
                .ok()
                .and_then(|s| s.lines().last().map(str::to_string))
                .unwrap_or_else(|| "rr_proof exit 2".into());
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
            mapped
        }
        w => w,
    }
}

/// The kernel's refusal message an attempt's OUTCOME carries (rows 41 / 42 / 13).
pub fn refusal_message(layout: &RunLayout) -> Option<String> {
    Outcome::read(&layout.outcome())
        .ok()
        .and_then(Result::ok)
        .and_then(|o| o.job.error.map(|e| e.message))
}

impl JobAdapter for SymrowsAdapter {
    type Request = SymrowsJobRequest;
    type Output = SymrowsOutcome;

    fn kind(&self) -> JobKind {
        JobKind::ProofSymrows
    }

    /// ONE job per relation (the kernel proves one record per run).
    fn plan(&self, reqs: &[SymrowsJobRequest]) -> Result<Vec<JobPlan>, AdapterError> {
        Ok(reqs
            .iter()
            .enumerate()
            .map(|(i, r)| {
                let id = json!({"kind": "proof-symrows", "program": self.program.blake3,
                    "request": rustred_campaign_contract::blake3_hex(&r.request_bytes()),
                    "table": rustred_campaign_contract::blake3_hex(&r.files.table),
                    "symrows": rustred_campaign_contract::blake3_hex(&r.files.symrows)});
                let content = rustred_campaign_contract::blake3_hex(id.to_string().as_bytes())
                    [..16]
                    .to_string();
                JobPlan {
                    job_id: rustred_campaign_contract::layout::job_id(&content, RUNG),
                    members: vec![i],
                    content,
                }
            })
            .collect())
    }

    fn prepare(
        &self,
        reqs: &[SymrowsJobRequest],
        plan: &JobPlan,
        work_root: &Path,
        attempt: u32,
    ) -> Result<Prepared, AdapterError> {
        let [m] = plan.members.as_slice() else {
            return Err(AdapterError::Request("one relation per job".into()));
        };
        let r = &reqs[*m];
        let root = std::path::absolute(work_root).map_err(|e| AdapterError::Io(e.to_string()))?;
        let layout = create_run(&root, &plan.job_id, attempt)?;
        let inputs = vec![
            write_input(&layout, "request.json", "request", &r.request_bytes())?,
            write_input(&layout, "relation.table.jsonl", "table", &r.files.table)?,
            write_input(&layout, "keys.txt", "keys", &r.files.keys)?,
            write_input(&layout, "symrows.jsonl", "symrows", &r.files.symrows)?,
        ];
        let spec = JobSpec {
            schema: rustred_campaign_contract::CONTRACT.into(),
            job_id: plan.job_id.clone(),
            attempt,
            kind: JobKind::ProofSymrows,
            run_dir: layout.root.display().to_string(),
            program: ProgramRef {
                name: self.program.name.clone(),
                blake3: self.program.blake3.clone(),
                features_required: vec![],
            },
            inputs,
            args: vec![],
            seed: SeedSpec::default(),
            budgets: Budgets::default(),
            on_case_budget: OnCaseBudget::Stop,
            determinism: Determinism::Recorded,
            output: OutputOptions::default(),
            heartbeat_s: 10,
            stop_grace_s: 5,
            resume_every_s: 30,
            resume: None,
            env_allow: vec![],
            fake: self.fake.clone(),
        };
        let argv = sc::argv(&spec).map_err(AdapterError::Spec)?;
        write_spec(&spec, &self.program, self.licence_env.as_deref(), &argv)
    }

    fn finish(
        &self,
        _reqs: &[SymrowsJobRequest],
        plan: &JobPlan,
        prep: &Prepared,
        end: &Ended,
    ) -> SymrowsOutcome {
        let mapped = wrap(&prep.layout, &prep.spec, &prep.spec_blake3, end.wait);
        let e2 = Ended {
            wait: mapped,
            kill: end.kill,
            oom: end.oom,
        };
        let (ev, c) = classify_attempt(prep, &e2);
        let mut out = SymrowsOutcome {
            job_id: plan.job_id.clone(),
            classification: c.clone(),
            certificate: None,
            refusal: None,
            unread: None,
        };
        if c.row != 40 {
            if matches!(c.row, 13 | 41 | 42) {
                out.refusal = refusal_message(&prep.layout);
            }
            out.unread = Some(format!(
                "row {}{}: {}{}{}",
                c.row,
                c.sub,
                c.why,
                ev.unsealed_why
                    .map(|w| format!(" ({w})"))
                    .unwrap_or_default(),
                super::legacy::refusal(&prep.layout)
                    .map(|w| format!(" [wrapper: {w}]"))
                    .unwrap_or_default()
            ));
            return out;
        }
        match ev
            .verified
            .as_ref()
            .map(|v| sc::certificates_of_run(&prep.layout.root, &prep.spec, v))
        {
            Some(Ok(mut certs)) if certs.len() == 1 => out.certificate = certs.pop(),
            Some(Ok(certs)) => out.unread = Some(format!("{} certificates", certs.len())),
            Some(Err(why)) => out.unread = Some(why),
            None => out.unread = Some("no verified outputs".into()),
        }
        out
    }
}
