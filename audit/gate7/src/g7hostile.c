// gate7 hostile helpers. ONLY run inside a private user+pid namespace.
//  ndaemon      : double fork, setsid, chdir /, close fds, prctl(PR_SET_DUMPABLE,0), pause (marker kept, unreadable to a capless reader)
//  ndchain HOP  : re-forking chain (parent exits after fork, setsid every hop), non-dumpable (inherited by fork)
//  ndorphan     : fork; child setsid + non-dumpable + pause; the parent exits at once (child re-parented)
//  reparent1 PIDFILE : (decoy) fork a child that becomes non-dumpable and writes its pid; the parent exits (re-parent)
#define _GNU_SOURCE
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <unistd.h>
#include <signal.h>
#include <sys/prctl.h>
#include <fcntl.h>
#include <time.h>
static void msleep(long ms){ struct timespec t={ms/1000,(ms%1000)*1000000L}; nanosleep(&t,0);}
static void closeall(void){ for(int fd=0; fd<1024; fd++) close(fd); int n=open("/dev/null",O_RDWR); (void)n; }
int main(int argc,char**argv){
  if(argc<2) return 2;
  signal(SIGHUP,SIG_IGN); signal(SIGTERM,SIG_IGN); signal(SIGINT,SIG_IGN);
  const char*m=argv[1];
  if(!strcmp(m,"ndaemon")){ // ndaemon [PIDFILE]: appends "<pid> <marker>" to PIDFILE after daemonising
    if(fork()) return 0; setsid(); if(fork()) _exit(0);
    if(chdir("/")){} closeall(); prctl(PR_SET_DUMPABLE,0,0,0,0);
    if(argc>2){ FILE*f=fopen(argv[2],"a"); if(f){ const char*v=getenv("RR_CAMPAIGN_JOB"); fprintf(f,"%d %s\n",getpid(),v?v:"-"); fclose(f);} }
    for(;;) pause();
  }
  if(!strcmp(m,"ndchain")){
    long hop = argc>2? atol(argv[2]) : 50;
    if(fork()) return 0;
    prctl(PR_SET_DUMPABLE,0,0,0,0);
    for(;;){ pid_t p=fork(); if(p<0){ msleep(5); continue; } if(p>0) _exit(0); setsid(); prctl(PR_SET_DUMPABLE,0,0,0,0); msleep(hop); }
  }
  if(!strcmp(m,"ndorphan") || !strcmp(m,"reparent1")){
    pid_t p=fork(); if(p) { if(argc>2){ FILE*f=fopen(argv[2],"w"); if(f){ fprintf(f,"%d\n",p); fclose(f);} } return 0; }
    setsid(); prctl(PR_SET_DUMPABLE,0,0,0,0); int dn=open("/dev/null",O_RDWR); dup2(dn,0); dup2(dn,1); dup2(dn,2);
    for(;;) pause();
  }
  if(!strcmp(m,"ndself")){ // ndself [PIDFILE]: become non-dumpable in place (no fork), record "<pid>", pause
    prctl(PR_SET_DUMPABLE,0,0,0,0);
    if(argc>2){ FILE*f=fopen(argv[2],"a"); if(f){ fprintf(f,"%d\n",getpid()); fclose(f);} }
    for(;;) pause();
  }
  return 3;
}
