//! Bounded external steering, not source or momentum-map authority.
use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;

use rustred::family::IntegralKey;
use rustred::sector::Mask;
use serde::{
    Deserialize, Deserializer,
    de::{self, MapAccess, Visitor},
};

use crate::{AppError, CandidateOwnerLoadLimits, MAX_CANDIDATE_BUNDLE_BYTES};

pub(super) const MAX_STEERING_BYTES: usize = 16 * 1024 * 1024;
pub(super) const MAX_ROUTES: usize = 100_000;
pub(super) const PREFERRED_RULE_SUBSET_POLICY: &str = "saved-ordinal-subset-v1";

#[derive(Debug, Deserialize)]
pub(super) struct Selection {
    pub family_fingerprint: String,
    pub owners: Vec<Owner>,
    /// Ordered source-replayed partial rules. Original owners stay immutable.
    #[serde(default)]
    pub domain_rule_overlays: Vec<DomainRuleOverlay>,
    /// Trusted alternatives with the original owner's terminal boundary.
    #[serde(default)]
    pub preferred_owner_programs: Vec<PreferredOwnerProgram>,
    pub initial_frontier_routes: Vec<Route>,
    #[serde(default, deserialize_with = "unique_limits")]
    pub load_limits: BTreeMap<String, usize>,
}

fn unique_limits<'de, D: Deserializer<'de>>(
    deserializer: D,
) -> Result<BTreeMap<String, usize>, D::Error> {
    struct Limits;
    impl<'de> Visitor<'de> for Limits {
        type Value = BTreeMap<String, usize>;
        fn expecting(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
            f.write_str("unique positive integer load limits")
        }
        fn visit_map<M: MapAccess<'de>>(self, mut access: M) -> Result<Self::Value, M::Error> {
            let mut result = BTreeMap::new();
            while let Some((key, value)) = access.next_entry::<String, usize>()? {
                if result.insert(key, value).is_some() {
                    return Err(de::Error::custom("duplicate load limit"));
                }
            }
            Ok(result)
        }
    }
    deserializer.deserialize_map(Limits)
}

#[derive(Debug, Deserialize)]
pub(super) struct Owner {
    pub path: PathBuf,
    pub bytes: u64,
    pub mask: String,
}

#[derive(Debug, Deserialize)]
pub(super) struct DomainRuleOverlay {
    pub path: PathBuf,
    pub bytes: u64,
    pub owner_mask: String,
}

#[derive(Debug, Deserialize)]
pub(super) struct PreferredOwnerProgram {
    pub path: PathBuf,
    pub bytes: u64,
    pub owner_mask: String,
    pub residual_policy: PreferredResidualPolicy,
    /// Saved provenance IDs, never vector positions or a requested reordering.
    /// None (also JSON null) preserves all-preferred behavior; [] enables none.
    #[serde(default)]
    pub rule_ordinals: Option<Vec<usize>>,
}

/// Identity marker only; authoritative admission remains Selection::parse and
/// the native loader. Ignored fields avoid decoding route matrices a
/// second time. Invalid JSON is still bound literally by checkpoint identity
/// and will be refused by normal request admission, never executed here.
pub(super) fn has_preferred_rule_subsets(text: &str) -> bool {
    #[derive(Deserialize)]
    struct Probe {
        #[serde(default)]
        preferred_owner_programs: Vec<Entry>,
    }
    #[derive(Deserialize)]
    struct Entry {
        rule_ordinals: Option<de::IgnoredAny>,
    }
    if text.len() > MAX_STEERING_BYTES {
        return false;
    }
    serde_json::from_str::<Probe>(text).is_ok_and(|probe| {
        probe
            .preferred_owner_programs
            .iter()
            .any(|entry| entry.rule_ordinals.is_some())
    })
}

#[derive(Clone, Copy, Debug, Deserialize)]
pub(super) enum PreferredResidualPolicy {
    #[serde(rename = "defer-to-baseline")]
    DeferToBaseline,
}

