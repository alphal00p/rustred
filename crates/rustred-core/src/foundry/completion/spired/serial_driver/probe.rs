use crate::foundry::completion::source_discovery::{CampaignLimits, CampaignModularProbe};

use super::super::SpiredCoordinateCaseObligation;
use super::{
    SpiredSerialProbeBuildError, SpiredSerialProbeCensus, SpiredSerialProbePortfolio,
    SpiredSerialProbePortfolioError, SpiredSerialProbePortfolioLimits,
};

const MODULI: &str = "moduli";
const BASE_POINTS: &str = "base-parameter points";
const BASE_CELLS: &str = "base-parameter cells";
const CHART_RANK_POINTS: &str = "chart-rank points";
const CHART_RANK_CELLS: &str = "chart-rank cells";
const TEMPLATES: &str = "probe templates";
const PROBES: &str = "generated probes";
const COORDINATE_CELLS: &str = "retained coordinate cells";

impl SpiredSerialProbePortfolio {
    /// Retain one finite, explicitly ordered Cartesian probe portfolio.
    pub(crate) fn try_new<M, B, P, R, Q>(
        moduli: M,
        base_parameter_points: B,
        chart_rank_points: R,
        limits: SpiredSerialProbePortfolioLimits,
    ) -> Result<Self, SpiredSerialProbePortfolioError>
    where
        M: IntoIterator<Item = u64>,
        B: IntoIterator<Item = P>,
        P: IntoIterator<Item = i64>,
        R: IntoIterator<Item = Q>,
        Q: IntoIterator<Item = u64>,
    {
        let moduli = try_collect_bounded(moduli, MODULI, limits.max_moduli)?;
        let mut rank_points = Vec::new();
        let mut rank_cells = 0usize;
        for point in chart_rank_points {
            let requested = checked_add(CHART_RANK_POINTS, rank_points.len(), 1)?;
            check_limit(CHART_RANK_POINTS, requested, limits.max_chart_rank_points)?;
            let mut retained = Vec::new();
            for value in point {
                rank_cells = checked_add(CHART_RANK_CELLS, rank_cells, 1)?;
                check_limit(CHART_RANK_CELLS, rank_cells, limits.max_chart_rank_cells)?;
                retained.try_reserve(1).map_err(|_| {
                    SpiredSerialProbePortfolioError::AllocationFailure {
                        resource: CHART_RANK_CELLS,
                        requested: retained.len().saturating_add(1),
                    }
                })?;
                retained.push(value);
            }
            rank_points.try_reserve(1).map_err(|_| {
                SpiredSerialProbePortfolioError::AllocationFailure {
                    resource: CHART_RANK_POINTS,
                    requested,
                }
            })?;
            rank_points.push(retained.into_boxed_slice());
        }
        let mut points = Vec::new();
        let mut cells = 0usize;
        for point in base_parameter_points {
            let requested = checked_add(BASE_POINTS, points.len(), 1)?;
            check_limit(BASE_POINTS, requested, limits.max_base_parameter_points)?;
            let mut retained = Vec::new();
            for value in point {
                cells = checked_add(BASE_CELLS, cells, 1)?;
                check_limit(BASE_CELLS, cells, limits.max_base_parameter_cells)?;
                retained.try_reserve(1).map_err(|_| {
                    SpiredSerialProbePortfolioError::AllocationFailure {
                        resource: BASE_CELLS,
                        requested: retained.len().saturating_add(1),
                    }
                })?;
                retained.push(value);
            }
            points.try_reserve(1).map_err(|_| {
                SpiredSerialProbePortfolioError::AllocationFailure {
                    resource: BASE_POINTS,
                    requested,
                }
            })?;
            points.push(retained.into_boxed_slice());
        }
        let template_count = checked_mul(TEMPLATES, moduli.len(), points.len())
            .and_then(|value| checked_mul(TEMPLATES, value, rank_points.len()))?;
        check_limit(TEMPLATES, template_count, limits.max_probe_templates)?;
        Ok(Self {
            moduli: moduli.into_boxed_slice(),
            base_parameter_points: points.into_boxed_slice(),
            chart_rank_points: rank_points.into_boxed_slice(),
            probe_template_count: template_count,
        })
    }
}

pub(super) fn try_build_case_probes(
    case: &SpiredCoordinateCaseObligation,
    expected_base_parameters: usize,
    portfolio: &SpiredSerialProbePortfolio,
    campaign_limits: CampaignLimits,
    max_probes: usize,
    max_coordinate_cells: usize,
) -> Result<(Vec<CampaignModularProbe>, SpiredSerialProbeCensus), SpiredSerialProbeBuildError> {
    for (point_ordinal, point) in portfolio.base_parameter_points().iter().enumerate() {
        if point.len() != expected_base_parameters {
            return Err(SpiredSerialProbeBuildError::WrongBaseParameterArity {
                point_ordinal,
                expected: expected_base_parameters,
                actual: point.len(),
            });
        }
    }
    for (point_ordinal, point) in portfolio.chart_rank_points().iter().enumerate() {
        if point.len() != case.stratum().domain().arity() {
            return Err(SpiredSerialProbeBuildError::WrongChartRankArity {
                point_ordinal,
                expected: case.stratum().domain().arity(),
                actual: point.len(),
            });
        }
    }
    let probe_count = portfolio.probe_template_count();
    check_build_limit(PROBES, probe_count, max_probes)?;
    let per_probe = checked_build_add(
        COORDINATE_CELLS,
        expected_base_parameters,
        case.stratum().domain().arity(),
    )?;
    let coordinate_cells = checked_build_mul(COORDINATE_CELLS, probe_count, per_probe)?;
    check_build_limit(COORDINATE_CELLS, coordinate_cells, max_coordinate_cells)?;

    let mut probes = Vec::new();
    probes.try_reserve_exact(probe_count).map_err(|_| {
        SpiredSerialProbeBuildError::AllocationFailure {
            resource: PROBES,
            requested: probe_count,
        }
    })?;
    let mut free_axes = 0usize;
    let mut fixed_axes = 0usize;
    for ranks in portfolio.chart_rank_points() {
        let (chart, this_free, this_fixed) = try_chart_anchor(case, ranks)?;
        free_axes = this_free;
        fixed_axes = this_fixed;
        for base in portfolio.base_parameter_points() {
            for &modulus in portfolio.moduli() {
                probes.push(
                    CampaignModularProbe::try_new(
                        modulus,
                        base.iter().copied(),
                        chart.iter().copied(),
                        campaign_limits,
                    )
                    .map_err(SpiredSerialProbeBuildError::Campaign)?,
                );
            }
        }
    }
    debug_assert_eq!(probes.len(), probe_count);
    Ok((
        probes,
        SpiredSerialProbeCensus {
            free_axes,
            fixed_axes,
            probes_generated: probe_count,
            retained_coordinate_cells: coordinate_cells,
        },
    ))
}

