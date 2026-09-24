use std::fs::File;
use std::io::Read;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use rustred::sector::symmetry::{self, CoefficientMatrix, MomentumMap, integral_transport};
use rustred::solver::{CandidateOwnerRoute, RoutedCandidateReducer};
use serde_json::{Value, json};
use symbolica::prelude::{Matrix, Q, Rational};

use super::{
    RoutedCampaignRequest,
    input::{Selection, mask},
};
use crate::{
    AppError, CandidateOwnerBundle, CandidateOwnerLoadLimits, load_generated_candidate_owners,
};

fn matrix(
    rows: &[Vec<String>],
) -> Result<Matrix<symbolica::domains::rational::RationalField>, AppError> {
    let values = rows
        .iter()
        .map(|row| {
            row.iter()
                .map(|x| x.parse::<i64>().map(Rational::from))
                .collect::<Result<Vec<_>, _>>()
        })
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| AppError::input(e.to_string()))?;
    Matrix::from_nested_vec(values, Q).map_err(|e| AppError::input(format!("native matrix: {e:?}")))
}

pub(super) fn prepare<const N: usize>(
    request: &RoutedCampaignRequest,
    selection: &Selection,
    limits: CandidateOwnerLoadLimits,
    cancellation: &AtomicBool,
    observer: &impl Fn(Value),
) -> Result<Option<RoutedCandidateReducer<N>>, AppError> {
    prepare_with_fingerprints(request, selection, limits, cancellation, observer, None)
}

/// Bind checkpoint identity to the exact admitted payloads before native import.
pub(super) fn prepare_with_fingerprints<const N: usize>(
    request: &RoutedCampaignRequest,
    selection: &Selection,
    limits: CandidateOwnerLoadLimits,
    cancellation: &AtomicBool,
    observer: &impl Fn(Value),
    mut fingerprints: Option<&mut dyn FnMut(Vec<String>) -> Result<(), String>>,
) -> Result<Option<RoutedCandidateReducer<N>>, AppError> {
    let mut bytes = Vec::new();
    let mut masks = Vec::new();
    for (ordinal, owner) in selection.owners.iter().enumerate() {
        if cancellation.load(Ordering::Relaxed) {
            return Ok(None);
        }
        observer(
            json!({"event":"preparation", "phase":"owner_read", "completed":ordinal, "total":selection.owners.len()}),
        );
        let path = request.owner_base.join(&owner.path);
        let file =
            File::open(&path).map_err(|e| AppError::input(format!("{}: {e}", path.display())))?;
        if file
            .metadata()
            .map_err(|e| AppError::input(e.to_string()))?
            .len()
            != owner.bytes
        {
            return Err(AppError::input(format!(
                "{}: owner size differs from manifest",
                path.display()
            )));
        }
        let mut payload = Vec::new();
        file.take(owner.bytes + 1)
            .read_to_end(&mut payload)
            .map_err(|e| AppError::input(format!("{}: {e}", path.display())))?;
        if u64::try_from(payload.len()).ok() != Some(owner.bytes) {
            return Err(AppError::input("owner changed during bounded read"));
        }
        bytes.push(payload);
        masks.push(mask(&owner.mask, N)?);
    }
    if let Some(bind) = fingerprints.as_mut() {
        bind(
            bytes
                .iter()
                .map(|payload| blake3::hash(payload).to_hex().to_string())
                .collect(),
        )
        .map_err(AppError::input)?;
    }
    observer(json!({"event":"preparation", "phase":"native_owner_load", "owners":bytes.len()}));
    if cancellation.load(Ordering::Relaxed) {
        return Ok(None);
    }
    let inputs = bytes
        .iter()
        .zip(&masks)
        .map(|(bytes, owner_sector)| CandidateOwnerBundle {
            bytes,
            owner_sector,
        })
        .collect::<Vec<_>>();
    let (family, programs) =
        load_generated_candidate_owners::<N>(&inputs, limits, request.reduction_limits)?;
    if family.fingerprint() != selection.family_fingerprint {
        return Err(AppError::input(
            "loaded family differs from selection fingerprint",
        ));
    }
    if family.external_count() != 0 {
        return Err(AppError::input(
            "this signed loop-map selection format requires a vacuum family",
        ));
    }
    let owner_count = programs.owner_count();
    let terminal_count = programs.terminal_count();
    drop(inputs);
    drop(bytes);
    observer(
        json!({"event":"loaded", "owners":owner_count, "saved_terminals":terminal_count,
        "family_fingerprint":family.fingerprint(), "rank_bound":programs.context().scope().max_numerator_rank,
        "finite_case_policy":format!("{:?}", programs.context().scope().finite_case_policy)}),
    );
    let mut routes = Vec::new();
    for (ordinal, route) in selection.initial_frontier_routes.iter().enumerate() {
        if cancellation.load(Ordering::Relaxed) {
            return Ok(None);
        }
        let loops = family.loop_count();
        if route.source_to_representative.len() != loops
            || route.owner_to_representative.len() != loops
        {
            return Err(AppError::input(
                "route matrix dimensions differ from native family",
            ));
        }
        if !route.requires_transport {
            continue;
        }
        observer(
            json!({"event":"preparation", "phase":"native_map_verification", "completed":ordinal,
            "total":selection.initial_frontier_routes.len(), "verified":routes.len()}),
        );
        let source = mask(&route.source_mask, N)?;
        let target = mask(&route.owner_mask, N)?;
        let inverse = matrix(&route.owner_to_representative)?
            .inv()
            .map_err(|e| AppError::input(format!("native route inverse: {e:?}")))?;
        let composed = &matrix(&route.source_to_representative)? * &inverse;
        let context = family.coefficient_context();
        let mut entries = Vec::new();
        for i in 0..loops {
            for j in 0..loops {
                let value = composed[(i as u32, j as u32)]
                    .to_string()
                    .parse::<i64>()
                    .map_err(|_| {
                        AppError::input("composed witness must remain an integral loop map")
                    })?;
                entries.push(context.integer(value));
            }
        }
        let map_error = |e| AppError::input(format!("native map: {e:?}"));
        let momentum = MomentumMap::new(
            CoefficientMatrix::try_new(loops, loops, entries).map_err(map_error)?,
            CoefficientMatrix::try_new(loops, 0, []).map_err(map_error)?,
            CoefficientMatrix::try_new(0, 0, []).map_err(map_error)?,
        );
        let verified = symmetry::verify(&family, &family, momentum, Default::default())
            .map_err(|e| AppError::input(format!("native map verification: {e:?}")))?;
        let transport = integral_transport::compile(
            &family,
            family.clone(),
            Arc::new(verified),
            source,
            target.clone(),
            Default::default(),
        )
        .map_err(|e| AppError::input(format!("native integral transport: {e:?}")))?;
        routes.push(CandidateOwnerRoute {
            owner_sector: target,
            transport: Arc::new(transport),
        });
    }
    observer(json!({"event":"routes_verified", "routes":routes.len()}));
    RoutedCandidateReducer::try_new(Arc::new(programs), routes, request.trace_limits)
        .map(Some)
        .map_err(|e| AppError::input(format!("routed programs: {e:?}")))
}