#[derive(Debug, Deserialize)]
pub(super) struct Route {
    pub source_mask: String,
    pub owner_mask: String,
    pub requires_transport: bool,
    pub source_to_representative: Vec<Vec<String>>,
    pub owner_to_representative: Vec<Vec<String>>,
}

pub(super) fn mask(text: &str, n: usize) -> Result<Mask, AppError> {
    if text.len() != n || text.bytes().any(|b| b != b'0' && b != b'1') {
        return Err(AppError::input("invalid original-coordinate sector mask"));
    }
    Mask::try_new(text.bytes().map(|b| b == b'1')).map_err(|e| AppError::input(e.to_string()))
}

impl Selection {
    pub(super) fn physical_arity(&self) -> usize {
        self.owners
            .first()
            .map(|owner| owner.mask.len())
            .unwrap_or(0)
    }

    pub fn parse(text: &str) -> Result<(Self, usize, CandidateOwnerLoadLimits), AppError> {
        if text.len() > MAX_STEERING_BYTES {
            return Err(AppError::limit("owner selection exceeds 16 MiB"));
        }
        let selection: Self =
            serde_json::from_str(text).map_err(|e| AppError::input(e.to_string()))?;
        let limits = selection.limits()?;
        let n = selection
            .owners
            .first()
            .ok_or_else(|| AppError::input("no owners"))?
            .mask
            .len();
        if !(1..=16).contains(&n) {
            return Err(AppError::input("owner arity outside supported 1..=16"));
        }
        if selection
            .owners
            .len()
            .checked_add(selection.domain_rule_overlays.len())
            .and_then(|count| count.checked_add(selection.preferred_owner_programs.len()))
            .is_none_or(|count| count > limits.bundle.max_collection_entries)
            || selection.initial_frontier_routes.len() > MAX_ROUTES
        {
            return Err(AppError::limit(
                "owner or route count exceeds admission limit",
            ));
        }
        let mut owners = BTreeSet::new();
        let mut total = 0usize;
        for owner in &selection.owners {
            mask(&owner.mask, n)?;
            if !owners.insert(&owner.mask) {
                return Err(AppError::input("duplicate owner mask"));
            }
            let size =
                usize::try_from(owner.bytes).map_err(|_| AppError::limit("owner size overflow"))?;
            total = total
                .checked_add(size)
                .ok_or_else(|| AppError::limit("input size overflow"))?;
            if size > limits.bundle.max_bundle_bytes || total > limits.max_total_input_bytes {
                return Err(AppError::limit(
                    "owner bytes exceed declared ingress limits",
                ));
            }
        }
        for overlay in &selection.domain_rule_overlays {
            mask(&overlay.owner_mask, n)?;
            if !owners.contains(&overlay.owner_mask) {
                return Err(AppError::input("domain-rule overlay owner is missing"));
            }
            let size = usize::try_from(overlay.bytes)
                .map_err(|_| AppError::limit("domain-rule overlay size overflow"))?;
            total = total
                .checked_add(size)
                .ok_or_else(|| AppError::limit("input size overflow"))?;
            if size == 0 {
                return Err(AppError::input("domain-rule overlay payload is empty"));
            }
            if size > limits.bundle.max_bundle_bytes || total > limits.max_total_input_bytes {
                return Err(AppError::limit(
                    "owner and overlay bytes exceed declared ingress limits",
                ));
            }
        }
        let mut preferred = BTreeSet::new();
        let mut subset_entries = 0usize;
        for program in &selection.preferred_owner_programs {
            mask(&program.owner_mask, n)?;
            if !owners.contains(&program.owner_mask) || !preferred.insert(&program.owner_mask) {
                return Err(AppError::input("preferred owner is missing or duplicated"));
            }
            if selection
                .domain_rule_overlays
                .iter()
                .any(|p| p.owner_mask == program.owner_mask)
            {
                return Err(AppError::input(
                    "preferred owner and repair overlay collision is unsupported",
                ));
            }
            let size = usize::try_from(program.bytes)
                .map_err(|_| AppError::limit("preferred owner size overflow"))?;
            total = total
                .checked_add(size)
                .ok_or_else(|| AppError::limit("input size overflow"))?;
            if size == 0 {
                return Err(AppError::input("preferred owner payload is empty"));
            }
            if size > limits.bundle.max_bundle_bytes || total > limits.max_total_input_bytes {
                return Err(AppError::limit(
                    "owner, overlay and preferred bytes exceed declared ingress limits",
                ));
            }
            if let Some(ordinals) = &program.rule_ordinals {
                subset_entries = subset_entries
                    .checked_add(ordinals.len())
                    .ok_or_else(|| AppError::limit("preferred rule subset count overflow"))?;
                if subset_entries > limits.bundle.max_collection_entries {
                    return Err(AppError::limit(
                        "preferred rule subset metadata exceeds admission limit",
                    ));
                }
                if ordinals.windows(2).any(|pair| pair[0] >= pair[1]) {
                    return Err(AppError::input(
                        "preferred rule ordinals must be strictly increasing",
                    ));
                }
            }
        }
        let mut sources = BTreeSet::new();
        for route in &selection.initial_frontier_routes {
            mask(&route.source_mask, n)?;
            mask(&route.owner_mask, n)?;
            if !owners.contains(&route.owner_mask) || !sources.insert(&route.source_mask) {
                return Err(AppError::input("route owner missing or duplicate source"));
            }
            if !route.requires_transport && route.source_mask != route.owner_mask {
                return Err(AppError::input(
                    "nonidentity route requires native transport",
                ));
            }
            let loops = route.source_to_representative.len();
            if loops == 0 || loops > n || route.owner_to_representative.len() != loops {
                return Err(AppError::input("invalid route matrix dimensions"));
            }
            for matrix in [
                &route.source_to_representative,
                &route.owner_to_representative,
            ] {
                for row in matrix {
                    if row.len() != loops || row.iter().any(|x| x.parse::<i64>().is_err()) {
                        return Err(AppError::input(
                            "route matrices require square signed-integer entries",
                        ));
                    }
                }
            }
        }
        Ok((selection, n, limits))
    }

