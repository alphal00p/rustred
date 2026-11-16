// gate10 multi-thread churner (ONLY inside a private user+pid namespace): g10churn PIDFILE LIFE_US K
// = gate9's g9churn with K concurrent self-replacing worker threads (each sleeps LIFE_US, starts its successor, returns);
// the MAIN thread exits with pthread_exit (the leader is a ZOMBIE, the process lives on). Marker = inherited environment.
#define _GNU_SOURCE
#include <pthread.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <unistd.h>
#include <signal.h>
#include <fcntl.h>
static long life_us = 500;
static void* w(void* a){ (void)a; if (life_us > 0) usleep(life_us); pthread_t t; pthread_attr_t at; pthread_attr_init(&at);
  pthread_attr_setdetachstate(&at, PTHREAD_CREATE_DETACHED); pthread_attr_setstacksize(&at, 65536);
  while (pthread_create(&t,&at,w,NULL)!=0) usleep(1000); return NULL; }
int main(int argc,char**argv){
  if(argc<4) return 2; life_us = atol(argv[2]); int k = atoi(argv[3]); if (k < 1) k = 1;
  pid_t p=fork(); if(p) return 0;
  setsid(); signal(SIGHUP,SIG_IGN); int dn=open("/dev/null",O_RDWR); dup2(dn,0); dup2(dn,1); dup2(dn,2);
  FILE*f=fopen(argv[1],"w"); if(f){ fprintf(f,"%d\n",getpid()); fclose(f);}
  for (int i = 0; i < k; i++) { pthread_t t; pthread_attr_t at; pthread_attr_init(&at); pthread_attr_setdetachstate(&at, PTHREAD_CREATE_DETACHED);
    pthread_attr_setstacksize(&at, 65536); pthread_create(&t,&at,w,NULL); }
  pthread_exit(NULL);
}
