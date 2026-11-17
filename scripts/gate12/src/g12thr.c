// gate12 g12thr PIDFILE MARKER MODE [ARG] : test helper, ONLY inside private pid namespaces (gate12 item 1, G11-M1 variants).
// Run with an EMPTY environment (env -i): this process and every intermediate are UNMARKED; only the final sleep carries
// RR_CAMPAIGN_JOB=MARKER. setsid() first. Writes "<marked pid> <top pid>" to PIDFILE once the marked child exists.
//   depth N   : N nested levels: a NON-LEADER thread of this process forks a child process, whose own non-leader thread
//               forks the next level, ...; the non-leader thread of level N forks the marked sleep (N = 2, 3, ...).
//   texit     : leader alive, worker W1 alive; worker W2 forks the marked sleep and EXITS at once (re-parented to the
//               oldest live thread: the leader).
//   texitz    : the leader exits first (zombie leader), W1 alive; W2 forks the marked sleep and EXITS at once (re-parented
//               to the live NON-LEADER thread W1).
//   churn MS  : W1 forks the marked sleep and lives on; the leader creates a short-lived thread every MS ms (0: back to
//               back, 4 churners) for ever: the task list changes all the time, the marked child's parent thread does not.
//   hop MS    : the leader exits (zombie leader); thread T0 forks the marked sleep; every thread Tk starts T(k+1), sleeps
//               MS ms and exits: the marked child is re-parented from thread to thread for ever (and the task list churns).
#define _GNU_SOURCE
#include <pthread.h>
#include <signal.h>
#include <stdatomic.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <sys/prctl.h>
#include <sys/types.h>
#include <sys/wait.h>
#include <unistd.h>

static const char *pidfile, *marker, *mode;
static int arg;
static int top;

static pid_t spawn_marked(void) {
  pid_t c = fork();
  if (c == 0) {
    char env[512];
    snprintf(env, sizeof env, "RR_CAMPAIGN_JOB=%s", marker);
    char *envp[] = {env, "PATH=/usr/bin:/bin", NULL};
    char *argv[] = {"sleep", "3600", NULL};
    execve("/run/current-system/sw/bin/sleep", argv, envp);
    execve("/bin/sleep", argv, envp);
    execve("/usr/bin/sleep", argv, envp);
    _exit(127);
  }
  return c;
}

static void write_pids(pid_t m) {
  char tmp[1024];
  snprintf(tmp, sizeof tmp, "%s.tmp", pidfile);
  FILE *f = fopen(tmp, "w");
  if (!f) return;
  fprintf(f, "%d %d\n", (int)m, top);
  fclose(f);
  rename(tmp, pidfile);
}

static void forever(void) { for (;;) pause(); }

// ---- depth
static int level;
static void *depth_worker(void *a) {
  (void)a;
  if (level >= arg) {
    pid_t m = spawn_marked();
    usleep(200000);
    write_pids(m);
    forever();
  }
  pid_t c = fork();
  if (c == 0) {
    // the child: one thread (a copy of this worker); start the next level's non-leader thread
    level++;
    pthread_t t;
    if (pthread_create(&t, NULL, depth_worker, NULL)) _exit(1);
    forever();
  }
  forever();
  return NULL;
}

// ---- texit / texitz
static void *idle_worker(void *a) { (void)a; forever(); return NULL; }
static void *fork_and_exit(void *a) {
  (void)a;
  if (!strcmp(mode, "texitz")) usleep(300000);  // after the leader's exit
  pid_t m = spawn_marked();
  write_pids(m);
  return NULL;  // this thread exits right after the fork: the marked child is re-parented
}

// ---- churn
static void *marked_parent(void *a) {
  (void)a;
  pid_t m = spawn_marked();
  usleep(100000);
  write_pids(m);
  forever();
  return NULL;
}
static void *nothing(void *a) { (void)a; return NULL; }
static void *churner(void *a) {
  (void)a;
  pthread_attr_t at;
  pthread_attr_init(&at);
  pthread_attr_setdetachstate(&at, PTHREAD_CREATE_DETACHED);
  for (;;) {
    pthread_t t;
    if (pthread_create(&t, &at, nothing, NULL) != 0) usleep(1000);
    if (arg > 0) usleep(arg * 1000);
  }
  return NULL;
}

// ---- hop
static atomic_int hop_written;
static void *hop_thread(void *a) {
  long first = (long)a;
  if (first) {
    pid_t m = spawn_marked();
    usleep(100000);
    write_pids(m);
    atomic_store(&hop_written, 1);
  }
  pthread_attr_t at;
  pthread_attr_init(&at);
  pthread_attr_setdetachstate(&at, PTHREAD_CREATE_DETACHED);
  pthread_t t;
  while (pthread_create(&t, &at, hop_thread, (void *)0L) != 0) usleep(1000);
  if (arg > 0) usleep(arg * 1000);
  return NULL;  // exit: the marked child goes to the oldest live thread (the successor)
}

int main(int argc, char **argv) {
  if (argc < 4) { fprintf(stderr, "usage: g12thr PIDFILE MARKER MODE [ARG]\n"); return 2; }
  pidfile = argv[1]; marker = argv[2]; mode = argv[3]; arg = argc > 4 ? atoi(argv[4]) : 0;
  setsid();
  top = getpid();
  signal(SIGCHLD, SIG_DFL);
  pthread_t t;
  if (!strcmp(mode, "depth")) {
    if (arg < 1) arg = 2;
    level = 1;
    if (pthread_create(&t, NULL, depth_worker, NULL)) return 1;
    forever();
  } else if (!strcmp(mode, "texit")) {
    if (pthread_create(&t, NULL, idle_worker, NULL)) return 1;
    usleep(50000);
    if (pthread_create(&t, NULL, fork_and_exit, NULL)) return 1;
    forever();
  } else if (!strcmp(mode, "texitz")) {
    if (pthread_create(&t, NULL, idle_worker, NULL)) return 1;
    pthread_t t2;
    if (pthread_create(&t2, NULL, fork_and_exit, NULL)) return 1;  // forks after the leader's exit below (it sleeps)
    pthread_exit(NULL);
  } else if (!strcmp(mode, "churn")) {
    if (pthread_create(&t, NULL, marked_parent, NULL)) return 1;
    int n = arg == 0 ? 4 : 1;
    for (int i = 0; i < n; i++) if (pthread_create(&t, NULL, churner, NULL)) return 1;
    forever();
  } else if (!strcmp(mode, "hop")) {
    if (pthread_create(&t, NULL, hop_thread, (void *)1L)) return 1;
    while (!atomic_load(&hop_written)) usleep(1000);
    pthread_exit(NULL);
  }
  fprintf(stderr, "g12thr: mode?\n");
  return 2;
}
