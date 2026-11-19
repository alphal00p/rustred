// gate12 g12stream MS LIFE_MS [z] : test helper for tests/gate12_walk.rs, ONLY inside private pid namespaces.
// Every MS ms (0: back to back) a NEW thread forks a child and EXITS at once, so the child is re-parented while the
// completeness walk may be reading this process. The child sleeps LIFE_MS ms and exits (SIGCHLD ignored: auto-reaped). Each
// child's pid is written to stdout (one line, written before the forking thread exits).
//   (default) the leader lives: children are re-parented to the oldest live thread, the LEADER (lowest tid: read first).
//   z         the leader exits first (zombie leader); one idle worker W0 lives: children go to W0 (a NON-leader thread).
#define _GNU_SOURCE
#include <pthread.h>
#include <signal.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <time.h>
#include <unistd.h>

static int ms, life_ms;

static void *idle(void *a) { (void)a; for (;;) pause(); return NULL; }

static void *forker(void *a) {
  (void)a;
  pid_t c = fork();
  if (c == 0) {
    struct timespec t = {life_ms / 1000, (long)(life_ms % 1000) * 1000000L};
    nanosleep(&t, NULL);
    _exit(0);
  }
  if (c > 0) {
    char b[32];
    int n = snprintf(b, sizeof b, "%d\n", (int)c);
    if (write(1, b, n) != n) _exit(1);
  }
  return NULL;  // this thread exits right after the fork
}

static void *stream(void *a) {
  (void)a;
  pthread_attr_t at;
  pthread_attr_init(&at);
  pthread_attr_setdetachstate(&at, PTHREAD_CREATE_DETACHED);
  for (;;) {
    pthread_t t;
    if (pthread_create(&t, &at, forker, NULL) != 0) usleep(1000);
    if (ms > 0) usleep(ms * 1000); else usleep(200);
  }
  return NULL;
}

int main(int argc, char **argv) {
  if (argc < 3) return 2;
  ms = atoi(argv[1]);
  life_ms = atoi(argv[2]);
  signal(SIGCHLD, SIG_IGN);
  pthread_t t;
  if (argc > 3 && !strcmp(argv[3], "z")) {
    if (pthread_create(&t, NULL, idle, NULL)) return 1;  // W0: the oldest live thread once the leader is gone
    usleep(100000);
    if (pthread_create(&t, NULL, stream, NULL)) return 1;
    pthread_exit(NULL);
  }
  if (pthread_create(&t, NULL, stream, NULL)) return 1;
  for (;;) pause();
}
