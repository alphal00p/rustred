// gate10 FIFO holder (test harness only): g10hold FIFO OUTFILE
// Creates nothing; opens FIFO for reading (O_RDWR first so the open does not block, then a second O_RDONLY descriptor and
// the O_RDWR one closed once a writer has appeared), drains it, and writes to OUTFILE:
//   "dead <unix time>"  when every writer has closed the FIFO (EOF: the kernel's ATOMIC answer that no process holds it)
//   "alive"             on SIGTERM while a writer still holds it (the reader then closes: the chain ends by EPIPE)
#define _GNU_SOURCE
#include <errno.h>
#include <fcntl.h>
#include <poll.h>
#include <signal.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <time.h>
#include <unistd.h>

static volatile sig_atomic_t term = 0;
static void on_term(int s) { (void)s; term = 1; }

static void put(const char *out, const char *what) {
  FILE *f = fopen(out, "w");
  if (!f) return;
  if (!strcmp(what, "dead")) fprintf(f, "dead %ld\n", (long)time(NULL)); else fprintf(f, "%s\n", what);
  fclose(f);
}

int main(int argc, char **argv) {
  if (argc != 3) { fprintf(stderr, "usage: g10hold FIFO OUTFILE\n"); return 2; }
  struct sigaction sa; memset(&sa, 0, sizeof sa); sa.sa_handler = on_term; sigaction(SIGTERM, &sa, NULL);
  int rw = open(argv[1], O_RDWR | O_NONBLOCK);
  if (rw < 0) { perror("open"); return 2; }
  int r = open(argv[1], O_RDONLY | O_NONBLOCK);
  if (r < 0) { perror("open ro"); return 2; }
  char b[4096];
  long got = 0;
  // phase 1: wait for the first byte (a writer exists), keeping rw open so that no EOF is seen before
  while (!term && got == 0) {
    ssize_t n = read(r, b, sizeof b);
    if (n > 0) got += n; else usleep(2000);
  }
  if (term) { put(argv[2], "alive"); return 0; }
  close(rw);
  put(argv[2], "running");
  for (;;) {
    if (term) { put(argv[2], "alive"); return 0; }
    struct pollfd p = { r, POLLIN, 0 };
    int k = poll(&p, 1, 100);
    if (k < 0 && errno == EINTR) continue;
    if (k > 0) {
      ssize_t n = read(r, b, sizeof b);
      if (n == 0) { put(argv[2], "dead"); return 0; }
      if (n > 0) got += n;
    }
  }
}
