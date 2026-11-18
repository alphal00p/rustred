//! CERTIFY BY REPLACEMENT (emission contract section 5; TRICKS 68): re-lift a rule's case under the table's options with
//! weight export, EMIT every piece through the emitter (check on the piece's own bytes), turn each branchable OPEN locus
//! and each new exception branch into a child case covered by the next piece (bounded depth), prove that the pieces
//! partition the replaced record's domain, and return the pieces as positioned lines. Nothing is written unless the
//! ladder CLOSES with every piece admitted by the policy; every other end is a typed outcome.
use super::child;
use super::emitter::{Candidate, Emission, Emitter};
use rustred_campaign_core::emission::body::sha256_hex;
use rustred_campaign_core::emission::class::{ConeEvidence, Kind, Liveness, Origin};
use rustred_campaign_core::emission::ladder::{
    BranchPlan, PartCheck, branch_key, branch_texts, branches_of, child_record, fixed_of,
    locus_keys_in_simplex, open_locus_branch, plan_branch, prove_partition, union_branches,
};
use serde_json::{Value, json};
use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

/// One lift of a case by the producer.
#[derive(Clone, Debug)]
pub struct Lift {
    /// the lifted rule record (the producer's output line, parsed)
    pub record: Value,
    /// the certificate files the lift wrote (the checker decides which binds)
    pub certs: Vec<PathBuf>,
    pub seconds: f64,
    pub peak_mb: f64,
    pub binary_sha256: String,
    pub provenance: Value,
}

/// A producer of lifts (the engine's `rr_node witness` in production; a scripted one in tests).
pub trait Lifter {
    fn lift(&self, header: &str, case: &Value, dir: &Path) -> Result<Lift, String>;
}

/// `rr_node witness` with weight-certificate export (engine notes WEIGHT-CERT.md: RUSTRED_WEIGHT_CERT_DIR / _CHECK / _RULE /
/// _ROWS).
pub struct RrNodeWitness {
    pub binary: PathBuf,
    pub sha256: String,
    pub case_budget_s: u64,
    pub wall_budget_s: f64,
    pub rows: bool,
    pub extra_env: Vec<(String, String)>,
    /// the family file when the table header carries no `family_source`
    pub family: Option<PathBuf>,
}

impl RrNodeWitness {
    pub fn new(binary: &Path) -> Result<RrNodeWitness, String> {
        Ok(RrNodeWitness {
            sha256: child::file_sha256(binary)?,
            binary: binary.to_path_buf(),
            case_budget_s: 7200,
            wall_budget_s: 4.0 * 3600.0,
            rows: true,
            extra_env: vec![],
            family: None,
        })
    }
}

