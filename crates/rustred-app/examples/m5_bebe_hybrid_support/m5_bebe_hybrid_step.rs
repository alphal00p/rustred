//! Experimental bebe owner: saved rank-gated rules at positive numerator
//! rank, original shipped rules at rank zero. No core or package-format change.
//! The gated ordering is used uniformly for the hybrid. The ordered engine
//! still verifies every same-owner child; a rank-zero shipped rule that violates
//! that ordering is an error, not a silent fallback or a fabricated terminal.

use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

use rustred::propagator::OwnerClassId;
use rustred::propagator::custom::{build_custom_step, inspect_custom_payload};
use rustred::propagator::error::PError;
use rustred::propagator::lane::OwnerLane;
use rustred::propagator::library::{OwnerProgram, PLibrary};
use rustred::propagator::step::{PKeyView, PLaneStep, POrderKey, PStepContext, PStepOut};

const BEBE_CLASS: u64 = 0xd261f9c2d25e2dac;
const DEFAULT_GATED_PAYLOAD: &str = "/shared/vhirschi/t3code/vakint_dev/p_integral_dev/SCRATCHPAD/m5-r0/exec/p2/gen/bebe-globisp/L4T08-d261f9c2d25e2dac.rrbin";

#[derive(Default)]
pub struct Stats {
    mode: String,
    gated_payload: PathBuf,
    original_terminals: usize,
    original_residuals: u32,
    gated_residuals_not_added: u32,
    support: u64,
    rank_zero_calls: AtomicU64,
    positive_rank_calls: AtomicU64,
}

impl Stats {
    pub fn json(&self) -> serde_json::Value {
        serde_json::json!({
            "mode": self.mode,
            "owner_class": format!("{BEBE_CLASS:016x}"),
            "gated_payload": self.gated_payload,
            "support": format!("{:#x}", self.support),
            "rank_definition": "sum max(-n_i,0) on this owner's non-support slots",
            "original_library_terminals": self.original_terminals,
            "original_bebe_residuals": self.original_residuals,
            "gated_residuals_not_added_to_library": self.gated_residuals_not_added,
            "extra_terminals_added": 0,
            "extra_basis_rows_added": 0,
            "step_counters_enabled": self.mode == "hybrid",
            "rank_zero_step_calls": self.rank_zero_calls.load(Ordering::Relaxed),
            "positive_rank_step_calls": self.positive_rank_calls.load(Ordering::Relaxed),
            "order": if self.mode == "hybrid" { "gated order for every bebe key; same-owner descent remains enforced" } else { "original shipped order" },
            "step_cache": "none (the experimental wrapper is not a serialized library identity)",
            "call_count_scope": "all native step invocations, including modular replay; not a unique-key count",
        })
    }
}

struct BebeHybrid {
    shipped: Arc<dyn PLaneStep>,
    gated: Arc<dyn PLaneStep>,
    stats: Arc<Stats>,
}

fn positive_rank(powers: &[i64], support: u64) -> bool {
    // Only the zero/positive partition is needed; no sum or signed negation
    // can overflow, including for a deliberately malformed extreme index.
    powers
        .iter()
        .enumerate()
        .any(|(slot, &power)| support >> slot & 1 == 0 && power < 0)
}

impl PLaneStep for BebeHybrid {
    fn lane(&self) -> OwnerLane {
        OwnerLane::Custom
    }

    fn step(&self, cx: &PStepContext<'_>, key: &PKeyView<'_>) -> Result<PStepOut, PError> {
        if positive_rank(key.powers, self.stats.support) {
            self.stats.positive_rank_calls.fetch_add(1, Ordering::Relaxed);
            let out = self.gated.step(cx, key)?;
            if !out.terminals.is_empty() {
                return Err(PError::Contract(
                    "bebe hybrid: gated positive-rank step reached a terminal; original terminal inventory must remain unchanged".to_owned(),
                ));
            }
            Ok(out)
        } else {
            self.stats.rank_zero_calls.fetch_add(1, Ordering::Relaxed);
            self.shipped.step(cx, key)
        }
    }

    fn order_key(&self, cx: &PStepContext<'_>, powers: &[i64]) -> Result<POrderKey, PError> {
        self.gated.order_key(cx, powers)
    }
}

