use super::error::{check_limit, checked_add};
use super::{ModularGuideError, ModularGuideLimits, ModularProbeCensus};

/// Complete bounded-work census for one successful modular normal-form lane.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(super) struct ModularNormalFormCensus {
    problem_basis_rows: usize,
    problem_row_terms: usize,
    problem_guard_references: usize,
    peak_live_row_terms: usize,
    peak_live_guard_references: usize,
    axpy_input_terms: usize,
    axpy_transformed_terms: usize,
    monic_transformed_terms: usize,
    shift_coordinate_operations: usize,
    sampled_term_observations: usize,
    sampled_zero_observations: usize,
    normal_form_steps: usize,
    divisor_visits: usize,
    divisor_index_query_operations: usize,
    trace_steps: usize,
    trace_shift_coordinate_cells: usize,
    trace_bytes: usize,
    dag_nodes: usize,
    physical_deltas: usize,
    probe: ModularProbeCensus,
}

impl ModularNormalFormCensus {
    pub(super) const fn problem_basis_rows(self) -> usize {
        self.problem_basis_rows
    }

    pub(super) const fn problem_row_terms(self) -> usize {
        self.problem_row_terms
    }

    pub(super) const fn problem_guard_references(self) -> usize {
        self.problem_guard_references
    }

    pub(super) const fn peak_live_row_terms(self) -> usize {
        self.peak_live_row_terms
    }

    pub(super) const fn peak_live_guard_references(self) -> usize {
        self.peak_live_guard_references
    }

    pub(super) const fn axpy_input_terms(self) -> usize {
        self.axpy_input_terms
    }

    pub(super) const fn axpy_transformed_terms(self) -> usize {
        self.axpy_transformed_terms
    }

    pub(super) const fn monic_transformed_terms(self) -> usize {
        self.monic_transformed_terms
    }

    pub(super) const fn shift_coordinate_operations(self) -> usize {
        self.shift_coordinate_operations
    }

    pub(super) const fn sampled_term_observations(self) -> usize {
        self.sampled_term_observations
    }

    pub(super) const fn sampled_zero_observations(self) -> usize {
        self.sampled_zero_observations
    }

    pub(super) const fn normal_form_steps(self) -> usize {
        self.normal_form_steps
    }

    pub(super) const fn divisor_visits(self) -> usize {
        self.divisor_visits
    }

    pub(super) const fn divisor_index_query_operations(self) -> usize {
        self.divisor_index_query_operations
    }

    pub(super) const fn trace_steps(self) -> usize {
        self.trace_steps
    }

    pub(super) const fn trace_shift_coordinate_cells(self) -> usize {
        self.trace_shift_coordinate_cells
    }

    pub(super) const fn trace_bytes(self) -> usize {
        self.trace_bytes
    }

    pub(super) const fn dag_nodes(self) -> usize {
        self.dag_nodes
    }

    pub(super) const fn physical_deltas(self) -> usize {
        self.physical_deltas
    }

    pub(super) const fn probe(self) -> ModularProbeCensus {
        self.probe
    }
}

#[derive(Debug, Default)]
pub(super) struct ModularNormalFormWork {
    census: ModularNormalFormCensus,
}

impl ModularNormalFormWork {
    pub(super) fn admit_problem(
        &mut self,
        basis_rows: usize,
        row_terms: usize,
        guard_references: usize,
        limits: ModularGuideLimits,
    ) -> Result<(), ModularGuideError> {
        check_limit(
            "modular normal-form problem basis rows",
            basis_rows,
            limits.max_problem_basis_rows,
        )?;
        check_limit(
            "modular normal-form problem row terms",
            row_terms,
            limits.max_problem_row_terms,
        )?;
        check_limit(
            "modular normal-form problem guard references",
            guard_references,
            limits.max_problem_guard_references,
        )?;
        self.census.problem_basis_rows = basis_rows;
        self.census.problem_row_terms = row_terms;
        self.census.problem_guard_references = guard_references;
        Ok(())
    }

    pub(super) fn observe_live_row(
        &mut self,
        terms: usize,
        guards: usize,
        limits: ModularGuideLimits,
    ) -> Result<(), ModularGuideError> {
        check_limit("modular live row terms", terms, limits.max_live_row_terms)?;
        check_limit(
            "modular live guard references",
            guards,
            limits.max_live_guard_references,
        )?;
        self.census.peak_live_row_terms = self.census.peak_live_row_terms.max(terms);
        self.census.peak_live_guard_references = self.census.peak_live_guard_references.max(guards);
        Ok(())
    }

    pub(super) fn charge_axpy(
        &mut self,
        input_terms: usize,
        transformed_terms: usize,
        arity: usize,
        limits: ModularGuideLimits,
    ) -> Result<(), ModularGuideError> {
        check_limit(
            "modular Ore AXPY input terms",
            input_terms,
            limits.max_axpy_input_terms,
        )?;
        self.census.axpy_input_terms = checked_add(
            "modular cumulative Ore AXPY input terms",
            self.census.axpy_input_terms,
            input_terms,
        )?;
        self.census.axpy_transformed_terms = checked_add(
            "modular Ore AXPY transformed terms",
            self.census.axpy_transformed_terms,
            transformed_terms,
        )?;
        check_limit(
            "modular Ore AXPY transformed terms",
            self.census.axpy_transformed_terms,
            limits.max_total_axpy_transformed_terms,
        )?;
        self.charge_shift_coordinates(transformed_terms, arity, limits)
    }

