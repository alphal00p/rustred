//! LOAD-TIME VERIFICATION OF SUPPLIED RELATIONS (lane symfix, audit r2-sym F1 / F2 / F3 / F4 / F8; interfaces/import.md
//! 13.3, interfaces/closeloop.md 24). A `supplied_relations` record gets the EXACT verdict bit, and so serves, ONLY when
//! THIS import verifies it AT LOAD, per record, the result bound to (record SHA-256, key):
//!
//! - (a) GROUP RE-DERIVATION (plain one-element symmetry relations): `import::symmetry::verify_relation_with` against the
//!   group the import derives itself (`verify`, else the frame header's family), exact rational arithmetic. Level
//!   `per-key`, basis `symmetry_relation` (import.md 13.2, unchanged).
//! - (b) KERNEL CERTIFICATE (`kernel_certificates`): a SEALED results stream of the contract kind `proof-symrows`
//!   (row 40, schema [`CERT_SCHEMA`]) pinned by SHA-256 in the spec, CHECKED here per record: the stream shape of the
//!   seal (one end record, last, `records` = the certificate count, `request_blake3` present), the relation digest
//!   ([`relation_sha256`]) recomputed from the IMPORT'S OWN record (key + coefficient texts, byte-exact) equal to the
//!   certificate's, the key, class `certified` / verdict `EXACT_IN_SPAN_AT_KEY`, the family (SHA-256 of the frame
//!   header's `family_source`) and the kernel (`kernel_sha256` in the source's allow-list `kernels`); a stated group
//!   digest must be the import's. Level `per-key`, basis `symmetry_relation_kernel_certified`: the kernel proves the
//!   relation from IBP rows + the relation's own symmetry rows, independently of the composer's code.
//! - (c) SAME-BACKEND RE-VERIFICATION (composites, `verification`): the record's verification line (schema
//!   `rustred.campaign.symmetry-composite.v1`) is READ and the relation re-verified by
//!   `repairflow::composite_verify::verify_line` against THIS import's catalogue (image re-derived, every replaced term's
//!   FIRST MATCH = the cited record, composition repeated mod p, shipped terms compared as a map). It re-uses the
//!   composer's functions (`rows_by_element`, `image_of`, `compose_image`): NOT an independent route. Level `imported`,
//!   basis `symmetry_composite_same_backend`. Two-row lines (`kind` two_row*) enter through (b) only (F8).
//!
//! Anything else (no route, a missing or failing verification line, an unbound certificate) gets NO EXACT bit: the key
//! is open (`recipe_unverified`, never certified at any `required_evidence`), with the typed reason in
//! `SourceReport.relations_not_verified` and [`SuppliedRoute::Unverified`]. A named-but-unread file gives nothing.
//! Malformed coefficient texts (unparseable, an identically vanishing denominator) and repeated right-hand-side keys are
//! refused at parse (load error).
//!
//! Generic (L8): no node, arity, count, threshold or path is known here; d values and bounds are data.

use std::collections::{BTreeMap, HashMap};
use std::path::Path;

use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::error::ImportError;
use super::lines::{parse_key_text, verify_file};
use super::load::ImportedSet;
use super::spec::{FileRef, resolve};
use crate::closure::sha256::Sha256;
use crate::closure::{RelationStore, VERDICT_EXACT};
use crate::independent_algebra::Independent;
use crate::poly::ENGINE_PRIMES;
use crate::replay_base::algebra::{CheckAlgebra, Ctx};
use crate::replay_base::symrows::VerifiedGroup;

/// Schema of one kernel certificate line (contract kind proof-symrows, interfaces/proof-symrows-job.md 3).
pub const CERT_SCHEMA: &str = "rustred.campaign.symrows-certificate.v1";
/// Schema word of the canonical relation object hashed by [`relation_sha256`].
pub const RELATION_SCHEMA: &str = "rustred.campaign.symrows-relation.v1";
/// The kernel's verdict word of a proof (first word of its status).
pub const CERT_VERDICT_PROVED: &str = "EXACT_IN_SPAN_AT_KEY";
/// The certificate class of a proof.
pub const CERT_CLASS_CERTIFIED: &str = "certified";

