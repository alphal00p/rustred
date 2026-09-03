use super::error::{check_limit, checked_add, checked_mul, checked_sort_coordinate_work, try_vec};
use super::janet::{EpochId, JanetMultiplicativeMask};
use super::limits::InvolutiveWorkBudget;
use super::{ForwardShift, InvolutiveError, InvolutiveLimits};

const POSTING_WORD_BITS: usize = u64::BITS as usize;
const NO_POSTING: usize = usize::MAX;

/// Borrowed, coefficient-free geometry for one sealed Janet basis ordinal.
///
/// The view deliberately carries no [`super::OreConsequence`] handle.  Both
/// exact consequence epochs and future exact-support circuit epochs can feed
/// this same boundary without teaching the divisor index about coefficient
/// ownership.  Construction alone grants no trust: [`JanetDivisorIndex`]
/// checks canonical ordinals and both arities while building its postings.
#[derive(Clone, Copy, Debug)]
pub(super) struct JanetMonomialView<'a> {
    ordinal: usize,
    leading_shift: &'a ForwardShift,
    multiplicative: &'a JanetMultiplicativeMask,
}

impl<'a> JanetMonomialView<'a> {
    pub(super) const fn new(
        ordinal: usize,
        leading_shift: &'a ForwardShift,
        multiplicative: &'a JanetMultiplicativeMask,
    ) -> Self {
        Self {
            ordinal,
            leading_shift,
            multiplicative,
        }
    }

    const fn ordinal(self) -> usize {
        self.ordinal
    }

    const fn leading_shift(self) -> &'a ForwardShift {
        self.leading_shift
    }

    const fn multiplicative(self) -> &'a JanetMultiplicativeMask {
        self.multiplicative
    }
}

/// Immutable coordinate postings for exact Janet-divisor lookup.
///
/// A multiplicative coordinate admits every leader exponent at most the
/// target exponent, so each sorted threshold owns a cumulative ordinal
/// bitset. A nonmultiplicative coordinate admits only an equal exponent, so
/// each sorted value owns an exact ordinal bitset. Intersecting their union at
/// every coordinate is precisely Janet division; the first surviving bit is
/// therefore the same lowest ordinal selected by the historical flat scan.
#[derive(Debug, PartialEq, Eq)]
pub(super) struct JanetDivisorIndex {
    epoch: EpochId,
    arity: usize,
    element_count: usize,
    word_count: usize,
    coordinates: Box<[CoordinatePostings]>,
    retained_bytes: usize,
}

#[derive(Debug, PartialEq, Eq)]
struct CoordinatePostings {
    multiplicative: PostingTable,
    nonmultiplicative: PostingTable,
}

#[derive(Debug, PartialEq, Eq)]
struct PostingTable {
    values: Box<[u64]>,
    words: Box<[u64]>,
}

/// Reusable, epoch-tagged query storage. One instance is allocated for an
/// entire normal form, never once per subject term.
#[derive(Debug, PartialEq, Eq)]
pub(super) struct JanetDivisorScratch {
    epoch: EpochId,
    candidates: Vec<u64>,
    multiplicative_postings: Vec<usize>,
    nonmultiplicative_postings: Vec<usize>,
    retained_bytes: usize,
}

