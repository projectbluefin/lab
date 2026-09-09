"""Unit tests for the BuildStream / remote-execution cache collector.

Covers the pure transform layer of ``scripts/collect_bst_cache.py``:
``parse_metric_value``, ``parse_metric_samples``, ``_metric_total``,
``collect_cache_heat`` and ``persist_cache_heat``. These functions turn raw
BuildBarn Prometheus text into ``docs/data/bst-cache.json`` and the rolling
``docs/data/history/cache-heat.ndjson`` history. Per
``docs/data/page-contracts.md`` the collector must degrade to an explicit
``state: "unavailable"`` with a ``state_reason`` rather than invent values, so
the honesty contract and the retention/dedup rules of the history writer are
pinned here.
"""

from __future__ import annotations

import datetime
import json
import sys
from pathlib import Path

repo_root = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(repo_root / "scripts"))

import collect_bst_cache as collector  # noqa: E402

SIZE = "buildbarn_blobstore_blob_access_operations_blob_size_bytes"
DURATION = "buildbarn_blobstore_blob_access_operations_duration_seconds"


def _sample(metric, storage_type, operation, value, **extra):
    labels = {"storage_type": storage_type, "operation": operation}
    labels.update(extra)
    rendered = ",".join(f'{k}="{v}"' for k, v in labels.items())
    return f"{metric}{{{rendered}}} {value}"


class TestParseMetricValue:
    def test_returns_none_for_empty_text(self):
        assert collector.parse_metric_value("", "m", {"a": "b"}) is None
        assert collector.parse_metric_value(None, "m", {"a": "b"}) is None

    def test_matches_all_labels(self):
        text = 'm{a="1",b="2"} 7.5'
        assert collector.parse_metric_value(text, "m", {"a": "1", "b": "2"}) == 7.5

    def test_requires_every_label_to_match(self):
        text = 'm{a="1",b="2"} 7.5'
        assert collector.parse_metric_value(text, "m", {"a": "1", "b": "3"}) is None

    def test_ignores_other_metric_names(self):
        text = 'other{a="1"} 7.5'
        assert collector.parse_metric_value(text, "m", {"a": "1"}) is None

    def test_ignores_unlabelled_lines(self):
        # The reader only accepts ``name{`` lines, never a bare ``name value``.
        assert collector.parse_metric_value("m 7.5", "m", {}) is None

    def test_skips_unparseable_value_and_keeps_scanning(self):
        text = 'm{a="1"} notanumber\nm{a="1"} 3'
        assert collector.parse_metric_value(text, "m", {"a": "1"}) == 3.0

    def test_first_match_wins(self):
        text = 'm{a="1"} 1\nm{a="1"} 2'
        assert collector.parse_metric_value(text, "m", {"a": "1"}) == 1.0


class TestParseMetricSamples:
    def test_returns_empty_for_empty_text(self):
        assert collector.parse_metric_samples("", "m") == []
        assert collector.parse_metric_samples(None, "m") == []

    def test_parses_labels_and_value(self):
        samples = collector.parse_metric_samples('m{a="1",b="x"} 4', "m")
        assert samples == [({"a": "1", "b": "x"}, 4.0)]

    def test_parses_unlabelled_sample(self):
        assert collector.parse_metric_samples("m 4", "m") == [({}, 4.0)]

    def test_does_not_match_metric_name_prefix(self):
        # ``m_count`` must not be collected when asking for ``m``.
        assert collector.parse_metric_samples("m_count 4", "m") == []

    def test_suffixed_family_is_addressable(self):
        assert collector.parse_metric_samples("m_count 4", "m_count") == [({}, 4.0)]

    def test_ignores_comment_and_other_families(self):
        text = "# HELP m help text\nother{a=\"1\"} 9\nm{a=\"1\"} 2"
        assert collector.parse_metric_samples(text, "m") == [({"a": "1"}, 2.0)]

    def test_accepts_scientific_notation(self):
        assert collector.parse_metric_samples("m 1.5e3", "m") == [({}, 1500.0)]

    def test_collects_every_sample_in_family(self):
        text = 'm{a="1"} 1\nm{a="2"} 2'
        assert collector.parse_metric_samples(text, "m") == [
            ({"a": "1"}, 1.0),
            ({"a": "2"}, 2.0),
        ]


class TestMetricTotal:
    def test_returns_none_when_no_sample_matches(self):
        text = _sample(SIZE + "_count", "cas", "Put", 5)
        assert collector._metric_total(text, SIZE, "cas", "Get", "_count") is None

    def test_sums_matching_samples_only(self):
        text = "\n".join(
            [
                _sample(SIZE + "_count", "cas", "Get", 3, grpc_code="OK"),
                _sample(SIZE + "_count", "cas", "Get", 4, grpc_code="NotFound"),
                _sample(SIZE + "_count", "ac", "Get", 100),
                _sample(SIZE + "_count", "cas", "Put", 100),
            ]
        )
        assert collector._metric_total(text, SIZE, "cas", "Get", "_count") == 7.0

    def test_zero_total_is_distinguished_from_missing(self):
        text = _sample(SIZE + "_count", "cas", "Get", 0)
        assert collector._metric_total(text, SIZE, "cas", "Get", "_count") == 0.0


