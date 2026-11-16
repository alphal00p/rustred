// gate9: can a process set its environment bounds to 0 / 0 (the exec-window evidence) on purpose?
#define _GNU_SOURCE
#include <stdio.h>
#include <string.h>
#include <errno.h>
#include <unistd.h>
#include <sys/prctl.h>
#include <linux/prctl.h>
int main(void){
  int r = prctl(PR_SET_MM, PR_SET_MM_ENV_START, 0, 0, 0); printf("PR_SET_MM ENV_START 0: %d %s\n", r, r? strerror(errno):"ok");
  r = prctl(PR_SET_MM, PR_SET_MM_ENV_END, 0, 0, 0); printf("PR_SET_MM ENV_END 0: %d %s\n", r, r? strerror(errno):"ok");
  unsigned int sz=0; prctl(PR_SET_MM, PR_SET_MM_MAP_SIZE, &sz, 0, 0);
  struct prctl_mm_map m; memset(&m,0,sizeof m);
  // read the current map from /proc/self/stat fields would be needed for a valid map; try env 0/0 with zeros elsewhere
  m.exe_fd = (unsigned int)-1;
  r = prctl(PR_SET_MM, PR_SET_MM_MAP, &m, sizeof m, 0); printf("PR_SET_MM_MAP all-zero (env 0/0): %d %s (size %u)\n", r, r? strerror(errno):"ok", sz);
  FILE*f=fopen("/proc/self/stat","r"); char b[4096]; size_t n=fread(b,1,sizeof b-1,f); b[n]=0; fclose(f);
  char*p=strrchr(b,')'); int i=0; char*tok=strtok(p+2," "); while(tok){ i++; if(i==48||i==49) printf("stat field %d: %s\n", i+2, tok); tok=strtok(NULL," "); }
  return 0;
}
