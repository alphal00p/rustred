# ptydrive.py MODE OPEN_S WAIT_S -- CMD... ; MODE: ctty-close | noctty-close
# Runs CMD on a pty slave, reads the master for OPEN_S seconds, closes the master, waits up to WAIT_S for the exit.
# Prints: "exited rc=<code> after <t>s" or "STILL RUNNING after <WAIT_S>s pid=<pid>" (caller decides; inside a namespace).
import os, sys, time, select, signal
mode, open_s, wait_s = sys.argv[1], float(sys.argv[2]), float(sys.argv[3])
cmd = sys.argv[sys.argv.index('--') + 1:]
m, s = os.openpty()
pid = os.fork()
if pid == 0:
    os.close(m)
    os.setsid()
    if mode == 'ctty-close':
        import fcntl, termios
        fcntl.ioctl(s, termios.TIOCSCTTY, 0)
    for fd in (0, 1, 2):
        os.dup2(s, fd)
    if s > 2:
        os.close(s)
    os.environ['TERM'] = 'xterm-256color'
    os.execvp(cmd[0], cmd)
os.close(s)
t0 = time.time()
n = 0
while time.time() - t0 < open_s:
    r, _, _ = select.select([m], [], [], 0.2)
    if r:
        try:
            n += len(os.read(m, 65536))
        except OSError:
            break
    p, st = os.waitpid(pid, os.WNOHANG)
    if p:
        print(f"exited early rc={os.waitstatus_to_exitcode(st)} after {time.time()-t0:.1f}s (read {n} bytes)")
        sys.exit(0)
os.close(m)
t1 = time.time()
while time.time() - t1 < wait_s:
    p, st = os.waitpid(pid, os.WNOHANG)
    if p:
        print(f"exited rc={os.waitstatus_to_exitcode(st)} {time.time()-t1:.1f}s after the master closed (read {n} bytes)")
        sys.exit(0)
    time.sleep(0.1)
print(f"STILL RUNNING {wait_s}s after the master closed pid={pid} (read {n} bytes)")
