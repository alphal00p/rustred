use crate::foundry::campaign::FoundryCampaignSetupStage;
use crate::foundry::completion::{LatticeBox, SectorChart};
use crate::identity::TranslatedSourceBatch;
use crate::sector::Mask;

use super::CachedSetupFailure;

/// Exact componentwise one-step stencil of the complete ordinary module.
///
/// From a physical support `x`, nominating source term `s` constructs request
/// offset `x - s`; translating that request constructs physical term
/// `x + (q - s)`. Only the latter may become obstruction support in a later
/// epoch, so the one-shot request suffix and repeatable physical step are
/// deliberately distinct.
#[derive(Debug)]
struct SourceExpansionStencil {
    physical_minimum: Vec<i128>,
    physical_maximum: Vec<i128>,
    request_minimum: Vec<i128>,
    request_maximum: Vec<i128>,
}

impl SourceExpansionStencil {
    fn try_new(
        zero_sources: &TranslatedSourceBatch,
        arity: usize,
    ) -> Result<Self, CachedSetupFailure> {
        let mut physical_minimum =
            try_filled(arity, 0, "could not reserve K6 physical-step minima")?;
        let mut physical_maximum =
            try_filled(arity, 0, "could not reserve K6 physical-step maxima")?;
        let mut request_minimum = try_filled(
            arity,
            i128::MAX,
            "could not reserve K6 request-suffix minima",
        )?;
        let mut request_maximum = try_filled(
            arity,
            i128::MIN,
            "could not reserve K6 request-suffix maxima",
        )?;

        if zero_sources.sources().is_empty() {
            return Err(invariant("K6 ordinary source module is empty"));
        }
        for source in zero_sources.sources() {
            if source.terms().is_empty() {
                return Err(invariant("K6 ordinary source row has empty support"));
            }
            for position in 0..arity {
                let mut source_minimum = i128::MAX;
                let mut source_maximum = i128::MIN;
                for shift in source.terms().keys() {
                    let value =
                        i128::from(*shift.values().get(position).ok_or_else(|| {
                            invariant("K6 ordinary source term has the wrong arity")
                        })?);
                    source_minimum = source_minimum.min(value);
                    source_maximum = source_maximum.max(value);
                }

                // q and s range independently within one translated row.
                // Reusing the same extremizing pair makes each endpoint
                // repeatable, so these component extrema are exact.
                physical_minimum[position] =
                    physical_minimum[position].min(source_minimum - source_maximum);
                physical_maximum[position] =
                    physical_maximum[position].max(source_maximum - source_minimum);
                request_minimum[position] = request_minimum[position].min(-source_maximum);
                request_maximum[position] = request_maximum[position].max(-source_minimum);
            }
        }
        if request_minimum.contains(&i128::MAX) || request_maximum.contains(&i128::MIN) {
            return Err(invariant("K6 request suffix stencil is incomplete"));
        }
        Ok(Self {
            physical_minimum,
            physical_maximum,
            request_minimum,
            request_maximum,
        })
    }
}

/// Exact componentwise displacement envelope of one bounded scheduler probe.
#[derive(Debug)]
struct SourceExpansionEnvelope {
    minimum: Vec<i128>,
    maximum: Vec<i128>,
}

impl SourceExpansionEnvelope {
    fn try_new(
        stencil: &SourceExpansionStencil,
        max_iterations_per_probe: usize,
    ) -> Result<Self, CachedSetupFailure> {
        // Epoch zero materializes bootstrap translations at physical depth 1.
        // The last admitted epoch immediately translates its residual
        // nominations before the next epoch-admission check, so a probe with
        // limit N can construct physical depth N+1. Its nominated request
        // offsets start from support depth at most N.
        let physical_depth = max_iterations_per_probe
            .checked_add(1)
            .ok_or_else(|| invariant("K6 bounded source-expansion depth overflowed"))?;
        let physical_depth = i128::try_from(physical_depth)
            .map_err(|_| invariant("K6 bounded source-expansion depth is not representable"))?;
        let request_support_depth = i128::try_from(max_iterations_per_probe)
            .map_err(|_| invariant("K6 bounded request-support depth is not representable"))?;

        let arity = stencil.physical_minimum.len();
        let mut minimum = try_filled(arity, 0, "could not reserve K6 cumulative minima")?;
        let mut maximum = try_filled(arity, 0, "could not reserve K6 cumulative maxima")?;
        for position in 0..arity {
            let physical_minimum = stencil.physical_minimum[position]
                .checked_mul(physical_depth)
                .ok_or_else(|| invariant("K6 physical source-expansion minimum overflowed"))?;
            let physical_maximum = stencil.physical_maximum[position]
                .checked_mul(physical_depth)
                .ok_or_else(|| invariant("K6 physical source-expansion maximum overflowed"))?;
            let request_minimum = stencil.physical_minimum[position]
                .checked_mul(request_support_depth)
                .and_then(|value| value.checked_add(stencil.request_minimum[position]))
                .ok_or_else(|| invariant("K6 request source-expansion minimum overflowed"))?;
            let request_maximum = stencil.physical_maximum[position]
                .checked_mul(request_support_depth)
                .and_then(|value| value.checked_add(stencil.request_maximum[position]))
                .ok_or_else(|| invariant("K6 request source-expansion maximum overflowed"))?;
            minimum[position] = physical_minimum.min(request_minimum).min(0);
            maximum[position] = physical_maximum.max(request_maximum).max(0);
        }
        Ok(Self { minimum, maximum })
    }
}

