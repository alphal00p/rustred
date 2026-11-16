// gate4 hostile descendants / decoys. ONLY ever run inside a private user+pid namespace (unshare -Urpf --mount-proc).
// usage: hostile MODE [args]
//  chain HOP [LOG]     : re-forking chain, parent exits after fork, setsid at every hop
//  chainroot HOP [LOG] : same + chdir("/") + close all fds at every hop
//  daemon              : double fork, setsid, chdir /, close all fds, pause
//  clearmark           : setsid, overwrite RR_CAMPAIGN_JOB in own environ memory (no exec), pause
//  nodump              : setsid, prctl(PR_SET_DUMPABLE,0), pause (marker kept; on the HOST /proc owner becomes root)
//  dropexec            : setsid, then exec /bin/sleep with an environment WITHOUT the marker (marker dropped by exec)
//  envcopy PID -- PROG ARGS : exec PROG with EXACTLY the environment of /proc/PID/environ, in a new session
//  flipexec HOP        : loop: exec self with marker, then without, every HOP ms (TOCTOU between check and send)
//  pidfill TARGET      : write ns_last_pid=TARGET-1 then fork+exec sleep: the child gets pid TARGET (pid reuse)
//  sleep               : pause forever (decoy body)
#define _GNU_SOURCE
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <unistd.h>
#include <signal.h>
#include <sys/prctl.h>
#include <sys/types.h>
#include <sys/wait.h>
#include <sys/stat.h>
#include <time.h>
#include <fcntl.h>
extern char **environ;
static void msleep(long ms){ struct timespec t={ms/1000,(ms%1000)*1000000L}; nanosleep(&t,0);}
static void closeall(void){ for(int fd=0; fd<1024; fd++) close(fd); int n=open("/dev/null",O_RDWR); (void)n; }
static const char*hoplog=0;
static void logline(const char*what){
  if(!hoplog) return;
  int fd=open(hoplog,O_WRONLY|O_APPEND|O_CREAT,0600); if(fd<0) return;
  char b[128]; int n=snprintf(b,sizeof b,"%s %d %d %d %ld\n",what,getpid(),getppid(),getsid(0),(long)time(0));
  if(write(fd,b,n)){} close(fd);
}
static void chain(int root, long hop_ms){
  for(;;){
    logline("hop");
    pid_t p=fork();
    if(p<0){ msleep(5); continue; }
    if(p>0) _exit(0);
    setsid();
    if(root){ if(chdir("/")){} const char*h=hoplog; closeall(); hoplog=h; }
    msleep(hop_ms);
  }
}
static int read_environ(int pid, char**env, int max, char*buf, size_t bl){
  char path[64]; snprintf(path,sizeof path,"/proc/%d/environ",pid);
  int fd=open(path,O_RDONLY); if(fd<0) return -1;
  ssize_t n=read(fd,buf,bl-1); close(fd); if(n<=0) return -1; buf[n]=0;
  int k=0; char*p=buf; while(p<buf+n && k<max-1){ env[k++]=p; p+=strlen(p)+1; }
  env[k]=0; return k;
}
int main(int argc,char**argv){
  if(argc<2) return 2;
  signal(SIGHUP,SIG_IGN); signal(SIGTERM,SIG_IGN); signal(SIGINT,SIG_IGN);
  const char*m=argv[1];
  if(!strcmp(m,"chain")||!strcmp(m,"chainroot")){
    long hop = argc>2? atol(argv[2]) : 20; if(argc>3) hoplog=argv[3];
    pid_t p=fork(); if(p) return 0;
    chain(!strcmp(m,"chainroot"),hop);
  }
  if(!strcmp(m,"daemon")){
    if(fork()) return 0; setsid(); if(fork()) _exit(0);
    if(chdir("/")){} closeall();
    for(;;) pause();
  }
  if(!strcmp(m,"clearmark")){
    if(fork()) return 0; setsid();
    for(char**e=environ; *e; e++) if(!strncmp(*e,"RR_CAMPAIGN_JOB=",16)) memset(*e,'X',strlen(*e));
    for(;;) pause();
  }
  if(!strcmp(m,"nodump")){
    if(fork()) return 0; setsid(); prctl(PR_SET_DUMPABLE,0,0,0,0);
    for(;;) pause();
  }
  if(!strcmp(m,"dropexec")){
    if(fork()) return 0; setsid();
    char*env[]={"PATH=/usr/bin:/bin",0}; char*av[]={"sleep","100000",0};
    execve(argv[2]?argv[2]:"/bin/sleep",av,env); return 7;
  }
  if(!strcmp(m,"envcopy")){ // envcopy PID -- PROG ARGS
    if(argc<5) return 4; int pid=atoi(argv[2]); static char buf[1<<16]; char*env[512];
    if(read_environ(pid,env,512,buf,sizeof buf)<0) return 5;
    setsid(); execve(argv[4], argv+4, env); return 6;
  }
  if(!strcmp(m,"flipexec")){ // flipexec HOP STATE(0|1) : alternate exec with / without marker, same pid
    long hop = argc>2? atol(argv[2]) : 5; int st = argc>3? atoi(argv[3]) : 1;
    if(argc<=3){ if(fork()) return 0; setsid(); }
    msleep(hop);
    char hopb[32], stb[4]; snprintf(hopb,sizeof hopb,"%ld",hop); snprintf(stb,sizeof stb,"%d",!st);
    char*av[]={argv[0],"flipexec",hopb,stb,0};
    if(st){ // next exec without marker
      static char*env[512]; int k=0; for(char**e=environ;*e&&k<510;e++) if(strncmp(*e,"RR_CAMPAIGN_JOB=",16)) env[k++]=*e;
      static char keep[512]; const char*mk=getenv("HOSTILE_MARK"); if(mk){ snprintf(keep,sizeof keep,"HOSTILE_MARK=%s",mk); }
      env[k]=0; execve("/proc/self/exe",av,env);
    } else {
      static char*env[512]; int k=0; for(char**e=environ;*e&&k<509;e++) env[k++]=*e;
      static char mk[512]; const char*v=getenv("HOSTILE_MARK"); if(v){ snprintf(mk,sizeof mk,"RR_CAMPAIGN_JOB=%s",v); env[k++]=mk; }
      env[k]=0; execve("/proc/self/exe",av,env);
    }
    return 8;
  }
  if(!strcmp(m,"pidfill")){ // pidfill TARGET PROG ARGS...: the forked child gets pid TARGET if free
    if(argc<4) return 4; int t=atoi(argv[2]);
    for(int tries=0; tries<50; tries++){
      int fd=open("/proc/sys/kernel/ns_last_pid",O_WRONLY); if(fd<0){ perror("ns_last_pid"); return 9; }
      char b[32]; int n=snprintf(b,sizeof b,"%d",t-1); if(write(fd,b,n)<0){ perror("write"); close(fd); return 10; } close(fd);
      pid_t p=fork();
      if(p==0){ setsid(); int dn=open("/dev/null",O_RDWR); dup2(dn,0); dup2(dn,1); dup2(dn,2); execv(argv[3], argv+3); _exit(11); }
      if(p==t){ printf("%d\n",p); return 0; }
      kill(p,SIGKILL); waitpid(p,0,0);
    }
    return 12;
  }
  if(!strcmp(m,"envraw")){ // envraw K=V ... -- prog args : setsid, exec with exactly this env array (duplicates kept)
    int i=2; char*env[64]; int n=0; while(i<argc && strcmp(argv[i],"--")) { if(n<63) env[n++]=argv[i]; i++; }
    env[n]=0; if(i+1>=argc) return 4; setsid(); execve(argv[i+1], argv+i+1, env); return 5; }
  if(!strcmp(m,"sleep")){ for(;;) pause(); }
  return 3;
}