    pub(super) fn charge_monic(
        &mut self,
        transformed_terms: usize,
        limits: ModularGuideLimits,
    ) -> Result<(), ModularGuideError> {
        self.census.monic_transformed_terms = checked_add(
            "modular monic transformed terms",
            self.census.monic_transformed_terms,
            transformed_terms,
        )?;
        check_limit(
            "modular monic transformed terms",
            self.census.monic_transformed_terms,
            limits.max_total_monic_transformed_terms,
        )
    }

    pub(super) fn charge_sampled_terms(
        &mut self,
        terms: usize,
        sampled_zeros: usize,
        limits: ModularGuideLimits,
    ) -> Result<(), ModularGuideError> {
        self.census.sampled_term_observations = checked_add(
            "modular sampled term observations",
            self.census.sampled_term_observations,
            terms,
        )?;
        check_limit(
            "modular sampled term observations",
            self.census.sampled_term_observations,
            limits.max_total_sampled_term_observations,
        )?;
        self.census.sampled_zero_observations = checked_add(
            "modular sampled-zero observations",
            self.census.sampled_zero_observations,
            sampled_zeros,
        )?;
        Ok(())
    }

    pub(super) fn charge_divisor_visits(
        &mut self,
        amount: usize,
        limits: ModularGuideLimits,
    ) -> Result<(), ModularGuideError> {
        charge(
            "modular normal-form divisor visits",
            &mut self.census.divisor_visits,
            amount,
            limits.max_normal_form_divisor_visits,
        )
    }

    pub(super) fn charge_normal_form_step(
        &mut self,
        limits: ModularGuideLimits,
    ) -> Result<(), ModularGuideError> {
        charge_one(
            "modular normal-form steps",
            &mut self.census.normal_form_steps,
            limits.max_normal_form_steps,
        )
    }

    pub(super) fn charge_trace(
        &mut self,
        steps: usize,
        shift_coordinate_cells: usize,
        bytes: usize,
        limits: ModularGuideLimits,
    ) -> Result<(), ModularGuideError> {
        self.census.trace_steps = checked_add(
            "modular normal-form trace steps",
            self.census.trace_steps,
            steps,
        )?;
        check_limit(
            "modular normal-form trace steps",
            self.census.trace_steps,
            limits.max_trace_steps,
        )?;
        self.census.trace_shift_coordinate_cells = checked_add(
            "modular normal-form trace shift coordinate cells",
            self.census.trace_shift_coordinate_cells,
            shift_coordinate_cells,
        )?;
        check_limit(
            "modular normal-form trace shift coordinate cells",
            self.census.trace_shift_coordinate_cells,
            limits.max_trace_shift_coordinate_cells,
        )?;
        self.census.trace_bytes = checked_add(
            "modular normal-form trace bytes",
            self.census.trace_bytes,
            bytes,
        )?;
        check_limit(
            "modular normal-form trace bytes",
            self.census.trace_bytes,
            limits.max_trace_bytes,
        )
    }

    pub(super) fn finish(
        mut self,
        divisor_index_query_operations: usize,
        dag_nodes: usize,
        physical_deltas: usize,
        probe: ModularProbeCensus,
    ) -> ModularNormalFormCensus {
        self.census.divisor_index_query_operations = divisor_index_query_operations;
        self.census.dag_nodes = dag_nodes;
        self.census.physical_deltas = physical_deltas;
        self.census.probe = probe;
        self.census
    }

    fn charge_shift_coordinates(
        &mut self,
        terms: usize,
        arity: usize,
        limits: ModularGuideLimits,
    ) -> Result<(), ModularGuideError> {
        let cells = terms
            .checked_mul(arity)
            .ok_or(ModularGuideError::ResourceCountOverflow {
                resource: "modular Ore shift coordinate operations",
            })?;
        self.census.shift_coordinate_operations = checked_add(
            "modular Ore shift coordinate operations",
            self.census.shift_coordinate_operations,
            cells,
        )?;
        check_limit(
            "modular Ore shift coordinate operations",
            self.census.shift_coordinate_operations,
            limits.max_total_shift_coordinate_operations,
        )
    }
}

fn charge_one(
    resource: &'static str,
    value: &mut usize,
    limit: usize,
) -> Result<(), ModularGuideError> {
    let requested = checked_add(resource, *value, 1)?;
    check_limit(resource, requested, limit)?;
    *value = requested;
    Ok(())
}

fn charge(
    resource: &'static str,
    value: &mut usize,
    amount: usize,
    limit: usize,
) -> Result<(), ModularGuideError> {
    let requested = checked_add(resource, *value, amount)?;
    check_limit(resource, requested, limit)?;
    *value = requested;
    Ok(())
}