fn try_chart_anchor(
    case: &SpiredCoordinateCaseObligation,
    ranks: &[u64],
) -> Result<(Vec<u64>, usize, usize), SpiredSerialProbeBuildError> {
    let arity = case.stratum().domain().arity();
    let mut chart = Vec::new();
    chart
        .try_reserve_exact(arity)
        .map_err(|_| SpiredSerialProbeBuildError::AllocationFailure {
            resource: COORDINATE_CELLS,
            requested: arity,
        })?;
    let mut free_count = 0usize;
    let mut fixed = 0usize;
    for (position, (&active, &bounds)) in case
        .stratum()
        .domain()
        .sector()
        .active_bits()
        .iter()
        .zip(case.stratum().domain().bounds())
        .enumerate()
    {
        let (lower, upper) = try_chart_interval(position, active, bounds.lower(), bounds.upper())?;
        let coordinate = if bounds.lower() == bounds.upper() {
            fixed += 1;
            lower
        } else {
            let width = upper
                .checked_sub(lower)
                .and_then(|span| span.checked_add(1))
                .ok_or(SpiredSerialProbeBuildError::CoordinateOverflow { position })?;
            free_count += 1;
            lower
                .checked_add(ranks[position] % width)
                .ok_or(SpiredSerialProbeBuildError::CoordinateOverflow { position })?
        };
        chart.push(coordinate);
    }
    Ok((chart, free_count, fixed))
}

fn try_chart_interval(
    position: usize,
    active: bool,
    lower: i64,
    upper: i64,
) -> Result<(u64, u64), SpiredSerialProbeBuildError> {
    if lower > upper || (active && lower < 1) || (!active && upper > 0) {
        return Err(SpiredSerialProbeBuildError::InvalidCaseCoordinate {
            position,
            active,
            lower,
            upper,
        });
    }
    if active {
        Ok(((lower - 1) as u64, (upper - 1) as u64))
    } else {
        Ok((
            index_to_inactive_chart(upper),
            index_to_inactive_chart(lower),
        ))
    }
}

fn index_to_inactive_chart(index: i64) -> u64 {
    if index == i64::MIN {
        (i64::MAX as u64) + 1
    } else {
        (-index) as u64
    }
}

fn try_collect_bounded<T>(
    values: impl IntoIterator<Item = T>,
    resource: &'static str,
    limit: usize,
) -> Result<Vec<T>, SpiredSerialProbePortfolioError> {
    let mut retained = Vec::new();
    for value in values {
        let requested = checked_add(resource, retained.len(), 1)?;
        check_limit(resource, requested, limit)?;
        retained.try_reserve(1).map_err(|_| {
            SpiredSerialProbePortfolioError::AllocationFailure {
                resource,
                requested,
            }
        })?;
        retained.push(value);
    }
    Ok(retained)
}

fn checked_add(
    resource: &'static str,
    left: usize,
    right: usize,
) -> Result<usize, SpiredSerialProbePortfolioError> {
    left.checked_add(right)
        .ok_or(SpiredSerialProbePortfolioError::ResourceCountOverflow { resource })
}

fn checked_mul(
    resource: &'static str,
    left: usize,
    right: usize,
) -> Result<usize, SpiredSerialProbePortfolioError> {
    left.checked_mul(right)
        .ok_or(SpiredSerialProbePortfolioError::ResourceCountOverflow { resource })
}

fn check_limit(
    resource: &'static str,
    requested: usize,
    limit: usize,
) -> Result<(), SpiredSerialProbePortfolioError> {
    if requested > limit {
        Err(SpiredSerialProbePortfolioError::ResourceLimit {
            resource,
            requested,
            limit,
        })
    } else {
        Ok(())
    }
}

fn checked_build_add(
    resource: &'static str,
    left: usize,
    right: usize,
) -> Result<usize, SpiredSerialProbeBuildError> {
    left.checked_add(right)
        .ok_or(SpiredSerialProbeBuildError::ResourceCountOverflow { resource })
}

fn checked_build_mul(
    resource: &'static str,
    left: usize,
    right: usize,
) -> Result<usize, SpiredSerialProbeBuildError> {
    left.checked_mul(right)
        .ok_or(SpiredSerialProbeBuildError::ResourceCountOverflow { resource })
}

fn check_build_limit(
    resource: &'static str,
    requested: usize,
    limit: usize,
) -> Result<(), SpiredSerialProbeBuildError> {
    if requested > limit {
        Err(SpiredSerialProbeBuildError::ResourceLimit {
            resource,
            requested,
            limit,
        })
    } else {
        Ok(())
    }
}