/// Largest origin-anchored sector carrier on which the complete bounded
/// scheduler walk can materialize every nomination and translated term.
pub(super) fn source_safe_closure_carrier(
    zero_sources: &TranslatedSourceBatch,
    sector: &Mask,
    max_iterations_per_probe: usize,
) -> Result<LatticeBox, CachedSetupFailure> {
    let arity = sector.arity();
    let stencil = SourceExpansionStencil::try_new(zero_sources, arity)?;
    let envelope = SourceExpansionEnvelope::try_new(&stencil, max_iterations_per_probe)?;
    let chart_carrier = SectorChart::new(sector.clone())
        .carrier_box()
        .map_err(|error| CachedSetupFailure::new(FoundryCampaignSetupStage::Ledger, error))?;

    let lower = try_filled_u64(arity, 0, "could not reserve K6 carrier lower endpoints")?;
    let mut upper = Vec::new();
    upper
        .try_reserve_exact(arity)
        .map_err(|_| invariant("could not reserve K6 carrier upper endpoints"))?;
    for (position, &active) in sector.active_bits().iter().enumerate() {
        // Boundary tasks pass a corner-relative IntegralShift to source
        // translation. Thus an active chart coordinate c has target shift c
        // (although its absolute integral power is c+1); an inactive one has
        // target shift -c. SectorChart independently bounds the absolute key.
        let source_safe_upper = if active {
            if envelope.minimum[position] < i128::from(i64::MIN) {
                return Err(invariant("K6 active source-safe carrier is empty"));
            }
            i128::from(i64::MAX)
                .checked_sub(envelope.maximum[position])
                .and_then(|value| u64::try_from(value).ok())
                .ok_or_else(|| invariant("K6 active source-safe carrier is empty"))?
        } else {
            if envelope.maximum[position] > i128::from(i64::MAX) {
                return Err(invariant("K6 inactive source-safe carrier is empty"));
            }
            envelope.minimum[position]
                .checked_sub(i128::from(i64::MIN))
                .and_then(|value| u64::try_from(value).ok())
                .ok_or_else(|| invariant("K6 inactive source-safe carrier is empty"))?
        };
        let chart_upper = chart_carrier.upper()[position]
            .ok_or_else(|| invariant("K6 sector chart has an unbounded endpoint"))?;
        upper.push(Some(source_safe_upper.min(chart_upper)));
    }
    LatticeBox::try_new(lower, upper)
        .map_err(|error| CachedSetupFailure::new(FoundryCampaignSetupStage::Ledger, error))
}

fn try_filled(
    arity: usize,
    value: i128,
    allocation_error: &'static str,
) -> Result<Vec<i128>, CachedSetupFailure> {
    let mut values = Vec::new();
    values.try_reserve_exact(arity).map_err(|_| {
        CachedSetupFailure::invariant(FoundryCampaignSetupStage::Ledger, allocation_error)
    })?;
    values.resize(arity, value);
    Ok(values)
}

fn try_filled_u64(
    arity: usize,
    value: u64,
    allocation_error: &'static str,
) -> Result<Vec<u64>, CachedSetupFailure> {
    let mut values = Vec::new();
    values.try_reserve_exact(arity).map_err(|_| {
        CachedSetupFailure::invariant(FoundryCampaignSetupStage::Ledger, allocation_error)
    })?;
    values.resize(arity, value);
    Ok(values)
}

