// gate10 fork chain (test harness only, ONLY inside private pid namespaces).
// usage: g10chain FIFO LIFE_US [TRIGGER LIFE2_US]
// Double fork + setsid (leaves the job's session), then a CHAIN of processes: each incarnation sleeps LIFE_US (or LIFE2_US
// once the file TRIGGER exists), forks its successor and _exits. Every incarnation inherits the environment (the campaign
// marker of the job that started it) and holds the write end of FIFO; it writes one byte (non-blocking) per hop. Once the
// reader closes the FIFO the next write fails with EPIPE and the chain ends (the harness's clean-up). Hard end: 1800 s.
#define _GNU_SOURCE
#include <errno.h>
#include <fcntl.h>
#include <signal.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <time.h>
#include <unistd.h>

int main(int argc, char **argv) {
  if (argc < 3) { fprintf(stderr, "usage: g10chain FIFO LIFE_US [TRIGGER LIFE2_US]\n"); return 2; }
  long life = atol(argv[2]);
  const char *trig = argc >= 5 ? argv[3] : NULL;
  long life2 = argc >= 5 ? atol(argv[4]) : life;
  signal(SIGPIPE, SIG_IGN);
  int w = open(argv[1], O_WRONLY | O_CLOEXEC);
  if (w < 0) { perror("open fifo"); return 2; }
  fcntl(w, F_SETFL, fcntl(w, F_GETFL) | O_NONBLOCK);
  fcntl(w, F_SETFD, 0); // keep across the (non-)exec; we never exec anyway
  pid_t p = fork();
  if (p < 0) return 1;
  if (p > 0) _exit(0);
  setsid();
  int dn = open("/dev/null", O_RDWR);
  if (dn >= 0) { dup2(dn, 0); dup2(dn, 1); dup2(dn, 2); if (dn > 2) close(dn); }
  time_t t0 = time(NULL);
  for (;;) {
    if (write(w, "h", 1) < 0 && errno == EPIPE) _exit(0);
    if (time(NULL) - t0 > 1800) _exit(0);
    long l = (trig && access(trig, F_OK) == 0) ? life2 : life;
    if (l > 0) {
      struct timespec ts = { l / 1000000L, (l % 1000000L) * 1000L };
      while (nanosleep(&ts, &ts) != 0 && errno == EINTR) {}
    }
    pid_t c = fork();
    if (c < 0) { usleep(1000); continue; } // fork failed: stay this incarnation and retry
    if (c > 0) _exit(0);                     // the parent exits: the successor carries on
  }
}
