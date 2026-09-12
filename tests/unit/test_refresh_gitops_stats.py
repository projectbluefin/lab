"""Unit tests for scripts/refresh_gitops_stats.py."""

from __future__ import annotations

import json
import subprocess
import sys
from pathlib import Path
from unittest.mock import patch

repo_root = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(repo_root / "scripts"))

import refresh_gitops_stats as rgs


class TestRefreshGitopsStats:
    def test_run_cmd_success(self):
        result = rgs.run_cmd("echo 'hello world'")
        assert result == "hello world"

    def test_run_cmd_failure(self):
        result = rgs.run_cmd("false")
        assert result is None

    def test_run_cmd_exception(self):
        with patch("subprocess.check_output", side_effect=subprocess.TimeoutExpired(cmd="test", timeout=15)):
            assert rgs.run_cmd("test") is None

    def test_main_fallback_mode(self, tmp_path, monkeypatch):
        # Change working directory so docs/data is written inside tmp_path
        monkeypatch.chdir(tmp_path)
        monkeypatch.setattr(rgs, "run_cmd", lambda cmd: None)

        rgs.main()

        status_file = tmp_path / "docs" / "data" / "gitops-status.json"
        deployments_file = tmp_path / "docs" / "data" / "gitops-deployments.json"

        assert status_file.exists()
        assert deployments_file.exists()

        status_data = json.loads(status_file.read_text())
        assert status_data["schema_version"] == "v1"
        assert status_data["_meta"]["live_snapshot_ok"] is False
        apps = status_data["applications"]
        assert len(apps) == 6
        app_names = {a["name"] for a in apps}
        assert "arc-runners" in app_names
        assert "testing-lab-infra" in app_names

        deployments_data = json.loads(deployments_file.read_text())
        assert deployments_data["schema_version"] == "v1"
        assert deployments_data["_meta"]["live_snapshot_ok"] is False
        assert deployments_data["deployments"] == []

    def test_main_live_applications(self, tmp_path, monkeypatch):
        monkeypatch.chdir(tmp_path)

        mock_argo_json = json.dumps({
            "items": [
                {
                    "metadata": {"name": "test-app", "namespace": "argocd"},
                    "spec": {
                        "source": {
                            "path": "manifests/test",
                            "repoURL": "https://github.com/projectbluefin/lab",
                            "targetRevision": "main"
                        },
                        "destination": {"namespace": "test-ns"}
                    },
                    "status": {
                        "sync": {"status": "Synced"},
                        "health": {"status": "Healthy"},
                        "resources": [
                            {
                                "group": "apps",
                                "kind": "Deployment",
                                "name": "my-dep",
                                "namespace": "test-ns",
                                "status": "OutOfSync"
                            },
                            {
                                "group": "",
                                "kind": "Service",
                                "name": "my-svc",
                                "namespace": "test-ns",
                                "status": "Synced"
                            }
                        ],
                        "history": [
                            {
                                "id": 1,
                                "revision": "abcdef1",
                                "deployStartedAt": "2026-01-01T00:00:00Z",
                                "deployedAt": "2026-01-01T00:05:00Z"
                            },
                            {
                                "id": 2,
                                "revision": "abcdef2",
                                "deployStartedAt": "2026-01-02T00:00:00Z",
                                "deployedAt": None
                            }
                        ]
                    }
                }
            ]
        })

        monkeypatch.setattr(rgs, "run_cmd", lambda cmd: mock_argo_json)

        rgs.main()

        status_file = tmp_path / "docs" / "data" / "gitops-status.json"
        deployments_file = tmp_path / "docs" / "data" / "gitops-deployments.json"

        status_data = json.loads(status_file.read_text())
        assert status_data["_meta"]["live_snapshot_ok"] is True
        assert len(status_data["applications"]) == 1
        app = status_data["applications"][0]
        assert app["name"] == "test-app"
        assert app["sync_status"] == "Synced"
        assert app["health_status"] == "Healthy"
        assert app["drifted_count"] == 1
        assert app["drifted_resources"][0]["name"] == "my-dep"

        deployments_data = json.loads(deployments_file.read_text())
        assert deployments_data["_meta"]["live_snapshot_ok"] is True
        deps = deployments_data["deployments"]
        assert len(deps) == 2
        # sorted latest first (2026-01-02 before 2026-01-01)
        assert deps[0]["id"] == 2
        assert deps[0]["status"] == "failed"
        assert deps[1]["id"] == 1
        assert deps[1]["status"] == "passed"
