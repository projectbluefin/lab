import re
import subprocess
import sys
import time

from dogtail import tree as dtree


time_re = re.compile(r"\d{1,2}:\d{2}|clock", re.IGNORECASE)
last_err = "unknown"

for attempt in range(1, 31):
    try:
        result = subprocess.run(
            [
                "gdbus",
                "call",
                "--session",
                "--dest",
                "org.gnome.Shell",
                "--object-path",
                "/org/gnome/Shell",
                "--method",
                "org.gnome.Shell.Eval",
                "global.context.unsafe_mode = true; Main.panel ? true : false",
            ],
            capture_output=True,
            text=True,
            timeout=5,
        )
        if result.returncode != 0 or "(true," not in result.stdout:
            err = result.stderr.strip() or result.stdout.strip() or "unknown gdbus failure"
            last_err = f"Shell.Eval not ready: {err[:200]}"
            print(f"Readiness attempt {attempt}: {last_err}", flush=True)
            time.sleep(2)
            continue

        shell = dtree.root.application("gnome-shell")
        panels = shell.findChildren(lambda n: n.roleName == "panel")
        if not panels:
            last_err = "gnome-shell panel not exposed in AT-SPI yet"
            print(f"Readiness attempt {attempt}: {last_err}", flush=True)
            time.sleep(2)
            continue

        toggles = panels[0].findChildren(lambda n: n.roleName == "toggle button")
        toggle_names = [t.name for t in toggles]
        if any(name and time_re.search(name) for name in toggle_names):
            print(f"GNOME Shell ready (attempt {attempt}): {toggle_names}", flush=True)
            sys.exit(0)

        last_err = f"panel toggles not ready yet: {toggle_names}"
        print(f"Readiness attempt {attempt}: {last_err}", flush=True)
    except Exception as exc:  # noqa: BLE001
        last_err = str(exc)
        print(f"Readiness attempt {attempt} failed: {last_err}", flush=True)
    time.sleep(2)

print(
    f"ERROR: GNOME Shell AT-SPI readiness failed after 30 attempts ({last_err})",
    file=sys.stderr,
    flush=True,
)
sys.exit(1)