class TestCollectCacheHeat:
    def test_hits_and_misses_produce_effectiveness(self):
        text = "\n".join(
            [
                _sample(SIZE + "_count", "cas", "Get", 10),
                _sample(SIZE + "_sum", "cas", "Get", 2048),
                _sample(DURATION + "_sum", "cas", "Get", 1.5, grpc_code="OK"),
                _sample(DURATION + "_count", "cas", "Get", 8, grpc_code="OK"),
                _sample(DURATION + "_count", "cas", "Get", 2, grpc_code="NotFound"),
            ]
        )
        record = collector.collect_cache_heat(text, "storage-0", "cas")
        assert record["state"] == "available"
        assert record["state_reason"] is None
        assert record["hit_count"] == 8
        assert record["miss_count"] == 2
        assert record["effectiveness"] == 0.8
        # requests is recomputed from the status counters, not the size family.
        assert record["requests"] == 10
        assert record["bytes"] == 2048
        assert record["duration_seconds"] == 1.5
        assert record["cache_backend"] == "buildbarn"
        assert record["pod"] == "storage-0"
        assert record["storage_type"] == "cas"

    def test_all_hits_no_notfound_series_is_zero_misses(self):
        text = "\n".join(
            [
                _sample(SIZE + "_count", "cas", "Get", 5),
                _sample(DURATION + "_count", "cas", "Get", 5, grpc_code="OK"),
            ]
        )
        record = collector.collect_cache_heat(text, "storage-0", "cas")
        assert record["hit_count"] == 5
        assert record["miss_count"] == 0
        assert record["effectiveness"] == 1.0
        assert record["state"] == "available"

    def test_unaccounted_status_codes_do_not_invent_a_miss_count(self):
        # OK plus a non-NotFound failure code: hits no longer explain the total,
        # so misses are unknown and the record must be flagged unavailable.
        text = "\n".join(
            [
                _sample(SIZE + "_count", "cas", "Get", 6),
                _sample(DURATION + "_count", "cas", "Get", 5, grpc_code="OK"),
                _sample(DURATION + "_count", "cas", "Get", 1, grpc_code="Internal"),
            ]
        )
        record = collector.collect_cache_heat(text, "storage-0", "cas")
        assert record["miss_count"] is None
        assert record["effectiveness"] is None
        assert record["state"] == "unavailable"
        assert "gRPC status counters unavailable" in record["state_reason"]

    def test_partial_metrics_report_timing_and_byte_reason(self):
        text = "\n".join(
            [
                _sample(SIZE + "_count", "cas", "Get", 6),
                _sample(SIZE + "_sum", "cas", "Get", 900),
            ]
        )
        record = collector.collect_cache_heat(text, "storage-1", "cas")
        assert record["state"] == "unavailable"
        assert record["requests"] == 6
        assert record["bytes"] == 900
        assert "timing and byte" in record["state_reason"]

    def test_no_metrics_at_all_reports_blob_access_unavailable(self):
        record = collector.collect_cache_heat("", "storage-1", "ac")
        assert record["state"] == "unavailable"
        assert record["state_reason"] == "BuildBarn blob access metrics unavailable"
        assert record["hit_count"] is None
        assert record["miss_count"] is None
        assert record["requests"] is None
        assert record["bytes"] is None
        assert record["duration_seconds"] is None

    def test_requests_falls_back_to_duration_family(self):
        text = _sample(DURATION + "_count", "ac", "Get", 12)
        record = collector.collect_cache_heat(text, "storage-0", "ac")
        assert record["requests"] == 12

    def test_storage_type_isolates_cas_from_ac(self):
        text = "\n".join(
            [
                _sample(DURATION + "_count", "cas", "Get", 9, grpc_code="OK"),
                _sample(DURATION + "_count", "ac", "Get", 1, grpc_code="OK"),
            ]
        )
        assert collector.collect_cache_heat(text, "p", "cas")["hit_count"] == 9
        assert collector.collect_cache_heat(text, "p", "ac")["hit_count"] == 1

    def test_non_integer_values_are_not_truncated(self):
        text = "\n".join(
            [
                _sample(SIZE + "_sum", "cas", "Get", 1024.5),
                _sample(DURATION + "_count", "cas", "Get", 4, grpc_code="OK"),
            ]
        )
        record = collector.collect_cache_heat(text, "p", "cas")
        assert record["bytes"] == 1024.5

    def test_source_url_and_derivation_are_reported(self):
        record = collector.collect_cache_heat("", "storage-0", "cas")
        assert "pods/storage-0:9980/proxy/metrics" in record["source_url"]
        assert "NotFound is a miss" in record["derivation"]


