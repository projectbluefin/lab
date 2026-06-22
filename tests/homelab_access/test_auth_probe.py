"""
In-cluster HTTPS auth probe checks.
"""

from __future__ import annotations

import subprocess

from tests.service_catalog.shared.kube import TEST_NAMESPACE, TEST_SERVICE_NAME, write_artifact


HOSTNAME = "homelab-access.local"
SERVICE_FQDN = f"{TEST_SERVICE_NAME}.{TEST_NAMESPACE}.svc.cluster.local"


def run(*args: str) -> subprocess.CompletedProcess[str]:
    return subprocess.run(args, capture_output=True, text=True, timeout=30)


class TestAuthProbe:
    def test_unauthenticated_request_is_rejected(self):
        result = run(
            "curl",
            "-sk",
            "-D",
            "-",
            "-o",
            "/dev/null",
            "-H",
            f"Host: {HOSTNAME}",
            f"https://{SERVICE_FQDN}:8443/healthz",
        )
        write_artifact("auth-unauthenticated.txt", result.stdout + result.stderr)
        assert result.returncode == 0, result.stdout + result.stderr
        assert "401 Unauthorized" in result.stdout, result.stdout

    def test_authenticated_request_succeeds(self):
        result = run(
            "curl",
            "-sk",
            "-u",
            "homelab:controlnode",
            "-H",
            f"Host: {HOSTNAME}",
            f"https://{SERVICE_FQDN}:8443/healthz",
        )
        write_artifact("auth-authenticated.txt", result.stdout + result.stderr)
        assert result.returncode == 0, result.stdout + result.stderr
        assert result.stdout.strip() == "access-ok"
