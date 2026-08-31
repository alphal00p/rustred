use rustred::algebra::CoefficientContext;
use rustred::family::{AffineDenominator, IntegralFamily};
use rustred::identity::{
    IntegralShift, ParametricIbpGenerator, SelectedTranslatedSourceBatch, TranslatedSourceBatch,
    TranslatedSourceLimits, TranslatedSourceRequest,
};

#[test]
fn public_api_translates_a_sealed_complete_batch_without_exposing_its_owner() {
    let base = CoefficientContext::try_new(["d"]).unwrap();
    let family = IntegralFamily::new(
        "public-translated-source-tadpole",
        vec!["k".into()],
        Vec::new(),
        base.clone(),
        base.parameter("d").unwrap(),
        vec![AffineDenominator::new(base.integer(-1), vec![base.one()])],
        Vec::new(),
        vec![base.zero()],
    )
    .unwrap();
    let generator = ParametricIbpGenerator::try_new(&family).unwrap();
    let prepared = generator.prepare_ordinary_ibp().unwrap();
    let row = prepared.generate(0);
    let completed = prepared.complete(vec![row]).unwrap();

    let translated: TranslatedSourceBatch = generator
        .translate_completed_source_rows(
            &completed,
            [IntegralShift::try_new([-1]).unwrap()],
            TranslatedSourceLimits::default(),
        )
        .unwrap();
    assert_eq!(translated.offsets()[0].values(), &[-1]);
    assert_eq!(
        translated.sources()[0].row_id().stable_string(),
        "ordinary-ibp:0:0"
    );
    assert_eq!(
        translated.sources()[0].provenance().stable_string(),
        "translated-source-v1:0:ordinary-ibp:0:0:[-1]"
    );
    assert!(!translated.sources()[0].terms().is_empty());

    let selected: SelectedTranslatedSourceBatch = generator
        .translate_selected_completed_source_rows(
            &completed,
            [TranslatedSourceRequest::new(
                0,
                IntegralShift::try_new([-1]).unwrap(),
            )],
            TranslatedSourceLimits::default(),
        )
        .unwrap();
    assert_eq!(selected.completed_source_row_count(), 1);
    assert_eq!(selected.requests()[0].source_ordinal(), 0);
    assert_eq!(selected.requests()[0].offset().values(), &[-1]);
    assert_eq!(&selected.sources()[0], &translated.sources()[0]);
}