impl Lifter for RrNodeWitness {
    fn lift(&self, header: &str, case: &Value, dir: &Path) -> Result<Lift, String> {
        std::fs::create_dir_all(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
        let idx = case["index"].as_i64().ok_or("case without index")?;
        let input = dir.join("in.table.jsonl");
        std::fs::write(&input, format!("{}\n{}\n", header.trim_end(), case))
            .map_err(|e| format!("{}: {e}", input.display()))?;
        let cert_dir = dir.join("cert");
        let out = dir.join("w");
        let mut env: Vec<(String, String)> = vec![
            (
                "RUSTRED_WEIGHT_CERT_DIR".into(),
                cert_dir.display().to_string(),
            ),
            ("RUSTRED_WEIGHT_CERT_CHECK".into(), "1".into()),
            ("RUSTRED_WEIGHT_CERT_RULE".into(), idx.to_string()),
        ];
        if self.rows {
            env.push(("RUSTRED_WEIGHT_CERT_ROWS".into(), "1".into()));
        }
        env.extend(self.extra_env.iter().cloned());
        let mut args: Vec<String> = vec![
            "witness".into(),
            "--table".into(),
            input.display().to_string(),
            "--rules".into(),
            idx.to_string(),
            "--out".into(),
            out.display().to_string(),
            "--case-budget".into(),
            self.case_budget_s.to_string(),
        ];
        if let Some(f) = &self.family {
            args.push("--family".into());
            args.push(f.display().to_string());
        }
        let run = child::run(
            &self.binary,
            &args,
            &env,
            dir,
            "witness",
            self.wall_budget_s,
        )?;
        if !run.ok() {
            return Err(format!(
                "rr_node witness exit {:?}{}: {}",
                run.code,
                if run.over_budget {
                    " (over budget)"
                } else {
                    ""
                },
                run.stderr_tail(400)
            ));
        }
        let table = out.join(format!("r{idx}")).join("F.table.jsonl");
        let text =
            std::fs::read_to_string(&table).map_err(|e| format!("{}: {e}", table.display()))?;
        let record: Value = text
            .lines()
            .filter_map(|l| serde_json::from_str::<Value>(l).ok())
            .find(|v| v["type"] == "rule")
            .ok_or_else(|| format!("{}: no rule record", table.display()))?;
        let mut certs: Vec<PathBuf> = std::fs::read_dir(&cert_dir)
            .map_err(|e| format!("{}: {e}", cert_dir.display()))?
            .filter_map(|e| e.ok().map(|e| e.path()))
            .filter(|p| p.extension().is_some_and(|x| x == "json"))
            .collect();
        certs.sort();
        // the certificate of the FINAL rule first when the producer marks it (engine fix E1), else every file
        if let Some(k) = certs
            .iter()
            .position(|p| p.file_name().is_some_and(|n| n == "cert-final.json"))
        {
            let f = certs.remove(k);
            certs.insert(0, f);
        }
        let witness = std::fs::read_to_string(out.join("WITNESS.jsonl")).unwrap_or_default();
        let provenance = json!({"tool": "rr_node witness", "binary": self.binary.display().to_string(),
            "binary_sha256": self.sha256, "argv": args, "env": env.iter().filter(|(k, _)| k.starts_with("RUSTRED_")).map(|(k, v)| format!("{k}={v}")).collect::<Vec<_>>(),
            "header_sha256": sha256_hex(header.trim_end().as_bytes()), "case_sha256": sha256_hex(case.to_string().as_bytes()),
            "witness": witness.lines().next().and_then(|l| serde_json::from_str::<Value>(l).ok()),
            "lift_seconds": run.seconds, "lift_peak_mb": run.peak_mb});
        Ok(Lift {
            record,
            certs,
            seconds: run.seconds,
            peak_mb: run.peak_mb,
            binary_sha256: self.sha256.clone(),
            provenance,
        })
    }
}

/// Ladder bounds and options.
#[derive(Clone, Debug)]
pub struct LadderConfig {
    pub max_depth: u32,
    pub max_pieces: usize,
    /// domain witness options of the partition proof (`[false, true]` for real tables)
    pub witness_modes: Vec<bool>,
    /// the excess simplex `D + r <= s` in which curve loci are enumerated (None: a curve ends the ladder)
    pub cone_s: Option<i64>,
    /// line slots of the frame (from the header's mask), for the curve enumeration
    pub is_line: Vec<bool>,
    pub curve_cap: u64,
}

impl Default for LadderConfig {
    fn default() -> Self {
        LadderConfig {
            max_depth: 6,
            max_pieces: 16,
            witness_modes: vec![false, true],
            cone_s: None,
            is_line: vec![],
            curve_cap: 50_000_000,
        }
    }
}

/// One emitted piece.
#[derive(Clone, Debug)]
pub struct Piece {
    pub tag: String,
    pub depth: u32,
    pub emission: Emission,
    pub record: Value,
    pub lift_seconds: f64,
    pub lift_peak_mb: f64,
    pub check_seconds: f64,
    pub check_peak_mb: f64,
    pub rhs_terms: usize,
    pub bytes: usize,
    pub certs_tried: usize,
}

/// The typed end of a ladder.
#[derive(Clone, Debug, PartialEq)]
pub enum Outcome {
    Closed,
    DepthExceeded { open: Vec<String> },
    TooManyPieces,
    CurveLocus { factor: String, keys: u64 },
    LiftFailed { piece: String, reason: String },
    CheckFailed { piece: String, reason: String },
    PartitionFailed,
}

impl Outcome {
    pub fn json(&self) -> Value {
        match self {
            Outcome::Closed => json!({"outcome": "closed"}),
            Outcome::DepthExceeded { open } => json!({"outcome": "depth_exceeded", "open": open}),
            Outcome::TooManyPieces => json!({"outcome": "too_many_pieces"}),
            Outcome::CurveLocus { factor, keys } => {
                json!({"outcome": "curve_locus", "factor": factor, "keys_in_cone": keys})
            }
            Outcome::LiftFailed { piece, reason } => {
                json!({"outcome": "lift_failed", "piece": piece, "reason": reason})
            }
            Outcome::CheckFailed { piece, reason } => {
                json!({"outcome": "check_failed", "piece": piece, "reason": reason})
            }
            Outcome::PartitionFailed => json!({"outcome": "partition_failed"}),
        }
    }
}

/// The ladder's result.
pub struct Report {
    pub outcome: Outcome,
    pub pieces: Vec<Piece>,
    pub partition: Option<(bool, Vec<PartCheck>)>,
    pub log: Vec<Value>,
}

impl Report {
    /// The piece lines positioned at `first_index` (consecutive), only when the ladder closed.
    pub fn lines(&self, first_index: i64) -> Result<Vec<String>, String> {
        if self.outcome != Outcome::Closed {
            return Err(format!("the ladder did not close: {}", self.outcome.json()));
        }
        self.pieces
            .iter()
            .enumerate()
            .map(|(k, p)| {
                let line = p.emission.line.as_ref().ok_or("a piece was refused")?;
                rustred_campaign_core::economy::improve::set_index(line, first_index + k as i64)
            })
            .collect()
    }
}

struct Ctx<'a> {
    header: &'a str,
    original: &'a Value,
    old_index: i64,
    lifter: &'a dyn Lifter,
    emitter: &'a Emitter,
    cfg: &'a LadderConfig,
    work: &'a Path,
    e_orig: Vec<Vec<String>>,
    pieces: Vec<Piece>,
    log: Vec<Value>,
    attempts: usize,
}

