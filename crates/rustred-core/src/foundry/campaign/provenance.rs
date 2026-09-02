//! Non-authoritative search provenance for foundry diagnostics.

/// Provenance of search-policy inputs supplied to a campaign.
///
/// This value is deliberately absent from exact owner, artifact, and reducer
/// types. External hints may select seeds, ordering, domains, or an itinerary,
/// but RustRed still derives and authenticates every identity from its native
/// ordinary IBP module. No recurrence RHS, coefficient, or rule payload can be
/// represented here.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum FoundrySearchProvenance {
    #[default]
    Autonomous,
    ExternalHintsOnly,
}

impl FoundrySearchProvenance {
    pub const AUTONOMOUS_ID: &'static str = "autonomous";
    pub const EXTERNAL_HINTS_ONLY_ID: &'static str = "external-hints-only";

    pub const fn stable_id(self) -> &'static str {
        match self {
            Self::Autonomous => Self::AUTONOMOUS_ID,
            Self::ExternalHintsOnly => Self::EXTERNAL_HINTS_ONLY_ID,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stable_ids_are_report_only_and_contain_no_rule_payload() {
        assert_eq!(
            FoundrySearchProvenance::Autonomous.stable_id(),
            "autonomous"
        );
        assert_eq!(
            FoundrySearchProvenance::ExternalHintsOnly.stable_id(),
            "external-hints-only"
        );
    }
}
