//! Symbolica `Arc<PolynomialContext>` scaling reproducer.
//!
//! Builds a read-only set of integer polynomials that all share ONE polynomial context (as
//! polynomials created from a common template do), then lets K threads perform the same fixed
//! amount of polynomial work on that set and reports thread CPU time per operation.
//! The variants differ only in which context / variable map each thread's polynomials use:
//!
//! * `shared`      - all threads read the same polynomials (one context, one variable map);
//! * `private`     - each thread first rebuilds its own copy on a fresh variable map and context;
//! * `private-ctx` - each thread rebuilds its own copy on a fresh context that still shares the
//!                   single variable map (separates the context count from the map count);
//! * `rehome`      - threads read the one shared set, but each operation first copies the stored
//!                   polynomial onto the thread's own fresh context (public API: `zero_with_capacity`
//!                   + `append_monomial_back`) and works on that copy: shared data, private contexts;
//! * `rehome-api`  - as `rehome`, but with the API proposed in `patch/proposed.diff`
//!                   (`zero_with_new_context` + `clone_with_context_of`); needs the patched Symbolica
//!                   and `--features proposed-api`.
//!
//! All polynomial work goes through Symbolica's public API. The work per operation mirrors an
//! application hot path: validate a stored polynomial against the expected variable map, clone
//! it, substitute a few variables by integers (`replace`), test for zero, validate the result,
//! and split a stored polynomial by a subset of its variables (`to_multivariate_polynomial_list`).
//!
//! Cache-line layout. Where the shared context's `ArcInner` (strong count, weak count, then the
//! `variables` pointer) starts within its 64-byte line decides whether `variables()`/`nvars()`
//! loads share a line with the count that `clone()`/drop write. glibc hands out 16-byte aligned
//! blocks, so this depends on the allocation history. `--ctx-offset`/`--map-offset` pin the
//! offsets on purpose (by allocating candidates through the public API until one lands at the
//! requested offset) and every run reports them.

use std::cell::Cell;
use std::collections::HashSet;
use std::io::{Read, Write};
use std::sync::{Arc, Barrier};
use std::time::Instant;

use symbolica::domains::integer::{Integer, IntegerRing, Z};
use symbolica::license::LicenseManager;
use symbolica::poly::PolyVariable;
use symbolica::poly::polynomial::MultivariatePolynomial;
use symbolica::symbol;

type Poly = MultivariatePolynomial<IntegerRing, u16>;
type VarMap = Arc<Vec<PolyVariable>>;

// The layout arithmetic below assumes a zero-sized ring (the context is then `variables` only).
const _: () = assert!(std::mem::size_of::<IntegerRing>() == 0);

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Variant {
    Shared,
    Private,
    PrivateCtx,
    Rehome,
    RehomeApi,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Work {
    /// validate + clone + replace + is_zero + validate
    Specialize,
    /// validate + to_multivariate_polynomial_list
    Split,
    /// both of the above on the same stored polynomial
    Full,
}

#[derive(Clone, Debug)]
struct Config {
    threads: usize,
    variant: Variant,
    work: Work,
    ops: usize,
    warmup: usize,
    cpus: Vec<usize>,
    npolys: usize,
    base_vars: usize,
    index_vars: usize,
    terms_min: usize,
    terms_max: usize,
    fixed: usize,
    seed: u64,
    ctx_offset: Option<usize>,
    map_offset: Option<usize>,
    ctx_vars_at: usize,
    ahash_thread_local: bool,
}

const USAGE: &str = "usage: symbolica-arc-mre --threads K --variant shared|private|private-ctx|rehome|rehome-api \
[--work full|specialize|split] [--ops N] [--warmup W] [--cpus LIST] [--npolys P] \
[--base-vars B] [--index-vars I] [--terms-min A] [--terms-max Z] [--fixed F] [--seed S] \
[--ctx-offset 0|16|32|48|any] [--map-offset 0|16|32|48|any] [--ctx-vars-at 16] [--ahash-source default|thread-local]
  --ops N         operations per thread (default 20000); warm-up W (default N/10) runs first, untimed
  --cpus LIST     e.g. 128-135,136-143: thread i is pinned to the i-th listed CPU (Linux only)
  --ctx-offset    offset in its 64-byte line of the shared context's ArcInner (default 0: strong count,
                  weak count and the `variables` pointer in one line; 48: the pointer on the next line)
  --map-offset    same for the shared variable map's ArcInner (default 32: Vec length on the next line)
  --ctx-vars-at   byte offset of `variables` in the context's ArcInner (16 upstream; 136 with
                  patch/experiment-context-padding.diff); only used to compute the reported offsets
  --ahash-source  thread-local: install a per-thread ahash seed source (ahash::random_state::
                  set_random_source) before Symbolica creates its first map