/// The piece body: the lifted record with the piece's case, the original's domain bounds and the full exception set.
fn piece_body(
    lifted: &Value,
    case: &Value,
    exc: &[Vec<String>],
    original: &Value,
    index: i64,
) -> Value {
    let mut r = lifted.clone();
    r["type"] = json!("rule");
    r["index"] = json!(index);
    r["fixed"] = case["fixed"].clone();
    r["equations"] = case["equations"].clone();
    r["exceptions"] = json!(exc);
    r["domain_bounds"] = original["domain_bounds"].clone();
    if let Some(o) = r.as_object_mut() {
        o.remove("derivation");
    }
    r
}

/// Emit one piece against each certificate of its lift until one binds; the emission of the last try otherwise.
fn emit_piece(
    x: &Ctx<'_>,
    body: &Value,
    lift: &Lift,
    tag: &str,
    curves_only: bool,
    cone: Option<ConeEvidence>,
    ladder: &Value,
) -> Result<(Emission, usize), String> {
    let mut last = None;
    let certs: Vec<Option<&PathBuf>> = if lift.certs.is_empty() {
        vec![None]
    } else {
        lift.certs.iter().map(Some).collect()
    };
    let n = certs.len();
    for (k, cert) in certs.into_iter().enumerate() {
        let c = Candidate {
            header: x.header.to_string(),
            line: body.to_string(),
            kind: Kind::Lifted,
            origin: Origin::Lift,
            cert: cert.cloned(),
            provenance: Some((lift.binary_sha256.clone(), lift.provenance.clone())),
            lift_seconds: lift.seconds,
            ladder: Some(ladder.clone()),
            cone: cone.clone(),
            sample: None,
            tag: format!("{tag}/c{k}"),
        };
        let e = x.emitter.emit(&c, Liveness::Unknown, curves_only)?;
        let binds = e.summary.as_ref().is_some_and(|s| s.binding_equal());
        if binds {
            return Ok((e, k + 1));
        }
        last = Some(e);
    }
    Ok((last.expect("at least one try"), n))
}