    fn limits(&self) -> Result<CandidateOwnerLoadLimits, AppError> {
        let mut limits = CandidateOwnerLoadLimits::default();
        for (name, &value) in &self.load_limits {
            if value == 0 {
                return Err(AppError::input("load limits must be positive"));
            }
            match name.as_str() {
                "max_bundle_bytes" => limits.bundle.max_bundle_bytes = value,
                "max_total_input_bytes" => limits.max_total_input_bytes = value,
                "max_total_coefficient_bytes" => limits.bundle.max_total_coefficient_bytes = value,
                "max_collection_entries" => limits.bundle.max_collection_entries = value,
                "max_total_symbolica_state_bytes" => limits.max_total_symbolica_state_bytes = value,
                "max_zero_sector_visits" => limits.max_zero_sector_visits = value,
                "max_coefficient_bytes" => limits.bundle.max_coefficient_bytes = value,
                _ => return Err(AppError::input(format!("unknown load limit: {name}"))),
            }
        }
        if limits.bundle.max_bundle_bytes > MAX_CANDIDATE_BUNDLE_BYTES {
            return Err(AppError::limit(
                "per-owner bundle exceeds the 1 GiB hard ceiling",
            ));
        }
        Ok(limits)
    }
}

#[cfg(test)]
mod overlay_tests {
    use super::*;
    use serde_json::json;

    fn selection() -> serde_json::Value {
        json!({"family_fingerprint":"fixture", "owners":[{"path":"owner.rrbin",
            "bytes":100,"mask":"1"}], "initial_frontier_routes":[]})
    }

