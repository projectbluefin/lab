"""Contract tests for derived-metric records under docs/data/derived/.

Validates every JSON record against schemas/v2/derived-metric.schema.json and
enforces invariants the schema cannot express (CI bracketing of point estimate,
hero-tile confidence floor, proportion bounds, evidence URL scheme, etc.).

Passes vacuously when no derived files exist yet so the first CI run on this
branch is green before notebooks have produced any output.
"""

from __future__ import annotations

import json
import math
import re
from pathlib import Path

import pytest


SCHEMA_PATH = Path("schemas/v2/derived-metric.schema.json")
DERIVED_ROOT = Path("docs/data/derived")

ALLOWED_METHODS = {
    "wilson_score_95", "wilson_score_99", "clopper_pearson_95",
    "bayesian_online_changepoint", "cusum", "kaplan_meier",
    "fisher_exact", "mann_kendall", "mutual_information",
    "ssim", "pixelmatch", "mad_outlier",
    "reproducible_rebuild_match", "slsa_attestation_walk",
    "raw_proportion",
}

HERO_DERIVED_METRIC_IDS = {
    "trust_window_pass_rate",
    "reproducibility_rate",
    "provenance_completeness",
    "regression_localization_latency",
}

SHA256_RE = re.compile(r"^[0-9a-f]{64}$")


def _discover_derived_files() -> list[Path]:
    if not DERIVED_ROOT.exists():
        return []
    return [
        p for p in sorted(DERIVED_ROOT.rglob("*.json"))
        if not p.name.startswith(".") and p.name != ".gitkeep"
    ]


def _load(path: Path) -> dict:
    return json.loads(path.read_text())


def _iter_metrics(doc: dict):
    for metric in doc.get("metrics", []):
        yield metric


DERIVED_FILES = _discover_derived_files()


def test_schemas_load():
    """derived-metric.schema.json parses and is a valid Draft 2020-12 schema."""
    schema = json.loads(SCHEMA_PATH.read_text())
    assert schema.get("$schema") == "https://json-schema.org/draft/2020-12/schema"
    assert schema.get("type") == "object"
    assert "metrics" in schema["properties"]
    # Method enum in the schema must match the test's allowed set exactly.
    method_enum = set(schema["$defs"]["metric"]["properties"]["method"]["enum"])
    assert method_enum == ALLOWED_METHODS, (
        f"schema method enum drifted from test ALLOWED_METHODS: "
        f"only-in-schema={method_enum - ALLOWED_METHODS}, "
        f"only-in-test={ALLOWED_METHODS - method_enum}"
    )


def test_no_derived_files_okay():
    """Passes vacuously when notebooks have not produced any derived files yet."""
    assert DERIVED_FILES is not None  # discovery succeeded
    if not DERIVED_FILES:
        pytest.skip("no derived files yet; contract holds vacuously")


@pytest.mark.parametrize(
    "path",
    DERIVED_FILES or [pytest.param(None, marks=pytest.mark.skip(reason="no derived files"))],
    ids=lambda p: str(p) if p else "none",
)
def test_every_derived_file_validates_against_schema(path: Path):
    jsonschema = pytest.importorskip("jsonschema")
    from jsonschema import Draft202012Validator

    schema = json.loads(SCHEMA_PATH.read_text())
    Draft202012Validator.check_schema(schema)
    validator = Draft202012Validator(schema)
    doc = _load(path)
    errors = sorted(validator.iter_errors(doc), key=lambda e: list(e.absolute_path))
    assert not errors, "schema violations in {}: {}".format(
        path,
        "; ".join(f"{list(e.absolute_path)}: {e.message}" for e in errors),
    )


@pytest.mark.parametrize(
    "path",
    DERIVED_FILES or [pytest.param(None, marks=pytest.mark.skip(reason="no derived files"))],
    ids=lambda p: str(p) if p else "none",
)
def test_ci_bounds_consistent_with_value(path: Path):
    """ci_lower <= value <= ci_upper when n>0; null trio + unknown/not_attested when n==0."""
    doc = _load(path)
    for m in _iter_metrics(doc):
        mid = m.get("id", "<no-id>")
        n = m["n"]
        if n == 0:
            assert m["value"] is None, f"{path}:{mid} n==0 must have value=null"
            assert m["ci_lower"] is None, f"{path}:{mid} n==0 must have ci_lower=null"
            assert m["ci_upper"] is None, f"{path}:{mid} n==0 must have ci_upper=null"
            assert m["state"] in {"unknown", "not_attested"}, (
                f"{path}:{mid} n==0 state must be unknown|not_attested, got {m['state']}"
            )
        else:
            if m["value"] is None:
                continue
            lo, hi, v = m["ci_lower"], m["ci_upper"], m["value"]
            assert lo is not None and hi is not None, (
                f"{path}:{mid} n>0 with non-null value requires CI bounds"
            )
            # Allow a tiny floating-point tolerance for non-strict CI bounds,
            # e.g. proportion Wilson intervals that land at 0.9999... instead of 1.0.
            tol = 1e-12
            assert (lo - tol) <= v <= (hi + tol), (
                f"{path}:{mid} CI bracket violated: {lo} <= {v} <= {hi}"
            )


