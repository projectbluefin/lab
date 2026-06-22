"""
Phase 2 — Distrobox workflow smoke tests.
Validates ephemeral create, enter, verify, and cleanup flows.
"""
from __future__ import annotations

import subprocess
import uuid


class TestDistroboxWorkflow:
    """Verify disposable distrobox lifecycle from the desktop user session."""

    @staticmethod
    def _run(*args: str, timeout: int = 300) -> subprocess.CompletedProcess[str]:
        return subprocess.run(
            list(args),
            capture_output=True,
            text=True,
            timeout=timeout,
        )

    def _list_boxes(self) -> subprocess.CompletedProcess[str]:
        return self._run("distrobox", "list", "--no-color", timeout=60)

    def test_distrobox_create_enter_and_cleanup(self):
        """A disposable distrobox must create, run a command, and clean up."""
        box_name = f"testing-lab-{uuid.uuid4().hex[:8]}"
        image = "registry.fedoraproject.org/fedora-toolbox:41"

        version_result = self._run("distrobox", "--version", timeout=20)
        assert version_result.returncode == 0, (
            "distrobox --version failed:\n"
            f"stdout:\n{version_result.stdout}\n"
            f"stderr:\n{version_result.stderr}"
        )

        create_result = self._run(
            "distrobox",
            "create",
            "--yes",
            "--name",
            box_name,
            "--image",
            image,
        )
        assert create_result.returncode == 0, (
            "distrobox create failed:\n"
            f"stdout:\n{create_result.stdout}\n"
            f"stderr:\n{create_result.stderr}"
        )

        try:
            list_result = self._list_boxes()
            assert list_result.returncode == 0, (
                "distrobox list failed after create:\n"
                f"stdout:\n{list_result.stdout}\n"
                f"stderr:\n{list_result.stderr}"
            )
            assert box_name in list_result.stdout, (
                "Created distrobox was not listed:\n"
                f"{list_result.stdout}"
            )

            enter_result = self._run(
                "distrobox",
                "enter",
                box_name,
                "--",
                "sh",
                "-lc",
                "printf 'inside-distrobox'; cat /etc/os-release | grep '^ID='",
            )
            assert enter_result.returncode == 0, (
                "distrobox enter command failed:\n"
                f"stdout:\n{enter_result.stdout}\n"
                f"stderr:\n{enter_result.stderr}"
            )
            assert "inside-distrobox" in enter_result.stdout, (
                "distrobox enter did not execute the expected command:\n"
                f"{enter_result.stdout}"
            )
        finally:
            self._run("distrobox", "rm", "--force", box_name, timeout=120)

        final_list = self._list_boxes()
        assert final_list.returncode == 0, (
            "distrobox list failed after cleanup:\n"
            f"stdout:\n{final_list.stdout}\n"
            f"stderr:\n{final_list.stderr}"
        )
        assert box_name not in final_list.stdout, (
            "Disposable distrobox still appeared after cleanup:\n"
            f"{final_list.stdout}"
        )