/// Typed reason classes of an unverified supplied relation (the first word of [`SuppliedRoute::Unverified`]'s reason).
pub const UNVERIFIED_NO_ROUTE: &str = "no_verification_route";
pub const UNVERIFIED_NO_LINE: &str = "verification_line_missing";
pub const UNVERIFIED_REFUSED: &str = "verification_refused";
pub const UNVERIFIED_TWO_ROW: &str = "two_row_needs_kernel_certificate";
pub const UNVERIFIED_NO_GROUP: &str = "group_unavailable";
pub const UNVERIFIED_NO_CATALOGUE: &str = "catalogue_not_loaded";
pub const UNVERIFIED_CERTIFICATE: &str = "certificate_not_bound";
pub const UNVERIFIED_NOT_IN_SPAN: &str = "group_rederivation_failed";

/// How a supplied relation was verified at load (per record; [`super::load::SuppliedInfo::route`]).
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(tag = "route", rename_all = "snake_case")]
pub enum SuppliedRoute {
    /// (b) a kernel certificate checked at load: per-key.
    KernelCertified {
        certificate_sha256: String,
        kernel_sha256: String,
    },
    /// (a) re-derived from the verified group by the import's exact verifier: per-key.
    GroupRederived,
    /// (c) re-verified by the composer's own functions (modular): imported.
    SameBackend,
    /// A `diagnostic` keys-only source: followed, never certified.
    Diagnostic,
    /// Not verified: no EXACT bit, never served; the typed reason (`<class>: <detail>`).
    Unverified { reason: String },
}

impl SuppliedRoute {
    /// The record carries the EXACT verdict bit (it is followed by the closure).
    pub fn exact(&self) -> bool {
        !matches!(self, SuppliedRoute::Unverified { .. })
    }
    pub fn unverified(class: &str, detail: impl std::fmt::Display) -> Self {
        SuppliedRoute::Unverified {
            reason: format!("{class}: {detail}"),
        }
    }
}

/// `kernel_certificates` of a `supplied_relations` source: sealed certificate streams (each pinned by SHA-256) and the
/// allow-list of kernel binaries (SHA-256 of the rr_proof the contract ran; data, e.g. from the frozen engine's
/// SHA256SUMS). An empty allow-list accepts no certificate.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CertificateSpec {
    pub files: Vec<FileRef>,
    #[serde(default)]
    pub kernels: Vec<String>,
}

fn sha256_hex(b: &[u8]) -> String {
    let mut h = Sha256::new();
    h.update(b);
    h.finish_hex()
}

fn is_hex64(s: &str) -> bool {
    s.len() == 64 && s.bytes().all(|b| b.is_ascii_hexdigit())
}

/// The RELATION DIGEST the import binds a certificate with: SHA-256 of the canonical JSON object
/// `{"key":[..],"rhs":[[[..],"<text>"],..],"schema":"rustred.campaign.symrows-relation.v1"}` (members sorted, no
/// whitespace, rhs sorted by key, the producer's coefficient texts byte-exact; the campaign's canonical form of that
/// object). The same object as the contract's BLAKE3 `relation_digest` (proof-symrows-job.md 2). A repeated rhs key is
/// refused.
pub fn relation_sha256(key: &[i32], rhs: &[(Vec<i32>, String)]) -> Result<String, String> {
    relation_canonical_text(key, rhs).map(|t| sha256_hex(t.as_bytes()))
}