fn invariant(message: &'static str) -> CachedSetupFailure {
    CachedSetupFailure::invariant(FoundryCampaignSetupStage::Ledger, message)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::foundry::artifact::FULL_RANK_ORBITS;
    use crate::foundry::completion::source_discovery::ProbeCampaignLimits;

    use super::super::shared_k6_algebra_inputs;

    #[test]
    fn bounded_recursive_stencil_inset_is_safe_and_endpoint_maximal_for_every_k6_orbit() {
        let inputs = shared_k6_algebra_inputs().unwrap();
        let iterations = ProbeCampaignLimits::default()
            .replay
            .scheduler
            .max_iterations_per_probe;
        let stencil = SourceExpansionStencil::try_new(inputs.zero_sources(), 6).unwrap();
        let envelope = SourceExpansionEnvelope::try_new(&stencil, iterations).unwrap();
        assert_eq!(stencil.physical_minimum, [-2; 6]);
        assert_eq!(stencil.physical_maximum, [2; 6]);
        assert_eq!(stencil.request_minimum, [-1; 6]);
        assert_eq!(stencil.request_maximum, [1; 6]);
        assert_eq!(envelope.minimum, [-8_194; 6]);
        assert_eq!(envelope.maximum, [8_194; 6]);

        for orbit in FULL_RANK_ORBITS {
            let sector = Mask::try_from_indices(&orbit.representative).unwrap();
            let carrier =
                source_safe_closure_carrier(inputs.zero_sources(), &sector, iterations).unwrap();
            let chart_carrier = SectorChart::new(sector.clone()).carrier_box().unwrap();
            assert_eq!(carrier.lower(), [0; 6]);

            for (position, &active) in sector.active_bits().iter().enumerate() {
                let coordinate = carrier.upper()[position].unwrap();
                let chart_upper = chart_carrier.upper()[position].unwrap();
                let expected = if active {
                    u64::try_from(i128::from(i64::MAX) - envelope.maximum[position]).unwrap()
                } else {
                    u64::try_from(envelope.minimum[position] - i128::from(i64::MIN)).unwrap()
                }
                .min(chart_upper);
                assert_eq!(coordinate, expected);

                let target = chart_target_shift(coordinate, active);
                assert_representable(target + envelope.minimum[position]);
                assert_representable(target + envelope.maximum[position]);
                let absolute_power = if active { target + 1 } else { target };
                assert_representable(absolute_power);

                if coordinate < chart_upper {
                    let outside_target = chart_target_shift(coordinate + 1, active);
                    assert!(
                        outside_target + envelope.minimum[position] < i128::from(i64::MIN)
                            || outside_target + envelope.maximum[position] > i128::from(i64::MAX),
                        "axis {position} retained a nonmaximal bounded-walk inset"
                    );
                }
            }
        }
    }

    #[test]
    fn carrier_endpoint_admits_every_iterated_physical_and_request_nomination_prefix() {
        let inputs = shared_k6_algebra_inputs().unwrap();
        let iterations = ProbeCampaignLimits::default()
            .replay
            .scheduler
            .max_iterations_per_probe;
        let stencil = SourceExpansionStencil::try_new(inputs.zero_sources(), 6).unwrap();

        for orbit in FULL_RANK_ORBITS {
            let sector = Mask::try_from_indices(&orbit.representative).unwrap();
            let carrier =
                source_safe_closure_carrier(inputs.zero_sources(), &sector, iterations).unwrap();
            for (position, &active) in sector.active_bits().iter().enumerate() {
                assert_extrema_have_source_witness(inputs.zero_sources(), &stencil, position);
                let target = chart_target_shift(carrier.upper()[position].unwrap(), active);

                for depth in 0..=iterations {
                    let depth = i128::try_from(depth).unwrap();
                    for (step, suffix) in [
                        (&stencil.physical_minimum, &stencil.request_minimum),
                        (&stencil.physical_maximum, &stencil.request_maximum),
                    ] {
                        let support = target + depth * step[position];
                        assert_representable(support);
                        assert_representable(support + suffix[position]);
                    }
                }

                let final_physical_depth = i128::try_from(iterations + 1).unwrap();
                assert_representable(
                    target + final_physical_depth * stencil.physical_minimum[position],
                );
                assert_representable(
                    target + final_physical_depth * stencil.physical_maximum[position],
                );
            }
        }
    }

    fn chart_target_shift(coordinate: u64, active: bool) -> i128 {
        if active {
            i128::from(coordinate)
        } else {
            -i128::from(coordinate)
        }
    }

    fn assert_representable(value: i128) {
        assert!((i128::from(i64::MIN)..=i128::from(i64::MAX)).contains(&value));
    }

    fn assert_extrema_have_source_witness(
        sources: &TranslatedSourceBatch,
        stencil: &SourceExpansionStencil,
        position: usize,
    ) {
        for expected in [
            stencil.physical_minimum[position],
            stencil.physical_maximum[position],
        ] {
            assert!(sources.sources().iter().any(|source| {
                source.terms().keys().any(|q| {
                    source.terms().keys().any(|s| {
                        i128::from(q.values()[position]) - i128::from(s.values()[position])
                            == expected
                    })
                })
            }));
        }
        for expected in [
            stencil.request_minimum[position],
            stencil.request_maximum[position],
        ] {
            assert!(sources.sources().iter().any(|source| {
                source
                    .terms()
                    .keys()
                    .any(|s| -i128::from(s.values()[position]) == expected)
            }));
        }
    }
}
