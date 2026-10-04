use super::input::{Powers, Query, RecorderLimits};
use rustred::{
    persistence::{BinaryIoLimits, CoefficientTableBuilder},
    solver::{
        OwnerAppliedError, OwnerAppliedEvent, OwnerAppliedFailure, OwnerAppliedNonzero,
        OwnerAppliedProblemKind, OwnerAppliedStats, OwnerDomainMatchDisposition,
        OwnerDomainMatchPiece, OwnerDomainPredicate,
    },
};
use serde::Serialize;
use serde_json::{Value, json};
use std::{
    fmt,
    fs::{File, OpenOptions},
    io::{self, Write},
    ops::ControlFlow,
    path::{Path, PathBuf},
    time::Instant,
};

// Fixed-prefix formatting never allocates an unbounded diagnostic String.
pub fn bounded_debug(value: &impl fmt::Debug, cap: usize) -> Value {
    struct Text {
        text: String,
        cap: usize,
    }
    impl fmt::Write for Text {
        fn write_str(&mut self, text: &str) -> fmt::Result {
            let available = self.cap.saturating_sub(self.text.len());
            let mut keep = available.min(text.len());
            while !text.is_char_boundary(keep) {
                keep -= 1;
            }
            self.text.push_str(&text[..keep]);
            if keep < text.len() {
                Err(fmt::Error)
            } else {
                Ok(())
            }
        }
    }
    let mut out = Text {
        text: String::new(),
        cap,
    };
    let truncated = fmt::write(&mut out, format_args!("{value:?}")).is_err();
    json!({"debug":out.text,"truncated":truncated,"authority":"diagnostic only"})
}

struct Buffer {
    bytes: Vec<u8>,
    cap: usize,
}
impl Write for Buffer {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        let requested = self
            .bytes
            .len()
            .checked_add(bytes.len())
            .ok_or_else(|| io::Error::other("output overflow"))?;
        if requested > self.cap {
            return Err(io::Error::other("record byte cap"));
        }
        self.bytes
            .try_reserve(bytes.len())
            .map_err(io::Error::other)?;
        self.bytes.extend_from_slice(bytes);
        Ok(bytes.len())
    }
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}
pub fn write_new_json(path: &Path, value: &impl Serialize, cap: usize) -> Result<usize, String> {
    let mut buffer = Buffer {
        bytes: Vec::new(),
        cap: cap.saturating_sub(1),
    };
    serde_json::to_writer(&mut buffer, value).map_err(|e| e.to_string())?;
    let mut file = OpenOptions::new()
        .create_new(true)
        .write(true)
        .open(path)
        .map_err(|e| e.to_string())?;
    file.write_all(&buffer.bytes)
        .and_then(|_| file.write_all(b"\n"))
        .and_then(|_| file.flush())
        .map_err(|e| e.to_string())?;
    Ok(buffer.bytes.len() + 1)
}

#[derive(Clone, Default, Debug, Serialize)]
pub(super) struct Counts {
    pub events: usize,
    pub classified: usize,
    pub selected: usize,
    pub terminals: usize,
    pub zeros: usize,
    pub other_classifications: usize,
    pub successors: usize,
    pub conditional_successors: usize,
    pub problems: usize,
    pub optional_refusal_events: usize,
    pub finished: usize,
}
#[derive(Clone, Default)]
pub(super) struct PieceState {
    pub selected: bool,
    pub successors: usize,
    pub problems: usize,
}
impl PieceState {
    pub fn finish(&mut self, successors: usize, problems: usize) -> Result<(), String> {
        if !self.selected || self.successors != successors || self.problems != problems {
            return Err("RuleFinished missing selection or delivered-count mismatch".into());
        }
        self.selected = false;
        Ok(())
    }
}
pub(super) fn complete_counts(
    counts: &Counts,
    state: &PieceState,
    stats: &OwnerAppliedStats,
) -> bool {
    !state.selected
        && counts.selected == counts.finished
        && counts.selected == stats.selected_pieces
        && counts.events == stats.events
        && counts.successors == stats.successors
        && counts.problems == stats.problems
        && counts.conditional_successors == stats.conditional_successors
        && counts.problems == 0
        && counts.optional_refusal_events == 0
        && stats.optional_coefficient_refusals == 0
        && stats.optional_original_refusals == 0
        && stats.optional_coalesced_refusals == 0
        && counts.other_classifications == 0
    // matching.pieces includes failed optional whole-piece probes; it is NOT a delivery count.
}

