"""Execute the real container runner's reporting tail without a nested target."""

import json
import os
import subprocess
from pathlib import Path

import pytest
import yaml


ROOT = Path(__file__).resolve().parents[2]
TEMPLATE = ROOT / "argo/workflow-templates/run-container-tests.yaml"


def _report(tmp_path, data, behave_rc):
    document = yaml.safe_load(TEMPLATE.read_text(encoding="utf-8"))
    runner = next(t for t in document["spec"]["templates"] if t["name"] == "run-container-tests")
    source = runner["script"]["source"]
    # Run the actual shell tail, including its saved behave status and final exit.
    source = source[source.index("BEHAVE_RC=$(cat /tmp/results/behave-rc.txt"):]
    source = source.replace("/tmp/results", str(tmp_path))
    if data is not None:
        (tmp_path / "results.json").write_text(json.dumps(data), encoding="utf-8")
    (tmp_path / "behave-rc.txt").write_text(str(behave_rc), encoding="utf-8")
    result = subprocess.run(
        ["bash", "-euo", "pipefail", "-c", source],
        env={**os.environ, "QECORE_RC": "0"},
        capture_output=True,
        text=True,
        check=False,
    )
    assert result.returncode == behave_rc, result.stderr
    summary = (tmp_path / "result-summary.txt").read_text(encoding="utf-8")
    failed_path = tmp_path / "failed-scenarios.json"
    failed = json.loads(failed_path.read_text(encoding="utf-8")) if failed_path.exists() else []
    return summary, failed


@pytest.mark.parametrize("behave_rc", [0, 1, 2])
def test_all_skipped_excludes_backgrounds_and_preserves_behave_exit(tmp_path, behave_rc):
    data = [
        {"elements": [{"type": "background"}] + [
            {"type": "scenario", "name": f"Skipped {i}", "status": "skipped"}
            for i in range(10)
        ]},
        {"elements": [{"type": "background", "status": "passed"}] + [
            {"type": "scenario", "name": f"Skipped {i}", "status": "skipped"}
            for i in range(9)
        ]},
    ]

    summary, failed = _report(tmp_path, data, behave_rc)

    assert summary == "0/19 scenarios passed (0 failed, 0 errors, 19 skipped, 0 unexecuted)"
    assert failed == []


def test_mixed_error_and_missing_statuses_never_become_passes(tmp_path):
    elements = [
        {"type": "scenario", "name": status, "status": status}
        for status in (
            "passed", "failed", "error", "hook_error", "cleanup_error", "pending",
            "undefined", "skipped", "untested", "unknown",
        )
    ]
    elements += [
        {"type": "scenario", "name": "No status"},
        {"type": "scenario", "name": "Null status", "status": None},
        {"type": "background", "name": "Background failure", "status": "failed"},
    ]

    summary, failed = _report(tmp_path, [{"elements": elements}], 2)

    assert summary == "1/12 scenarios passed (1 failed, 5 errors, 1 skipped, 4 unexecuted)"
    assert failed == ["failed", "error", "hook_error", "cleanup_error", "pending", "undefined"]


def test_failed_and_errored_names_keep_existing_safety_bounds(tmp_path):
    names = [
        "token value", "Password reset", "Secret value", "Authorization check",
        "bad\nname", "x" * 161, None, "Safe failure", "Safe failure",
    ] + [f"Error {i}" for i in range(25)]
    elements = [
        {"type": "scenario", "name": name, "status": "error" if i % 2 else "failed"}
        for i, name in enumerate(names)
    ]

    _, failed = _report(tmp_path, [{"elements": elements}], 1)

    assert failed == ["Safe failure"] + [f"Error {i}" for i in range(19)]


def test_missing_results_do_not_invent_a_pass(tmp_path):
    summary, failed = _report(tmp_path, None, 1)

    assert summary == "Execution failed"
    assert failed == []