/// The canonical JSON text hashed by [`relation_sha256`] (equal to the contract's `canon::canonical_line` of the
/// same object: tested in the supervisor crate, tests/symfix_sup.rs).
pub fn relation_canonical_text(key: &[i32], rhs: &[(Vec<i32>, String)]) -> Result<String, String> {
    let mut r: Vec<&(Vec<i32>, String)> = rhs.iter().collect();
    r.sort_by(|a, b| a.0.cmp(&b.0));
    if r.windows(2).any(|w| w[0].0 == w[1].0) {
        return Err("a repeated right-hand-side key".into());
    }
    let ints = |k: &[i32]| {
        format!(
            "[{}]",
            k.iter().map(i32::to_string).collect::<Vec<_>>().join(",")
        )
    };
    let mut s = String::with_capacity(64 + 48 * r.len());
    s.push_str("{\"key\":");
    s.push_str(&ints(key));
    s.push_str(",\"rhs\":[");
    for (i, (k, c)) in r.iter().enumerate() {
        if i > 0 {
            s.push(',');
        }
        s.push('[');
        s.push_str(&ints(k));
        s.push(',');
        s.push_str(&serde_json::to_string(c).map_err(|e| e.to_string())?);
        s.push(']');
    }
    s.push_str("],\"schema\":");
    s.push_str(&serde_json::to_string(RELATION_SCHEMA).map_err(|e| e.to_string())?);
    s.push('}');
    Ok(s)
}

/// SHA-256 of a frame header's `family_source` text (the family a certificate must be bound to).
pub fn family_sha256(family_source: &str) -> String {
    sha256_hex(family_source.as_bytes())
}

/// The modular contexts of the parse check / the same-backend re-verification: the two engine primes x `ds`.
fn contexts(ds: &[u64]) -> Vec<Ctx> {
    ENGINE_PRIMES
        .iter()
        .flat_map(|&p| {
            ds.iter().map(move |&d| Ctx {
                p,
                vals: vec![d % p],
            })
        })
        .collect()
}

/// PARSE CHECK of a supplied coefficient text (a rational function of d): it must be readable by the import's algebra
/// and its denominator must not vanish at EVERY context (two engine primes x `ds`; an identically zero denominator,
/// e.g. "1/0"). A denominator vanishing at some contexts only is a pole at special d, not a malformed text.
pub fn check_coefficient_text(text: &str, ds: &[u64]) -> Result<(), String> {
    let ctxs = contexts(ds);
    let mut num = vec![0u64; ctxs.len()];
    let mut den = vec![0u64; ctxs.len()];
    Independent
        .eval_ratio(text, 0, &ctxs, &mut num, &mut den)
        .map_err(|e| format!("coefficient {text:.80}: unreadable ({e:?})"))?;
    if den.iter().all(|&x| x == 0) {
        return Err(format!(
            "coefficient {text:.80}: the denominator vanishes identically"
        ));
    }
    Ok(())
}

/// One certificate line as the import reads it.
#[derive(Clone, Debug)]
pub struct CertEntry {
    /// SHA-256 of the certificate line (as written).
    pub certificate_sha256: String,
    pub class: String,
    pub verdict: String,
    pub kernel_sha256: String,
    pub family_sha256: String,
    pub group_digest: Option<String>,
    /// The file it came from (for messages).
    pub file: String,
}

/// The certificates of one source, by (relation SHA-256, key).
#[derive(Clone, Debug, Default)]
pub struct CertificateBook {
    by: HashMap<(String, Vec<i32>), Vec<CertEntry>>,
    pub kernels: Vec<String>,
    /// (path, sha256) of every certificate file read.
    pub files: Vec<(String, String)>,
}