fn run_piece(
    x: &mut Ctx<'_>,
    case: &Value,
    inherited: &[Vec<String>],
    depth: u32,
    tag: &str,
) -> Result<Option<Outcome>, String> {
    if x.pieces.len() >= x.cfg.max_pieces {
        return Ok(Some(Outcome::TooManyPieces));
    }
    let k = x.attempts;
    x.attempts += 1;
    let dir = x.work.join(format!("p{k}"));
    let mut case = case.clone();
    case["index"] = json!(900_000 + k as i64);
    let lift = match x.lifter.lift(x.header, &case, &dir) {
        Ok(l) => l,
        Err(e) => {
            return Ok(Some(Outcome::LiftFailed {
                piece: tag.into(),
                reason: e,
            }));
        }
    };
    let e_new = branches_of(&lift.record);
    let fixed = fixed_of(&case);
    let mut exc = union_branches(&[&x.e_orig, inherited, &e_new]);
    let mut open_branches: Vec<Vec<String>> = vec![];
    let mut curves: Vec<(String, Vec<String>)> = vec![];
    let ladder_info = json!({"replaces": {"index": x.old_index, "body_sha256": rustred_campaign_core::emission::body::body_ids(&x.original.to_string()).map(|i| i.body_sha256).unwrap_or_default()},
        "piece_tag": tag, "depth": depth});
    // emit; every branchable OPEN locus becomes a branch of THIS piece and the piece is checked again on its new bytes;
    // when only curves stay open and a cone is given, their keys in the cone are enumerated and the piece re-emitted
    let mut cone: Option<ConeEvidence> = None;
    let mut rounds = 0;
    let (mut em, mut tried);
    loop {
        let body = piece_body(&lift.record, &case, &exc, x.original, x.old_index);
        (em, tried) = emit_piece(
            x,
            &body,
            &lift,
            &format!("{tag}-r{rounds}"),
            !curves.is_empty(),
            cone.clone(),
            &ladder_info,
        )?;
        rounds += 1;
        let Some(s) = em.summary.clone() else { break };
        if rounds > 8 {
            break;
        }
        let mut added = false;
        let mut new_curves = vec![];
        for o in &s.open {
            match open_locus_branch(&o.coefficients, &fixed)? {
                p @ (BranchPlan::Fix(_) | BranchPlan::Equation(_)) => {
                    let b = branch_texts(&p).expect("fix / equation");
                    if !exc.iter().any(|e| branch_key(e) == branch_key(&b)) {
                        exc.push(b.clone());
                        open_branches.push(b);
                        added = true;
                    }
                }
                BranchPlan::Empty => {}
                BranchPlan::Curve(g) => new_curves.push((o.factor.clone(), g)),
            }
        }
        if added {
            curves.clear();
            cone = None;
            continue;
        }
        if !new_curves.is_empty() && cone.is_none() && x.cfg.cone_s.is_some() {
            curves = new_curves;
            cone = curve_evidence(x, &body, &curves)?;
            continue;
        }
        curves = new_curves;
        break;
    }
    let record: Value = em
        .line
        .as_ref()
        .and_then(|l| serde_json::from_str(l).ok())
        .unwrap_or_else(|| piece_body(&lift.record, &case, &exc, x.original, x.old_index));
    let check_run = em.check.clone();
    let rhs_terms = record["rhs"].as_array().map_or(0, Vec::len);
    let bytes = em.line.as_ref().map_or(0, String::len);
    x.log.push(json!({"piece": tag, "depth": depth, "case_fixed": case["fixed"], "equations": case["equations"],
        "lift_seconds": lift.seconds, "lift_peak_mb": lift.peak_mb, "certificates": lift.certs.len(), "certs_tried": tried,
        "check_seconds": check_run.as_ref().map(|c| c.run.seconds), "check_peak_mb": check_run.as_ref().map(|c| c.run.peak_mb),
        "class": em.classed.class.word(), "reason": em.classed.reason, "refused": em.refused,
        "stages": em.summary.as_ref().map(|s| s.record_json()), "new_branches_of_lift": e_new, "open_branches": open_branches,
        "curves": curves.iter().map(|c| c.0.clone()).collect::<Vec<_>>(), "rhs_terms": rhs_terms, "bytes": bytes}));
    if em.line.is_none() {
        if let Some((f, _)) = curves.first() {
            return Ok(Some(Outcome::CurveLocus {
                factor: f.clone(),
                keys: x
                    .log
                    .last()
                    .and_then(|l| l["curve_keys"].as_u64())
                    .unwrap_or(0),
            }));
        }
        return Ok(Some(Outcome::CheckFailed {
            piece: tag.into(),
            reason: em.refused.clone().unwrap_or_default(),
        }));
    }
    x.pieces.push(Piece {
        tag: tag.into(),
        depth,
        record,
        lift_seconds: lift.seconds,
        lift_peak_mb: lift.peak_mb,
        check_seconds: check_run.as_ref().map_or(0.0, |c| c.run.seconds),
        check_peak_mb: check_run.as_ref().map_or(0.0, |c| c.run.peak_mb),
        rhs_terms,
        bytes,
        certs_tried: tried,
        emission: em,
    });
    // children: every NEW branch of this piece (the lift's own and the open loci) not known above
    let known: BTreeSet<String> = x
        .e_orig
        .iter()
        .chain(inherited.iter())
        .map(|b| branch_key(b))
        .collect();
    let mut news: Vec<Vec<String>> = vec![];
    for b in e_new.iter().chain(open_branches.iter()) {
        if !known.contains(&branch_key(b)) && !news.iter().any(|n| branch_key(n) == branch_key(b)) {
            news.push(b.clone());
        }
    }
    let mut children = vec![];
    for b in &news {
        match plan_branch(b, &fixed)? {
            BranchPlan::Empty => {}
            BranchPlan::Curve(g) => {
                return Ok(Some(Outcome::CurveLocus {
                    factor: g.join("&"),
                    keys: 0,
                }));
            }
            p => children.push((b.clone(), p)),
        }
    }
    if !children.is_empty() && depth >= x.cfg.max_depth {
        return Ok(Some(Outcome::DepthExceeded {
            open: children.iter().map(|(b, _)| b.join("&")).collect(),
        }));
    }
    let mut done: Vec<Vec<String>> = vec![];
    for (b, plan) in children {
        let inh: Vec<Vec<String>> = inherited
            .iter()
            .cloned()
            .chain(done.iter().cloned())
            .collect();
        let child = child_record(&case, &plan, &inh)?;
        if let Some(o) = run_piece(
            x,
            &child,
            &inh,
            depth + 1,
            &format!("{tag}/{}", b.join("&")),
        )? {
            return Ok(Some(o));
        }
        done.push(b);
    }
    Ok(None)
}

