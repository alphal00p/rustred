//! Optional exact-frame visitation over an unchanged full original-source bank.
use super::*;

pub fn ordinals(r: &Value, count: usize) -> Result<Option<Vec<usize>>> {
    let Some(value) = r.get("exact_source_ordinals") else {
        return Ok(None);
    };
    let values = value
        .as_array()
        .ok_or("exact source ordinals must be an array")?;
    require(
        !values.is_empty() && values.len() <= count,
        "exact frame size outside source bank",
    )?;
    let mut seen = BTreeSet::new();
    let mut result = Vec::with_capacity(values.len());
    for value in values {
        let ordinal = value
            .as_u64()
            .and_then(|v| usize::try_from(v).ok())
            .ok_or("exact source ordinal must be a native unsigned integer")?;
        require(
            ordinal < count && seen.insert(ordinal),
            "exact source ordinal duplicated or outside bank",
        )?;
        result.push(ordinal);
    }
    Ok(Some(result))
}

pub fn apply(span: &mut source::Span, ordinals: &[usize]) -> Result<()> {
    require(
        span.images.len() == span.weights.len(),
        "source image/weight counts differ",
    )?;
    let mut seen = BTreeSet::new();
    require(
        !ordinals.is_empty()
            && ordinals
                .iter()
                .all(|&i| i < span.images.len() && seen.insert(i)),
        "invalid exact frame selection",
    )?;
    // Move, do not duplicate, Symbolica-backed images. Original row bindings,
    // all original rows and every pre-cancellation guard remain untouched.
    let mut images = std::mem::take(&mut span.images)
        .into_iter()
        .map(Some)
        .collect::<Vec<_>>();
    let mut weights = std::mem::take(&mut span.weights)
        .into_iter()
        .map(Some)
        .collect::<Vec<_>>();
    span.images = ordinals
        .iter()
        .map(|&i| images[i].take().expect("validated unique ordinal"))
        .collect();
    span.weights = ordinals
        .iter()
        .map(|&i| weights[i].take().expect("validated unique ordinal"))
        .collect();
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn selection_is_optional_checked_and_preserves_explicit_order() {
        assert_eq!(ordinals(&json!({}), 3).unwrap(), None);
        assert_eq!(
            ordinals(&json!({"exact_source_ordinals":[2,0]}), 3).unwrap(),
            Some(vec![2, 0])
        );
        for bad in [
            json!(null),
            json!([]),
            json!([0, 0]),
            json!([3]),
            json!([-1]),
            json!([0.5]),
        ] {
            assert!(ordinals(&json!({"exact_source_ordinals":bad}), 3).is_err());
        }
    }

    #[test]
    fn full_bank_forbidden_columns_survive_sparse_frame_and_export_matches() {
        let (bytes, mut r) = crate::tests::tadpole();
        r["forbid_cofinally_higher_columns"] = json!(true);
        let (full, full_artifact) = crate::run::<1>(&bytes, &r, true).unwrap();
        assert_eq!(full["status"], "EXACT_CHART_PROVED_EXPORT_REFUSED");
        assert!(full_artifact.is_none());
        r["exact_source_ordinals"] = json!([1]);
        let (selected, selected_artifact) = crate::run::<1>(&bytes, &r, true).unwrap();
        assert_eq!(selected["status"], "CHECKED_PRIORITY_OWNER_EXPORTED");
        assert_eq!(selected["finite_bank_rows"], 2);
        assert_eq!(selected["exact_projection_rows"], 1);
        assert_eq!(
            selected["cofinal_higher_new_count"],
            full["cofinal_higher_new_count"]
        );
        assert_eq!(
            selected["attempts"][0]["forbidden_shifts"],
            full["attempts"][0]["forbidden_shifts"]
        );
        assert_eq!(
            selected["attempts"][0]["ordinary_contributions"],
            full["attempts"][0]["ordinary_contributions"]
        );
        // The full two-row elimination may retain an avoidable forward-pivot
        // guard which the conservative exporter refuses. The intended byte
        // control is the same backward source alone, without that arithmetic
        // detour; keep the full-bank F/product assertions above independently.
        let mut backward = r.clone();
        backward
            .as_object_mut()
            .unwrap()
            .remove("exact_source_ordinals");
        backward["sources"] = json!([r["sources"][1].clone()]);
        let (reference, reference_artifact) = crate::run::<1>(&bytes, &backward, true).unwrap();
        assert_eq!(reference["status"], "CHECKED_PRIORITY_OWNER_EXPORTED");
        assert_eq!(selected_artifact, reference_artifact);
    }

    #[test]
    fn selected_miss_is_not_silently_repaired_from_omitted_sources() {
        let (bytes, mut r) = crate::tests::tadpole();
        r["forbid_cofinally_higher_columns"] = json!(true);
        r["exact_source_ordinals"] = json!([0]);
        let (report, artifact) = crate::run::<1>(&bytes, &r, false).unwrap();
        assert_eq!(
            report["status"],
            "NO_TARGET_IN_SELECTED_EXACT_FRAME_WITH_CURRENT_F"
        );
        assert!(artifact.is_none());
        assert_eq!(report["finite_bank_rows"], 2);
        assert_eq!(report["exact_projection_rows"], 1);
    }

    #[test]
    fn exact_selection_is_not_a_modular_authority_input() {
        let (bytes, mut r) = crate::tests::tadpole();
        r["exact_source_ordinals"] = json!([1]);
        assert!(
            crate::run_mode::<1>(&bytes, &r, false, true)
                .unwrap_err()
                .contains("exact source selection")
        );
    }

    #[test]
    fn moving_images_preserves_original_bindings_guards_and_weight_coordinates() {
        use rustred::algebra::{CoefficientContext, IndexedCoefficientContext};
        use rustred::identity::RowId;
        let c = IndexedCoefficientContext::try_new(
            &CoefficientContext::try_new(["d"]).unwrap(),
            "frame-selection",
            1,
        )
        .unwrap();
        let guard = c
            .numerator_condition_with_limits(&c.index(0).unwrap(), Default::default())
            .unwrap();
        let mut span = source::Span {
            provenance: source::SpanProvenance::Ordinary,
            bindings: (0..2)
                .map(|i| source::SourceBinding {
                    row: RowId::OrdinaryIbp {
                        contraction_momentum: 0,
                        differentiated_loop: 0,
                    },
                    offset: IntegralShift::try_new([i]).unwrap(),
                })
                .collect(),
            originals: vec![project::Row::new(), project::Row::new()],
            images: vec![project::Row::new(), project::Row::new()],
            weights: (0..2).map(|i| BTreeMap::from([(i, c.one())])).collect(),
            guards: vec![project::Guard {
                polynomial: guard.clone(),
                origin: "omitted row assumption".into(),
            }],
        };
        apply(&mut span, &[1]).unwrap();
        assert_eq!(span.originals.len(), 2);
        assert_eq!(span.bindings.len(), 2);
        assert_eq!(span.bindings[0].offset.values(), &[0]);
        assert_eq!(span.bindings[1].offset.values(), &[1]);
        assert_eq!(span.weights, vec![BTreeMap::from([(1, c.one())])]);
        assert_eq!(span.guards.len(), 1);
        assert_eq!(span.guards[0].polynomial, guard);
        assert_eq!(span.guards[0].origin, "omitted row assumption");
    }

    #[test]
    fn optional_compaction_and_both_backends_preserve_checked_export() {
        let (bytes, mut r) = crate::tests::tadpole();
        r["forbid_cofinally_higher_columns"] = json!(true);
        r["exact_source_ordinals"] = json!([1]);
        let (_, baseline) = crate::run::<1>(&bytes, &r, true).unwrap();
        for backend in ["augmented", "direct-l"] {
            for compact in [false, true] {
                r["projection_backend"] = json!(backend);
                r["compact_coefficient_variables"] = json!(compact);
                let (report, artifact) = crate::run::<1>(&bytes, &r, true).unwrap();
                assert_eq!(report["status"], "CHECKED_PRIORITY_OWNER_EXPORTED");
                assert_eq!(artifact, baseline);
            }
        }
        r["compact_coefficient_variables"] = json!("true");
        assert!(crate::validate(&r).is_err());
    }
}
