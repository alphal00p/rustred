// gate11 g11thr PIDFILE MARKER [MODE] : test helper, ONLY inside private pid namespaces.
// Run with an EMPTY environment (env -i): this process is UNMARKED. It calls setsid(), then
//   MODE "thread" (default): a pthread (a NON-LEADER thread) forks a child that execs /bin/sleep 3600 with
//        RR_CAMPAIGN_JOB=MARKER (a MARKED process whose parent is the non-leader thread: it is listed in
//        /proc/<self>/task/<tid>/children, not in /proc/<self>/task/<self>/children);
//   MODE "zleader": PR_SET_CHILD_SUBREAPER; a pthread forks an intermediate (marked) that forks the marked sleep and
//        exits; then the MAIN thread exits (pthread_exit: a zombie leader with a live thread), so the orphaned marked
//        sleep is re-parented to the live NON-LEADER thread of this subreaper.
// Writes "<marked pid> <self pid>" to PIDFILE once the marked child exists. Both threads then sleep forever.
#define _GNU_SOURCE
#include <pthread.h>
#include <signal.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <sys/prctl.h>
#include <sys/types.h>
#include <sys/wait.h>
#include <unistd.h>

static const char *pidfile, *marker, *mode;

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
  fprintf(f, "%d %d\n", (int)m, (int)getpid());
  fclose(f);
  rename(tmp, pidfile);
}

static void *worker(void *arg) {
  (void)arg;
  if (!strcmp(mode, "zleader")) {
    int p[2];
    if (pipe(p)) _exit(1);
    pid_t mid = fork();
    if (mid == 0) {
      close(p[0]);
      pid_t m = spawn_marked();
      if (write(p[1], &m, sizeof m) != sizeof m) _exit(1);
      _exit(0);  // the marked sleep is orphaned: re-parented to this subreaper's live thread
    }
    close(p[1]);
    pid_t m = 0;
    if (read(p[0], &m, sizeof m) != sizeof m) _exit(1);
    waitpid(mid, NULL, 0);
    usleep(200000);
    write_pids(m);
  } else {
    pid_t m = spawn_marked();
    usleep(100000);
    write_pids(m);
  }
  for (;;) pause();
  return NULL;
}

int main(int argc, char **argv) {
  if (argc < 3) { fprintf(stderr, "usage: g11thr PIDFILE MARKER [thread|zleader]\n"); return 2; }
  pidfile = argv[1]; marker = argv[2]; mode = argc > 3 ? argv[3] : "thread";
  setsid();
  signal(SIGCHLD, SIG_DFL);
  if (!strcmp(mode, "zleader")) prctl(PR_SET_CHILD_SUBREAPER, 1, 0, 0, 0);
  pthread_t t;
  if (pthread_create(&t, NULL, worker, NULL)) return 1;
  if (!strcmp(mode, "zleader")) {
    usleep(500000);
    pthread_exit(NULL);  // the leader exits; the worker thread lives on
  }
  for (;;) pause();
}
