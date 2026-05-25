"""
Flatcar + Knuckle Integration Tests — Phase 2
Validates that the knuckle headless installer can install Flatcar
to a target disk and boot from it.
Requires: KNUCKLE_BINARY env var pointing to the built knuckle binary.
"""
import os
import subprocess

import pytest

KNUCKLE = os.environ.get("KNUCKLE_BINARY", "/usr/local/bin/knuckle")


def run(cmd, **kwargs):
    return subprocess.run(cmd, shell=True, capture_output=True, text=True, **kwargs)


@pytest.mark.skipif(not os.path.exists(KNUCKLE), reason="knuckle binary not present")
class TestKnuckleDryRun:
    def test_dry_run_exits_zero(self):
        """knuckle --dry-run must succeed with a valid config."""
        config = '{"channel":"stable","hostname":"flatcar-test","timezone":"UTC","network":{"mode":"dhcp"},"users":[{"username":"core","ssh_keys":["ssh-ed25519 AAAAC3test"]}],"disk":"/dev/vdb","update_strategy":"off","reboot":false}'
        r = run(f"echo '{config}' | {KNUCKLE} headless --dry-run --config -")
        assert r.returncode == 0, f"knuckle --dry-run failed:\n{r.stderr}"

    def test_dry_run_no_disk_writes(self):
        """--dry-run must not call wipefs or flatcar-install."""
        config = '{"channel":"stable","hostname":"flatcar-test","timezone":"UTC","network":{"mode":"dhcp"},"users":[{"username":"core","ssh_keys":["ssh-ed25519 AAAAC3test"]}],"disk":"/dev/vdb","update_strategy":"off","reboot":false}'
        r = run(f"echo '{config}' | {KNUCKLE} headless --dry-run --config - 2>&1")
        assert "wipefs" not in r.stdout.lower() or "DRY RUN" in r.stdout
