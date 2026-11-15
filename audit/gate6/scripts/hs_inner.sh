# host scope script --dry-run, INSIDE ns() under strace: does it start, signal, stop or create anything?
. /common/build-cache/vhirschi/p_integral_dev/implementer-scratch/gate6/scripts/lib.sh
in_ns_guard6
HS=/shared/vhirschi/t3code/vakint_dev/p_integral_dev/plan/auto-campaign/host-tests/letter_a_scope_containment.sh
R=$S/runs/hostscript; [ -e "$R" ] && { echo "exists: $R"; exit 3; }; mkdir -p $R/cwd; cd $R/cwd
sha256sum $HS
for variant in "BIN=$S/bin/release OUT=$R/out1" "OUT=$R/out2" ""; do
  n=$((${n:-0}+1))
  env $variant strace -f -qq -o $R/hs$n.strace -e trace=execve,kill,tgkill,tkill,pidfd_send_signal,pidfd_open,mkdir,mkdirat,openat,creat,unlink,unlinkat,rename,renameat,renameat2,mount,umount2 -e signal=none $BASH_BIN $HS --dry-run > $R/dry$n.out 2>&1
  rc=$?
  echo "variant $n [$variant]: rc=$rc, $(wc -l < $R/dry$n.out) lines; execve: $(grep -o 'execve("[^"]*"' $R/hs$n.strace | sed 's/execve("//;s/"$//' | xargs -n1 basename | sort | uniq -c | tr '\n' ' ')"
  echo "   signal syscalls: $(grep -c 'kill(\|pidfd_send_signal(\|pidfd_open(' $R/hs$n.strace); mkdir/creat/unlink/rename/mount: $(grep -c 'mkdir\|creat(\|unlink\|rename\|mount(' $R/hs$n.strace); openat with O_CREAT or write mode: $(grep 'openat(' $R/hs$n.strace | grep -c 'O_CREAT\|O_WRONLY\|O_RDWR')"
  grep 'openat(' $R/hs$n.strace | grep 'O_CREAT\|O_WRONLY\|O_RDWR' | grep -o '"[^"]*"' | sort | uniq -c | head -5 | sed 's/^/     write-open: /'
done
echo "OUT dirs created: $(ls -d $R/out1 $R/out2 2>/dev/null | wc -l); files under cwd: $(ls -A $R/cwd | wc -l)"
echo "--- dry-run output (variant 1, first 40 lines) ---"; sed -n 1,40p $R/dry1.out | cut -c1-230
echo "--- typo '--dryrun' with BIN set (inside the namespace: the script must refuse as a HOST test) ---"
env BIN=$S/bin/release OUT=$R/out3 $BASH_BIN $HS --dryrun > $R/typo.out 2>&1; echo "rc=$? $(tail -2 $R/typo.out | cut -c1-200 | tr '\n' ' ') out3 created: $([ -e $R/out3 ] && echo YES || echo no)"