    #[test]
    fn no_overlay_preserves_the_existing_selection_shape() {
        let (parsed, n, _) = Selection::parse(&selection().to_string()).unwrap();
        assert_eq!(n, 1);
        assert!(parsed.domain_rule_overlays.is_empty());
        assert!(parsed.preferred_owner_programs.is_empty());
    }

    #[test]
    fn preferences_require_explicit_unique_residual_policy_and_existing_owner() {
        let mut input = selection();
        let preferred = json!({"path":"preferred.rrbin", "bytes":10,
            "owner_mask":"1", "residual_policy":"defer-to-baseline", "sha256":"diagnostic-only"});
        input["preferred_owner_programs"] = json!([preferred.clone()]);
        assert!(Selection::parse(&input.to_string()).is_ok());
        input["preferred_owner_programs"][0]
            .as_object_mut()
            .unwrap()
            .remove("residual_policy");
        assert!(Selection::parse(&input.to_string()).is_err());
        input["preferred_owner_programs"] = json!([preferred.clone()]);
        input["preferred_owner_programs"][0]["residual_policy"] = "promote-extra-terminals".into();
        assert!(Selection::parse(&input.to_string()).is_err());
        input["preferred_owner_programs"] = json!([preferred.clone(), preferred.clone()]);
        assert!(
            Selection::parse(&input.to_string())
                .unwrap_err()
                .message()
                .contains("duplicated")
        );
        input["preferred_owner_programs"] = json!([preferred.clone()]);
        input["preferred_owner_programs"][0]["owner_mask"] = "0".into();
        assert!(
            Selection::parse(&input.to_string())
                .unwrap_err()
                .message()
                .contains("missing")
        );
        input["preferred_owner_programs"] = json!([preferred]);
        input["domain_rule_overlays"] =
            json!([{"path":"repair.rrbin", "bytes":10,"owner_mask":"1"}]);
        assert!(
            Selection::parse(&input.to_string())
                .unwrap_err()
                .message()
                .contains("collision")
        );
    }

    #[test]
    fn preferences_share_all_three_payload_classes_ingress_admission() {
        let mut input = selection();
        input["owners"]
            .as_array_mut()
            .unwrap()
            .push(json!({"path":"second.rrbin","bytes":50,"mask":"0"}));
        input["preferred_owner_programs"] = json!([{"path":"preferred.rrbin", "bytes":10,
            "owner_mask":"1","residual_policy":"defer-to-baseline"}]);
        input["domain_rule_overlays"] =
            json!([{"path":"repair.rrbin","bytes":20,"owner_mask":"0"}]);
        input["load_limits"] = json!({"max_total_input_bytes":180});
        assert!(Selection::parse(&input.to_string()).is_ok());
        input["load_limits"]["max_total_input_bytes"] = 179.into();
        assert!(Selection::parse(&input.to_string()).is_err());
        input["load_limits"] = json!({"max_collection_entries":3});
        assert!(Selection::parse(&input.to_string()).is_err());
        input["load_limits"] = json!({});
        input["preferred_owner_programs"][0]["bytes"] = 0.into();
        assert!(
            Selection::parse(&input.to_string())
                .unwrap_err()
                .message()
                .contains("payload is empty")
        );
    }