env MRE_PERF_CTL / MRE_PERF_ACK: perf stat --control fifos; counters are enabled only for the timed loop";

fn parse_cpu_list(s: &str) -> Result<Vec<usize>, String> {
    let mut out = Vec::new();
    for part in s.split(',').filter(|p| !p.is_empty()) {
        if let Some((a, b)) = part.split_once('-') {
            let a: usize = a.parse().map_err(|e| format!("bad cpu '{part}': {e}"))?;
            let b: usize = b.parse().map_err(|e| format!("bad cpu '{part}': {e}"))?;
            if b < a {
                return Err(format!("bad cpu range '{part}'"));
            }
            out.extend(a..=b);
        } else {
            out.push(part.parse().map_err(|e| format!("bad cpu '{part}': {e}"))?);
        }
    }
    Ok(out)
}

fn parse_offset(key: &str, val: &str) -> Result<Option<usize>, String> {
    match val {
        "any" => Ok(None),
        "0" | "16" | "32" | "48" => Ok(Some(val.parse().unwrap())),
        _ => Err(format!("{key} must be 0, 16, 32, 48 or any")),
    }
}

fn parse_args() -> Result<Config, String> {
    let mut cfg = Config {
        threads: 1,
        variant: Variant::Shared,
        work: Work::Full,
        ops: 20_000,
        warmup: usize::MAX,
        cpus: Vec::new(),
        npolys: 4096,
        base_vars: 4,
        index_vars: 12,
        terms_min: 8,
        terms_max: 64,
        fixed: 3,
        seed: 0x5eed_2026_0928,
        ctx_offset: Some(0),
        map_offset: Some(32),
        ctx_vars_at: 16,
        ahash_thread_local: false,
    };
    let args: Vec<String> = std::env::args().skip(1).collect();
    let mut i = 0;
    while i < args.len() {
        let key = args[i].as_str();
        if key == "-h" || key == "--help" {
            println!("{USAGE}");
            std::process::exit(0);
        }
        let val = args
            .get(i + 1)
            .ok_or_else(|| format!("missing value for {key}\n{USAGE}"))?
            .as_str();
        let num = || -> Result<usize, String> {
            val.parse::<usize>()
                .map_err(|e| format!("bad value for {key}: {e}"))
        };
        match key {
            "--threads" => cfg.threads = num()?,
            "--variant" => {
                cfg.variant = match val {
                    "shared" => Variant::Shared,
                    "private" => Variant::Private,
                    "private-ctx" => Variant::PrivateCtx,
                    "rehome" => Variant::Rehome,
                    "rehome-api" if cfg!(feature = "proposed-api") => Variant::RehomeApi,
                    _ => return Err(format!("unknown variant {val}\n{USAGE}")),
                }
            }
            "--work" => {
                cfg.work = match val {
                    "full" => Work::Full,
                    "specialize" => Work::Specialize,
                    "split" => Work::Split,
                    _ => return Err(format!("unknown work {val}\n{USAGE}")),
                }
            }
            "--ops" => cfg.ops = num()?,
            "--warmup" => cfg.warmup = num()?,
            "--cpus" => cfg.cpus = parse_cpu_list(val)?,
            "--npolys" => cfg.npolys = num()?,
            "--base-vars" => cfg.base_vars = num()?,
            "--index-vars" => cfg.index_vars = num()?,
            "--terms-min" => cfg.terms_min = num()?,
            "--terms-max" => cfg.terms_max = num()?,
            "--fixed" => cfg.fixed = num()?,
            "--seed" => cfg.seed = num()? as u64,
            "--ctx-offset" => cfg.ctx_offset = parse_offset(key, val)?,
            "--map-offset" => cfg.map_offset = parse_offset(key, val)?,
            "--ctx-vars-at" => cfg.ctx_vars_at = num()?,
            "--ahash-source" => {
                cfg.ahash_thread_local = match val {
                    "default" => false,
                    "thread-local" => true,
                    _ => return Err(format!("unknown ahash source {val}\n{USAGE}")),
                }
            }
            _ => return Err(format!("unknown option {key}\n{USAGE}")),
        }
        i += 2;
    }
    if cfg.warmup == usize::MAX {
        cfg.warmup = cfg.ops / 10;
    }
    if cfg.threads == 0 || cfg.npolys == 0 || cfg.base_vars == 0 {
        return Err("threads, npolys and base-vars must be positive".into());
    }
    if cfg.fixed > cfg.index_vars || cfg.terms_min == 0 || cfg.terms_max < cfg.terms_min {
        return Err("need fixed <= index-vars and 0 < terms-min <= terms-max".into());
    }
    if !cfg.cpus.is_empty() && cfg.cpus.len() < cfg.threads {
        return Err(format!(
            "--cpus lists {} CPUs for {} threads",
            cfg.cpus.len(),
            cfg.threads
        ));
    }
    Ok(cfg)
}

