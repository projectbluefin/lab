"""Unit tests for the linuxserver.io catalog collector.

Covers the pure transform layer of ``scripts/collect_lsio_catalog.py``:
``as_int``, ``extract_architectures``, ``config_pointer``, ``map_image``,
``build_index`` and ``now_iso``. These functions produce
``docs/data/catalog/linuxserver.json``, which is regenerated on a cadence, so a
silent mapping regression would ship straight into the catalog index consumed
by the applications page.
"""

from __future__ import annotations

import json
import re
import sys
from pathlib import Path

repo_root = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(repo_root / "scripts"))

import collect_lsio_catalog as collector  # noqa: E402

FIXTURES = Path(__file__).resolve().parent / "fixtures"
CATALOG_FIXTURE = FIXTURES / "linuxserver-catalog.json"


def _load_fixture():
    with open(CATALOG_FIXTURE) as f:
        return json.load(f)


class TestAsInt:
    def test_passes_through_int(self):
        assert collector.as_int(42) == 42

    def test_coerces_numeric_string(self):
        assert collector.as_int("2955411") == 2955411

    def test_truncates_float(self):
        assert collector.as_int(3.9) == 3

    def test_none_becomes_none(self):
        assert collector.as_int(None) is None

    def test_non_numeric_string_becomes_none(self):
        assert collector.as_int("many") is None

    def test_list_becomes_none(self):
        assert collector.as_int(["1"]) is None

    def test_zero_is_preserved_not_dropped(self):
        """0 must survive as 0, not collapse to None via truthiness."""
        assert collector.as_int(0) == 0


class TestExtractArchitectures:
    def test_extracts_arch_names_in_order(self):
        image = {"architectures": [{"arch": "x86_64"}, {"arch": "arm64"}]}
        assert collector.extract_architectures(image) == ["x86_64", "arm64"]

    def test_deduplicates_preserving_first_occurrence(self):
        image = {
            "architectures": [
                {"arch": "x86_64", "tag": "amd64-latest"},
                {"arch": "arm64", "tag": "arm64v8-latest"},
                {"arch": "x86_64", "tag": "amd64-develop"},
            ]
        }
        assert collector.extract_architectures(image) == ["x86_64", "arm64"]

    def test_skips_entries_without_arch(self):
        image = {"architectures": [{"tag": "amd64-latest"}, {"arch": "arm64"}, {"arch": ""}]}
        assert collector.extract_architectures(image) == ["arm64"]

    def test_missing_key_yields_empty_list(self):
        assert collector.extract_architectures({}) == []

    def test_null_architectures_yields_empty_list(self):
        assert collector.extract_architectures({"architectures": None}) == []


class TestConfigPointer:
    def test_prefers_application_setup(self):
        image = {
            "config": {"application_setup": "https://example.invalid/setup"},
            "project_url": "https://example.invalid/project",
            "github_url": "https://example.invalid/github",
        }
        assert collector.config_pointer(image) == "https://example.invalid/setup"

    def test_falls_back_to_project_url(self):
        image = {
            "config": {},
            "project_url": "https://example.invalid/project",
            "github_url": "https://example.invalid/github",
        }
        assert collector.config_pointer(image) == "https://example.invalid/project"

    def test_falls_back_to_github_url(self):
        image = {"config": {}, "github_url": "https://example.invalid/github"}
        assert collector.config_pointer(image) == "https://example.invalid/github"

    def test_null_config_does_not_raise(self):
        image = {"config": None, "project_url": "https://example.invalid/project"}
        assert collector.config_pointer(image) == "https://example.invalid/project"

    def test_all_sources_absent_yields_none(self):
        assert collector.config_pointer({}) is None

    def test_empty_application_setup_falls_through(self):
        image = {"config": {"application_setup": ""}, "project_url": "https://example.invalid/project"}
        assert collector.config_pointer(image) == "https://example.invalid/project"


