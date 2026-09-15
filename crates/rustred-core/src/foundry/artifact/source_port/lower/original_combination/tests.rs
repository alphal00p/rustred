//! Arithmetic-only budget regression using actual generated, translated rows.
//! This shifted-power fixture is not a unit-mass closing artifact candidate.
use crate::algebra::CoefficientContext;
use crate::family::{AffineDenominator, IntegralFamily};
use crate::identity::{IntegralShift, ParametricIbpGenerator};

use super::*;

#[test]
fn native_translated_condition_coordinates_obey_the_original_rule_budget() {
    let base = CoefficientContext::new(["d", "x"]);
    let shift = base
        .try_div(
            &base.one(),
            &base.parameter("x").unwrap(),
            Default::default(),
        )
        .unwrap();
    let family = IntegralFamily::new(
        "original-domain-provenance-budget",
        vec!["k".into()],
        Vec::new(),
        base.clone(),
        base.parameter("d").unwrap(),
        vec![AffineDenominator::new(base.integer(-1), vec![base.one()])],
        Vec::new(),
        vec![shift],
    )
    .unwrap();
    let generator = ParametricIbpGenerator::try_new(&family).unwrap();
    let batch = generator.prepare_ordinary_ibp().unwrap();
    let generated = batch.generate(0);
    let completed = batch.complete(vec![generated]).unwrap();
    let translated = generator
        .translate_completed_source_rows(
            &completed,
            [IntegralShift::try_new([1]).unwrap()],
            Default::default(),
        )
        .unwrap();
    let sources = SourceViewBatch::try_select(translated, &[0], Default::default()).unwrap();
    let context = generator.context();
    let provenance = sources.provenance()[0].translated();
    let contributions = vec![JoinedContribution {
        source_ordinal: 0,
        original_row: provenance.source_row().clone(),
        offset: provenance.offset().values().to_vec(),
        weight: context.one(),
    }];
    let cells: usize = sources.relations()[0]
        .nonzero_conditions()
        .iter()
        .flat_map(|condition| condition.sources())
        .map(condition_source_index_cells)
        .sum();
    assert!(
        cells > 0,
        "fixture must actually own translated condition coordinates"
    );
    let mut exact = ParametricRuleLimits::default();
    exact.max_guard_provenance_index_cells = cells;
    let (combined, normalized) =
        compile_with_limits(context, &sources, &contributions, &[], exact).unwrap();
    assert!(!combined.columns.is_empty());
    assert_eq!(normalized.len(), 1);
    let mut zero = exact;
    zero.max_guard_provenance_index_cells = 0;
    let failure = compile_with_limits(context, &sources, &contributions, &[], zero)
        .err()
        .unwrap();
    assert!(
        failure
            .0
            .contains("original condition provenance coordinate cells")
    );
    assert!(failure.0.contains(&format!("requested {cells}, limit 0")));
}