/// Rebuild through public PLibrary APIs, retaining the original census,
/// presentations, maps, owner specs, terminal numbers and basis map. No terminal
/// from the gated payload is registered. Non-bebe owner programs remain lazy.
pub fn install(original: Arc<PLibrary>) -> Result<(Arc<PLibrary>, Arc<Stats>), PError> {
    let mode = std::env::var("M5_BEBE_HYBRID_MODE").unwrap_or_else(|_| "hybrid".to_owned());
    if mode != "hybrid" && mode != "shipped" {
        return Err(PError::Contract(format!(
            "M5_BEBE_HYBRID_MODE must be hybrid or shipped, got {mode:?}"
        )));
    }
    let gated_payload = std::env::var_os("M5_BEBE_GATED_PAYLOAD")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(DEFAULT_GATED_PAYLOAD));
    let class = OwnerClassId(BEBE_CLASS);
    let owner = original.owner(class).ok_or_else(|| {
        PError::Contract("the supplied library has no bebe class d261f9c2d25e2dac".to_owned())
    })?;
    if owner.spec.lane != OwnerLane::Custom {
        return Err(PError::Contract("bebe is not a Custom owner".to_owned()));
    }
    let original_meta = owner.spec.custom.as_ref().ok_or_else(|| {
        PError::Contract("bebe has no custom metadata".to_owned())
    })?;
    let original_bytes = owner.spec.payload.as_ref().ok_or_else(|| {
        PError::Contract("bebe has no payload".to_owned())
    })?.bytes()?;
    let original_summary = inspect_custom_payload(&original_bytes)?;
    let bytes = std::fs::read(&gated_payload).map_err(|error| {
        PError::Contract(format!("read gated bebe payload {}: {error}", gated_payload.display()))
    })?;
    let gated_summary = inspect_custom_payload(&bytes)?;
    if gated_summary.arity != original_summary.arity
        || gated_summary.family_fingerprint != original_summary.family_fingerprint
        || gated_summary.support != original_summary.support
        || gated_summary.two_sided_axes != original_summary.two_sided_axes
    {
        return Err(PError::Contract(
            "gated bebe family/arity/support/two-sided axes differ from the original".to_owned(),
        ));
    }
    let gated_meta = gated_summary.meta(original_meta.lp_count)?;
    if gated_meta.charts != original_meta.charts {
        return Err(PError::Contract("gated bebe raw charts differ".to_owned()));
    }
    // Its extra residual(s) must be rank-zero, where this wrapper never uses
    // the gated program. Refuse a positive-rank residual before running.
    for residual in gated_summary.residual_keys() {
        if positive_rank(&residual, gated_summary.support) {
            return Err(PError::Contract(format!(
                "gated bebe has a positive-rank residual {residual:?}"
            )));
        }
    }
    let stats = Arc::new(Stats {
        mode: mode.clone(),
        gated_payload,
        original_terminals: original.terminals().len(),
        original_residuals: original_meta.residuals,
        gated_residuals_not_added: gated_meta.residuals,
        support: original_summary.support,
        ..Stats::default()
    });
    let mut builder = PLibrary::builder(original.coefficient_context().clone(), original.scope());
    for presentation in original.presentations() {
        builder.add_presentation(presentation.as_ref().clone())?;
    }
    if let Some(topology) = original.topology() {
        builder.set_topology(Arc::clone(topology))?;
    }
    builder.set_census(original.census().clone())?;
    for (index, map) in original.edge_maps().iter().enumerate() {
        if builder.add_edge_map(map.clone())? != index {
            return Err(PError::Contract("bebe hybrid edge maps were renumbered".to_owned()));
        }
    }
    for (index, terminal) in original.terminals().iter().enumerate().skip(1) {
        if builder.add_terminal(terminal.clone())?.0 as usize != index {
            return Err(PError::Contract("bebe hybrid terminals were renumbered".to_owned()));
        }
    }
    for existing in original.owners() {
        if existing.spec.class == class && mode == "hybrid" {
            let shipped = match existing.program(&original)? {
                OwnerProgram::Lane(step) => Arc::clone(step),
                OwnerProgram::Missing { .. } => {
                    return Err(PError::Contract("bebe original program is missing".to_owned()));
                }
            };
            let gated = build_custom_step(
                original.presentation(existing.spec.presentation), &bytes, &gated_meta,
            )?;
            builder.add_owner_with_program(
                existing.spec.clone(),
                OwnerProgram::Lane(Arc::new(BebeHybrid { shipped, gated, stats: Arc::clone(&stats) })),
            )?;
        } else {
            builder.add_owner(existing.spec.clone())?;
        }
    }
    if let Some(map) = original.basis_map() {
        builder.set_basis_map(Arc::clone(map))?;
    }
    let library = builder.build()?;
    if library.terminals() != original.terminals() {
        return Err(PError::Contract("bebe hybrid changed the terminal inventory".to_owned()));
    }
    Ok((Arc::new(library), stats))
}
