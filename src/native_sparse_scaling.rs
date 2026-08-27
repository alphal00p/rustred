//! Topology-neutral reporting for committed native Symbolica sparse stages.
//!
//! These snapshots are observational campaign data. They do not own reducer
//! state, participate in algebraic admission or replay identity, or retain a
//! per-event history. A driver that needs a scaling curve should sample the
//! fixed-size snapshot after each committed transition and retain that series
//! outside the exact database.
//!
//! Wall time, CPU time, RSS, hardware, and worker-count observations belong in
//! a benchmark-runner envelope or sidecar. They are intentionally absent here
//! so nondeterministic process measurements cannot become algebraic state.

use serde::{Serialize, Serializer};

use crate::generated_affine_residual_group_exact_database::{
    GeneratedAffineResidualGroupExactNativeSparseScalingStats,
    GeneratedAffineResidualGroupExactNativeSparseStageStats,
};

pub(crate) const NATIVE_SYMBOLICA_SPARSE_SCALING_V1_SCHEMA: &str =
    "rustred.native-symbolica-sparse-scaling.v1";
const COMMITTED_EXACT_DATABASE_STAGES_SCOPE: &str = "committed_exact_database_stages";
const UNSIGNED_DECIMAL_STRING_COUNTER_ENCODING: &str = "unsigned-decimal-string";

fn serialize_usize_as_unsigned_decimal_string<S>(
    value: &usize,
    serializer: S,
) -> Result<S::Ok, S::Error>
where
    S: Serializer,
{
    serializer.collect_str(value)
}

/// Exact counters for one successfully validated native reconstruction.
///
/// `rows` includes the candidate row. `physical_columns` excludes the unused
/// sentinel column installed solely for full-rank dependent transcripts.
/// `prospective_native_output_entries` is the conservative admitted U+L
/// envelope; `observed_native_output_entries` is the checked native U+L
/// census. Every `*_entries` value here counts stored sparse slots, not
/// semantic nonzeros. Coefficient-work counters cover checked adapter copies
/// and native field callbacks, but not catalog sorting, structural validation,
/// or the database's guarded provenance replay.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize)]
pub(crate) struct NativeSparseStageSnapshot {
    #[serde(serialize_with = "serialize_usize_as_unsigned_decimal_string")]
    rows: usize,
    #[serde(serialize_with = "serialize_usize_as_unsigned_decimal_string")]
    physical_columns: usize,
    #[serde(serialize_with = "serialize_usize_as_unsigned_decimal_string")]
    input_entries: usize,
    #[serde(serialize_with = "serialize_usize_as_unsigned_decimal_string")]
    prospective_native_output_entries: usize,
    #[serde(serialize_with = "serialize_usize_as_unsigned_decimal_string")]
    observed_native_output_entries: usize,
    #[serde(serialize_with = "serialize_usize_as_unsigned_decimal_string")]
    native_u_entries: usize,
    #[serde(serialize_with = "serialize_usize_as_unsigned_decimal_string")]
    native_l_entries: usize,
    #[serde(serialize_with = "serialize_usize_as_unsigned_decimal_string")]
    returned_trace_entries: usize,
    #[serde(serialize_with = "serialize_usize_as_unsigned_decimal_string")]
    coefficient_algebra_work: usize,
    #[serde(serialize_with = "serialize_usize_as_unsigned_decimal_string")]
    coefficient_exponent_entry_work: usize,
    #[serde(serialize_with = "serialize_usize_as_unsigned_decimal_string")]
    coefficient_integer_bit_work: usize,
}

impl NativeSparseStageSnapshot {
    pub(crate) const fn rows(self) -> usize {
        self.rows
    }

    pub(crate) const fn physical_columns(self) -> usize {
        self.physical_columns
    }

    pub(crate) const fn input_entries(self) -> usize {
        self.input_entries
    }

    pub(crate) const fn prospective_native_output_entries(self) -> usize {
        self.prospective_native_output_entries
    }

    pub(crate) const fn observed_native_output_entries(self) -> usize {
        self.observed_native_output_entries
    }

    pub(crate) const fn native_u_entries(self) -> usize {
        self.native_u_entries
    }

    pub(crate) const fn native_l_entries(self) -> usize {
        self.native_l_entries
    }

    pub(crate) const fn returned_trace_entries(self) -> usize {
        self.returned_trace_entries
    }

    pub(crate) const fn coefficient_algebra_work(self) -> usize {
        self.coefficient_algebra_work
    }

    pub(crate) const fn coefficient_exponent_entry_work(self) -> usize {
        self.coefficient_exponent_entry_work
    }

    pub(crate) const fn coefficient_integer_bit_work(self) -> usize {
        self.coefficient_integer_bit_work
    }
}

