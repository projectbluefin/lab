"""Unit tests for scripts/refresh_factory_stats.py."""

from __future__ import annotations

import datetime
import json
import subprocess
import sys
from pathlib import Path

repo_root = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(repo_root / "scripts"))

import refresh_factory_stats as rfs


class TestRefreshFactoryStatsHelpers:
    def test_parse_iso(self):
        assert rfs.parse_iso(None) is None
        assert rfs.parse_iso("") is None
        assert rfs.parse_iso("invalid") is None

        dt = rfs.parse_iso("2026-01-01T12:00:00Z")
        assert dt is not None
        assert dt.year == 2026
        assert dt.tzinfo is not None

        dt_offset = rfs.parse_iso("2026-01-01T12:00:00+00:00")
        assert dt_offset is not None

    def test_argo_ui_url(self):
        assert rfs.argo_ui_url("my-wf") == "http://192.168.1.102:32746/workflows/argo/my-wf"

    def test_phase_to_overall(self):
        assert rfs.phase_to_overall("Running") == "running"
        assert rfs.phase_to_overall("Pending") == "running"
        assert rfs.phase_to_overall("Succeeded") == "passed"
        assert rfs.phase_to_overall("Failed") == "fail"
        assert rfs.phase_to_overall("Error") == "fail"
        assert rfs.phase_to_overall("Unknown") == "pending"

    def test_infer_trigger(self):
        assert rfs.infer_trigger(None) == "manual"
        assert rfs.infer_trigger("") == "manual"
        assert rfs.infer_trigger("nightly-build") == "nightly"
        assert rfs.infer_trigger("image-poll-123") == "poller"
        assert rfs.infer_trigger("digest-watch-456") == "poller"
        assert rfs.infer_trigger("pr-123-check") == "pr-poller"
        assert rfs.infer_trigger("custom-build") == "manual"

    def test_pipeline_base_name_and_key(self):
        assert rfs.pipeline_base_name("build-containerdisk-dsrlm") == "build-containerdisk"
        assert rfs.pipeline_base_name("orphan-pod-gc-1783047600") == "orphan-pod-gc"
        assert rfs.pipeline_base_name("bluefin-qa-pipeline") == "bluefin-qa-pipeline"

        assert rfs.build_pipeline_key("build-containerdisk-dsrlm") == "build-containerdisk"
        assert rfs.build_pipeline_key("dakota-qa-pipeline-abcde") == "dakota-qa-pipeline"
        assert rfs.build_pipeline_key("orphan-pod-gc-1783047600") is None

    def test_infer_label(self):
        assert rfs.infer_label([{"name": "variant", "value": "silverblue"}, {"name": "image-tag", "value": "39"}], "wf") == "silverblue:39"
        assert rfs.infer_label([{"name": "image", "value": "ghcr.io/org/bluefin"}, {"name": "image-tag", "value": "latest"}], "wf") == "bluefin:latest"
        assert rfs.infer_label([{"name": "image", "value": "ghcr.io/org/bluefin"}], "wf") == "bluefin"
        assert rfs.infer_label([], "dakota-wf") == "dakota:latest"
        assert rfs.infer_label([], "other-wf") is None

    def test_safe_int(self):
        assert rfs.safe_int(42) == 42
        assert rfs.safe_int("123") == 123
        assert rfs.safe_int("invalid", default=99) == 99

    def test_parse_mem_gib(self):
        assert rfs.parse_mem_gib(None) is None
        assert rfs.parse_mem_gib("1048576Ki") == 1
        assert rfs.parse_mem_gib("2048Mi") == 2
        assert rfs.parse_mem_gib("16Gi") == 16
        assert rfs.parse_mem_gib("invalid") is None

    def test_parse_mem_bytes(self):
        assert rfs.parse_mem_bytes(None) is None
        assert rfs.parse_mem_bytes("1Ki") == 1024
        assert rfs.parse_mem_bytes("1Mi") == 1024 * 1024
        assert rfs.parse_mem_bytes("1Gi") == 1024 * 1024 * 1024
        assert rfs.parse_mem_bytes("1Ti") == 1024 * 1024 * 1024 * 1024
        assert rfs.parse_mem_bytes("500") == 500
        assert rfs.parse_mem_bytes("invalid") is None

    def test_parse_cpu_millicores(self):
        assert rfs.parse_cpu_millicores(None) is None
        assert rfs.parse_cpu_millicores("1000000n") == 1.0
        assert rfs.parse_cpu_millicores("1000u") == 1.0
        assert rfs.parse_cpu_millicores("500m") == 500.0
        assert rfs.parse_cpu_millicores(2) == 2000.0
        assert rfs.parse_cpu_millicores("invalid") is None

    def test_gh_run_to_overall(self):
        assert rfs.gh_run_to_overall({"status": "in_progress"}) == "running"
        assert rfs.gh_run_to_overall({"status": "completed", "conclusion": "success"}) == "passed"
        assert rfs.gh_run_to_overall({"status": "completed", "conclusion": "failure"}) == "fail"
        assert rfs.gh_run_to_overall({"status": "completed", "conclusion": "timed_out"}) == "fail"
        assert rfs.gh_run_to_overall({"status": "completed", "conclusion": "startup_failure"}) == "fail"
        assert rfs.gh_run_to_overall({"status": "completed", "conclusion": "cancelled"}) == "pending"
        assert rfs.gh_run_to_overall({"status": "completed", "conclusion": "neutral"}) == "pending"

    def test_gh_run_duration_min(self):
        run_valid = {
            "run_started_at": "2026-01-01T12:00:00Z",
            "updated_at": "2026-01-01T12:35:00Z"
        }
        assert rfs.gh_run_duration_min(run_valid) == 35
        run_invalid = {
            "run_started_at": "invalid",
            "updated_at": "2026-01-01T12:35:00Z"
        }
        assert rfs.gh_run_duration_min(run_invalid) is None

    def test_sha256_file(self, tmp_path):
        f = tmp_path / "test.txt"
        f.write_text("bluefin lab", encoding="utf-8")
        h = rfs.sha256_file(f)
        assert h.startswith("sha256:")
        assert len(h) == 7 + 64

    def test_append_build_runs_ndjson(self, tmp_path, monkeypatch):
        monkeypatch.chdir(tmp_path)
        history_dir = tmp_path / "docs" / "data" / "history"
        history_dir.mkdir(parents=True, exist_ok=True)
        ndjson_file = history_dir / "build-runs.ndjson"

        # Prepopulate with old (to prune) and recent records
        old_time = (datetime.datetime.now(datetime.timezone.utc) - datetime.timedelta(days=200)).strftime("%Y-%m-%dT%H:%M:%SZ")
        recent_time = (datetime.datetime.now(datetime.timezone.utc) - datetime.timedelta(days=10)).strftime("%Y-%m-%dT%H:%M:%SZ")

        ndjson_file.write_text(
            json.dumps({"plane": "publish", "run_id": "old_run", "recorded_at": old_time}) + "\n" +
            json.dumps({"plane": "publish", "run_id": "existing_run", "recorded_at": recent_time}) + "\n"
        )

        catalog = [
            {"id": "bluefin-stable", "repo": "projectbluefin/bluefin"}
        ]
        now = datetime.datetime.now(datetime.timezone.utc).strftime("%Y-%m-%dT%H:%M:%SZ")
        stats = {
            "image_builds": {
                "bluefin-stable": [
                    {
                        "id": "existing_run",  # Already exists, should not duplicate
                        "overall": "passed",
                        "duration_min": 15,
                    },
                    {
                        "id": "new_run_1",
                        "overall": "passed",
                        "duration_min": 25,
                        "started_at": "2026-01-02T00:00:00Z",
                        "finished_at": "2026-01-02T00:25:00Z",
                        "run_url": "https://github.com/test",
                    },
                    {
                        "id": "pending_run",  # Non-terminal, should be skipped
                        "overall": "running",
                        "duration_min": None,
                    }
                ]
            },
            "build_history": {
                "bluefin-qa-pipeline": [
                    {
                        "id": "wf_run_1",
                        "overall": "failed",
                        "duration_min": 30,
                        "started_at": "2026-01-02T01:00:00Z",
                        "finished_at": "2026-01-02T01:30:00Z",
                        "run_url": "http://argo/wf_run_1",
                    }
                ]
            }
        }

        rfs.append_build_runs_ndjson(stats, catalog, now)

        lines = [json.loads(line) for line in ndjson_file.read_text().splitlines()]
        run_ids = {r["run_id"] for r in lines}

        assert "old_run" not in run_ids
        assert "existing_run" in run_ids
        assert "new_run_1" in run_ids
        assert "wf_run_1" in run_ids
        assert "pending_run" not in run_ids

        new_pub = next(r for r in lines if r["run_id"] == "new_run_1")
        assert new_pub["plane"] == "publish"
        assert new_pub["status"] == "passed"
        assert new_pub["repo"] == "projectbluefin/bluefin"

        new_lab = next(r for r in lines if r["run_id"] == "wf_run_1")
        assert new_lab["plane"] == "lab"
        assert new_lab["status"] == "failed"
        assert new_lab["repo"] == "projectbluefin/lab"