/// Cone evidence for the curve loci of a piece: their keys in the cone (enumerated); complete and with zero keys, the
/// cone class needs no per-key proof (keys > 0 need per-key proofs: not run here, the evidence stays unproven).
fn curve_evidence(
    x: &mut Ctx<'_>,
    body: &Value,
    curves: &[(String, Vec<String>)],
) -> Result<Option<ConeEvidence>, String> {
    let Some(s) = x.cfg.cone_s else {
        return Ok(None);
    };
    let mut all = vec![];
    let mut complete = true;
    for (_, g) in curves {
        let (keys, c, _) = locus_keys_in_simplex(body, g, &x.cfg.is_line, s, x.cfg.curve_cap)?;
        complete &= c;
        all.extend(keys);
    }
    all.sort();
    all.dedup();
    let text: String = all
        .iter()
        .map(|k| format!("{}\n", serde_json::to_string(k).unwrap_or_default()))
        .collect();
    let sha = x
        .emitter
        .put_side(rustred_campaign_core::emission::AREA_KEYS, text.as_bytes())?;
    if let Some(l) = x.log.last_mut() {
        l["curve_keys"] = json!(all.len());
    }
    Ok(Some(ConeEvidence {
        cone: format!("simplex:{s}"),
        scope: "open-locus keys of the domain".into(),
        keys: all.len() as u64,
        keys_sha256: sha,
        complete,
        proven: 0,
        position: None,
        prefix_sha256: None,
    }))
}

