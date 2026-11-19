// gate8: after a CLOEXEC error pipe reports a successful execve (what std::process::Command::spawn waits for), is
// /proc/<child>/environ still EMPTY (the exec window: env bounds set later, in create_elf_tables)? N trials; for each:
// empty at once? still empty after 1 ms? after 10 ms? Children are our own (killed + reaped). Namespaces only.
#define _GNU_SOURCE
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <unistd.h>
#include <fcntl.h>
#include <signal.h>
#include <time.h>
#include <sys/wait.h>
static long envlen(pid_t p){ char f[64]; snprintf(f,sizeof f,"/proc/%d/environ",p); int fd=open(f,O_RDONLY); if(fd<0) return -1; char b[8192]; long n=read(fd,b,sizeof b); close(fd); return n; }
static void ms(long m){ struct timespec t={m/1000,(m%1000)*1000000L}; nanosleep(&t,0); }
int main(int argc,char**argv){
  int N = argc>1? atoi(argv[1]) : 200; int e0=0,e1=0,e10=0;
  for(int i=0;i<N;i++){
    int pp[2]; if(pipe2(pp,O_CLOEXEC)) return 1;
    pid_t c=fork();
    if(c==0){ close(pp[0]); char*av[]={"sleep","30",0}; char*ev[]={"RUSTRED_CAMPAIGN_LOCK_TOKEN=abc",0}; execve("/run/current-system/sw/bin/sleep",av,ev); execve(argv[2]?argv[2]:"/bin/sleep",av,ev); _exit(127); }
    close(pp[1]); char x; while(read(pp[0],&x,1)>0){} close(pp[0]);
    long a=envlen(c); if(a==0){ e0++; ms(1); long b=envlen(c); if(b==0){ e1++; ms(9); if(envlen(c)==0) e10++; } }
    kill(c,SIGKILL); waitpid(c,0,0);
  }
  printf("trials %d empty_at_once %d still_empty_1ms %d still_empty_10ms %d\n",N,e0,e1,e10); return 0; }
