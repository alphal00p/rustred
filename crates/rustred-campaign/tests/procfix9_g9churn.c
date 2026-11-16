// gate9 thread churner (ONLY inside a private user+pid namespace): fork; child setsid's, writes "<pid>" to PIDFILE, starts a
// worker thread and ends its MAIN thread with pthread_exit (the leader becomes a ZOMBIE, the process lives on). Each worker
// sleeps LIFE_US, starts its successor (detached) and returns: at any moment about one live thread, each living LIFE_US.
// The process keeps the environment (and RR_CAMPAIGN_JOB marker) it was started with. Mode "nozombie": the main thread
// stays alive (control).
#define _GNU_SOURCE
#include <pthread.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <unistd.h>
#include <signal.h>
#include <fcntl.h>
static long life_us = 500;
static void* w(void* a){ (void)a; usleep(life_us); pthread_t t; pthread_attr_t at; pthread_attr_init(&at);
  pthread_attr_setdetachstate(&at, PTHREAD_CREATE_DETACHED); pthread_attr_setstacksize(&at, 65536);
  while (pthread_create(&t,&at,w,NULL)!=0) usleep(1000); return NULL; }
int main(int argc,char**argv){
  if(argc<3) return 2; life_us = atol(argv[2]); int nozombie = argc>3 && !strcmp(argv[3],"nozombie");
  pid_t p=fork(); if(p) return 0;
  setsid(); signal(SIGHUP,SIG_IGN); int dn=open("/dev/null",O_RDWR); dup2(dn,0); dup2(dn,1); dup2(dn,2);
  FILE*f=fopen(argv[1],"w"); if(f){ fprintf(f,"%d\n",getpid()); fclose(f);} 
  pthread_t t; pthread_attr_t at; pthread_attr_init(&at); pthread_attr_setdetachstate(&at, PTHREAD_CREATE_DETACHED);
  pthread_create(&t,&at,w,NULL);
  if(nozombie){ for(;;) pause(); }
  pthread_exit(NULL);
}