pub struct Recorder {
    output: PathBuf,
    limits: RecorderLimits,
    file: File,
    table: Option<CoefficientTableBuilder>,
    bytes: usize,
    events: usize,
    query: usize,
    piece: usize,
    source: Option<Value>,
    state: PieceState,
    counts: Counts,
    failure: Option<String>,
    queries_finished: usize,
}
impl Recorder {
    pub fn new(output: &Path, limits: RecorderLimits) -> Result<Self, String> {
        let binding_bytes = match std::fs::metadata(output.join("binding.json")) {
            Ok(metadata) => {
                usize::try_from(metadata.len()).map_err(|_| "binding length overflow")?
            }
            Err(error) if error.kind() == io::ErrorKind::NotFound => 0,
            Err(error) => return Err(error.to_string()),
        };
        if binding_bytes > limits.max_record_bytes || binding_bytes > limits.max_json_bytes {
            return Err("binding JSON byte cap".into());
        }
        let file = OpenOptions::new()
            .create_new(true)
            .write(true)
            .open(output.join("events.jsonl"))
            .map_err(|e| e.to_string())?;
        let binary = BinaryIoLimits {
            max_program_bytes: limits
                .max_total_atom_bytes
                .checked_add(limits.max_state_bytes)
                .ok_or("table cap overflow")?,
            max_collection_entries: limits.max_coefficients,
            max_state_bytes: limits.max_state_bytes,
            max_atom_bytes: limits.max_atom_bytes,
            max_total_atom_bytes: limits.max_total_atom_bytes,
            ..Default::default()
        };
        Ok(Self {
            output: output.to_owned(),
            limits,
            file,
            table: Some(CoefficientTableBuilder::new(binary)),
            bytes: binding_bytes,
            events: 0,
            query: 0,
            piece: 0,
            source: None,
            state: PieceState::default(),
            counts: Counts::default(),
            failure: None,
            queries_finished: 0,
        })
    }
    pub fn stopped(&self) -> bool {
        self.failure.is_some()
    }
    fn admit(&self, geometry_axes: usize) -> Result<(), String> {
        // Before any JSON geometry/error copy or coefficient interning, reserve a
        // conservative full record envelope (N<=16). Native table has its own
        // structural/table limits, but clones the coefficient before atom-byte
        // admission. Those are NOT native scratch allocation bounds; outer
        // RSS/time is mandatory. Three records reserve this event/query-begin,
        // the query ledger, and final result; binding bytes are already charged.
        let bound = 8192usize
            .checked_add(geometry_axes.checked_mul(512).ok_or("geometry overflow")?)
            .and_then(|n| n.checked_add(self.limits.max_error_bytes.checked_mul(6)?))
            .ok_or("record envelope overflow")?;
        if bound > self.limits.max_record_bytes
            || self
                .bytes
                .checked_add(self.limits.max_record_bytes.saturating_mul(3))
                .is_none_or(|n| n > self.limits.max_json_bytes)
        {
            return Err("recorder prospective byte cap".into());
        }
        Ok(())
    }
    fn emit(&mut self, value: &Value) -> Result<(), String> {
        let mut buffer = Buffer {
            bytes: Vec::new(),
            cap: self.limits.max_record_bytes.saturating_sub(1),
        };
        serde_json::to_writer(&mut buffer, value).map_err(|e| e.to_string())?;
        buffer.write_all(b"\n").map_err(|e| e.to_string())?;
        let total = self
            .bytes
            .checked_add(buffer.bytes.len())
            .ok_or("output overflow")?;
        if total > self.limits.max_json_bytes {
            return Err("aggregate JSON byte cap".into());
        }
        self.file
            .write_all(&buffer.bytes)
            .map_err(|e| e.to_string())?;
        self.bytes = total;
        Ok(())
    }
    pub fn begin_query(&mut self, ordinal: usize, query: &Query) -> Result<(), String> {
        self.admit(query.lower.len())?;
        self.query = ordinal;
        self.piece = 0;
        self.source = None;
        self.state = PieceState::default();
        self.counts = Counts::default();
        self.emit(&json!({"event":"query_begin","query":ordinal,"input":query}))
    }
    fn source_check<const N: usize>(
        &self,
        source: &OwnerDomainMatchPiece<N>,
    ) -> Result<(), String> {
        if !self.state.selected || self.source.as_ref() != Some(&piece(source)) {
            return Err("event source differs from active selected piece".into());
        }
        Ok(())
    }
    pub fn event<const N: usize>(&mut self, event: OwnerAppliedEvent<'_, N>) -> ControlFlow<()> {
        if self.failure.is_some() {
            return ControlFlow::Break(());
        }
        if let Err(error) = self.admit(N) {
            self.failure = Some(error);
            return ControlFlow::Break(());
        }
        let previous = (
            self.counts.clone(),
            self.state.clone(),
            self.source.clone(),
            self.piece,
        );
        if let Err(error) = self.record(event) {
            (self.counts, self.state, self.source, self.piece) = previous;
            self.failure = Some(error);
            ControlFlow::Break(())
        } else {
            ControlFlow::Continue(())
        }
    }
    fn record<const N: usize>(&mut self, event: OwnerAppliedEvent<'_, N>) -> Result<(), String> {
        self.admit(N)?;
        if self.events >= self.limits.max_events {
            return Err("aggregate event cap".into());
        }
        let mut value = match event {
            OwnerAppliedEvent::Classified(source) => {
                if self.state.selected {
                    return Err("new classification before RuleFinished".into());
                }
                self.piece += 1;
                self.counts.classified += 1;
                self.state = PieceState::default();
                match source.disposition() {
                    OwnerDomainMatchDisposition::SelectedRule { .. } => {
                        self.state.selected = true;
                        self.counts.selected += 1;
                    }
                    OwnerDomainMatchDisposition::Terminal { .. } => self.counts.terminals += 1,
                    OwnerDomainMatchDisposition::ExactZeroSector => self.counts.zeros += 1,
                    _ => self.counts.other_classifications += 1,
                }
                let source = piece(source);
                self.source = Some(source.clone());
                json!({"event":"classified","source":source})
            }
            OwnerAppliedEvent::Successor(child) => {
                self.source_check(child.source)?;
                let coefficient = self
                    .table
                    .as_mut()
                    .unwrap()
                    .intern(child.coefficient.raw())
                    .map_err(|e| e.to_string())?
                    .index();
                self.state.successors += 1;
                self.counts.successors += 1;
                if child.coefficient_nonzero == OwnerAppliedNonzero::Conditional {
                    self.counts.conditional_successors += 1;
                }
                json!({"event":"successor","source_lower":child.source_lower,"source_upper":child.source_upper,
                    "shift":child.shift.as_slice(),"coefficient":coefficient,"coefficient_nonzero":nonzero(child.coefficient_nonzero),
                    "target_sector":child.target_sector.as_slice(),"target_lower":child.target_lower,"target_upper":child.target_upper,
                    "target_rank_limit":child.target_rank_limit,"target_power_bounds":Powers::from_native(child.target_power_bounds),
                    "has_installed_target_owner":child.has_installed_target_owner})
            }
            OwnerAppliedEvent::Problem(problem) => {
                self.source_check(problem.source)?;
                let coefficient = problem
                    .coefficient
                    .map(|c| {
                        self.table
                            .as_mut()
                            .unwrap()
                            .intern(c.raw())
                            .map(|id| id.index())
                    })
                    .transpose()
                    .map_err(|e| e.to_string())?;
                self.state.problems += 1;
                self.counts.problems += 1;
                json!({"event":"problem","source_lower":problem.source_lower,"source_upper":problem.source_upper,
                    "shift":problem.shift.as_slice(),"original_term_ordinal":problem.original_term_ordinal,
                    "coefficient":coefficient,"coefficient_nonzero":nonzero(problem.coefficient_nonzero),
                    "kind":problem_kind(&problem.kind,self.limits.max_error_bytes)})
            }
            OwnerAppliedEvent::OptionalCoefficientRefusal {
                source,
                source_lower,
                source_upper,
                shift,
                original_term_ordinal,
                failure,
            } => {
                self.source_check(source)?;
                self.counts.optional_refusal_events += 1;
                json!({"event":"optional_coefficient_refusal","source_lower":source_lower,"source_upper":source_upper,
                    "shift":shift.as_slice(),"original_term_ordinal":original_term_ordinal,
                    "failure":bounded_debug(failure,self.limits.max_error_bytes)})
            }
            OwnerAppliedEvent::RuleFinished {
                source,
                successors,
                problems,
            } => {
                self.source_check(source)?;
                self.state.finish(successors, problems)?;
                self.counts.finished += 1;
                json!({"event":"rule_finished","successors":successors,"problems":problems})
            }
        };
        value["query"] = json!(self.query);
        value["piece"] = json!(self.piece);
        value["sequence"] = json!(self.events);
        self.emit(&value)?;
        self.events += 1;
        self.counts.events += 1;
        Ok(())
    }
    pub fn end_query(
        &mut self,
        result: Result<OwnerAppliedStats, OwnerAppliedError>,
        seconds: f64,
    ) -> Result<bool, String> {
        let (native_ok, stats, failure) = match result {
            Ok(stats) => (true, stats, Value::Null),
            Err(error) => (
                false,
                error.stats,
                json!({"kind":failure_kind(&error.failure),
                "detail":bounded_debug(&error.failure,self.limits.max_error_bytes),
                "max_numerator_rank":error.max_numerator_rank,"power_bounds":Powers::from_native(error.power_bounds)}),
            ),
        };
        let complete = native_ok
            && self.failure.is_none()
            && complete_counts(&self.counts, &self.state, &stats);
        let value = json!({"event":"query_finished","query":self.query,"native_ok":native_ok,
            "complete_outcome":complete,"native_failure":failure,"recorder_failure":self.failure,
            "delivered":self.counts,"native_stats":stats_json(&stats),"visitor_seconds":seconds});
        // On output exhaustion, emit this small failure receipt separately, never
        // claim that a missing/partial event prefix is a complete application.
        let available = self
            .limits
            .max_json_bytes
            .saturating_sub(self.bytes)
            .saturating_sub(self.limits.max_record_bytes);
        let bytes = write_new_json(
            &self.output.join(format!("query-{:06}.json", self.query)),
            &value,
            self.limits.max_record_bytes.min(available),
        )?;
        self.bytes = self.bytes.checked_add(bytes).ok_or("JSON byte overflow")?;
        self.queries_finished += 1;
        Ok(complete)
    }
    pub fn finish(mut self, mut timing: Value, mut complete: bool) -> Result<bool, String> {
        let encoding = Instant::now();
        complete &= self.failure.is_none();
        self.file.flush().map_err(|e| e.to_string())?;
        let count = self.table.as_ref().unwrap().len();
        let mut encoding_error = None;
        let mut table_info = Value::Null;
        match self.table.take().unwrap().finish() {
            Ok(table) => {
                for (name, bytes) in [
                    ("coefficients.state", &table.state),
                    ("coefficients.atoms", &table.atoms),
                ] {
                    let mut file = OpenOptions::new()
                        .create_new(true)
                        .write(true)
                        .open(self.output.join(name))
                        .map_err(|e| e.to_string())?;
                    file.write_all(bytes)
                        .and_then(|_| file.flush())
                        .map_err(|e| e.to_string())?;
                }
                table_info = json!({"count":count,"state_bytes":table.state.len(),"atoms_bytes":table.atoms.len(),
                    "state_blake3":blake3::hash(&table.state).to_hex().as_str(),"atoms_blake3":blake3::hash(&table.atoms).to_hex().as_str(),
                    "format":"native CoefficientTableBuilder state/atoms; IDs local to this output",
                    "import":"DecodedCoefficientTable::import_generated; generated trusted bytes only"});
            }
            Err(error) => {
                complete = false;
                encoding_error = Some(bounded_debug(&error, self.limits.max_error_bytes));
            }
        }
        timing["output_encoding_seconds"] = json!(encoding.elapsed().as_secs_f64());
        let report = json!({"schema":"rustred.owner-applied-observer.result.v1","complete_outcome":complete,
            "recorder_failure":self.failure,"coefficient_encoding_error":encoding_error,
            "events":self.events,"json_bytes_before_result":self.bytes,"queries_finished":self.queries_finished,
            "json_budget_scope":"binding + events + per-query ledgers + final result; result length is not self-counted in json_bytes_before_result",
            "coefficient_table":table_info,"timing":timing,
            "claims":{"pre_containment_observation_only":true,"closure":false,"portable_source_proof":false,
                "target_applicability":false,"policy_cost":false}});
        write_new_json(
            &self.output.join("result.json"),
            &report,
            self.limits
                .max_record_bytes
                .min(self.limits.max_json_bytes.saturating_sub(self.bytes)),
        )?;
        Ok(complete)
    }
}

