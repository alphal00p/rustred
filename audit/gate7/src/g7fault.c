// gate7 LD_PRELOAD fault injector (test harness only, inside private namespaces): opens of a path ending with
// $G7_FAIL_SUFFIX are counted (per process); occurrence numbers listed in $G7_FAIL_AT (comma list, 1-based) or every
// occurrence >= $G7_FAIL_FROM fail with errno $G7_FAIL_ERRNO (default EMFILE). Every counted open is logged to $G7_FAIL_LOG.
#define _GNU_SOURCE
#include <dlfcn.h>
#include <errno.h>
#include <fcntl.h>
#include <stdarg.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <unistd.h>
#include <sys/syscall.h>
static int count=0;
static int should_fail(const char*path){
  const char*suf=getenv("G7_FAIL_SUFFIX"); if(!suf||!path) return 0;
  size_t lp=strlen(path), ls=strlen(suf); if(lp<ls||strcmp(path+lp-ls,suf)) return 0;
  int n=__atomic_add_fetch(&count,1,__ATOMIC_SEQ_CST); int fail=0;
  const char*at=getenv("G7_FAIL_AT"); if(at){ char b[256]; strncpy(b,at,255); b[255]=0; for(char*t=strtok(b,",");t;t=strtok(0,",")) if(atoi(t)==n) fail=1; }
  const char*from=getenv("G7_FAIL_FROM"); if(from && n>=atoi(from)) fail=1;
  const char*lg=getenv("G7_FAIL_LOG"); if(lg){ int fd=syscall(SYS_openat,AT_FDCWD,lg,O_WRONLY|O_APPEND|O_CREAT,0600); if(fd>=0){ char m[512]; int k=snprintf(m,sizeof m,"pid %d tid %ld open#%d %s%s\n",getpid(),(long)syscall(SYS_gettid),n,path,fail?" FAILED":""); if(write(fd,m,k)){} close(fd);} }
  return fail;
}
static int err(void){ const char*e=getenv("G7_FAIL_ERRNO"); errno = e? atoi(e) : EMFILE; return -1; }
#define WRAP(name, ...) \
int name(const char*p, int f, ...){ mode_t mo=0; if(f&(O_CREAT|O_TMPFILE)){ va_list a; va_start(a,f); mo=va_arg(a,int); va_end(a);} \
  if(should_fail(p)) return err(); static int(*r)(const char*,int,...)=0; if(!r) r=dlsym(RTLD_NEXT,#name); return r(p,f,mo); }
WRAP(open)
WRAP(open64)
int openat(int d,const char*p,int f,...){ mode_t mo=0; if(f&(O_CREAT|O_TMPFILE)){ va_list a; va_start(a,f); mo=va_arg(a,int); va_end(a);} if(should_fail(p)) return err(); static int(*r)(int,const char*,int,...)=0; if(!r) r=dlsym(RTLD_NEXT,"openat"); return r(d,p,f,mo); }
int openat64(int d,const char*p,int f,...){ mode_t mo=0; if(f&(O_CREAT|O_TMPFILE)){ va_list a; va_start(a,f); mo=va_arg(a,int); va_end(a);} if(should_fail(p)) return err(); static int(*r)(int,const char*,int,...)=0; if(!r) r=dlsym(RTLD_NEXT,"openat64"); return r(d,p,f,mo); }
