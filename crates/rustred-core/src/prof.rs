//! fwd-profile: a built-in phase timer of the p reduction (default off).
//!
//! With the cargo feature `fwd-profile` the hot paths of the ordered
//! reduction carry [`prof!`] scopes; without it every scope expands to
//! nothing (zero cost, the production build). With the feature compiled in,
//! the timer still does nothing until [`start`] is called (the probe calls it
//! when `RUSTRED_FWD_PROFILE` is set): every scope then costs one relaxed
//! load and a branch while stopped.
//!
//! Accounting is *exclusive*: each thread keeps a stack of open categories;
//! the time-stamp counter is read when a scope opens or closes, and the
//! elapsed ticks since the previous read are charged to the category on top
//! of the stack. The categories of one thread therefore add up exactly to the
//! time that thread spent inside its outermost scope (the probe opens a root
//! scope around the reduce call, so the main thread's categories add up to
//! the reduce wall time). Ticks are converted to seconds with the counter
//! rate measured between [`start`] and [`report`].
//!
//! Level 1 opens the per-step scopes only (coarse, the overhead budget) and
//! counts allocator calls; level 2 also opens the fine per-child scopes
//! ([`prof_fine!`]: frontier insert, interning, trace record, orbit
//! canonicalisation, key packing, zero pruning, step-local merge); level 3
//! also times every allocator call as the pseudo-category `alloc`.
//!
//! Nothing here changes a decision of the run: it only reads clocks.

/// Open an exclusive timer scope of category `$cat` until the end of the
/// enclosing block (nothing without the `fwd-profile` feature).
#[macro_export]
#[doc(hidden)]
macro_rules! prof {
    ($cat:ident) => {
        #[cfg(feature = "fwd-profile")]
        let _prof_scope = $crate::prof::Scope::enter($crate::prof::Cat::$cat);
    };
}

/// A fine scope (per child/edge, millions per request): open only at level
/// >= 2, a load and a branch at level 1.
#[macro_export]
#[doc(hidden)]
macro_rules! prof_fine {
    ($cat:ident) => {
        #[cfg(feature = "fwd-profile")]
        let _prof_scope = $crate::prof::Scope::enter_fine($crate::prof::Cat::$cat);
    };
}

/// `$body` inside a fine scope of category `$cat`.
#[macro_export]
#[doc(hidden)]
macro_rules! prof_wrap_fine {
    ($cat:ident, $body:expr) => {{
        $crate::prof_fine!($cat);
        $body
    }};
}

/// `$body` inside a scope of category `$cat`.
#[macro_export]
#[doc(hidden)]
macro_rules! prof_wrap {
    ($cat:ident, $body:expr) => {{
        $crate::prof!($cat);
        $body
    }};
}

macro_rules! categories {
    ($($name:ident => $label:literal,)*) => {
        /// Timer categories (`label` = `group.item`; groups map to the
        /// user's a-e: arith, fetch, canon, apply, combine, plus other).
        #[derive(Clone, Copy, Debug, PartialEq, Eq)]
        #[repr(u8)]
        #[allow(non_camel_case_types)]
        pub enum Cat { $($name,)* }
        /// The labels of [`Cat`], in order.
        pub const LABELS: &[&str] = &[$($label,)*];
    };
}

