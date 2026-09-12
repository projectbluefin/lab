"""Unit tests for scripts/collect_bst_cache.py."""

from __future__ import annotations

import datetime
import json
import sys
from pathlib import Path
from unittest.mock import patch

import pytest

repo_root = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(repo_root / "scripts"))

import collect_bst_cache as cbc

SAMPLE_METRICS = """
# HELP buildbarn_blobstore_block_device_backed_block_allocator_allocations_total Total number of blocks allocated.
# TYPE buildbarn_blobstore_block_device_backed_block_allocator_allocations_total counter
buildbarn_blobstore_block_device_backed_block_allocator_allocations_total{storage_type="cas"} 10
buildbarn_blobstore_block_device_backed_block_allocator_allocations_total{storage_type="ac"} 5
# HELP buildbarn_blobstore_block_device_backed_block_allocator_releases_total Total number of blocks released.
# TYPE buildbarn_blobstore_block_device_backed_block_allocator_releases_total counter
buildbarn_blobstore_block_device_backed_block_allocator_releases_total{storage_type="cas"} 2
buildbarn_blobstore_block_device_backed_block_allocator_releases_total{storage_type="ac"} 1
# HELP buildbarn_blobstore_blob_access_operations_duration_seconds Blob access operations duration
buildbarn_blobstore_blob_access_operations_duration_seconds_count{grpc_code="OK",operation="Get",storage_type="cas"} 80
buildbarn_blobstore_blob_access_operations_duration_seconds_count{grpc_code="NotFound",operation="Get",storage_type="cas"} 20
buildbarn_blobstore_blob_access_operations_duration_seconds_sum{grpc_code="OK",operation="Get",storage_type="cas"} 1.5
buildbarn_blobstore_blob_access_operations_duration_seconds_sum{grpc_code="NotFound",operation="Get",storage_type="cas"} 0.5
# HELP buildbarn_blobstore_blob_access_operations_blob_size_bytes Blob access operations blob size
buildbarn_blobstore_blob_access_operations_blob_size_bytes_count{operation="Get",storage_type="cas"} 100
buildbarn_blobstore_blob_access_operations_blob_size_bytes_sum{operation="Get",storage_type="cas"} 5000000
"""