    #[test]
    fn preferred_rule_subsets_are_optional_canonical_and_bounded() {
        let mut input = selection();
        input["preferred_owner_programs"] = json!([{"path":"p.rrbin","bytes":1,
            "owner_mask":"1","residual_policy":"defer-to-baseline"}]);
        let text = input.to_string();
        assert!(
            Selection::parse(&text).unwrap().0.preferred_owner_programs[0]
                .rule_ordinals
                .is_none()
        );
        assert!(!has_preferred_rule_subsets(&text));
        for value in [json!(null), json!([]), json!([0, 2, 110])] {
            input["preferred_owner_programs"][0]["rule_ordinals"] = value.clone();
            let text = input.to_string();
            let parsed = Selection::parse(&text).unwrap().0;
            assert_eq!(
                parsed.preferred_owner_programs[0].rule_ordinals.is_some(),
                !value.is_null()
            );
            assert_eq!(has_preferred_rule_subsets(&text), !value.is_null());
        }
        for value in [
            json!([1, 1]),
            json!([2, 1]),
            json!([-1]),
            json!([true]),
            json!([1.0]),
            json!("all"),
        ] {
            input["preferred_owner_programs"][0]["rule_ordinals"] = value;
            assert!(Selection::parse(&input.to_string()).is_err());
        }
        input["preferred_owner_programs"][0]["rule_ordinals"] = json!([0, 1, 2]);
        input["load_limits"] = json!({"max_collection_entries":2});
        assert!(
            Selection::parse(&input.to_string())
                .unwrap_err()
                .message()
                .contains("subset")
        );
        assert!(!has_preferred_rule_subsets("not JSON"));
    }

    #[test]
    fn overlay_order_and_owner_binding_are_explicit() {
        let mut input = selection();
        input["domain_rule_overlays"] = json!([
            {"path":"repair/first.rrbin", "bytes":12, "owner_mask":"1"},
            {"path":"repair/second.rrbin", "bytes":13, "owner_mask":"1"}
        ]);
        let (parsed, _, _) = Selection::parse(&input.to_string()).unwrap();
        assert_eq!(
            parsed.domain_rule_overlays[0].path,
            PathBuf::from("repair/first.rrbin")
        );
        assert_eq!(
            parsed.domain_rule_overlays[1].path,
            PathBuf::from("repair/second.rrbin")
        );
        input["domain_rule_overlays"][0]["owner_mask"] = "0".into();
        assert!(
            Selection::parse(&input.to_string())
                .unwrap_err()
                .to_string()
                .contains("owner is missing")
        );
    }

    #[test]
    fn overlays_share_the_aggregate_byte_and_collection_admission() {
        let mut input = selection();
        input["domain_rule_overlays"] = json!([
            {"path":"repair.rrbin", "bytes":10, "owner_mask":"1"}
        ]);
        input["load_limits"] = json!({"max_total_input_bytes":109});
        assert!(
            Selection::parse(&input.to_string())
                .unwrap_err()
                .to_string()
                .contains("ingress limits")
        );
        input["load_limits"] = json!({"max_total_input_bytes":110});
        assert!(Selection::parse(&input.to_string()).is_ok());
        input["load_limits"] = json!({"max_collection_entries":1});
        assert!(Selection::parse(&input.to_string()).is_err());
        input["load_limits"] = json!({});
        input["domain_rule_overlays"][0]["bytes"] = 0.into();
        assert!(
            Selection::parse(&input.to_string())
                .unwrap_err()
                .to_string()
                .contains("payload is empty")
        );
    }
}

pub(super) fn targets(text: &str, n: usize, max: usize) -> Result<Vec<IntegralKey>, AppError> {
    if text.len() > MAX_STEERING_BYTES {
        return Err(AppError::limit("target CSV exceeds 16 MiB"));
    }
    let mut result = Vec::new();
    for line in text.lines().filter(|l| !l.trim().is_empty()) {
        if result.len() >= max {
            return Err(AppError::limit("target count limit"));
        }
        let powers = line
            .split(',')
            .map(|s| s.trim().parse::<i64>())
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| AppError::input(format!("target CSV: {e}")))?;
        if powers.len() != n {
            return Err(AppError::input("target arity mismatch"));
        }
        result.push(IntegralKey::try_new(powers).map_err(|e| AppError::input(e.to_string()))?);
    }
    if result.is_empty() {
        return Err(AppError::input("no concrete targets"));
    }
    Ok(result)
}