class TestRefreshFactoryStatsExecution:
    def test_main_execution(self, tmp_path, monkeypatch):
        monkeypatch.chdir(tmp_path)
        docs_data = tmp_path / "docs" / "data"
        docs_data.mkdir(parents=True)

        stats_path = docs_data / "factory-stats.json"
        stats_path.write_text(json.dumps({
            "test_coverage": {},
            "recent_runs": [],
            "image_builds": {}
        }))

        # Mock argv: issue_count=10, pr_count=5, merged_7d=3
        monkeypatch.setattr(sys, "argv", ["refresh_factory_stats.py", "10", "5", "3"])

        # Mock subprocess.check_output to simulate offline / fallback environment
        monkeypatch.setattr(subprocess, "check_output", lambda *args, **kwargs: (_ for _ in ()).throw(Exception("offline")))

        rfs.main()

        assert stats_path.exists()
        updated_stats = json.loads(stats_path.read_text())
        assert updated_stats["github"]["testing_lab"]["open_issues"] == 10
        assert updated_stats["github"]["testing_lab"]["open_prs"] == 5
        assert updated_stats["github"]["testing_lab"]["prs_merged_7d"] == 3
        assert "_meta" in updated_stats

        telemetry_path = docs_data / "factory-telemetry.json"
        assert telemetry_path.exists()
        telemetry = json.loads(telemetry_path.read_text())
        assert telemetry["schema_version"] == "v2"
        assert "metrics" in telemetry

        history_path = docs_data / "factory-history.json"
        assert history_path.exists()
        history = json.loads(history_path.read_text())
        assert history["window_days"] == 7
