//! Conservative readability proof for this one writer-owned metadata shape.
//!
//! Pinned bincode 2.0.1 reserves a vector length (u64), then len*sizeof(T),
//! releases sizeof(T) before each element, and charges nested fields. Summing
//! ALL positive claims and ignoring every release bounds its peak budget.
//! This is not a decoder, format, validation capability, or caller authority.
//! A bound overflow/overshoot selects the ordinary bounded decode path.
use super::*;

fn sum(values: impl IntoIterator<Item = Option<usize>>) -> Option<usize> {
    values
        .into_iter()
        .try_fold(0usize, |total, value| total.checked_add(value?))
}
fn vector<T>(items: &[T], element: impl Fn(&T) -> Option<usize>) -> Option<usize> {
    let container = items.len().checked_mul(size_of::<T>())?;
    sum([Some(8), Some(container), sum(items.iter().map(element))])
}
fn digest(_: &Digest) -> Option<usize> {
    // Arrays claim their full footprint; adding their scalar claims too is
    // conservative even when bincode uses its optimized u8 array reader.
    Some(32 + 32)
}
fn execution(value: &RawExecutionBinding) -> Option<usize> {
    let RawExecutionBinding {
        physical_arity: _,
        program_identity,
    } = value;
    sum([Some(4), digest(program_identity)])
}
fn object(value: &ObjectRef) -> Option<usize> {
    let ObjectRef {
        digest: id,
        bytes: _,
    } = value;
    sum([digest(id), Some(8)])
}
fn key(value: &StoredWork) -> Option<usize> {
    // Exhaustive variants/fields intentionally make a schema addition require
    // updating this bounded writer optimization rather than silently skipping.
    let fields = match value {
        StoredWork::Route(powers) => vector(powers, |_| Some(8)),
        StoredWork::Apply { owner, powers } => {
            sum([vector(owner, |_| Some(1)), vector(powers, |_| Some(8))])
        }
    };
    sum([Some(4), fields]) // enum discriminant u32
}
fn work_page(value: &PageRef) -> Option<usize> {
    let PageRef {
        object: reference,
        records: _,
        first,
        last,
    } = value;
    sum([object(reference), Some(8), key(first), key(last)])
}
fn raw_page(value: &raw::RawPageRef) -> Option<usize> {
    let raw::RawPageRef {
        kind,
        object: reference,
        records: _,
        payload_bytes: _,
        first,
        last,
    } = value;
    match kind {
        RawKeyKind::Terminal | RawKeyKind::Zero => {}
    }
    sum([
        Some(4),
        object(reference),
        Some(8 + 8),
        vector(first, |_| Some(8)),
        vector(last, |_| Some(8)),
    ])
}
fn raw_directory(value: &RawCensusDirectory) -> Option<usize> {
    let RawCensusDirectory { terminals, zeros } = value;
    sum([
        vector(terminals, |run| vector(run, raw_page)),
        vector(zeros, |run| vector(run, raw_page)),
    ])
}

pub(crate) fn manifest_decode_charge_upper_bound(value: &Manifest) -> Option<usize> {
    let Manifest {
        execution: binding,
        generation: _,
        previous,
        state,
        scope_digest,
        runs,
        raw,
    } = value;
    sum([
        execution(binding),
        Some(8),
        Some(1),
        previous.as_ref().map_or(Some(0), digest),
        object(state),
        digest(scope_digest),
        vector(runs, |run| vector(run, work_page)),
        raw_directory(raw),
    ])
}

#[cfg(test)]
mod tests {
    use super::*;

    fn decode_at_budget(
        bytes: &[u8],
        budget: usize,
    ) -> std::result::Result<Manifest, bincode::error::DecodeError> {
        use bincode::de::Decoder;
        let mut decoder = bincode::de::DecoderImpl::new(
            bincode::de::read::SliceReader::new(bytes),
            bincode::config::standard().with_limit::<{ usize::MAX }>(),
            (),
        );
        decoder.claim_bytes_read(usize::MAX - budget)?;
        bincode::Decode::decode(&mut decoder)
    }

