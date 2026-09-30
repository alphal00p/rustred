//! Native, read-only admission for staged runtime-order studies.
//!
//! Usage: inspect_candidate_orders --selection selection.json --owner-base DIR
//!        --expected-order EXACT_NATIVE_ORDER_IDENTITY
//!
//! This checks actual saved identities, selected sectors and common native
//! family binding. It neither regenerates/replays IBPs nor proves closure or
//! route witnesses. Only use trusted matching RustRed/Symbolica payloads.
use std::collections::{BTreeMap, BTreeSet};
use std::error::Error;
use std::fs::File;
use std::io::Read;
use std::path::{Component, Path, PathBuf};

use rustred::reduction::ReductionLimits;
use rustred::sector::Mask;
use rustred_app::{
    CandidateBundleInspection, CandidateOwnerBundle, CandidateOwnerLoadLimits,
    MAX_CANDIDATE_BUNDLE_BYTES, MAX_INPUT_BYTES, inspect_generated_candidate_bundle,
    load_generated_candidate_owners,
};
use serde::{Deserialize, Deserializer};
use serde_json::{Value, json};

type Result<T> = std::result::Result<T, Box<dyn Error>>;
const SCHEMA: &str = "rustred.runtime-order-native-admission.v1";

#[derive(Debug)]
struct Args {
    selection: PathBuf,
    owner_base: PathBuf,
    expected_order: String,
}

impl Args {
    fn parse(mut args: impl Iterator<Item = String>) -> Result<Self> {
        let mut values = BTreeMap::new();
        while let Some(key) = args.next() {
            if !matches!(
                key.as_str(),
                "--selection" | "--owner-base" | "--expected-order"
            ) {
                return Err(format!("unknown option {key}").into());
            }
            let value = args
                .next()
                .ok_or_else(|| format!("missing value for {key}"))?;
            if value.is_empty() || value.starts_with("--") {
                return Err(format!("missing value for {key}").into());
            }
            if values.insert(key.clone(), value).is_some() {
                return Err(format!("duplicate option {key}").into());
            }
        }
        let mut take = |key| {
            values
                .remove(key)
                .ok_or_else(|| format!("missing option {key}"))
        };
        Ok(Self {
            selection: take("--selection")?.into(),
            owner_base: take("--owner-base")?.into(),
            expected_order: take("--expected-order")?,
        })
    }
}

#[derive(Debug, Deserialize)]
struct Selection {
    family_fingerprint: String,
    owner_count: usize,
    owners: Vec<Owner>,
    #[serde(default)]
    load_limits: LoadLimits,
}

/// The same caller-owned ingress keys as the native routed selection loader.
/// Kept local to this read-only example; no candidate data can raise its limits.
#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
struct LoadLimits {
    #[serde(default, deserialize_with = "explicit_positive_limit")]
    max_bundle_bytes: Option<usize>,
    #[serde(default, deserialize_with = "explicit_positive_limit")]
    max_total_input_bytes: Option<usize>,
    #[serde(default, deserialize_with = "explicit_positive_limit")]
    max_total_coefficient_bytes: Option<usize>,
    #[serde(default, deserialize_with = "explicit_positive_limit")]
    max_collection_entries: Option<usize>,
    #[serde(default, deserialize_with = "explicit_positive_limit")]
    max_total_symbolica_state_bytes: Option<usize>,
    #[serde(default, deserialize_with = "explicit_positive_limit")]
    max_zero_sector_visits: Option<usize>,
    #[serde(default, deserialize_with = "explicit_positive_limit")]
    max_coefficient_bytes: Option<usize>,
}

fn explicit_positive_limit<'de, D: Deserializer<'de>>(
    deserializer: D,
) -> std::result::Result<Option<usize>, D::Error> {
    let value = usize::deserialize(deserializer)?;
    if value == 0 {
        return Err(serde::de::Error::custom("load limits must be positive"));
    }
    Ok(Some(value))
}

impl LoadLimits {
    fn resolve(&self) -> Result<CandidateOwnerLoadLimits> {
        let mut limits = CandidateOwnerLoadLimits::default();
        if let Some(value) = self.max_bundle_bytes {
            limits.bundle.max_bundle_bytes = value;
        }
        if let Some(value) = self.max_total_input_bytes {
            limits.max_total_input_bytes = value;
        }
        if let Some(value) = self.max_total_coefficient_bytes {
            limits.bundle.max_total_coefficient_bytes = value;
        }
        if let Some(value) = self.max_collection_entries {
            limits.bundle.max_collection_entries = value;
        }
        if let Some(value) = self.max_total_symbolica_state_bytes {
            limits.max_total_symbolica_state_bytes = value;
        }
        if let Some(value) = self.max_zero_sector_visits {
            limits.max_zero_sector_visits = value;
        }
        if let Some(value) = self.max_coefficient_bytes {
            limits.bundle.max_coefficient_bytes = value;
        }
        if limits.bundle.max_bundle_bytes > MAX_CANDIDATE_BUNDLE_BYTES {
            return Err("per-owner bundle exceeds the 1 GiB hard ceiling".into());
        }
        Ok(limits)
    }
}

