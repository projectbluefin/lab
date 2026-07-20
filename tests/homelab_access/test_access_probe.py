"""
In-cluster HTTPS access probe checks.
"""

from __future__ import annotations

import subprocess

from tests.service_catalog.shared.kube import TEST_NAMESPACE, TEST_SERVICE_NAME, write_artifact


HOSTNAME = "homelab-access.local"
SERVICE_FQDN = f"{TEST_SERVICE_NAME}.{TEST_NAMESPACE}.svc.cluster.local"


def run(*args: str) -> subprocess.CompletedProcess[str]:
    return subprocess.run(args, capture_output=True, text=True, timeout=30)


class TestAccessProbe:
    def test_cluster_dns_resolves_service(self):
        result = run("getent", "hosts", SERVICE_FQDN)
        write_artifact("access-dns.txt", result.stdout + result.stderr)
        assert result.returncode == 0, result.stdout + result.stderr

    def test_https_handshake_exposes_certificate(self):
        result = run(
            "openssl",
            "s_client",
            "-connect",
            f"{SERVICE_FQDN}:8443",
            "-servername",
            HOSTNAME,
            "-brief",
        )
        write_artifact("access-openssl.txt", result.stdout + result.stderr)
        assert result.returncode == 0, result.stdout + result.stderr
        assert "Protocol version" in (result.stdout + result.stderr)

    def test_expected_host_reaches_fixture(self):
        result = run(
            "curl",
            "-sk",
            "-H",
            f"Host: {HOSTNAME}",
            f"https://{SERVICE_FQDN}:8443/healthz",
        )
        write_artifact("access-curl.txt", result.stdout + result.stderr)
        assert result.returncode == 0, result.stdout + result.stderr
        assert result.stdout.strip() == "access-ok"