impl JanetDivisorIndex {
    /// Seal coordinate postings from a replayable coefficient-free geometry
    /// owner.
    ///
    /// `monomials` must describe every ordinal in `0..element_count`.  A
    /// cloneable borrowed iterator, rather than an intermediate descriptor
    /// allocation, keeps the established exact epoch's retained bytes and work
    /// trajectory unchanged while replaying one immutable snapshot for every
    /// coordinate.
    pub(super) fn try_new_from_geometry<'a>(
        epoch: &EpochId,
        arity: usize,
        element_count: usize,
        monomials: impl ExactSizeIterator<Item = JanetMonomialView<'a>> + Clone,
        limits: InvolutiveLimits,
        work: &mut InvolutiveWorkBudget,
    ) -> Result<Self, InvolutiveError> {
        if monomials.len() != element_count {
            return Err(InvolutiveError::Invariant {
                detail: "Janet divisor index geometry omitted a basis ordinal",
            });
        }
        let word_count = element_count.div_ceil(POSTING_WORD_BITS);
        let build_scratch_bytes = checked_mul(
            "Janet divisor index build scratch bytes",
            checked_mul("Janet divisor index build scratch bytes", element_count, 2)?,
            std::mem::size_of::<(u64, usize)>(),
        )?;
        check_limit(
            "Janet divisor index build scratch bytes",
            build_scratch_bytes,
            limits.max_divisor_index_build_scratch_bytes,
        )?;
        let coordinate_bytes = checked_mul(
            "Janet divisor index retained bytes",
            arity,
            std::mem::size_of::<CoordinatePostings>(),
        )?;
        let mut retained_bytes = checked_add(
            "Janet divisor index retained bytes",
            std::mem::size_of::<Self>(),
            coordinate_bytes,
        )?;
        check_limit(
            "Janet divisor index retained bytes",
            retained_bytes,
            limits.max_divisor_index_retained_bytes,
        )?;

        let mut coordinates = try_vec("Janet divisor index coordinates", arity)?;
        for coordinate in 0..arity {
            // Authentication and mask construction are sealed before this
            // index boundary. These checks protect the index from an internal
            // stale/malformed epoch without repeating row authentication.
            work.charge_divisor_index_build_operations(element_count, limits)?;
            let mut multiplicative =
                try_vec("Janet divisor index multiplicative pairs", element_count)?;
            let mut nonmultiplicative =
                try_vec("Janet divisor index nonmultiplicative pairs", element_count)?;
            let mut observed = 0usize;
            for (ordinal, monomial) in monomials.clone().enumerate() {
                if ordinal >= element_count {
                    return Err(InvolutiveError::Invariant {
                        detail: "Janet divisor index geometry exceeded its sealed shape",
                    });
                }
                if monomial.ordinal() != ordinal {
                    return Err(InvolutiveError::Invariant {
                        detail: "Janet divisor index saw a noncanonical basis ordinal",
                    });
                }
                if monomial.leading_shift().arity() != arity {
                    return Err(InvolutiveError::WrongArity {
                        object: "Janet divisor index element",
                        expected: arity,
                        actual: monomial.leading_shift().arity(),
                    });
                }
                if monomial.multiplicative().bits().len() != arity {
                    return Err(InvolutiveError::WrongArity {
                        object: "Janet divisor index mask",
                        expected: arity,
                        actual: monomial.multiplicative().bits().len(),
                    });
                }
                let entry = (monomial.leading_shift().values()[coordinate], ordinal);
                if monomial.multiplicative().bits()[coordinate] {
                    multiplicative.push(entry);
                } else {
                    nonmultiplicative.push(entry);
                }
                observed += 1;
            }
            if observed != element_count {
                return Err(InvolutiveError::Invariant {
                    detail: "Janet divisor index geometry omitted a basis ordinal",
                });
            }

            sort_pairs(&mut multiplicative, limits, work)?;
            sort_pairs(&mut nonmultiplicative, limits, work)?;
            let multiplicative = try_build_postings(
                multiplicative,
                word_count,
                true,
                &mut retained_bytes,
                limits,
                work,
            )?;
            let nonmultiplicative = try_build_postings(
                nonmultiplicative,
                word_count,
                false,
                &mut retained_bytes,
                limits,
                work,
            )?;
            coordinates.push(CoordinatePostings {
                multiplicative,
                nonmultiplicative,
            });
        }

        Ok(Self {
            epoch: epoch.clone(),
            arity,
            element_count,
            word_count,
            coordinates: coordinates.into_boxed_slice(),
            retained_bytes,
        })
    }

    pub(super) fn retained_bytes(&self) -> usize {
        self.retained_bytes
    }

    pub(super) fn try_scratch(
        &self,
        limits: InvolutiveLimits,
    ) -> Result<JanetDivisorScratch, InvolutiveError> {
        let word_bytes = checked_mul(
            "Janet divisor index scratch bytes",
            self.word_count,
            std::mem::size_of::<u64>(),
        )?;
        let posting_cells = checked_mul("Janet divisor index scratch bytes", self.arity, 2)?;
        let posting_bytes = checked_mul(
            "Janet divisor index scratch bytes",
            posting_cells,
            std::mem::size_of::<usize>(),
        )?;
        let scratch_bytes = checked_add(
            "Janet divisor index scratch bytes",
            std::mem::size_of::<JanetDivisorScratch>(),
            checked_add(
                "Janet divisor index scratch bytes",
                word_bytes,
                posting_bytes,
            )?,
        )?;
        check_limit(
            "Janet divisor index scratch bytes",
            scratch_bytes,
            limits.max_divisor_index_scratch_bytes,
        )?;

        let mut candidates = try_vec("Janet divisor index candidate words", self.word_count)?;
        candidates.resize(self.word_count, 0);
        let mut multiplicative_postings =
            try_vec("Janet divisor index multiplicative selections", self.arity)?;
        multiplicative_postings.resize(self.arity, NO_POSTING);
        let mut nonmultiplicative_postings = try_vec(
            "Janet divisor index nonmultiplicative selections",
            self.arity,
        )?;
        nonmultiplicative_postings.resize(self.arity, NO_POSTING);
        Ok(JanetDivisorScratch {
            epoch: self.epoch.clone(),
            candidates,
            multiplicative_postings,
            nonmultiplicative_postings,
            retained_bytes: scratch_bytes,
        })
    }

    pub(super) fn try_first_divisor(
        &self,
        current_epoch: &EpochId,
        target: &ForwardShift,
        excluded_ordinal: Option<usize>,
        scratch: &mut JanetDivisorScratch,
        limits: InvolutiveLimits,
        work: &mut InvolutiveWorkBudget,
    ) -> Result<Option<usize>, InvolutiveError> {
        if &self.epoch != current_epoch {
            return Err(InvolutiveError::StaleEpoch {
                expected: current_epoch.clone(),
                actual: self.epoch.clone(),
            });
        }
        if scratch.epoch != self.epoch {
            return Err(InvolutiveError::StaleEpoch {
                expected: self.epoch.clone(),
                actual: scratch.epoch.clone(),
            });
        }
        if target.arity() != self.arity {
            return Err(InvolutiveError::WrongArity {
                object: "Janet divisibility target",
                expected: self.arity,
                actual: target.arity(),
            });
        }
        if excluded_ordinal.is_some_and(|ordinal| ordinal >= self.element_count) {
            return Err(InvolutiveError::InvalidProlongation {
                detail: "excluded Janet divisor is outside the current epoch",
            });
        }
        if scratch.candidates.len() != self.word_count
            || scratch.multiplicative_postings.len() != self.arity
            || scratch.nonmultiplicative_postings.len() != self.arity
        {
            return Err(InvolutiveError::Invariant {
                detail: "Janet divisor query scratch has a malformed sealed shape",
            });
        }

        let mut query_operations = self.arity;
        for (coordinate, (&target_value, postings)) in target
            .values()
            .iter()
            .zip(self.coordinates.iter())
            .enumerate()
        {
            let (multiplicative, comparisons) =
                upper_bound(&postings.multiplicative.values, target_value)?;
            query_operations = checked_add(
                "Janet divisor index query operations",
                query_operations,
                comparisons,
            )?;
            scratch.multiplicative_postings[coordinate] = multiplicative.unwrap_or(NO_POSTING);

            let (nonmultiplicative, comparisons) =
                exact_value(&postings.nonmultiplicative.values, target_value)?;
            query_operations = checked_add(
                "Janet divisor index query operations",
                query_operations,
                comparisons,
            )?;
            scratch.nonmultiplicative_postings[coordinate] =
                nonmultiplicative.unwrap_or(NO_POSTING);
        }
        let bitmap_operations = checked_mul(
            "Janet divisor index query operations",
            checked_add("Janet divisor index query operations", self.arity, 2)?,
            self.word_count,
        )?;
        query_operations = checked_add(
            "Janet divisor index query operations",
            query_operations,
            bitmap_operations,
        )?;
        if excluded_ordinal.is_some() {
            query_operations =
                checked_add("Janet divisor index query operations", query_operations, 1)?;
        }
        work.charge_divisor_index_query_operations(query_operations, limits)?;

        scratch.candidates.fill(u64::MAX);
        if let Some(last) = scratch.candidates.last_mut() {
            let retained_bits = self.element_count % POSTING_WORD_BITS;
            if retained_bits != 0 {
                *last = (1_u64 << retained_bits) - 1;
            }
        }
        for (coordinate, postings) in self.coordinates.iter().enumerate() {
            let multiplicative = scratch.multiplicative_postings[coordinate];
            let nonmultiplicative = scratch.nonmultiplicative_postings[coordinate];
            for word in 0..self.word_count {
                let multiplicative_word =
                    postings
                        .multiplicative
                        .try_word(multiplicative, self.word_count, word)?;
                let nonmultiplicative_word = postings.nonmultiplicative.try_word(
                    nonmultiplicative,
                    self.word_count,
                    word,
                )?;
                scratch.candidates[word] &= multiplicative_word | nonmultiplicative_word;
            }
        }
        if let Some(ordinal) = excluded_ordinal {
            scratch.candidates[ordinal / POSTING_WORD_BITS] &=
                !(1_u64 << (ordinal % POSTING_WORD_BITS));
        }
        for (word_ordinal, &word) in scratch.candidates.iter().enumerate() {
            if word != 0 {
                let ordinal = checked_add(
                    "Janet divisor ordinal",
                    checked_mul("Janet divisor ordinal", word_ordinal, POSTING_WORD_BITS)?,
                    word.trailing_zeros() as usize,
                )?;
                if ordinal >= self.element_count {
                    return Err(InvolutiveError::Invariant {
                        detail: "Janet divisor index selected padding outside the basis",
                    });
                }
                return Ok(Some(ordinal));
            }
        }
        Ok(None)
    }
}

