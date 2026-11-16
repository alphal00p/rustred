// gate10: g9fault.c + G8_ERRNO=delay also for op pidfd (pidfd_open delayed, never failed)
// gate9 LD_PRELOAD fault injector (G8_ERRNO=delay + G8_DELAY_US: an open of a candidate is DELAYED, never failed; extends gate8 g8fault.c: + getdents64/pidfd_open/waitid via syscall(), waitpid/waitid/wait4,
// clock_gettime (CLOCK_BOOTTIME / MONOTONIC / REALTIME: op "clock"), G8_ERRNO=pshort = POSIX short read (fewer bytes, NOT torn);
// ops: open,read,readdir,opendir,stat,readlink,getdents,wait,clock,pidfd; G8_MATCH "*" matches every path (and the path-less ops)).
// gate8 LD_PRELOAD fault injector (test harness only, ONLY inside private namespaces). Extends gate7's g7fault.c.
// Active only in a process whose argv[1] equals $G8_ARGV1 (default "run": the supervisor, never its shims / jobs).
// A call is a CANDIDATE when its operation is in $G8_OPS (comma list of open,read,readdir,opendir,stat,readlink;
// default "open") and its path contains one of the '|' separated substrings of $G8_MATCH (for read: the path the fd was
// opened with; for readdir: the path given to opendir; for stat with AT_EMPTY_PATH: the fd's path).
// Candidates are numbered 1,2,... (process-wide, or per thread with G8_PERTHREAD=1). Candidate n FAILS when
//   n is in $G8_AT (comma list), or $G8_FROM <= n <= $G8_TO (TO default: infinity), and (n - FROM) % $G8_EVERY == $G8_EVERY-1
//   (EVERY default 1: every candidate in the range),
// and, if $G8_ARM is set, only while that file exists. A failure sets errno $G8_ERRNO (default EMFILE=24) and returns -1 /
// NULL; G8_ERRNO=short makes a read return at most half the requested bytes and every later read of that fd return 0
// (a torn file); G8_ERRNO=trunc makes readdir end early without an error (errno 0). Every candidate is logged to $G8_LOG.
#define _GNU_SOURCE
#include <dlfcn.h>
#include <dirent.h>
#include <errno.h>
#include <fcntl.h>
#include <pthread.h>
#include <stdarg.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <unistd.h>
#include <time.h>
#include <sys/stat.h>
#include <sys/syscall.h>
#include <sys/types.h>

static int enabled = -1;
static int count_global = 0;
static __thread int count_thread = 0;
#define MAXFD 65536
static char *fdpath[MAXFD];
static unsigned char fdtorn[MAXFD];
#define MAXDIR 256
static DIR *dirs[MAXDIR];
static char *dirpath[MAXDIR];
static pthread_mutex_t mu = PTHREAD_MUTEX_INITIALIZER;
static __thread int inside = 0;