fn nonzero(value: OwnerAppliedNonzero) -> &'static str {
    match value {
        OwnerAppliedNonzero::Uniform => "uniform",
        OwnerAppliedNonzero::Conditional => "conditional-coefficient-not-zero",
    }
}
fn piece<const N: usize>(p: &OwnerDomainMatchPiece<N>) -> Value {
    json!({"owner":p.owner().as_slice(),"lower":p.lower(),"upper":p.upper(),"max_numerator_rank":p.max_numerator_rank(),
        "power_bounds":Powers::from_native(p.power_bounds()),"disposition":disposition(p.disposition())})
}
fn disposition(value: OwnerDomainMatchDisposition) -> Value {
    match value {
        OwnerDomainMatchDisposition::SelectedRule { batch, rule } => {
            json!({"kind":"selected_rule","batch":batch,"rule":rule})
        }
        OwnerDomainMatchDisposition::Terminal { batch } => json!({"kind":"terminal","batch":batch}),
        OwnerDomainMatchDisposition::ExactGap => json!({"kind":"exact_gap"}),
        OwnerDomainMatchDisposition::Unresolved { predicate } => {
            json!({"kind":"unresolved","predicate":predicate_json(predicate)})
        }
        OwnerDomainMatchDisposition::InvalidSourceCondition { ordinal } => {
            json!({"kind":"invalid_source_condition","ordinal":ordinal})
        }
        OwnerDomainMatchDisposition::ExactZeroSector => json!({"kind":"exact_zero_sector"}),
    }
}
fn predicate_json(value: OwnerDomainPredicate) -> Value {
    match value {
        OwnerDomainPredicate::WholePieceAlternative { batch, rule } => {
            json!({"kind":"whole_piece_alternative","batch":batch,"rule":rule})
        }
        OwnerDomainPredicate::SourceCondition { ordinal } => {
            json!({"kind":"source_condition","ordinal":ordinal})
        }
        OwnerDomainPredicate::Equality {
            batch,
            rule,
            ordinal,
        } => json!({"kind":"equality","batch":batch,"rule":rule,"ordinal":ordinal}),
        OwnerDomainPredicate::ExcludedConjunction {
            batch,
            rule,
            branch,
            ordinal,
        } => {
            json!({"kind":"excluded_conjunction","batch":batch,"rule":rule,"branch":branch,"ordinal":ordinal})
        }
        OwnerDomainPredicate::OriginalDenominator { batch, rule, term } => {
            json!({"kind":"original_denominator","batch":batch,"rule":rule,"term":term})
        }
    }
}
fn problem_kind(value: &OwnerAppliedProblemKind, cap: usize) -> Value {
    match value {
        OwnerAppliedProblemKind::InvalidChildRoot => json!({"kind":"invalid_child_root"}),
        OwnerAppliedProblemKind::InvalidChildSourceCondition { ordinal } => {
            json!({"kind":"invalid_child_source_condition","ordinal":ordinal})
        }
        OwnerAppliedProblemKind::UnresolvedChildSourceCondition { ordinal } => {
            json!({"kind":"unresolved_child_source_condition","ordinal":ordinal})
        }
        OwnerAppliedProblemKind::DescentNotEstablished { detail } => {
            json!({"kind":"descent_not_established","detail":bounded_debug(detail,cap)})
        }
        OwnerAppliedProblemKind::UnresolvedFixedCoordinate { axis } => {
            json!({"kind":"unresolved_fixed_coordinate","axis":axis})
        }
        OwnerAppliedProblemKind::UnresolvedImage { detail } => {
            json!({"kind":"unresolved_image","detail":bounded_debug(detail,cap)})
        }
    }
}
fn failure_kind(value: &OwnerAppliedFailure) -> &'static str {
    match value {
        OwnerAppliedFailure::Matching(_) => "matching",
        OwnerAppliedFailure::Cancelled => "cancelled",
        OwnerAppliedFailure::StoppedByConsumer => "stopped_by_consumer",
        OwnerAppliedFailure::ResourceLimit { .. } => "resource_limit",
        OwnerAppliedFailure::CountOverflow { .. } => "count_overflow",
        OwnerAppliedFailure::AllocationFailure { .. } => "allocation_failure",
        OwnerAppliedFailure::Algebra(_) => "algebra",
        OwnerAppliedFailure::AffineRestriction(_) => "affine_restriction",
        OwnerAppliedFailure::Geometry(_) => "geometry",
        OwnerAppliedFailure::PowerDomain(_) => "power_domain",
        OwnerAppliedFailure::InternalInvariant(_) => "internal_invariant",
    }
}
fn stats_json(s: &OwnerAppliedStats) -> Value {
    macro_rules! object {($s:expr,$($field:ident),+)=>{json!({$(stringify!($field):$s.$field),+})};}
    let mut value = object!(
        s,
        selected_pieces,
        term_visits,
        shift_groups,
        boundary_cells,
        application_refinement_steps,
        application_refinement_cells,
        sign_splits,
        native_operations,
        optional_coefficient_refusals,
        optional_original_refusals,
        optional_coalesced_refusals,
        coalescing_additions,
        events,
        successors,
        conditional_successors,
        same_support_successors,
        strict_subsupport_successors,
        unsupported_support_successors,
        conditional_unsupported_support_successors,
        problems,
        zero_terms,
        cancelled_groups,
        zero_sector_groups,
        correlation_empty_cells
    );
    value["matching"] = object!(
        s.matching,
        rules,
        terminal_checks,
        predicates,
        pieces,
        cells,
        split_operations,
        coordinate_cells,
        rank_empty_cells,
        correlation_empty_cells,
        refinement_cells,
        refinement_steps
    );
    value
}