impl From<GeneratedAffineResidualGroupExactNativeSparseStageStats> for NativeSparseStageSnapshot {
    fn from(value: GeneratedAffineResidualGroupExactNativeSparseStageStats) -> Self {
        Self {
            rows: value.rows(),
            physical_columns: value.physical_columns(),
            input_entries: value.input_entries(),
            prospective_native_output_entries: value.prospective_native_output_entries(),
            observed_native_output_entries: value.observed_native_output_entries(),
            native_u_entries: value.native_u_entries(),
            native_l_entries: value.native_l_entries(),
            returned_trace_entries: value.returned_trace_entries(),
            coefficient_algebra_work: value.coefficient_algebra_work(),
            coefficient_exponent_entry_work: value.coefficient_exponent_entry_work(),
            coefficient_integer_bit_work: value.coefficient_integer_bit_work(),
        }
    }
}

/// Fixed-size snapshot of committed native sparse work.
///
/// `componentwise_peak` contains independent maxima and therefore need not be
/// a realizable stage; in particular, its observed-output maximum need not be
/// the sum of its independently maximized U and L counters. `cumulative`
/// saturates componentwise, and `cumulative_saturated` also covers saturation
/// of `committed_stage_count`. Serialization keeps the internal `usize`
/// counters exact as unsigned decimal strings, including saturated values that
/// exceed TOML's signed-integer range.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
pub(crate) struct NativeSparseScalingSnapshot {
    schema: &'static str,
    scope: &'static str,
    counter_encoding: &'static str,
    #[serde(serialize_with = "serialize_usize_as_unsigned_decimal_string")]
    committed_stage_count: usize,
    cumulative_saturated: bool,
    last: NativeSparseStageSnapshot,
    componentwise_peak: NativeSparseStageSnapshot,
    cumulative: NativeSparseStageSnapshot,
}

impl Default for NativeSparseScalingSnapshot {
    fn default() -> Self {
        Self {
            schema: NATIVE_SYMBOLICA_SPARSE_SCALING_V1_SCHEMA,
            scope: COMMITTED_EXACT_DATABASE_STAGES_SCOPE,
            counter_encoding: UNSIGNED_DECIMAL_STRING_COUNTER_ENCODING,
            committed_stage_count: 0,
            cumulative_saturated: false,
            last: NativeSparseStageSnapshot::default(),
            componentwise_peak: NativeSparseStageSnapshot::default(),
            cumulative: NativeSparseStageSnapshot::default(),
        }
    }
}

impl NativeSparseScalingSnapshot {
    pub(crate) const fn schema(self) -> &'static str {
        self.schema
    }

    pub(crate) const fn scope(self) -> &'static str {
        self.scope
    }

    pub(crate) const fn counter_encoding(self) -> &'static str {
        self.counter_encoding
    }

    pub(crate) const fn committed_stage_count(self) -> usize {
        self.committed_stage_count
    }

    pub(crate) const fn cumulative_saturated(self) -> bool {
        self.cumulative_saturated
    }

    pub(crate) const fn last(self) -> NativeSparseStageSnapshot {
        self.last
    }

    pub(crate) const fn componentwise_peak(self) -> NativeSparseStageSnapshot {
        self.componentwise_peak
    }

    pub(crate) const fn cumulative(self) -> NativeSparseStageSnapshot {
        self.cumulative
    }
}