class TestCollectBstCache:
    def test_run_raw(self):
        with patch("subprocess.check_output", return_value="raw-data\n"):
            assert cbc.run_raw("/some/path") == "raw-data\n"
        with patch("subprocess.check_output", side_effect=Exception("fail")):
            assert cbc.run_raw("/some/path") is None

    def test_run_json_raw(self):
        with patch("subprocess.check_output", return_value='{"key": "value"}'):
            assert cbc.run_json_raw("/some/path") == {"key": "value"}
        with patch("subprocess.check_output", return_value='invalid json'):
            assert cbc.run_json_raw("/some/path") is None
        with patch("subprocess.check_output", side_effect=Exception("fail")):
            assert cbc.run_json_raw("/some/path") is None

    def test_parse_metric_value(self):
        assert cbc.parse_metric_value(None, "metric", {}) is None
        val_cas = cbc.parse_metric_value(
            SAMPLE_METRICS,
            "buildbarn_blobstore_block_device_backed_block_allocator_allocations_total",
            {"storage_type": "cas"},
        )
        assert val_cas == 10.0

        val_ac = cbc.parse_metric_value(
            SAMPLE_METRICS,
            "buildbarn_blobstore_block_device_backed_block_allocator_allocations_total",
            {"storage_type": "ac"},
        )
        assert val_ac == 5.0

        val_missing = cbc.parse_metric_value(SAMPLE_METRICS, "nonexistent_metric", {})
        assert val_missing is None

    def test_parse_metric_samples(self):
        assert cbc.parse_metric_samples(None, "metric") == []
        samples = cbc.parse_metric_samples(
            SAMPLE_METRICS,
            "buildbarn_blobstore_blob_access_operations_duration_seconds_count",
        )
        assert len(samples) == 2
        labels, val = samples[0]
        assert labels["operation"] == "Get"
        assert labels["grpc_code"] == "OK"
        assert val == 80.0

    def test_collect_cache_heat_available(self):
        heat = cbc.collect_cache_heat(SAMPLE_METRICS, "storage-0", "cas")
        assert heat["cache_backend"] == "buildbarn"
        assert heat["pod"] == "storage-0"
        assert heat["storage_type"] == "cas"
        assert heat["hit_count"] == 80
        assert heat["miss_count"] == 20
        assert heat["requests"] == 100
        assert heat["effectiveness"] == 0.8
        assert heat["bytes"] == 5000000
        assert heat["duration_seconds"] == 2.0
        assert heat["state"] == "available"
        assert heat["state_reason"] is None

    def test_collect_cache_heat_unavailable(self):
        heat = cbc.collect_cache_heat("", "storage-0", "cas")
        assert heat["state"] == "unavailable"
        assert heat["hit_count"] is None
        assert heat["miss_count"] is None
        assert heat["effectiveness"] is None
        assert "unavailable" in heat["state_reason"]

    def test_persist_cache_heat(self, tmp_path, monkeypatch):
        history_file = tmp_path / "cache-heat.ndjson"
        monkeypatch.setattr(cbc, "HISTORY_PATH", str(history_file))

        # Write an old record (older than 180 days) and a recent record
        old_time = (datetime.datetime.now(datetime.timezone.utc) - datetime.timedelta(days=200)).strftime("%Y-%m-%dT%H:%M:%SZ")
        recent_time = (datetime.datetime.now(datetime.timezone.utc) - datetime.timedelta(days=10)).strftime("%Y-%m-%dT%H:%M:%SZ")

        history_file.write_text(
            json.dumps({"schema_version": "1.0", "recorded_at": old_time, "hit_count": 10}) + "\n" +
            json.dumps({"schema_version": "1.0", "recorded_at": recent_time, "hit_count": 20}) + "\n"
        )

        now = datetime.datetime.now(datetime.timezone.utc).strftime("%Y-%m-%dT%H:%M:%SZ")
        records = [{"hit_count": 50, "miss_count": 5}]

        cbc.persist_cache_heat(records, now)

        lines = [json.loads(line) for line in history_file.read_text().splitlines()]
        # old_time should be pruned, recent_time kept, now added
        timestamps = [rec["recorded_at"] for rec in lines]
        assert old_time not in timestamps
        assert recent_time in timestamps
        assert now in timestamps

    def test_node_for_pod(self):
        with patch.object(cbc, "run_json_raw", return_value={"spec": {"nodeName": "ghost"}}):
            assert cbc.node_for_pod("buildbarn", "storage-0") == "ghost"
        with patch.object(cbc, "run_json_raw", return_value=None):
            assert cbc.node_for_pod("buildbarn", "storage-0") is None

    def test_main_unavailable_cluster(self, tmp_path, monkeypatch):
        monkeypatch.chdir(tmp_path)
        out_file = tmp_path / "docs" / "data" / "bst-cache.json"
        history_file = tmp_path / "docs" / "data" / "history" / "cache-heat.ndjson"
        monkeypatch.setattr(cbc, "OUT_PATH", str(out_file))
        monkeypatch.setattr(cbc, "HISTORY_PATH", str(history_file))

        monkeypatch.setattr(cbc, "run_raw", lambda p: None)
        monkeypatch.setattr(cbc, "run_json_raw", lambda p: None)

        cbc.main()

        assert out_file.exists()
        doc = json.loads(out_file.read_text())
        assert doc["schema_version"] == 1
        assert doc["_meta"]["freshness_state"] == "unavailable"
        assert len(doc["rows"]) == 5  # 1 bazel-remote + 2 storage pods * 2 storage types (cas, ac)
        for row in doc["rows"]:
            assert row["state"] == "unavailable"

    def test_main_live_cluster(self, tmp_path, monkeypatch):
        monkeypatch.chdir(tmp_path)
        out_file = tmp_path / "docs" / "data" / "bst-cache.json"
        history_file = tmp_path / "docs" / "data" / "history" / "cache-heat.ndjson"
        monkeypatch.setattr(cbc, "OUT_PATH", str(out_file))
        monkeypatch.setattr(cbc, "HISTORY_PATH", str(history_file))

        def mock_raw(path):
            if "nodes/ghost" in path or "nodes/exo-0" in path:
                return json.dumps({
                    "metadata": {
                        "annotations": {
                            "lab.projectbluefin.io/usb4-link": "up",
                            "lab.projectbluefin.io/usb4-link-observed-at": "2026-01-01T00:00:00Z"
                        }
                    }
                })
            if "metrics" in path:
                return SAMPLE_METRICS
            return None

        def mock_json_raw(path):
            if "status" in path:
                return {"CurrSize": 1000, "MaxSize": 5000}
            if "pods/storage-0" in path:
                return {"spec": {"nodeName": "ghost"}}
            if "pods/storage-1" in path:
                return {"spec": {"nodeName": "exo-0"}}
            return None

        monkeypatch.setattr(cbc, "run_raw", mock_raw)
        monkeypatch.setattr(cbc, "run_json_raw", mock_json_raw)

        cbc.main()

        assert out_file.exists()
        doc = json.loads(out_file.read_text())
        assert doc["_meta"]["freshness_state"] == "fresh"
        assert doc["usb4_telemetry"]["status"] == "up"

        rows_by_id = {r["id"]: r for r in doc["rows"]}
        bazel_row = rows_by_id["bazel-remote-ghost"]
        assert bazel_row["state"] == "available"
        assert bazel_row["used_bytes"] == 1000
        assert bazel_row["percent"] == 20.0

        cas_row_0 = rows_by_id["buildbarn-storage-0-cas"]
        assert cas_row_0["state"] == "available"
        assert cas_row_0["node"] == "ghost"
        # 10 allocations - 2 releases = 8 blocks. block_size = 20GiB / 35
        expected_used = 8 * (cbc.CAS_CAPACITY_BYTES / cbc.CAS_BLOCKS)
        assert cas_row_0["used_bytes"] == pytest.approx(expected_used)
