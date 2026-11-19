# gate17: kernel facts behind LaunchState::Gone (run as pid 1 of a private pid namespace).
# Child: starts a worker thread (sleeps 30 s), then ends its MAIN thread with the raw exit syscall (60 on x86_64).
import os, sys, time, subprocess
child_src = r'''
import ctypes, threading, time
t = threading.Thread(target=time.sleep, args=(30,)); t.start()
ctypes.CDLL(None).syscall(60, 0)
'''
p = subprocess.Popen([sys.executable, "-I", "-c", child_src])
pid = p.pid
for _ in range(400):
    st = open(f"/proc/{pid}/stat").read().rsplit(")", 1)[1].split()[0]
    if st == "Z":
        break
    time.sleep(0.01)
status = open(f"/proc/{pid}/status").read()
thr = [l for l in status.splitlines() if l.startswith(("State:", "Threads:"))]
try:
    exe = os.readlink(f"/proc/{pid}/exe")
except OSError as e:
    exe = f"ERR errno {e.errno}"
cg = open(f"/proc/{pid}/cgroup").read().strip()
w = os.waitid(os.P_PID, pid, os.WEXITED | os.WNOHANG | os.WNOWAIT)
tasks = os.listdir(f"/proc/{pid}/task")
live = [t for t in tasks if open(f"/proc/{pid}/task/{t}/stat").read().rsplit(")",1)[1].split()[0] != "Z"]
print("leader stat state:", st, "| status:", thr)
print("exe:", exe)
print("cgroup:", cg)
print("waitid(WEXITED|WNOHANG|WNOWAIT):", w)
print("tasks:", tasks, "live tasks:", live)
time.sleep(3)
w2 = os.waitid(os.P_PID, pid, os.WEXITED | os.WNOHANG | os.WNOWAIT)
print("after 3 s: waitid:", w2, "exe still:", (lambda: os.readlink(f"/proc/{pid}/exe"))() if False else "n/a")
p.kill(); p.wait()