#[allow(clippy::too_many_arguments)]
/// Run the ladder on `original` (a rule record of the table whose header is `header`, at index `old_index`).
pub fn certify_by_replacement(
    header: &str,
    charts: &[String],
    original: &Value,
    old_index: i64,
    lifter: &dyn Lifter,
    emitter: &Emitter,
    cfg: &LadderConfig,
    work: &Path,
) -> Result<Report, String> {
    std::fs::create_dir_all(work).map_err(|e| format!("{}: {e}", work.display()))?;
    let mut x = Ctx {
        header,
        original,
        old_index,
        lifter,
        emitter,
        cfg,
        work,
        e_orig: branches_of(original),
        pieces: vec![],
        log: vec![],
        attempts: 0,
    };
    let mut case = original.clone();
    if let Some(o) = case.as_object_mut() {
        o.remove("derivation");
    }
    let outcome = run_piece(&mut x, &case, &[], 0, "case")?;
    let mut partition = None;
    let outcome = match outcome {
        Some(o) => o,
        None => {
            let recs: Vec<Value> = x.pieces.iter().map(|p| p.record.clone()).collect();
            let tmp = work.join("partition");
            std::fs::create_dir_all(&tmp).map_err(|e| e.to_string())?;
            let (ok, checks) =
                prove_partition(header, charts, original, &recs, &tmp, &cfg.witness_modes)?;
            partition = Some((ok, checks));
            if ok {
                Outcome::Closed
            } else {
                Outcome::PartitionFailed
            }
        }
    };
    Ok(Report {
        outcome,
        pieces: x.pieces,
        partition,
        log: x.log,
    })
}