impl CertificateBook {
    /// Read and shape-check every certificate stream (each MUST be pinned by SHA-256). A file that is not a sealed
    /// results stream of [`CERT_SCHEMA`] lines (e.g. an unsealed journal) refuses the load, typed.
    pub fn load(
        spec: &CertificateSpec,
        base: &Path,
        source: &str,
        arity: usize,
    ) -> Result<Self, ImportError> {
        let mut book = CertificateBook {
            kernels: spec
                .kernels
                .iter()
                .map(|k| k.trim_start_matches("sha256:").to_ascii_lowercase())
                .collect(),
            ..Default::default()
        };
        for f in &spec.files {
            let path = resolve(base, &f.path);
            let Some(want) = f.sha256.as_deref() else {
                return Err(ImportError::Spec(format!(
                    "source {source}: the certificate file {} is not pinned: give its sha256",
                    path.display()
                )));
            };
            let got = verify_file(&path, Some(want))?;
            book.files.push((path.display().to_string(), got));
            let text = std::fs::read_to_string(&path).map_err(|e| ImportError::Io {
                path: path.display().to_string(),
                err: e,
            })?;
            let lerr = |line: usize, msg: String| ImportError::Line {
                source: source.to_string(),
                path: path.display().to_string(),
                line,
                msg,
            };
            let lines: Vec<(usize, &str)> = text
                .lines()
                .enumerate()
                .filter(|(_, l)| !l.trim().is_empty())
                .map(|(i, l)| (i + 1, l))
                .collect();
            let Some(&(end_no, end_line)) = lines.last() else {
                return Err(lerr(
                    0,
                    "an empty certificate file (not a sealed stream)".into(),
                ));
            };
            let end: Value = serde_json::from_str(end_line)
                .map_err(|e| lerr(end_no, format!("end record: {e}")))?;
            let n_certs = lines.len() - 1;
            if end["type"] != "end"
                || end["records"].as_u64() != Some(n_certs as u64)
                || !end["request_blake3"].as_str().is_some_and(is_hex64)
            {
                return Err(lerr(
                    end_no,
                    format!(
                        "not a sealed certificate stream: the last line must be the end record \
                         {{\"type\":\"end\",\"records\":{n_certs},\"request_blake3\":<64 hex>}}"
                    ),
                ));
            }
            for &(no, l) in &lines[..n_certs] {
                let v: Value = serde_json::from_str(l).map_err(|e| lerr(no, e.to_string()))?;
                if v["type"] == "end" {
                    return Err(lerr(no, "an end record before the last line".into()));
                }
                if v["schema"] != CERT_SCHEMA {
                    return Err(lerr(
                        no,
                        format!("not a certificate line (schema {CERT_SCHEMA})"),
                    ));
                }
                let s = |k: &str| -> Result<String, ImportError> {
                    v[k].as_str()
                        .map(str::to_string)
                        .ok_or_else(|| lerr(no, format!("certificate field `{k}` missing")))
                };
                let rel = s("relation_sha256")?;
                let fam = s("family_sha256")?;
                let kern = s("kernel_sha256")?;
                if !is_hex64(&rel) || !is_hex64(&fam) || !is_hex64(&kern) {
                    return Err(lerr(
                        no,
                        "relation_sha256 / family_sha256 / kernel_sha256 must be 64 hex".into(),
                    ));
                }
                let key: Vec<i32> = v["key"]
                    .as_array()
                    .and_then(|a| {
                        a.iter()
                            .map(|x| x.as_i64().and_then(|y| i32::try_from(y).ok()))
                            .collect::<Option<Vec<i32>>>()
                    })
                    .filter(|k| k.len() == arity)
                    .ok_or_else(|| {
                        lerr(no, format!("certificate key is not a key of arity {arity}"))
                    })?;
                book.by
                    .entry((rel.to_ascii_lowercase(), key))
                    .or_default()
                    .push(CertEntry {
                        certificate_sha256: sha256_hex(l.as_bytes()),
                        class: s("class")?,
                        verdict: v["verdict"].as_str().unwrap_or_default().to_string(),
                        kernel_sha256: kern.to_ascii_lowercase(),
                        family_sha256: fam.to_ascii_lowercase(),
                        group_digest: v["group_digest"].as_str().map(str::to_string),
                        file: path.display().to_string(),
                    });
            }
        }
        Ok(book)
    }

