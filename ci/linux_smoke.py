"""Check that the task-owned native Linux application opens a window."""

from pathlib import Path
import os
import re
import subprocess
import tempfile
import time

binary = Path("src-tauri/target/debug/cryptography-playground").resolve()
with tempfile.TemporaryDirectory(prefix="playground-linux-smoke-") as temporary:
    root = Path(temporary)
    environment = os.environ.copy()
    environment["GDK_BACKEND"] = "x11"
    environment["LIBGL_ALWAYS_SOFTWARE"] = "1"
    for key, directory in [
        ("XDG_CONFIG_HOME", "config"),
        ("XDG_DATA_HOME", "data"),
        ("XDG_CACHE_HOME", "cache"),
    ]:
        environment[key] = str(root / directory)
    with (root / "app.log").open("w+") as log:
        process = subprocess.Popen([str(binary)], env=environment, stdout=log, stderr=log)
        try:
            deadline = time.monotonic() + 30
            while time.monotonic() < deadline:
                if process.poll() is not None:
                    raise RuntimeError(f"Native app exited with {process.returncode}")
                tree = subprocess.check_output(
                    ["xwininfo", "-root", "-tree"], text=True, stderr=subprocess.STDOUT
                )
                for window in re.findall(r'(0x[0-9a-f]+) "Cryptography Playground"', tree):
                    identity = subprocess.check_output(
                        ["xprop", "-id", window, "_NET_WM_PID"], text=True
                    )
                    if re.search(rf"= {process.pid}\s*$", identity):
                        print(f"Native window {window} belongs to running PID {process.pid}.")
                        break
                else:
                    time.sleep(0.25)
                    continue
                break
            else:
                raise RuntimeError("Native app did not open its window")
        finally:
            process.terminate()
            try:
                process.wait(timeout=10)
            except subprocess.TimeoutExpired:
                process.kill()
                process.wait()
            log.seek(0)
            print(log.read()[-8000:])