fn limits_report(limits: CandidateOwnerLoadLimits) -> Value {
    json!({
        "max_bundle_bytes": limits.bundle.max_bundle_bytes,
        "max_total_input_bytes": limits.max_total_input_bytes,
        "max_total_coefficient_bytes": limits.bundle.max_total_coefficient_bytes,
        "max_collection_entries": limits.bundle.max_collection_entries,
        "max_total_symbolica_state_bytes": limits.max_total_symbolica_state_bytes,
        "max_zero_sector_visits": limits.max_zero_sector_visits,
        "max_coefficient_bytes": limits.bundle.max_coefficient_bytes,
    })
}

#[derive(Debug, Deserialize)]
struct Owner {
    mask: String,
    path: PathBuf,
    bytes: Option<usize>,
}

impl Selection {
    fn masks(&self) -> Result<Vec<Mask>> {
        if self.family_fingerprint.is_empty()
            || self.owners.is_empty()
            || self.owner_count != self.owners.len()
        {
            return Err("empty family/owners or inconsistent owner count".into());
        }
        let arity = self.owners[0].mask.len();
        if !(1..=16).contains(&arity) {
            return Err("owner arity is outside the native loader's supported range 1..=16".into());
        }
        let mut seen = BTreeSet::new();
        self.owners
            .iter()
            .map(|owner| {
                if owner.mask.len() != arity
                    || !owner.mask.bytes().all(|bit| matches!(bit, b'0' | b'1'))
                    || !seen.insert(owner.mask.as_str())
                {
                    return Err("invalid, mixed-arity or duplicate owner mask".into());
                }
                Ok(Mask::try_new(owner.mask.bytes().map(|bit| bit == b'1'))?)
            })
            .collect()
    }
}

fn read_bounded(path: &Path, maximum: usize) -> Result<Vec<u8>> {
    let mut bytes = Vec::new();
    File::open(path)?
        .take(maximum as u64 + 1)
        .read_to_end(&mut bytes)?;
    if bytes.len() > maximum {
        return Err(format!("input exceeds admission limit: {}", path.display()).into());
    }
    Ok(bytes)
}

fn relative_owner_path(path: &Path) -> bool {
    !path.as_os_str().is_empty()
        && path
            .components()
            .all(|part| matches!(part, Component::Normal(_) | Component::CurDir))
}

fn validate_inspection(
    info: &CandidateBundleInspection,
    expected_order: &str,
    expected_family: &str,
    arity: usize,
) -> Result<()> {
    if info.integral_order != expected_order {
        return Err("actual saved integral_order differs from the planned identity".into());
    }
    if info.family_fingerprint != expected_family || info.arity != arity {
        return Err("actual saved family/arity differs from the selection".into());
    }
    if info.solved_sectors != 1 {
        return Err("selected payload is not a single-sector owner shard".into());
    }
    Ok(())
}

