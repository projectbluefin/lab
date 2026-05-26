"""
In-cluster homelab substrate checks.
Verifies that the first homelab workload lane can deploy, restart, and remain reachable.
"""

from __future__ import annotations

import json
import os
import subprocess
from pathlib import Path


NAMESPACE = os.environ["TEST_NAMESPACE"]
APP_LABEL = os.environ.get("TEST_APP_LABEL", "app=homelab-substrate")
SERVICE_NAME = os.environ.get("TEST_SERVICE_NAME", "homelab-substrate")
RESULTS_DIR = Path(os.environ.get("TEST_RESULTS_DIR", "/tmp/results"))
RESULTS_DIR.mkdir(parents=True, exist_ok=True)


def run_kubectl(*args: str) -> subprocess.CompletedProcess[str]:
    return subprocess.run(
        ["kubectl", *args],
        capture_output=True,
        text=True,
        timeout=120,
    )


def write_artifact(name: str, content: str) -> None:
    (RESULTS_DIR / name).write_text(content)


def json_text(*args: str) -> str:
    result = run_kubectl(*args)
    assert result.returncode == 0, (
        f"kubectl {' '.join(args)} failed:\nstdout:\n{result.stdout}\nstderr:\n{result.stderr}"
    )
    return result.stdout


class TestHomelabSubstrateLifecycle:
    def test_deployment_becomes_ready(self):
        deployment_json = json_text(
            "get",
            "deployment",
            "homelab-substrate",
            "-n",
            NAMESPACE,
            "-o",
            "json",
        )
        write_artifact("deployment-status-before.json", deployment_json)
        data = json.loads(deployment_json)
        status = data.get("status", {})
        assert status.get("availableReplicas", 0) >= 1, deployment_json
        assert status.get("readyReplicas", 0) >= 1, deployment_json

    def test_service_has_endpoints(self):
        service_json = json_text(
            "get",
            "service",
            SERVICE_NAME,
            "-n",
            NAMESPACE,
            "-o",
            "json",
        )
        endpoints_json = json_text(
            "get",
            "endpoints",
            SERVICE_NAME,
            "-n",
            NAMESPACE,
            "-o",
            "json",
        )
        write_artifact("service.json", service_json)
        write_artifact("endpoints.json", endpoints_json)
        endpoints = json.loads(endpoints_json)
        subsets = endpoints.get("subsets") or []
        addresses = [addr for subset in subsets for addr in subset.get("addresses", [])]
        assert addresses, endpoints_json

    def test_rollout_restart_changes_pod_identity(self):
        before_pods = json.loads(
            json_text(
                "get",
                "pods",
                "-n",
                NAMESPACE,
                "-l",
                APP_LABEL,
                "-o",
                "json",
            )
        )
        write_artifact("pods-before-restart.json", json.dumps(before_pods, indent=2))
        before_uids = {item["metadata"]["uid"] for item in before_pods.get("items", [])}
        assert before_uids, "No substrate pods found before restart"

        restart = run_kubectl("rollout", "restart", "deployment/homelab-substrate", "-n", NAMESPACE)
        assert restart.returncode == 0, (
            f"rollout restart failed:\nstdout:\n{restart.stdout}\nstderr:\n{restart.stderr}"
        )
        write_artifact("restart.txt", restart.stdout + restart.stderr)

        status = run_kubectl(
            "rollout",
            "status",
            "deployment/homelab-substrate",
            "-n",
            NAMESPACE,
            "--timeout=300s",
        )
        assert status.returncode == 0, (
            f"rollout status failed:\nstdout:\n{status.stdout}\nstderr:\n{status.stderr}"
        )
        write_artifact("rollout-status.txt", status.stdout + status.stderr)

        after_pods = json.loads(
            json_text(
                "get",
                "pods",
                "-n",
                NAMESPACE,
                "-l",
                APP_LABEL,
                "-o",
                "json",
            )
        )
        write_artifact("pods-after-restart.json", json.dumps(after_pods, indent=2))
        after_uids = {item["metadata"]["uid"] for item in after_pods.get("items", [])}
        assert after_uids, "No substrate pods found after restart"
        assert before_uids != after_uids, (
            "Pod identity did not change across rollout restart:\n"
            f"before={sorted(before_uids)}\nafter={sorted(after_uids)}"
        )
