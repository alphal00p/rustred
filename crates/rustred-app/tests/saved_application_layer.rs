//! RR-11 relations-layer tests, run against the non-test library. The bodies
//! live in `saved_application/layer_tests.rs` (also compiled as unit tests).
#![cfg(feature = "layer-test-support")]
use rustred_app::saved_application_layer_tests as layer;

#[test]
fn composition_is_exact_and_equals_sequential_application_mod_p() {
    layer::composition_is_exact_and_equals_sequential_application_mod_p();
}

#[test]
fn identity_layer_reproduces_maps_and_bytes() {
    layer::identity_layer_reproduces_maps_and_bytes();
}

#[test]
fn malformed_layers_are_rejected() {
    layer::malformed_layers_are_rejected();
}

#[test]
fn coefficient_text_grammar_is_strict() {
    layer::coefficient_text_grammar_is_strict();
}

#[test]
fn layer_files_roundtrip_with_native_table_and_text() {
    layer::layer_files_roundtrip_with_native_table_and_text();
}

#[test]
fn minimality_claim_requires_complete_matching_class_counts() {
    layer::minimality_claim_requires_complete_matching_class_counts();
}

#[test]
fn compose_layer_package_roundtrip_composed_and_layered() {
    layer::compose_layer_package_roundtrip_composed_and_layered();
}

#[test]
fn masters_with_raw_rows_must_compose_to_identity() {
    layer::masters_with_raw_rows_must_compose_to_identity();
}
