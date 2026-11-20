//! Classes, the check summary bound into a record, `class_of` and the POLICY (emission contract sections 4 and 7, with
//! the implementer's review of 2026-11-18: a rule the automation LIFTS must be `whole domain`, or `every first-match key
//! in cone` only when its open loci are curves that cannot be branched and their keys in the cone are enumerated and
//! proven; `sampled` is never produced for a lift, only kept for a LEGACY record brought in unchanged).
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::collections::BTreeMap;

/// The four class words, ordered (`None` < `Sampled` < `EveryKeyInCone` < `WholeDomain`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum ClassWord {
    None,
    Sampled,
    EveryKeyInCone,
    WholeDomain,
}

impl ClassWord {
    pub fn word(self) -> &'static str {
        match self {
            ClassWord::None => "none",
            ClassWord::Sampled => "sampled",
            ClassWord::EveryKeyInCone => "every first-match key in cone",
            ClassWord::WholeDomain => "whole domain",
        }
    }
    pub fn from_word(w: &str) -> Option<ClassWord> {
        [
            ClassWord::None,
            ClassWord::Sampled,
            ClassWord::EveryKeyInCone,
            ClassWord::WholeDomain,
        ]
        .into_iter()
        .find(|c| c.word() == w)
    }
}

/// What a record is (section 6 of the contract): decides which check kind applies.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Kind {
    Lifted,
    Point,
    Symmetry,
    Alias,
    Recipe,
    Legacy,
}

impl Kind {
    pub fn word(self) -> &'static str {
        match self {
            Kind::Lifted => "lifted",
            Kind::Point => "point",
            Kind::Symmetry => "symmetry",
            Kind::Alias => "alias",
            Kind::Recipe => "recipe",
            Kind::Legacy => "legacy",
        }
    }
    pub fn from_word(w: &str) -> Option<Kind> {
        [
            Kind::Lifted,
            Kind::Point,
            Kind::Symmetry,
            Kind::Alias,
            Kind::Recipe,
            Kind::Legacy,
        ]
        .into_iter()
        .find(|k| k.word() == w)
    }
}

/// Who brings the record into the table: the automation lifted it now, or an import of an old table brought it unchanged.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Origin {
    Lift,
    Import,
}

/// One OPEN locus of a check: the factor (stored frame) and its d-coefficient polynomials (the locus is where all vanish).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct OpenLocus {
    pub factor: String,
    pub coefficients: Vec<String>,
}

/// The parts of an independent checker output (`rustred.proof-kernel.weight-cert.v1`) that a class rests on.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CheckSummary {
    pub rows: String,
    pub identity: String,
    pub binding: String,
    pub factors: String,
    /// verdict -> count (EMPTY_CONSTANT, EMPTY_SIGN, EMPTY_IDEAL, IN_EXCEPTION, D_ONLY, OPEN)
    pub loci: BTreeMap<String, u64>,
    pub checker_class: String,
    /// what the checker read: its `rule.target_rhs_sha256` / `rule.domain_sha256` and `cert.sha256`
    pub relation_sha256: String,
    pub domain_sha256: String,
    pub cert_sha256: String,
    /// the OPEN loci (not stored in the record, only in the check output beside the table)
    #[serde(skip)]
    pub open: Vec<OpenLocus>,
}

fn s(v: &Value, path: &[&str]) -> String {
    let mut c = v;
    for k in path {
        c = &c[*k];
    }
    match c {
        Value::String(t) => t.clone(),
        Value::Null => String::new(),
        o => o.to_string(),
    }
}

impl CheckSummary {
    /// Parse a checker output line; Err for an error record (`verdict` ERROR / UNSUPPORTED) or a malformed line.
    pub fn parse(line: &str) -> Result<CheckSummary, String> {
        let v: Value = serde_json::from_str(line).map_err(|e| format!("check output: {e}"))?;
        if let Some(w) = v.get("verdict").and_then(Value::as_str) {
            return Err(format!(
                "checker {w}: {}",
                v.get("error").map(|e| e.to_string()).unwrap_or_default()
            ));
        }
        if v.get("mode").and_then(Value::as_str) != Some("weight-cert") {
            return Err("check output is not a weight-cert record".into());
        }
        let mut loci = BTreeMap::new();
        if let Some(o) = v.get("loci_counts").and_then(Value::as_object) {
            for (k, n) in o {
                loci.insert(k.clone(), n.as_u64().unwrap_or(0));
            }
        }
        let mut open = vec![];
        for l in v
            .get("loci")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
        {
            if l.get("verdict").and_then(Value::as_str) == Some("OPEN") {
                let coefficients = l["argument"]["generators"]
                    .as_array()
                    .map(|a| {
                        a.iter()
                            .filter_map(|x| x.as_str().map(str::to_string))
                            .collect()
                    })
                    .unwrap_or_default();
                open.push(OpenLocus {
                    factor: s(l, &["factor"]),
                    coefficients,
                });
            }
        }
        Ok(CheckSummary {
            rows: s(&v, &["rows", "verdict"]),
            identity: s(&v, &["identity", "verdict"]),
            binding: s(&v, &["binding", "verdict"]),
            factors: s(&v, &["factors", "producer"]),
            loci,
            checker_class: s(&v, &["class"]),
            relation_sha256: s(&v, &["rule", "target_rhs_sha256"]),
            domain_sha256: s(&v, &["rule", "domain_sha256"]),
            cert_sha256: s(&v, &["cert", "sha256"]),
            open,
        })
    }

