use std::error::Error;

use rustred::{
    family::IntegralKey, foundry::artifact::derive_two_loop_unit_mass_sunset, reduction::Reducer,
};
use symbolica::prelude::AtomCore;

fn main() -> Result<(), Box<dyn Error>> {
    // Generate the complete unit-mass K=3 artifact from four ordinary IBP
    // sources. The builder derives every recurrence, residual projection,
    // exact S3 route, zero terminal, and pinched-face factorization.
    let artifact = derive_two_loop_unit_mass_sunset()?;
    let durable = artifact.encode_durable()?;

    assert_eq!(artifact.source_relations().len(), 4);
    assert_eq!(artifact.rule_cells().len(), 5);
    assert_eq!(artifact.canonicalizer().unwrap().group_order(), 6);
    assert_eq!(artifact.masters().len(), 2);
    assert_eq!(artifact.zero_sectors().len(), 4);

    println!("algorithm = {}", artifact.algorithm_id());
    println!("ordinary_sources = {}", artifact.source_relations().len());
    println!("closing_rule_cells = {}", artifact.rule_cells().len());
    println!("durable_bytes = {}", durable.len());
    for source in artifact.source_relations() {
        println!("source = {}", source.row_id().stable_string());
    }

    // Apply the generated parametric IBPs to a representative dotted sunset.
    // The common mass was set to one in the artifact; the separate powers of
    // m^2 restore an arbitrary common mass by dimensional homogeneity.
    let target = IntegralKey::try_new([2, 2, 1])?;
    let mut reducer = Reducer::new(&artifact)?;
    let reduction = reducer.reduce_with_common_mass_homogeneity(&target)?;
    println!("target = {:?}", target.powers());
    for (master, coefficient) in reduction.terms() {
        println!(
            "master {:?}: coefficient = {}, mass_squared_power = {}",
            master.powers(),
            coefficient
                .unit_mass_coefficient()
                .to_expression()
                .to_canonical_string(),
            coefficient.common_mass_squared_power(),
        );
    }
    Ok(())
}
