//! THE EMITTER: the only writer of rule records under the emission contract (plan/auto-campaign/interfaces/
//! emission-check.md section 3). For one candidate: the body (no `derivation`, index = its position), the independent
//! checker run on EXACTLY these bytes (`[header, body]` at an offset) with the certificate of THIS lift, the binding of
//! the checker's output to the body (relation, domain, certificate sha256), the class from `class_of`, the policy, the
//! `derivation` member, and the side files (content-addressed: certificate, check output, provenance, policy).
use super::child::{self, ChildRun};
use rustred_campaign_core::emission::body::{
    BodyIds, attach_derivation, body_ids, sha256_hex, strip_derivation,
};
use rustred_campaign_core::emission::class::{
    CheckSummary, Classed, ConeEvidence, Kind, Liveness, Origin, Policy, SampleEvidence, class_of,
    span_exact,
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
    /// the engine binary of the DESCENT test (`rr_node descent`) and its sha256 (contract section 14)
    pub descent_node: Option<(PathBuf, String)>,
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
            descent_node: None,
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
        Err(format!(
            "budget_exceeded ({:.1} s > {budget_s:.1} s)",
            run.seconds
        ))
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

/// The key of a POINT record (every slot fixed in `fixed`), as instance-span's key line over all slots; None when a
/// slot is free (a parametric record has no single key: its exactness is per key only).
pub fn point_key(body: &str) -> Option<Value> {
    let v: Value = serde_json::from_str(body).ok()?;
    let fixed = v["fixed"].as_array()?;
    let mut key = serde_json::Map::new();
    for (i, x) in fixed.iter().enumerate() {
        key.insert(format!("n{i}"), json!(x.as_i64()?));
    }
    Some(json!({"key": Value::Object(key)}))
}

/// Run the checker in INSTANCE-SPAN mode on `[header, body]` at the record's own key (point / symmetry records):
/// returns (output line, its exactness bound to the body, the child run).
pub fn run_span_checker(
    checker: &Checker,
    header: &str,
    body: &str,
    dir: &Path,
    budget_s: f64,
) -> Result<(Option<String>, Result<bool, String>, ChildRun), String> {
    run_span_checker_with(checker, header, body, dir, budget_s, None)
}

/// [`run_span_checker`] with a SYMMETRY-RELATION request (contract section 15): `Some((request, cert_dir))` adds
/// `--symrows request --symrows-only --emit-certs cert_dir` (the checker's `--symmetry-group` is required; the kernel
/// writes `cert_dir/cert_<key>.json` per exact key; the directory is emptied first).
pub fn run_span_checker_with(
    checker: &Checker,
    header: &str,
    body: &str,
    dir: &Path,
    budget_s: f64,
    symrows: Option<(&Path, &Path)>,
) -> Result<(Option<String>, Result<bool, String>, ChildRun), String> {
    std::fs::create_dir_all(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    let input = dir.join("check-input.table.jsonl");
    let header = header.trim_end();
    std::fs::write(&input, format!("{header}\n{body}\n"))
        .map_err(|e| format!("{}: {e}", input.display()))?;
    let keys = dir.join("keys.jsonl");
    let Some(k) = point_key(body) else {
        return Err("not a point record (a free slot): its exactness is per key only".into());
    };
    std::fs::write(&keys, format!("{k}\n")).map_err(|e| format!("{}: {e}", keys.display()))?;
    let index = serde_json::from_str::<Value>(body)
        .ok()
        .and_then(|v| v["index"].as_i64())
        .unwrap_or(0);
    let mut args: Vec<String> = vec![
        "--mode".into(),
        "instance-span".into(),
        "--table".into(),
        input.display().to_string(),
        "--rule".into(),
        index.to_string(),
        "--offset".into(),
        (header.len() + 1).to_string(),
        "--keys".into(),
        keys.display().to_string(),
    ];
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
    if let Some((req, cert)) = symrows {
        if checker.symmetry_group.is_none() {
            return Err("a symmetry relation needs the symmetry group (--symmetry-group / emission.symmetry_group): contract section 15".into());
        }
        if cert.exists() {
            std::fs::remove_dir_all(cert).map_err(|e| format!("{}: {e}", cert.display()))?;
        }
        for a in ["--symrows", &req.display().to_string(), "--symrows-only", "--emit-certs", &cert.display().to_string()] {
            args.push(a.to_string());
        }
    }
    let run = child::run(&checker.binary, &args, &[], dir, "span", budget_s)?;
    let output = run
        .stdout_text()
        .lines()
        .rev()
        .find(|l| l.trim_start().starts_with('{'))
        .map(str::to_string);
    let exact = if run.over_budget {
        Err(format!("budget_exceeded ({:.1} s > {budget_s:.1} s)", run.seconds))
    } else {
        match &output {
            None => Err(format!(
                "checker_error (exit {:?}, no output: {})",
                run.code,
                run.stderr_tail(300)
            )),
            Some(o) => span_exact(o, body),
        }
    };
    Ok((output, exact, run))
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
    pub fn new(
        checker: Checker,
        policy: Policy,
        side: &Path,
        work: &Path,
    ) -> Result<Emitter, String> {
        // the policy is judged under the CURRENT rule (version 2 = rule 21: d = 4 regularity reported, tie-break only),
        // whatever version a config states; the stored policy (named by every derivation) says so
        let mut policy = policy;
        policy.version = rustred_campaign_core::emission::class::POLICY_VERSION;
        let text = serde_json::to_vec(&policy).map_err(|e| e.to_string())?;
        // ABSOLUTE paths: the children run in their own work directory, so a relative --work / side named files they could
        // not find (round 6: emit-table --work work failed with descent_not_checked)
        let abs = |p: &Path| std::path::absolute(p).unwrap_or_else(|_| p.to_path_buf());
        let e = Emitter {
            checker,
            policy,
            side: abs(side),
            work: abs(work),
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

    /// THE DESCENT TEST of a record (contract section 14): `rr_node descent --table <header + body>` (the engine's own test,
    /// the order of the table header), its output stored beside the table and bound to the body. Returns the derivation's
    /// descent tuple and, when the record does not descend or could not be tested, the refusal reason.
    #[allow(clippy::type_complexity)]
    pub fn descent(
        &self,
        header: &str,
        body: &str,
        tag: &str,
    ) -> Result<(Option<(String, String, String, Option<String>)>, Option<String>), String> {
        let Some((node, node_sha)) = &self.checker.descent_node else {
            return Ok((None, Some("descent_not_checked (no engine descent binary given: contract section 14)".into())));
        };
        let dir = self.work.join(tag).join("descent");
        std::fs::create_dir_all(&dir).map_err(|e| format!("{}: {e}", dir.display()))?;
        let input = dir.join("descent-input.table.jsonl");
        std::fs::write(&input, format!("{}\n{body}\n", header.trim_end())).map_err(|e| format!("{}: {e}", input.display()))?;
        let args: Vec<String> = vec!["descent".into(), "--table".into(), input.display().to_string()];
        let run = child::run(node, &args, &[], &dir, "descent", self.checker.floor_s.max(600.0))?;
        let out = run.stdout_text();
        if run.over_budget || out.lines().all(|l| !l.trim_start().starts_with('{')) {
            return Ok((None, Some(format!("descent_not_checked (rr_node descent: exit {:?}, {})", run.code, run.stderr_tail(300)))));
        }
        let line = out.lines().rev().find(|l| l.trim_start().starts_with('{')).unwrap_or("").to_string();
        let sha = self.put_side(rustred_campaign_core::emission::AREA_DESCENT, line.as_bytes())?;
        let d = rustred_campaign_core::emission::descent::parse(&line)?;
        let tuple = Some((node_sha.clone(), sha, d.word().to_string(), d.first.clone()));
        match rustred_campaign_core::emission::descent::verify(&line, body) {
            Ok(true) => Ok((tuple, None)),
            Ok(false) => Ok((tuple, Some(format!("not_lower ({})", d.first.unwrap_or_default())))),
            Err(e) => Ok((tuple, Some(format!("descent_not_checked ({e})")))),
        }
    }

    /// The symmetry-relation evidence (contract section 15): the request, the group and the certificate stored beside the
    /// table and bound to the record (`symrows::verify`). Returns the check's `symrows` member and the certificate part.
    fn bind_symrows(
        &self,
        out: &str,
        req: &Path,
        cert: &Path,
        body: &str,
        e: u64,
    ) -> Result<(Value, (String, u64, String)), String> {
        use rustred_campaign_core::emission::{AREA_SYMROWS, symrows};
        let rq = std::fs::read(req).map_err(|x| format!("{}: {x}", req.display()))?;
        // the kernel's certificate of the record's one key: exactly one cert_*.json in the directory
        let certs: Vec<PathBuf> = std::fs::read_dir(cert)
            .map_err(|x| format!("no certificate emitted ({}: {x})", cert.display()))?
            .filter_map(|e| e.ok().map(|e| e.path()))
            .filter(|p| p.file_name().and_then(|n| n.to_str()).is_some_and(|n| n.starts_with("cert_") && n.ends_with(".json")))
            .collect();
        let [one] = certs.as_slice() else {
            return Err(format!("{} certificate(s) in {} (one key: one expected)", certs.len(), cert.display()));
        };
        let ct = std::fs::read(one).map_err(|x| format!("{}: {x}", one.display()))?;
        symrows::verify(out, &ct, rq.trim_ascii_end(), body)?;
        let group = self.checker.symmetry_group.as_ref().ok_or("no symmetry group")?;
        let gb = std::fs::read(group).map_err(|x| format!("{}: {x}", group.display()))?;
        let rows = serde_json::from_str::<Value>(out).ok().and_then(|o| o["symrows"]["given"].as_u64()).unwrap_or(0);
        let member = json!({"element": e, "request_sha256": self.put_side(AREA_SYMROWS, rq.trim_ascii_end())?,
            "group_sha256": self.put_side(AREA_SYMROWS, &gb)?, "only": true, "rows": rows});
        let cert_sha = self.put_side(AREA_CERT, &ct)?;
        Ok((member, (cert_sha, ct.len() as u64, "span-cert".into())))
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
        run_checker(
            &self.checker,
            header,
            body,
            cert,
            &self.work.join(tag),
            budget_s,
        )
    }

    /// EMIT one candidate: check, class, policy, derivation. `liveness` of the record at its position (Unknown =
    /// live); `curves_only`: every open locus of the check is a curve the ladder cannot branch.
    pub fn emit(
        &self,
        c: &Candidate,
        liveness: Liveness,
        curves_only: bool,
    ) -> Result<Emission, String> {
        let (body, _) = strip_derivation(&c.line)?;
        let ids = body_ids(&body)?;
        let mut certificate = None;
        let mut check_run = None;
        // THE DESCENT TEST (contract section 14): the engine's own `rr_node descent` on exactly these bytes, in the order of
        // the table header; every rhs term must be LOWER than the target. Not run (no engine binary) = refused as well.
        let (descent, descent_refusal) = self.descent(&c.header, &body, &c.tag)?;
        // POINT / SYMMETRY records: the instance-span check at the record's own key (no certificate)
        let point = matches!(c.kind, Kind::Point | Kind::Symmetry);
        let mut span: Option<(String, String, String)> = None;
        let mut exact_at_key: Option<bool> = None;
        let mut span_err: Option<String> = None;
        // a SYMMETRY RELATION (the record names its element: contract section 15): proven on the expanded symmetry row
        // alone (--symrows --symrows-only), its exact certificate bound to the record bytes
        let sym_el = if point { rustred_campaign_core::emission::symrows::element(&body) } else { None };
        let sym_files = match sym_el {
            Some(e) => {
                let d = self.work.join(&c.tag);
                std::fs::create_dir_all(&d).map_err(|x| format!("{}: {x}", d.display()))?;
                let req = d.join("symrows.request.jsonl");
                std::fs::write(&req, format!("{}\n", rustred_campaign_core::emission::symrows::request(&body, e)?))
                    .map_err(|x| format!("{}: {x}", req.display()))?;
                Some((req, d.join("span-certs")))
            }
            None => None,
        };
        let mut symrows_member: Option<Value> = None;
        if point {
            match run_span_checker_with(
                &self.checker,
                &c.header,
                &body,
                &self.work.join(&c.tag),
                self.checker.budget(c.lift_seconds),
                sym_files.as_ref().map(|(r, k)| (r.as_path(), k.as_path())),
            ) {
                Ok((Some(o), exact, _)) => {
                    let out = self.put_side(AREA_CHECK, o.as_bytes())?;
                    let exact = match (&sym_files, exact) {
                        (Some((req, cert)), Ok(true)) => match self.bind_symrows(&o, req, cert, &body, sym_el.unwrap_or(0)) {
                            Ok((member, cert_side)) => {
                                symrows_member = Some(member);
                                certificate = Some(cert_side);
                                Ok(true)
                            }
                            Err(e) => Err(format!("symrows_binding ({e})")),
                        },
                        (_, x) => x,
                    };
                    let verdict = serde_json::from_str::<Value>(&o)
                        .ok()
                        .and_then(|v| v["verdict"].as_str().map(str::to_string))
                        .unwrap_or_default();
                    match exact {
                        Ok(x) => {
                            exact_at_key = Some(x);
                            span = Some((self.checker.sha256.clone(), out, verdict));
                        }
                        Err(e) => span_err = Some(e),
                    }
                }
                Ok((None, exact, _)) => span_err = Some(exact.err().unwrap_or_default()),
                Err(e) => span_err = Some(e),
            }
        }
        let check: Result<(String, String, CheckSummary), String> = match &c.cert {
            _ if point => Err(span_err
                .clone()
                .unwrap_or_else(|| "point / symmetry record: instance-span check".into())),
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
                let r = self.check(
                    &c.header,
                    &body,
                    cert,
                    self.checker.budget(c.lift_seconds),
                    &c.tag,
                )?;
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
            exact_at_key,
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
            span,
            descent,
            symrows: symrows_member,
            classed: classed.clone(),
            cone: c.cone.clone(),
            sample: c.sample.clone(),
            ladder: c.ladder.clone(),
            legacy: None,
            policy: (self.policy.name.clone(), self.policy_sha.clone()),
        };
        let d = derivation(&parts);
        let sample_keys = c.sample.as_ref().map_or(0, |s| s.keys);
        let admitted = match &descent_refusal {
            Some(why) => Err(why.clone()),
            None => self.policy.admit(c.origin, liveness, &classed, curves_only, sample_keys),
        };
        let (line, refused) = match admitted {
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