    pub fn rows_verified(&self) -> bool {
        self.rows == "ROWS_VERIFIED"
            || self.rows == "ROWS_VERIFIED_BY_RECOVERY"
            || self.rows == "ROWS_VERIFIED_BY_RECOVERY_MODULO_ZERO_SECTORS"
            || self.rows == "ROWS_VERIFIED_BY_RECOVERY_MODULO_STATED_RECORDS"
    }
    pub fn identity_exact(&self) -> bool {
        self.identity == "IDENTITY_EXACT"
    }
    pub fn binding_equal(&self) -> bool {
        self.binding == "BINDING_EQUAL" || self.binding == "BINDING_EQUAL_MODULO"
    }
    pub fn open_count(&self) -> u64 {
        self.loci.get("OPEN").copied().unwrap_or(0)
    }
    /// The compact form stored IN the record (verdict words and counts only).
    pub fn record_json(&self) -> Value {
        json!({"rows": self.rows, "identity": self.identity, "binding": self.binding, "factors": self.factors,
               "loci": self.loci, "checker_class": self.checker_class})
    }
}

/// The verdict of an independent INSTANCE-SPAN check (`rr_proof --mode instance-span`, schema
/// `rustred.proof-kernel.span.v1`) of a POINT / SYMMETRY record, BOUND to the record's body: Ok(true) when every key it
/// checked is EXACT_IN_SPAN_AT_KEY (verdict EXACT_ALL_KEYS_IN_SPAN, none not in span, none other), Ok(false) for any
/// other verdict; Err when the output is not an instance-span record of THIS body (`record_sha256` = sha256 of the
/// body line the checker read) or checked no key.
pub fn span_exact(out: &str, body: &str) -> Result<bool, String> {
    let v: Value = serde_json::from_str(out).map_err(|e| format!("span output: {e}"))?;
    if v["mode"].as_str() != Some("instance-span") {
        return Err("the check output is not an instance-span record".into());
    }
    if v["record_sha256"].as_str() != Some(super::body::sha256_hex(body.as_bytes()).as_str()) {
        return Err("the instance-span check was made on another record line".into());
    }
    let keys = v["keys"].as_array().map_or(0, Vec::len);
    if keys == 0 {
        return Err("the instance-span check checked no key".into());
    }
    let c = &v["counts"];
    Ok(v["verdict"].as_str() == Some("EXACT_ALL_KEYS_IN_SPAN")
        && c["not_in_span"].as_u64() == Some(0)
        && c["other"].as_u64() == Some(0)
        && c["in_span"].as_u64() == Some(keys as u64))
}

/// Per-key evidence of the cone class: keys of the record's domain inside cone `cone` that need a per-key proof (the keys
/// on the open loci, or every first-match key), how many were enumerated, whether the enumeration is complete, how many
/// are proven exact.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ConeEvidence {
    pub cone: String,
    /// what the keys are: "open-locus keys of the domain" or "first-match keys at the position"
    pub scope: String,
    pub keys: u64,
    pub keys_sha256: String,
    pub complete: bool,
    pub proven: u64,
    /// for first-match scope: the position and the sha256 of the bodies of the earlier records
    #[serde(default)]
    pub position: Option<u64>,
    #[serde(default)]
    pub prefix_sha256: Option<String>,
}

/// Sampled per-key evidence (legacy records only).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SampleEvidence {
    pub keys: u64,
    pub proven: u64,
    pub keys_sha256: String,
    pub how: String,
}

