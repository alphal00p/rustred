// gate10 (i): g10zinit N CMD... : (inside a private pid ns, as a child of its pid 1) become a CHILD SUBREAPER, fork N
// children that exit at once and are NEVER reaped (N zombies), then run CMD as a child, wait for exactly it, exit with its
// status. The zombies stay until this process exits (the namespace then ends).
#define _GNU_SOURCE
#include <stdio.h>
#include <stdlib.h>
#include <unistd.h>
#include <sys/prctl.h>
#include <sys/wait.h>
int main(int argc, char **argv) {
  if (argc < 3) { fprintf(stderr, "usage: g10zinit N CMD...\n"); return 2; }
  int n = atoi(argv[1]);
  prctl(PR_SET_CHILD_SUBREAPER, 1, 0, 0, 0);
  for (int i = 0; i < n; i++) { pid_t p = fork(); if (p == 0) _exit(0); if (p < 0) { perror("fork"); break; } }
  pid_t c = fork();
  if (c == 0) { execvp(argv[2], argv + 2); perror("exec"); _exit(127); }
  int st = 0; if (waitpid(c, &st, 0) < 0) return 1;
  return WIFEXITED(st) ? WEXITSTATUS(st) : 128 + WTERMSIG(st);
}
