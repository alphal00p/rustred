// gate7: run CMD with an EMPTY capability bounding set (uid 0 of a root-mapped namespace -> no capabilities after exec),
// like a process of the user on the host. Test harness only.
#define _GNU_SOURCE
#include <sys/prctl.h>
#include <unistd.h>
#include <stdio.h>
int main(int argc,char**argv){ if(argc<2) return 2; for(int c=0;c<64;c++) prctl(PR_CAPBSET_DROP,c,0,0,0); execvp(argv[1],argv+1); perror("exec"); return 127; }
