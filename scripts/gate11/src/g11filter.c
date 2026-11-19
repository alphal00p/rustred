// gate11 LD_PRELOAD LISTING FILTER (test harness only, ONLY inside private pid namespaces).
// Rewrites the /proc ROOT listing (and, with G11_TASKS=1, every /proc/<pid>/task listing) seen by BOTH enumerations of
// the supervisor (glibc readdir64/readdir after opendir, and raw syscall(SYS_getdents64) on an fd opened by open/openat),
// IN AGREEMENT, without any error: the gate10 G10-B1 "agreed truncation" generalised to a FILTER.
// Active only in a process whose argv[1] equals $G11_ARGV1 (default "run": the supervisor; never its shims / jobs) and,
// if $G11_ARM is set, only while that file exists.
//   G11_MODE = drop (default): entries whose name is a pid listed in $G11_DROP_FILE (whitespace separated; re-read at
//              every listing, so the harness can add a pid later) or in $G11_DROP (comma list) are removed;
//              droprange: entries with $G11_LO <= pid <= $G11_HI are removed;
//              dup: every entry is returned twice; reorder: entries in reverse order (within one getdents buffer);
//              dropk: the k-th numeric entry ($G11_K, 0-based) of every listing is removed (drop-one for every entry);
//              keepk: only the first $G11_K numeric entries are kept (gate10's truncation, both enumerations).
//   G11_LOG: one line per filtered listing (pid of the caller, path, entries in / out, dropped names).
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
#include <sys/syscall.h>
#include <sys/types.h>
#include <ctype.h>

static int enabled = -1;
static __thread int inside = 0;
static pthread_mutex_t mu = PTHREAD_MUTEX_INITIALIZER;
#define MAXFD 65536
static char *fdpath[MAXFD];
#define MAXDIR 512
struct dstate { DIR *d; char *path; struct dirent64 *ents; int n, pos, loaded; };
static struct dstate ds[MAXDIR];

static int is_enabled(void) {
  if (enabled >= 0) return enabled;
  const char *want = getenv("G11_ARGV1"); if (!want) want = "run";
  char buf[4096]; int fd = syscall(SYS_openat, AT_FDCWD, "/proc/self/cmdline", O_RDONLY | O_CLOEXEC);
  int ok = 0;
  if (fd >= 0) { ssize_t n = syscall(SYS_read, fd, buf, sizeof buf - 1); syscall(SYS_close, fd);
    if (n > 0) { buf[n] = 0; size_t l0 = strlen(buf); if ((ssize_t)l0 + 1 < n && !strcmp(buf + l0 + 1, want)) ok = 1; } }
  enabled = ok;
  return enabled;
}
static int armed(void) {
  const char *arm = getenv("G11_ARM");
  return !arm || syscall(SYS_faccessat, AT_FDCWD, arm, F_OK, 0) == 0;
}
static int is_numeric(const char *s) { if (!*s) return 0; for (; *s; s++) if (!isdigit((unsigned char)*s)) return 0; return 1; }
// is this directory one we rewrite?
static int target_dir(const char *p) {
  if (!p) return 0;
  if (!strcmp(p, "/proc") || !strcmp(p, "/proc/")) return 1;
  if (getenv("G11_TASKS") && !strncmp(p, "/proc/", 6)) {
    const char *q = p + 6; while (isdigit((unsigned char)*q)) q++;
    if (q > p + 6 && (!strcmp(q, "/task") || !strcmp(q, "/task/"))) return 1;
  }
  return 0;
}
static const char *mode(void) { const char *m = getenv("G11_MODE"); return m ? m : "drop"; }
static int in_list(const char *list, long v, char sep) {
  if (!list) return 0; const char *p = list;
  while (*p) { while (*p && (*p == sep || isspace((unsigned char)*p))) p++; if (!*p) break;
    char *e; long x = strtol(p, &e, 10); if (e == p) { p++; continue; } if (x == v) return 1; p = e; }
  return 0;
}
static char dropbuf[8192];
static void load_dropfile(void) {
  dropbuf[0] = 0; const char *f = getenv("G11_DROP_FILE"); if (!f) return;
  int fd = syscall(SYS_openat, AT_FDCWD, f, O_RDONLY | O_CLOEXEC); if (fd < 0) return;
  ssize_t n = syscall(SYS_read, fd, dropbuf, sizeof dropbuf - 1); syscall(SYS_close, fd);
  dropbuf[n > 0 ? n : 0] = 0;
}
// decide whether numeric entry #idx (0-based) with value v is dropped
static int drop_it(long v, int idx) {
  const char *m = mode();
  if (!strcmp(m, "drop")) return in_list(dropbuf, v, ' ') || in_list(getenv("G11_DROP"), v, ',');
  if (!strcmp(m, "droprange")) { long lo = atol(getenv("G11_LO") ? getenv("G11_LO") : "0"), hi = atol(getenv("G11_HI") ? getenv("G11_HI") : "0"); return v >= lo && v <= hi; }
  if (!strcmp(m, "dropk")) return idx == atoi(getenv("G11_K") ? getenv("G11_K") : "0");
  if (!strcmp(m, "keepk")) return idx >= atoi(getenv("G11_K") ? getenv("G11_K") : "0");
  return 0;
}
static void logf_(const char *what, const char *path, int in, int out, const char *dropped) {
  const char *lg = getenv("G11_LOG"); if (!lg) return;
  int fd = syscall(SYS_openat, AT_FDCWD, lg, O_WRONLY | O_APPEND | O_CREAT | O_CLOEXEC, 0600); if (fd < 0) return;
  char m[2048]; int k = snprintf(m, sizeof m, "pid %d tid %ld %s %s mode %s in %d out %d dropped [%s]\n", getpid(),
                                 (long)syscall(SYS_gettid), what, path, mode(), in, out, dropped);
  if (k > 0) { if (syscall(SYS_write, fd, m, (size_t)(k < (int)sizeof m ? k : (int)sizeof m - 1))) {} }
  syscall(SYS_close, fd);
}

