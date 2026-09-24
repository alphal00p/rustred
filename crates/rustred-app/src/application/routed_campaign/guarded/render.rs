use rustred::solver::{
    OwnerAppliedNonzero, OwnerAppliedStats, OwnerGuardedDomain, OwnerGuardedEvent,
    OwnerGuardedStats,
};
use serde_json::{Value, json};
use std::fmt::{self, Write};
use std::sync::atomic::{AtomicBool, Ordering};

pub(super) struct Budget<'a> {
    pub bytes: usize,
    pub events: usize,
    pub limit: usize,
    pub event_limit: usize,
    pub expression_limit: usize,
    pub cancel: &'a AtomicBool,
}
impl Budget<'_> {
    pub fn charge(&mut self, n: usize) -> Result<(), &'static str> {
        if self.cancel.load(Ordering::Acquire) {
            return Err("cancelled during diagnostic rendering");
        }
        let next = self
            .bytes
            .checked_add(n)
            .ok_or("report payload count overflow")?;
        if next > self.limit {
            return Err("report payload byte allowance");
        }
        self.bytes = next;
        Ok(())
    }
    pub fn text(&mut self, value: &impl fmt::Display) -> Result<String, &'static str> {
        self.charge(64)?;
        let limit = self.expression_limit.min((self.limit - self.bytes) / 6);
        self.charge(limit * 6)?;
        let out = limited(value, limit, self.cancel).map_err(|_| {
            if self.cancel.load(Ordering::Acquire) {
                "cancelled during diagnostic rendering"
            } else {
                "expression/report byte allowance"
            }
        })?;
        self.bytes -= (limit - out.len()) * 6;
        Ok(out)
    }
    pub fn geometry(&mut self, n: usize) -> Result<(), &'static str> {
        self.charge(
            n.checked_mul(128)
                .and_then(|n| n.checked_add(512))
                .ok_or("report geometry count overflow")?,
        )
    }
    pub fn debug(&mut self, value: &impl fmt::Debug) -> Result<String, &'static str> {
        // Native failure/detail strings can be large too. Use the same
        // prospective, cancelling writer as polynomial display.
        self.text(&DebugDisplay(value))
    }
}
struct DebugDisplay<'a, T>(&'a T);
impl<T: fmt::Debug> fmt::Display for DebugDisplay<'_, T> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{:?}", self.0)
    }
}
fn limited(
    value: &impl fmt::Display,
    limit: usize,
    cancel: &AtomicBool,
) -> Result<String, fmt::Error> {
    struct Sink<'a> {
        out: String,
        limit: usize,
        cancel: &'a AtomicBool,
    }
    impl Write for Sink<'_> {
        fn write_str(&mut self, s: &str) -> fmt::Result {
            if self.cancel.load(Ordering::Acquire)
                || s.len() > self.limit.saturating_sub(self.out.len())
            {
                return Err(fmt::Error);
            }
            self.out.try_reserve(s.len()).map_err(|_| fmt::Error)?;
            self.out.push_str(s);
            Ok(())
        }
    }
    let mut sink = Sink {
        out: String::new(),
        limit,
        cancel,
    };
    write!(&mut sink, "{value}")?;
    Ok(sink.out)
}
pub(super) fn bounded_debug(value: &impl fmt::Debug) -> String {
    limited(&DebugDisplay(value), 4096, &AtomicBool::new(false))
        .unwrap_or_else(|_| "diagnostic detail exceeded 4096 bytes".into())
}
pub(super) fn domain<const N: usize>(
    d: &OwnerGuardedDomain<'_, N>,
    b: &mut Budget<'_>,
) -> Result<Value, &'static str> {
    b.geometry(N)?;
    b.charge(2048)?;
    let mut equalities = Vec::new();
    for p in d.equalities() {
        b.charge(128)?;
        equalities.push(b.text(p.raw())?);
    }
    let mut exclusions = Vec::new();
    for group in d.excluded_conjunctions() {
        b.charge(128)?;
        let mut atoms = Vec::new();
        for p in group {
            b.charge(128)?;
            atoms.push(b.text(p.raw())?);
        }
        exclusions.push(json!({"operator":"not_all_zero","atoms":atoms}));
    }
    let mut source = Vec::new();
    for p in d.source_conditions() {
        b.charge(128)?;
        source.push(b.text(p.raw())?);
    }
    let mut denominators = Vec::new();
    for (term, p) in d.original_denominators().enumerate() {
        b.charge(128)?;
        denominators.push(json!({"original_term":term,"nonzero":b.text(p.raw())?}));
    }
    Ok(
        json!({"owner":d.owner().iter().map(|&v|if v{'1'}else{'0'}).collect::<String>(),"batch":d.batch(),"rule":d.rule_ordinal(),
        "lower":d.lower(),"upper":d.upper(),"max_numerator_rank":d.max_numerator_rank(),
        "case_kind":if d.case().affine().is_some(){"affine"}else{"coordinate"},
        "equalities_zero":equalities,"excluded_conjunctions":exclusions,"source_conditions_nonzero":source,"original_denominators":denominators,
        "nonempty_integer_feasibility_asserted":false,"expression_text_is_display_only":true}),
    )
}
pub(super) fn event<const N: usize>(
    e: OwnerGuardedEvent<'_, N>,
    id: &str,
    definition: &mut Option<Value>,
    b: &mut Budget<'_>,
) -> Result<Value, &'static str> {
    if b.events == b.event_limit {
        return Err("retained report event allowance");
    }
    b.charge(2048)?;
    let d = match &e {
        OwnerGuardedEvent::Admitted(d) => Some(*d),
        OwnerGuardedEvent::Residual { domain, .. } => *domain,
        OwnerGuardedEvent::Successor(s) => Some(s.image.source),
        OwnerGuardedEvent::Problem { domain, .. }
        | OwnerGuardedEvent::OptionalCoefficientRefusal { domain, .. }
        | OwnerGuardedEvent::RuleFinished { domain, .. } => Some(*domain),
    };
    if definition.is_none() {
        if let Some(d) = d {
            *definition = Some(domain(d, b)?);
        }
    }
    let out = match e {
        OwnerGuardedEvent::Admitted(_) => {
            json!({"kind":"guarded_domain_admitted","domain_ref":id,"conditional_own_rule_domain":true})
        }
        OwnerGuardedEvent::Residual {
            kind,
            requested_lower,
            requested_upper,
            requested_rank,
            domain,
        } => {
            b.geometry(N)?;
            json!({"kind":"retained_residual","residual":b.debug(&kind)?,"requested_lower":requested_lower,
                "requested_upper":requested_upper,"requested_rank":requested_rank,"domain_ref":domain.map(|_|id),"no_missing_rule_claim":true})
        }
        OwnerGuardedEvent::Successor(s) => {
            b.geometry(N)?;
            b.geometry(N)?;
            let inverse: Vec<_> = s
                .image
                .argument_shift
                .iter()
                .map(ToString::to_string)
                .collect();
            json!({"kind":"guarded_successor","domain_ref":id,"source_lower":s.image.source_lower,"source_upper":s.image.source_upper,
                "target_owner":s.target_sector.iter().map(|&v|if v{'1'}else{'0'}).collect::<String>(),"target_lower":s.target_lower,"target_upper":s.target_upper,
                "target_rank_limit":s.target_rank_limit,"image_substitution":"source_n = child_m + argument_shift",
                "argument_shift_decimal":inverse,"inherited_source_rank":s.image.source.max_numerator_rank(),
                "source_inactive_axes":s.image.source.owner().iter().enumerate().filter_map(|(i,&on)|(!on).then_some(i)).collect::<Vec<_>>(),
                "coefficient_in_restricted_source_coordinates":b.text(s.coefficient.raw())?,
                "coefficient_nonzero":nonzero(s.coefficient_nonzero),"has_installed_target_owner":s.has_installed_target_owner,
                "provisional_until_successful_return_and_rule_finished_without_problems":true,"reached_missing_rule_claim":false})
        }
        OwnerGuardedEvent::Problem { problem: p, .. } => {
            b.geometry(N)?;
            json!({"kind":"guarded_rhs_problem","domain_ref":id,"problem":b.debug(&p.kind)?,
            "source_lower":p.source_lower,"source_upper":p.source_upper,"shift":p.shift.as_slice(),"original_term":p.original_term_ordinal,
            "coefficient_nonzero":nonzero(p.coefficient_nonzero),"coefficient":p.coefficient.map(|v|b.text(v.raw())).transpose()?,"reached_missing_rule_claim":false})
        }
        OwnerGuardedEvent::OptionalCoefficientRefusal {
            source_lower,
            source_upper,
            shift,
            original_term_ordinal,
            failure,
            ..
        } => {
            b.geometry(N)?;
            json!({"kind":"optional_coefficient_refusal","domain_ref":id,"source_lower":source_lower,"source_upper":source_upper,
                "shift":shift.as_slice(),"original_term":original_term_ordinal,"failure":b.debug(failure)?})
        }
        OwnerGuardedEvent::RuleFinished {
            successors,
            problems,
            ..
        } => {
            json!({"kind":"guarded_rule_finished","domain_ref":id,"successors":successors,"problems":problems})
        }
    };
    b.events += 1;
    Ok(out)
}
fn nonzero(n: OwnerAppliedNonzero) -> &'static str {
    match n {
        OwnerAppliedNonzero::Uniform => "uniform",
        OwnerAppliedNonzero::Conditional => "conditional",
    }
}
pub(super) fn stats(s: OwnerGuardedStats) -> Value {
    json!({"predicates":s.predicates,"predicate_terms":s.predicate_terms,"events":s.events,
    "residuals":s.residuals,"successors":s.successors,"applied":applied_stats(s.applied)})
}
fn applied_stats(s: OwnerAppliedStats) -> Value {
    json!({"selected_pieces":s.selected_pieces,"term_visits":s.term_visits,"shift_groups":s.shift_groups,
    "boundary_cells":s.boundary_cells,"sign_splits":s.sign_splits,"native_operations":s.native_operations,"events":s.events,"successors":s.successors,
    "conditional_successors":s.conditional_successors,"problems":s.problems,"zero_terms":s.zero_terms,"cancelled_groups":s.cancelled_groups,
    "same_support_successors":s.same_support_successors,"strict_subsupport_successors":s.strict_subsupport_successors,
    "unsupported_support_successors":s.unsupported_support_successors,
    "conditional_unsupported_support_successors":s.conditional_unsupported_support_successors,
    "zero_sector_groups":s.zero_sector_groups,"coalescing_additions":s.coalescing_additions,
    "optional_coefficient_refusals":s.optional_coefficient_refusals,"optional_original_refusals":s.optional_original_refusals,
    "optional_coalesced_refusals":s.optional_coalesced_refusals})
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn applied_support_attempt_partition_is_rendered_without_authority_upgrade() {
        let rendered = applied_stats(OwnerAppliedStats {
            successors: 9,
            conditional_successors: 3,
            same_support_successors: 2,
            strict_subsupport_successors: 3,
            unsupported_support_successors: 4,
            conditional_unsupported_support_successors: 1,
            ..Default::default()
        });
        for (name, count) in [
            ("successors", 9),
            ("conditional_successors", 3),
            ("same_support_successors", 2),
            ("strict_subsupport_successors", 3),
            ("unsupported_support_successors", 4),
            ("conditional_unsupported_support_successors", 1),
        ] {
            assert_eq!(rendered[name], count);
        }
        assert!(rendered.get("complete").is_none());
    }

    #[test]
    fn guarded_rendering_bounds_before_extending_and_observes_cancel() {
        let cancel = AtomicBool::new(false);
        let mut b = Budget {
            bytes: 0,
            events: 0,
            limit: 100,
            event_limit: 2,
            expression_limit: 100,
            cancel: &cancel,
        };
        assert!(b.text(&"a".repeat(1000)).is_err());
        assert!(b.bytes <= 100);
        cancel.store(true, Ordering::Release);
        assert!(b.charge(1).is_err());
    }
}