impl JanetDivisorScratch {
    #[cfg(test)]
    pub(super) fn retained_bytes(&self) -> usize {
        self.retained_bytes
    }
}

impl PostingTable {
    fn try_word(
        &self,
        posting: usize,
        word_count: usize,
        word: usize,
    ) -> Result<u64, InvolutiveError> {
        if posting == NO_POSTING {
            return Ok(0);
        }
        if posting >= self.values.len() || word >= word_count {
            return Err(InvolutiveError::Invariant {
                detail: "Janet divisor index posting selection is outside its sealed shape",
            });
        }
        let offset = checked_add(
            "Janet divisor index posting offset",
            checked_mul("Janet divisor index posting offset", posting, word_count)?,
            word,
        )?;
        self.words
            .get(offset)
            .copied()
            .ok_or(InvolutiveError::Invariant {
                detail: "Janet divisor index posting payload is shorter than its sealed shape",
            })
    }
}

fn sort_pairs(
    pairs: &mut [(u64, usize)],
    limits: InvolutiveLimits,
    work: &mut InvolutiveWorkBudget,
) -> Result<(), InvolutiveError> {
    let sort_operations =
        checked_sort_coordinate_work("Janet divisor index build operations", pairs.len(), 1)?;
    work.charge_divisor_index_build_operations(sort_operations, limits)?;
    pairs.sort_unstable();
    Ok(())
}