categories! {
    Root => "other.reduce-unattributed",
    Admit => "other.admit-inputs",
    Loop => "combine.loop-bookkeeping",
    Pop => "combine.frontier-pop",
    Child => "combine.frontier-insert-merge",
    Position => "combine.position-order-key",
    Intern => "combine.coefficient-intern",
    InternNew => "arith.probe-eval-new-coefficient",
    Record => "combine.trace-record",
    Terminal => "combine.terminal-merge",
    Source => "fetch.step-source",
    CacheLookup => "fetch.step-cache-lookup-decode",
    CacheRecord => "combine.step-cache-record",
    Wait => "other.wait-for-workers",
    Prestage => "combine.prestage-worker",
    RouteStep => "fetch.route-sector-lookup",
    RouteCompile => "fetch.route-compile-restore",
    MemoRoute => "fetch.memo-route-lookup",
    RouteTransport => "apply.route-transport-expansion",
    RouteChildren => "apply.route-child-emit",
    ApplyStep => "fetch.owner-lookup",
    OwnerPrepare => "fetch.owner-program-prepare",
    LaneInsertion => "apply.lane-insertion",
    LaneSeriesParallel => "apply.lane-series-parallel",
    LaneTriangle => "apply.lane-triangle",
    LaneCarpet => "apply.lane-carpet",
    LaneDiamond => "apply.lane-diamond",
    LaneScheme => "apply.lane-scheme",
    LaneCustom => "apply.lane-custom",
    LaneOther => "apply.lane-other",
    MemoInsertion => "fetch.memo-insertion-lookup",
    InsertionCompute => "apply.insertion-weights-compute",
    CustomDispatch => "fetch.custom-rule-select",
    CustomSpecialise => "apply.custom-rule-specialise",
    SchemeMatch => "fetch.scheme-rule-select",
    ChildAssembly => "apply.child-assembly",
    CanonApply => "canon.apply-key-orbit",
    CanonRoute => "canon.route-key-orbit",
    CanonKey => "canon.key-pack",
    CanonZero => "canon.zero-sector-prune",
    OrbitInit => "canon.orbit-init",
    StepMerge => "combine.step-local-merge",
    StepAdd => "arith.step-local-exact-add",
    FinPrepare => "arith.finish-prepare",
    FinFactor => "arith.factor-basis",
    FinTables => "arith.replay-tables-mod-p",
    FinReplay => "arith.replay",
    FinCompose => "arith.compose-images",
    FinLift => "arith.lift-crt-ratrec-verify",
    FinOutput => "arith.output-assembly",
    Alloc => "other.alloc",
}

/// Number of categories.
pub const COUNT: usize = LABELS.len();

/// Whether the timer was compiled in (`fwd-profile` feature).
pub const fn compiled() -> bool {
    cfg!(feature = "fwd-profile")
}

#[cfg(feature = "fwd-profile")]
pub use imp::Scope;
#[cfg(feature = "fwd-profile")]
pub use imp::{
    alloc_enter, alloc_event, alloc_exit, note_memo, phase_mark, report, reset, start, stop,
};

#[cfg(feature = "fwd-profile")]
mod imp {
    use super::{COUNT, Cat, LABELS};
    use std::cell::UnsafeCell;
    use std::sync::Mutex;
    use std::sync::atomic::{AtomicBool, AtomicU8, AtomicU64, Ordering::Relaxed};

    static ON: AtomicBool = AtomicBool::new(false);
    static LEVEL: AtomicU8 = AtomicU8::new(0);
    static MAIN: AtomicU64 = AtomicU64::new(0);
    static NEXT_ID: AtomicU64 = AtomicU64::new(1);

    struct ThreadAcc {
        id: u64,
        ticks: [AtomicU64; COUNT],
        counts: [AtomicU64; COUNT],
        /// alloc, dealloc, realloc, bytes
        allocs: [AtomicU64; 4],
    }

    impl ThreadAcc {
        fn bump(cell: &AtomicU64, by: u64) {
            // Only the owner thread writes: a plain load/store pair.
            cell.store(cell.load(Relaxed).wrapping_add(by), Relaxed);
        }
    }

    static REGISTRY: Mutex<Vec<&'static ThreadAcc>> = Mutex::new(Vec::new());
    /// Main-thread tick snapshots at phase boundaries: (phase, ticks).
    static PHASES: Mutex<Vec<(usize, Vec<u64>)>> = Mutex::new(Vec::new());
    static CLOCK: Mutex<Option<(std::time::Instant, u64)>> = Mutex::new(None);
    /// The p step memo's counters at the end of the last request.
    static MEMO: Mutex<Option<[u64; 7]>> = Mutex::new(None);

