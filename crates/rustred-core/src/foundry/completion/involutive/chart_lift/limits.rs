use super::super::InvolutiveLimits;

/// Resource policy for converting a sealed ordinary source module into one
/// sector-local forward Ore chart.
///
/// `max_chart_conversion_work` counts coordinate visits.  The exact bound is
/// four visits per input term-coordinate cell (preflight and construction
/// each scan the minima once and the output once), plus one visit per retained
/// source-row/coordinate cell when constructing the common left shifts.
///
/// Before any lifted batch storage is allocated, every input numerator,
/// denominator, and nonzero-condition polynomial is authenticated under the
/// nested exact-algebra limits. The same pass checks the largest input integer
/// magnitude under the indexed translation bit limit and computes aggregate
/// chart caps. Retained-byte accounting includes sparse coefficient/exponent
/// buffers and large-integer magnitude bytes; guards use the same wrapper/`Arc`
/// footprint as Ore localization storage. These are exact bounds on admitted
/// input payload. Affine translation output is checked separately, immediately
/// before each Symbolica-backed translation, because its sparse expansion
/// cannot be inferred from the input census alone.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct OrdinaryChartLiftLimits {
    pub(crate) max_source_rows: usize,
    pub(crate) max_input_terms: usize,
    pub(crate) max_input_conditions: usize,
    pub(crate) max_input_guard_terms: usize,
    pub(crate) max_input_guard_exponent_cells: usize,
    pub(crate) max_input_guard_retained_bytes: usize,
    pub(crate) max_input_symbolic_terms: usize,
    pub(crate) max_input_symbolic_exponent_cells: usize,
    pub(crate) max_input_symbolic_retained_bytes: usize,
    pub(crate) max_input_coordinate_cells: usize,
    pub(crate) max_lifted_coordinate_cells: usize,
    pub(crate) max_coefficient_translations: usize,
    pub(crate) max_chart_conversion_work: usize,
    pub(crate) involutive: InvolutiveLimits,
}

impl Default for OrdinaryChartLiftLimits {
    fn default() -> Self {
        Self {
            max_source_rows: 1_000_000,
            max_input_terms: 1_000_000,
            max_input_conditions: 1_000_000,
            max_input_guard_terms: 4_000_000,
            max_input_guard_exponent_cells: 64_000_000,
            max_input_guard_retained_bytes: 536_870_912,
            max_input_symbolic_terms: 8_000_000,
            max_input_symbolic_exponent_cells: 128_000_000,
            max_input_symbolic_retained_bytes: 1_073_741_824,
            max_input_coordinate_cells: 64_000_000,
            max_lifted_coordinate_cells: 64_000_000,
            max_coefficient_translations: 1_000_000,
            max_chart_conversion_work: 256_000_000,
            involutive: InvolutiveLimits::default(),
        }
    }
}
