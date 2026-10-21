//! Read-only actual saved-family P102 identity gate; no source publication writes.
use rustred::algebra::CoefficientContext;
use rustred::family::isp::IspCompletion;
use rustred::family::{AffineDenominator, IntegralFamily, IntegralKey};
use rustred::sector::Mask;
use rustred::sector::symmetry::integral_transport;
use rustred::sector::symmetry::{
    self, CoefficientMatrix, DenominatorAction, Jacobian, MomentumMap,
};
use rustred_app::load_master_reduction;
use serde_json::json;
use std::error::Error;
use std::path::Path;
use std::sync::Arc;

fn target_family(context: &CoefficientContext) -> Result<Arc<IntegralFamily>, Box<dyn Error>> {
    // Exact pinned DOT stored basis a,b,h,c,q; final input row is complete N.
    let momenta = [
        [-1, -1, 0, 1, 1],
        [1, 0, 0, 0, 0],
        [0, 1, 0, 0, 0],
        [0, 0, 1, 0, 1],
        [0, 0, 1, 0, 0],
        [0, 0, 0, 1, 0],
        [0, 0, 0, 0, 1],
        [1, 1, 0, -1, 0],
    ];
    let denominators = momenta
        .into_iter()
        .map(|p| {
            let mut row = vec![];
            for i in 0..5 {
                for j in i..5 {
                    row.push(context.integer(p[i] * p[j] * if i == j { 1 } else { 2 }));
                }
            }
            AffineDenominator::new(context.integer(-1), row)
        })
        .collect();
    Ok(Arc::new(
        IspCompletion::try_new(
            "P102_actual_DOT_basis_independent_witness",
            (0..5).map(|i| format!("l{i}")).collect(),
            vec![],
            context.clone(),
            context.parameter("d").unwrap(),
            denominators,
            vec![],
            vec![context.zero(); 8],
        )?
        .into_family(),
    ))
}

fn main() -> Result<(), Box<dyn Error>> {
    let path = std::env::args_os()
        .nth(1)
        .ok_or("expected pinned artifact path")?;
    let manifest = Path::new(&path).join("artifact.json");
    let before = blake3::hash(&std::fs::read(&manifest)?);
    let artifact = load_master_reduction(Path::new(&path))?;
    let session = artifact.primary_session();
    let source = session.family_owner();
    assert_eq!(source.loop_count(), 5);
    assert_eq!(source.denominator_count(), 15);
    let context = source.coefficient_context();
    let target = target_family(context)?;
    // Raw DOT common_basis_to_graph_loops: source k_i = M_ij graph l_j.
    let matrix = [
        [0, -1, 0, 1, 0],
        [-1, 0, 0, 1, 0],
        [-1, -1, 0, 1, 1],
        [-1, -1, 0, 1, 0],
        [-1, -1, -1, 1, 0],
    ];
    let proposed = MomentumMap::new(
        CoefficientMatrix::try_new(
            5,
            5,
            matrix.into_iter().flatten().map(|i| context.integer(i)),
        )?,
        CoefficientMatrix::try_new(5, 0, [])?,
        CoefficientMatrix::try_new(0, 0, [])?,
    );
    let verified = Arc::new(symmetry::verify(
        source,
        &target,
        proposed,
        Default::default(),
    )?);
    assert!(matches!(verified.jacobian(), Jacobian::Unit { .. }));
    assert_eq!(verified.source_family_fingerprint(), source.fingerprint());
    assert_eq!(verified.target_family_fingerprint(), target.fingerprint());
    let physical = [2, 6, 9, 11, 12, 13, 14];
    for (destination, source_slot) in physical.into_iter().enumerate().chain([(7, 3)]) {
        let DenominatorAction::Monomial {
            target: found,
            scale,
        } = &verified.row_actions()[source_slot]
        else {
            return Err(
                format!("source axis {source_slot} did not map to one exact denominator").into(),
            );
        };
        assert_eq!(*found, destination);
        assert_eq!(scale, &context.one());
    }
    let source_key = IntegralKey::try_new([0, 0, 1, -1, 0, 0, 1, 0, 0, 1, 0, 1, 1, 1, 1])?;
    let expected = IntegralKey::try_new([1, 1, 1, 1, 1, 1, 1, -1, 0, 0, 0, 0, 0, 0, 0])?;
    let transport = integral_transport::compile(
        source,
        Arc::clone(&target),
        verified,
        Mask::try_from_indices(source_key.powers())?,
        Mask::try_from_indices(expected.powers())?,
        Default::default(),
    )?;
    let image = transport.transport(&source_key, Default::default())?;
    assert_eq!(image.terms().len(), 1);
    assert_eq!(image.terms()[0].key(), &expected);
    assert_eq!(image.terms()[0].coefficient(), &context.one());
    let raw_member = session.raw_terminals().contains(&source_key);
    let current = if raw_member {
        let map = artifact.apply_terminal(source, &source_key)?;
        json!({
            "terms": map.terms().iter().map(|(key, coefficient)|json!({
                "family_fingerprint_blake3": blake3::hash(key.family_fingerprint().as_bytes()).to_hex().to_string(),
                "indices": key.integral().powers(), "coefficient": coefficient.to_string(),
            })).collect::<Vec<_>>(),
            "nonzero_conditions":map.nonzero_conditions().len(),
            "is_identity":map.terms().len()==1 && map.terms().iter().next().is_some_and(|(key,c)|
                key.family_fingerprint()==source.fingerprint() && key.integral()==&source_key && c==&context.one()),
        })
    } else {
        json!(null)
    };
    assert_eq!(before, blake3::hash(&std::fs::read(&manifest)?));
    println!(
        "{}",
        serde_json::to_string_pretty(&json!({
            "status":"passed_actual_saved_family_mapping",
            "artifact":path.to_string_lossy(), "source_manifest_blake3":before.to_hex().to_string(),
            "source_family_fingerprint": source.fingerprint(),
            "source_family_fingerprint_blake3":blake3::hash(source.fingerprint().as_bytes()).to_hex().to_string(),
            "source_key":source_key.powers(), "stored_DOT_lmb_edges":[1,2,4,5,6],
            "physical_source_slots":physical, "complete_numerator_source_slot":3,
            "verified_complete_numerator_transport":true, "raw_terminal_member":raw_member,
            "current_published_reduction":current,
        }))?
    );
    Ok(())
}
