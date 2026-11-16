// gate7: run CMD with a seccomp filter that makes renameat2(..., RENAME_NOREPLACE) fail with EINVAL (as on a file
// system without RENAME_NOREPLACE support: NFS, some FUSE). Test harness only.
#define _GNU_SOURCE
#include <linux/filter.h>
#include <linux/seccomp.h>
#include <linux/audit.h>
#include <sys/prctl.h>
#include <sys/syscall.h>
#include <stddef.h>
#include <errno.h>
#include <unistd.h>
#include <stdio.h>
int main(int argc,char**argv){
  if(argc<2) return 2;
  struct sock_filter f[] = {
    BPF_STMT(BPF_LD|BPF_W|BPF_ABS, offsetof(struct seccomp_data, arch)),
    BPF_JUMP(BPF_JMP|BPF_JEQ|BPF_K, AUDIT_ARCH_X86_64, 1, 0),
    BPF_STMT(BPF_RET|BPF_K, SECCOMP_RET_ALLOW),
    BPF_STMT(BPF_LD|BPF_W|BPF_ABS, offsetof(struct seccomp_data, nr)),
    BPF_JUMP(BPF_JMP|BPF_JEQ|BPF_K, __NR_renameat2, 0, 3),
    BPF_STMT(BPF_LD|BPF_W|BPF_ABS, offsetof(struct seccomp_data, args[4])),
    BPF_JUMP(BPF_JMP|BPF_JSET|BPF_K, 1 /* RENAME_NOREPLACE */, 0, 1),
    BPF_STMT(BPF_RET|BPF_K, SECCOMP_RET_ERRNO|(EINVAL & SECCOMP_RET_DATA)),
    BPF_STMT(BPF_RET|BPF_K, SECCOMP_RET_ALLOW),
  };
  struct sock_fprog p = { sizeof f / sizeof f[0], f };
  if(prctl(PR_SET_NO_NEW_PRIVS,1,0,0,0)){ perror("nnp"); return 3; }
  if(prctl(PR_SET_SECCOMP,SECCOMP_MODE_FILTER,&p)){ perror("seccomp"); return 4; }
  execvp(argv[1],argv+1); perror("exec"); return 127;
}