/// SplitMix64: deterministic, dependency-free test-data generator (not used for algebra).
struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9e37_79b9_7f4a_7c15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
        z ^ (z >> 31)
    }
    fn below(&mut self, n: usize) -> usize {
        (self.next() % n as u64) as usize
    }
    fn range_i64(&mut self, lo: i64, hi: i64) -> i64 {
        lo + (self.next() % (hi - lo + 1) as u64) as i64
    }
}

/// ahash seed source without process-global state (a per-thread counter); installed with
/// `ahash::random_state::set_random_source` before the first `RandomState` exists.
struct PerThreadSeed;

impl ahash::random_state::RandomSource for PerThreadSeed {
    fn gen_hasher_seed(&self) -> usize {
        thread_local! { static NEXT: Cell<usize> = const { Cell::new(0) }; }
        NEXT.with(|n| {
            let v = n.get().wrapping_add(0x9e37_79b9_7f4a_7c15);
            n.set(v);
            v ^ (n as *const Cell<usize> as usize)
        })
    }
}

// ---------------------------------------------------------------------------------------------
// Identity and cache-line layout of contexts and variable maps, through public accessors only.
// `p.variables()` borrows the `variables` field inside the context's `ArcInner` (std: `#[repr(C)]
// { strong, weak, data }`), so equal addresses mean a shared context, and the `ArcInner` starts
// `ctx_vars_at` bytes earlier (16 upstream). `Arc::as_ptr(map)` is the `Vec` inside the map's
// `ArcInner`, 16 bytes after its strong count.

fn ctx_id(p: &Poly) -> usize {
    p.variables() as *const VarMap as usize
}

fn map_id(map: &VarMap) -> usize {
    Arc::as_ptr(map) as usize
}

fn ctx_line_offset(p: &Poly, ctx_vars_at: usize) -> usize {
    (ctx_id(p) - ctx_vars_at) % 64
}

fn map_line_offset(map: &VarMap) -> usize {
    (map_id(map) - 16) % 64
}

/// Allocates candidates with `make` until `offset(candidate) == want`. Rejected candidates and
/// filler blocks of another size class are returned so that they stay alive: the allocator then
/// cannot return the same block again, and the fillers shift the next block by 16 bytes.
fn pin_alloc<T>(
    what: &str,
    want: Option<usize>,
    mut make: impl FnMut() -> T,
    offset: impl Fn(&T) -> usize,
) -> (T, Vec<T>, Vec<Box<[u8; 40]>>) {
    let (mut rejected, mut fillers) = (Vec::new(), Vec::new());
    for attempt in 0..4096 {
        let c = make();
        if want.is_none_or(|w| offset(&c) == w) {
            return (c, rejected, fillers);
        }
        rejected.push(c);
        if attempt % 2 == 1 {
            fillers.push(std::hint::black_box(Box::new([0u8; 40])));
        }
    }
    eprintln!("ERROR: could not place the {what} at line offset {want:?}; use `any`");
    std::process::exit(4);
}

/// The shared variable map: `base_vars` "kinematic" variables followed by `index_vars` variables
/// that the workload substitutes by integers.
fn variable_list(cfg: &Config) -> Vec<PolyVariable> {
    let mut vars = Vec::with_capacity(cfg.base_vars + cfg.index_vars);
    for i in 0..cfg.base_vars {
        let name = format!("mre_x{i}");
        vars.push(symbol!(name.as_str()).into());
    }
    for i in 0..cfg.index_vars {
        let name = format!("mre_n{i}");
        vars.push(symbol!(name.as_str()).into());
    }
    vars
}

