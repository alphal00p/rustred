//! ENGINE FILE: FOR THE ENGINE LANE'S GATE (written by lane gencheck, round 7 K5; branch engine/node-choice-rule21).
//!
//! The choice among ADMISSIBLE candidate relations of one key (seed minimisation's centre scan, the pole-diagnostics
//! preference; interface coord-seed-minimise.md section 6).
//!
//! USER RULING 2026-11-19 (WS/ops/LANE_RULES.md rule 21; WS/plan/auto-campaign/implementer_ops/RULING-20261119-D4-TIEBREAK.md)
//! REPLACES the ruling of 2026-11-15 ("by default 'regular at d=4' always wins"): d = 4 regularity is a TIE-BREAK ONLY.
//! DEFAULT order ([`Preference::EconomyFirst`]):
//! (1) lower downstream single-core COST, when every candidate carries one (measured ops / record seconds; a candidate
//!     without a cost ranks after those with one);
//! (2) fewest RHS terms (size);
//! (3) regular at d0 before singular (the tie-break);
//! (4) the earlier candidate.
//! [`Preference::RegularFirst`] keeps the old order (regular first, subject to `max_size_ratio`) as an EXPLICIT, NON-DEFAULT
//! option; every reason it returns says it was explicit. Only admissible candidates enter (the caller filters by descent,
//! domain and proof), so the choice never changes whether a key is served (a closure verdict).

/// `max_size_ratio`: a rational >= 1, or unbounded (used by the explicit [`Preference::RegularFirst`] only).
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
        let num: u64 = n
            .trim()
            .parse()
            .map_err(|_| format!("max_size_ratio {s:?}"))?;
        let den: u64 = d
            .trim()
            .parse()
            .map_err(|_| format!("max_size_ratio {s:?}"))?;
        if den == 0 || num < den {
            return Err(format!(
                "max_size_ratio {s:?} must be a rational >= 1 or \"unbounded\""
            ));
        }
        Ok(Self::Ratio { num, den })
    }

    /// Whether `size` is within the ratio of `smallest`.
    fn admits(&self, size: u64, smallest: u64) -> bool {
        match self {
            Self::Unbounded => true,
            Self::Ratio { num, den } => {
                u128::from(size) * u128::from(*den) <= u128::from(smallest) * u128::from(*num)
            }
        }
    }
}

/// The order of the choice.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum Preference {
    /// THE DEFAULT (rule 21): cost, then size, then regularity (tie-break), then position.
    #[default]
    EconomyFirst,
    /// EXPLICIT, NON-DEFAULT (the withdrawn 2026-11-15 order): regular first within `max_size_ratio`, then size.
    RegularFirst { max_size_ratio: SizeRatio },
}

/// One admissible candidate.
#[derive(Clone, Debug)]
pub struct Candidate {
    pub rhs_terms: u64,
    pub regular_at_d0: bool,
    /// downstream single-core cost (measured ops or record seconds, any one unit for all candidates), when known
    pub cost: Option<u64>,
}

/// Why the chosen candidate won.
#[derive(Clone, Debug, PartialEq)]
pub enum Reason {
    /// (default) the lowest downstream cost decided
    Cheapest,
    /// (default) equal cost or no cost: the fewest RHS terms decided
    Smallest,
    /// (default) equal cost and size: the regular candidate won the TIE-BREAK
    RegularTieBreak,
    /// (default) everything equal: the earlier candidate
    Position,
    /// (explicit regular_first) a regular candidate won (within the ratio, or unbounded)
    ExplicitRegularFirst,
    /// (explicit regular_first) no regular candidate: the smallest won
    ExplicitSmallest,
    /// (explicit regular_first) a regular candidate existed but exceeded the ratio: the smaller singular one won
    ExplicitSizeGuard { regular_index: usize, ratio: f64 },
}

impl Reason {
    /// Whether the choice was made under the explicit, non-default `regular_first` order.
    pub fn explicit_regular_first(&self) -> bool {
        matches!(
            self,
            Self::ExplicitRegularFirst | Self::ExplicitSmallest | Self::ExplicitSizeGuard { .. }
        )
    }
}

