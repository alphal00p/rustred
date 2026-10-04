//! Epoch record authority. Only `project` constructs the diagnostic JSON view;
//! the native publisher and binary reader use these concrete fields directly.
//! No atom or symbolic expression occurs here, so this codec has no CAS state.
use super::super::{anchors, job, merge};
use super::{CONTAINMENT_AUTHORITY, ResolverCounters};
use crate::application::routed_campaign::walking::{
    power_bounds_json,
    queue::{CompactDomain, Domain, Phase},
};
use bincode::{Decode, Encode};
use rustred::solver::DomainPowerBounds;
use serde_json::{Value, json};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Encode, Decode)]
pub(in crate::application::routed_campaign::walking) struct Powers {
    pub max_positive: Option<u64>,
    pub min_difference: Option<i64>,
    pub max_difference: Option<i64>,
}
impl From<DomainPowerBounds> for Powers {
    fn from(p: DomainPowerBounds) -> Self {
        Self {
            max_positive: p.max_positive_power,
            min_difference: p.min_power_difference,
            max_difference: p.max_power_difference,
        }
    }
}
impl Powers {
    pub fn native(self) -> DomainPowerBounds {
        DomainPowerBounds {
            max_positive_power: self.max_positive,
            min_power_difference: self.min_difference,
            max_power_difference: self.max_difference,
        }
    }
}

/// Same compact coordinate range as the canonical Store; rank remains u32.
/// The arity is explicit and checked before converting to an N-specific image.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode)]
pub(in crate::application::routed_campaign::walking) struct Image {
    pub route: bool,
    pub owner: Vec<bool>,
    pub lower: Vec<u16>,
    pub upper: Vec<u16>,
    pub rank: Option<u32>,
    pub powers: Powers,
}
impl Image {
    pub fn of<const N: usize>(image: &CompactDomain<N>) -> Self {
        let domain = image.expand();
        Self {
            route: domain.phase == Phase::Route,
            owner: domain.owner.to_vec(),
            lower: domain
                .lower
                .iter()
                .map(|&n| u16::try_from(n).expect("canonical coordinate"))
                .collect(),
            upper: domain
                .upper
                .iter()
                .map(|n| {
                    n.map_or(u16::MAX, |n| {
                        u16::try_from(n).expect("canonical coordinate")
                    })
                })
                .collect(),
            rank: domain.rank,
            powers: domain.powers.into(),
        }
    }
    pub fn validate(&self) -> Result<(), &'static str> {
        let n = self.owner.len();
        if n == 0
            || n > 32
            || self.lower.len() != n
            || self.upper.len() != n
            || self.lower.contains(&u16::MAX)
        {
            return Err("epoch record geometry shape");
        }
        Ok(())
    }
    pub fn domain<const N: usize>(&self) -> Result<Domain<N>, &'static str> {
        self.validate()?;
        Ok(Domain {
            phase: if self.route {
                Phase::Route
            } else {
                Phase::Apply
            },
            owner: self
                .owner
                .as_slice()
                .try_into()
                .map_err(|_| "epoch record arity")?,
            lower: self.lower.iter().map(|&n| u64::from(n)).collect(),
            upper: self
                .upper
                .iter()
                .map(|&n| (n != u16::MAX).then_some(u64::from(n)))
                .collect(),
            rank: self.rank,
            powers: self.powers.native(),
        })
    }
    fn project(&self) -> Value {
        json!({"phase":if self.route {"Route"} else {"Apply"},
            "owner":self.owner.iter().map(|&b| if b {'1'} else {'0'}).collect::<String>(),
            "lower":self.lower,"upper":self.upper.iter().map(|&n| (n!=u16::MAX).then_some(u64::from(n))).collect::<Vec<_>>(),
            "rank":self.rank,"power_bounds":power_bounds_json(self.powers.native())})
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode)]
pub(in crate::application::routed_campaign::walking) struct Anchor {
    pub id: u32,
    pub stamp: Option<u64>,
    pub low_slice: bool,
}
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode)]
pub(in crate::application::routed_campaign::walking) struct Piece {
    pub d_lo: Option<i64>,
    pub d_hi: Option<i64>,
    pub lower: Vec<u16>,
    pub upper: Vec<u16>,
}
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode)]
pub(in crate::application::routed_campaign::walking) enum Scope {
    Whole,
    Initial {
        anchor: u32,
        cut: i64,
        residual: Powers,
    },
    G2 {
        kind: u8,
        dispatch: u64,
        anchors: Vec<Anchor>,
        pieces: Vec<Piece>,
    },
    FiniteReplay(super::super::super::finite_replay::Recipe),
}

