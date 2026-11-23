//! Byte accounting of the two memo families of a p reduction (lane
//! bytebound): the interior kernel memos of the lane programs
//! ([`super::kernel::Memo`]: numerator tables, bubble ratios, carpet weights)
//! and the per-thread step memo ([`super::step_memo`]).
//!
//! Every insertion is charged at the insert, from the capacities and element
//! sizes of what it allocates (no allocator call): `capacity * size_of` per
//! allocation plus [`ALLOC_OVERHEAD`] per allocation, the heap of the
//! multi-precision integers from their significant bits, and the growth of
//! the hash tables from their bucket layout. Every removal is discharged with
//! the bytes that were charged for it, so the counters return to zero when
//! the memos are dropped. The charge is deterministic and independent of the
//! host's allocator (vakint and gammaloop hosts link RustRed with their own).
//!
//! The counters change no value and no output: nothing is printed unless
//! `RUSTRED_P_CACHE_REPORT=1` (one line at the end of each forward pass).
//!
//! Families:
//! - kernel memos: entries (key, slot, value) in [`KERNEL_BYTES`]; the
//!   numerator tables are counted once, while they live, in [`TABLES_LIVE`]
//!   (a table can be held by a kernel memo, by step memo entries and by a
//!   running step at the same time); the part held by kernel memos is
//!   [`KERNEL_TABLE_BYTES`];
//! - the per-thread step memo (one-thread requests): [`STEP_BYTES`], the sum
//!   over threads; each thread's share is reset to zero with its memo state;
//! - the shared step memo of multi-thread requests: [`SHARED_STEP_BYTES`]
//!   (counted, approximate: its sharded tables' growth is not charged).
use std::sync::atomic::{AtomicI64, AtomicU64, Ordering::Relaxed};

use symbolica::prelude::{Integer, Rational};

use crate::algebra::{Coefficient, CoefficientPolynomial};

/// Bytes charged per heap allocation on top of its requested size: the
/// allocator's bookkeeping and size-class rounding, as an average.
pub const ALLOC_OVERHEAD: u64 = 16;

/// Heap bytes and allocations of a value (what [`Size::charge`] turns into a
/// byte count).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Size {
    pub bytes: u64,
    pub allocations: u64,
}

impl Size {
    pub const ZERO: Self = Self {
        bytes: 0,
        allocations: 0,
    };

    /// One allocation of `bytes` (none for zero bytes).
    pub fn block(bytes: usize) -> Self {
        if bytes == 0 {
            Self::ZERO
        } else {
            Self {
                bytes: bytes as u64,
                allocations: 1,
            }
        }
    }

    /// The heap block of a `Vec<T>` (or a boxed slice) of `capacity`.
    pub fn vec<T>(capacity: usize) -> Self {
        Self::block(capacity.saturating_mul(std::mem::size_of::<T>()))
    }

    /// The table of a `std::collections::HashMap<K, V>` of `capacity`
    /// (hashbrown: buckets of `(K, V)`, one control byte per bucket plus one
    /// group, data padded to the group alignment).
    pub fn map<K, V>(capacity: usize) -> Self {
        if capacity == 0 {
            return Self::ZERO;
        }
        let buckets = if capacity < 4 {
            4
        } else if capacity < 8 {
            8
        } else {
            (capacity * 8 / 7).next_power_of_two()
        };
        const GROUP: usize = 16;
        let data = (buckets * std::mem::size_of::<(K, V)>()).next_multiple_of(GROUP);
        Self::block(data + buckets + GROUP)
    }

    /// The charge in bytes.
    pub fn charge(self) -> u64 {
        self.bytes + self.allocations * ALLOC_OVERHEAD
    }
}

impl std::ops::Add for Size {
    type Output = Self;
    fn add(self, other: Self) -> Self {
        Self {
            bytes: self.bytes + other.bytes,
            allocations: self.allocations + other.allocations,
        }
    }
}

impl std::ops::AddAssign for Size {
    fn add_assign(&mut self, other: Self) {
        *self = *self + other;
    }
}

impl std::ops::Sub for Size {
    type Output = Self;
    fn sub(self, other: Self) -> Self {
        Self {
            bytes: self.bytes.wrapping_sub(other.bytes),
            allocations: self.allocations.wrapping_sub(other.allocations),
        }
    }
}

