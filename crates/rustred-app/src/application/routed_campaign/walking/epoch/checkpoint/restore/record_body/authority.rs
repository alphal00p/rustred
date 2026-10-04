//! Restore authority checks on the typed record, without diagnostic JSON.
use super::*;
use crate::application::routed_campaign::walking::epoch::records::typed::{
    Authority, Body, Image, Scope,
};

pub(super) fn validate<const N: usize>(
    record: Authority,
    source: u32,
    targets: &[u32],
    view: &View<'_, N>,
    totals: &mut Totals,
) -> io::Result<()> {
    record.validate().map_err(invalid)?;
    let image = view
        .store
        .domains
        .get(source as usize)
        .ok_or_else(|| invalid("epoch record ID range"))?;
    if record.id != source || record.image != Image::of(image) {
        return Err(invalid("epoch record canonical geometry differs"));
    }
    if record.merge_epoch > view.k || record.merge_epoch < totals.merge_epoch {
        return Err(invalid("epoch record merge version"));
    }
    totals.merge_epoch = record.merge_epoch;
    let entry = view
        .ledger
        .get(source)
        .map_err(|_| invalid("epoch record ledger source"))?;
    if let Entry6::Alias { to } = entry {
        return match record.body {
            Body::Alias { to: actual, .. } if actual == to => Ok(()),
            _ => Err(invalid("epoch alias record differs from ledger")),
        };
    }
    let Body::Native(native) = record.body else {
        return Err(invalid("epoch native record missing"));
    };
    let (class, stored_epoch) = match entry {
        Entry6::Native { epoch, .. } => (Class::C0, epoch),
        Entry6::NativeFrontier { epoch } | Entry6::Abandoned { epoch } => (Class::C4, epoch),
        Entry6::NativeError { epoch, .. } => (Class::C2, epoch),
        _ => return Err(invalid("epoch record belongs to unmerged ID")),
    };
    let anchored = view.anchors.get(source);
    let abandoned = matches!(entry, Entry6::Abandoned { .. });
    if native.abandoned() != abandoned
        || abandoned && (native.frontiers != 1 || !targets.is_empty() || anchored.is_some())
    {
        return Err(invalid("epoch abandoned record differs from ledger"));
    }
    if record.merge_epoch != stored_epoch
        || native.v0 >= record.merge_epoch
        || native.distinct_edges as usize != targets.len()
        || native.self_edge != targets.binary_search(&source).is_ok()
        || native.class().map_err(invalid)? != class
        || native.has_error != (class == Class::C2)
        || class == Class::C0 && native.frontiers != 0
        || class == Class::C4
            && !abandoned
            && view.frontier_counts.get(&source).copied() != Some(native.frontiers)
    {
        return Err(invalid("epoch native record class, scope or run differs"));
    }
    let expected_kind = if matches!(native.scope, Scope::FiniteReplay(_)) {
        NativeKind::FiniteReplay
    } else if abandoned {
        NativeKind::Abandoned
    } else if image.phase() == Phase::Route {
        NativeKind::Route
    } else if anchored.is_some_and(|a| a.kind.is_g2()) {
        NativeKind::G2Residual
    } else if anchored.is_some() {
        NativeKind::ApplyPartial
    } else {
        NativeKind::Apply
    };
    if native.kind().map_err(invalid)? != expected_kind {
        return Err(invalid(
            "epoch native record kind differs from domain/scope",
        ));
    }
    match (anchored, &native.scope) {
        (None, Scope::Whole) => {}
        (None, Scope::FiniteReplay(_)) if source == 0 && targets.is_empty() => {}
        (
            Some(anchor),
            Scope::Initial {
                anchor: id,
                cut,
                residual,
            },
        ) => {
            let slice = low_slice(cell_of(image), *cut)
                .ok_or_else(|| invalid("epoch record residual cut"))?;
            if anchor.dispatch_version != native.v0
                || anchor.d_band() != Some((*id, *cut))
                || residual.native() != slice.powers
            {
                return Err(invalid("epoch partial record scope differs from anchor"));
            }
        }
        (
            Some(anchor),
            Scope::G2 {
                kind,
                dispatch,
                anchors,
                pieces,
            },
        ) => {
            let AnchorScope::Residual(expected) = &anchor.scope else {
                return Err(invalid("epoch residual scope"));
            };
            if *kind != anchor.kind as u8
                || *dispatch != native.v0
                || anchor.dispatch_version != native.v0
                || anchors.len() != anchor.anchors.len()
                || anchors.iter().zip(&anchor.anchors).any(|(a, b)| {
                    a.id != b.anchor
                        || a.stamp != b.stamp
                        || a.low_slice != (b.lent == Lent::LowSlice)
                })
                || pieces.len() != expected.len()
                || pieces.iter().zip(expected).any(|(a, b)| {
                    a.d_lo != b.d_lo || a.d_hi != b.d_hi || a.lower != b.lower || a.upper != b.upper
                })
            {
                return Err(invalid("epoch G2 record scope differs from anchor"));
            }
        }
        _ => return Err(invalid("epoch record anchor/scope mismatch")),
    }
    let delta = native.resolver;
    match image.phase() {
        Phase::Apply if delta.route_masks != 0 || delta.route_joint_pruned != 0 => {
            return Err(invalid("epoch Apply record carries Route counters"));
        }
        Phase::Route if delta.optional != [0; 3] => {
            return Err(invalid("epoch Route record carries Apply counters"));
        }
        _ => {}
    }
    if native.panic && (!targets.is_empty() || native.stats_events != u64::MAX) {
        return Err(invalid("epoch recurring panic has stats or edges"));
    }
    // P1's class/parity check remains the single definition. No diagnostic
    // frontier payload is allocated here: P1 consults emptiness only.
    let observation = JobResult::<N> {
        seq: 0,
        parent: source,
        v0: native.v0,
        kind: expected_kind,
        error_kind: native.error_kind().map_err(invalid)?,
        break_reason: native.break_reason().map_err(invalid)?,
        panic: native.panic,
        emitted: native.emitted,
        accepted: native.accepted,
        stats_events: native.stats_events,
        successors: delta.successors,
        conditional: delta.conditional,
        known_reuse: native.known_reuse,
        job_duplicates: native.job_duplicates,
        optional: delta.optional,
        route_masks: delta.route_masks,
        route_joint_pruned: delta.route_joint_pruned,
        seconds: 0.0,
        stats_json: Vec::new(),
        error: None,
        frontiers: if native.frontiers == 0 {
            Vec::new()
        } else {
            vec![Vec::new()]
        },
        refusals: Vec::new(),
        refusals_truncated: false,
        scope: None,
        g2: None,
        finite_replay: match native.scope {
            Scope::FiniteReplay(recipe) => Some(recipe),
            _ => None,
        },
        lookup: None,
        misses: Vec::new(),
    };
    if abandoned
        && (observation.emitted != 1
            || observation.accepted != 1
            || observation.stats_events != 1
            || observation.successors != 0
            || observation.conditional != 0
            || observation.known_reuse != 0
            || observation.job_duplicates != 0
            || observation.optional != [0; 3]
            || observation.route_masks != 0
            || observation.route_joint_pruned != 0
            || observation.panic)
    {
        return Err(invalid("epoch abandonment carries native work counters"));
    }
    let (derived, _) =
        merge::classify(&observation, 4).map_err(|_| invalid("epoch record P1 parity/protocol"))?;
    if derived != class {
        return Err(invalid("epoch record P1 class differs from ledger"));
    }
    if let Entry6::NativeError { err, .. } = entry {
        let checked = CheckedResult {
            recurring_panic: native.panic,
            result: observation.clone(),
            class,
            cause: None,
            anchors: None,
        };
        if merge::error_class(&checked) != err || native.err_class != Some(err) {
            return Err(invalid("epoch record terminal error class differs"));
        }
    }
    if expected_kind == NativeKind::Route && !abandoned {
        add(&mut totals.routed, 1)?;
    }
    totals.resolver.add(&delta).map_err(io::Error::other)?;
    add(&mut totals.events, observation.emitted)?;
    if !abandoned {
        add(&mut totals.frontiers, u64::from(native.frontiers))?;
    }
    add(&mut totals.known_reuse, observation.known_reuse)?;
    add(&mut totals.job_duplicates, observation.job_duplicates)?;
    Ok(())
}