/// Evidence other than the weight-certificate check.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Evidence {
    #[serde(default)]
    pub cone: Option<ConeEvidence>,
    #[serde(default)]
    pub sample: Option<SampleEvidence>,
    /// point / symmetry kinds: the exact per-key (instance-span) verdict at the record's key (true = exact)
    #[serde(default)]
    pub exact_at_key: Option<bool>,
    /// alias kind: the class of the aliased record read at validation (bound by position + body sha256)
    #[serde(default)]
    pub alias_target: Option<ClassWord>,
    /// recipe kind: the seal covers every key of the face up to the cone
    #[serde(default)]
    pub sealed: Option<bool>,
    /// legacy import: the bound verdict row (side file) re-verified at validation: same relation and domain as the
    /// record's body and still a whole-domain proof object (`legacy::verify_row`)
    #[serde(default)]
    pub legacy_proof: Option<bool>,
}

/// The class of a record and the reason for anything below `whole domain`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Classed {
    pub class: ClassWord,
    pub reason: Option<String>,
    pub evidence: Vec<&'static str>,
}

fn none(reason: impl Into<String>) -> Classed {
    Classed {
        class: ClassWord::None,
        reason: Some(reason.into()),
        evidence: vec![],
    }
}

/// The reason word of a check that is not whole domain.
pub fn check_reason(c: &CheckSummary) -> String {
    if !c.binding_equal() {
        format!("binding_mismatch ({})", c.binding)
    } else if c.identity == "IDENTITY_FAIL" {
        "identity_fail".into()
    } else if !c.rows_verified() {
        format!("rows_not_verified ({})", c.rows)
    } else if !c.identity_exact() {
        format!("identity_not_checkable ({})", c.identity)
    } else if c.open_count() > 0 {
        format!("open_loci_unclosed ({})", c.open_count())
    } else {
        format!("checker_class {}", c.checker_class)
    }
}

/// THE CLASS FUNCTION: derived from the record's kind, its origin, the check bound to its body and the other evidence.
/// Never reads a producer claim; `Sampled` is never returned for `Origin::Lift`.
pub fn class_of(
    kind: Kind,
    origin: Origin,
    check: Option<&CheckSummary>,
    ev: &Evidence,
) -> Classed {
    let cone_ok =
        |e: &Option<ConeEvidence>| e.as_ref().is_some_and(|c| c.complete && c.proven == c.keys);
    match kind {
        Kind::Lifted | Kind::Legacy => {
            if let Some(c) = check {
                let base = c.binding_equal() && c.identity_exact() && c.rows_verified();
                if base && c.checker_class == "WHOLE_DOMAIN" && c.open_count() == 0 {
                    return Classed {
                        class: ClassWord::WholeDomain,
                        reason: None,
                        evidence: vec!["check"],
                    };
                }
                if base && c.checker_class == "OPEN_LOCI" && cone_ok(&ev.cone) {
                    return Classed {
                        class: ClassWord::EveryKeyInCone,
                        reason: None,
                        evidence: vec!["check", "cone"],
                    };
                }
            }
            // legacy import: a whole-domain proof object of a reviewing lane, bound to this body (relation + domain)
            if kind == Kind::Legacy && origin == Origin::Import && ev.legacy_proof == Some(true) {
                return Classed {
                    class: ClassWord::WholeDomain,
                    reason: None,
                    evidence: vec!["proof_object"],
                };
            }
            // per-key evidence on every first-match key of the cone (no whole-domain check needed)
            if ev
                .cone
                .as_ref()
                .is_some_and(|c| c.scope.starts_with("first-match"))
                && cone_ok(&ev.cone)
            {
                return Classed {
                    class: ClassWord::EveryKeyInCone,
                    reason: None,
                    evidence: vec!["cone"],
                };
            }
            if origin == Origin::Import
                && ev
                    .sample
                    .as_ref()
                    .is_some_and(|s| s.keys > 0 && s.proven == s.keys)
            {
                return Classed {
                    class: ClassWord::Sampled,
                    reason: Some("legacy record: per-key proofs at sampled keys only".into()),
                    evidence: vec!["sample"],
                };
            }
            match check {
                Some(c) => none(check_reason(c)),
                None if kind == Kind::Legacy => none("legacy_unchecked"),
                None => none("no_certificate"),
            }
        }
        Kind::Point | Kind::Symmetry => match ev.exact_at_key {
            Some(true) => Classed {
                class: ClassWord::WholeDomain,
                reason: None,
                evidence: vec!["exact_at_key"],
            },
            Some(false) => none("per_key_not_exact"),
            None => none("per_key_not_run"),
        },
        Kind::Alias => match ev.alias_target {
            Some(c) => Classed {
                class: c,
                reason: (c != ClassWord::WholeDomain)
                    .then(|| format!("alias of a record classed {}", c.word())),
                evidence: vec!["alias_target"],
            },
            None => none("alias_target_unresolved"),
        },
        Kind::Recipe => {
            if ev.sealed == Some(true) && cone_ok(&ev.cone) {
                Classed {
                    class: ClassWord::EveryKeyInCone,
                    reason: None,
                    evidence: vec!["seal", "cone"],
                }
            } else {
                none("recipe_not_sealed")
            }
        }
    }
}