    /// The certificate of the import's record (key, rhs texts), CHECKED: relation digest + key bound, proved, the
    /// family and (when stated) the group the import's, the kernel in the allow-list. Ok((certificate sha256, kernel
    /// sha256)); Err = the typed reason why none applies.
    pub fn check(
        &self,
        key: &[i32],
        rhs: &[(Vec<i32>, String)],
        family: Option<&str>,
        group_digest: Option<&str>,
    ) -> Result<(String, String), String> {
        let rel = relation_sha256(key, rhs)?;
        let Some(cands) = self.by.get(&(rel, key.to_vec())) else {
            return Err("no certificate bound to this relation and key".into());
        };
        let Some(fam) = family else {
            return Err(
                "the frame header carries no family_source to bind a certificate to".into(),
            );
        };
        let mut why = vec![];
        for c in cands {
            let r = if c.class != CERT_CLASS_CERTIFIED || c.verdict != CERT_VERDICT_PROVED {
                Err(format!("class {} verdict {}", c.class, c.verdict))
            } else if c.family_sha256 != fam {
                Err(format!(
                    "family {} is not the import's {fam}",
                    c.family_sha256
                ))
            } else if !self.kernels.contains(&c.kernel_sha256) {
                Err(format!("kernel {} not in the allow-list", c.kernel_sha256))
            } else if let (Some(g), Some(mine)) = (&c.group_digest, group_digest)
                && g != mine
            {
                Err(format!("group {g} is not the import's {mine}"))
            } else {
                Ok((c.certificate_sha256.clone(), c.kernel_sha256.clone()))
            };
            match r {
                Ok(x) => return Ok(x),
                Err(e) => why.push(format!("{}: {e}", c.file)),
            }
        }
        Err(why.join("; "))
    }
}

/// A composite awaiting its same-backend re-verification (after the catalogue is loaded).
#[derive(Clone, Debug)]
pub struct PendingComposite {
    pub key: Vec<i32>,
    pub rhs: Vec<(Vec<i32>, String)>,
}

/// The pending composites of one supplied source.
pub struct PendingSource {
    /// The relation set's name (= the source id).
    pub name: String,
    pub group: Option<(VerifiedGroup, String, usize)>,
    /// Why no group exists (when `group` is None).
    pub group_error: Option<String>,
    /// The verification file (resolved path), read only when a record needs it.
    pub verification: Option<std::path::PathBuf>,
    pub records: Vec<PendingComposite>,
}

/// The verification lines of a file, by target key (only lines of the composite schema; a file in another format
/// gives none). A target named twice keeps neither (ambiguous).
fn read_verification_lines(path: &Path, arity: usize) -> Result<BTreeMap<Vec<i32>, Value>, String> {
    let text = std::fs::read_to_string(path).map_err(|e| format!("{}: {e}", path.display()))?;
    let mut out: BTreeMap<Vec<i32>, Option<Value>> = BTreeMap::new();
    for l in text.lines() {
        let Ok(v) = serde_json::from_str::<Value>(l) else {
            continue;
        };
        if v["schema"] != crate::repairflow::composite::COMPOSITE_SCHEMA {
            continue;
        }
        let Some(k) = v["target"].as_str().and_then(|t| parse_key_text(t, arity)) else {
            continue;
        };
        out.entry(k).and_modify(|e| *e = None).or_insert(Some(v));
    }
    Ok(out
        .into_iter()
        .filter_map(|(k, v)| v.map(|v| (k, v)))
        .collect())
}