/// Heap of an integer: the limbs of a multi-precision value.
pub fn integer_heap(value: &Integer) -> Size {
    match value {
        Integer::Large(_) => Size::block(value.significant_bits().div_ceil(64) as usize * 8),
        _ => Size::ZERO,
    }
}

/// Heap of a rational number.
pub fn rational_heap(value: &Rational) -> Size {
    integer_heap(value.numerator_ref()) + integer_heap(value.denominator_ref())
}

/// Heap of a polynomial (its context is shared and counts nothing).
pub fn polynomial_heap(polynomial: &CoefficientPolynomial) -> Size {
    let mut size = Size::vec::<Integer>(polynomial.coefficients.capacity())
        + Size::vec::<u16>(polynomial.exponents.capacity());
    for coefficient in &polynomial.coefficients {
        size += integer_heap(coefficient);
    }
    size
}

/// Heap of a coefficient of `Q(d)`.
pub fn coefficient_heap(coefficient: &Coefficient) -> Size {
    polynomial_heap(&coefficient.numerator) + polynomial_heap(&coefficient.denominator)
}

/// Kernel memo entries (keys, slots, values; tables excluded), bytes.
pub(crate) static KERNEL_BYTES: AtomicI64 = AtomicI64::new(0);
/// Kernel memo entries.
pub(crate) static KERNEL_ENTRIES: AtomicI64 = AtomicI64::new(0);
/// Bytes of the numerator tables held by kernel memos (part of [`TABLES_LIVE`]).
pub(crate) static KERNEL_TABLE_BYTES: AtomicI64 = AtomicI64::new(0);
/// Bytes of every numerator table alive in the process.
pub(crate) static TABLES_LIVE: AtomicI64 = AtomicI64::new(0);
/// Per-thread step memos, summed over threads.
pub(crate) static STEP_BYTES: AtomicI64 = AtomicI64::new(0);
/// The shared step memo of multi-thread requests (not covered by a budget).
pub(crate) static SHARED_STEP_BYTES: AtomicI64 = AtomicI64::new(0);
/// Highest covered total seen at a charge.
static PEAK_COVERED: AtomicU64 = AtomicU64::new(0);

/// Kernel memo entries + live tables + per-thread step memos: the bytes a
/// budget covers.
pub(crate) fn covered() -> u64 {
    let total = KERNEL_BYTES.load(Relaxed) + TABLES_LIVE.load(Relaxed) + STEP_BYTES.load(Relaxed);
    total.max(0) as u64
}

/// Account `delta` bytes on `counter` (negative: a discharge).
#[inline]
pub(crate) fn account(counter: &AtomicI64, delta: i64) {
    if delta == 0 {
        return;
    }
    counter.fetch_add(delta, Relaxed);
    if delta > 0 {
        PEAK_COVERED.fetch_max(covered(), Relaxed);
    }
}

/// A snapshot of the counters (bytes unless named otherwise).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct CacheStats {
    pub kernel_bytes: i64,
    pub kernel_entries: i64,
    pub kernel_table_bytes: i64,
    pub tables_live: i64,
    pub step_bytes: i64,
    pub shared_step_bytes: i64,
    pub covered: u64,
    pub peak_covered: u64,
}

/// The counters now.
pub fn stats() -> CacheStats {
    CacheStats {
        kernel_bytes: KERNEL_BYTES.load(Relaxed),
        kernel_entries: KERNEL_ENTRIES.load(Relaxed),
        kernel_table_bytes: KERNEL_TABLE_BYTES.load(Relaxed),
        tables_live: TABLES_LIVE.load(Relaxed),
        step_bytes: STEP_BYTES.load(Relaxed),
        shared_step_bytes: SHARED_STEP_BYTES.load(Relaxed),
        covered: covered(),
        peak_covered: PEAK_COVERED.load(Relaxed),
    }
}

/// Bytes of the calling thread's step memo.
pub fn thread_step_bytes() -> u64 {
    super::step_memo::local_bytes()
}

/// `RUSTRED_P_CACHE_REPORT=1`: one report line per forward pass.
pub(crate) fn report_requested() -> bool {
    static ON: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *ON.get_or_init(|| {
        matches!(
            std::env::var("RUSTRED_P_CACHE_REPORT").as_deref(),
            Ok("1" | "on" | "true")
        )
    })
}

fn gb(bytes: i64) -> String {
    format!("{:.3}", bytes as f64 / 1e9)
}

