//! The exchange format `rustred-harness-solution.v1` of finite-field
//! solutions: writer and strict reader (the schema is documented in
//! `docs/interfaces.md`, "Sector-box solution exchange format").

use std::collections::{BTreeMap, BTreeSet, HashMap};

use serde_json::{Map, Value, json};
use symbolica::domains::integer::Integer;
use symbolica::domains::rational::Rational;

use crate::algebra::CoefficientContext;
use crate::algebra::reconstruct::{
    PrimeReport, RationalFunction, ReconstructionReport, VerificationReport,
};

use super::super::SolverError;
use super::exact::{BoxCertificate, CertificateMode};
use super::ff::FfReport;
use super::output::{BoxProvenance, BoxSolution, coefficient_of};

/// The schema identifier of the exchange format.
pub const HARNESS_SOLUTION_SCHEMA: &str = "rustred-harness-solution.v1";

/// The `family` block of the exchange format. A reader accepts a file only
/// for the same family fingerprint and arity, and the same router
/// fingerprint when both sides give one.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HarnessFamily {
    pub name: String,
    pub arity: usize,
    pub fingerprint: String,
    /// Hex blake3 fingerprint of the sector router.
    pub router_fingerprint: Option<String>,
}

/// What a harness file records besides the solution.
#[derive(Clone, Debug, PartialEq)]
pub struct HarnessRecord {
    pub family: HarnessFamily,
    /// Certificates checked before the file was written.
    pub certificates: Vec<BoxCertificate>,
    /// The caller's provenance object (`provenance.extra`).
    pub extra: Value,
    /// Whether the file carried `content_blake3` (and it matched; a file
    /// whose digest does not match is refused). Files of older v1 writers
    /// have none.
    pub content_checked: bool,
}

/// blake3 of the mathematical content: the compact JSON of
/// `[parameters, fixed, masters, rules]`.
fn content_digest(parameters: &Value, fixed: &Value, masters: &Value, rules: &Value) -> String {
    let content = Value::Array(vec![
        parameters.clone(),
        fixed.clone(),
        masters.clone(),
        rules.clone(),
    ]);
    blake3::hash(content.to_string().as_bytes())
        .to_hex()
        .to_string()
}

/// One JSON value per line inside the top-level object, compact inside.
fn layout(fields: &[(&str, Value)], rules: &[Value]) -> String {
    let mut out = String::from("{\n");
    for (key, value) in fields {
        out.push_str(&format!(" {}: ", Value::from(*key)));
        if *key == "rules" {
            out.push_str("[\n");
            for (index, rule) in rules.iter().enumerate() {
                let comma = if index + 1 < rules.len() { "," } else { "" };
                out.push_str(&format!("  {rule}{comma}\n"));
            }
            out.push_str(" ]");
        } else {
            out.push_str(&value.to_string());
        }
        out.push_str(",\n");
    }
    out.truncate(out.len() - 2);
    out.push_str("\n}\n");
    out
}

fn family_json(family: &HarnessFamily) -> Value {
    let mut block = json!({
        "name": family.name,
        "arity": family.arity,
        "fingerprint": family.fingerprint,
    });
    if let Some(router) = &family.router_fingerprint {
        block["router_fingerprint"] = Value::from(router.clone());
    }
    block
}

fn fixed_json(fixed: &[(String, Rational)]) -> Value {
    Value::Object(
        fixed
            .iter()
            .map(|(name, value)| (name.clone(), Value::from(value.to_string())))
            .collect(),
    )
}

fn certificate_json(certificate: &BoxCertificate) -> Value {
    json!({
        "mode": certificate.mode.label(),
        "prime": certificate.prime,
        "rows": certificate.rows,
        "rules": certificate.rules,
    })
}

