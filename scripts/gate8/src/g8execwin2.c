// gate8: posix_spawn (glibc: clone(CLONE_VM|CLONE_VFORK), what std's Command uses) of a child with a marker; the time
// from posix_spawn's return until /proc/<child>/environ is non-empty (polled every ~20 us). Histogram. Namespaces only.
#define _GNU_SOURCE
#include <spawn.h>
#include <stdio.h>
#include <stdlib.h>
#include <unistd.h>
#include <fcntl.h>
#include <signal.h>
#include <time.h>
#include <sys/wait.h>
static double now(void){ struct timespec t; clock_gettime(CLOCK_MONOTONIC,&t); return t.tv_sec*1e6+t.tv_nsec/1e3; }
static long envlen(pid_t p){ char f[64]; snprintf(f,sizeof f,"/proc/%d/environ",p); int fd=open(f,O_RDONLY); if(fd<0) return -1; char b[256]; long n=read(fd,b,sizeof b); close(fd); return n; }
int main(int argc,char**argv){
  int N=atoi(argv[1]); const char*prog=argv[2]; long h[6]={0}; double mx=0;
  for(int i=0;i<N;i++){ pid_t c; char*av[]={"sleep","30",0}; char*ev[]={"RR_CAMPAIGN_JOB=x.0123456789abcdef:J:a1",0};
    if(posix_spawn(&c,prog,0,0,av,ev)) return 1; double t0=now(), dt=0;
    while(envlen(c)==0){ dt=now()-t0; if(dt>2e6) break; }
    dt=now()-t0; if(dt>mx) mx=dt;
    int k= dt<100?0: dt<1000?1: dt<5000?2: dt<20000?3: dt<100000?4:5; h[k]++;
    kill(c,SIGKILL); waitpid(c,0,0); }
  printf("trials %d window <0.1ms %ld 0.1-1ms %ld 1-5ms %ld 5-20ms %ld 20-100ms %ld >=100ms %ld max %.1f ms\n",N,h[0],h[1],h[2],h[3],h[4],h[5],mx/1000);
  return 0; }
