import os
import subprocess
import sys

session = int(sys.argv[1])
pids = subprocess.check_output(["/bin/ps", "-A", "-o", "pid="])
groups = set()
for entry in pids.splitlines():
    try:
        pid = int(entry)
        if os.getsid(pid) == session:
            groups.add(os.getpgid(pid))
    except ProcessLookupError:
        pass
print("ok")
for group in sorted(groups):
    print(group)