impl<const N: usize> BoxSolution<N> {
    /// The exchange format `rustred-harness-solution.v1`: coefficients as
    /// strings in the free parameter, every reconstruction and verification
    /// prime in `stats`, the given certificates (with their primes) and the
    /// caller's provenance object `extra`.
    pub fn to_harness_json(
        &self,
        family: &HarnessFamily,
        extra: &Value,
        certificates: &[BoxCertificate],
    ) -> Result<String, SolverError> {
        self.check_shape()?;
        if family.arity != self.arity {
            return Err(SolverError::InvalidInput(format!(
                "family block arity {} differs from the solution arity {}",
                family.arity, self.arity
            )));
        }
        let rules: Vec<Value> = (0..self.targets.len())
            .map(|t| {
                let terms: Vec<Value> = self
                    .terms(t)
                    .map(|(master, function)| {
                        json!({"integral": &master[..self.arity], "coefficient": function.format(&self.free)})
                    })
                    .collect();
                json!({"target": &self.targets[t][..self.arity], "terms": terms})
            })
            .collect();
        let masters: Vec<&[i16]> = self.masters.iter().map(|m| &m[..self.arity]).collect();
        let report = &self.report;
        let recon = &report.reconstruction;
        let primes: Vec<Value> = recon
            .primes
            .iter()
            .map(|p| {
                json!({"prime": p.prime, "open": p.open, "interpolated": p.interpolated, "points": p.points,
                       "good_points": p.good_points, "replayed": p.replayed, "accepted": p.accepted,
                       "misfits": p.misfits, "unfinished": p.unfinished, "polynomial_fits": p.polynomial_fits,
                       "failures": p.failures})
            })
            .collect();
        let verifications: Vec<Value> = recon
            .verifications
            .iter()
            .map(|v| {
                json!({"prime": v.prime, "points": v.points, "failed_points": v.failed_points,
                       "failed_functions": v.failed_functions})
            })
            .collect();
        let stats = json!({
            "rows": report.rows, "terms": report.terms, "columns": report.columns,
            "targets": report.targets, "masters": report.masters, "entries": report.entries,
            "nonzero_entries": report.nonzero_entries, "functions": recon.functions,
            "zero_functions": recon.zero_functions,
            "max_numerator_degree": recon.max_numerator_degree,
            "max_denominator_degree": recon.max_denominator_degree,
            "max_primes_per_function": recon.max_primes_per_function,
            "recovered": recon.recovered, "shape_changes": recon.shape_changes,
            "known_denominators": recon.known_denominators,
            "skipped_primes": recon.skipped_primes.len(), "skipped": recon.skipped_primes,
            "resumed_probes": report.resumed_probes, "unlucky": report.unlucky,
            "primes": primes, "verifications": verifications,
            "probe_seconds": micros(recon.probe_seconds),
            "reconstruction_seconds": micros(recon.reconstruction_seconds),
            "verification_seconds": micros(recon.verification_seconds),
            "build_seconds": micros(report.build_seconds), "seconds": micros(report.seconds),
        });
        let provenance = json!({
            "lane": "FiniteField", "free": self.free,
            "selected_hash": self.provenance.selected_hash,
            "master_hash": self.provenance.master_hash,
            "unseeded_master_sectors": self.provenance.unseeded_master_sectors,
            "extra": extra,
        });
        let certificates: Vec<Value> = certificates.iter().map(certificate_json).collect();
        let (parameters, fixed, masters) = (
            json!(self.parameters),
            fixed_json(&self.fixed),
            json!(masters),
        );
        let digest = content_digest(&parameters, &fixed, &masters, &Value::Array(rules.clone()));
        Ok(layout(
            &[
                ("schema", Value::from(HARNESS_SOLUTION_SCHEMA)),
                ("family", family_json(family)),
                ("parameters", parameters),
                ("fixed", fixed),
                ("coefficient_field", json!({"kind": "Q"})),
                ("masters", masters),
                ("rules", Value::Null),
                ("content_blake3", Value::from(digest)),
                ("stats", stats),
                ("certificates", Value::Array(certificates)),
                ("provenance", provenance),
            ],
            &rules,
        ))
    }

    /// Read a finite-field `rustred-harness-solution.v1` file of `family`.
    ///
    /// The reader is strict: the schema, the family (fingerprint, arity and
    /// router fingerprint when both give one), the lane, the parameter
    /// split, every integral, every coefficient (canonical form, lowest
    /// terms) and the stats must be well formed, and no verification or
    /// certificate prime may be a reconstruction prime (nor a certificate
    /// prime a verification prime). Keys a v1 writer does not produce are
    /// ignored inside `stats`, `provenance` and `family`; optional stats
    /// keys absent from older v1 files read as zero or empty.
    pub fn from_harness_json(
        text: &str,
        family: &HarnessFamily,
    ) -> Result<(Self, HarnessRecord), SolverError> {
        read_harness(text, family)
    }
}

