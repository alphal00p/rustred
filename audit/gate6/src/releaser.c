// gate5 barrier probe. ONLY run inside a private user+pid namespace. A same-uid process that releases spawn barriers
// early: scans /proc for "<exe> __spawn-barrier FD -- ..." and writes 'G' into /proc/PID/fd/FD opened for writing.
// Logs each release (pid, fd, time) to argv[1]. Runs until killed.
#define _GNU_SOURCE
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <unistd.h>
#include <fcntl.h>
#include <dirent.h>
#include <time.h>
int main(int argc, char**argv){
  if(argc<2) return 2;
  char done[65536]={0};
  for(;;){
    DIR*d=opendir("/proc"); struct dirent*e;
    while(d && (e=readdir(d))){
      int pid=atoi(e->d_name); if(pid<=1 || pid>=65536 || done[pid]) continue;
      char p[64],buf[4096]; snprintf(p,sizeof p,"/proc/%d/cmdline",pid);
      int fd=open(p,O_RDONLY); if(fd<0) continue; ssize_t n=read(fd,buf,sizeof buf-1); close(fd); if(n<=0) continue; buf[n]=0;
      char*a0=buf; char*a1=a0+strlen(a0)+1; if(a1>=buf+n) continue;
      if(strcmp(a1,"__spawn-barrier")) continue;
      char*a2=a1+strlen(a1)+1; if(a2>=buf+n) continue;
      int bfd=atoi(a2);
      snprintf(p,sizeof p,"/proc/%d/fd/%d",pid,bfd);
      int w=open(p,O_WRONLY|O_NONBLOCK);
      struct timespec ts; clock_gettime(CLOCK_REALTIME,&ts);
      FILE*lf=fopen(argv[1],"a");
      if(w>=0){ ssize_t k=write(w,"G",1); close(w); if(lf) fprintf(lf,"released pid %d fd %d write=%zd at %ld.%09ld\n",pid,bfd,k,(long)ts.tv_sec,ts.tv_nsec); }
      else if(lf) fprintf(lf,"open failed pid %d fd %d\n",pid,bfd);
      if(lf) fclose(lf);
      done[pid]=1;
    }
    if(d) closedir(d);
    usleep(500);
  }
}