/// Builds `npolys` random sparse polynomials from one template, so all of them share the
/// template's context (the recommended way to create related polynomials).
fn build_shared_set(cfg: &Config, template: &Poly) -> Vec<Poly> {
    let nvars = template.nvars();
    let mut rng = Rng(cfg.seed);
    let mut exps = vec![0u16; nvars];
    let mut polys = Vec::with_capacity(cfg.npolys);
    for _ in 0..cfg.npolys {
        let nterms = cfg.terms_min + rng.below(cfg.terms_max - cfg.terms_min + 1);
        let mut p = template.zero_with_capacity(nterms);
        while p.nterms() < nterms {
            exps.fill(0);
            for _ in 0..1 + rng.below(4) {
                exps[rng.below(nvars)] += 1 + rng.below(3) as u16;
            }
            let c = rng.range_i64(-1000, 1000);
            if c != 0 {
                p.append_monomial(Integer::from(c), &exps);
            }
        }
        polys.push(p);
    }
    polys
}

/// Copies one stored polynomial onto `template`'s context through the public API.
#[inline(never)]
fn rehome_one(template: &Poly, p: &Poly) -> Poly {
    #[cfg(feature = "proposed-api")]
    if USE_PROPOSED_API.with(|f| f.get()) {
        return p.clone_with_context_of(template);
    }
    rehome_one_public(template, p)
}

#[cfg(feature = "proposed-api")]
thread_local! {
    static USE_PROPOSED_API: Cell<bool> = const { Cell::new(false) };
}

/// (distinct contexts, distinct variable maps) of a polynomial set.
fn sharing_census(polys: &[Poly]) -> (usize, usize) {
    let contexts: HashSet<usize> = polys.iter().map(ctx_id).collect();
    let maps: HashSet<usize> = polys.iter().map(|p| map_id(p.variables())).collect();
    (contexts.len(), maps.len())
}

/// Application-side validation of a stored polynomial (reads the variable map and `nvars()`).
#[inline(never)]
fn check_on_map(p: &Poly, map: &VarMap) -> u64 {
    assert!(p.variables().as_ref() == map.as_ref(), "variable map mismatch");
    assert_eq!(p.exponents.len(), p.nterms() * p.nvars(), "exponent layout");
    p.nterms() as u64
}

/// Clone, substitute the fixed variables (descending positions), test for zero, validate.
#[inline(never)]
fn specialize(p: &Poly, map: &VarMap, fixed: &[(usize, i64)]) -> u64 {
    let mut r = p.clone();
    for &(var, value) in fixed {
        if r.degree(var) != 0 {
            r = r.replace(var, &Integer::from(value));
        }
    }
    let mut acc = r.nterms() as u64;
    if r.is_zero() {
        acc = acc.wrapping_add(1_000_003);
    }
    acc.wrapping_add(check_on_map(&r, map))
}

/// Split by the base variables (keys: base monomials; values: polynomials in the rest).
#[inline(never)]
fn split(p: &Poly, base_positions: &[usize]) -> u64 {
    let parts = p.to_multivariate_polynomial_list(base_positions, true);
    let terms: u64 = parts.values().map(|q| q.nterms() as u64).sum();
    terms.wrapping_mul(1_000_033).wrapping_add(parts.len() as u64)
}

fn draw_fixed(cfg: &Config, rng: &mut Rng, out: &mut Vec<(usize, i64)>) {
    out.clear();
    while out.len() < cfg.fixed {
        let var = cfg.base_vars + rng.below(cfg.index_vars);
        if out.iter().all(|&(v, _)| v != var) {
            out.push((var, rng.range_i64(-2, 4)));
        }
    }
    out.sort_unstable_by(|a, b| b.0.cmp(&a.0));
}

/// `rehome`: Some((template on the thread's private context, private variable map)).
fn run_ops(
    cfg: &Config,
    polys: &[Poly],
    map: &VarMap,
    rehome: Option<(&Poly, &VarMap)>,
    base_positions: &[usize],
    seed: u64,
    n: usize,
) -> u64 {
    let mut rng = Rng(seed);
    let mut fixed = Vec::with_capacity(cfg.fixed);
    let mut checksum = 0u64;
    for _ in 0..n {
        let stored = &polys[rng.below(polys.len())];
        draw_fixed(cfg, &mut rng, &mut fixed);
        let mut acc = check_on_map(stored, map);
        let own;
        let (p, map) = match rehome {
            None => (stored, map),
            Some((template, own_map)) => {
                own = rehome_one(template, stored);
                (&own, own_map)
            }
        };
        match cfg.work {
            Work::Specialize => acc = acc.wrapping_add(specialize(p, map, &fixed)),
            Work::Split => acc = acc.wrapping_add(split(p, base_positions)),
            Work::Full => {
                acc = acc.wrapping_add(split(p, base_positions));
                acc = acc.wrapping_add(specialize(p, map, &fixed));
            }
        }
        checksum = checksum.wrapping_mul(0x100_0000_01b3).wrapping_add(acc);
    }
    checksum
}