class TestMapImage:
    def test_maps_fixture_image_to_schema(self):
        images = _load_fixture()["data"]["repositories"]["linuxserver"]
        jellyfin = next(i for i in images if i["name"] == "jellyfin")

        mapped = collector.map_image(jellyfin)

        assert mapped["name"] == "jellyfin"
        assert mapped["category"] == "Media Servers,Music,Audiobooks"
        assert mapped["image_ref"] == "lscr.io/linuxserver/jellyfin"
        assert mapped["monthly_pulls"] == 2955411
        assert mapped["stars"] == 888
        assert mapped["architectures"] == ["x86_64", "arm64"]
        assert mapped["config_pointer"].startswith("https://github.com/linuxserver/docker-jellyfin")
        assert mapped["verified"] is False

    def test_emits_exactly_the_schema_keys(self):
        images = _load_fixture()["data"]["repositories"]["linuxserver"]
        mapped = collector.map_image(images[0])

        assert set(mapped) == {
            "name",
            "description",
            "category",
            "logo_url",
            "image_ref",
            "monthly_pulls",
            "stars",
            "architectures",
            "config_pointer",
            "readonly_supported",
            "nonroot_supported",
            "verified",
        }

    def test_image_ref_uses_the_lscr_prefix(self):
        mapped = collector.map_image({"name": "radarr"})
        assert mapped["image_ref"] == f"{collector.IMAGE_PREFIX}/radarr"

    def test_missing_name_yields_null_image_ref(self):
        """A nameless image must not produce 'lscr.io/linuxserver/None'."""
        mapped = collector.map_image({"description": "orphan"})
        assert mapped["name"] is None
        assert mapped["image_ref"] is None

    def test_support_flags_are_booleans_not_passthrough(self):
        mapped = collector.map_image(
            {"name": "app", "config": {"readonly_supported": "yes", "nonroot_supported": None}}
        )
        assert mapped["readonly_supported"] is True
        assert mapped["nonroot_supported"] is False

    def test_support_flags_default_false_when_config_missing(self):
        mapped = collector.map_image({"name": "app"})
        assert mapped["readonly_supported"] is False
        assert mapped["nonroot_supported"] is False

    def test_non_numeric_counters_become_none(self):
        mapped = collector.map_image({"name": "app", "monthly_pulls": "n/a", "stars": None})
        assert mapped["monthly_pulls"] is None
        assert mapped["stars"] is None

    def test_verified_is_always_false_even_if_upstream_claims_true(self):
        """Verification is a local decision; upstream must not be able to set it."""
        mapped = collector.map_image({"name": "app", "verified": True})
        assert mapped["verified"] is False


class TestBuildIndex:
    def test_index_envelope(self):
        index = collector.build_index(_load_fixture())

        assert index["provider"] == collector.PROVIDER == "linuxserver"
        assert index["source_api"] == collector.API_URL
        assert isinstance(index["apps"], list)
        assert set(index) == {"provider", "generated_at", "source_api", "apps"}

    def test_maps_every_named_fixture_app(self):
        index = collector.build_index(_load_fixture())
        assert [a["name"] for a in index["apps"]] == ["jellyfin", "sonarr"]

    def test_apps_are_sorted_by_name(self):
        data = {
            "data": {
                "repositories": {
                    "linuxserver": [{"name": "sonarr"}, {"name": "audacity"}, {"name": "jellyfin"}]
                }
            }
        }
        names = [a["name"] for a in collector.build_index(data)["apps"]]
        assert names == sorted(names) == ["audacity", "jellyfin", "sonarr"]

    def test_drops_entries_without_a_name(self):
        data = {
            "data": {
                "repositories": {
                    "linuxserver": [{"name": "jellyfin"}, {"description": "no name"}, {"name": ""}]
                }
            }
        }
        assert [a["name"] for a in collector.build_index(data)["apps"]] == ["jellyfin"]

    def test_empty_payload_yields_empty_apps(self):
        assert collector.build_index({})["apps"] == []

    def test_null_repositories_yields_empty_apps(self):
        assert collector.build_index({"data": {"repositories": None}})["apps"] == []

    def test_other_repositories_are_ignored(self):
        """Only the 'linuxserver' repository feeds the linuxserver index."""
        data = {
            "data": {
                "repositories": {
                    "linuxserver": [{"name": "jellyfin"}],
                    "lspipepr": [{"name": "should-not-appear"}],
                }
            }
        }
        assert [a["name"] for a in collector.build_index(data)["apps"]] == ["jellyfin"]

    def test_index_is_json_serializable(self):
        json.dumps(collector.build_index(_load_fixture()))


class TestNowIso:
    def test_returns_utc_zulu_timestamp(self):
        assert re.fullmatch(r"\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}Z", collector.now_iso())