static int is_enabled(void) {
  if (enabled >= 0) return enabled;
  const char *want = getenv("G8_ARGV1"); if (!want) want = "run";
  char buf[4096]; int fd = syscall(SYS_openat, AT_FDCWD, "/proc/self/cmdline", O_RDONLY | O_CLOEXEC);
  int ok = 0;
  if (fd >= 0) { ssize_t n = syscall(SYS_read, fd, buf, sizeof buf - 1); syscall(SYS_close, fd);
    if (n > 0) { buf[n] = 0; size_t l0 = strlen(buf); if ((ssize_t)l0 + 1 < n && !strcmp(buf + l0 + 1, want)) ok = 1; } }
  enabled = ok && getenv("G8_MATCH") != NULL;
  return enabled;
}
static int op_on(const char *op) {
  const char *ops = getenv("G8_OPS"); if (!ops) ops = "open";
  size_t lo = strlen(op); const char *p = ops;
  while (*p) { const char *e = strchr(p, ','); size_t l = e ? (size_t)(e - p) : strlen(p);
    if (l == lo && !strncmp(p, op, l)) return 1; if (!e) break; p = e + 1; }
  return 0;
}
static int path_match(const char *path) {
  if (!path) return 0; const char *m = getenv("G8_MATCH"); if (!m) return 0;
  char b[1024]; strncpy(b, m, sizeof b - 1); b[sizeof b - 1] = 0; char *save = 0;
  for (char *t = strtok_r(b, "|", &save); t; t = strtok_r(0, "|", &save)) { if (!strcmp(t, "*")) return 1; if (*t && strstr(path, t)) return 1; }
  return 0;
}
static void logline(const char *op, const char *path, int n, int fail) {
  const char *lg = getenv("G8_LOG"); if (!lg) return;
  int fd = syscall(SYS_openat, AT_FDCWD, lg, O_WRONLY | O_APPEND | O_CREAT | O_CLOEXEC, 0600); if (fd < 0) return;
  char m[1400]; struct timespec ts; clock_gettime(CLOCK_MONOTONIC, &ts);
  int k = snprintf(m, sizeof m, "%ld.%03ld pid %d tid %ld %s#%d %s%s\n", (long)ts.tv_sec, ts.tv_nsec / 1000000L, getpid(),
                   (long)syscall(SYS_gettid), op, n, path, fail ? " FAILED" : "");
  if (k > 0) { if (syscall(SYS_write, fd, m, (size_t)k)) {} } syscall(SYS_close, fd);
}
// 0: pass; 1: fail
static int decide(const char *op, const char *path) {
  if (inside || !is_enabled() || !op_on(op) || !path_match(path)) return 0;
  inside = 1;
  const char *arm = getenv("G8_ARM");
  if (arm && syscall(SYS_faccessat, AT_FDCWD, arm, F_OK, 0) != 0) { inside = 0; return 0; }
  int n = getenv("G8_PERTHREAD") ? ++count_thread : __atomic_add_fetch(&count_global, 1, __ATOMIC_SEQ_CST);
  int fail = 0;
  const char *at = getenv("G8_AT");
  if (at) { char b[512]; strncpy(b, at, 511); b[511] = 0; char *s = 0;
    for (char *t = strtok_r(b, ",", &s); t; t = strtok_r(0, ",", &s)) if (atoi(t) == n) fail = 1; }
  const char *from = getenv("G8_FROM");
  if (from) { int f = atoi(from); const char *to = getenv("G8_TO"); int tt = to ? atoi(to) : 0x7fffffff;
    const char *ev = getenv("G8_EVERY"); int e = ev ? atoi(ev) : 1; if (e < 1) e = 1;
    if (n >= f && n <= tt && ((n - f) % e) == e - 1) fail = 1; }
  const char *pr = getenv("G8_PROB");   // per-mille probability (thread-local xorshift), within FROM..TO when FROM is set
  if (pr) { static __thread unsigned long long x = 0; if (!x) x = 0x9E3779B97F4A7C15ULL ^ ((unsigned long long)syscall(SYS_gettid) << 17) ^ (unsigned long long)n ^ __builtin_ia32_rdtsc() /* gate9: per run */;
    x ^= x << 13; x ^= x >> 7; x ^= x << 17; int inr = 1;
    if (from) { const char *to = getenv("G8_TO"); inr = n >= atoi(from) && n <= (to ? atoi(to) : 0x7fffffff); }
    fail = inr && (int)(x % 1000) < atoi(pr); }
  logline(op, path, n, fail);
  inside = 0;
  return fail;
}
static int mode_errno(void) { const char *e = getenv("G8_ERRNO"); if (!e) return EMFILE; return atoi(e); }
static int mode_is(const char *w) { const char *e = getenv("G8_ERRNO"); return e && !strcmp(e, w); }

static void remember_fd(int fd, const char *path) {
  if (fd < 0 || fd >= MAXFD || inside) return;
  if (!is_enabled() || !path_match(path)) return;
  pthread_mutex_lock(&mu); free(fdpath[fd]); fdpath[fd] = strdup(path); fdtorn[fd] = 0; pthread_mutex_unlock(&mu);
}
static void forget_fd(int fd) {
  if (fd < 0 || fd >= MAXFD) return;
  if (fdpath[fd]) { pthread_mutex_lock(&mu); free(fdpath[fd]); fdpath[fd] = 0; fdtorn[fd] = 0; pthread_mutex_unlock(&mu); }
}