// ---------------------------------------------------------------- fd tracking for raw getdents64
static void remember_fd(int fd, const char *p) {
  if (fd < 0 || fd >= MAXFD || inside || !is_enabled() || !target_dir(p)) return;
  pthread_mutex_lock(&mu); free(fdpath[fd]); fdpath[fd] = strdup(p); pthread_mutex_unlock(&mu);
}
static void forget_fd(int fd) {
  if (fd < 0 || fd >= MAXFD || !fdpath[fd]) return;
  pthread_mutex_lock(&mu); free(fdpath[fd]); fdpath[fd] = 0; pthread_mutex_unlock(&mu);
}
#define OPENWRAP(name)                                                                                                  \
  int name(const char *p, int f, ...) {                                                                                \
    mode_t mo = 0; if (f & (O_CREAT | O_TMPFILE)) { va_list a; va_start(a, f); mo = va_arg(a, int); va_end(a); }      \
    static int (*r)(const char *, int, ...) = 0; if (!r) r = dlsym(RTLD_NEXT, #name);                                 \
    int fd = r(p, f, mo); remember_fd(fd, p); return fd; }
OPENWRAP(open)
OPENWRAP(open64)
#define OPENATWRAP(name)                                                                                                \
  int name(int d, const char *p, int f, ...) {                                                                         \
    mode_t mo = 0; if (f & (O_CREAT | O_TMPFILE)) { va_list a; va_start(a, f); mo = va_arg(a, int); va_end(a); }      \
    static int (*r)(int, const char *, int, ...) = 0; if (!r) r = dlsym(RTLD_NEXT, #name);                            \
    int fd = r(d, p, f, mo); remember_fd(fd, p); return fd; }
OPENATWRAP(openat)
OPENATWRAP(openat64)
int close(int fd) {
  forget_fd(fd);
  static int (*r)(int) = 0; if (!r) r = dlsym(RTLD_NEXT, "close");
  return r(fd);
}

// rewrite one getdents64 buffer in place; returns the new length
struct lrec { unsigned long long ino; long long off; unsigned short reclen; unsigned char type; char name[256]; };
static long rewrite_buf(char *buf, long n, const char *path) {
  static __thread struct lrec recs[4096]; int cnt = 0, idx_num = 0, in = 0, out = 0;
  char dropped[1024]; dropped[0] = 0;
  long off = 0;
  while (off + 19 <= n && cnt < 4096) {
    unsigned short reclen; memcpy(&reclen, buf + off + 16, 2);
    if (reclen < 20 || off + reclen > n) return n;  // malformed: leave untouched
    struct lrec *r = &recs[cnt];
    memcpy(&r->ino, buf + off, 8); memcpy(&r->off, buf + off + 8, 8); r->type = (unsigned char)buf[off + 18];
    strncpy(r->name, buf + off + 19, 255); r->name[255] = 0;
    off += reclen;
    if (is_numeric(r->name)) {
      in++;
      if (drop_it(atol(r->name), idx_num++)) { if (strlen(dropped) < 1000) { strcat(dropped, r->name); strcat(dropped, " "); } continue; }
    }
    cnt++;
  }
  // order / duplication
  int order[8192]; int m = 0; const char *md = mode();
  if (!strcmp(md, "reorder")) { for (int i = cnt - 1; i >= 0; i--) order[m++] = i; }
  else if (!strcmp(md, "dup")) { for (int i = 0; i < cnt && m < 8190; i++) { order[m++] = i; if (is_numeric(recs[i].name)) order[m++] = i; } }
  else { for (int i = 0; i < cnt; i++) order[m++] = i; }
  long w = 0;
  for (int j = 0; j < m; j++) {
    struct lrec *r = &recs[order[j]];
    size_t nl = strlen(r->name); unsigned short rl = (unsigned short)((19 + nl + 1 + 7) & ~7);
    if (w + rl > n) break;  // cannot grow past the buffer: duplicates are dropped at the end
    memset(buf + w, 0, rl);
    memcpy(buf + w, &r->ino, 8); memcpy(buf + w + 8, &r->off, 8); memcpy(buf + w + 16, &rl, 2); buf[w + 18] = (char)r->type;
    memcpy(buf + w + 19, r->name, nl);
    w += rl;
    if (is_numeric(r->name)) out++;
  }
  logf_("getdents64", path, in, out, dropped);
  return w;
}

long syscall(long nr, ...) {
  va_list a; va_start(a, nr);
  long x1 = va_arg(a, long), x2 = va_arg(a, long), x3 = va_arg(a, long), x4 = va_arg(a, long), x5 = va_arg(a, long), x6 = va_arg(a, long);
  va_end(a);
  static long (*r)(long, ...) = 0; if (!r) r = dlsym(RTLD_NEXT, "syscall");
  long v = r(nr, x1, x2, x3, x4, x5, x6);
  if (nr == SYS_getdents64 && v > 0 && !inside && is_enabled() && armed() && x1 >= 0 && x1 < MAXFD && fdpath[x1]) {
    char p[1100]; int have = 0;
    pthread_mutex_lock(&mu); if (fdpath[x1]) { strncpy(p, fdpath[x1], sizeof p - 1); p[sizeof p - 1] = 0; have = 1; } pthread_mutex_unlock(&mu);
    if (have) {
      inside = 1;
      load_dropfile();
      // note: entry numbering for dropk / keepk restarts per buffer (one buffer holds the whole listing in a test ns)
      long w = rewrite_buf((char *)x2, v, p);
      inside = 0;
      if (w == 0 && v > 0) {
        // every entry dropped from this buffer: ask for the next one instead of signalling the end
        return syscall(nr, x1, x2, x3, x4, x5, x6);
      }
      return w;
    }
  }
  return v;
}

// ---------------------------------------------------------------- glibc readdir: load all entries, serve rewritten
static struct dstate *find_ds(DIR *d) { for (int i = 0; i < MAXDIR; i++) if (ds[i].d == d) return &ds[i]; return 0; }
DIR *opendir(const char *p) {
  static DIR *(*r)(const char *) = 0; if (!r) r = dlsym(RTLD_NEXT, "opendir");
  DIR *d = r(p);
  if (d && !inside && is_enabled() && target_dir(p)) {
    pthread_mutex_lock(&mu);
    for (int i = 0; i < MAXDIR; i++) if (!ds[i].d) { ds[i].d = d; ds[i].path = strdup(p); ds[i].ents = 0; ds[i].n = ds[i].pos = ds[i].loaded = 0; break; }
    pthread_mutex_unlock(&mu);
  }
  return d;
}
int closedir(DIR *d) {
  pthread_mutex_lock(&mu);
  struct dstate *s = find_ds(d); if (s) { free(s->path); free(s->ents); memset(s, 0, sizeof *s); }
  pthread_mutex_unlock(&mu);
  static int (*r)(DIR *) = 0; if (!r) r = dlsym(RTLD_NEXT, "closedir");
  return r(d);
}
static void load_all(struct dstate *s, struct dirent64 *(*real)(DIR *)) {
  int cap = 256, n = 0; struct dirent64 *v = malloc(sizeof(struct dirent64) * cap);
  struct dirent64 *e; int idx_num = 0, in = 0; char dropped[1024]; dropped[0] = 0;
  load_dropfile();
  errno = 0;
  while ((e = real(s->d))) {
    int num = is_numeric(e->d_name);
    if (num) { in++; if (drop_it(atol(e->d_name), idx_num++)) { if (strlen(dropped) < 1000) { strcat(dropped, e->d_name); strcat(dropped, " "); } continue; } }
    int reps = (num && !strcmp(mode(), "dup")) ? 2 : 1;
    for (int k = 0; k < reps; k++) { if (n == cap) { cap *= 2; v = realloc(v, sizeof(struct dirent64) * cap); } v[n++] = *e; }
  }
  if (!strcmp(mode(), "reorder")) for (int i = 0, j = n - 1; i < j; i++, j--) { struct dirent64 t = v[i]; v[i] = v[j]; v[j] = t; }
  int out = 0; for (int i = 0; i < n; i++) if (is_numeric(v[i].d_name)) out++;
  s->ents = v; s->n = n; s->pos = 0; s->loaded = 1;
  logf_("readdir", s->path, in, out, dropped);
}
struct dirent64 *readdir64(DIR *d) {
  static struct dirent64 *(*r)(DIR *) = 0; if (!r) r = dlsym(RTLD_NEXT, "readdir64");
  if (inside || !is_enabled() || !armed()) return r(d);
  pthread_mutex_lock(&mu); struct dstate *s = find_ds(d); pthread_mutex_unlock(&mu);
  if (!s) return r(d);
  if (!s->loaded) { inside = 1; load_all(s, r); inside = 0; }
  if (s->pos >= s->n) { errno = 0; return NULL; }
  return &s->ents[s->pos++];
}
struct dirent *readdir(DIR *d) { return (struct dirent *)readdir64(d); }