/// GIVEN PIECES (a candidate sub-table produced elsewhere, e.g. by an improve run): every piece is lifted again by the
/// producer from its OWN record (its case), and the GIVEN piece's bytes are checked with that lift's certificate (binding
/// equal only when the producer reproduces the piece's relation); open loci are reported, not branched (the candidate is
/// what it is); then the partition proof against the replaced record. Typed outcome as the ladder's.
#[allow(clippy::too_many_arguments)]
pub fn certify_given_pieces(
    header: &str,
    charts: &[String],
    original: &Value,
    old_index: i64,
    given: &[Value],
    lifter: &dyn Lifter,
    emitter: &Emitter,
    cfg: &LadderConfig,
    work: &Path,
) -> Result<Report, String> {
    std::fs::create_dir_all(work).map_err(|e| format!("{}: {e}", work.display()))?;
    let x = Ctx {
        header,
        original,
        old_index,
        lifter,
        emitter,
        cfg,
        work,
        e_orig: branches_of(original),
        pieces: vec![],
        log: vec![],
        attempts: 0,
    };
    let mut pieces = vec![];
    let mut log = vec![];
    let mut outcome = None;
    for (k, p) in given.iter().enumerate() {
        let tag = format!("given{k}");
        let mut case = p.clone();
        case["index"] = json!(900_000 + k as i64);
        if let Some(o) = case.as_object_mut() {
            o.remove("derivation");
        }
        let lift = match lifter.lift(header, &case, &work.join(format!("p{k}"))) {
            Ok(l) => l,
            Err(e) => {
                outcome = Some(Outcome::LiftFailed {
                    piece: tag.clone(),
                    reason: e,
                });
                break;
            }
        };
        let mut body = p.clone();
        body["type"] = json!("rule");
        body["index"] = json!(old_index + k as i64);
        if let Some(o) = body.as_object_mut() {
            o.remove("derivation");
        }
        let same = rustred_campaign_core::emission::body::body_ids(&body.to_string())
            .ok()
            .map(|i| i.relation_sha256)
            == rustred_campaign_core::emission::body::body_ids(&lift.record.to_string())
                .ok()
                .map(|i| i.relation_sha256);
        let ladder_info =
            json!({"replaces": {"index": old_index}, "piece_tag": tag, "given": true});
        let (em, tried) = emit_piece(&x, &body, &lift, &tag, false, None, &ladder_info)?;
        let check_run = em.check.clone();
        let rhs_terms = body["rhs"].as_array().map_or(0, Vec::len);
        log.push(json!({"piece": tag, "index": old_index + k as i64, "fixed": body["fixed"], "equations": body["equations"],
            "exceptions": body["exceptions"], "producer_reproduced_the_piece": same, "lift_seconds": lift.seconds,
            "lift_peak_mb": lift.peak_mb, "certificates": lift.certs.len(), "certs_tried": tried,
            "check_seconds": check_run.as_ref().map(|c| c.run.seconds), "check_peak_mb": check_run.as_ref().map(|c| c.run.peak_mb),
            "class": em.classed.class.word(), "reason": em.classed.reason, "refused": em.refused,
            "stages": em.summary.as_ref().map(|s| s.record_json()),
            "open_loci": em.summary.as_ref().map(|s| s.open.iter().map(|o| o.factor.clone()).collect::<Vec<_>>()),
            "rhs_terms": rhs_terms, "bytes": body.to_string().len()}));
        let refused = em.line.is_none();
        pieces.push(Piece {
            tag: tag.clone(),
            depth: 0,
            record: body.clone(),
            lift_seconds: lift.seconds,
            lift_peak_mb: lift.peak_mb,
            check_seconds: check_run.as_ref().map_or(0.0, |c| c.run.seconds),
            check_peak_mb: check_run.as_ref().map_or(0.0, |c| c.run.peak_mb),
            rhs_terms,
            bytes: em.line.as_ref().map_or(0, String::len),
            certs_tried: tried,
            emission: em,
        });
        if refused && outcome.is_none() {
            outcome = Some(Outcome::CheckFailed {
                piece: tag,
                reason: pieces
                    .last()
                    .and_then(|p| p.emission.refused.clone())
                    .unwrap_or_default(),
            });
        }
    }
    // the partition proof always runs (it answers a question of its own even when a piece is refused)
    let recs: Vec<Value> = pieces.iter().map(|p| p.record.clone()).collect();
    let tmp = work.join("partition");
    std::fs::create_dir_all(&tmp).map_err(|e| e.to_string())?;
    let partition = if recs.len() == given.len() {
        Some(prove_partition(
            header,
            charts,
            original,
            &recs,
            &tmp,
            &cfg.witness_modes,
        )?)
    } else {
        None
    };
    let outcome = match (outcome, &partition) {
        (Some(o), _) => o,
        (None, Some((true, _))) => Outcome::Closed,
        _ => Outcome::PartitionFailed,
    };
    Ok(Report {
        outcome,
        pieces,
        partition,
        log,
    })
}
