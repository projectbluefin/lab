"""
Service-catalog non-media lane checks.
Validates the first network-only non-media workload contract.
"""

from __future__ import annotations

import json

from tests.service_catalog.shared.kube import (
    TEST_LANE,
    first_pod_name,
    get_pods_json,
    http_get,
    restart_workload,
    run_kubectl,
    write_artifact,
)


class TestNonMediaLane:
    def test_http_endpoint_serves_nonmedia_fixture(self):
        body = http_get()
        assert "nonmedia-ready" in body, body

    def test_config_path_survives_rollout_restart(self):
        before = get_pods_json()
        write_artifact(f"{TEST_LANE}-pods-before.json", json.dumps(before, indent=2))
        namespace = before["items"][0]["metadata"]["namespace"]
        pod_name = first_pod_name()
        seed = run_kubectl(
            "exec",
            "-n",
            namespace,
            pod_name,
            "--",
            "sh",
            "-c",
            "echo nonmedia-sentinel >/data/nonmedia-sentinel.txt",
        )
        assert seed.returncode == 0, seed.stdout + seed.stderr

        restart_workload()

        after = get_pods_json()
        write_artifact(f"{TEST_LANE}-pods-after.json", json.dumps(after, indent=2))
        namespace = after["items"][0]["metadata"]["namespace"]
        pod_name = first_pod_name()
        check = run_kubectl(
            "exec",
            "-n",
            namespace,
            pod_name,
            "--",
            "cat",
            "/data/nonmedia-sentinel.txt",
        )
        assert check.returncode == 0, check.stdout + check.stderr
        assert check.stdout.strip() == "nonmedia-sentinel"