// ---------------------------------------------------------------------------------------------
// OS helpers. Pinning, the current CPU and per-thread context switches are Linux-only; elsewhere
// the program runs unpinned (not tested).

#[cfg(target_os = "linux")]
fn pin_to(cpu: usize) {
    // SAFETY: plain libc calls on a zero-initialised cpu_set_t owned by this frame.
    let rc = unsafe {
        let mut set: libc::cpu_set_t = std::mem::zeroed();
        libc::CPU_ZERO(&mut set);
        libc::CPU_SET(cpu, &mut set);
        libc::sched_setaffinity(0, std::mem::size_of::<libc::cpu_set_t>(), &set)
    };
    if rc != 0 {
        panic!(
            "sched_setaffinity(cpu {cpu}) failed: {}",
            std::io::Error::last_os_error()
        );
    }
}

#[cfg(not(target_os = "linux"))]
fn pin_to(_cpu: usize) {
    panic!("--cpus needs Linux (sched_setaffinity); run without --cpus");
}

#[cfg(target_os = "linux")]
fn current_cpu() -> i64 {
    // SAFETY: no arguments.
    unsafe { libc::sched_getcpu() as i64 }
}

#[cfg(not(target_os = "linux"))]
fn current_cpu() -> i64 {
    -1
}

/// (voluntary, involuntary) context switches of the calling thread.
#[cfg(target_os = "linux")]
fn thread_switches() -> (i64, i64) {
    // SAFETY: valid pointer to a zeroed local rusage.
    unsafe {
        let mut ru: libc::rusage = std::mem::zeroed();
        libc::getrusage(libc::RUSAGE_THREAD, &mut ru);
        (ru.ru_nvcsw, ru.ru_nivcsw)
    }
}

#[cfg(not(target_os = "linux"))]
fn thread_switches() -> (i64, i64) {
    (0, 0)
}

#[cfg(all(target_os = "linux", target_env = "gnu"))]
fn libc_version() -> String {
    // SAFETY: gnu_get_libc_version returns a static NUL-terminated string.
    let v = unsafe { std::ffi::CStr::from_ptr(libc::gnu_get_libc_version()) };
    format!("glibc {}", v.to_string_lossy())
}

#[cfg(not(all(target_os = "linux", target_env = "gnu")))]
fn libc_version() -> String {
    "not glibc".to_string()
}

fn thread_cpu_ns() -> u64 {
    let mut ts = libc::timespec {
        tv_sec: 0,
        tv_nsec: 0,
    };
    // SAFETY: valid pointer to a local timespec.
    let rc = unsafe { libc::clock_gettime(libc::CLOCK_THREAD_CPUTIME_ID, &mut ts) };
    assert_eq!(rc, 0, "clock_gettime(CLOCK_THREAD_CPUTIME_ID) failed");
    ts.tv_sec as u64 * 1_000_000_000 + ts.tv_nsec as u64
}

fn proc_status_kib(field: &str) -> u64 {
    std::fs::read_to_string("/proc/self/status")
        .ok()
        .and_then(|s| {
            s.lines()
                .find(|l| l.starts_with(field))
                .and_then(|l| l.split_whitespace().nth(1))
                .and_then(|v| v.parse().ok())
        })
        .unwrap_or(0)
}

/// `perf stat -D -1 --control fifo:CTL,ACK`: the counters run only between `enable` and `disable`.
struct PerfControl {
    ctl: std::fs::File,
    ack: std::fs::File,
}

impl PerfControl {
    fn from_env() -> Option<Self> {
        let ctl = std::env::var("MRE_PERF_CTL").ok()?;
        let ack = std::env::var("MRE_PERF_ACK").ok()?;
        // Read+write opens of a FIFO never block on Linux.
        let open = |p: &str| {
            std::fs::OpenOptions::new()
                .read(true)
                .write(true)
                .open(p)
                .unwrap_or_else(|e| panic!("cannot open perf control fifo {p}: {e}"))
        };
        Some(PerfControl {
            ctl: open(&ctl),
            ack: open(&ack),
        })
    }

    fn command(&mut self, cmd: &str) {
        self.ctl
            .write_all(format!("{cmd}\n").as_bytes())
            .expect("perf control write");
        let mut byte = [0u8; 1];
        let mut line = Vec::new();
        loop {
            self.ack.read_exact(&mut byte).expect("perf control ack");
            match byte[0] {
                b'\n' => break,
                0 => {}
                b => line.push(b),
            }
        }
        assert_eq!(line, b"ack", "unexpected perf control reply");
    }
}