fn try_build_postings(
    sorted: Vec<(u64, usize)>,
    word_count: usize,
    cumulative: bool,
    retained_bytes: &mut usize,
    limits: InvolutiveLimits,
    work: &mut InvolutiveWorkBudget,
) -> Result<PostingTable, InvolutiveError> {
    work.charge_divisor_index_build_operations(sorted.len(), limits)?;
    let mut group_count = 0usize;
    for (position, pair) in sorted.iter().enumerate() {
        if position == 0 || pair.0 != sorted[position - 1].0 {
            group_count = checked_add("Janet divisor index posting groups", group_count, 1)?;
        }
    }
    let posting_words = checked_mul("Janet divisor index posting words", group_count, word_count)?;
    let value_bytes = checked_mul(
        "Janet divisor index retained bytes",
        group_count,
        std::mem::size_of::<u64>(),
    )?;
    let word_bytes = checked_mul(
        "Janet divisor index retained bytes",
        posting_words,
        std::mem::size_of::<u64>(),
    )?;
    *retained_bytes = checked_add(
        "Janet divisor index retained bytes",
        *retained_bytes,
        checked_add(
            "Janet divisor index retained bytes",
            value_bytes,
            word_bytes,
        )?,
    )?;
    check_limit(
        "Janet divisor index retained bytes",
        *retained_bytes,
        limits.max_divisor_index_retained_bytes,
    )?;

    let copy_words = if cumulative {
        checked_mul(
            "Janet divisor index build operations",
            group_count.saturating_sub(1),
            word_count,
        )?
    } else {
        0
    };
    let bitmap_operations = checked_add(
        "Janet divisor index build operations",
        posting_words,
        checked_add(
            "Janet divisor index build operations",
            copy_words,
            sorted.len(),
        )?,
    )?;
    work.charge_divisor_index_build_operations(bitmap_operations, limits)?;

    let mut values = try_vec("Janet divisor index posting values", group_count)?;
    let mut words = try_vec("Janet divisor index posting words", posting_words)?;
    words.resize(posting_words, 0_u64);
    let mut start = 0usize;
    while start < sorted.len() {
        let value = sorted[start].0;
        let mut end = start + 1;
        while end < sorted.len() && sorted[end].0 == value {
            end += 1;
        }
        let posting = values.len();
        values.push(value);
        let offset = posting * word_count;
        if cumulative && posting != 0 {
            let previous = offset - word_count;
            for word in 0..word_count {
                words[offset + word] = words[previous + word];
            }
        }
        for &(_, ordinal) in &sorted[start..end] {
            words[offset + ordinal / POSTING_WORD_BITS] |= 1_u64 << (ordinal % POSTING_WORD_BITS);
        }
        start = end;
    }
    Ok(PostingTable {
        values: values.into_boxed_slice(),
        words: words.into_boxed_slice(),
    })
}

fn upper_bound(values: &[u64], target: u64) -> Result<(Option<usize>, usize), InvolutiveError> {
    let mut left = 0usize;
    let mut right = values.len();
    let mut comparisons = 0usize;
    while left < right {
        comparisons = checked_add("Janet divisor index query operations", comparisons, 1)?;
        let middle = left + (right - left) / 2;
        if values[middle] <= target {
            left = middle + 1;
        } else {
            right = middle;
        }
    }
    Ok((left.checked_sub(1), comparisons))
}

fn exact_value(values: &[u64], target: u64) -> Result<(Option<usize>, usize), InvolutiveError> {
    let mut left = 0usize;
    let mut right = values.len();
    let mut comparisons = 0usize;
    while left < right {
        comparisons = checked_add("Janet divisor index query operations", comparisons, 1)?;
        let middle = left + (right - left) / 2;
        match values[middle].cmp(&target) {
            std::cmp::Ordering::Less => left = middle + 1,
            std::cmp::Ordering::Greater => right = middle,
            std::cmp::Ordering::Equal => return Ok((Some(middle), comparisons)),
        }
    }
    Ok((None, comparisons))
}
