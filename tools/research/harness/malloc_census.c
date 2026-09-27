// W0.3 malloc census: an LD_PRELOAD shim that counts C-heap calls by the
// class of their direct caller and forwards to glibc's allocator.
//
// Build:  gcc -O2 -shared -fPIC -o malloc_census.so malloc_census.c -lpthread
// Run:    MALLOC_CENSUS_OUT=<prefix> MALLOC_CENSUS_GMP=<off:len,...>
//         MALLOC_CENSUS_OTHER_C=<off:len,...> LD_PRELOAD=./malloc_census.so <exe>
// Classes of the caller address (__builtin_return_address(0)):
//   gmp     inside a GMP/MPFR function that calls the heap directly;
//   other_c inside any other C function of the executable that does;
//   rust    anywhere else inside the executable (Rust's System allocator may
//           tail-jump to malloc, so the caller is arbitrary Rust code);
//   shlib   outside the executable (libc, libgcc, ...).
// Ranges are hex offsets from census_ranges.sh, relocated at load with
// dl_iterate_phdr. Counters are per thread and summed at exit into
// <prefix>.<pid>.json. Instrumentation only: allocation behaviour is glibc's.
#define _GNU_SOURCE
#include <link.h>
#include <pthread.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <unistd.h>

extern void *__libc_malloc(size_t);
extern void *__libc_calloc(size_t, size_t);
extern void *__libc_realloc(void *, size_t);
extern void __libc_free(void *);
extern void *__libc_memalign(size_t, size_t);

enum { RUST, GMP, OTHER_C, SHLIB, NCLASS };
enum { MALLOC, CALLOC, REALLOC, FREE, MEMALIGN, NOP };
static const char *class_names[NCLASS] = {"rust", "gmp", "other_c", "shlib"};
static const char *op_names[NOP] = {"malloc", "calloc", "realloc", "free", "memalign"};

struct counters {
    uint64_t calls[NCLASS][NOP];
    uint64_t bytes[NCLASS][NOP];
    struct counters *next;
};
static __thread struct counters *tls;
static struct counters *all;
static pthread_mutex_t all_lock = PTHREAD_MUTEX_INITIALIZER;

#define MAX_RANGES 256
static uintptr_t lo[NCLASS][MAX_RANGES], hi[NCLASS][MAX_RANGES];
static int nranges[NCLASS];
static uintptr_t base, exe_lo = UINTPTR_MAX, exe_hi;
static int ready;

static int first_object(struct dl_phdr_info *info, size_t size, void *data) {
    (void)size; (void)data;
    base = info->dlpi_addr;
    for (int i = 0; i < info->dlpi_phnum; i++) {
        const ElfW(Phdr) *p = &info->dlpi_phdr[i];
        if (p->p_type != PT_LOAD) continue;
        uintptr_t a = base + p->p_vaddr, b = a + p->p_memsz;
        if (a < exe_lo) exe_lo = a;
        if (b > exe_hi) exe_hi = b;
    }
    return 1; /* the main program is reported first */
}

static void parse(int class, const char *spec) {
    while (spec && *spec && nranges[class] < MAX_RANGES) {
        char *end;
        uintptr_t off = strtoull(spec, &end, 16);
        if (*end != ':') return;
        uintptr_t len = strtoull(end + 1, &end, 16);
        lo[class][nranges[class]] = base + off;
        hi[class][nranges[class]] = base + off + len;
        nranges[class]++;
        spec = *end == ',' ? end + 1 : NULL;
    }
}

__attribute__((constructor)) static void census_init(void) {
    dl_iterate_phdr(first_object, NULL);
    parse(GMP, getenv("MALLOC_CENSUS_GMP"));
    parse(OTHER_C, getenv("MALLOC_CENSUS_OTHER_C"));
    ready = 1;
}

static inline int classify(uintptr_t ra) {
    if (ra < exe_lo || ra >= exe_hi) return SHLIB;
    for (int c = GMP; c <= OTHER_C; c++)
        for (int i = 0; i < nranges[c]; i++)
            if (ra >= lo[c][i] && ra < hi[c][i]) return c;
    return RUST;
}

static inline struct counters *mine(void) {
    struct counters *c = tls;
    if (__builtin_expect(c == NULL, 0)) {
        c = __libc_calloc(1, sizeof *c);
        if (!c) return NULL;
        pthread_mutex_lock(&all_lock);
        c->next = all;
        all = c;
        pthread_mutex_unlock(&all_lock);
        tls = c;
    }
    return c;
}

static inline void count(uintptr_t ra, int op, size_t bytes) {
    if (!ready) return;
    struct counters *c = mine();
    if (!c) return;
    int class = classify(ra);
    c->calls[class][op]++;
    c->bytes[class][op] += bytes;
}

#define RA ((uintptr_t)__builtin_return_address(0))

void *malloc(size_t n) {
    count(RA, MALLOC, n);
    return __libc_malloc(n);
}
void *calloc(size_t a, size_t b) {
    count(RA, CALLOC, a * b);
    return __libc_calloc(a, b);
}
void *realloc(void *p, size_t n) {
    count(RA, REALLOC, n);
    return __libc_realloc(p, n);
}
void free(void *p) {
    if (p) count(RA, FREE, 0);
    __libc_free(p);
}
void *memalign(size_t a, size_t n) {
    count(RA, MEMALIGN, n);
    return __libc_memalign(a, n);
}
void *aligned_alloc(size_t a, size_t n) {
    count(RA, MEMALIGN, n);
    return __libc_memalign(a, n);
}
int posix_memalign(void **out, size_t a, size_t n) {
    count(RA, MEMALIGN, n);
    void *p = __libc_memalign(a, n);
    if (!p) return 12; /* ENOMEM */
    *out = p;
    return 0;
}

__attribute__((destructor)) static void census_dump(void) {
    const char *path = getenv("MALLOC_CENSUS_OUT");
    if (!path) return;
    uint64_t calls[NCLASS][NOP] = {{0}}, bytes[NCLASS][NOP] = {{0}};
    int threads = 0;
    pthread_mutex_lock(&all_lock);
    for (struct counters *c = all; c; c = c->next, threads++)
        for (int k = 0; k < NCLASS; k++)
            for (int o = 0; o < NOP; o++) {
                calls[k][o] += c->calls[k][o];
                bytes[k][o] += c->bytes[k][o];
            }
    pthread_mutex_unlock(&all_lock);
    char name[4096];
    snprintf(name, sizeof name, "%s.%d.json", path, (int)getpid());
    FILE *f = fopen(name, "w");
    if (!f) return;
    fprintf(f, "{\"schema\":\"rustred.malloc-census.v2\",\"threads\":%d,\"ranges\":{\"gmp\":%d,\"other_c\":%d},\"classes\":{",
            threads, nranges[GMP], nranges[OTHER_C]);
    for (int k = 0; k < NCLASS; k++) {
        fprintf(f, "%s\"%s\":{", k ? "," : "", class_names[k]);
        for (int o = 0; o < NOP; o++)
            fprintf(f, "%s\"%s\":{\"calls\":%llu,\"bytes\":%llu}", o ? "," : "", op_names[o],
                    (unsigned long long)calls[k][o], (unsigned long long)bytes[k][o]);
        fprintf(f, "}");
    }
    fprintf(f, "}}\n");
    fclose(f);
}