fn bad(message: impl std::fmt::Display) -> SolverError {
    SolverError::InvalidInput(format!("harness solution: {message}"))
}

fn field<'a>(object: &'a Value, key: &str) -> Result<&'a Value, SolverError> {
    object
        .get(key)
        .ok_or_else(|| bad(format!("missing `{key}`")))
}

fn text<'a>(object: &'a Value, key: &str) -> Result<&'a str, SolverError> {
    field(object, key)?
        .as_str()
        .ok_or_else(|| bad(format!("`{key}` is not a string")))
}

fn count(object: &Value, key: &str) -> Result<usize, SolverError> {
    match object.get(key) {
        None => Ok(0),
        Some(value) => value
            .as_u64()
            .map(|v| v as usize)
            .ok_or_else(|| bad(format!("`{key}` is not a count"))),
    }
}

/// Seconds are written to the microsecond. JSON readers need not parse
/// floats exactly, so the reader rounds again: a file reads back to the same
/// values and rewrites to the same text.
fn micros(seconds: f64) -> f64 {
    format!("{seconds:.6}").parse().unwrap_or(0.0)
}

fn seconds(object: &Value, key: &str) -> Result<f64, SolverError> {
    match object.get(key) {
        None => Ok(0.0),
        Some(value) => value
            .as_f64()
            .map(micros)
            .ok_or_else(|| bad(format!("`{key}` is not a number"))),
    }
}

fn counts(object: &Value, key: &str) -> Result<BTreeMap<String, usize>, SolverError> {
    let Some(value) = object.get(key) else {
        return Ok(BTreeMap::new());
    };
    let map = value
        .as_object()
        .ok_or_else(|| bad(format!("`{key}` is not an object")))?;
    map.iter()
        .map(|(name, n)| {
            let n = n
                .as_u64()
                .ok_or_else(|| bad(format!("`{key}.{name}` is not a count")))?;
            Ok((name.clone(), n as usize))
        })
        .collect()
}

fn prime_of(object: &Value) -> Result<u64, SolverError> {
    field(object, "prime")?
        .as_u64()
        .ok_or_else(|| bad("a prime is not a u64"))
}

fn integral<const N: usize>(value: &Value, arity: usize) -> Result<[i16; N], SolverError> {
    let list = value
        .as_array()
        .ok_or_else(|| bad("an integral is not an array"))?;
    if list.len() != arity || arity != N {
        return Err(bad(format!(
            "an integral has {} indices; the family has {arity} (storage {N})",
            list.len()
        )));
    }
    let mut out = [0i16; N];
    for (slot, entry) in list.iter().enumerate() {
        out[slot] = entry
            .as_i64()
            .and_then(|v| i16::try_from(v).ok())
            .ok_or_else(|| bad(format!("index {entry} is not an i16")))?;
    }
    Ok(out)
}

fn rational(text: &str) -> Option<Rational> {
    let digits = |s: &str| !s.is_empty() && s.bytes().all(|b| b.is_ascii_digit());
    let (sign, magnitude) = match text.strip_prefix('-') {
        Some(rest) => (-1, rest),
        None => (1, text),
    };
    let (numerator, denominator) = magnitude.split_once('/').unwrap_or((magnitude, "1"));
    if !digits(numerator) || !digits(denominator) {
        return None;
    }
    let numerator: Integer = numerator.parse().ok()?;
    let denominator: Integer = denominator.parse().ok()?;
    if denominator.is_zero() {
        return None;
    }
    let numerator = if sign < 0 { -numerator } else { numerator };
    Some(Rational::from((numerator, denominator)))
}