#define OPENWRAP(name)                                                                                                  \
  int name(const char *p, int f, ...) {                                                                                \
    mode_t mo = 0; if (f & (O_CREAT | O_TMPFILE)) { va_list a; va_start(a, f); mo = va_arg(a, int); va_end(a); }      \
    if (decide("open", p) && !mode_is("short") && !mode_is("trunc")) { if (mode_is("delay")) { const char *dl = getenv("G8_DELAY_US"); usleep(dl ? atoi(dl) : 1000); } else { errno = mode_errno(); return -1; } } \
    static int (*r)(const char *, int, ...) = 0; if (!r) r = dlsym(RTLD_NEXT, #name);                                 \
    int fd = r(p, f, mo); remember_fd(fd, p); return fd; }
OPENWRAP(open)
OPENWRAP(open64)
#define OPENATWRAP(name)                                                                                                \
  int name(int d, const char *p, int f, ...) {                                                                         \
    mode_t mo = 0; if (f & (O_CREAT | O_TMPFILE)) { va_list a; va_start(a, f); mo = va_arg(a, int); va_end(a); }      \
    if (decide("open", p) && !mode_is("short") && !mode_is("trunc")) { if (mode_is("delay")) { const char *dl = getenv("G8_DELAY_US"); usleep(dl ? atoi(dl) : 1000); } else { errno = mode_errno(); return -1; } } \
    static int (*r)(int, const char *, int, ...) = 0; if (!r) r = dlsym(RTLD_NEXT, #name);                            \
    int fd = r(d, p, f, mo); remember_fd(fd, p); return fd; }
OPENATWRAP(openat)
OPENATWRAP(openat64)

int close(int fd) {
  forget_fd(fd);
  static int (*r)(int) = 0; if (!r) r = dlsym(RTLD_NEXT, "close");
  return r(fd);
}
static ssize_t read_common(int fd, size_t c, int *handled) {
  *handled = 0;
  if (fd < 0 || fd >= MAXFD || !fdpath[fd]) return 0;
  if (fdtorn[fd]) { *handled = 1; return 0; }
  char path[1100]; pthread_mutex_lock(&mu); if (!fdpath[fd]) { pthread_mutex_unlock(&mu); return 0; }
  strncpy(path, fdpath[fd], sizeof path - 1); path[sizeof path - 1] = 0; pthread_mutex_unlock(&mu);
  if (!decide("read", path)) return 0;
  if (mode_is("short")) { *handled = 2; fdtorn[fd] = 1; return (ssize_t)(c / 2); }
  if (mode_is("pshort")) { *handled = 2; return (ssize_t)(c > 1 ? 1 : c); }
  *handled = 1; errno = mode_errno(); return -1;
}
ssize_t read(int fd, void *b, size_t c) {
  static ssize_t (*r)(int, void *, size_t) = 0; if (!r) r = dlsym(RTLD_NEXT, "read");
  int h; ssize_t v = read_common(fd, c, &h);
  if (h == 1) return v;
  if (h == 2) { if (v == 0) return 0; return r(fd, b, (size_t)v); }
  return r(fd, b, c);
}
ssize_t pread64(int fd, void *b, size_t c, off_t o) {
  static ssize_t (*r)(int, void *, size_t, off_t) = 0; if (!r) r = dlsym(RTLD_NEXT, "pread64");
  int h; ssize_t v = read_common(fd, c, &h);
  if (h == 1) return v;
  if (h == 2) { if (v == 0) return 0; return r(fd, b, (size_t)v, o); }
  return r(fd, b, c, o);
}

static void remember_dir(DIR *d, const char *path) {
  if (!d || inside || !is_enabled() || !path_match(path)) return;
  pthread_mutex_lock(&mu);
  for (int i = 0; i < MAXDIR; i++) if (!dirs[i]) { dirs[i] = d; dirpath[i] = strdup(path); break; }
  pthread_mutex_unlock(&mu);
}
static int dir_path(DIR *d, char *out, size_t n) {
  int ok = 0; pthread_mutex_lock(&mu);
  for (int i = 0; i < MAXDIR; i++) if (dirs[i] == d) { strncpy(out, dirpath[i], n - 1); out[n - 1] = 0; ok = 1; break; }
  pthread_mutex_unlock(&mu); return ok;
}
DIR *opendir(const char *p) {
  if (decide("opendir", p) && !mode_is("short") && !mode_is("trunc")) { errno = mode_errno(); return NULL; }
  static DIR *(*r)(const char *) = 0; if (!r) r = dlsym(RTLD_NEXT, "opendir");
  DIR *d = r(p); remember_dir(d, p); return d;
}
int closedir(DIR *d) {
  pthread_mutex_lock(&mu);
  for (int i = 0; i < MAXDIR; i++) if (dirs[i] == d) { dirs[i] = 0; free(dirpath[i]); dirpath[i] = 0; }
  pthread_mutex_unlock(&mu);
  static int (*r)(DIR *) = 0; if (!r) r = dlsym(RTLD_NEXT, "closedir");
  return r(d);
}
#define READDIRWRAP(name, T)                                                                                            \
  T *name(DIR *d) {                                                                                                    \
    static T *(*r)(DIR *) = 0; if (!r) r = dlsym(RTLD_NEXT, #name);                                                   \
    char p[1100];                                                                                                      \
    if (dir_path(d, p, sizeof p) && decide("readdir", p)) {                                                           \
      if (mode_is("trunc")) { errno = 0; return NULL; }                                                                \
      errno = mode_errno(); return NULL; }                                                                             \
    return r(d); }
READDIRWRAP(readdir64, struct dirent64)
READDIRWRAP(readdir, struct dirent)

int statx(int d, const char *p, int fl, unsigned int mask, struct statx *b) {
  static int (*r)(int, const char *, int, unsigned int, struct statx *) = 0; if (!r) r = dlsym(RTLD_NEXT, "statx");
  const char *path = p; char fp[1100];
  if (p && !*p && (fl & AT_EMPTY_PATH) && d >= 0 && d < MAXFD && fdpath[d]) {
    pthread_mutex_lock(&mu); if (fdpath[d]) { strncpy(fp, fdpath[d], sizeof fp - 1); fp[sizeof fp - 1] = 0; path = fp; }
    pthread_mutex_unlock(&mu); }
  /* gate9: the FIRST statx of a process always passes (a failed first statx sends Rust std into its availability probe,
     which crashes under LD_PRELOAD: SIGSEGV in any Rust program, an artefact of the harness) */
  static int first_statx_done = 0;
  if (!__atomic_exchange_n(&first_statx_done, 1, __ATOMIC_SEQ_CST)) return r(d, p, fl, mask, b);
  if (path && *path && decide("stat", path) && !mode_is("short") && !mode_is("trunc")) { errno = mode_errno(); return -1; }
  return r(d, p, fl, mask, b);
}
#define STATWRAP(name, ST)                                                                                              \
  int name(const char *p, ST *b) {                                                                                     \
    static int (*r)(const char *, ST *) = 0; if (!r) r = dlsym(RTLD_NEXT, #name);                                     \
    if (decide("stat", p) && !mode_is("short") && !mode_is("trunc")) { errno = mode_errno(); return -1; }            \
    return r(p, b); }
STATWRAP(stat, struct stat)
STATWRAP(stat64, struct stat64)
STATWRAP(lstat, struct stat)
STATWRAP(lstat64, struct stat64)
int fstatat64(int d, const char *p, struct stat64 *b, int fl) {
  static int (*r)(int, const char *, struct stat64 *, int) = 0; if (!r) r = dlsym(RTLD_NEXT, "fstatat64");
  if (p && *p && decide("stat", p) && !mode_is("short") && !mode_is("trunc")) { errno = mode_errno(); return -1; }
  return r(d, p, b, fl);
}
ssize_t readlink(const char *p, char *b, size_t n) {
  static ssize_t (*r)(const char *, char *, size_t) = 0; if (!r) r = dlsym(RTLD_NEXT, "readlink");
  if (decide("readlink", p) && !mode_is("short") && !mode_is("trunc")) { errno = mode_errno(); return -1; }
  return r(p, b, n);
}
ssize_t readlinkat(int d, const char *p, char *b, size_t n) {
  static ssize_t (*r)(int, const char *, char *, size_t) = 0; if (!r) r = dlsym(RTLD_NEXT, "readlinkat");
  if (decide("readlink", p) && !mode_is("short") && !mode_is("trunc")) { errno = mode_errno(); return -1; }
  return r(d, p, b, n);
}

// ---- gate9 additions ----
#include <sys/wait.h>
#include <stdint.h>
static int simple_fail(const char *op, const char *path) {
  if (!decide(op, path)) return 0;
  if (mode_is("short") || mode_is("trunc") || mode_is("pshort")) return 0;
  return 1;
}
long syscall(long n, ...) {
  va_list a; va_start(a, n);
  long x1 = va_arg(a, long), x2 = va_arg(a, long), x3 = va_arg(a, long), x4 = va_arg(a, long), x5 = va_arg(a, long), x6 = va_arg(a, long);
  va_end(a);
  static long (*r)(long, ...) = 0; if (!r) r = dlsym(RTLD_NEXT, "syscall");
  if (n == SYS_getdents64) {
    char p[1100]; int have = 0;
    if (x1 >= 0 && x1 < MAXFD && fdpath[x1]) { pthread_mutex_lock(&mu); if (fdpath[x1]) { strncpy(p, fdpath[x1], sizeof p - 1); p[sizeof p - 1] = 0; have = 1; } pthread_mutex_unlock(&mu); }
    if (have && decide("getdents", p)) {
      if (mode_is("trunc")) return 0;                      /* listing ends early, no error */
      if (!mode_is("short") && !mode_is("pshort")) { errno = mode_errno(); return -1; }
    }
  }
#ifdef SYS_pidfd_open
  if (n == SYS_pidfd_open && mode_is("delay")) { if (decide("pidfd", "pidfd_open")) { const char *dl = getenv("G8_DELAY_US"); usleep(dl ? atoi(dl) : 1000); } }
  else if (n == SYS_pidfd_open && simple_fail("pidfd", "pidfd_open")) { errno = mode_errno(); return -1; }
#endif
  if (n == SYS_waitid && simple_fail("wait", "waitid")) { errno = mode_errno(); return -1; }
  return r(n, x1, x2, x3, x4, x5, x6);
}
pid_t waitpid(pid_t p, int *st, int o) {
  static pid_t (*r)(pid_t, int *, int) = 0; if (!r) r = dlsym(RTLD_NEXT, "waitpid");
  if (simple_fail("wait", "waitpid")) { errno = mode_errno(); return -1; }
  return r(p, st, o);
}
int waitid(idtype_t t, id_t id, siginfo_t *i, int o) {
  static int (*r)(idtype_t, id_t, siginfo_t *, int) = 0; if (!r) r = dlsym(RTLD_NEXT, "waitid");
  if (simple_fail("wait", "waitid")) { errno = mode_errno(); return -1; }
  return r(t, id, i, o);
}
pid_t wait4(pid_t p, int *st, int o, struct rusage *ru) {
  static pid_t (*r)(pid_t, int *, int, struct rusage *) = 0; if (!r) r = dlsym(RTLD_NEXT, "wait4");
  if (simple_fail("wait", "wait4")) { errno = mode_errno(); return -1; }
  return r(p, st, o, ru);
}
// clock: G8_CLOCK_STEP=<seconds> (signed) shifts the answer instead of failing (a stepping clock); else errno.
int clock_gettime(clockid_t c, struct timespec *ts) {
  static int (*r)(clockid_t, struct timespec *) = 0; if (!r) r = dlsym(RTLD_NEXT, "clock_gettime");
  const char *name = c == CLOCK_BOOTTIME ? "clock:boottime" : c == CLOCK_MONOTONIC ? "clock:monotonic" : c == CLOCK_REALTIME ? "clock:realtime" : "clock:other";
  if (!inside && decide("clock", name)) {
    const char *stp = getenv("G8_CLOCK_STEP");
    if (stp) { int v = r(c, ts); if (v == 0) ts->tv_sec += atol(stp); return v; }
    errno = mode_errno(); return -1;
  }
  return r(c, ts);
}
