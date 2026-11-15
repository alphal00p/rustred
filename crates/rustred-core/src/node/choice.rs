//! The choice among ADMISSIBLE candidate relations of one key (seed minimisation's centre scan, the pole-diagnostics
//! preference; interface coord-seed-minimise.md section 6, USER RULING 2026-11-15 = pole-diagnostics.md addendum 7:
//! "Make this parametrizeable by user, by default 'regular at d=4' always wins").
//!
//! Order: (1) a candidate regular at d0 beats a singular one, subject to `max_size_ratio` (DEFAULT unbounded: regular
//! always wins; a finite ratio R lets a regular candidate win only up to R x the smallest admissible one);
//! (2) fewest RHS terms; (3) the earlier candidate. Only admissible candidates enter (the caller filters by descent,
//! domain and proof), so the choice never changes whether a key is served (a closure verdict).

/// `max_size_ratio`: a rational >= 1, or unbounded (the default).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum SizeRatio {
    #[default]
    Unbounded,
    /// num / den >= 1
    Ratio { num: u64, den: u64 },
}

impl SizeRatio {
    pub fn parse(s: &str) -> Result<Self, String> {
        if s == "unbounded" {
            return Ok(Self::Unbounded);
        }
        let (n, d) = match s.split_once('/') {
            Some((n, d)) => (n, d),
            None => (s, "1"),
        };
        let num: u64 = n.trim().parse().map_err(|_| format!("max_size_ratio {s:?}"))?;
        let den: u64 = d.trim().parse().map_err(|_| format!("max_size_ratio {s:?}"))?;
        if den == 0 || num < den {
            return Err(format!("max_size_ratio {s:?} must be a rational >= 1 or \"unbounded\""));
        }
        Ok(Self::Ratio { num, den })
    }

    /// Whether `size` is within the ratio of `smallest`.
    fn admits(&self, size: u64, smallest: u64) -> bool {
        match self {
            Self::Unbounded => true,
            Self::Ratio { num, den } => u128::from(size) * u128::from(*den) <= u128::from(smallest) * u128::from(*num),
        }
    }
}

/// One admissible candidate.
#[derive(Clone, Debug)]
pub struct Candidate {
    pub rhs_terms: u64,
    pub regular_at_d0: bool,
}

/// Why the chosen candidate won.
#[derive(Clone, Debug, PartialEq)]
pub enum Reason {
    /// a regular candidate won (within the ratio, or unbounded)
    Regular,
    /// no regular candidate: the smallest won
    Smallest,
    /// a regular candidate existed but exceeded the ratio: the smaller singular one won (`ratio` = regular / smallest)
    SizeGuard { regular_index: usize, ratio: f64 },
}

/// The chosen candidate's index and the reason; None for an empty list.
pub fn choose(candidates: &[Candidate], max_size_ratio: SizeRatio) -> Option<(usize, Reason)> {
    let smallest = candidates.iter().map(|c| c.rhs_terms).min()?;
    let best_regular = candidates
        .iter()
        .enumerate()
        .filter(|(_, c)| c.regular_at_d0)
        .min_by_key(|(i, c)| (c.rhs_terms, *i));
    let best_any = candidates
        .iter()
        .enumerate()
        .min_by_key(|(i, c)| (c.rhs_terms, *i))
        .map(|(i, _)| i)?;
    match best_regular {
        Some((i, c)) if max_size_ratio.admits(c.rhs_terms, smallest) => Some((i, Reason::Regular)),
        Some((i, c)) => Some((
            best_any,
            Reason::SizeGuard {
                regular_index: i,
                ratio: c.rhs_terms as f64 / smallest.max(1) as f64,
            },
        )),
        None => Some((best_any, Reason::Smallest)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn regular_always_wins_by_default_and_loses_to_a_finite_ratio() {
        let c = vec![
            Candidate { rhs_terms: 100, regular_at_d0: false },
            Candidate { rhs_terms: 10_000, regular_at_d0: true }, // 100x larger, regular
        ];
        // default (unbounded): the regular one wins whatever its size
        assert_eq!(choose(&c, SizeRatio::default()), Some((1, Reason::Regular)));
        assert_eq!(SizeRatio::parse("unbounded").unwrap(), SizeRatio::Unbounded);
        // max_size_ratio = 8: the regular one is 100x the smallest -> the singular one wins, recorded as size guard
        match choose(&c, SizeRatio::parse("8").unwrap()) {
            Some((0, Reason::SizeGuard { regular_index: 1, ratio })) => assert!((ratio - 100.0).abs() < 1e-9),
            other => panic!("{other:?}"),
        }
        // within the ratio the regular one wins
        let c2 = vec![
            Candidate { rhs_terms: 100, regular_at_d0: false },
            Candidate { rhs_terms: 700, regular_at_d0: true },
        ];
        assert_eq!(choose(&c2, SizeRatio::parse("8").unwrap()), Some((1, Reason::Regular)));
        // no regular candidate: the smallest; ties by position
        let c3 = vec![
            Candidate { rhs_terms: 5, regular_at_d0: false },
            Candidate { rhs_terms: 5, regular_at_d0: false },
        ];
        assert_eq!(choose(&c3, SizeRatio::default()), Some((0, Reason::Smallest)));
        assert!(SizeRatio::parse("1/2").is_err() && SizeRatio::parse("x").is_err() && SizeRatio::parse("3/2").is_ok());
        assert_eq!(choose(&[], SizeRatio::default()), None);
    }
}