class TestPersistCacheHeat:
    def _read(self, path):
        return [json.loads(line) for line in path.read_text().splitlines()]

    def _use_tmp_history(self, tmp_path, monkeypatch):
        path = tmp_path / "history" / "cache-heat.ndjson"
        monkeypatch.setattr(collector, "HISTORY_PATH", str(path))
        return path

    def test_creates_parent_directory_and_stamps_schema(self, tmp_path, monkeypatch):
        path = self._use_tmp_history(tmp_path, monkeypatch)
        collector.persist_cache_heat([{"pod": "storage-0"}], "2026-01-01T00:00:00+00:00")
        records = self._read(path)
        assert records == [
            {
                "schema_version": "1.0",
                "recorded_at": "2026-01-01T00:00:00+00:00",
                "pod": "storage-0",
            }
        ]

    def test_appends_across_distinct_collections(self, tmp_path, monkeypatch):
        path = self._use_tmp_history(tmp_path, monkeypatch)
        now = datetime.datetime.now(datetime.timezone.utc)
        first = (now - datetime.timedelta(days=1)).isoformat()
        second = now.isoformat()
        collector.persist_cache_heat([{"pod": "a"}], first)
        collector.persist_cache_heat([{"pod": "b"}], second)
        records = self._read(path)
        assert [r["pod"] for r in records] == ["a", "b"]

    def test_rerunning_the_same_collection_replaces_not_duplicates(
        self, tmp_path, monkeypatch
    ):
        path = self._use_tmp_history(tmp_path, monkeypatch)
        stamp = datetime.datetime.now(datetime.timezone.utc).isoformat()
        collector.persist_cache_heat([{"pod": "a", "hit_count": 1}], stamp)
        collector.persist_cache_heat([{"pod": "a", "hit_count": 2}], stamp)
        records = self._read(path)
        assert len(records) == 1
        assert records[0]["hit_count"] == 2

    def test_prunes_records_older_than_retention_window(self, tmp_path, monkeypatch):
        path = self._use_tmp_history(tmp_path, monkeypatch)
        now = datetime.datetime.now(datetime.timezone.utc)
        stale = (
            now - datetime.timedelta(days=collector.HISTORY_RETENTION_DAYS + 1)
        ).isoformat()
        fresh = (now - datetime.timedelta(days=1)).isoformat()
        collector.persist_cache_heat([{"pod": "stale"}], stale)
        collector.persist_cache_heat([{"pod": "fresh"}], fresh)
        collector.persist_cache_heat([{"pod": "new"}], now.isoformat())
        assert [r["pod"] for r in self._read(path)] == ["fresh", "new"]

    def test_drops_malformed_and_undated_existing_lines(self, tmp_path, monkeypatch):
        path = self._use_tmp_history(tmp_path, monkeypatch)
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_text(
            "not json\n"
            + json.dumps({"pod": "no-timestamp"})
            + "\n"
            + json.dumps({"pod": "bad-timestamp", "recorded_at": "whenever"})
            + "\n"
        )
        collector.persist_cache_heat([{"pod": "new"}], "2026-01-01T00:00:00+00:00")
        assert [r["pod"] for r in self._read(path)] == ["new"]

    def test_zulu_timestamps_are_retained(self, tmp_path, monkeypatch):
        path = self._use_tmp_history(tmp_path, monkeypatch)
        now = datetime.datetime.now(datetime.timezone.utc)
        zulu = (now - datetime.timedelta(days=2)).strftime("%Y-%m-%dT%H:%M:%SZ")
        collector.persist_cache_heat([{"pod": "old"}], zulu)
        collector.persist_cache_heat([{"pod": "new"}], now.isoformat())
        assert [r["pod"] for r in self._read(path)] == ["old", "new"]

    def test_writes_every_record_of_one_collection(self, tmp_path, monkeypatch):
        path = self._use_tmp_history(tmp_path, monkeypatch)
        stamp = datetime.datetime.now(datetime.timezone.utc).isoformat()
        collector.persist_cache_heat(
            [{"pod": "storage-0"}, {"pod": "storage-1"}], stamp
        )
        assert [r["pod"] for r in self._read(path)] == ["storage-0", "storage-1"]

    def test_empty_collection_still_prunes_history(self, tmp_path, monkeypatch):
        path = self._use_tmp_history(tmp_path, monkeypatch)
        stale = (
            datetime.datetime.now(datetime.timezone.utc)
            - datetime.timedelta(days=collector.HISTORY_RETENTION_DAYS + 1)
        ).isoformat()
        collector.persist_cache_heat([{"pod": "stale"}], stale)
        collector.persist_cache_heat([], "2026-01-01T00:00:00+00:00")
        assert self._read(path) == []
