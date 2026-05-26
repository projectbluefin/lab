"""
In-cluster local-path storage checks.
"""

from __future__ import annotations

import json
import subprocess

import pytest

from tests.service_catalog.shared.kube import (
    TEST_NAMESPACE,
    first_pod_name,
    get_pods_json,
    run_kubectl,
    write_artifact,
)


PVC_NAME = "homelab-storage-data"
DEPLOYMENT_NAME = "deployment/homelab-storage"
DATA_MOUNT_PATH = "/data"
EXPECTED_CAPACITY = "1Gi"
EXPECTED_ACCESS_MODES = ["ReadWriteOnce"]
EXPECTED_STORAGE_CLASS = "local-path"


class TestHomelabStorage:
    def _get_pvc(self) -> dict:
        pvc = run_kubectl("get", "pvc", PVC_NAME, "-n", TEST_NAMESPACE, "-o", "json")
        assert pvc.returncode == 0, pvc.stdout + pvc.stderr
        write_artifact("storage-pvc.json", pvc.stdout)
        return json.loads(pvc.stdout)

    def _exec_in_pod(self, *command: str) -> subprocess.CompletedProcess[str]:
        return run_kubectl("exec", "-n", TEST_NAMESPACE, first_pod_name(), "--", *command)

    def test_pvc_is_bound(self):
        data = self._get_pvc()
        assert data.get("status", {}).get("phase") == "Bound", json.dumps(data, indent=2)

    def test_pvc_capacity_and_access_modes(self):
        data = self._get_pvc()
        assert data.get("status", {}).get("capacity", {}).get("storage") == EXPECTED_CAPACITY, pvc_debug(data)
        assert data.get("spec", {}).get("accessModes") == EXPECTED_ACCESS_MODES, pvc_debug(data)
        assert data.get("status", {}).get("accessModes") == EXPECTED_ACCESS_MODES, pvc_debug(data)

    def test_storage_class_is_local_path(self):
        data = self._get_pvc()
        storage_class_name = data.get("spec", {}).get("storageClassName")
        assert storage_class_name == EXPECTED_STORAGE_CLASS, pvc_debug(data)

    def test_pod_mount_path_exists(self):
        result = self._exec_in_pod(
            "bash",
            "-lc",
            f"test -d {DATA_MOUNT_PATH} && test -w {DATA_MOUNT_PATH} && touch {DATA_MOUNT_PATH}/.writetest && rm -f {DATA_MOUNT_PATH}/.writetest",
        )
        assert result.returncode == 0, result.stdout + result.stderr

    def test_disk_usage_snapshot(self):
        result = self._exec_in_pod("df", "-h", DATA_MOUNT_PATH)
        assert result.returncode == 0, result.stdout + result.stderr
        write_artifact("storage-disk-usage.txt", result.stdout + result.stderr)
        assert DATA_MOUNT_PATH in result.stdout, result.stdout

    def test_ownership_is_stable(self):
        result = self._exec_in_pod("stat", DATA_MOUNT_PATH)
        assert result.returncode == 0, result.stdout + result.stderr
        write_artifact("storage-ownership.txt", result.stdout + result.stderr)
        assert "Uid:" in result.stdout and "Access:" in result.stdout, result.stdout

    def test_rwx_blocked_by_design(self):
        pytest.skip("RWX/shared-storage scenarios blocked by #62 until ReadWriteMany storage class is available")

    def test_state_path_survives_rollout_restart(self):
        before = get_pods_json()
        write_artifact("storage-pods-before.json", json.dumps(before, indent=2))

        seed = self._exec_in_pod(
            "bash",
            "-lc",
            f"echo storage-sentinel >{DATA_MOUNT_PATH}/storage-sentinel.txt",
        )
        assert seed.returncode == 0, seed.stdout + seed.stderr

        restart = run_kubectl("rollout", "restart", DEPLOYMENT_NAME, "-n", TEST_NAMESPACE)
        assert restart.returncode == 0, restart.stdout + restart.stderr
        write_artifact("storage-restart.txt", restart.stdout + restart.stderr)

        status = run_kubectl(
            "rollout",
            "status",
            DEPLOYMENT_NAME,
            "-n",
            TEST_NAMESPACE,
            "--timeout=300s",
        )
        assert status.returncode == 0, status.stdout + status.stderr
        write_artifact("storage-rollout-status.txt", status.stdout + status.stderr)

        after = get_pods_json()
        write_artifact("storage-pods-after.json", json.dumps(after, indent=2))
        verify = self._exec_in_pod("cat", f"{DATA_MOUNT_PATH}/storage-sentinel.txt")
        assert verify.returncode == 0, verify.stdout + verify.stderr
        assert verify.stdout.strip() == "storage-sentinel"

    def test_collects_storage_observability_artifacts(self):
        commands = {
            "storage-findmnt.txt": ["findmnt", DATA_MOUNT_PATH],
            "storage-df.txt": ["df", "-h", DATA_MOUNT_PATH],
            "storage-statfs.txt": ["stat", "-f", DATA_MOUNT_PATH],
            "storage-lsblk.txt": ["lsblk", "-f"],
        }
        for artifact, cmd in commands.items():
            result = self._exec_in_pod(*cmd)
            assert result.returncode == 0, (
                f"{' '.join(cmd)} failed:\nstdout:\n{result.stdout}\nstderr:\n{result.stderr}"
            )
            write_artifact(artifact, result.stdout + result.stderr)

        zpool = self._exec_in_pod("bash", "-lc", "command -v zpool >/dev/null && zpool status -x || true")
        zfs = self._exec_in_pod("bash", "-lc", "command -v zfs >/dev/null && zfs list || true")
        write_artifact("storage-zpool.txt", zpool.stdout + zpool.stderr)
        write_artifact("storage-zfs.txt", zfs.stdout + zfs.stderr)


def pvc_debug(data: dict) -> str:
    return json.dumps(data, indent=2)