    fn manifest(arity: usize, runs: usize) -> Manifest {
        let page = |n: usize| PageRef {
            object: ObjectRef {
                digest: [n as u8; 32],
                bytes: 128,
            },
            records: 1,
            first: StoredWork::Route(vec![i64::MIN; arity]),
            last: StoredWork::Apply {
                owner: vec![true; arity],
                powers: vec![i64::MAX; arity],
            },
        };
        let raw = |kind| raw::RawPageRef {
            kind,
            object: ObjectRef {
                digest: [7; 32],
                bytes: 128,
            },
            records: 1,
            payload_bytes: 64,
            first: vec![i64::MIN; arity],
            last: vec![i64::MAX; arity],
        };
        Manifest {
            execution: RawExecutionBinding {
                physical_arity: arity as u32,
                program_identity: [9; 32],
            },
            generation: 2,
            previous: Some([3; 32]),
            state: ObjectRef {
                digest: [4; 32],
                bytes: 128,
            },
            scope_digest: [5; 32],
            runs: Arc::new((0..runs).map(|n| vec![page(n)]).collect()),
            raw: Arc::new(RawCensusDirectory {
                terminals: vec![vec![raw(RawKeyKind::Terminal)]],
                zeros: vec![vec![raw(RawKeyKind::Zero)]],
            }),
        }
    }

    #[test]
    fn claimed_upper_bound_dominates_pinned_bincode_nested_decode_budget() {
        for arity in [1, 2, 15, 16, 64, 256] {
            for runs in [0, 1, 3, 24] {
                for previous in [None, Some([3; 32])] {
                    let mut value = manifest(arity, runs);
                    value.previous = previous;
                    let bytes =
                        bincode::encode_to_vec(&value, bincode::config::standard()).unwrap();
                    let bound = manifest_decode_charge_upper_bound(&value).unwrap();
                    assert_eq!(
                        decode_at_budget(&bytes, bound).unwrap(),
                        value,
                        "arity={arity}, runs={runs}"
                    );
                    // Independent fixed lower budgets can refuse, but never
                    // make the conservative charge an under-estimate.
                    if decode_at_budget(&bytes, 1024).is_err() {
                        assert!(bound > 1024);
                    }
                }
            }
        }
    }

    #[test]
    fn compact_manifest_over_64_mib_decoder_charge_selects_fallback_not_false_authority() {
        let arity = MAX_FRAME / (2 * size_of::<i64>());
        let mut value = manifest(1, 0);
        value.execution.physical_arity = arity as u32;
        value.runs = Arc::new(vec![vec![PageRef {
            object: ObjectRef {
                digest: [6; 32],
                bytes: 128,
            },
            records: 1,
            first: StoredWork::Route(vec![0; arity]),
            last: StoredWork::Route(vec![0; arity]),
        }]]);
        value.raw = Arc::new(RawCensusDirectory::default());
        let bytes = bincode::encode_to_vec(&value, bincode::config::standard()).unwrap();
        assert!(bytes.len() < MAX_FRAME);
        let bound = manifest_decode_charge_upper_bound(&value).unwrap();
        assert!(bound > metadata_decode_allowance(MAX_FRAME));
        assert!(matches!(
            decode_at_budget(&bytes, MAX_FRAME),
            Err(bincode::error::DecodeError::LimitExceeded)
        ));
        assert_eq!(decode_at_budget(&bytes, bound).unwrap(), value);

        // The bound is intentionally conservative: an overshoot must select
        // the ordinary bounded decoder, not reject a frame that really fits.
        let page = &mut Arc::get_mut(&mut value.runs).unwrap()[0][0];
        for key in [&mut page.first, &mut page.last] {
            let StoredWork::Route(powers) = key else {
                unreachable!()
            };
            powers.truncate(arity * 3 / 4);
        }
        value.execution.physical_arity = (arity * 3 / 4) as u32;
        let bytes = bincode::encode_to_vec(&value, bincode::config::standard()).unwrap();
        assert!(manifest_decode_charge_upper_bound(&value).unwrap() > MAX_FRAME);
        assert_eq!(decode_at_budget(&bytes, MAX_FRAME).unwrap(), value);
    }
}