/// Run the same-backend re-verification of every pending composite on the LOADED set and set the EXACT bit of each one
/// that passes ([`SuppliedRoute::SameBackend`]); the rest stay unverified with the typed reason. `catalogue` = the
/// record bodies are loaded (not a validate-bodies-only load).
pub fn finish(
    set: &mut ImportedSet,
    pending: Vec<PendingSource>,
    catalogue: bool,
    opts: &crate::repairflow::composite_verify::LineOptions,
) -> Result<(), ImportError> {
    for p in pending {
        if p.records.is_empty() {
            continue;
        }
        let lines_by_key = match &p.verification {
            Some(v) => read_verification_lines(v, set.frame.arity()),
            None => Ok(BTreeMap::new()),
        };
        let mut verdicts: Vec<(Vec<i32>, SuppliedRoute)> = Vec::with_capacity(p.records.len());
        for r in &p.records {
            let route = (|| -> SuppliedRoute {
                let byk = match &lines_by_key {
                    Ok(m) => m,
                    Err(e) => return SuppliedRoute::unverified(UNVERIFIED_NO_LINE, e),
                };
                let Some(line) = byk.get(&r.key) else {
                    return SuppliedRoute::unverified(
                        UNVERIFIED_NO_LINE,
                        "no verification line of this key (schema rustred.campaign.symmetry-composite.v1)",
                    );
                };
                if line["kind"]
                    .as_str()
                    .is_some_and(|k| k.starts_with("two_row"))
                {
                    return SuppliedRoute::unverified(
                        UNVERIFIED_TWO_ROW,
                        "a two-row relation enters through a kernel certificate only",
                    );
                }
                let Some((group, digest, max_terms)) = &p.group else {
                    return SuppliedRoute::unverified(
                        UNVERIFIED_NO_GROUP,
                        p.group_error.as_deref().unwrap_or("no verified group"),
                    );
                };
                if line["group_digest"].as_str() != Some(digest.as_str()) {
                    return SuppliedRoute::unverified(
                        UNVERIFIED_REFUSED,
                        format!(
                            "group digest {} is not the import's verified group {digest}",
                            line["group_digest"]
                        ),
                    );
                }
                if !catalogue {
                    return SuppliedRoute::unverified(
                        UNVERIFIED_NO_CATALOGUE,
                        "record bodies not loaded (validate-bodies-only load)",
                    );
                }
                let o = crate::repairflow::composite_verify::LineOptions {
                    max_row_terms: *max_terms,
                    ..opts.clone()
                };
                match crate::repairflow::composite_verify::verify_line(
                    set, group, &r.key, &r.rhs, line, &o,
                ) {
                    Ok(()) => SuppliedRoute::SameBackend,
                    Err(e) => SuppliedRoute::unverified(UNVERIFIED_REFUSED, e),
                }
            })();
            verdicts.push((r.key.clone(), route));
        }
        let Some(idx) = set.supplied.iter().position(|s| s.name == p.name) else {
            return Err(ImportError::Core(format!(
                "supplied relation set {} not in the set",
                p.name
            )));
        };
        let rs = std::sync::Arc::get_mut(&mut set.supplied[idx]).ok_or_else(|| {
            ImportError::Core(format!("supplied relation set {} is shared", p.name))
        })?;
        let rep = set.report.sources.iter_mut().find(|s| s.id == p.name);
        let mut rep = rep;
        let info = set.supplied_info.entry(p.name.clone()).or_default();
        for (key, route) in verdicts {
            if route.exact() {
                set_exact(&mut rs.relations, &key)?;
                if let Some(r) = rep.as_deref_mut() {
                    r.relations_same_backend += 1;
                }
            } else if let (Some(r), SuppliedRoute::Unverified { reason }) =
                (rep.as_deref_mut(), &route)
            {
                r.relations_not_verified.push((key.clone(), reason.clone()));
            }
            if let Some(i) = info.get_mut(&key) {
                i.route = route;
            }
        }
    }
    Ok(())
}

fn set_exact(store: &mut RelationStore, key: &[i32]) -> Result<(), ImportError> {
    let i = store
        .find(key)
        .ok_or_else(|| ImportError::Core(format!("relation {key:?} not in its store")))?;
    store.add_verdict_bits(i, VERDICT_EXACT);
    Ok(())
}