/// A dense polynomial printed by `RationalFunction::format` (terms
/// `c`, `c*x`, `x`, `c*x^k`, `x^k`, joined by `+` / `-`).
fn polynomial(text: &str, variable: &str) -> Option<Vec<Rational>> {
    let mut coefficients: Vec<Rational> = Vec::new();
    let (mut negative, mut rest) = match text.strip_prefix('-') {
        Some(rest) => (true, rest),
        None => (false, text),
    };
    loop {
        let end = rest.find(['+', '-']).unwrap_or(rest.len());
        let body = &rest[..end];
        let (magnitude, power) = match body.split_once('*') {
            Some((magnitude, power)) => (magnitude, Some(power)),
            None if body.starts_with(variable) => ("1", Some(body)),
            None => (body, None),
        };
        let degree = match power {
            None => 0,
            Some(power) => match power.strip_prefix(variable)? {
                "" => 1,
                exponent => exponent.strip_prefix('^')?.parse::<u16>().ok()?.into(),
            },
        };
        let mut value = rational(magnitude)?;
        if negative {
            value = -value;
        }
        if coefficients.len() <= degree {
            coefficients.resize(degree + 1, Rational::zero());
        }
        coefficients[degree] = &coefficients[degree] + &value;
        if end == rest.len() {
            return Some(coefficients);
        }
        negative = rest.as_bytes()[end] == b'-';
        rest = &rest[end + 1..];
    }
}

/// Parse a coefficient printed by `RationalFunction::format(variable)`;
/// only the canonical form in lowest terms is accepted.
fn function(
    text: &str,
    variable: &str,
    context: &CoefficientContext,
    slot: usize,
) -> Option<RationalFunction> {
    let (numerator, denominator) = match text.strip_prefix('(').and_then(|t| t.strip_suffix(')')) {
        Some(inner) => {
            let (numerator, denominator) = inner.split_once(")/(")?;
            (
                polynomial(numerator, variable)?,
                polynomial(denominator, variable)?,
            )
        }
        None => (polynomial(text, variable)?, vec![Rational::one()]),
    };
    let candidate = RationalFunction {
        numerator,
        denominator,
    };
    if candidate.format(variable) != text {
        return None;
    }
    // Lowest terms: Symbolica's normalisation must not cancel a factor.
    let exact = coefficient_of(&candidate, context, slot);
    let (n, d) = candidate.degrees();
    let lowest = usize::from(exact.numerator.degree(slot)) == n
        && usize::from(exact.denominator.degree(slot)) == d;
    lowest.then_some(candidate)
}

