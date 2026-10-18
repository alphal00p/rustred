//! Repeatable integer framing: byte identity and fail-closed size enforcement.
use bincode::{
    Encode,
    enc::{Encoder, write::Writer},
};
use rustred::persistence::finite_work::{
    GeneratedWorkErrorKind, decode_metadata_frame, encode_metadata_frame,
    encode_metadata_frame_repeatable,
};
use std::{cell::Cell, hint::black_box, time::Instant};

const MAGIC: &[u8] = b"RUSTRED-REPEATABLE-TEST\0";

#[test]
fn sized_frames_are_identical_and_obey_exact_frame_limit() {
    for length in [0, 1, 127, 251, 4096, 131072] {
        let value = (vec![vec![i64::MIN, 0, i64::MAX]; length], Some([11u8; 32]));
        let reference = encode_metadata_frame(MAGIC, &value, 16 * 1024 * 1024).unwrap();
        let actual = encode_metadata_frame_repeatable(MAGIC, &value, reference.len()).unwrap();
        assert_eq!(actual, reference);
        let decoded: (Vec<Vec<i64>>, Option<[u8; 32]>) =
            decode_metadata_frame(MAGIC, &actual, 16 * 1024 * 1024).unwrap();
        assert_eq!(decoded, value);
        for limit in [0, MAGIC.len(), MAGIC.len() + 31, reference.len() - 1] {
            assert_eq!(
                encode_metadata_frame_repeatable(MAGIC, &value, limit)
                    .unwrap_err()
                    .kind,
                GeneratedWorkErrorKind::Resource
            );
        }
        assert!(
            decode_metadata_frame::<(Vec<Vec<i64>>, Option<[u8; 32]>)>(
                MAGIC,
                &actual[..actual.len() - 1],
                16 * 1024 * 1024
            )
            .is_err()
        );
    }
    assert_eq!(
        encode_metadata_frame_repeatable(b"", &(), 32).unwrap(),
        encode_metadata_frame(b"", &(), 32).unwrap()
    );
}

struct ChangesLength {
    calls: Cell<usize>,
    longer: bool,
}
impl Encode for ChangesLength {
    fn encode<E: Encoder>(&self, encoder: &mut E) -> Result<(), bincode::error::EncodeError> {
        let first = self.calls.get() == 0;
        self.calls.set(self.calls.get() + 1);
        let value: &[u8] = if first == self.longer {
            &[1]
        } else {
            &[1, 2, 3]
        };
        value.encode(encoder)
    }
}
#[test]
fn stateful_length_changes_are_rejected_and_general_encoder_stays_single_pass() {
    for longer in [false, true] {
        let value = ChangesLength {
            calls: Cell::new(0),
            longer,
        };
        assert_eq!(
            encode_metadata_frame_repeatable(MAGIC, &value, 1024)
                .unwrap_err()
                .kind,
            GeneratedWorkErrorKind::Resource
        );
        assert_eq!(value.calls.get(), 2);
        value.calls.set(0);
        assert!(encode_metadata_frame(MAGIC, &value, 1024).is_ok());
        assert_eq!(value.calls.get(), 1);
    }
}

struct TooLarge {
    writes: Cell<usize>,
}
impl Encode for TooLarge {
    fn encode<E: Encoder>(&self, encoder: &mut E) -> Result<(), bincode::error::EncodeError> {
        for _ in 0..1000 {
            self.writes.set(self.writes.get() + 1);
            encoder.writer().write(&[0; 16])?;
        }
        Ok(())
    }
}
#[test]
fn bounded_count_stops_before_second_pass_or_full_payload_allocation() {
    let value = TooLarge {
        writes: Cell::new(0),
    };
    assert_eq!(
        encode_metadata_frame_repeatable(MAGIC, &value, MAGIC.len() + 32 + 31)
            .unwrap_err()
            .kind,
        GeneratedWorkErrorKind::Resource
    );
    assert_eq!(value.writes.get(), 2);
}

#[test]
#[ignore = "optimized explicit framing microbenchmark; not a campaign timing"]
fn bounded_repeatable_encoder_microbenchmark() {
    let rows = std::env::var("RUSTRED_METADATA_SIZED_ROWS")
        .ok()
        .map(|v| v.parse::<usize>().unwrap())
        .unwrap_or(200000);
    let value: Vec<_> = (0..rows)
        .map(|i| (i as u64, vec![i as i64; 15], vec![i as i64; 15], [7u8; 32]))
        .collect();
    let limit = 512 << 20;
    let expected = encode_metadata_frame(MAGIC, &value, limit).unwrap();
    for round in 0..4 {
        for sized in if round % 2 == 0 {
            [false, true]
        } else {
            [true, false]
        } {
            let start = Instant::now();
            let result = black_box(if sized {
                encode_metadata_frame_repeatable(MAGIC, black_box(&value), limit)
            } else {
                encode_metadata_frame(MAGIC, black_box(&value), limit)
            })
            .unwrap();
            let seconds = start.elapsed().as_secs_f64();
            assert_eq!(result, expected);
            println!(
                "{{\"benchmark\":\"bounded_repeatable_encoder\",\"round\":{round},\"sized\":{sized},\"rows\":{rows},\"bytes\":{},\"seconds\":{seconds}}}",
                result.len()
            );
        }
    }
}
