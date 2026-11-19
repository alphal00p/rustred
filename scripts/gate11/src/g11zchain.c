// gate11 g11zchain SECONDS LIFE_US : test helper, ONLY inside private pid namespaces. Detaches once (fork, parent exits),
// then a FORK CHAIN without setsid: each incarnation sleeps LIFE_US, forks its successor and _exits; the chain ends after
// SECONDS. Every exited incarnation becomes a zombie of whatever reaper it was re-parented to (the child subreaper).
#define _GNU_SOURCE
#include <stdlib.h>
#include <time.h>
#include <unistd.h>
static double now(void) { struct timespec t; clock_gettime(CLOCK_MONOTONIC, &t); return t.tv_sec + t.tv_nsec / 1e9; }
int main(int argc, char **argv) {
  if (argc < 3) return 2;
  double end = now() + atof(argv[1]); long life = atol(argv[2]);
  if (fork()) return 0;
  for (;;) {
    if (now() > end) _exit(0);
    if (life) usleep(life);
    pid_t p = fork();
    if (p < 0) { usleep(1000); continue; }
    if (p > 0) _exit(0);
  }
}