fn admit<const N: usize>(
    inputs: &[CandidateOwnerBundle<'_>],
    expected_family: &str,
    limits: CandidateOwnerLoadLimits,
) -> Result<()> {
    let (family, owners) =
        load_generated_candidate_owners::<N>(inputs, limits, ReductionLimits::default())?;
    if family.fingerprint() != expected_family || owners.owner_count() != inputs.len() {
        return Err("native admitted family/owner count differs from the selection".into());
    }
    let expected: BTreeSet<Vec<bool>> = inputs
        .iter()
        .map(|input| input.owner_sector.active_bits().to_vec())
        .collect();
    let actual: BTreeSet<Vec<bool>> = owners
        .owner_sectors()
        .map(|sector| sector.to_vec())
        .collect();
    if expected != actual {
        return Err("native admitted sector set differs from the selection".into());
    }
    Ok(())
}

fn run(args: Args) -> Result<Value> {
    let selection: Selection =
        serde_json::from_slice(&read_bounded(&args.selection, MAX_INPUT_BYTES)?)?;
    let masks = selection.masks()?;
    let arity = masks[0].arity();
    let owner_base = args.owner_base.canonicalize()?;
    let limits = selection.load_limits.resolve()?;
    let mut payloads = Vec::with_capacity(selection.owners.len());
    let mut summaries = Vec::with_capacity(selection.owners.len());
    let mut input_bytes = 0_usize;
    for owner in &selection.owners {
        if !relative_owner_path(&owner.path) {
            return Err(format!(
                "owner path must be relative inside owner-base: {}",
                owner.path.display()
            )
            .into());
        }
        let path = owner_base.join(&owner.path).canonicalize()?;
        if !path.starts_with(&owner_base) {
            return Err("owner symlink resolves outside owner-base".into());
        }
        let remaining = limits.max_total_input_bytes.saturating_sub(input_bytes);
        let bytes = read_bounded(&path, limits.bundle.max_bundle_bytes.min(remaining))?;
        if owner.bytes.is_some_and(|expected| expected != bytes.len()) {
            return Err(format!("owner size differs from selection: {}", owner.mask).into());
        }
        input_bytes = input_bytes
            .checked_add(bytes.len())
            .ok_or("input byte count overflow")?;
        let info = inspect_generated_candidate_bundle(&bytes, limits.bundle)?;
        validate_inspection(
            &info,
            &args.expected_order,
            &selection.family_fingerprint,
            arity,
        )
        .map_err(|error| format!("owner {}: {error}", owner.mask))?;
        summaries.push(json!({"mask": owner.mask, "path": owner.path,
            "bytes": bytes.len(), "schema": info.schema, "rules": info.generated_rules,
            "finite_residuals": info.finite_residuals}));
        payloads.push(bytes);
    }
    // Inspect and import the same immutable buffers, with no reread/swap window.
    let inputs: Vec<_> = payloads
        .iter()
        .zip(&masks)
        .map(|(bytes, mask)| CandidateOwnerBundle {
            bytes,
            owner_sector: mask,
        })
        .collect();
    macro_rules! dispatch {
        ($($n:literal),+) => { match arity {
            $($n => admit::<$n>(&inputs, &selection.family_fingerprint, limits)?,)+
            _ => return Err("unsupported native owner arity".into()),
        } };
    }
    dispatch!(1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16);
    Ok(
        json!({"schema": SCHEMA, "status": "NATIVE_OWNER_BINDING_ADMITTED",
        "arity": arity, "owner_count": inputs.len(), "input_bytes": input_bytes,
        "integral_order": args.expected_order, "owners": summaries,
        "effective_load_limits": limits_report(limits),
        "native_payload_admission": true, "actual_order_matches": true,
        "expected_sector_set_matches": true, "expected_family_matches": true,
        "source_replay_claim": false, "route_verification_claim": false,
        "closure_claim": false}),
    )
}

fn main() {
    match Args::parse(std::env::args().skip(1)).and_then(run) {
        Ok(report) => println!("{report}"),
        Err(error) => {
            eprintln!(
                "{}",
                json!({"schema": SCHEMA, "status": "REJECTED", "error": error.to_string()})
            );
            std::process::exit(1);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn omitted_load_limits_match_native_loader_defaults() {
        assert_eq!(
            serde_json::from_str::<LoadLimits>("{}")
                .unwrap()
                .resolve()
                .unwrap(),
            CandidateOwnerLoadLimits::default()
        );
        let selection: Selection = serde_json::from_str(
            r#"{"family_fingerprint":"family","owner_count":1,"owners":[{"mask":"1","path":"one"}]}"#,
        ).unwrap();
        assert_eq!(
            selection.load_limits.resolve().unwrap(),
            CandidateOwnerLoadLimits::default()
        );
    }

    #[test]
    fn all_explicit_limits_reach_the_native_budget_and_report() {
        let limits: LoadLimits = serde_json::from_value(json!({
            "max_bundle_bytes": 1_073_741_824usize,
            "max_total_input_bytes": 2_147_483_648usize,
            "max_total_coefficient_bytes": 2_147_483_648usize,
            "max_collection_entries": 32_000_000,
            "max_total_symbolica_state_bytes": 268_435_456,
            "max_zero_sector_visits": 9_000_000,
            "max_coefficient_bytes": 33_554_432,
        }))
        .unwrap();
        let actual = limits.resolve().unwrap();
        assert_eq!(actual.bundle.max_bundle_bytes, 1_073_741_824);
        assert_eq!(actual.max_total_input_bytes, 2_147_483_648);
        assert_eq!(actual.bundle.max_total_coefficient_bytes, 2_147_483_648);
        assert_eq!(actual.bundle.max_collection_entries, 32_000_000);
        assert_eq!(actual.max_total_symbolica_state_bytes, 268_435_456);
        assert_eq!(actual.max_zero_sector_visits, 9_000_000);
        assert_eq!(actual.bundle.max_coefficient_bytes, 33_554_432);
        assert_eq!(
            serde_json::from_value::<LoadLimits>(limits_report(actual))
                .unwrap()
                .resolve()
                .unwrap(),
            actual
        );
    }

    #[test]
    fn load_limits_reject_ambiguous_or_invalid_values() {
        for field in [
            "max_bundle_bytes",
            "max_total_input_bytes",
            "max_total_coefficient_bytes",
            "max_collection_entries",
            "max_total_symbolica_state_bytes",
            "max_zero_sector_visits",
            "max_coefficient_bytes",
        ] {
            for value in [
                "0",
                "-1",
                "null",
                "1.5",
                "true",
                "\"1\"",
                "18446744073709551616",
            ] {
                let text = format!("{{\"{field}\":{value}}}");
                assert!(serde_json::from_str::<LoadLimits>(&text).is_err(), "{text}");
            }
            let duplicate = format!("{{\"{field}\":1,\"{field}\":2}}");
            assert!(serde_json::from_str::<LoadLimits>(&duplicate).is_err());
        }
        assert!(serde_json::from_str::<LoadLimits>(r#"{"unknown_limit":1}"#).is_err());
        assert!(serde_json::from_str::<LoadLimits>("null").is_err());
        assert!(
            serde_json::from_str::<LoadLimits>(r#"{"max_bundle_bytes":1073741825}"#)
                .unwrap()
                .resolve()
                .is_err()
        );
    }

    fn arguments(values: &[&str]) -> Result<Args> {
        Args::parse(values.iter().map(|value| (*value).to_owned()))
    }

    #[test]
    fn exact_required_arguments_and_unknown_duplicate_missing_rejection() {
        let valid = [
            "--selection",
            "selection.json",
            "--owner-base",
            "inputs",
            "--expected-order",
            "native-id",
        ];
        let found = arguments(&valid).unwrap();
        assert_eq!(found.expected_order, "native-id");
        for invalid in [
            vec![],
            valid[..5].to_vec(),
            [valid.as_slice(), &["--extra", "value"]].concat(),
            [valid.as_slice(), &["--expected-order", "different"]].concat(),
        ] {
            assert!(arguments(&invalid).is_err());
        }
    }

    #[test]
    fn selection_requires_exact_distinct_masks_and_count() {
        let text = r#"{"family_fingerprint":"family","owner_count":2,
            "owners":[{"mask":"10","path":"one"},{"mask":"11","path":"two"}]}"#;
        let selection: Selection = serde_json::from_str(text).unwrap();
        assert_eq!(selection.masks().unwrap().len(), 2);
        for changed in [
            text.replace("\"owner_count\":2", "\"owner_count\":1"),
            text.replace("\"11\"", "\"10\""),
            text.replace("\"11\"", "\"1\""),
            text.replace("\"11\"", "\"x1\""),
        ] {
            assert!(
                serde_json::from_str::<Selection>(&changed)
                    .unwrap()
                    .masks()
                    .is_err()
            );
        }
        assert!(
            serde_json::from_str::<Selection>(
                &text.replace("\"owner_count\":2", "\"owner_count\":2,\"owner_count\":2")
            )
            .is_err()
        );
    }

    #[test]
    fn owner_paths_stay_below_explicit_base() {
        for allowed in ["owners/one.rrbin", "./owners/two.rrbin"] {
            assert!(relative_owner_path(Path::new(allowed)));
        }
        for rejected in ["", "/owners/one.rrbin", "../other", "owners/../../other"] {
            assert!(!relative_owner_path(Path::new(rejected)));
        }
    }

    #[test]
    fn actual_native_metadata_must_match_every_binding() {
        let make = || CandidateBundleInspection {
            schema: "synthetic-test-only".into(),
            status: "uncertified-candidates".into(),
            family_fingerprint: "family".into(),
            arity: 2,
            root_sector: vec![true, true],
            sectors: vec![vec![true, true]],
            integral_order: "expected".into(),
            numerical_depth: 2,
            max_numerator_rank: None,
            finite_case_policy: rustred_app::FiniteCasePolicy::SearchFinite,
            finite_case_limits: Default::default(),
            solved_sectors: 1,
            generated_rules: 0,
            finite_residuals: 0,
            unique_coefficients: 0,
            symbolica_state_bytes: 0,
            coefficient_table_bytes: 0,
        };
        assert!(validate_inspection(&make(), "expected", "family", 2).is_ok());
        for field in 0..4 {
            let mut changed = make();
            match field {
                0 => changed.integral_order = "old-order".into(),
                1 => changed.family_fingerprint = "other-family".into(),
                2 => changed.arity = 3,
                _ => changed.solved_sectors = 2,
            }
            assert!(validate_inspection(&changed, "expected", "family", 2).is_err());
        }
    }
}
