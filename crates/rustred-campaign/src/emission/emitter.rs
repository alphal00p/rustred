//! THE EMITTER: the only writer of rule records under the emission contract (plan/auto-campaign/interfaces/
//! emission-check.md section 3). For one candidate: the body (no `derivation`, index = its position), the independent
//! checker run on EXACTLY these bytes (`[header, body]` at an offset) with the certificate of THIS lift, the binding of
//! the checker's output to the body (relation, domain, certificate sha256), the class from `class_of`, the policy, the
//! `derivation` member, and the side files (content-addressed: certificate, check output, provenance, policy).
use super::child::{self, ChildRun};
use rustred_campaign_core::emission::body::{BodyIds, attach_derivation, body_ids, sha256_hex, strip_derivation};
use rustred_campaign_core::emission::class::{
    CheckSummary, Classed, ConeEvidence, Kind, Liveness, Origin, Policy, SampleEvidence, class_of,
};
use rustred_campaign_core::emission::{
    AREA_CERT, AREA_CHECK, AREA_POLICY, AREA_PROV, Parts, derivation, side_rel,
};
use serde_json::{Value, json};
use std::path::{Path, PathBuf};

/// The independent checker (a child; emission contract section 2).
#[derive(Clone, Debug)]
pub struct Checker {
    pub binary: PathBuf,
    pub sha256: String,
    /// v0+rows certificates: recover the carried rows over the checker's own ordinary rows
    pub recover_rows: bool,
    pub family: Option<PathBuf>,
    pub zero_sectors: Option<PathBuf>,
    pub symmetry_group: Option<PathBuf>,
    /// budget = max(floor_s, ratio x lift seconds) (contract section 8)
    pub floor_s: f64,
    pub ratio: f64,
}

impl Checker {
    pub fn new(binary: &Path) -> Result<Checker, String> {
        Ok(Checker {
            sha256: child::file_sha256(binary)?,
            binary: binary.to_path_buf(),
            recover_rows: true,
            family: None,
            zero_sectors: None,
            symmetry_group: None,
            floor_s: 120.0,
            ratio: 1.0,
        })
    }
    pub fn budget(&self, lift_seconds: f64) -> f64 {
        self.floor_s.max(self.ratio * lift_seconds)
    }
}