fn read_harness<const N: usize>(
    input: &str,
    expected: &HarnessFamily,
) -> Result<(BoxSolution<N>, HarnessRecord), SolverError> {
    let root: Value =
        serde_json::from_str(input).map_err(|error| bad(format!("not JSON: {error}")))?;
    let schema = text(&root, "schema")?;
    if schema != HARNESS_SOLUTION_SCHEMA {
        return Err(bad(format!(
            "schema {schema:?} is not {HARNESS_SOLUTION_SCHEMA}"
        )));
    }
    let block = field(&root, "family")?;
    let family = HarnessFamily {
        name: text(block, "name")?.to_string(),
        arity: count(block, "arity")?,
        fingerprint: text(block, "fingerprint")?.to_string(),
        router_fingerprint: block
            .get("router_fingerprint")
            .and_then(Value::as_str)
            .map(str::to_string),
    };
    if family.fingerprint != expected.fingerprint || family.arity != expected.arity {
        return Err(bad(format!(
            "written for family {} (arity {}), not {} (arity {})",
            family.name, family.arity, expected.name, expected.arity
        )));
    }
    if let (Some(found), Some(wanted)) = (&family.router_fingerprint, &expected.router_fingerprint)
        && found != wanted
    {
        return Err(bad("written with another sector router"));
    }
    let arity = family.arity;
    if text(field(&root, "coefficient_field")?, "kind")? != "Q" {
        return Err(bad("the coefficient field is not Q"));
    }
    let provenance = field(&root, "provenance")?;
    if text(provenance, "lane")? != "FiniteField" {
        return Err(bad("the reader reads finite-field files only"));
    }
    let parameters: Vec<String> = field(&root, "parameters")?
        .as_array()
        .ok_or_else(|| bad("`parameters` is not an array"))?
        .iter()
        .map(|p| {
            p.as_str()
                .map(str::to_string)
                .ok_or_else(|| bad("a parameter is not a string"))
        })
        .collect::<Result<_, _>>()?;
    let free = text(provenance, "free")?.to_string();
    let slot = parameters
        .iter()
        .position(|p| *p == free)
        .ok_or_else(|| bad("the free parameter is not a parameter"))?;
    let fixed_map: &Map<String, Value> = field(&root, "fixed")?
        .as_object()
        .ok_or_else(|| bad("`fixed` is not an object"))?;
    let mut fixed = Vec::new();
    for name in &parameters {
        match fixed_map.get(name) {
            Some(value) if *name != free => {
                let value = value
                    .as_str()
                    .and_then(|text| rational(text).filter(|value| value.to_string() == text))
                    .ok_or_else(|| {
                        bad(format!("fixed value of {name} is not a canonical rational"))
                    })?;
                fixed.push((name.clone(), value));
            }
            None if *name == free => {}
            _ => {
                return Err(bad(format!(
                    "parameter {name} must be fixed exactly when it is not free"
                )));
            }
        }
    }
    if fixed_map.len() != fixed.len() {
        return Err(bad("`fixed` names an unknown parameter"));
    }
    let masters: Vec<[i16; N]> = field(&root, "masters")?
        .as_array()
        .ok_or_else(|| bad("`masters` is not an array"))?
        .iter()
        .map(|m| integral(m, arity))
        .collect::<Result<_, _>>()?;
    let master_position: HashMap<[i16; N], usize> =
        masters.iter().enumerate().map(|(i, m)| (*m, i)).collect();
    if master_position.len() != masters.len() {
        return Err(bad("a master is listed twice"));
    }
    let context = CoefficientContext::try_new(parameters.iter().map(String::as_str))
        .map_err(|error| bad(format!("parameters: {error}")))?;
    let mut targets = Vec::new();
    let mut coefficients = Vec::new();
    for rule in field(&root, "rules")?
        .as_array()
        .ok_or_else(|| bad("`rules` is not an array"))?
    {
        targets.push(integral::<N>(field(rule, "target")?, arity)?);
        let mut row = vec![RationalFunction::zero(); masters.len()];
        let mut seen = BTreeSet::new();
        for term in field(rule, "terms")?
            .as_array()
            .ok_or_else(|| bad("`terms` is not an array"))?
        {
            let master = integral::<N>(field(term, "integral")?, arity)?;
            let position = *master_position
                .get(&master)
                .ok_or_else(|| bad(format!("{master:?} is not a master")))?;
            if !seen.insert(position) {
                return Err(bad(format!("master {master:?} appears twice in a rule")));
            }
            let printed = text(term, "coefficient")?;
            row[position] = function(printed, &free, &context, slot)
                .filter(|f| !f.is_zero())
                .ok_or_else(|| {
                    bad(format!(
                        "coefficient {printed:?} is not a canonical nonzero function of {free}"
                    ))
                })?;
        }
        coefficients.push(row);
    }
    let stats = field(&root, "stats")?;
    let mut primes = Vec::new();
    for p in field(stats, "primes")?
        .as_array()
        .ok_or_else(|| bad("`stats.primes` is not an array"))?
    {
        primes.push(PrimeReport {
            prime: prime_of(p)?,
            open: count(p, "open")?,
            interpolated: count(p, "interpolated")?,
            points: count(p, "points")?,
            good_points: count(p, "good_points")?,
            replayed: count(p, "replayed")?,
            failures: counts(p, "failures")?,
            accepted: count(p, "accepted")?,
            misfits: count(p, "misfits")?,
            unfinished: count(p, "unfinished")?,
            polynomial_fits: count(p, "polynomial_fits")?,
        });
    }
    let mut verifications = Vec::new();
    for v in field(stats, "verifications")?
        .as_array()
        .ok_or_else(|| bad("`stats.verifications` is not an array"))?
    {
        verifications.push(VerificationReport {
            prime: prime_of(v)?,
            points: count(v, "points")?,
            failed_points: count(v, "failed_points")?,
            failed_functions: count(v, "failed_functions")?,
        });
    }
    let reconstruction: BTreeSet<u64> = primes.iter().map(|p| p.prime).collect();
    if reconstruction.is_empty() || verifications.is_empty() {
        return Err(bad(
            "no reconstruction or no verification prime is recorded",
        ));
    }
    let verification: BTreeSet<u64> = verifications.iter().map(|v| v.prime).collect();
    if !reconstruction.is_disjoint(&verification) {
        return Err(bad("a verification prime is a reconstruction prime"));
    }
    let skipped = match stats.get("skipped") {
        None => Vec::new(),
        Some(list) => list
            .as_array()
            .ok_or_else(|| bad("`stats.skipped` is not an array"))?
            .iter()
            .map(|p| {
                p.as_u64()
                    .ok_or_else(|| bad("a skipped prime is not a u64"))
            })
            .collect::<Result<_, _>>()?,
    };
    let mut certificates = Vec::new();
    if let Some(list) = root.get("certificates") {
        for c in list
            .as_array()
            .ok_or_else(|| bad("`certificates` is not an array"))?
        {
            let mode = match text(c, "mode")? {
                "exact" => CertificateMode::Exact,
                "probabilistic" => CertificateMode::Probabilistic,
                other => return Err(bad(format!("certificate mode {other:?}"))),
            };
            let prime = c.get("prime").and_then(Value::as_u64);
            if mode == CertificateMode::Probabilistic {
                let p = prime.ok_or_else(|| bad("a probabilistic certificate without a prime"))?;
                if reconstruction.contains(&p) || verification.contains(&p) {
                    return Err(bad(format!(
                        "certificate prime {p} was used by the reconstruction"
                    )));
                }
            }
            certificates.push(BoxCertificate {
                mode,
                prime,
                rows: count(c, "rows")?,
                rules: count(c, "rules")?,
            });
        }
    }
    let entries = count(stats, "entries")?;
    let report = FfReport {
        rows: count(stats, "rows")?,
        terms: count(stats, "terms")?,
        columns: count(stats, "columns")?,
        targets: count(stats, "targets")?,
        masters: count(stats, "masters")?,
        entries,
        nonzero_entries: count(stats, "nonzero_entries")?,
        unlucky: counts(stats, "unlucky")?,
        reconstruction: ReconstructionReport {
            functions: stats
                .get("functions")
                .map_or(Ok(entries), |_| count(stats, "functions"))?,
            zero_functions: count(stats, "zero_functions")?,
            primes,
            skipped_primes: skipped,
            verifications,
            recovered: count(stats, "recovered")?,
            shape_changes: count(stats, "shape_changes")?,
            max_numerator_degree: count(stats, "max_numerator_degree")?,
            max_denominator_degree: count(stats, "max_denominator_degree")?,
            max_primes_per_function: count(stats, "max_primes_per_function")?,
            known_denominators: count(stats, "known_denominators")?,
            probe_seconds: seconds(stats, "probe_seconds")?,
            reconstruction_seconds: seconds(stats, "reconstruction_seconds")?,
            verification_seconds: seconds(stats, "verification_seconds")?,
        },
        resumed_probes: count(stats, "resumed_probes")?,
        build_seconds: seconds(stats, "build_seconds")?,
        seconds: seconds(stats, "seconds")?,
    };
    if report.targets != targets.len()
        || report.masters != masters.len()
        || report.entries != targets.len() * masters.len()
    {
        return Err(bad("`stats` does not match the rules and masters"));
    }
    let unseeded_master_sectors = field(provenance, "unseeded_master_sectors")?
        .as_array()
        .ok_or_else(|| bad("`unseeded_master_sectors` is not an array"))?
        .iter()
        .map(|s| {
            s.as_u64()
                .and_then(|s| u32::try_from(s).ok())
                .ok_or_else(|| bad("a sector id is not a u32"))
        })
        .collect::<Result<_, _>>()?;
    let solution = BoxSolution {
        parameters,
        fixed,
        free,
        targets,
        masters,
        coefficients,
        report,
        provenance: BoxProvenance {
            selected_hash: text(provenance, "selected_hash")?.to_string(),
            master_hash: text(provenance, "master_hash")?.to_string(),
            unseeded_master_sectors,
        },
        arity,
    };
    let content_checked = match root.get("content_blake3") {
        None => false,
        Some(digest) => {
            let expected = content_digest(
                field(&root, "parameters")?,
                field(&root, "fixed")?,
                field(&root, "masters")?,
                field(&root, "rules")?,
            );
            if digest.as_str() != Some(expected.as_str()) {
                return Err(bad("`content_blake3` does not match the content"));
            }
            true
        }
    };
    let record = HarnessRecord {
        family,
        certificates,
        extra: provenance.get("extra").cloned().unwrap_or(Value::Null),
        content_checked,
    };
    Ok((solution, record))
}