struct ThreadResult {
    cpu_ns: u64,
    wall_ns: u64,
    setup_cpu_ns: u64,
    setup_wall_ns: u64,
    checksum: u64,
    nvcsw: i64,
    nivcsw: i64,
    cpu_at_end: i64,
    census: (usize, usize),
}

fn main() {
    let cfg = match parse_args() {
        Ok(c) => c,
        Err(e) => {
            eprintln!("{e}");
            std::process::exit(2);
        }
    };

    // Must precede anything that may create an ahash RandomState (Symbolica's global state).
    if cfg.ahash_thread_local
        && ahash::random_state::set_random_source(PerThreadSeed).is_err()
    {
        eprintln!("ERROR: ahash's random source was already initialised; cannot install it");
        std::process::exit(5);
    }

    // The main thread (license check, globals, the shared set) runs on the first listed CPU, so
    // the shared data are first touched on that CPU's NUMA node.
    if let Some(&cpu) = cfg.cpus.first() {
        pin_to(cpu);
    }

    // Multi-threaded use of Symbolica requires an active license (SYMBOLICA_LICENSE); an
    // unlicensed instance is restricted to one Symbolica thread. Refuse to produce numbers then.
    if !LicenseManager::is_licensed() {
        eprintln!(
            "ERROR: no active Symbolica license. Set SYMBOLICA_LICENSE to a valid key \
             (multi-threaded Symbolica requires it); refusing to run."
        );
        std::process::exit(3);
    }

    let t_build = Instant::now();
    let var_list = variable_list(&cfg);
    // Shared variable map and shared context, each placed at the requested line offset.
    let (vars, _rejected_maps, _fillers_m) = pin_alloc(
        "variable map",
        cfg.map_offset,
        || Arc::new(var_list.clone()),
        map_line_offset,
    );
    let (template, _rejected_ctx, _fillers_c) = pin_alloc(
        "context",
        cfg.ctx_offset,
        || Poly::new(&Z, None, vars.clone()),
        |t| ctx_line_offset(t, cfg.ctx_vars_at),
    );
    let shared = Arc::new(build_shared_set(&cfg, &template));
    let build_s = t_build.elapsed().as_secs_f64();
    let (shared_ctx, shared_maps) = sharing_census(&shared);
    assert_eq!(
        (shared_ctx, shared_maps),
        (1, 1),
        "the input set must share one context"
    );
    assert_eq!(ctx_id(&shared[0]), ctx_id(&template));
    assert_eq!(map_id(shared[0].variables()), map_id(&vars));
    let shared_ctx_id = ctx_id(&template);
    let shared_map_id = map_id(&vars);
    let ctx_off = ctx_line_offset(&template, cfg.ctx_vars_at);
    let map_off = map_line_offset(&vars);
    let ctx_vars_same_line = ctx_off + cfg.ctx_vars_at < 64;
    let total_terms: usize = shared.iter().map(|p| p.nterms()).sum();
    let base_positions: Arc<Vec<usize>> = Arc::new((0..cfg.base_vars).collect());

    eprintln!(
        "# {} | {} | variant={:?} work={:?} threads={} ops/thread={} warmup={} | set: {} polys, {} vars \
         ({} base + {} index), {} terms total, built in {:.3} s | layout: context ArcInner at line \
         offset {} (`variables` {}), variable-map ArcInner at line offset {} | ahash source: {}",
        LicenseManager::get_version(),
        libc_version(),
        cfg.variant,
        cfg.work,
        cfg.threads,
        cfg.ops,
        cfg.warmup,
        cfg.npolys,
        vars.len(),
        cfg.base_vars,
        cfg.index_vars,
        total_terms,
        build_s,
        ctx_off,
        if ctx_vars_same_line {
            "in the counts' line"
        } else {
            "on the next line"
        },
        map_off,
        if cfg.ahash_thread_local {
            "per-thread"
        } else {
            "default"
        }
    );

    let setup_done = Arc::new(Barrier::new(cfg.threads + 1));
    let start = Arc::new(Barrier::new(cfg.threads + 1));
    let mut handles = Vec::with_capacity(cfg.threads);
    for t in 0..cfg.threads {
        let cfg = cfg.clone();
        let shared = shared.clone();
        let vars = vars.clone();
        let base_positions = base_positions.clone();
        let setup_done = setup_done.clone();
        let start = start.clone();
        handles.push(std::thread::spawn(move || {
            if let Some(&cpu) = cfg.cpus.get(t) {
                pin_to(cpu);
            }
            // Per-thread copies are built by the pinned thread itself (first touch is local).
            let s_cpu = thread_cpu_ns();
            let s_wall = Instant::now();
            let fresh_template = || {
                let fresh_map: VarMap = Arc::new(vars.as_ref().clone());
                (Poly::new(&Z, None, fresh_map.clone()), fresh_map)
            };
            let mut rehome_template = None;
            let (own, map): (Option<Vec<Poly>>, VarMap) = match cfg.variant {
                Variant::Shared => (None, vars.clone()),
                Variant::Rehome => {
                    rehome_template = Some(fresh_template());
                    (None, vars.clone())
                }
                #[cfg(feature = "proposed-api")]
                Variant::RehomeApi => {
                    USE_PROPOSED_API.with(|f| f.set(true));
                    let template = shared[0].zero_with_new_context();
                    let fresh_map = template.variables().clone();
                    rehome_template = Some((template, fresh_map));
                    (None, vars.clone())
                }
                #[cfg(not(feature = "proposed-api"))]
                Variant::RehomeApi => unreachable!("rehome-api needs --features proposed-api"),
                Variant::Private => {
                    let (template, fresh_map) = fresh_template();
                    (Some(rebuild_all(&template, &shared)), fresh_map)
                }
                Variant::PrivateCtx => {
                    let template = Poly::new(&Z, None, vars.clone());
                    (Some(rebuild_all(&template, &shared)), vars.clone())
                }
            };
            let polys: &[Poly] = own.as_deref().unwrap_or(&shared);
            let rehome = rehome_template.as_ref().map(|(t, m)| (t, m));
            let census = sharing_census(polys);
            // The variants must really use the contexts / maps they claim to use.
            let probe = match rehome {
                Some((template, _)) => rehome_one(template, &shared[0]),
                None => polys[0].clone(),
            };
            let (own_ctx, own_map) = (ctx_id(&probe), map_id(probe.variables()));
            let (want_new_ctx, want_new_map) = match cfg.variant {
                Variant::Shared => (false, false),
                Variant::PrivateCtx => (true, false),
                _ => (true, true),
            };
            assert_eq!(census, (1, 1), "thread {t}: set must use one context");
            assert_eq!(own_ctx != shared_ctx_id, want_new_ctx, "thread {t}: context");
            assert_eq!(own_map != shared_map_id, want_new_map, "thread {t}: map");
            if let Some((template, _)) = rehome {
                assert_eq!(own_ctx, ctx_id(template), "thread {t}: rehomed copy");
            }
            drop(probe);
            let setup_cpu_ns = thread_cpu_ns() - s_cpu;
            let setup_wall_ns = s_wall.elapsed().as_nanos() as u64;
            let seed = cfg.seed ^ (t as u64 + 1).wrapping_mul(0xd1b5_4a32_d192_ed03);
            // Warm-up on a separate stream so the timed operation sequence is fixed per thread.
            std::hint::black_box(run_ops(
                &cfg,
                polys,
                &map,
                rehome,
                &base_positions,
                !seed,
                cfg.warmup,
            ));
            setup_done.wait();
            start.wait();
            let (v0, i0) = thread_switches();
            let c0 = thread_cpu_ns();
            let w0 = Instant::now();
            let checksum = run_ops(&cfg, polys, &map, rehome, &base_positions, seed, cfg.ops);
            let cpu_ns = thread_cpu_ns() - c0;
            let wall_ns = w0.elapsed().as_nanos() as u64;
            let (v1, i1) = thread_switches();
            ThreadResult {
                cpu_ns,
                wall_ns,
                setup_cpu_ns,
                setup_wall_ns,
                checksum,
                nvcsw: v1 - v0,
                nivcsw: i1 - i0,
                cpu_at_end: current_cpu(),
                census,
            }
        }));
    }
    setup_done.wait();
    let rss_setup = proc_status_kib("VmRSS:");
    let mut perf = PerfControl::from_env();
    if let Some(p) = perf.as_mut() {
        p.command("enable");
    }
    start.wait();
    let w0 = Instant::now();
    let results: Vec<ThreadResult> = handles
        .into_iter()
        .map(|h| h.join().expect("worker panicked"))
        .collect();
    let wall_s = w0.elapsed().as_secs_f64();
    if let Some(p) = perf.as_mut() {
        p.command("disable");
    }

    let total_ops = (cfg.ops * cfg.threads) as f64;
    let cpu_ns: u64 = results.iter().map(|r| r.cpu_ns).sum();
    let per_thread: Vec<f64> = results
        .iter()
        .map(|r| r.cpu_ns as f64 / cfg.ops as f64)
        .collect();
    let min_t = per_thread.iter().cloned().fold(f64::INFINITY, f64::min);
    let max_t = per_thread.iter().cloned().fold(0.0, f64::max);
    let max_wall = results.iter().map(|r| r.wall_ns).max().unwrap_or(0) as f64 * 1e-9;
    let setup_cpu_s: f64 = results.iter().map(|r| r.setup_cpu_ns).sum::<u64>() as f64 * 1e-9;
    let setup_wall_s = results.iter().map(|r| r.setup_wall_ns).max().unwrap_or(0) as f64 * 1e-9;
    let checksum = results
        .iter()
        .fold(0u64, |a, r| a.rotate_left(7) ^ r.checksum);
    let nivcsw: i64 = results.iter().map(|r| r.nivcsw).sum();
    let nvcsw: i64 = results.iter().map(|r| r.nvcsw).sum();
    let mispinned = results
        .iter()
        .enumerate()
        .filter(|(t, r)| cfg.cpus.get(*t).is_some_and(|&c| c as i64 != r.cpu_at_end))
        .count();
    let (ctx0, map0) = results[0].census;
    let cpus = if cfg.cpus.is_empty() {
        "unpinned".to_string()
    } else {
        let c = &cfg.cpus[..cfg.threads];
        format!("{}-{}", c[0], c[c.len() - 1])
    };

    eprintln!(
        "# thread 0 set: {ctx0} context(s), {map0} variable map(s); setup {:.3} s wall, RSS after setup {} MiB",
        setup_wall_s,
        rss_setup / 1024
    );
    println!(
        "RESULT {{\"variant\":\"{}\",\"work\":\"{}\",\"threads\":{},\"ops_per_thread\":{},\
\"cpu_ns_per_op\":{:.1},\"min_thread_cpu_ns_per_op\":{:.1},\"max_thread_cpu_ns_per_op\":{:.1},\
\"wall_s\":{:.4},\"max_thread_wall_s\":{:.4},\"ops_per_s\":{:.0},\"setup_wall_s\":{:.4},\"setup_cpu_s\":{:.4},\
\"rss_mib_after_setup\":{},\"thread0_contexts\":{},\"thread0_maps\":{},\"checksum\":\"{:016x}\",\
\"nvcsw\":{},\"nivcsw\":{},\"mispinned\":{},\"cpus\":\"{}\",\"npolys\":{},\"nvars\":{},\"terms\":{},\
\"ctx_line_offset\":{},\"ctx_vars_same_line\":{},\"map_line_offset\":{},\"ahash_source\":\"{}\",\
\"perf_timed_loop_only\":{},\"libc\":\"{}\",\"symbolica\":\"{}\"}}",
        match cfg.variant {
            Variant::Shared => "shared",
            Variant::Private => "private",
            Variant::PrivateCtx => "private-ctx",
            Variant::Rehome => "rehome",
            Variant::RehomeApi => "rehome-api",
        },
        match cfg.work {
            Work::Full => "full",
            Work::Specialize => "specialize",
            Work::Split => "split",
        },
        cfg.threads,
        cfg.ops,
        cpu_ns as f64 / total_ops,
        min_t,
        max_t,
        wall_s,
        max_wall,
        total_ops / wall_s,
        setup_wall_s,
        setup_cpu_s,
        rss_setup / 1024,
        ctx0,
        map0,
        checksum,
        nvcsw,
        nivcsw,
        mispinned,
        cpus,
        cfg.npolys,
        vars.len(),
        total_terms,
        ctx_off,
        ctx_vars_same_line,
        map_off,
        if cfg.ahash_thread_local {
            "thread-local"
        } else {
            "default"
        },
        perf.is_some(),
        libc_version(),
        LicenseManager::get_version()
    );
    if mispinned > 0 {
        eprintln!("WARNING: {mispinned} thread(s) ended on a CPU other than the requested one");
    }
}

/// Rebuilds `source` term by term on `template`'s context, through the public API.
fn rebuild_all(template: &Poly, source: &[Poly]) -> Vec<Poly> {
    source.iter().map(|p| rehome_one_public(template, p)).collect()
}

/// Public-API copy onto `template`'s context (the `rehome` path without the proposed API).
fn rehome_one_public(template: &Poly, p: &Poly) -> Poly {
    let mut q = template.zero_with_capacity(p.nterms());
    for (c, e) in p.coefficients.iter().zip(p.exponents_iter()) {
        q.append_monomial_back(c.clone(), e);
    }
    q
}
