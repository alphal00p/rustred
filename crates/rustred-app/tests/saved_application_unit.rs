//! Existing saved-application unit tests, run against the non-test library
//! (regression coverage for the RR-11 codec/loader/substitution changes).
#![cfg(feature = "layer-test-support")]
use rustred_app::saved_application_unit_tests as unit;

#[test]
fn partial_saved_application_loads_policy_without_changing_recorded_coverage() {
    unit::partial_saved_application_loads_policy_without_changing_recorded_coverage();
}

#[test]
fn partial_saved_substitution_preserves_unpublished_leaves_and_cancellation() {
    unit::partial_saved_substitution_preserves_unpublished_leaves_and_cancellation();
}

#[test]
fn native_compact_maps_preserve_every_coefficient_context_and_guard() {
    unit::native_compact_maps_preserve_every_coefficient_context_and_guard();
}

#[test]
fn application_progress_preserves_output_and_orders_completion_after_substitution() {
    unit::application_progress_preserves_output_and_orders_completion_after_substitution();
}

#[test]
fn application_progress_reports_admission_and_terminal_substitution_failures() {
    unit::application_progress_reports_admission_and_terminal_substitution_failures();
}

#[test]
fn compact_boundary_rejects_unknown_terminals_and_impossible_guards() {
    unit::compact_boundary_rejects_unknown_terminals_and_impossible_guards();
}

#[test]
fn thousands_of_overlapping_guards_keep_first_seen_order_on_warm_application() {
    unit::thousands_of_overlapping_guards_keep_first_seen_order_on_warm_application();
}

#[test]
fn runtime_is_portable_reuses_cache_and_keeps_exact_scope_holes() {
    unit::runtime_is_portable_reuses_cache_and_keeps_exact_scope_holes();
}

#[test]
fn runtime_options_select_native_cache_representation_and_report_preparation() {
    unit::runtime_options_select_native_cache_representation_and_report_preparation();
}

#[test]
fn compressed_terminal_export_and_legacy_raw_load_have_identical_maps_and_metadata() {
    unit::compressed_terminal_export_and_legacy_raw_load_have_identical_maps_and_metadata();
}

#[test]
fn compact_saved_maps_use_caller_byte_grants_and_optional_count_diagnostics() {
    unit::compact_saved_maps_use_caller_byte_grants_and_optional_count_diagnostics();
}

#[test]
fn saved_runtime_eviction_is_default_and_preserves_exact_refined_outputs() {
    unit::saved_runtime_eviction_is_default_and_preserves_exact_refined_outputs();
}

#[test]
fn deferred_runtime_owns_authenticated_bytes_before_first_formula_use() {
    unit::deferred_runtime_owns_authenticated_bytes_before_first_formula_use();
}

#[test]
fn entry_routing_uses_only_admitted_images_and_preserves_exact_batch_coefficients() {
    unit::entry_routing_uses_only_admitted_images_and_preserves_exact_batch_coefficients();
}

#[test]
fn saved_application_persistent_file_preserves_terminal_substitutions_and_scope() {
    unit::saved_application_persistent_file_preserves_terminal_substitutions_and_scope();
}

#[test]
fn advisory_saved_coverage_is_independent_of_strict_complete_application() {
    unit::advisory_saved_coverage_is_independent_of_strict_complete_application();
}

#[test]
fn public_saved_application_keeps_resource_causes_and_can_retry() {
    unit::public_saved_application_keeps_resource_causes_and_can_retry();
}

#[test]
fn public_saved_input_count_template_is_inactive_until_diagnostics_are_enabled() {
    unit::public_saved_input_count_template_is_inactive_until_diagnostics_are_enabled();
}