/// The report line (GB = 1e9 B).
pub fn report_line() -> String {
    let s = stats();
    let pinned = s.tables_live - s.kernel_table_bytes;
    format!(
        "rustred cache budget: covered {} GB (peak {}) = kernel memos {} GB ({} entries) + tables {} GB \
         (in kernel memos {}, pinned elsewhere {}) + step memo {} GB | not covered: shared step memo {} GB, \
         prepared weights {} entries",
        gb(s.covered as i64),
        gb(s.peak_covered as i64),
        gb(s.kernel_bytes),
        s.kernel_entries,
        gb(s.tables_live),
        gb(s.kernel_table_bytes),
        gb(pinned),
        gb(s.step_bytes),
        gb(s.shared_step_bytes),
        super::insertion::prepared_entries(),
    )
}

/// The end of a forward pass: the report line when asked for.
pub(crate) fn forward_finished() {
    if report_requested() {
        eprintln!("{}", report_line());
    }
}

/// Tests: a global allocator that counts, per thread, the net bytes and the
/// net number of allocations, so the charges can be compared with what was
/// really allocated.
#[cfg(test)]
pub(crate) mod alloc_count {
    use std::alloc::{GlobalAlloc, Layout, System};
    use std::cell::Cell;

    pub(crate) struct Counting;

    thread_local! {
        static NET: Cell<(i64, i64)> = const { Cell::new((0, 0)) };
    }

    fn note(bytes: i64, allocations: i64) {
        let _ = NET.try_with(|net| {
            let (b, a) = net.get();
            net.set((b + bytes, a + allocations));
        });
    }

