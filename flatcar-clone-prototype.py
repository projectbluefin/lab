#!/usr/bin/env python3
import os
import sys
import termios
import tty

# --- PROTOTYPE QUESTION AND LOGIC MODULE ---
# Question: Does an FSDK-built bootc image pretending to be Flatcar (ID=flatcar, VERSION_ID=4593.2.3)
# correctly and safely merge and manage the lifecycle of Flatcar system extensions (sysexts)?
# And what happens to these extensions when bootc/ostree performs an OS upgrade?

class FlatcarCloneState:
    def __init__(self):
        # Base Host Configuration
        self.kernel_version = "7.1.1-fsdk-server"
        self.os_id = "flatcar"
        self.os_version_id = "4593.2.3"
        self.os_pretty_name = "Flatcar Clone (FSDK)"
        
        # Available system extensions in /usr/lib/extension-images/
        self.available_extensions = {
            "docker": {
                "name": "docker.raw",
                "target_id": "flatcar",
                "target_version": "4593.2.3",
                "size_mb": 250,
                "description": "Docker 28 container engine runtime",
                "active": False
            },
            "k3s": {
                "name": "k3s.raw",
                "target_id": "flatcar",
                "target_version": "4593.2.3",
                "size_mb": 120,
                "description": "K3s lightweight Kubernetes cluster",
                "active": False
            },
            "custom-app": {
                "name": "custom-app.raw",
                "target_id": "flatcar",
                "target_version": "4593.2.3",
                "size_mb": 45,
                "description": "Project Bluefin local test runner",
                "active": False
            },
            "invalid-ubuntu": {
                "name": "invalid-ubuntu.raw",
                "target_id": "ubuntu",
                "target_version": "24.04",
                "size_mb": 80,
                "description": "Ubuntu-built Python development stack",
                "active": False
            }
        }
        
        # Log of systemd-sysext daemon events
        self.logs = ["System booted successfully with bootc.", "systemd-sysext initialized empty /usr overlay."]

    def toggle_extension(self, key):
        ext = self.available_extensions.get(key)
        if not ext:
            return
            
        if ext["active"]:
            ext["active"] = False
            self.logs.append(f"systemd-sysext: Unmerged {ext['name']}. /usr path cleaned.")
        else:
            # systemd-sysext compatibility check
            if ext["target_id"] != self.os_id:
                self.logs.append(f"ERROR: Cannot merge {ext['name']}. Extension target ID '{ext['target_id']}' does not match host ID '{self.os_id}'.")
            elif ext["target_version"] != self.os_version_id:
                self.logs.append(f"ERROR: Cannot merge {ext['name']}. Extension target version '{ext['target_version']}' does not match host VERSION_ID '{self.os_version_id}'.")
            else:
                ext["active"] = True
                self.logs.append(f"SUCCESS: systemd-sysext merged {ext['name']} into /usr. Binaries available at /usr/bin/{key}.")

    def simulate_os_upgrade(self):
        old_version = self.os_version_id
        if self.os_version_id == "4593.2.3":
            self.os_version_id = "4595.0.0"
        else:
            self.os_version_id = "4593.2.3"
            
        self.logs.append(f"bootc: Simulated A/B atomic upgrade from {old_version} to {self.os_version_id}.")
        
        # Strict systemd-sysext lifecycle check on reboot/refresh:
        # All sysexts must match the new OS version. Any version-mismatched extension is automatically unmerged!
        unmerged_any = False
        for k, ext in self.available_extensions.items():
            if ext["active"] and ext["target_version"] != self.os_version_id:
                ext["active"] = False
                self.logs.append(f"LIFECYCLE WARNING: systemd-sysext automatically unmerged {ext['name']} due to OS version mismatch ({ext['target_version']} vs {self.os_version_id}).")
                unmerged_any = True
                
        if not unmerged_any:
            self.logs.append("systemd-sysext: All active extensions verified compatible with the new OS version.")


# --- TUI DRAWING FUNCTIONS ---

def draw_tui(state):
    # Clear screen
    print("\033[2J\033[H", end="")
    
    # Header
    print("\033[1;33m" + "=" * 80 + "\033[0m")
    print("\033[1;36m      FSDK FLATCAR-CLONE BOOTC PROTOTYPE (systemd-sysext State Machine)\033[0m")
    print("\033[1;33m" + "=" * 80 + "\033[0m")
    print()
    
    # Host State Card
    print("\033[1m[HOST OPERATING SYSTEM STATE]\033[0m")
    print(f"  Pretty Name     : \033[32m{state.os_pretty_name}\033[0m")
    print(f"  OS ID (/usr/lib/os-release): \033[1;32m{state.os_id}\033[0m")
    print(f"  VERSION_ID      : \033[1;32m{state.os_version_id}\033[0m")
    print(f"  Server Kernel   : \033[35m{state.kernel_version}\033[0m")
    print(f"  ComposeFS Status: \033[34mActive (Read-Only /usr)\033[0m")
    print()
    
    # Extension List Table
    print("\033[1m[AVAILABLE SYSTEM EXTENSIONS (/usr/lib/extension-images/)]\033[0m")
    print(f"  %-20s %-12s %-12s %-8s %-32s" % ("EXTENSION NAME", "TARGET ID", "TARGET VER", "STATUS", "DESCRIPTION"))
    print("  " + "-" * 76)
    
    idx = 1
    mapping = {}
    for k, ext in state.available_extensions.items():
        status_str = "\033[1;32mMERGED\033[0m" if ext["active"] else "\033[2mUNMERGED\033[0m"
        print(f"  [{idx}] %-16s %-12s %-12s %-18s %-32s" % (ext["name"], ext["target_id"], ext["target_version"], status_str, ext["description"]))
        mapping[str(idx)] = k
        idx += 1
        
    print()
    
    # Event Logs Section
    print("\033[1m[systemd-sysext & bootc SERVICE LOGS]\033[0m")
    for log in state.logs[-6:]:
        if "ERROR" in log:
            print(f"  \033[1;31m[!] {log}\033[0m")
        elif "WARNING" in log:
            print(f"  \033[1;33m[!] {log}\033[0m")
        elif "SUCCESS" in log:
            print(f"  \033[1;32m[*] {log}\033[0m")
        else:
            print(f"  [i] {log}")
            
    print()
    
    # Key Shortcuts
    print("\033[1;33m" + "=" * 80 + "\033[0m")
    print("\033[1mShortcuts:\033[0m \033[1;32m[1-4]\033[0m Toggle Extension  \033[1;32m[u]\033[0m Simulate OS Upgrade  \033[1;31m[q]\033[0m Quit Prototype")
    print("\033[1;33m" + "=" * 80 + "\033[0m")
    
    return mapping


# --- KEYBOARD INPUT UTILS ---

def get_char():
    fd = sys.stdin.fileno()
    old_settings = termios.tcgetattr(fd)
    try:
        tty.setraw(sys.stdin.fileno())
        ch = sys.stdin.read(1)
    finally:
        termios.tcsetattr(fd, termios.TCSADRAIN, old_settings)
    return ch


def main():
    state = FlatcarCloneState()
    
    # Main Interactive Loop
    while True:
        mapping = draw_tui(state)
        ch = get_char()
        
        if ch == 'q':
            print("\033[2J\033[H", end="")
            print("Prototype session terminated safely. Goodbye!")
            break
        elif ch in mapping:
            state.toggle_extension(mapping[ch])
        elif ch == 'u':
            state.simulate_os_upgrade()

if __name__ == "__main__":
    main()