#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode)]
pub(in crate::application::routed_campaign::walking) struct Native {
    pub class: u8,
    pub kind: u8,
    pub v0: u64,
    pub distinct_edges: u32,
    pub self_edge: bool,
    pub break_reason: u8,
    pub error_kind: u8,
    pub err_class: Option<u8>,
    pub panic: bool,
    pub emitted: u64,
    pub accepted: u64,
    pub stats_events: u64,
    pub has_error: bool,
    pub frontiers: u32,
    pub job_duplicates: u64,
    pub known_reuse: u64,
    pub resolver: ResolverCounters,
    pub scope: Scope,
}
impl Native {
    pub fn class(&self) -> Result<merge::Class, &'static str> {
        match self.class {
            0 => Ok(merge::Class::C0),
            2 => Ok(merge::Class::C2),
            4 => Ok(merge::Class::C4),
            _ => Err("epoch record class"),
        }
    }
    pub fn kind(&self) -> Result<job::NativeKind, &'static str> {
        match self.kind {
            0 => Ok(job::NativeKind::Apply),
            1 => Ok(job::NativeKind::ApplyPartial),
            2 => Ok(job::NativeKind::Route),
            3 => Ok(job::NativeKind::G2Residual),
            4 => Ok(job::NativeKind::Abandoned),
            5 => Ok(job::NativeKind::FiniteReplay),
            _ => Err("epoch record native kind"),
        }
    }
    pub fn break_reason(&self) -> Result<job::BreakReason, &'static str> {
        use job::BreakReason as B;
        match self.break_reason {
            0 => Ok(B::None),
            1 => Ok(B::Allowance),
            2 => Ok(B::ResolverRange),
            3 => Ok(B::ResolverSummary),
            4 => Ok(B::ResolverDiagnostic),
            5 => Ok(B::SpillIo),
            6 => Ok(B::Alloc),
            7 => Ok(B::Cancel),
            8 => Ok(B::Protocol),
            _ => Err("epoch record break reason"),
        }
    }
    pub fn error_kind(&self) -> Result<job::ErrorKind, &'static str> {
        use job::ErrorKind as E;
        match self.error_kind {
            0 => Ok(E::None),
            1 => Ok(E::Cancelled),
            2 => Ok(E::ConsumerStop),
            3 => Ok(E::NativeFailure),
            4 => Ok(E::Conversion),
            5 => Ok(E::Other),
            _ => Err("epoch record error kind"),
        }
    }
    pub fn abandoned(&self) -> bool {
        self.kind == job::NativeKind::Abandoned as u8
    }
    pub fn finished(&self) -> bool {
        matches!(self.class, 0 | 4) && !self.abandoned()
    }
}
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode)]
pub(in crate::application::routed_campaign::walking) enum Body {
    Alias { to: u32, exhausted: bool },
    Native(Native),
}
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode)]
pub(in crate::application::routed_campaign::walking) struct Authority {
    pub id: u32,
    pub image: Image,
    pub merge_epoch: u64,
    pub body: Body,
}
impl Authority {
    /// Structural checks only. Consumers independently validate against the
    /// authenticated domain/ledger/run/anchor inventories and replay counters.
    pub fn validate(&self) -> Result<(), &'static str> {
        self.image.validate()?;
        if self.merge_epoch == 0 {
            return Err("epoch record merge epoch");
        }
        if let Body::Native(n) = &self.body {
            n.class()?;
            n.kind()?;
            n.break_reason()?;
            n.error_kind()?;
            if !matches!(n.kind, 4 | 5)
                && (n.kind == job::NativeKind::Route as u8) != self.image.route
            {
                return Err("epoch record phase/kind mismatch");
            }
            if n.resolver.version != 1
                || n.err_class.is_some() != (n.class == 2)
                || n.has_error != (n.class == 2)
            {
                return Err("epoch record native metadata");
            }
            match &n.scope {
                Scope::Whole => {
                    if matches!(n.kind, 1 | 3 | 5) {
                        return Err("epoch record partial kind without scope");
                    }
                }
                Scope::Initial { .. } => {
                    if n.kind != job::NativeKind::ApplyPartial as u8 {
                        return Err("epoch record initial kind");
                    }
                }
                Scope::G2 {
                    kind,
                    anchors,
                    pieces,
                    ..
                } => {
                    if !matches!(kind, 1 | 2)
                        || n.kind != job::NativeKind::G2Residual as u8
                        || anchors.len() > anchors::MAX_ANCHORS
                        || pieces.len() > anchors::MAX_RESIDUAL_PIECES
                        || pieces.iter().any(|p| {
                            p.lower.len() != self.image.owner.len()
                                || p.upper.len() != self.image.owner.len()
                        })
                    {
                        return Err("epoch record G2 shape");
                    }
                }
                Scope::FiniteReplay(recipe) => {
                    recipe.validate()?;
                    if self.id != 0
                        || n.kind != job::NativeKind::FiniteReplay as u8
                        || n.class != 0
                        || n.has_error
                        || n.frontiers != 0
                        || n.panic
                        || n.error_kind != 0
                        || n.break_reason != 0
                        || n.distinct_edges != 0
                        || n.self_edge
                        || n.emitted != 1
                        || n.accepted != 1
                        || n.stats_events != 1
                        || n.known_reuse != 0
                        || n.job_duplicates != 0
                        || n.resolver.successors != 0
                        || n.resolver.conditional != 0
                        || n.resolver.optional != [0; 3]
                        || n.resolver.route_masks != 0
                        || n.resolver.route_joint_pruned != 0
                        || (self.image.lower.iter().any(|&lo| lo != 0)
                            && self.image.lower != self.image.upper)
                    {
                        return Err("epoch finite replay record shape");
                    }
                }
            }
        }
        Ok(())
    }
}

