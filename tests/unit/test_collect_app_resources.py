"""Unit tests for scripts/collect_app_resources.py."""

from __future__ import annotations

import json
import sys
from pathlib import Path
from unittest.mock import patch

import pytest

repo_root = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(repo_root / "scripts"))

import collect_app_resources as car


class TestCollectAppResources:
    def test_run_cmd(self):
        assert car.run_cmd("echo 'test'") == "test"
        assert car.run_cmd("false") is None
        with patch("subprocess.check_output", side_effect=Exception("boom")):
            assert car.run_cmd("echo 'fail'") is None

    @pytest.mark.parametrize(
        "raw,expected",
        [
            ("", 0.0),
            (None, 0.0),
            ("1000000000n", 1.0),
            ("500000000n", 0.5),
            ("1000000u", 1.0),
            ("500000u", 0.5),
            ("500m", 0.5),
            ("2000m", 2.0),
            ("2", 2.0),
            ("2.5", 2.5),
            ("invalid", 0.0),
        ],
    )
    def test_parse_cpu_cores(self, raw, expected):
        assert car.parse_cpu_cores(raw) == pytest.approx(expected)

    @pytest.mark.parametrize(
        "raw,expected",
        [
            ("", 0.0),
            (None, 0.0),
            ("1024Ki", 1.0),
            ("512Mi", 512.0),
            ("2Gi", 2048.0),
            ("1Ti", 1024.0 * 1024.0),
            ("1048576", 1.0),  # bytes
            ("invalid", 0.0),
        ],
    )
    def test_parse_mem_mib(self, raw, expected):
        assert car.parse_mem_mib(raw) == pytest.approx(expected)

    def test_get_app_for_pod(self):
        # Workflow label
        assert car.get_app_for_pod("wf-pod", "default", {"workflows.argoproj.io/workflow": "my-wf"}) == "testing-lab"

        # Namespace matching
        assert car.get_app_for_pod("runner-pod", "arc-runners", {}) == "arc-runners"
        assert car.get_app_for_pod("sys-pod", "arc-systems", {}) == "arc-systems"

        # argo namespace
        assert car.get_app_for_pod("argo-server-123", "argo", {}) == "argo-workflows"
        assert car.get_app_for_pod("argo-workflows-workflow-controller-456", "argo", {}) == "argo-workflows"
        assert car.get_app_for_pod("k8sgpt-789", "argo", {}) == "testing-lab-infra"

        # flatcar-update & system-upgrade
        assert car.get_app_for_pod("any-pod", "flatcar-update", {}) == "flatcar-update"
        assert car.get_app_for_pod("any-pod", "system-upgrade", {}) == "flatcar-update"

        # buildbarn, local-registry, cdi, kubevirt
        assert car.get_app_for_pod("storage-0", "buildbarn", {}) == "testing-lab-infra"
        assert car.get_app_for_pod("registry-pod", "local-registry", {}) == "testing-lab-infra"
        assert car.get_app_for_pod("cdi-pod", "cdi", {}) == "testing-lab-infra"
        assert car.get_app_for_pod("virt-pod", "kubevirt", {}) == "testing-lab-infra"

        # Standard label fallback
        assert car.get_app_for_pod("custom", "other", {"argocd.argoproj.io/instance": "arc-runners"}) == "arc-runners"
        assert car.get_app_for_pod("custom", "other", {"app.kubernetes.io/instance": "testing-lab"}) == "testing-lab"
        assert car.get_app_for_pod("custom", "other", {"app.kubernetes.io/instance": "unknown-app"}) is None
        assert car.get_app_for_pod("custom", "other", {}) is None

    def test_main_fallback_when_offline(self, tmp_path, monkeypatch):
        monkeypatch.chdir(tmp_path)
        monkeypatch.setattr(car, "run_cmd", lambda cmd: None)

        car.main()

        out_file = tmp_path / "docs" / "data" / "app-resource-usage.json"
        assert out_file.exists()

        data = json.loads(out_file.read_text())
        assert data["schema_version"] == "v1"
        assert data["_meta"]["live_snapshot_ok"] is False
        apps = {a["name"]: a for a in data["applications"]}
        assert "arc-systems" in apps
        assert apps["arc-systems"]["pods_count"] == 2
        assert apps["arc-systems"]["cpu"]["usage"] == 0.15
        assert apps["arc-systems"]["memory"]["usage"] == 128.0

    def test_main_live_pods_and_metrics(self, tmp_path, monkeypatch):
        monkeypatch.chdir(tmp_path)

        pods_json = json.dumps({
            "items": [
                {
                    "metadata": {
                        "name": "runner-pod-1",
                        "namespace": "arc-runners",
                        "labels": {}
                    },
                    "spec": {
                        "containers": [
                            {
                                "name": "runner",
                                "resources": {
                                    "requests": {"cpu": "500m", "memory": "256Mi"},
                                    "limits": {"cpu": "1000m", "memory": "512Mi"}
                                }
                            }
                        ],
                        "initContainers": [
                            {
                                "name": "init",
                                "resources": {
                                    "requests": {"cpu": "100m", "memory": "64Mi"},
                                    "limits": {"cpu": "200m", "memory": "128Mi"}
                                }
                            }
                        ]
                    },
                    "status": {
                        "phase": "Running"
                    }
                },
                {
                    "metadata": {
                        "name": "terminated-pod",
                        "namespace": "arc-runners",
                        "labels": {}
                    },
                    "spec": {"containers": []},
                    "status": {
                        "phase": "Succeeded"
                    }
                }
            ]
        })

        metrics_json = json.dumps({
            "items": [
                {
                    "metadata": {
                        "name": "runner-pod-1",
                        "namespace": "arc-runners"
                    },
                    "containers": [
                        {
                            "name": "runner",
                            "usage": {"cpu": "250m", "memory": "128Mi"}
                        }
                    ]
                }
            ]
        })

        def mock_cmd(cmd):
            if "kubectl get pods" in cmd:
                return pods_json
            if "metrics.k8s.io" in cmd:
                return metrics_json
            return None

        monkeypatch.setattr(car, "run_cmd", mock_cmd)

        car.main()

        out_file = tmp_path / "docs" / "data" / "app-resource-usage.json"
        assert out_file.exists()

        data = json.loads(out_file.read_text())
        assert data["_meta"]["live_snapshot_ok"] is True
        apps = {a["name"]: a for a in data["applications"]}
        runners = apps["arc-runners"]
        assert runners["pods_count"] == 1
        assert runners["cpu"]["usage"] == 0.25
        assert runners["cpu"]["request"] == 0.6  # 500m + 100m
        assert runners["cpu"]["limit"] == 1.2    # 1000m + 200m
        assert runners["memory"]["usage"] == 128.0
        assert runners["memory"]["request"] == 320.0  # 256 + 64
        assert runners["memory"]["limit"] == 640.0    # 512 + 128
