"""
In-cluster local-path storage checks.
"""

from __future__ import annotations

import json
import subprocess

from tests.service_catalog.shared.kube import (
    TEST_NAMESPACE,
    first_pod_name,
    get_pods_json,
    run_kubectl,
    write_artifact,
)


class TestHomelabStorage:
    def test_pvc_is_bound(self):
        pvc = run_kubectl("get", "pvc", "homelab-storage-data", "-n", TEST_NAMESPACE, "-o", "json")
        assert pvc.returncode == 0, pvc.stdout + pvc.stderr
        write_artifact("storage-pvc.json", pvc.stdout)
        data = json.loads(pvc.stdout)
        assert data.get("status", {}).get("phase") == "Bound", pvc.stdout

    def test_state_path_survives_rollout_restart(self):
        before = get_pods_json()
        write_artifact("storage-pods-before.json", json.dumps(before, indent=2))
        pod_name = first_pod_name()

        seed = run_kubectl(
            "exec",
            "-n",
            TEST_NAMESPACE,
            pod_name,
            "--",
            "bash",
            "-lc",
            "echo storage-sentinel >/data/storage-sentinel.txt",
        )
        assert seed.returncode == 0, seed.stdout + seed.stderr

        restart = run_kubectl("rollout", "restart", "deployment/homelab-storage", "-n", TEST_NAMESPACE)
        assert restart.returncode == 0, restart.stdout + restart.stderr
        write_artifact("storage-restart.txt", restart.stdout + restart.stderr)

        status = run_kubectl(
            "rollout",
            "status",
            "deployment/homelab-storage",
            "-n",
            TEST_NAMESPACE,
            "--timeout=300s",
        )
        assert status.returncode == 0, status.stdout + status.stderr
        write_artifact("storage-rollout-status.txt", status.stdout + status.stderr)

        after = get_pods_json()
        write_artifact("storage-pods-after.json", json.dumps(after, indent=2))
        pod_name = first_pod_name()
        verify = run_kubectl(
            "exec",
            "-n",
            TEST_NAMESPACE,
            pod_name,
            "--",
            "cat",
            "/data/storage-sentinel.txt",
        )
        assert verify.returncode == 0, verify.stdout + verify.stderr
        assert verify.stdout.strip() == "storage-sentinel"

    def test_collects_storage_observability_artifacts(self):
        pod_name = first_pod_name()
        commands = {
            "storage-findmnt.txt": ["findmnt", "/data"],
            "storage-df.txt": ["df", "-h", "/data"],
            "storage-statfs.txt": ["stat", "-f", "/data"],
            "storage-lsblk.txt": ["lsblk", "-f"],
        }
        for artifact, cmd in commands.items():
            result = run_kubectl("exec", "-n", TEST_NAMESPACE, pod_name, "--", *cmd)
            assert result.returncode == 0, (
                f"{' '.join(cmd)} failed:\nstdout:\n{result.stdout}\nstderr:\n{result.stderr}"
            )
            write_artifact(artifact, result.stdout + result.stderr)

        zpool = run_kubectl("exec", "-n", TEST_NAMESPACE, pod_name, "--", "bash", "-lc", "command -v zpool >/dev/null && zpool status -x || true")
        zfs = run_kubectl("exec", "-n", TEST_NAMESPACE, pod_name, "--", "bash", "-lc", "command -v zfs >/dev/null && zfs list || true")
        write_artifact("storage-zpool.txt", zpool.stdout + zpool.stderr)
        write_artifact("storage-zfs.txt", zfs.stdout + zfs.stderr)