/// Run the checker on `[header, body]` with `cert` in `dir`; the output is accepted only when it was made on these
/// bytes' relation and domain and on this certificate (binding by construction).
pub fn run_checker(
    checker: &Checker,
    header: &str,
    body: &str,
    cert: &Path,
    dir: &Path,
    budget_s: f64,
) -> Result<CheckRun, String> {
    std::fs::create_dir_all(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    let input = dir.join("check-input.table.jsonl");
    let header = header.trim_end();
    std::fs::write(&input, format!("{header}\n{body}\n"))
        .map_err(|e| format!("{}: {e}", input.display()))?;
    let index = serde_json::from_str::<Value>(body)
        .ok()
        .and_then(|v| v["index"].as_i64())
        .unwrap_or(0);
    let mut args: Vec<String> = vec![
        "--mode".into(),
        "weight-cert".into(),
        "--cert".into(),
        cert.display().to_string(),
        "--table".into(),
        input.display().to_string(),
        "--rule".into(),
        index.to_string(),
        "--offset".into(),
        (header.len() + 1).to_string(),
    ];
    if checker.recover_rows {
        args.push("--recover-rows".into());
    }
    for (flag, p) in [
        ("--family", &checker.family),
        ("--zero-sectors", &checker.zero_sectors),
        ("--symmetry-group", &checker.symmetry_group),
    ] {
        if let Some(p) = p {
            args.push(flag.into());
            args.push(p.display().to_string());
        }
    }
    let run = child::run(&checker.binary, &args, &[], dir, "check", budget_s)?;
    let output = run
        .stdout_text()
        .lines()
        .rev()
        .find(|l| l.trim_start().starts_with('{'))
        .map(str::to_string);
    let ids = body_ids(body)?;
    let cert_sha = child::file_sha256(cert)?;
    let summary = if run.over_budget {
        Err(format!("budget_exceeded ({:.1} s > {budget_s:.1} s)", run.seconds))
    } else {
        match &output {
            None => Err(format!(
                "checker_error (exit {:?}, no output: {})",
                run.code,
                run.stderr_tail(300)
            )),
            Some(o) => match CheckSummary::parse(o) {
                Err(e) => Err(format!("checker_error ({e})")),
                Ok(s) if s.relation_sha256 != ids.relation_sha256 => {
                    Err("binding_mismatch (the checker read another relation)".into())
                }
                Ok(s) if s.domain_sha256 != ids.domain_sha256 => {
                    Err("binding_mismatch (the checker read another domain)".into())
                }
                Ok(s) if s.cert_sha256 != cert_sha => {
                    Err("binding_mismatch (the checker read another certificate)".into())
                }
                Ok(s) => Ok(s),
            },
        }
    };
    Ok(CheckRun {
        output,
        summary,
        run,
    })
}

/// One checker run on a body.
#[derive(Clone, Debug)]
pub struct CheckRun {
    /// the checker's output line (absent when it produced none)
    pub output: Option<String>,
    /// parsed, and BOUND to the body and certificate (else the typed reason)
    pub summary: Result<CheckSummary, String>,
    pub run: ChildRun,
}

/// A candidate record (what a producer lifted, or a legacy record brought in).
#[derive(Clone, Debug)]
pub struct Candidate {
    /// the header of the table the record goes into (the checker reads the family and frame from it)
    pub header: String,
    /// the rule line; a `derivation` member it carries is dropped; its `index` must already be its position
    pub line: String,
    pub kind: Kind,
    pub origin: Origin,
    /// the certificate of THIS lift (None: no check can run)
    pub cert: Option<PathBuf>,
    /// producer binary sha256 + its full provenance record (stored beside, referenced by sha256)
    pub provenance: Option<(String, Value)>,
    pub lift_seconds: f64,
    pub ladder: Option<Value>,
    pub cone: Option<ConeEvidence>,
    pub sample: Option<SampleEvidence>,
    /// work subdirectory name
    pub tag: String,
}

/// The result of one emission.
#[derive(Clone, Debug)]
pub struct Emission {
    /// the record line with its derivation (None when the policy refused it)
    pub line: Option<String>,
    pub refused: Option<String>,
    pub derivation: Value,
    pub classed: Classed,
    pub ids: BodyIds,
    pub summary: Option<CheckSummary>,
    pub check: Option<CheckRun>,
}

/// Emitter state: the checker, the policy, the side directory of the target table and a work directory.
pub struct Emitter {
    pub checker: Checker,
    pub policy: Policy,
    pub side: PathBuf,
    pub work: PathBuf,
    policy_sha: String,
}

impl Emitter {
    pub fn new(checker: Checker, policy: Policy, side: &Path, work: &Path) -> Result<Emitter, String> {
        let text = serde_json::to_vec(&policy).map_err(|e| e.to_string())?;
        let e = Emitter {
            checker,
            policy,
            side: side.to_path_buf(),
            work: work.to_path_buf(),
            policy_sha: sha256_hex(&text),
        };
        e.put_side(AREA_POLICY, &text)?;
        Ok(e)
    }

    /// Store a side file under its sha256; returns the sha256.
    pub fn put_side(&self, area: &str, bytes: &[u8]) -> Result<String, String> {
        let sha = sha256_hex(bytes);
        let p = self.side.join(side_rel(area, &sha));
        if !p.exists() {
            std::fs::create_dir_all(p.parent().expect("side area"))
                .map_err(|e| format!("{}: {e}", p.display()))?;
            let tmp = p.with_extension("tmp");
            std::fs::write(&tmp, bytes).map_err(|e| format!("{}: {e}", tmp.display()))?;
            std::fs::rename(&tmp, &p).map_err(|e| format!("{}: {e}", p.display()))?;
        }
        Ok(sha)
    }

    /// Run the checker on `[header, body]` with `cert` (see [`run_checker`]).
    pub fn check(
        &self,
        header: &str,
        body: &str,
        cert: &Path,
        budget_s: f64,
        tag: &str,
    ) -> Result<CheckRun, String> {
        run_checker(&self.checker, header, body, cert, &self.work.join(tag), budget_s)
    }

    /// EMIT one candidate: check, class, policy, derivation. `liveness` of the record at its position (Unknown =
    /// live); `curves_only`: every open locus of the check is a curve the ladder cannot branch.
    pub fn emit(&self, c: &Candidate, liveness: Liveness, curves_only: bool) -> Result<Emission, String> {
        let (body, _) = strip_derivation(&c.line)?;
        let ids = body_ids(&body)?;
        let mut certificate = None;
        let mut check_run = None;
        let check: Result<(String, String, CheckSummary), String> = match &c.cert {
            None => Err(match c.kind {
                Kind::Legacy => "legacy_unchecked".into(),
                _ => "no_certificate".into(),
            }),
            Some(cert) => {
                let bytes = std::fs::read(cert).map_err(|e| format!("{}: {e}", cert.display()))?;
                let form = serde_json::from_slice::<Value>(&bytes)
                    .ok()
                    .map(|v| {
                        let schema = v["schema"].as_str().unwrap_or("").to_string();
                        if schema.ends_with(".v1") {
                            format!("v1-{}", v["basis"].as_str().unwrap_or("?"))
                        } else if v.get("rows").is_some() {
                            "v0+rows".to_string()
                        } else {
                            "v0".to_string()
                        }
                    })
                    .unwrap_or_else(|| "unreadable".into());
                let cert_sha = self.put_side(AREA_CERT, &bytes)?;
                certificate = Some((cert_sha, bytes.len() as u64, form));
                let r = self.check(&c.header, &body, cert, self.checker.budget(c.lift_seconds), &c.tag)?;
                let out = match &r.output {
                    Some(o) => Some(self.put_side(AREA_CHECK, o.as_bytes())?),
                    None => None,
                };
                let res = match (&r.summary, out) {
                    (Ok(s), Some(o)) => Ok((self.checker.sha256.clone(), o, s.clone())),
                    (Err(e), _) => Err(e.clone()),
                    (Ok(_), None) => Err("checker_error (no output)".into()),
                };
                check_run = Some(r);
                res
            }
        };
        let producer = match &c.provenance {
            Some((bin, rec)) => {
                let b = serde_json::to_vec(rec).map_err(|e| e.to_string())?;
                Some((bin.clone(), self.put_side(AREA_PROV, &b)?))
            }
            None => None,
        };
        let summary = check.as_ref().ok().map(|x| x.2.clone());
        let ev = rustred_campaign_core::emission::class::Evidence {
            cone: c.cone.clone(),
            sample: c.sample.clone(),
            ..Default::default()
        };
        let mut classed = class_of(c.kind, c.origin, summary.as_ref(), &ev);
        if classed.class == rustred_campaign_core::emission::class::ClassWord::None
            && let Err(why) = &check
        {
            classed.reason = Some(why.clone());
        }
        let parts = Parts {
            kind: c.kind,
            origin: c.origin,
            body_sha256: ids.body_sha256.clone(),
            producer,
            certificate,
            check,
            classed: classed.clone(),
            cone: c.cone.clone(),
            sample: c.sample.clone(),
            ladder: c.ladder.clone(),
            policy: (self.policy.name.clone(), self.policy_sha.clone()),
        };
        let d = derivation(&parts);
        let sample_keys = c.sample.as_ref().map_or(0, |s| s.keys);
        let (line, refused) = match self
            .policy
            .admit(c.origin, liveness, &classed, curves_only, sample_keys)
        {
            Ok(()) => (Some(attach_derivation(&body, &d)?), None),
            Err(e) => {
                let rec = json!({"body_sha256": ids.body_sha256, "tag": c.tag, "reason": e, "derivation": d});
                let p = self.side.join("refused.jsonl");
                std::fs::create_dir_all(&self.side).map_err(|e| e.to_string())?;
                let mut f = std::fs::OpenOptions::new()
                    .create(true)
                    .append(true)
                    .open(&p)
                    .map_err(|e| format!("{}: {e}", p.display()))?;
                use std::io::Write;
                writeln!(f, "{rec}").map_err(|e| e.to_string())?;
                (None, Some(e))
            }
        };
        Ok(Emission {
            line,
            refused,
            derivation: d,
            classed,
            ids,
            summary,
            check: check_run,
        })
    }
}