impl From<GeneratedAffineResidualGroupExactNativeSparseScalingStats>
    for NativeSparseScalingSnapshot
{
    fn from(value: GeneratedAffineResidualGroupExactNativeSparseScalingStats) -> Self {
        Self {
            schema: NATIVE_SYMBOLICA_SPARSE_SCALING_V1_SCHEMA,
            scope: COMMITTED_EXACT_DATABASE_STAGES_SCOPE,
            counter_encoding: UNSIGNED_DECIMAL_STRING_COUNTER_ENCODING,
            committed_stage_count: value.event_count(),
            cumulative_saturated: value.cumulative_saturated(),
            last: value.last().into(),
            componentwise_peak: value.componentwise_peak().into(),
            cumulative: value.cumulative().into(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn stage(seed: usize) -> NativeSparseStageSnapshot {
        NativeSparseStageSnapshot {
            rows: seed + 1,
            physical_columns: seed + 2,
            input_entries: seed + 3,
            prospective_native_output_entries: seed + 7,
            observed_native_output_entries: seed + 6,
            native_u_entries: seed + 4,
            native_l_entries: 2,
            returned_trace_entries: seed,
            coefficient_algebra_work: seed + 11,
            coefficient_exponent_entry_work: seed + 13,
            coefficient_integer_bit_work: seed + 17,
        }
    }

    #[test]
    fn topology_neutral_snapshot_has_deterministic_toml_bytes() {
        let snapshot = NativeSparseScalingSnapshot {
            schema: NATIVE_SYMBOLICA_SPARSE_SCALING_V1_SCHEMA,
            scope: COMMITTED_EXACT_DATABASE_STAGES_SCOPE,
            counter_encoding: UNSIGNED_DECIMAL_STRING_COUNTER_ENCODING,
            committed_stage_count: 3,
            cumulative_saturated: false,
            last: stage(3),
            componentwise_peak: stage(5),
            cumulative: stage(19),
        };
        let first = toml::to_string_pretty(&snapshot).unwrap();
        let second = toml::to_string_pretty(&snapshot).unwrap();
        assert_eq!(first.as_bytes(), second.as_bytes());

        let document: toml::Value = toml::from_str(&first).unwrap();
        let table = document.as_table().unwrap();
        assert_eq!(
            table.keys().map(String::as_str).collect::<Vec<_>>(),
            [
                "committed_stage_count",
                "componentwise_peak",
                "counter_encoding",
                "cumulative",
                "cumulative_saturated",
                "last",
                "schema",
                "scope",
            ]
        );
        assert_eq!(
            document["schema"].as_str(),
            Some(NATIVE_SYMBOLICA_SPARSE_SCALING_V1_SCHEMA)
        );
        assert_eq!(
            document["scope"].as_str(),
            Some(COMMITTED_EXACT_DATABASE_STAGES_SCOPE)
        );
        assert_eq!(
            document["counter_encoding"].as_str(),
            Some(UNSIGNED_DECIMAL_STRING_COUNTER_ENCODING)
        );
        assert_eq!(document["committed_stage_count"].as_str(), Some("3"));
        assert_eq!(document["last"]["rows"].as_str(), Some("4"));
        assert_eq!(
            document["last"]["observed_native_output_entries"].as_str(),
            Some("9")
        );
        assert_eq!(document["last"]["native_u_entries"].as_str(), Some("7"));
        assert_eq!(document["last"]["native_l_entries"].as_str(), Some("2"));
        assert!(!first.contains("family"));
        assert!(!first.contains("sector"));
        assert!(!first.contains("topology"));
        assert!(!first.contains("wall_time"));
        assert!(!first.contains("rss"));
    }

    #[test]
    fn default_snapshot_is_an_empty_committed_census() {
        let snapshot = NativeSparseScalingSnapshot::default();
        assert_eq!(snapshot.schema(), NATIVE_SYMBOLICA_SPARSE_SCALING_V1_SCHEMA);
        assert_eq!(snapshot.scope(), COMMITTED_EXACT_DATABASE_STAGES_SCOPE);
        assert_eq!(
            snapshot.counter_encoding(),
            UNSIGNED_DECIMAL_STRING_COUNTER_ENCODING
        );
        assert_eq!(snapshot.committed_stage_count(), 0);
        assert!(!snapshot.cumulative_saturated());
        assert_eq!(snapshot.last(), NativeSparseStageSnapshot::default());
        assert_eq!(
            snapshot.componentwise_peak(),
            NativeSparseStageSnapshot::default()
        );
        assert_eq!(snapshot.cumulative(), NativeSparseStageSnapshot::default());
    }

    #[test]
    fn saturated_snapshot_remains_valid_exact_toml() {
        let maximum = NativeSparseStageSnapshot {
            rows: usize::MAX,
            physical_columns: usize::MAX,
            input_entries: usize::MAX,
            prospective_native_output_entries: usize::MAX,
            observed_native_output_entries: usize::MAX,
            native_u_entries: usize::MAX,
            native_l_entries: usize::MAX,
            returned_trace_entries: usize::MAX,
            coefficient_algebra_work: usize::MAX,
            coefficient_exponent_entry_work: usize::MAX,
            coefficient_integer_bit_work: usize::MAX,
        };
        let snapshot = NativeSparseScalingSnapshot {
            schema: NATIVE_SYMBOLICA_SPARSE_SCALING_V1_SCHEMA,
            scope: COMMITTED_EXACT_DATABASE_STAGES_SCOPE,
            counter_encoding: UNSIGNED_DECIMAL_STRING_COUNTER_ENCODING,
            committed_stage_count: usize::MAX,
            cumulative_saturated: true,
            last: maximum,
            componentwise_peak: maximum,
            cumulative: maximum,
        };

        let serialized = toml::to_string_pretty(&snapshot).unwrap();
        let document: toml::Value = toml::from_str(&serialized).unwrap();
        let expected = usize::MAX.to_string();
        assert_eq!(
            document["committed_stage_count"].as_str(),
            Some(expected.as_str())
        );
        for section in ["last", "componentwise_peak", "cumulative"] {
            for counter in [
                "rows",
                "physical_columns",
                "input_entries",
                "prospective_native_output_entries",
                "observed_native_output_entries",
                "native_u_entries",
                "native_l_entries",
                "returned_trace_entries",
                "coefficient_algebra_work",
                "coefficient_exponent_entry_work",
                "coefficient_integer_bit_work",
            ] {
                assert_eq!(
                    document[section][counter].as_str(),
                    Some(expected.as_str()),
                    "counter {section}.{counter} lost its exact usize value"
                );
            }
        }
        assert!(document["cumulative_saturated"].as_bool().unwrap());
    }
}