/// The chosen candidate's index and the reason; None for an empty list.
pub fn choose(candidates: &[Candidate], pref: Preference) -> Option<(usize, Reason)> {
    if candidates.is_empty() {
        return None;
    }
    match pref {
        Preference::EconomyFirst => {
            // the full key, lexicographic: cost (None after Some), size, singular after regular, position
            let key = |i: usize| {
                let c = &candidates[i];
                (
                    c.cost.is_none(),
                    c.cost.unwrap_or(u64::MAX),
                    c.rhs_terms,
                    !c.regular_at_d0,
                    i,
                )
            };
            let mut order: Vec<usize> = (0..candidates.len()).collect();
            order.sort_by_key(|&i| key(i));
            let (b, r) = (order[0], order.get(1).copied());
            let reason = match r {
                None => Reason::Position,
                Some(r) => {
                    let (kb, kr) = (key(b), key(r));
                    if (kb.0, kb.1) != (kr.0, kr.1) {
                        Reason::Cheapest
                    } else if kb.2 != kr.2 {
                        Reason::Smallest
                    } else if kb.3 != kr.3 {
                        Reason::RegularTieBreak
                    } else {
                        Reason::Position
                    }
                }
            };
            Some((b, reason))
        }
        Preference::RegularFirst { max_size_ratio } => {
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
                Some((i, c)) if max_size_ratio.admits(c.rhs_terms, smallest) => {
                    Some((i, Reason::ExplicitRegularFirst))
                }
                Some((i, c)) => Some((
                    best_any,
                    Reason::ExplicitSizeGuard {
                        regular_index: i,
                        ratio: c.rhs_terms as f64 / smallest.max(1) as f64,
                    },
                )),
                None => Some((best_any, Reason::ExplicitSmallest)),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn c(rhs_terms: u64, regular_at_d0: bool, cost: Option<u64>) -> Candidate {
        Candidate {
            rhs_terms,
            regular_at_d0,
            cost,
        }
    }

    #[test]
    fn by_default_a_cheaper_singular_candidate_wins_and_regularity_only_breaks_ties() {
        // size: the smaller singular candidate beats the 100x larger regular one
        let s = vec![c(100, false, None), c(10_000, true, None)];
        assert_eq!(
            choose(&s, Preference::default()),
            Some((0, Reason::Smallest))
        );
        // cost first: the cheaper singular one wins even when it is larger
        let k = vec![c(500, false, Some(10)), c(100, true, Some(90))];
        assert_eq!(
            choose(&k, Preference::default()),
            Some((0, Reason::Cheapest))
        );
        // a candidate with a measured cost ranks before one without
        let m = vec![c(10, true, None), c(500, false, Some(1))];
        assert_eq!(
            choose(&m, Preference::default()),
            Some((1, Reason::Cheapest))
        );
        // equal cost and size: the regular one wins the tie-break
        let t = vec![c(100, false, Some(7)), c(100, true, Some(7))];
        assert_eq!(
            choose(&t, Preference::default()),
            Some((1, Reason::RegularTieBreak))
        );
        let t2 = vec![c(100, false, None), c(100, true, None)];
        assert_eq!(
            choose(&t2, Preference::default()),
            Some((1, Reason::RegularTieBreak))
        );
        // everything equal: the earlier one
        let e = vec![c(5, false, None), c(5, false, None)];
        assert_eq!(
            choose(&e, Preference::default()),
            Some((0, Reason::Position))
        );
        assert_eq!(choose(&[], Preference::default()), None);
        assert!(!Reason::Smallest.explicit_regular_first());
    }

    #[test]
    fn regular_first_is_an_explicit_option_that_reports_itself() {
        let c1 = vec![c(100, false, None), c(10_000, true, None)];
        let unb = Preference::RegularFirst {
            max_size_ratio: SizeRatio::default(),
        };
        assert_eq!(choose(&c1, unb), Some((1, Reason::ExplicitRegularFirst)));
        assert!(Reason::ExplicitRegularFirst.explicit_regular_first());
        assert_eq!(SizeRatio::parse("unbounded").unwrap(), SizeRatio::Unbounded);
        // max_size_ratio = 8: the regular one is 100x the smallest -> the singular one wins, recorded as size guard
        match choose(
            &c1,
            Preference::RegularFirst {
                max_size_ratio: SizeRatio::parse("8").unwrap(),
            },
        ) {
            Some((
                0,
                Reason::ExplicitSizeGuard {
                    regular_index: 1,
                    ratio,
                },
            )) => assert!((ratio - 100.0).abs() < 1e-9),
            other => panic!("{other:?}"),
        }
        let c3 = vec![c(5, false, None), c(5, false, None)];
        assert_eq!(choose(&c3, unb), Some((0, Reason::ExplicitSmallest)));
        assert!(
            SizeRatio::parse("1/2").is_err()
                && SizeRatio::parse("x").is_err()
                && SizeRatio::parse("3/2").is_ok()
        );
    }
}