    unsafe impl GlobalAlloc for Counting {
        unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
            note(layout.size() as i64, 1);
            unsafe { System.alloc(layout) }
        }
        unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
            note(layout.size() as i64, 1);
            unsafe { System.alloc_zeroed(layout) }
        }
        unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
            note(-(layout.size() as i64), -1);
            unsafe { System.dealloc(ptr, layout) }
        }
        unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
            note(new_size as i64 - layout.size() as i64, 0);
            unsafe { System.realloc(ptr, layout, new_size) }
        }
    }

    #[global_allocator]
    static COUNTING: Counting = Counting;

    /// Net (bytes, allocations) of this thread so far.
    pub(crate) fn net() -> (i64, i64) {
        NET.with(Cell::get)
    }

    /// The charge of a net (bytes, allocations) difference.
    pub(crate) fn charge(before: (i64, i64), after: (i64, i64)) -> i64 {
        (after.0 - before.0) + (after.1 - before.1) * super::ALLOC_OVERHEAD as i64
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use super::alloc_count::{charge, net};
    use super::*;
    use crate::algebra::{CoefficientContext, ExactAlgebraLimits};
    use crate::propagator::kernel::{Memo, NumeratorTable, read_memo, write_memo};

    /// A small deterministic generator (xorshift).
    pub(crate) struct Rng(pub u64);
    impl Rng {
        pub(crate) fn next(&mut self) -> u64 {
            self.0 ^= self.0 << 13;
            self.0 ^= self.0 >> 7;
            self.0 ^= self.0 << 17;
            self.0
        }
    }

    /// A coefficient of `Q(d)` with `terms` terms and integers of about
    /// `limbs` extra words (multi-precision from 2 on).
    pub(crate) fn coefficient(
        context: &CoefficientContext,
        rng: &mut Rng,
        terms: u64,
        limbs: u32,
    ) -> Coefficient {
        let limits = ExactAlgebraLimits::default();
        let d = context.parameter("d").unwrap();
        let mut numerator = context.integer(1);
        for term in 0..terms {
            let mut c = context.integer((rng.next() % 1000) as i64 + 1);
            for _ in 0..limbs {
                c = context
                    .try_mul(
                        &c,
                        &context.integer(i64::MAX - (rng.next() % 1000) as i64),
                        limits,
                    )
                    .unwrap();
            }
            let mut monomial = c;
            for _ in 0..term {
                monomial = context.try_mul(&monomial, &d, limits).unwrap();
            }
            numerator = context.try_add(&numerator, &monomial, limits).unwrap();
        }
        let denominator = context
            .try_add(&d, &context.integer((rng.next() % 7) as i64 + 3), limits)
            .unwrap();
        context.try_div(&numerator, &denominator, limits).unwrap()
    }

    fn memo_charged<K, V: crate::propagator::kernel::MemoValue>(memo: &Memo<K, V>) -> i64 {
        memo.charged() as i64
    }

    /// The charge of a kernel memo equals what its insertions allocated, after
    /// random insertions, replacements (racing builds) and growth; dropping it
    /// frees exactly what was charged.
    #[test]
    fn kernel_memo_charges_match_the_allocator() {
        let context = CoefficientContext::try_new(["d"]).unwrap();
        let mut rng = Rng(0x9e37_79b9_7f4a_7c15);
        let values: Vec<Coefficient> = (0..64)
            .map(|i| coefficient(&context, &mut rng, 1 + i % 6, (i % 2) as u32))
            .collect();
        let start = net();
        let memo: Memo<(i64, i64, i64, i64, i64, i64), Coefficient> = Memo::default();
        for round in 0..400_i64 {
            let key = ((rng.next() % 300) as i64, round % 3, 0, 0, 0, 0);
            let value = &values[(rng.next() % 64) as usize];
            write_memo(&memo, key, value, usize::MAX).unwrap();
            assert_eq!(
                charge(start, net()),
                memo_charged(&memo),
                "after insertion {round}"
            );
            if round % 37 == 0 {
                assert!(read_memo(&memo, &key).unwrap().is_some());
            }
        }
        let charged = memo_charged(&memo);
        assert!(charged > 0);
        drop(memo);
        assert_eq!(charge(start, net()), 0, "the memo frees what it charged");
    }

    /// Table memos: keys are boxed; the table bytes are counted once, in
    /// TABLES_LIVE, while the table lives, whoever holds it.
    #[test]
    fn table_bytes_match_the_allocator() {
        let context = CoefficientContext::try_new(["d"]).unwrap();
        let mut rng = Rng(7);
        let _ = coefficient(&context, &mut rng, 1, 0);
        for rows in [0usize, 1, 3, 17, 200] {
            let start = net();
            let table = Arc::new(NumeratorTable::for_test(
                (0..rows)
                    .map(|row| {
                        let orders: Vec<(u32, Rational)> = (0..(row % 5) as u32)
                            .map(|order| {
                                let mut value = Rational::from(rng.next() as i64 / 3 + 1);
                                for _ in 0..row % 2 {
                                    value = &value * &Rational::from(i64::MAX - 5);
                                }
                                (order, value)
                            })
                            .collect();
                        (vec![row as u32; row % 9], orders)
                    })
                    .collect(),
            ));
            // The temporaries of the test rows are freed: what is left is the table.
            assert_eq!(charge(start, net()), table.bytes() as i64, "{rows} rows");
            let memo: Memo<Box<[u32]>, Arc<NumeratorTable>> = Memo::default();
            let before = net();
            write_memo(
                &memo,
                vec![1, 2, rows as u32].into_boxed_slice(),
                &table,
                usize::MAX,
            )
            .unwrap();
            assert_eq!(charge(before, net()), memo_charged(&memo));
            drop(memo);
            drop(table);
            assert_eq!(charge(start, net()), 0);
        }
    }

    /// Multi-precision integers are charged their limbs (GMP allocates them
    /// outside the Rust allocator, so the allocator tests above use integers
    /// of at most two words).
    #[test]
    fn large_integers_are_charged_their_limbs() {
        let mut value = Integer::from(i64::MAX);
        for _ in 0..5 {
            value = &value * &Integer::from(i64::MAX);
        }
        assert!(matches!(value, Integer::Large(_)));
        let bits = value.significant_bits();
        assert_eq!(
            integer_heap(&value),
            Size::block(bits.div_ceil(64) as usize * 8)
        );
        assert_eq!(integer_heap(&Integer::from(5)), Size::ZERO);
    }

    /// `Size::map` follows the table layout of `std::collections::HashMap`.
    #[test]
    fn map_sizes_match_the_allocator() {
        let mut map: std::collections::HashMap<u64, [u8; 24]> = std::collections::HashMap::new();
        let start = net();
        for i in 0..5000_u64 {
            map.insert(i, [0; 24]);
            assert_eq!(
                charge(start, net()),
                Size::map::<u64, [u8; 24]>(map.capacity()).charge() as i64,
                "at {i} entries"
            );
        }
    }
}
