use super::*;
use crate::foundry::artifact::derive_two_loop_unit_mass_sunset;
use std::sync::Arc;

#[test]
fn dynamic_cell_plan_rejects_combined_replay_before_writing_an_anchor() {
    // The per-cell plan encoder is arity-generic. Authentic existing cells
    // exercise its proof-kind gate without pretending to construct a K6
    // owner, whose new combined-domain grammar is not implemented yet.
    let mut artifact = derive_two_loop_unit_mass_sunset().unwrap();
    for cell in &mut artifact.rule_cells {
        encode_cell_plan(&mut Writer::new(Default::default()), cell).unwrap();
        Arc::get_mut(cell)
            .expect("fresh artifact owns the cell uniquely")
            .replace_replay_with_uncertified_combined_domain_for_test();
        let mut writer = Writer::new(Default::default());
        assert!(matches!(
            encode_cell_plan(&mut writer, cell),
            Err(ArtifactPersistenceError::UnsupportedFeature { .. })
        ));
        assert!(
            writer.finish().is_empty(),
            "unsupported proof must not emit a partial cell plan"
        );
    }
}