/// Opaque diagnostic JSON parts are not containment authority. Their exact
/// bytes remain authenticated, and are only parsed for the optional JSON view.
#[derive(Clone, Debug, PartialEq, Encode, Decode, Default)]
pub(in crate::application::routed_campaign::walking) struct Diagnostics {
    pub seconds: f64,
    pub stats_json: Vec<u8>,
    pub error: Option<String>,
    pub frontiers: Vec<Vec<u8>>,
    pub refusals: Vec<Vec<u8>>,
    pub refusals_truncated: bool,
}
#[derive(Clone, Debug, PartialEq)]
pub(in crate::application::routed_campaign::walking) struct Record {
    pub authority: Authority,
    pub diagnostics: Diagnostics,
}
impl Record {
    pub fn alias<const N: usize>(
        id: u32,
        image: &CompactDomain<N>,
        to: u32,
        merge_epoch: u64,
        exhausted: bool,
    ) -> Self {
        Self {
            authority: Authority {
                id,
                image: Image::of(image),
                merge_epoch,
                body: Body::Alias { to, exhausted },
            },
            diagnostics: Diagnostics::default(),
        }
    }
    pub fn native<const N: usize>(
        id: u32,
        image: &CompactDomain<N>,
        entry: &merge::CheckedResult<N>,
        merge_epoch: u64,
        distinct_edges: u32,
        self_edge: bool,
    ) -> Result<Self, String> {
        let r = &entry.result;
        let abandoned = r.kind == job::NativeKind::Abandoned;
        let finished = matches!(entry.class, merge::Class::C0 | merge::Class::C4) && !abandoned;
        let error = if finished || abandoned {
            None
        } else {
            let detail = r.error.clone().unwrap_or_else(|| "native error".into());
            Some(match r.break_reason {
                job::BreakReason::None => detail,
                _ => format!("{}: {detail}", r.break_reason.name()),
            })
        };
        let scope = if let Some(recipe) = r.finite_replay {
            if entry.anchors.is_some() || r.scope.is_some() || r.g2.is_some() {
                return Err("finite replay record with partial scope".into());
            }
            Scope::FiniteReplay(recipe)
        } else if let Some(a) = entry.anchors.as_ref().filter(|a| a.record.kind.is_g2()) {
            let anchors::AnchorScope::Residual(pieces) = &a.record.scope else {
                return Err("G2 record lacks residual scope".into());
            };
            Scope::G2 {
                kind: a.record.kind as u8,
                dispatch: a.record.dispatch_version,
                anchors: a
                    .record
                    .anchors
                    .iter()
                    .map(|a| Anchor {
                        id: a.anchor,
                        stamp: a.stamp,
                        low_slice: a.lent == anchors::Lent::LowSlice,
                    })
                    .collect(),
                pieces: pieces
                    .iter()
                    .map(|p| Piece {
                        d_lo: p.d_lo,
                        d_hi: p.d_hi,
                        lower: p.lower.clone(),
                        upper: p.upper.clone(),
                    })
                    .collect(),
            }
        } else if let (Some((anchor, cut)), Some(scope)) = (entry.d_band(), r.scope) {
            Scope::Initial {
                anchor,
                cut,
                residual: scope.residual.into(),
            }
        } else {
            Scope::Whole
        };
        let record = Self {
            authority: Authority {
                id,
                image: Image::of(image),
                merge_epoch,
                body: Body::Native(Native {
                    class: entry.class as u8,
                    kind: r.kind as u8,
                    v0: r.v0,
                    distinct_edges,
                    self_edge,
                    break_reason: r.break_reason as u8,
                    error_kind: r.error_kind as u8,
                    err_class: (entry.class == merge::Class::C2).then(|| merge::error_class(entry)),
                    panic: r.panic,
                    emitted: r.emitted,
                    accepted: r.accepted,
                    stats_events: r.stats_events,
                    has_error: error.is_some(),
                    frontiers: u32::try_from(r.frontiers.len())
                        .map_err(|_| "record frontier count")?,
                    job_duplicates: r.job_duplicates,
                    known_reuse: r.known_reuse,
                    resolver: ResolverCounters::from_result(r),
                    scope,
                }),
            },
            diagnostics: Diagnostics {
                seconds: r.seconds,
                stats_json: r.stats_json.clone(),
                error,
                frontiers: r.frontiers.clone(),
                refusals: r.refusals.clone(),
                refusals_truncated: r.refusals_truncated,
            },
        };
        record.authority.validate().map_err(str::to_owned)?;
        Ok(record)
    }
    pub fn validate(&self) -> Result<(), &'static str> {
        self.authority.validate()?;
        match &self.authority.body {
            Body::Alias { .. } if self.diagnostics != Diagnostics::default() => {
                return Err("epoch alias diagnostics");
            }
            Body::Native(n)
                if self.diagnostics.frontiers.len() != n.frontiers as usize
                    || self.diagnostics.error.is_some() != n.has_error =>
            {
                return Err("epoch diagnostic inventory");
            }
            _ => {}
        }
        Ok(())
    }
    pub fn project(&self) -> Result<Value, String> {
        self.validate().map_err(str::to_owned)?;
        let a = &self.authority;
        let mut record = a.image.project();
        record["id"] = json!(a.id);
        match &a.body {
            Body::Alias { to, exhausted } => {
                record["record_kind"] = json!("delegated_not_inspected");
                record["representative_id"] = json!(to);
                record["local_inspection_finished"] = json!(false);
                record["responsibility_status"] = json!("pending");
                record["containment_authority"] = json!(CONTAINMENT_AUTHORITY);
                record["epoch"] = json!({"merge_epoch":a.merge_epoch,"transition":if *exhausted {"T10"} else {"T3"},"authority":"verify_chokepoint: explicit phase and owner, raw inclusion or native summary on canonical images"});
            }
            Body::Native(n) => {
                let d = &self.diagnostics;
                let parse = |bytes: &[u8]| {
                    serde_json::from_slice::<Value>(bytes)
                        .map_err(|e| format!("record diagnostic JSON: {e}"))
                };
                record["local_inspection_finished"] = json!(n.finished());
                record["stats"] = if d.stats_json.is_empty() {
                    Value::Null
                } else {
                    parse(&d.stats_json)?
                };
                record["seconds"] = json!(d.seconds);
                record["error"] = json!(d.error);
                record["accepted_events"] = json!(n.accepted);
                record["frontiers"] = Value::Array(
                    d.frontiers
                        .iter()
                        .map(|v| parse(v))
                        .collect::<Result<_, _>>()?,
                );
                record["record_kind"] = json!("native_inspection");
                if n.abandoned() {
                    record["rescue_abandoned"] = json!(true);
                }
                record["local_classification_discharged"] =
                    json!(n.class == 0 && n.scope == Scope::Whole);
                if a.image.route {
                    record["conservative_route_overcover"] = json!(true);
                } else {
                    record["optional_refusal_provenance_truncated"] = json!(d.refusals_truncated);
                    record["optional_refusal_provenance_scope"] =
                        json!("first_per_phase_per_query");
                    record["optional_refusals"] = Value::Array(
                        d.refusals
                            .iter()
                            .map(|v| parse(v))
                            .collect::<Result<_, _>>()?,
                    );
                }
                match &n.scope {
                    Scope::Whole => {}
                    Scope::FiniteReplay(recipe) => {
                        record["record_kind"] = json!("finite_replay_summary");
                        record["finite_replay_recipe"] = json!(recipe);
                        record["native_inspection_scope"] =
                            json!("whole_initial_domain_exact_replay");
                        record["local_classification_discharged"] = json!(true);
                        record
                            .as_object_mut()
                            .expect("record object")
                            .remove("conservative_route_overcover");
                    }
                    Scope::Initial {
                        anchor,
                        cut,
                        residual,
                    } => {
                        record["record_kind"] = json!("partial_initial_overlap_inspection");
                        record["native_inspection_scope"] = json!("low_D_residual_only");
                        record["initial_overlap"] = json!({"anchor_id":anchor,"cut":cut,"covered_slice":"original_intersect_D_ge_cut","residual_power_bounds":power_bounds_json(residual.native()),"coordinates_and_rank_unchanged":true,"authority":CONTAINMENT_AUTHORITY});
                    }
                    Scope::G2 {
                        kind,
                        dispatch,
                        anchors,
                        pieces,
                    } => {
                        record["record_kind"] = json!("g2_residual_inspection");
                        record["native_inspection_scope"] = json!("g2_residual_only");
                        record["g2"] = json!({"kind":anchors::AnchorKind::from_code(*kind).expect("validated kind").name(),"dispatch_version":dispatch,
                            "anchors":anchors.iter().map(|a|json!({"id":a.id,"stamp":a.stamp,"lent":if a.low_slice{"inspected_low_D_slice"}else{"domain"}})).collect::<Vec<_>>(),
                            "residual":pieces.iter().map(|p|json!({"d_lo":p.d_lo,"d_hi":p.d_hi,"lower":p.lower,"upper":p.upper})).collect::<Vec<_>>(),"authority":"exact_union_cover_lattice"});
                    }
                }
                if matches!(n.scope, Scope::Initial { .. } | Scope::G2 { .. }) {
                    record["local_inspection_finished"] = json!(false);
                    record["residual_inspection_finished"] = json!(n.finished());
                    record["local_classification_discharged"] = json!(false);
                }
                record["epoch"] = json!({"merge_epoch":a.merge_epoch,"v0":n.v0,"refresh_points":[],"distinct_edge_count":n.distinct_edges,"self_edge":n.self_edge,"class":n.class().map_err(str::to_owned)?.name(),"break_reason":n.break_reason().map_err(str::to_owned)?.name(),"panic":n.panic,"emitted_events":n.emitted,"error_kind":n.error_kind().map_err(str::to_owned)?.name(),"job_local_duplicates":n.job_duplicates,"known_reuse":n.known_reuse,"resolver_counters":n.resolver});
                if let Some(code) = n.err_class {
                    record["epoch"]["err_class"] =
                        json!(super::super::ledger6::err_class::name(code));
                }
            }
        }
        Ok(record)
    }
}
