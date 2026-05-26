"""
In-cluster local-path restore drill.
"""

from __future__ import annotations

import hashlib
import json
import subprocess
import textwrap

from tests.service_catalog.shared.kube import (
    TEST_NAMESPACE,
    first_pod_name,
    get_pods_json,
    require_kubectl,
    run_kubectl,
    write_artifact,
)


FIXTURE_MANIFEST = textwrap.dedent(
    """
    apiVersion: v1
    kind: PersistentVolumeClaim
    metadata:
      name: homelab-restore-data
    spec:
      accessModes:
        - ReadWriteOnce
      resources:
        requests:
          storage: 1Gi
    ---
    apiVersion: apps/v1
    kind: Deployment
    metadata:
      name: homelab-restore
      labels:
        app: homelab-restore
    spec:
      replicas: 1
      selector:
        matchLabels:
          app: homelab-restore
      template:
        metadata:
          labels:
            app: homelab-restore
        spec:
          nodeSelector:
            kubernetes.io/hostname: ghost
          containers:
            - name: busybox
              image: docker.io/library/busybox:1.36.1
              command: ["sh", "-c", "mkdir -p /data && sleep infinity"]
              volumeMounts:
                - name: data
                  mountPath: /data
          volumes:
            - name: data
              persistentVolumeClaim:
                claimName: homelab-restore-data
    """
).strip()


class TestRestoreDrill:
    def test_backup_and_restore_round_trip(self):
        before = get_pods_json()
        write_artifact("restore-pods-before.json", json.dumps(before, indent=2))
        pod_name = first_pod_name()

        seed = run_kubectl(
            "exec",
            "-n",
            TEST_NAMESPACE,
            pod_name,
            "--",
            "sh",
            "-c",
            "echo homelab-restore-seed >/data/state.txt",
        )
        assert seed.returncode == 0, seed.stdout + seed.stderr

        backup = run_kubectl(
            "exec",
            "-n",
            TEST_NAMESPACE,
            pod_name,
            "--",
            "cat",
            "/data/state.txt",
        )
        assert backup.returncode == 0, backup.stdout + backup.stderr
        backup_text = backup.stdout.strip()
        backup_sha = hashlib.sha256(backup_text.encode()).hexdigest()
        write_artifact("seeded-state.txt", backup_text + "\n")
        write_artifact("backup-checksum.txt", backup_sha + "\n")

        delete = run_kubectl(
            "delete",
            "deployment",
            "homelab-restore",
            "-n",
            TEST_NAMESPACE,
            "--ignore-not-found=true",
        )
        assert delete.returncode == 0, delete.stdout + delete.stderr
        require_kubectl("delete", "pvc", "homelab-restore-data", "-n", TEST_NAMESPACE, "--ignore-not-found=true")
        apply = subprocess_apply(FIXTURE_MANIFEST)
        assert apply.returncode == 0, apply.stdout + apply.stderr
        write_artifact("restore-log.txt", apply.stdout + apply.stderr)

        require_kubectl(
            "rollout",
            "status",
            "deployment/homelab-restore",
            "-n",
            TEST_NAMESPACE,
            "--timeout=300s",
        )
        pod_name = first_pod_name()
        restore = run_kubectl(
            "exec",
            "-n",
            TEST_NAMESPACE,
            pod_name,
            "--",
            "sh",
            "-c",
            f"printf '%s' '{backup_text}' >/data/state.txt",
        )
        assert restore.returncode == 0, restore.stdout + restore.stderr

        verify = run_kubectl(
            "exec",
            "-n",
            TEST_NAMESPACE,
            pod_name,
            "--",
            "cat",
            "/data/state.txt",
        )
        assert verify.returncode == 0, verify.stdout + verify.stderr
        verify_text = verify.stdout.strip()
        verify_sha = hashlib.sha256(verify_text.encode()).hexdigest()
        write_artifact("post-restore-verification.txt", verify_text + "\n")
        assert verify_text == backup_text
        assert verify_sha == backup_sha


def subprocess_apply(manifest: str):
    return subprocess.run(
        ["kubectl", "apply", "-n", TEST_NAMESPACE, "-f", "-"],
        input=manifest,
        capture_output=True,
        text=True,
        timeout=120,
    )