@pytest.mark.parametrize(
    "path",
    DERIVED_FILES or [pytest.param(None, marks=pytest.mark.skip(reason="no derived files"))],
    ids=lambda p: str(p) if p else "none",
)
def test_method_in_allowed_set(path: Path):
    doc = _load(path)
    for m in _iter_metrics(doc):
        assert m["method"] in ALLOWED_METHODS, (
            f"{path}:{m.get('id')} method {m['method']!r} not in ALLOWED_METHODS"
        )


@pytest.mark.parametrize(
    "path",
    DERIVED_FILES or [pytest.param(None, marks=pytest.mark.skip(reason="no derived files"))],
    ids=lambda p: str(p) if p else "none",
)
def test_failure_modes_at_least_one(path: Path):
    doc = _load(path)
    for m in _iter_metrics(doc):
        fms = m.get("failure_modes", [])
        assert len(fms) >= 1, f"{path}:{m.get('id')} must declare >=1 failure_mode"


@pytest.mark.parametrize(
    "path",
    DERIVED_FILES or [pytest.param(None, marks=pytest.mark.skip(reason="no derived files"))],
    ids=lambda p: str(p) if p else "none",
)
def test_evidence_urls_https(path: Path):
    doc = _load(path)
    for m in _iter_metrics(doc):
        for ref in m.get("evidence", []):
            assert ref["url"].startswith("https://"), (
                f"{path}:{m.get('id')} evidence url not https: {ref['url']}"
            )
        for fm in m.get("failure_modes", []):
            url = fm.get("evidence_url")
            if url is not None:
                assert url.startswith("https://"), (
                    f"{path}:{m.get('id')} failure_mode {fm.get('id')} evidence_url not https"
                )


@pytest.mark.parametrize(
    "path",
    DERIVED_FILES or [pytest.param(None, marks=pytest.mark.skip(reason="no derived files"))],
    ids=lambda p: str(p) if p else "none",
)
def test_inputs_sha256_format(path: Path):
    doc = _load(path)
    sha = doc["generator"]["inputs_sha256"]
    assert SHA256_RE.match(sha), f"{path} generator.inputs_sha256 not 64-char hex: {sha!r}"


@pytest.mark.parametrize(
    "path",
    DERIVED_FILES or [pytest.param(None, marks=pytest.mark.skip(reason="no derived files"))],
    ids=lambda p: str(p) if p else "none",
)
def test_notebook_url_under_factory_domain(path: Path):
    doc = _load(path)
    url = doc["generator"]["notebook_url"]
    assert url.startswith("https://factory.projectbluefin.io/methods/"), (
        f"{path} generator.notebook_url must be under https://factory.projectbluefin.io/methods/, got {url}"
    )


@pytest.mark.parametrize(
    "path",
    DERIVED_FILES or [pytest.param(None, marks=pytest.mark.skip(reason="no derived files"))],
    ids=lambda p: str(p) if p else "none",
)
def test_confidence_low_metrics_not_in_hero_set(path: Path):
    """Hero-tile metric IDs MUST NOT carry confidence==low."""
    doc = _load(path)
    for m in _iter_metrics(doc):
        mid = m["id"]
        prefix = mid.split(".", 1)[0]
        if prefix in HERO_DERIVED_METRIC_IDS:
            assert m["confidence"] in {"high", "medium"}, (
                f"{path}:{mid} is in HERO set but confidence={m['confidence']!r}; "
                "low-confidence estimates must not be promoted to the headline set"
            )


@pytest.mark.parametrize(
    "path",
    DERIVED_FILES or [pytest.param(None, marks=pytest.mark.skip(reason="no derived files"))],
    ids=lambda p: str(p) if p else "none",
)
def test_unit_proportion_value_in_zero_one(path: Path):
    doc = _load(path)
    tol = 1e-9
    for m in _iter_metrics(doc):
        if m.get("unit") != "proportion":
            continue
        for field in ("value", "ci_lower", "ci_upper"):
            x = m.get(field)
            if x is None:
                continue
            assert (-tol <= x <= 1 + tol) or math.isclose(x, 0, abs_tol=tol) or math.isclose(x, 1, abs_tol=tol), (
                f"{path}:{m.get('id')} unit=proportion {field}={x} outside [0,1]"
            )