/// Liveness of a record at its position in the policy's cone.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Liveness {
    Live,
    Dead,
    /// not computed: treated as live
    Unknown,
}

/// The emission POLICY (data; defaults = the implementer's ruling of 2026-11-18).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Policy {
    pub name: String,
    /// classes a LIVE record the automation lifted may have
    pub lift_live: Vec<ClassWord>,
    /// the cone class is allowed for a lift only when every open locus is a curve that cannot be branched
    pub lift_cone_only_for_curves: bool,
    /// classes a LIVE legacy record brought in unchanged may have
    pub import_live: Vec<ClassWord>,
    /// whether a DEAD record may enter with class none
    pub dead_allow_none: bool,
    /// minimum number of sampled keys for a legacy `sampled` class
    pub min_sample: u64,
    /// the cone of the liveness decision (spec text, e.g. "simplex:39"); None = every record counts as live
    pub liveness_cone: Option<String>,
    /// CONE PRICING (contract section 17, opt-in: it needs engine runs): a replacement must name an ACCEPTED cone verdict
    /// (`replaces.cone_sha256`). Off by default; a policy without it serialises as before (same sha256).
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub cone_pricing: bool,
    /// POLICY VERSION. 2 = the user ruling of 2026-11-19 (LANE_RULES rule 21): d = 4 regularity is a reported number and a
    /// tie-break only, never an acceptance condition. A policy without this member is version 1 (regularity was an
    /// acceptance condition): records and verdicts naming it are recognisably from the old rule.
    #[serde(default = "policy_v1")]
    pub version: u32,
}

fn policy_v1() -> u32 {
    1
}

/// The current policy version (see [`Policy::version`]).
pub const POLICY_VERSION: u32 = 2;

impl Default for Policy {
    fn default() -> Self {
        Policy {
            name: "default".into(),
            lift_live: vec![ClassWord::WholeDomain, ClassWord::EveryKeyInCone],
            lift_cone_only_for_curves: true,
            import_live: vec![
                ClassWord::WholeDomain,
                ClassWord::EveryKeyInCone,
                ClassWord::Sampled,
            ],
            dead_allow_none: true,
            min_sample: 200,
            liveness_cone: None,
            cone_pricing: false,
            version: POLICY_VERSION,
        }
    }
}

impl Policy {
    pub fn sha256(&self) -> String {
        super::body::sha256_hex(serde_json::to_string(self).unwrap_or_default().as_bytes())
    }

    /// Ok when a record of this origin / liveness / class may enter a table. `curves_only` = every open locus of the
    /// record's check is a curve that the ladder cannot branch.
    pub fn admit(
        &self,
        origin: Origin,
        liveness: Liveness,
        c: &Classed,
        curves_only: bool,
        sample_keys: u64,
    ) -> Result<(), String> {
        if liveness == Liveness::Dead {
            return if c.class != ClassWord::None || self.dead_allow_none {
                Ok(())
            } else {
                Err(format!(
                    "policy {}: a dead record with class none is refused",
                    self.name
                ))
            };
        }
        let allowed = match origin {
            Origin::Lift => &self.lift_live,
            Origin::Import => &self.import_live,
        };
        if !allowed.contains(&c.class) {
            return Err(format!(
                "policy {}: a live {} record with class '{}' is refused ({})",
                self.name,
                match origin {
                    Origin::Lift => "lifted",
                    Origin::Import => "imported",
                },
                c.class.word(),
                c.reason.as_deref().unwrap_or("-")
            ));
        }
        if origin == Origin::Lift
            && c.class == ClassWord::EveryKeyInCone
            && self.lift_cone_only_for_curves
            && !curves_only
        {
            return Err(format!(
                "policy {}: a lifted record may be 'every first-match key in cone' only when its open loci are curves",
                self.name
            ));
        }
        if c.class == ClassWord::Sampled && sample_keys < self.min_sample {
            return Err(format!(
                "policy {}: sampled with {sample_keys} keys < min_sample {}",
                self.name, self.min_sample
            ));
        }
        Ok(())
    }
}