    /// Keep the step memo's counters (`step_memo::end`, before it clears).
    pub fn note_memo(counters: [u64; 7]) {
        *MEMO
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner) = Some(counters);
    }

    const DEPTH: usize = 64;

    struct Local {
        acc: *const ThreadAcc,
        stack: [u8; DEPTH],
        depth: usize,
        last: u64,
        busy: bool,
    }

    thread_local! {
        static LOCAL: UnsafeCell<Local> = const {
            UnsafeCell::new(Local {
                acc: std::ptr::null(),
                stack: [0; DEPTH],
                depth: 0,
                last: 0,
                busy: false,
            })
        };
    }

    #[inline(always)]
    fn tsc() -> u64 {
        #[cfg(target_arch = "x86_64")]
        #[allow(unused_unsafe)]
        // SAFETY: rdtsc has no preconditions on x86_64.
        unsafe {
            core::arch::x86_64::_rdtsc()
        }
        #[cfg(not(target_arch = "x86_64"))]
        {
            static T0: std::sync::OnceLock<std::time::Instant> = std::sync::OnceLock::new();
            T0.get_or_init(std::time::Instant::now).elapsed().as_nanos() as u64
        }
    }

    fn register() -> &'static ThreadAcc {
        let acc: &'static ThreadAcc = Box::leak(Box::new(ThreadAcc {
            id: NEXT_ID.fetch_add(1, Relaxed),
            ticks: std::array::from_fn(|_| AtomicU64::new(0)),
            counts: std::array::from_fn(|_| AtomicU64::new(0)),
            allocs: std::array::from_fn(|_| AtomicU64::new(0)),
        }));
        REGISTRY
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .push(acc);
        acc
    }

    /// This thread's accumulator (registered on first use, outside any
    /// borrow of the thread-local state: registration allocates).
    #[inline]
    fn acc() -> Option<*const ThreadAcc> {
        let current = LOCAL.try_with(|l| unsafe { (*l.get()).acc }).ok()?;
        if !current.is_null() {
            return Some(current);
        }
        let busy = LOCAL.try_with(|l| unsafe { (*l.get()).busy }).ok()?;
        if busy {
            return None;
        }
        LOCAL.with(|l| unsafe { (*l.get()).busy = true });
        let acc = register() as *const ThreadAcc;
        LOCAL.with(|l| unsafe {
            let l = &mut *l.get();
            l.acc = acc;
            l.busy = false;
        });
        Some(acc)
    }

    #[inline(always)]
    fn push(cat: u8) -> bool {
        let fast = LOCAL.with(|l| {
            // SAFETY: thread-local, no re-entrancy inside this block (no
            // allocation, no callback).
            let l = unsafe { &mut *l.get() };
            if l.acc.is_null() {
                return None;
            }
            if l.depth >= DEPTH || l.busy {
                return Some(false);
            }
            let now = tsc();
            let acc = unsafe { &*l.acc };
            if l.depth > 0 {
                ThreadAcc::bump(
                    &acc.ticks[l.stack[l.depth - 1] as usize],
                    now.wrapping_sub(l.last),
                );
            }
            l.stack[l.depth] = cat;
            l.depth += 1;
            ThreadAcc::bump(&acc.counts[cat as usize], 1);
            l.last = now;
            Some(true)
        });
        match fast {
            Some(entered) => entered,
            None => acc().is_some() && push(cat),
        }
    }

    #[inline(always)]
    fn pop() {
        LOCAL.with(|l| {
            // SAFETY: as in `push`.
            let l = unsafe { &mut *l.get() };
            if l.depth == 0 || l.acc.is_null() {
                return;
            }
            let now = tsc();
            let acc = unsafe { &*l.acc };
            ThreadAcc::bump(
                &acc.ticks[l.stack[l.depth - 1] as usize],
                now.wrapping_sub(l.last),
            );
            l.depth -= 1;
            l.last = now;
        });
    }

    /// An open scope (closes on drop).
    #[must_use]
    pub struct Scope(bool);

    impl Scope {
        #[inline(always)]
        pub fn enter(cat: Cat) -> Self {
            if !ON.load(Relaxed) {
                return Self(false);
            }
            Self(push(cat as u8))
        }

        /// A fine scope: open only at level >= 2.
        #[inline(always)]
        pub fn enter_fine(cat: Cat) -> Self {
            if !ON.load(Relaxed) || LEVEL.load(Relaxed) < 2 {
                return Self(false);
            }
            Self(push(cat as u8))
        }
    }

    impl Drop for Scope {
        #[inline(always)]
        fn drop(&mut self) {
            if self.0 {
                pop();
            }
        }
    }

    /// Allocator hook (level >= 1): count one call of `kind` (0 alloc,
    /// 1 dealloc, 2 realloc) of `bytes`. Never allocates.
    #[inline(always)]
    pub fn alloc_event(kind: usize, bytes: usize) {
        if !ON.load(Relaxed) {
            return;
        }
        let _ = LOCAL.try_with(|l| {
            let acc = unsafe { (*l.get()).acc };
            if acc.is_null() {
                return;
            }
            let acc = unsafe { &*acc };
            ThreadAcc::bump(&acc.allocs[kind], 1);
            if kind != 1 {
                ThreadAcc::bump(&acc.allocs[3], bytes as u64);
            }
        });
    }

    /// Allocator hook (level 2): time the call as `alloc`. Never allocates.
    #[inline(always)]
    pub fn alloc_enter() -> bool {
        if !ON.load(Relaxed) || LEVEL.load(Relaxed) < 3 {
            return false;
        }
        LOCAL
            .try_with(|l| {
                let l = unsafe { &mut *l.get() };
                if l.acc.is_null() || l.depth == 0 || l.depth >= DEPTH || l.busy {
                    return false;
                }
                let now = tsc();
                let acc = unsafe { &*l.acc };
                ThreadAcc::bump(
                    &acc.ticks[l.stack[l.depth - 1] as usize],
                    now.wrapping_sub(l.last),
                );
                l.stack[l.depth] = Cat::Alloc as u8;
                l.depth += 1;
                ThreadAcc::bump(&acc.counts[Cat::Alloc as usize], 1);
                l.last = now;
                true
            })
            .unwrap_or(false)
    }

    #[inline(always)]
    pub fn alloc_exit(entered: bool) {
        if entered {
            let _ = LOCAL.try_with(|l| {
                let l = unsafe { &mut *l.get() };
                let now = tsc();
                let acc = unsafe { &*l.acc };
                ThreadAcc::bump(
                    &acc.ticks[l.stack[l.depth - 1] as usize],
                    now.wrapping_sub(l.last),
                );
                l.depth -= 1;
                l.last = now;
            });
        }
    }

    /// Start the timer on this (main) thread at `level` (1 scopes and
    /// allocator counts, 2 also allocator time).
    pub fn start(level: u8) {
        let acc = acc().map_or(0, |a| unsafe { (*a).id });
        MAIN.store(acc, Relaxed);
        LEVEL.store(level.max(1), Relaxed);
        *CLOCK
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner) =
            Some((std::time::Instant::now(), tsc()));
        ON.store(true, Relaxed);
    }

    pub fn stop() {
        ON.store(false, Relaxed);
    }

    /// Zero every accumulator (call while no thread is inside a scope but
    /// the caller's root).
    pub fn reset() {
        for acc in REGISTRY
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .iter()
        {
            for c in acc.ticks.iter().chain(&acc.counts).chain(&acc.allocs) {
                c.store(0, Relaxed);
            }
        }
        PHASES
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .clear();
    }

    /// Main thread: snapshot its ticks at the end of forward phase `phase`.
    pub fn phase_mark(phase: usize) {
        if !ON.load(Relaxed) {
            return;
        }
        let Some(acc) = acc() else { return };
        let acc = unsafe { &*acc };
        if acc.id != MAIN.load(Relaxed) {
            return;
        }
        // Charge the open category up to now so the snapshot is exact.
        LOCAL.with(|l| {
            let l = unsafe { &mut *l.get() };
            if l.depth > 0 {
                let now = tsc();
                ThreadAcc::bump(
                    &acc.ticks[l.stack[l.depth - 1] as usize],
                    now.wrapping_sub(l.last),
                );
                l.last = now;
            }
        });
        let ticks = acc.ticks.iter().map(|t| t.load(Relaxed)).collect();
        PHASES
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .push((phase, ticks));
    }

    /// The accumulated totals as JSON: per category the main thread's and
    /// the other threads' seconds and scope counts, allocator counts, the
    /// counter rate, and the main thread's per-phase seconds.
    pub fn report() -> serde_json::Value {
        let (t0, c0) = CLOCK
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .unwrap_or((std::time::Instant::now(), tsc()));
        let (elapsed, ticks) = (t0.elapsed().as_secs_f64(), tsc().wrapping_sub(c0));
        let hz = if elapsed > 0.0 {
            ticks as f64 / elapsed
        } else {
            1.0
        };
        let main = MAIN.load(Relaxed);
        let registry = REGISTRY
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let mut cats = Vec::new();
        for (i, label) in LABELS.iter().enumerate() {
            let (mut mt, mut mc, mut wt, mut wc) = (0u64, 0u64, 0u64, 0u64);
            for acc in registry.iter() {
                let (t, c) = (acc.ticks[i].load(Relaxed), acc.counts[i].load(Relaxed));
                if acc.id == main {
                    (mt, mc) = (mt + t, mc + c);
                } else {
                    (wt, wc) = (wt + t, wc + c);
                }
            }
            if mt + mc + wt + wc > 0 {
                cats.push(serde_json::json!({
                    "cat": label, "main_s": mt as f64 / hz, "main_n": mc,
                    "workers_s": wt as f64 / hz, "workers_n": wc,
                }));
            }
        }
        let mut allocs = [[0u64; 4]; 2];
        let mut threads = 0;
        for acc in registry.iter() {
            let side = usize::from(acc.id != main);
            threads += 1;
            for k in 0..4 {
                allocs[side][k] += acc.allocs[k].load(Relaxed);
            }
        }
        let phases = PHASES
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let mut previous = vec![0u64; COUNT];
        let mut per_phase = Vec::new();
        for (phase, ticks) in phases.iter() {
            let delta: serde_json::Map<String, serde_json::Value> = (0..COUNT)
                .filter(|&i| ticks[i] > previous[i])
                .map(|i| {
                    (
                        LABELS[i].to_string(),
                        serde_json::json!((ticks[i] - previous[i]) as f64 / hz),
                    )
                })
                .collect();
            per_phase.push(serde_json::json!({"phase": phase, "main_s": delta}));
            previous.clone_from(ticks);
        }
        let memo = *MEMO
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        serde_json::json!({
            "step_memo_at_end": memo.map(|m| serde_json::json!({
                "insertion_hits": m[0], "insertion_misses": m[1], "insertion_entries": m[2],
                "route_hits": m[3], "route_misses": m[4], "route_entries": m[5],
                "coefficients": m[6],
            })),
            "tsc_hz": hz,
            "level": LEVEL.load(Relaxed),
            "threads_registered": threads,
            "categories": cats,
            "allocator": {
                "main": {"alloc": allocs[0][0], "dealloc": allocs[0][1], "realloc": allocs[0][2], "bytes": allocs[0][3]},
                "workers": {"alloc": allocs[1][0], "dealloc": allocs[1][1], "realloc": allocs[1][2], "bytes": allocs[1][3]},
            },
            "phases": per_phase,
        })
    }
}
