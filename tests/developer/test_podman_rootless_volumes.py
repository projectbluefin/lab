"""
Phase 2 — Rootless Podman volume persistence smoke tests.
Validates user-space container storage without mutating the host image.
"""
from __future__ import annotations

import subprocess
import uuid


class TestPodmanRootlessVolumes:
    """Verify rootless Podman volume lifecycle and persistence."""

    @staticmethod
    def _run(*args: str, timeout: int = 90) -> subprocess.CompletedProcess[str]:
        return subprocess.run(
            list(args),
            capture_output=True,
            text=True,
            timeout=timeout,
        )

    def test_rootless_podman_volume_persists_data(self):
        """A rootless Podman volume must survive across separate container runs."""
        volume_name = f"testing-lab-{uuid.uuid4().hex[:8]}"
        image = "docker.io/library/alpine:latest"

        create_result = self._run("podman", "volume", "create", volume_name, timeout=30)
        assert create_result.returncode == 0, (
            "podman volume create failed:\n"
            f"stdout:\n{create_result.stdout}\n"
            f"stderr:\n{create_result.stderr}"
        )

        try:
            write_result = self._run(
                "podman",
                "run",
                "--rm",
                "-v",
                f"{volume_name}:/data",
                image,
                "sh",
                "-c",
                "echo persisted-by-testing-lab > /data/probe.txt",
            )
            assert write_result.returncode == 0, (
                "podman run write probe failed:\n"
                f"stdout:\n{write_result.stdout}\n"
                f"stderr:\n{write_result.stderr}"
            )

            read_result = self._run(
                "podman",
                "run",
                "--rm",
                "-v",
                f"{volume_name}:/data",
                image,
                "cat",
                "/data/probe.txt",
            )
            assert read_result.returncode == 0, (
                "podman run read probe failed:\n"
                f"stdout:\n{read_result.stdout}\n"
                f"stderr:\n{read_result.stderr}"
            )
            assert "persisted-by-testing-lab" in read_result.stdout, (
                "Rootless Podman volume did not preserve written content across runs:\n"
                f"{read_result.stdout}"
            )
        finally:
            self._run("podman", "volume", "rm", "-f", volume_name, timeout=30)

    def test_rootless_podman_volume_is_listed_after_create(self):
        """Created rootless Podman volumes must be visible in podman volume ls."""
        volume_name = f"testing-lab-{uuid.uuid4().hex[:8]}"
        create_result = self._run("podman", "volume", "create", volume_name, timeout=30)
        assert create_result.returncode == 0, (
            "podman volume create failed:\n"
            f"stdout:\n{create_result.stdout}\n"
            f"stderr:\n{create_result.stderr}"
        )

        try:
            list_result = self._run("podman", "volume", "ls", "--format", "{{.Name}}", timeout=20)
            assert list_result.returncode == 0, (
                "podman volume ls failed:\n"
                f"stdout:\n{list_result.stdout}\n"
                f"stderr:\n{list_result.stderr}"
            )
            volumes = [line.strip() for line in list_result.stdout.splitlines() if line.strip()]
            assert volume_name in volumes, (
                "Created rootless Podman volume was not listed:\n"
                f"{list_result.stdout}"
            )
        finally:
            self._run("podman", "volume", "rm", "-f", volume_name, timeout=30)
